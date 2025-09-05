use std::io::BufRead;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use crate::AuthManager;
use bytes::Bytes;
use codex_protocol::mcp_protocol::AuthMode;
use eventsource_stream::Eventsource;
use futures::prelude::*;
use regex_lite::Regex;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::io::ReaderStream;
use tracing::debug;
use tracing::trace;
use tracing::warn;
use uuid::Uuid;

use crate::chat_completions::AggregateStreamExt;
use crate::chat_completions::stream_chat_completions;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::client_common::ResponsesApiRequest;
use crate::client_common::create_reasoning_param_for_request;
use crate::client_common::create_text_param_for_request;
use crate::config::Config;
use crate::default_client::create_client;
use crate::default_client::get_codex_user_agent;
use crate::error::CodexErr;
use crate::error::Result;
use crate::error::UsageLimitReachedError;
use crate::flags::CODEX_RS_SSE_FIXTURE;
use crate::model_family::ModelFamily;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::WireApi;
use crate::openai_model_info::get_model_info;
use crate::openai_tools::create_tools_json_for_responses_api;
use crate::protocol::TokenUsage;
use crate::util::backoff;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::ResponseItem;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: Error,
}

#[derive(Debug, Deserialize)]
struct Error {
    r#type: Option<String>,
    #[allow(dead_code)]
    code: Option<String>,
    message: Option<String>,

    // Optional fields available on "usage_limit_reached" and "usage_not_included" errors
    plan_type: Option<String>,
    resets_in_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ModelClient {
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    client: reqwest::Client,
    provider: ModelProviderInfo,
    session_id: Uuid,
    effort: ReasoningEffortConfig,
    summary: ReasoningSummaryConfig,
    /// Tracks the last completed `response_id` for chaining when server-side
    /// storage is enabled (e.g., Azure Responses API).
    last_response_id: Arc<tokio::sync::Mutex<Option<String>>>,
    /// Whether the last completed response was stored server-side.
    last_response_was_stored: Arc<tokio::sync::Mutex<bool>>,
    /// Fingerprint of the provider/model context used for the last response id.
    last_provider_fingerprint: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl ModelClient {
    pub fn new(
        config: Arc<Config>,
        auth_manager: Option<Arc<AuthManager>>,
        provider: ModelProviderInfo,
        effort: ReasoningEffortConfig,
        summary: ReasoningSummaryConfig,
        session_id: Uuid,
    ) -> Self {
        let client = create_client(&config.responses_originator_header);

        Self {
            config,
            auth_manager,
            client,
            provider,
            session_id,
            effort,
            summary,
            last_response_id: Arc::new(tokio::sync::Mutex::new(None)),
            last_response_was_stored: Arc::new(tokio::sync::Mutex::new(false)),
            last_provider_fingerprint: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    pub fn get_model_context_window(&self) -> Option<u64> {
        self.config
            .model_context_window
            .or_else(|| get_model_info(&self.config.model_family).map(|info| info.context_window))
    }

    /// Dispatches to either the Responses or Chat implementation depending on
    /// the provider config.  Public callers always invoke `stream()` – the
    /// specialised helpers are private to avoid accidental misuse.
    pub async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
        match self.provider.wire_api {
            WireApi::Responses => self.stream_responses(prompt).await,
            WireApi::Chat => {
                // Create the raw streaming connection first.
                let response_stream = stream_chat_completions(
                    prompt,
                    &self.config.model_family,
                    &self.client,
                    &self.provider,
                )
                .await?;

                // Wrap it with the aggregation adapter so callers see *only*
                // the final assistant message per turn (matching the
                // behaviour of the Responses API).
                let mut aggregated = if self.config.show_raw_agent_reasoning {
                    crate::chat_completions::AggregatedChatStream::streaming_mode(response_stream)
                } else {
                    response_stream.aggregate()
                };

                // Bridge the aggregated stream back into a standard
                // `ResponseStream` by forwarding events through a channel.
                let (tx, rx) = mpsc::channel::<Result<ResponseEvent>>(16);

                tokio::spawn(async move {
                    use futures::StreamExt;
                    while let Some(ev) = aggregated.next().await {
                        // Exit early if receiver hung up.
                        if tx.send(ev).await.is_err() {
                            break;
                        }
                    }
                });

                // Intercept Completed events to capture the response id (if any).
                let (tx2, rx2) = mpsc::channel::<Result<ResponseEvent>>(16);
                let last_id = self.last_response_id.clone();
                tokio::spawn(async move {
                    let mut rx = rx;
                    while let Some(ev) = rx.recv().await {
                        if let Ok(ResponseEvent::Completed { response_id, .. }) = &ev {
                            let mut guard = last_id.lock().await;
                            if !response_id.is_empty() {
                                *guard = Some(response_id.clone());
                            }
                        }
                        if tx2.send(ev).await.is_err() {
                            break;
                        }
                    }
                });
                Ok(ResponseStream { rx_event: rx2 })
            }
        }
    }

    /// Implementation for the OpenAI *Responses* experimental API.
    async fn stream_responses(&self, prompt: &Prompt) -> Result<ResponseStream> {
        if let Some(path) = &*CODEX_RS_SSE_FIXTURE {
            // short circuit for tests
            warn!(path, "Streaming from fixture");
            return stream_from_fixture(path, self.provider.clone()).await;
        }

        let auth_manager = self.auth_manager.clone();

        let auth_mode = auth_manager
            .as_ref()
            .and_then(|m| m.auth())
            .as_ref()
            .map(|a| a.mode);

        // Background streaming requires `store=true` on Azure. If storage is
        // disabled via auth mode (e.g., ChatGPT), we may need to force it on
        // when background streaming is enabled.
        let mut store = prompt.store && auth_mode != Some(AuthMode::ChatGPT);

        let full_instructions = prompt.get_full_instructions(&self.config.model_family);
        let tools_json = create_tools_json_for_responses_api(&prompt.tools)?;
        let input_with_instructions = prompt.get_formatted_input();
        // compute static inputs for payload construction outside the retry loop
        let base_instructions_ref = full_instructions.clone();
        let input_ref = input_with_instructions.clone();
        let tools_ref = tools_json.clone();

        // Request encrypted COT if we are not storing responses,
        // otherwise reasoning items will be referenced by ID
        let include: Vec<String> =
            if !store && self.config.model_family.supports_reasoning_summaries {
                vec!["reasoning.encrypted_content".to_string()]
            } else {
                vec![]
            };

        // Only include `text.verbosity` for GPT-5 family models
        // verbosity/text control computed per attempt (cheap) based on family
        // and config; reasoning is also recomputed each attempt.

        // Compute a provider fingerprint (base_url + query params + model) so
        // we only chain across consistent contexts.
        let current_provider_fingerprint = {
            let mut s = String::new();
            if let Some(b) = &self.provider.base_url {
                s.push_str(b);
            }
            if let Some(q) = &self.provider.query_params {
                let mut kv: Vec<_> = q.iter().collect();
                kv.sort_by(|a, b| a.0.cmp(b.0));
                for (k, v) in kv {
                    s.push('|');
                    s.push_str(k);
                    s.push('=');
                    s.push_str(v);
                }
            }
            s.push('|');
            s.push_str(&self.config.model);
            s
        };

        // Decide whether chaining is allowed for this request.
        // Per spec, rely on previous_response_id only when responses are stored.
        let allow_chain = store;

        // Candidate previous id (from prompt override or last tracked id).
        let candidate_prev_id = if allow_chain {
            prompt
                .previous_response_id
                .clone()
                .or(self.last_response_id.lock().await.clone())
        } else {
            None
        };

        // debug logging removed

        let background = match std::env::var("CODEX_ENABLE_BACKGROUND") {
            Ok(v) if v == "1" => Some(true),
            _ => None,
        };

        if background == Some(true) && !store {
            tracing::warn!(
                "background=true requires store=true; forcing store=true for this request"
            );
            store = true;
        }

        // One-shot fallback toggle for Azure chaining errors.
        let mut force_no_previous_id = false;
        let mut tried_chain_fallback = false;

        let mut attempt = 0;
        let max_retries = self.provider.request_max_retries();

        loop {
            attempt += 1;

            // Always fetch the latest auth in case a prior attempt refreshed the token.
            let auth = auth_manager.as_ref().and_then(|m| m.auth());

            // Build request payload for this attempt (allows toggling prev id).
            let effective_prev_id = if force_no_previous_id {
                None
            } else {
                candidate_prev_id.clone()
            };
            let reasoning = create_reasoning_param_for_request(
                &self.config.model_family,
                self.effort,
                self.summary,
            );
            let text = if self.config.model_family.family == "gpt-5" {
                create_text_param_for_request(self.config.model_verbosity)
            } else {
                if self.config.model_verbosity.is_some() {
                    warn!(
                        "model_verbosity is set but ignored for non-gpt-5 model family: {}",
                        self.config.model_family.family
                    );
                }
                None
            };
            let payload = ResponsesApiRequest {
                model: &self.config.model,
                instructions: &base_instructions_ref,
                input: &input_ref,
                tools: &tools_ref,
                tool_choice: "auto",
                parallel_tool_calls: false,
                reasoning,
                store,
                stream: true,
                include: include.clone(),
                previous_response_id: effective_prev_id,
                background,
                prompt_cache_key: Some(self.session_id.to_string()),
                text,
            };

            // payload dump removed

            trace!(
                "POST to {}: {}",
                self.provider.get_full_url(&auth),
                serde_json::to_string(&payload)?
            );

            let mut req_builder = self
                .provider
                .create_request_builder(&self.client, &auth)
                .await?;

            if !self.provider.is_probably_azure() {
                req_builder = req_builder.header("OpenAI-Beta", "responses=experimental");
            }
            req_builder = req_builder
                .header("session_id", self.session_id.to_string())
                .header(reqwest::header::ACCEPT, "text/event-stream")
                .json(&payload);

            if let Some(auth) = auth.as_ref()
                && auth.mode == AuthMode::ChatGPT
                && let Some(account_id) = auth.get_account_id()
            {
                req_builder = req_builder.header("chatgpt-account-id", account_id);
            }

            let res = req_builder.send().await;
            if let Ok(resp) = &res {
                trace!(
                    "Response status: {}, request-id: {}",
                    resp.status(),
                    resp.headers()
                        .get("x-request-id")
                        .map(|v| v.to_str().unwrap_or_default())
                        .unwrap_or_default()
                );
            }

            match res {
                Ok(resp) if resp.status().is_success() => {
                    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

                    let is_sse = resp
                        .headers()
                        .get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .map(|ct| ct.starts_with("text/event-stream"))
                        .unwrap_or(false);

                    if is_sse {
                        // spawn task to process SSE (with optional Azure resume context)
                        let stream = resp.bytes_stream().map_err(CodexErr::Reqwest);
                        // For Azure, enable resume for all streaming responses.
                        let resume_ctx = if self.provider.is_probably_azure() {
                            Some(ResumeCtx {
                                client: self.client.clone(),
                                provider: self.provider.clone(),
                                auth_manager: self.auth_manager.clone(),
                                background: background.unwrap_or(false),
                            })
                        } else {
                            None
                        };
                        tokio::spawn(process_sse(
                            Box::pin(stream),
                            tx_event,
                            self.provider.stream_idle_timeout(),
                            resume_ctx,
                        ));

                        // Intercept Completed to track response id for chaining.
                        let (tx2, rx2) = mpsc::channel::<Result<ResponseEvent>>(1600);
                        let last_id = self.last_response_id.clone();
                        let last_was_stored = self.last_response_was_stored.clone();
                        let last_fp = self.last_provider_fingerprint.clone();
                        let provider_fp_for_this_turn = current_provider_fingerprint.clone();
                        let stored_flag_for_this_turn = store;
                        tokio::spawn(async move {
                            let mut rx = rx_event;
                            while let Some(ev) = rx.recv().await {
                                if let Ok(ResponseEvent::Completed { response_id, .. }) = &ev {
                                    let mut guard = last_id.lock().await;
                                    if !response_id.is_empty() {
                                        *guard = Some(response_id.clone());
                                    }
                                    // Record chain safety info for next turn.
                                    *last_was_stored.lock().await = stored_flag_for_this_turn;
                                    *last_fp.lock().await = Some(provider_fp_for_this_turn.clone());
                                }
                                if tx2.send(ev).await.is_err() {
                                    break;
                                }
                            }
                        });

                        return Ok(ResponseStream { rx_event: rx2 });
                    }

                    // Not SSE: background envelope -> poll GET /responses/{id}
                    let v: serde_json::Value = match resp.json().await {
                        Ok(v) => v,
                        Err(e) => {
                            let _ = tx_event
                                .send(Err(CodexErr::Stream(
                                    format!("invalid JSON response: {e}"),
                                    None,
                                )))
                                .await;
                            return Ok(ResponseStream { rx_event });
                        }
                    };
                    let id = v
                        .get("response")
                        .and_then(|r| r.get("id"))
                        .or_else(|| v.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if id.is_empty() {
                        let _ = tx_event
                            .send(Err(CodexErr::Stream(
                                "missing id in background response".to_string(),
                                None,
                            )))
                            .await;
                        return Ok(ResponseStream { rx_event });
                    }
                    let manager = BackgroundTaskManager::new(
                        self.client.clone(),
                        self.provider.clone(),
                        self.auth_manager.clone(),
                    );

                    // Emit Created before polling begins to mirror SSE behavior.
                    let _ = tx_event.send(Ok(ResponseEvent::Created)).await;

                    // Start forwarder BEFORE starting the poller, so events get relayed immediately.
                    let (tx2, rx2) = mpsc::channel::<Result<ResponseEvent>>(1600);
                    let last_id = self.last_response_id.clone();
                    let last_was_stored = self.last_response_was_stored.clone();
                    let last_fp = self.last_provider_fingerprint.clone();
                    let provider_fp_for_this_turn = current_provider_fingerprint.clone();
                    let stored_flag_for_this_turn = store;
                    tokio::spawn(async move {
                        let mut rx = rx_event;
                        while let Some(ev) = rx.recv().await {
                            if let Ok(ResponseEvent::Completed { response_id, .. }) = &ev {
                                let mut guard = last_id.lock().await;
                                if !response_id.is_empty() {
                                    *guard = Some(response_id.clone());
                                }
                                *last_was_stored.lock().await = stored_flag_for_this_turn;
                                *last_fp.lock().await = Some(provider_fp_for_this_turn.clone());
                            }
                            if tx2.send(ev).await.is_err() {
                                break;
                            }
                        }
                    });

                    // Spawn poller asynchronously.
                    tokio::spawn(manager.poll_until_complete(id, tx_event.clone()));

                    return Ok(ResponseStream { rx_event: rx2 });
                }
                Ok(res) => {
                    let status = res.status();

                    // Pull out Retry‑After header if present.
                    let retry_after_secs = res
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok());

                    if status == StatusCode::UNAUTHORIZED
                        && let Some(manager) = auth_manager.as_ref()
                        && manager.auth().is_some()
                    {
                        let _ = manager.refresh_token().await;
                    }

                    // The OpenAI Responses endpoint returns structured JSON bodies even for 4xx/5xx
                    // errors. When we bubble early with only the HTTP status the caller sees an opaque
                    // "unexpected status 400 Bad Request" which makes debugging nearly impossible.
                    // Instead, read (and include) the response text so higher layers and users see the
                    // exact error message (e.g. "Unknown parameter: 'input[0].metadata'"). The body is
                    // small and this branch only runs on error paths so the extra allocation is
                    // negligible.
                    if !(status == StatusCode::TOO_MANY_REQUESTS
                        || status == StatusCode::UNAUTHORIZED
                        || status.is_server_error())
                    {
                        let body = res.text().await.unwrap_or_default();
                        if self.provider.is_probably_azure() {
                            // If Azure rejected chaining, retry once without previous_response_id.
                            if candidate_prev_id.is_some()
                                && !tried_chain_fallback
                                && azure_previous_response_not_found(&body)
                            {
                                tried_chain_fallback = true;
                                force_no_previous_id = true;
                                // small delay to avoid hammering
                                tokio::time::sleep(Duration::from_millis(50)).await;
                                continue;
                            }
                            let msg = parse_azure_error_message(&body).unwrap_or(body);
                            return Err(CodexErr::UnexpectedStatus(status, msg));
                        }
                        return Err(CodexErr::UnexpectedStatus(status, body));
                    }

                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let body = res.json::<ErrorResponse>().await.ok();
                        if let Some(ErrorResponse { error }) = body {
                            if error.r#type.as_deref() == Some("usage_limit_reached") {
                                // Prefer the plan_type provided in the error message if present
                                // because it's more up to date than the one encoded in the auth
                                // token.
                                let plan_type = error
                                    .plan_type
                                    .or_else(|| auth.and_then(|a| a.get_plan_type()));
                                let resets_in_seconds = error.resets_in_seconds;
                                return Err(CodexErr::UsageLimitReached(UsageLimitReachedError {
                                    plan_type,
                                    resets_in_seconds,
                                }));
                            } else if error.r#type.as_deref() == Some("usage_not_included") {
                                return Err(CodexErr::UsageNotIncluded);
                            }
                        }
                    }

                    if attempt > max_retries {
                        if status == StatusCode::INTERNAL_SERVER_ERROR {
                            return Err(CodexErr::InternalServerError);
                        }

                        return Err(CodexErr::RetryLimit(status));
                    }

                    let delay = retry_after_secs
                        .map(|s| Duration::from_millis(s * 1_000))
                        .unwrap_or_else(|| backoff(attempt));
                    tokio::time::sleep(delay).await;
                }
                Err(e) => {
                    if attempt > max_retries {
                        return Err(e.into());
                    }
                    let delay = backoff(attempt);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    pub fn get_provider(&self) -> ModelProviderInfo {
        self.provider.clone()
    }

    /// Returns the currently configured model slug.
    pub fn get_model(&self) -> String {
        self.config.model.clone()
    }

    /// Returns the currently configured model family.
    pub fn get_model_family(&self) -> ModelFamily {
        self.config.model_family.clone()
    }

    /// Returns the current reasoning effort setting.
    pub fn get_reasoning_effort(&self) -> ReasoningEffortConfig {
        self.effort
    }

    /// Returns the current reasoning summary setting.
    pub fn get_reasoning_summary(&self) -> ReasoningSummaryConfig {
        self.summary
    }

    pub fn get_auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }

    /// Attempts to cancel a background response.
    /// - Azure: `POST /v1/responses/{id}/cancel`
    /// - Others: `DELETE /v1/responses/{id}`
    pub async fn cancel_background(&self, response_id: &str) -> Result<()> {
        let auth = self.auth_manager.as_ref().and_then(|m| m.auth());
        let base_url = self.provider.get_response_url(&auth, response_id);
        let (use_post, url) = if self.provider.is_probably_azure() {
            if let Some((path, qs)) = base_url.split_once('?') {
                (true, format!("{path}/cancel?{qs}"))
            } else {
                (true, format!("{base_url}/cancel"))
            }
        } else {
            (false, base_url)
        };

        let mut req = if use_post {
            self.client.post(url)
        } else {
            self.client.delete(url)
        };
        if let Some(auth) = auth.as_ref() {
            match self.provider.auth_type {
                crate::model_provider_info::AuthType::ApiKey => {
                    req = req.header("api-key", auth.get_token().await?);
                }
                _ => {
                    req = req.bearer_auth(auth.get_token().await?);
                }
            }
        }
        req = self.provider.apply_http_headers(req);
        let resp = req.send().await.map_err(CodexErr::Reqwest)?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let msg = if self.provider.is_probably_azure() {
                parse_azure_error_message(&body).unwrap_or(body)
            } else {
                body
            };
            Err(CodexErr::UnexpectedStatus(status, msg))
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct SseEvent {
    #[serde(rename = "type")]
    kind: String,
    response: Option<Value>,
    item: Option<Value>,
    delta: Option<String>,
    /// Monotonic sequence number for resuming background streams (Azure).
    #[serde(default)]
    sequence_number: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ResponseCreated {}

#[derive(Debug, Deserialize)]
struct ResponseCompleted {
    id: String,
    usage: Option<ResponseCompletedUsage>,
}

#[derive(Debug, Deserialize, Clone)]
struct ResponseCompletedUsage {
    input_tokens: u64,
    input_tokens_details: Option<ResponseCompletedInputTokensDetails>,
    output_tokens: u64,
    output_tokens_details: Option<ResponseCompletedOutputTokensDetails>,
    total_tokens: u64,
}

impl From<ResponseCompletedUsage> for TokenUsage {
    fn from(val: ResponseCompletedUsage) -> Self {
        TokenUsage {
            input_tokens: val.input_tokens,
            cached_input_tokens: val.input_tokens_details.map(|d| d.cached_tokens),
            output_tokens: val.output_tokens,
            reasoning_output_tokens: val.output_tokens_details.map(|d| d.reasoning_tokens),
            total_tokens: val.total_tokens,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct ResponseCompletedInputTokensDetails {
    cached_tokens: u64,
}

#[derive(Debug, Deserialize, Clone)]
struct ResponseCompletedOutputTokensDetails {
    reasoning_tokens: u64,
}

/// Minimal background task poller for providers (e.g., Azure) that process
/// Responses asynchronously when `background: true` is requested.
struct BackgroundTaskManager {
    client: reqwest::Client,
    provider: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
}

impl BackgroundTaskManager {
    fn new(
        client: reqwest::Client,
        provider: ModelProviderInfo,
        auth_manager: Option<Arc<AuthManager>>,
    ) -> Self {
        Self {
            client,
            provider,
            auth_manager,
        }
    }

    async fn poll_until_complete(
        self,
        response_id: String,
        tx_event: mpsc::Sender<Result<ResponseEvent>>,
    ) {
        let mut attempt = 0u64;
        let start = Instant::now();

        loop {
            attempt += 1;
            let auth = self.auth_manager.as_ref().and_then(|m| m.auth());
            let url = self.provider.get_response_url(&auth, &response_id);

            let mut req = self.client.get(&url);
            if let Some(auth) = auth.as_ref() {
                // Use the same header semantics as POST.
                match self.provider.auth_type {
                    crate::model_provider_info::AuthType::ApiKey => match auth.get_token().await {
                        Ok(t) => {
                            req = req.header("api-key", t);
                        }
                        Err(e) => {
                            let _ = tx_event.send(Err(e.into())).await;
                            return;
                        }
                    },
                    _ => match auth.get_token().await {
                        Ok(t) => {
                            req = req.bearer_auth(t);
                        }
                        Err(e) => {
                            let _ = tx_event.send(Err(e.into())).await;
                            return;
                        }
                    },
                }
            }
            req = self.provider.apply_http_headers(req);
            // Mirror important POST headers for consistency.
            req = req
                .header("originator", get_codex_user_agent(None))
                .header("User-Agent", get_codex_user_agent(None));

            let res = req.send().await;
            match res {
                Ok(resp) if resp.status().is_success() => {
                    let v: serde_json::Value = match resp.json().await {
                        Ok(v) => v,
                        Err(e) => {
                            let _ = tx_event
                                .send(Err(CodexErr::Stream(
                                    format!("invalid JSON from poll: {e}"),
                                    None,
                                )))
                                .await;
                            return;
                        }
                    };
                    let r = v.get("response").cloned().unwrap_or(v);
                    let status = r.get("status").and_then(|v| v.as_str()).unwrap_or("");
                    match status {
                        "queued" | "in_progress" => {}
                        "completed" => {
                            // Emit output items, then Completed
                            if let Some(items) = r.get("output").and_then(|o| o.as_array()) {
                                for it in items {
                                    if let Ok(item) =
                                        serde_json::from_value::<ResponseItem>(it.clone())
                                        && tx_event
                                            .send(Ok(ResponseEvent::OutputItemDone(item)))
                                            .await
                                            .is_err()
                                    {
                                        return;
                                    }
                                }
                            }
                            let usage = r
                                .get("usage")
                                .cloned()
                                .and_then(|u| {
                                    serde_json::from_value::<ResponseCompletedUsage>(u).ok()
                                })
                                .map(TokenUsage::from);
                            let _ = tx_event
                                .send(Ok(ResponseEvent::Completed {
                                    response_id: response_id.clone(),
                                    token_usage: usage,
                                }))
                                .await;
                            return;
                        }
                        "failed" | "canceled" | "cancelled" => {
                            // Surface the error message when available.
                            let msg = r
                                .get("error")
                                .and_then(|e| e.get("message"))
                                .and_then(|m| m.as_str())
                                .unwrap_or("background task failed");
                            let _ = tx_event
                                .send(Err(CodexErr::Stream(msg.to_string(), None)))
                                .await;
                            return;
                        }
                        other => {
                            let _ = tx_event
                                .send(Err(CodexErr::Stream(
                                    format!("unknown background status: {other}"),
                                    None,
                                )))
                                .await;
                            return;
                        }
                    }

                    // Backoff between polls with a small cap to keep tests snappy.
                    let delay = backoff(attempt).min(Duration::from_millis(300));
                    tokio::time::sleep(delay).await;
                }
                Ok(resp) => {
                    // Non-success – attempt to extract a friendly message.
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    let msg = if self.provider.is_probably_azure() {
                        parse_azure_error_message(&body).unwrap_or(body)
                    } else {
                        body
                    };
                    let _ = tx_event
                        .send(Err(CodexErr::UnexpectedStatus(status, msg)))
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = tx_event.send(Err(CodexErr::Reqwest(e))).await;
                    return;
                }
            }

            if start.elapsed() > Duration::from_secs(300) {
                let _ = tx_event
                    .send(Err(CodexErr::Stream(
                        "background task timed out (5m)".into(),
                        None,
                    )))
                    .await;
                return;
            }
        }
    }
}

fn parse_azure_error_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let err = v.get("error")?;
    let code = err.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let msg = err
        .get("message")
        .and_then(|v| v.as_str())
        .or_else(|| {
            err.get("innererror")
                .and_then(|ie| ie.get("message").and_then(|m| m.as_str()))
        })
        .unwrap_or("");
    if code.is_empty() && msg.is_empty() {
        None
    } else if code.is_empty() {
        Some(msg.to_string())
    } else if msg.is_empty() {
        Some(code.to_string())
    } else {
        Some(format!("{code}: {msg}"))
    }
}

fn azure_previous_response_not_found(body: &str) -> bool {
    let v: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let err = match v.get("error") {
        Some(e) => e,
        None => return false,
    };
    // Check common locations for a code signal.
    let code = err.get("code").and_then(|v| v.as_str());
    let inner_code = err
        .get("innererror")
        .and_then(|ie| ie.get("code"))
        .and_then(|v| v.as_str());
    let msg = err
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    matches!(code, Some(c) if c.eq_ignore_ascii_case("previous_response_not_found"))
        || matches!(inner_code, Some(c) if c.eq_ignore_ascii_case("previous_response_not_found"))
        || msg.contains("previous_response_not_found")
        || (msg.contains("previous") && msg.contains("response") && msg.contains("not found"))
}
#[derive(Clone)]
struct ResumeCtx {
    client: reqwest::Client,
    provider: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
    background: bool,
}

type BytesResultStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;

async fn process_sse(
    stream: BytesResultStream,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    idle_timeout: Duration,
    resume_ctx: Option<ResumeCtx>,
) {
    let mut stream = stream.eventsource();

    // If the stream stays completely silent for an extended period treat it as disconnected.
    // Track the final `response.completed` envelope. Emit `Completed` as soon
    // as we see it to avoid hanging when servers keep connections open after
    // the terminal event (which some mock servers do).
    let mut response_completed: Option<ResponseCompleted> = None;
    let mut completed_emitted = false;
    let mut response_error: Option<CodexErr> = None;
    let mut last_sequence_number: Option<u64> = None;
    let mut response_id_for_resume: Option<String> = None;

    // Resume config for Azure background streaming.
    let mut resume_attempts: u64 = 0;
    let max_resume_retries = resume_ctx
        .as_ref()
        .map(|c| c.provider.stream_max_retries())
        .unwrap_or(0);

    loop {
        let sse = match timeout(idle_timeout, stream.next()).await {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("SSE Error: {e:#}");
                // Attempt resume on parser/network error if eligible.
                if resume_ctx.is_some()
                    && !completed_emitted
                    && response_completed.is_none()
                    && response_id_for_resume.is_some()
                    && last_sequence_number.is_some()
                    && resume_attempts < max_resume_retries
                    && let Some(ctx) = resume_ctx.as_ref()
                    && let Some(new_stream) = attempt_resume(
                        &ctx.client,
                        &ctx.provider,
                        &ctx.auth_manager,
                        response_id_for_resume.as_ref().unwrap(),
                        last_sequence_number.unwrap(),
                    )
                    .await
                {
                    resume_attempts += 1;
                    stream = new_stream.eventsource();
                    continue;
                }
                let event = CodexErr::Stream(e.to_string(), None);
                let _ = tx_event.send(Err(event)).await;
                return;
            }
            Ok(None) => {
                // If the server closed the connection after the terminal event
                // we may have already emitted Completed. Avoid duplicating it.
                if !completed_emitted && response_completed.is_none() {
                    // Try to resume first if eligible; otherwise fall through to emit error or
                    // final Completed.
                    if resume_ctx.is_some()
                        && response_id_for_resume.is_some()
                        && last_sequence_number.is_some()
                        && resume_attempts < max_resume_retries
                        && let Some(ctx) = resume_ctx.as_ref()
                        && let Some(new_stream) = attempt_resume(
                            &ctx.client,
                            &ctx.provider,
                            &ctx.auth_manager,
                            response_id_for_resume.as_ref().unwrap(),
                            last_sequence_number.unwrap(),
                        )
                        .await
                    {
                        resume_attempts += 1;
                        stream = new_stream.eventsource();
                        continue;
                    }
                }

                if !completed_emitted {
                    match response_completed {
                        Some(ResponseCompleted {
                            id: response_id,
                            usage,
                        }) => {
                            let event = ResponseEvent::Completed {
                                response_id,
                                token_usage: usage.map(Into::into),
                            };
                            let _ = tx_event.send(Ok(event)).await;
                        }
                        None => {
                            let _ = tx_event
                                .send(Err(response_error.unwrap_or(CodexErr::Stream(
                                    "stream closed before response.completed".into(),
                                    None,
                                ))))
                                .await;
                        }
                    }
                }
                return;
            }
            Err(_) => {
                // Idle timeout – attempt resume if eligible before failing.
                if resume_ctx.is_some()
                    && !completed_emitted
                    && response_completed.is_none()
                    && response_id_for_resume.is_some()
                    && last_sequence_number.is_some()
                    && resume_attempts < max_resume_retries
                    && let Some(ctx) = resume_ctx.as_ref()
                    && let Some(new_stream) = attempt_resume(
                        &ctx.client,
                        &ctx.provider,
                        &ctx.auth_manager,
                        response_id_for_resume.as_ref().unwrap(),
                        last_sequence_number.unwrap(),
                    )
                    .await
                {
                    resume_attempts += 1;
                    stream = new_stream.eventsource();
                    continue;
                }
                let _ = tx_event
                    .send(Err(CodexErr::Stream(
                        "idle timeout waiting for SSE".into(),
                        None,
                    )))
                    .await;
                return;
            }
        };

        let raw = sse.data.clone();
        trace!("SSE event: {}", raw);

        let event: SseEvent = match serde_json::from_str(&sse.data) {
            Ok(event) => event,
            Err(e) => {
                debug!("Failed to parse SSE event: {e}, data: {}", &sse.data);
                continue;
            }
        };

        // Track sequence number and response id for potential resume.
        if let Some(seq) = event.sequence_number {
            last_sequence_number = Some(seq);
        }

        match event.kind.as_str() {
                // Azure: reasoning content delta (alias of response.reasoning_text.delta)
                "response.reasoning.delta" => {
                    if let Some(delta) = event.delta {
                        let event = ResponseEvent::ReasoningContentDelta(delta);
                        if tx_event.send(Ok(event)).await.is_err() {
                            return;
                        }
                    }
                }
                // Azure: preamble (treat as a reasoning summary style area)
                "response.preamble.delta" => {
                    if let Some(delta) = event.delta {
                        let event = ResponseEvent::ReasoningSummaryDelta(delta);
                        if tx_event.send(Ok(event)).await.is_err() {
                            return;
                        }
                    }
                }
            // Individual output item finalised. Forward immediately so the
            // rest of the agent can stream assistant text/functions *live*
            // instead of waiting for the final `response.completed` envelope.
            //
            // IMPORTANT: We used to ignore these events and forward the
            // duplicated `output` array embedded in the `response.completed`
            // payload.  That produced two concrete issues:
            //   1. No real‑time streaming – the user only saw output after the
            //      entire turn had finished, which broke the "typing" UX and
            //      made long‑running turns look stalled.
            //   2. Duplicate `function_call_output` items – both the
            //      individual *and* the completed array were forwarded, which
            //      confused the backend and triggered 400
            //      "previous_response_not_found" errors because the duplicated
            //      IDs did not match the incremental turn chain.
            //
            // The fix is to forward the incremental events *as they come* and
            // drop the duplicated list inside `response.completed`.
            "response.output_item.done" => {
                let Some(item_val) = event.item else { continue };
                let Ok(item) = serde_json::from_value::<ResponseItem>(item_val) else {
                    debug!("failed to parse ResponseItem from output_item.done");
                    continue;
                };

                let event = ResponseEvent::OutputItemDone(item);
                if tx_event.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            "response.output_text.delta" => {
                if let Some(delta) = event.delta {
                    let event = ResponseEvent::OutputTextDelta(delta);
                    if tx_event.send(Ok(event)).await.is_err() {
                        return;
                    }
                }
            }
            "response.reasoning_summary_text.delta" => {
                if let Some(delta) = event.delta {
                    let event = ResponseEvent::ReasoningSummaryDelta(delta);
                    if tx_event.send(Ok(event)).await.is_err() {
                        return;
                    }
                }
            }
            "response.reasoning_text.delta" => {
                if let Some(delta) = event.delta {
                    let event = ResponseEvent::ReasoningContentDelta(delta);
                    if tx_event.send(Ok(event)).await.is_err() {
                        return;
                    }
                }
            }
            "response.created" => {
                if let Some(resp) = event.response.as_ref() {
                    if let Some(id) = resp.get("id").and_then(|v| v.as_str()) {
                        response_id_for_resume = Some(id.to_string());
                    }
                    let _ = tx_event.send(Ok(ResponseEvent::Created)).await;
                }
            }
            "response.failed" => {
                if let Some(resp_val) = event.response {
                    // Default if no error object present.
                    response_error = Some(CodexErr::Stream(
                        "response.failed event received".to_string(),
                        None,
                    ));

                    if let Some(error_val) = resp_val.get("error") {
                        match serde_json::from_value::<Error>(error_val.clone()) {
                            Ok(error) => {
                                let delay = try_parse_retry_after(&error);
                                let message = error.message.unwrap_or_default();
                                response_error = Some(CodexErr::Stream(message, delay));
                            }
                            Err(e) => {
                                debug!("failed to parse ErrorResponse: {e}");
                            }
                        }
                    }
                }
            }
            // Graceful terminal outcome with reason (no subsequent completed)
            "response.incomplete" => {
                if let Some(resp_val) = event.response.as_ref() {
                    let id = resp_val
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let reason = resp_val
                        .get("incomplete_details")
                        .and_then(|d| d.get("reason"))
                        .and_then(|r| r.as_str())
                        .map(|s| s.to_string());
                    let ev = ResponseEvent::Incomplete { response_id: id, _reason: reason };
                    if tx_event.send(Ok(ev)).await.is_err() {
                        return;
                    }
                    // Mark terminal state to suppress idle-close error.
                    completed_emitted = true;
                }
            }

            // Final response completed – includes array of output items & id
            "response.completed" => {
                if let Some(resp_val) = event.response {
                    match serde_json::from_value::<ResponseCompleted>(resp_val) {
                        Ok(r) => {
                            // Capture id for any subsequent resume attempts (though this is terminal).
                            response_id_for_resume = Some(r.id.clone());
                            // Emit Completed immediately to avoid hanging if
                            // the server doesn't close the SSE stream right
                            // away. Also retain it so the stream-closure path
                            // can still emit it if we didn't already.
                            if !completed_emitted {
                                let event = ResponseEvent::Completed {
                                    response_id: r.id.clone(),
                                    token_usage: r.usage.clone().map(Into::into),
                                };
                                let _ = tx_event.send(Ok(event)).await;
                                completed_emitted = true;
                            }
                            response_completed = Some(r);
                        }
                        Err(e) => {
                            debug!("failed to parse ResponseCompleted: {e}");
                            continue;
                        }
                    };
                };
            }
            "response.content_part.done"
            | "response.function_call_arguments.delta"
            | "response.custom_tool_call_input.delta"
            | "response.custom_tool_call_input.done" // also emitted as response.output_item.done
            | "response.in_progress"
            | "response.output_text.done" => {}
            "response.output_item.added" => {
                if let Some(item) = event.item.as_ref() {
                    // Detect web_search_call begin and forward a synthetic event upstream.
                    if let Some(ty) = item.get("type").and_then(|v| v.as_str())
                        && ty == "web_search_call"
                    {
                        let call_id = item
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let ev = ResponseEvent::WebSearchCallBegin { call_id };
                        if tx_event.send(Ok(ev)).await.is_err() {
                            return;
                        }
                    }
                }
            }
            "response.reasoning_summary_part.added" => {
                // Boundary between reasoning summary sections (e.g., titles).
                let event = ResponseEvent::ReasoningSummaryPartAdded;
                if tx_event.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            "response.reasoning_summary_text.done" => {}
            // Azure image generation partials and MCP approval requests are
            // not yet surfaced via a dedicated UI event. Ignore gracefully.
            "response.image_generation_call.partial_image" | "response.mcp_approval_request" => {}
            _ => {}
        }
    }
}

/// Attempt to resume an Azure Responses SSE stream starting after the given
/// sequence cursor. Returns a boxed byte stream on success, or None if resume
/// is not possible.
async fn attempt_resume(
    client: &reqwest::Client,
    provider: &ModelProviderInfo,
    auth_manager: &Option<Arc<AuthManager>>,
    response_id: &str,
    starting_after: u64,
) -> Option<BytesResultStream> {
    use reqwest::header::CONTENT_TYPE;

    let auth = auth_manager.as_ref().and_then(|m| m.auth());
    let base = provider.get_response_url(&auth, response_id);
    let sep = if base.contains('?') { '&' } else { '?' };
    let url = format!("{base}{sep}stream=true&starting_after={starting_after}");

    let mut req = client
        .get(url)
        .header(reqwest::header::ACCEPT, "text/event-stream");

    // Prefer provider API key when configured; otherwise fall back to
    // AuthManager token. Mirror create_request_builder semantics.
    if let Ok(Some(api_key)) = provider.api_key() {
        match provider.auth_type {
            crate::model_provider_info::AuthType::ApiKey => {
                req = req.header("api-key", api_key);
            }
            _ => {
                req = req.bearer_auth(api_key);
            }
        }
    } else if let Some(ca) = auth.as_ref() {
        match provider.auth_type {
            crate::model_provider_info::AuthType::ApiKey => match ca.get_token().await {
                Ok(t) => req = req.header("api-key", t),
                Err(_) => return None,
            },
            _ => match ca.get_token().await {
                Ok(t) => req = req.bearer_auth(t),
                Err(_) => return None,
            },
        }
    }

    req = provider.apply_http_headers(req);

    let resp = match req.send().await {
        Ok(r) => r,
        Err(_) => return None,
    };
    if !resp.status().is_success() {
        return None;
    }
    let is_sse = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/event-stream"))
        .unwrap_or(false);
    if !is_sse {
        return None;
    }

    let s = resp.bytes_stream().map_err(CodexErr::Reqwest);
    Some(Box::pin(s))
}

/// used in tests to stream from a text SSE file
async fn stream_from_fixture(
    path: impl AsRef<Path>,
    provider: ModelProviderInfo,
) -> Result<ResponseStream> {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);
    let f = std::fs::File::open(path.as_ref())?;
    let lines = std::io::BufReader::new(f).lines();

    // insert \n\n after each line for proper SSE parsing
    let mut content = String::new();
    for line in lines {
        content.push_str(&line?);
        content.push_str("\n\n");
    }

    let rdr = std::io::Cursor::new(content);
    let stream = ReaderStream::new(rdr).map_err(CodexErr::Io);
    tokio::spawn(process_sse(
        Box::pin(stream),
        tx_event,
        provider.stream_idle_timeout(),
        None,
    ));
    Ok(ResponseStream { rx_event })
}

fn rate_limit_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();

    #[expect(clippy::unwrap_used)]
    RE.get_or_init(|| Regex::new(r"Please try again in (\d+(?:\.\d+)?)(s|ms)").unwrap())
}

fn try_parse_retry_after(err: &Error) -> Option<Duration> {
    if err.code != Some("rate_limit_exceeded".to_string()) {
        return None;
    }

    // parse the Please try again in 1.898s format using regex
    let re = rate_limit_regex();
    if let Some(message) = &err.message
        && let Some(captures) = re.captures(message)
    {
        let seconds = captures.get(1);
        let unit = captures.get(2);

        if let (Some(value), Some(unit)) = (seconds, unit) {
            let value = value.as_str().parse::<f64>().ok()?;
            let unit = unit.as_str();

            if unit == "s" {
                return Some(Duration::from_secs_f64(value));
            } else if unit == "ms" {
                return Some(Duration::from_millis(value as u64));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_test::io::Builder as IoBuilder;
    use tokio_util::io::ReaderStream;

    // ────────────────────────────
    // Helpers
    // ────────────────────────────

    /// Runs the SSE parser on pre-chunked byte slices and returns every event
    /// (including any final `Err` from a stream-closure check).
    async fn collect_events(
        chunks: &[&[u8]],
        provider: ModelProviderInfo,
    ) -> Vec<Result<ResponseEvent>> {
        let mut builder = IoBuilder::new();
        for chunk in chunks {
            builder.read(chunk);
        }

        let reader = builder.build();
        let stream = ReaderStream::new(reader).map_err(CodexErr::Io);
        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent>>(16);
        tokio::spawn(process_sse(
            Box::pin(stream),
            tx,
            provider.stream_idle_timeout(),
            None,
        ));

        let mut events = Vec::new();
        while let Some(ev) = rx.recv().await {
            events.push(ev);
        }
        events
    }

    /// Builds an in-memory SSE stream from JSON fixtures and returns only the
    /// successfully parsed events (panics on internal channel errors).
    async fn run_sse(
        events: Vec<serde_json::Value>,
        provider: ModelProviderInfo,
    ) -> Vec<ResponseEvent> {
        let mut body = String::new();
        for e in events {
            let kind = e
                .get("type")
                .and_then(|v| v.as_str())
                .expect("fixture event missing type");
            if e.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                body.push_str(&format!("event: {kind}\n\n"));
            } else {
                body.push_str(&format!("event: {kind}\ndata: {e}\n\n"));
            }
        }

        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent>>(8);
        let stream = ReaderStream::new(std::io::Cursor::new(body)).map_err(CodexErr::Io);
        tokio::spawn(process_sse(
            Box::pin(stream),
            tx,
            provider.stream_idle_timeout(),
            None,
        ));

        let mut out = Vec::new();
        while let Some(ev) = rx.recv().await {
            out.push(ev.expect("channel closed"));
        }
        out
    }

    // ────────────────────────────
    // Tests from `implement-test-for-responses-api-sse-parser`
    // ────────────────────────────

    #[tokio::test]
    async fn parses_items_and_completed() {
        let item1 = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Hello"}]
            }
        })
        .to_string();

        let item2 = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "World"}]
            }
        })
        .to_string();

        let completed = json!({
            "type": "response.completed",
            "response": { "id": "resp1" }
        })
        .to_string();

        let sse1 = format!("event: response.output_item.done\ndata: {item1}\n\n");
        let sse2 = format!("event: response.output_item.done\ndata: {item2}\n\n");
        let sse3 = format!("event: response.completed\ndata: {completed}\n\n");

        let provider = ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            auth_type: Default::default(),
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
        };

        let events = collect_events(
            &[sse1.as_bytes(), sse2.as_bytes(), sse3.as_bytes()],
            provider,
        )
        .await;

        assert_eq!(events.len(), 3);

        matches!(
            &events[0],
            Ok(ResponseEvent::OutputItemDone(ResponseItem::Message { role, .. }))
                if role == "assistant"
        );

        matches!(
            &events[1],
            Ok(ResponseEvent::OutputItemDone(ResponseItem::Message { role, .. }))
                if role == "assistant"
        );

        match &events[2] {
            Ok(ResponseEvent::Completed {
                response_id,
                token_usage,
            }) => {
                assert_eq!(response_id, "resp1");
                assert!(token_usage.is_none());
            }
            other => panic!("unexpected third event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn error_when_missing_completed() {
        let item1 = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Hello"}]
            }
        })
        .to_string();

        let sse1 = format!("event: response.output_item.done\ndata: {item1}\n\n");
        let provider = ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            auth_type: Default::default(),
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
        };

        let events = collect_events(&[sse1.as_bytes()], provider).await;

        assert_eq!(events.len(), 2);

        matches!(events[0], Ok(ResponseEvent::OutputItemDone(_)));

        match &events[1] {
            Err(CodexErr::Stream(msg, _)) => {
                assert_eq!(msg, "stream closed before response.completed")
            }
            other => panic!("unexpected second event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn error_when_error_event() {
        let raw_error = r#"{"type":"response.failed","sequence_number":3,"response":{"id":"resp_689bcf18d7f08194bf3440ba62fe05d803fee0cdac429894","object":"response","created_at":1755041560,"status":"failed","background":false,"error":{"code":"rate_limit_exceeded","message":"Rate limit reached for gpt-5 in organization org-AAA on tokens per min (TPM): Limit 30000, Used 22999, Requested 12528. Please try again in 11.054s. Visit https://platform.openai.com/account/rate-limits to learn more."}, "usage":null,"user":null,"metadata":{}}}"#;

        let sse1 = format!("event: response.failed\ndata: {raw_error}\n\n");
        let provider = ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            auth_type: Default::default(),
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
        };

        let events = collect_events(&[sse1.as_bytes()], provider).await;

        assert_eq!(events.len(), 1);

        match &events[0] {
            Err(CodexErr::Stream(msg, delay)) => {
                assert_eq!(
                    msg,
                    "Rate limit reached for gpt-5 in organization org-AAA on tokens per min (TPM): Limit 30000, Used 22999, Requested 12528. Please try again in 11.054s. Visit https://platform.openai.com/account/rate-limits to learn more."
                );
                assert_eq!(*delay, Some(Duration::from_secs_f64(11.054)));
            }
            other => panic!("unexpected second event: {other:?}"),
        }
    }

    // ────────────────────────────
    // Table-driven test from `main`
    // ────────────────────────────

    /// Verifies that the adapter produces the right `ResponseEvent` for a
    /// variety of incoming `type` values.
    #[tokio::test]
    async fn table_driven_event_kinds() {
        struct TestCase {
            name: &'static str,
            event: serde_json::Value,
            expect_first: fn(&ResponseEvent) -> bool,
            expected_len: usize,
        }

        fn is_created(ev: &ResponseEvent) -> bool {
            matches!(ev, ResponseEvent::Created)
        }
        fn is_output(ev: &ResponseEvent) -> bool {
            matches!(ev, ResponseEvent::OutputItemDone(_))
        }
        fn is_completed(ev: &ResponseEvent) -> bool {
            matches!(ev, ResponseEvent::Completed { .. })
        }

        let completed = json!({
            "type": "response.completed",
            "response": {
                "id": "c",
                "usage": {
                    "input_tokens": 0,
                    "input_tokens_details": null,
                    "output_tokens": 0,
                    "output_tokens_details": null,
                    "total_tokens": 0
                },
                "output": []
            }
        });

        let cases = vec![
            TestCase {
                name: "created",
                event: json!({"type": "response.created", "response": {}}),
                expect_first: is_created,
                expected_len: 2,
            },
            TestCase {
                name: "output_item.done",
                event: json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "message",
                        "role": "assistant",
                        "content": [
                            {"type": "output_text", "text": "hi"}
                        ]
                    }
                }),
                expect_first: is_output,
                expected_len: 2,
            },
            TestCase {
                name: "unknown",
                event: json!({"type": "response.new_tool_event"}),
                expect_first: is_completed,
                expected_len: 1,
            },
        ];

        for case in cases {
            let mut evs = vec![case.event];
            evs.push(completed.clone());

            let provider = ModelProviderInfo {
                name: "test".to_string(),
                base_url: Some("https://test.com".to_string()),
                env_key: Some("TEST_API_KEY".to_string()),
                env_key_instructions: None,
                wire_api: WireApi::Responses,
                auth_type: Default::default(),
                query_params: None,
                http_headers: None,
                env_http_headers: None,
                request_max_retries: Some(0),
                stream_max_retries: Some(0),
                stream_idle_timeout_ms: Some(1000),
                requires_openai_auth: false,
            };

            let out = run_sse(evs, provider).await;
            assert_eq!(out.len(), case.expected_len, "case {}", case.name);
            assert!(
                (case.expect_first)(&out[0]),
                "first event mismatch in case {}",
                case.name
            );
        }
    }

    #[test]
    fn test_try_parse_retry_after() {
        let err = Error {
            r#type: None,
            message: Some("Rate limit reached for gpt-5 in organization org- on tokens per min (TPM): Limit 1, Used 1, Requested 19304. Please try again in 28ms. Visit https://platform.openai.com/account/rate-limits to learn more.".to_string()),
            code: Some("rate_limit_exceeded".to_string()),
            plan_type: None,
            resets_in_seconds: None
        };

        let delay = try_parse_retry_after(&err);
        assert_eq!(delay, Some(Duration::from_millis(28)));
    }

    #[test]
    fn test_try_parse_retry_after_no_delay() {
        let err = Error {
            r#type: None,
            message: Some("Rate limit reached for gpt-5 in organization <ORG> on tokens per min (TPM): Limit 30000, Used 6899, Requested 24050. Please try again in 1.898s. Visit https://platform.openai.com/account/rate-limits to learn more.".to_string()),
            code: Some("rate_limit_exceeded".to_string()),
            plan_type: None,
            resets_in_seconds: None
        };
        let delay = try_parse_retry_after(&err);
        assert_eq!(delay, Some(Duration::from_secs_f64(1.898)));
    }
}
