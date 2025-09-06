//! Stream wrapper that provides resumption capabilities.

use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use futures::stream::Stream;
use pin_project::pin_project;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::advanced_features::AdvancedFeatures;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::error::CodexErr;
use crate::error::Result;
use crate::model_provider_info::ModelProviderInfo;
use crate::stream_resumption::context::ResumptionContext;
use crate::stream_resumption::providers::ResumptionProvider;
use crate::stream_resumption::providers::create_provider_resumption;

/// A stream wrapper that provides automatic resumption capabilities.
///
/// This wrapper monitors the underlying stream for errors and automatically
/// attempts to resume from where it left off using provider-specific resumption APIs.
#[pin_project]
pub struct ResumableStream {
    #[pin]
    rx_event: mpsc::Receiver<Result<ResponseEvent>>,
    resumption_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ResumableStream {
    /// Create a new resumable stream wrapper.
    ///
    /// This sets up the resumption infrastructure and starts monitoring
    /// the stream for failures that can be recovered.
    pub fn new(
        stream: ResponseStream,
        provider: &ModelProviderInfo,
        features: &AdvancedFeatures,
    ) -> Self {
        let (tx_event, rx_event) = mpsc::channel(32);

        // Create provider-specific resumption handler
        let resumption_provider = create_provider_resumption(provider, None);

        // Clone data for the resumption task
        let provider_clone = provider.clone();
        let features_clone = features.clone();
        let tx_event_clone = tx_event.clone();

        // Spawn the resumption monitoring task
        let resumption_handle = tokio::spawn(async move {
            Self::monitor_and_resume(
                stream,
                resumption_provider,
                provider_clone,
                features_clone,
                tx_event_clone,
            )
            .await;
        });

        Self {
            rx_event,
            resumption_handle: Some(resumption_handle),
        }
    }

    /// Convert back to a ResponseStream for compatibility.
    pub fn into_response_stream(mut self) -> ResponseStream {
        // Detach the background task so it can continue forwarding events.
        let _ = self.resumption_handle.take();

        // Use mem::replace to avoid moving out of Drop type
        let (dummy_tx, dummy_rx) = tokio::sync::mpsc::channel(32);
        let old_rx = std::mem::replace(&mut self.rx_event, dummy_rx);
        // Drop the dummy channel since we're returning the old one
        drop(dummy_tx);

        ResponseStream { rx_event: old_rx }
    }

    /// Main monitoring loop that handles resumption logic.
    async fn monitor_and_resume(
        mut stream: ResponseStream,
        resumption_provider: ResumptionProvider,
        provider: ModelProviderInfo,
        features: AdvancedFeatures,
        tx_event: mpsc::Sender<Result<ResponseEvent>>,
    ) {
        let mut resumption_ctx: Option<ResumptionContext> = None;
        // Check TEST_DISABLE_RETRIES environment variable
        let max_attempts = if std::env::var("TEST_DISABLE_RETRIES").is_ok() {
            0
        } else {
            resumption_provider.max_resume_attempts()
        };

        // Check if provider supports resumption
        if !resumption_provider.supports_resumption() || max_attempts == 0 {
            debug!(
                "Provider does not support stream resumption or retries disabled, using passthrough mode"
            );
            Self::passthrough_stream(stream, tx_event).await;
            return;
        }

        debug!(
            "Starting stream monitoring with resumption support (max_attempts={})",
            max_attempts
        );

        loop {
            match stream.rx_event.recv().await {
                Some(Ok(event)) => {
                    // Track resumption info from successful events
                    if let Some(ref mut ctx) = resumption_ctx {
                        resumption_provider.extract_resumption_info(&event, ctx);
                    } else if let ResponseEvent::Completed { response_id, .. } = &event {
                        // Initialize resumption context when we get a response ID
                        resumption_ctx =
                            Some(ResumptionContext::new(response_id.clone(), max_attempts));
                        debug!(
                            "Initialized resumption context with response_id: {}",
                            response_id
                        );
                    }

                    // Forward the event
                    if tx_event.send(Ok(event)).await.is_err() {
                        debug!("Receiver dropped, stopping stream monitoring");
                        break;
                    }
                }
                Some(Err(error)) => {
                    // Check if this error is resumable
                    if let Some(ref mut ctx) = resumption_ctx {
                        if resumption_provider.is_resumable_error(&error) && ctx.can_retry() {
                            info!(
                                "Attempting stream resumption (attempt {}/{}) due to error: {}",
                                ctx.attempt_count + 1,
                                ctx.max_attempts,
                                error
                            );

                            match Self::attempt_resume(
                                &resumption_provider,
                                ctx,
                                &provider,
                                &features,
                            )
                            .await
                            {
                                Ok(new_stream) => {
                                    ctx.increment_attempt();
                                    stream = new_stream;
                                    continue; // Resume streaming with new stream
                                }
                                Err(resume_error) => {
                                    warn!("Stream resumption failed: {}", resume_error);
                                    // Fall through to forward the original error
                                }
                            }
                        } else {
                            debug!("Error not resumable or max attempts reached: {}", error);
                        }
                    } else {
                        debug!("No resumption context available for error: {}", error);
                    }

                    // Forward the error (resumption failed or not applicable)
                    if tx_event.send(Err(error)).await.is_err() {
                        debug!("Receiver dropped during error forwarding");
                    }
                    break; // Stop on error
                }
                None => {
                    // Stream ended normally
                    debug!("Stream ended normally");
                    break;
                }
            }
        }

        debug!("Stream monitoring completed");
    }

    /// Attempt to resume the stream from the last known good position.
    async fn attempt_resume(
        resumption_provider: &ResumptionProvider,
        context: &ResumptionContext,
        _provider: &ModelProviderInfo,
        _features: &AdvancedFeatures,
    ) -> Result<ResponseStream> {
        // Wait before attempting resume (exponential backoff)
        let delay = resumption_provider.resume_delay(context.attempt_count);
        debug!("Waiting {:?} before resume attempt", delay);
        sleep(delay).await;

        // Create the resume request
        let resume_request = resumption_provider
            .create_resume_request(context, &serde_json::json!({}))
            .await?;

        debug!("Created resume request: {:?}", resume_request.url());

        // Execute the resume request and create a new stream
        debug!("Executing resume request to: {}", resume_request.url());

        let client = reqwest::Client::new();
        let response = client.execute(resume_request).await.map_err(|e| {
            CodexErr::Stream(format!("Failed to execute resume request: {e}"), None)
        })?;

        if !response.status().is_success() {
            return Err(CodexErr::Stream(
                format!("Resume request failed with status: {}", response.status()),
                None,
            ));
        }

        // Convert the SSE response back to ResponseStream
        let (tx, rx) = mpsc::channel(32);

        // Spawn a task to parse SSE events from the response stream
        let bytes_stream = response.bytes_stream();
        tokio::spawn(async move {
            let result = Self::parse_sse_stream(bytes_stream, tx.clone()).await;
            if let Err(e) = result {
                debug!("SSE parsing error: {}", e);
                let _ = tx
                    .send(Err(CodexErr::Stream(
                        format!("SSE parsing failed: {e}"),
                        None,
                    )))
                    .await;
            }
        });

        Ok(ResponseStream { rx_event: rx })
    }

    /// Parse Server-Sent Events from a byte stream and convert to ResponseEvents.
    async fn parse_sse_stream(
        mut bytes_stream: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin + Send,
        tx: mpsc::Sender<Result<ResponseEvent>>,
    ) -> Result<()> {
        use futures::StreamExt;

        let mut buffer = String::new();

        while let Some(chunk) = bytes_stream.next().await {
            let chunk =
                chunk.map_err(|e| CodexErr::Stream(format!("Stream read error: {e}"), None))?;
            let text = String::from_utf8_lossy(&chunk);

            // Append new chunk to buffer
            buffer.push_str(&text);

            // Process complete lines in the buffer
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer.drain(..=newline_pos).collect::<String>();

                if line.trim().starts_with("data:") {
                    // Extract JSON from the data: line
                    let json_str = line.trim()["data:".len()..].trim();

                    // Try to parse as SSE event
                    match serde_json::from_str::<serde_json::Value>(json_str) {
                        Ok(raw_event) => {
                            // Convert to ResponseEvent using similar logic from client.rs
                            if let Some(event_type) = raw_event.get("type").and_then(|v| v.as_str())
                            {
                                let response_event =
                                    Self::map_sse_to_response_event(event_type, &raw_event)?;
                                if tx.send(Ok(response_event)).await.is_err() {
                                    debug!("Receiver dropped during SSE parsing");
                                    return Ok(());
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Failed to parse SSE JSON: {}, data: {}", e, json_str);
                            continue;
                        }
                    }
                } else if line.trim().is_empty() {
                    // End of SSE event
                    continue;
                }
            }
        }

        Ok(())
    }

    /// Map SSE event JSON to ResponseEvent enum.
    fn map_sse_to_response_event(
        event_type: &str,
        raw_event: &serde_json::Value,
    ) -> Result<ResponseEvent> {
        let event = match event_type {
            "response.created" => ResponseEvent::Created,
            "response.queued" => ResponseEvent::Queued,
            "response.in_progress" => ResponseEvent::InProgress,
            "response.completed" => {
                let id = raw_event
                    .get("response")
                    .and_then(|r| r.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                ResponseEvent::Completed {
                    response_id: id,
                    token_usage: None,
                }
            }
            "response.failed" => ResponseEvent::Failed(raw_event.clone()),
            "error" => ResponseEvent::Error(raw_event.clone()),
            "response.incomplete" => {
                let id = raw_event
                    .get("response")
                    .and_then(|r| r.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                ResponseEvent::Incomplete {
                    response_id: id,
                    _reason: None,
                }
            }
            "response.output_text.delta" => {
                if let Some(delta) = raw_event.get("delta").and_then(|v| v.as_str()) {
                    ResponseEvent::OutputTextDelta(delta.to_string())
                } else {
                    return Err(CodexErr::Stream(
                        "Missing delta in output_text.delta".to_string(),
                        None,
                    ));
                }
            }
            "response.output_item.done" => {
                if let Some(item) = raw_event.get("item") {
                    let response_item = serde_json::from_value(item.clone()).map_err(|e| {
                        CodexErr::Stream(format!("Failed to parse ResponseItem: {e}"), None)
                    })?;
                    ResponseEvent::OutputItemDone(response_item)
                } else {
                    return Err(CodexErr::Stream(
                        "Missing item in output_item.done".to_string(),
                        None,
                    ));
                }
            }
            _ => {
                // Handle unknown events
                ResponseEvent::Unknown {
                    event_type: event_type.to_string(),
                    payload: raw_event.clone(),
                }
            }
        };

        Ok(event)
    }

    /// Passthrough mode that forwards events without resumption logic.
    async fn passthrough_stream(
        mut stream: ResponseStream,
        tx_event: mpsc::Sender<Result<ResponseEvent>>,
    ) {
        while let Some(event) = stream.rx_event.recv().await {
            if tx_event.send(event).await.is_err() {
                debug!("Receiver dropped in passthrough mode");
                break;
            }
        }
    }
}

impl Stream for ResumableStream {
    type Item = Result<ResponseEvent>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        this.rx_event.poll_recv(cx)
    }
}

// Drop is handled automatically by pin_project
// The background task will be aborted when ResumableStream is dropped

/// Type alias for the resumable response stream.
pub type ResumableResponseStream = ResumableStream;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advanced_features::AdvancedFeatures;
    use crate::model_provider_info::ModelProviderInfo;
    use crate::model_provider_info::WireApi;
    use tokio::sync::mpsc;

    fn create_test_provider() -> ModelProviderInfo {
        ModelProviderInfo {
            name: "Test Provider".to_string(),
            base_url: Some("https://api.test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            auth_type: Default::default(),
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(3),
            stream_max_retries: Some(0), // No resumption support
            stream_idle_timeout_ms: Some(30000),
            requires_openai_auth: false,
        }
    }

    fn create_test_stream() -> (mpsc::Sender<Result<ResponseEvent>>, ResponseStream) {
        let (tx, rx) = mpsc::channel(10);
        (tx, ResponseStream { rx_event: rx })
    }

    #[tokio::test]
    async fn test_resumable_stream_creation() {
        let provider = create_test_provider();
        let features = AdvancedFeatures::default();
        let (_tx, stream) = create_test_stream();

        let resumable = ResumableStream::new(stream, &provider, &features);

        // Should be able to convert back to ResponseStream
        let _response_stream = resumable.into_response_stream();
    }

    #[tokio::test]
    async fn test_passthrough_mode() {
        let provider = create_test_provider(); // No resumption support
        let features = AdvancedFeatures {
            enable_stream_resumption: true,
            ..Default::default()
        };

        // Create a test stream with a single event
        let (tx, rx) = mpsc::channel(10);
        let test_stream = ResponseStream { rx_event: rx };

        // Send a test event
        let test_event = ResponseEvent::Created;
        tx.send(Ok(test_event)).await.unwrap();
        drop(tx); // Close the stream

        let mut resumable = ResumableStream::new(test_stream, &provider, &features);

        // Should receive the event in passthrough mode
        let received = resumable.rx_event.recv().await;
        assert!(received.is_some());
        match received.unwrap() {
            Ok(ResponseEvent::Created) => (), // Expected
            other => panic!("Unexpected event: {other:?}"),
        }

        // Stream should end normally
        let end = resumable.rx_event.recv().await;
        assert!(end.is_none());
    }
}
