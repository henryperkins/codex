//! Azure OpenAI specific stream resumption implementation.

use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;

use crate::AuthManager;
use crate::client_common::ResponseEvent;
use crate::error::CodexErr;
use crate::error::Result;
use crate::model_provider_info::AuthType;
use crate::model_provider_info::ModelProviderInfo;
use crate::stream_resumption::context::ProviderResumption;
use crate::stream_resumption::context::ResumptionContext;
use codex_protocol::mcp_protocol::AuthMode;

/// Azure OpenAI stream resumption implementation.
///
/// Azure supports background processing and response chaining via previous_response_id.
/// This implementation uses Azure's background=true parameter and polling for resilience.
#[derive(Debug)]
pub struct AzureResumption {
    provider: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
    client: reqwest::Client,
}

impl AzureResumption {
    pub fn new(provider: ModelProviderInfo, auth_manager: Option<Arc<AuthManager>>) -> Self {
        Self {
            provider,
            auth_manager,
            client: reqwest::Client::new(),
        }
    }

    /// Extract the base URL for Azure OpenAI API calls.
    fn get_base_url(&self) -> Option<&str> {
        self.provider.base_url.as_deref()
    }

    /// Build a stream resume URL with starting_after parameter.
    fn build_resume_url(&self, response_id: &str, starting_after: Option<u64>) -> Result<String> {
        let base_url = self
            .get_base_url()
            .ok_or_else(|| CodexErr::InternalServerError)?;

        let mut url = format!(
            "{}/openai/v1/responses/{}?stream=true",
            base_url.trim_end_matches('/'),
            response_id
        );

        if let Some(sequence) = starting_after {
            url.push_str(&format!("&starting_after={sequence}"));
        }

        Ok(url)
    }
}

#[async_trait]
impl ProviderResumption for AzureResumption {
    fn supports_resumption(&self) -> bool {
        // Azure supports stream resumption using starting_after parameter
        self.provider.base_url.is_some() && self.provider.stream_max_retries.unwrap_or(0) > 0
    }

    fn max_resume_attempts(&self) -> u32 {
        // Use provider's configured max retries, with a reasonable default
        self.provider.stream_max_retries.unwrap_or(3) as u32
    }

    async fn create_resume_request(
        &self,
        context: &ResumptionContext,
        _original_request_body: &serde_json::Value,
    ) -> Result<reqwest::Request> {
        let resume_url = self.build_resume_url(&context.response_id, context.last_sequence)?;

        debug!(
            "Creating Azure stream resume request: url={}, sequence={:?}, attempt={}",
            resume_url, context.last_sequence, context.attempt_count
        );

        // Azure uses GET requests with query parameters for stream resumption
        let mut request_builder = self
            .client
            .get(&resume_url)
            .header("Accept", "text/event-stream");

        // Add authentication headers using same pattern as ModelProviderInfo
        if let Some(auth_manager) = &self.auth_manager
            && let Some(auth) = auth_manager.auth()
        {
            let token = auth
                .get_token()
                .await
                .map_err(|e| CodexErr::Stream(format!("Failed to get auth token: {e}"), None))?;

            // Match the auth pattern from ModelProviderInfo::create_request_builder
            match (&self.provider.auth_type, &auth.mode) {
                (AuthType::ApiKey, AuthMode::ApiKey) => {
                    request_builder = request_builder.header("api-key", token);
                }
                _ => {
                    request_builder =
                        request_builder.header("Authorization", format!("Bearer {token}"));
                }
            }
        }

        // Add any provider-specific headers
        if let Some(headers) = &self.provider.http_headers {
            for (key, value) in headers {
                request_builder = request_builder.header(key, value);
            }
        }

        request_builder.build().map_err(|e| {
            CodexErr::Stream(format!("Failed to build Azure resume request: {e}"), None)
        })
    }

    fn extract_resumption_info(&self, event: &ResponseEvent, context: &mut ResumptionContext) {
        match event {
            ResponseEvent::Completed { response_id, .. } => {
                // Update context with the response ID for future resumption
                if context.response_id.is_empty() {
                    context.response_id = response_id.clone();
                }
                debug!("Extracted response_id for resumption: {}", response_id);
            }
            ResponseEvent::OutputItemDone(_) => {
                // Increment sequence number for each completed output item
                let new_sequence = context.last_sequence.map(|s| s + 1).unwrap_or(1);
                context.update_sequence(new_sequence);
                debug!("Updated sequence number to: {}", new_sequence);
            }
            ResponseEvent::OutputTextDelta(_) => {
                // For text deltas, we don't increment sequence but we note progress
                // The sequence number represents completed items, not individual deltas
            }
            _ => {
                // Other events don't affect resumption tracking
            }
        }
    }

    fn is_resumable_error(&self, error: &CodexErr) -> bool {
        match error {
            // Network-related errors that are likely recoverable
            CodexErr::Stream(msg, _) if msg.contains("timeout") => true,
            CodexErr::Stream(msg, _) if msg.contains("connection") => true,
            CodexErr::Stream(msg, _) if msg.contains("network") => true,
            CodexErr::Stream(msg, _) if msg.contains("EOF") => true,

            // HTTP errors that might be temporary
            CodexErr::UnexpectedStatus(status_code, _) => {
                match status_code.as_u16() {
                    // Server errors that might be temporary
                    500..=599 => true,
                    // Rate limiting that might resolve
                    429 => true,
                    // Client errors are typically not recoverable
                    400..=499 => false,
                    _ => false,
                }
            }

            // Other error types are typically not recoverable via resumption
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_provider_info::WireApi;

    fn create_test_azure_provider() -> ModelProviderInfo {
        ModelProviderInfo {
            name: "Azure OpenAI Test".to_string(),
            base_url: Some("https://test-resource.openai.azure.com".to_string()),
            env_key: Some("AZURE_OPENAI_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            auth_type: Default::default(),
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(3),
            stream_max_retries: Some(3),
            stream_idle_timeout_ms: Some(30000),
            requires_openai_auth: false,
        }
    }

    #[tokio::test]
    async fn test_azure_resumption_support() {
        let provider = create_test_azure_provider();
        let resumption = AzureResumption::new(provider, None);

        // Azure supports stream resumption using starting_after parameter
        assert!(resumption.supports_resumption());
        assert_eq!(resumption.max_resume_attempts(), 3);
    }

    #[test]
    fn test_azure_resume_url_building() {
        let provider = create_test_azure_provider();
        let resumption = AzureResumption::new(provider, None);

        // Test without sequence number
        let url = resumption
            .build_resume_url("test-response-id", None)
            .unwrap();
        assert_eq!(
            url,
            "https://test-resource.openai.azure.com/openai/v1/responses/test-response-id?stream=true"
        );

        // Test with sequence number
        let url_with_seq = resumption
            .build_resume_url("test-response-id", Some(42))
            .unwrap();
        assert_eq!(
            url_with_seq,
            "https://test-resource.openai.azure.com/openai/v1/responses/test-response-id?stream=true&starting_after=42"
        );
    }

    #[test]
    fn test_error_resumability() {
        let provider = create_test_azure_provider();
        let resumption = AzureResumption::new(provider, None);

        // Timeout errors should be resumable
        let timeout_error = CodexErr::Stream("connection timeout".to_string(), None);
        assert!(resumption.is_resumable_error(&timeout_error));

        // 500 errors should be resumable
        let server_error = CodexErr::UnexpectedStatus(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        );
        assert!(resumption.is_resumable_error(&server_error));

        // 400 errors should not be resumable
        let client_error =
            CodexErr::UnexpectedStatus(reqwest::StatusCode::BAD_REQUEST, "Bad Request".to_string());
        assert!(!resumption.is_resumable_error(&client_error));
    }

    #[test]
    fn test_sequence_tracking() {
        let provider = create_test_azure_provider();
        let resumption = AzureResumption::new(provider, None);
        let mut context = ResumptionContext::new("test-id".to_string(), 3);

        // Test sequence number tracking
        let output_done_event =
            ResponseEvent::OutputItemDone(codex_protocol::models::ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![],
            });

        resumption.extract_resumption_info(&output_done_event, &mut context);
        assert_eq!(context.last_sequence, Some(1));

        resumption.extract_resumption_info(&output_done_event, &mut context);
        assert_eq!(context.last_sequence, Some(2));
    }
}
