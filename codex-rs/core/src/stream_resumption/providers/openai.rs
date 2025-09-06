//! OpenAI specific stream resumption implementation.

use async_trait::async_trait;
use std::sync::Arc;
use tracing::warn;

use crate::stream_resumption::context::{ProviderResumption, ResumptionContext};
use crate::client_common::ResponseEvent;
use crate::error::{Result, CodexErr};
use crate::model_provider_info::ModelProviderInfo;
use crate::AuthManager;

/// OpenAI stream resumption implementation.
///
/// Currently, OpenAI's standard API doesn't support stream resumption in the same way
/// that Azure does. This implementation serves as a placeholder and fallback, with
/// the potential for future enhancement if OpenAI adds resumption capabilities.
#[derive(Debug)]
pub struct OpenAIResumption {
    provider: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
}

impl OpenAIResumption {
    pub fn new(provider: ModelProviderInfo, auth_manager: Option<Arc<AuthManager>>) -> Self {
        Self {
            provider,
            auth_manager,
        }
    }
}

#[async_trait]
impl ProviderResumption for OpenAIResumption {
    fn supports_resumption(&self) -> bool {
        // OpenAI's standard API currently doesn't support mid-stream resumption
        // This could be updated in the future if OpenAI adds this capability
        false
    }
    
    fn max_resume_attempts(&self) -> u32 {
        // Since we don't support resumption, return 0
        0
    }
    
    async fn create_resume_request(
        &self,
        _context: &ResumptionContext,
        _original_request_body: &serde_json::Value,
    ) -> Result<reqwest::Request> {
        // OpenAI doesn't currently support resume requests
        warn!("OpenAI resumption requested but not supported");
        Err(CodexErr::InternalServerError)
    }
    
    fn extract_resumption_info(
        &self,
        _event: &ResponseEvent,
        _context: &mut ResumptionContext,
    ) {
        // No resumption info to extract for OpenAI standard API
        // In the future, if OpenAI adds resumption support, we could track
        // response IDs or other resumption markers here
    }
    
    fn is_resumable_error(&self, _error: &CodexErr) -> bool {
        // Since we don't support resumption, no errors are resumable
        false
    }
}

// Note: If OpenAI adds resumption support in the future, this implementation
// could be enhanced to:
// 
// 1. Track response IDs from completed responses
// 2. Use OpenAI's resumption API (if/when available)
// 3. Handle OpenAI-specific error patterns
// 4. Support OpenAI's authentication for resume requests
//
// The infrastructure is here to support it when the capability becomes available.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_provider_info::WireApi;

    fn create_test_openai_provider() -> ModelProviderInfo {
        ModelProviderInfo {
            name: "OpenAI".to_string(),
            base_url: Some("https://api.openai.com".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            auth_type: Default::default(),
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(3),
            stream_max_retries: Some(0), // OpenAI doesn't support stream resumption
            stream_idle_timeout_ms: Some(30000),
            requires_openai_auth: true,
        }
    }

    #[tokio::test]
    async fn test_openai_no_resumption_support() {
        let provider = create_test_openai_provider();
        let resumption = OpenAIResumption::new(provider, None);
        
        // Should not support resumption (yet)
        assert!(!resumption.supports_resumption());
        assert_eq!(resumption.max_resume_attempts(), 0);
    }

    #[test]
    fn test_openai_no_resumable_errors() {
        let provider = create_test_openai_provider();
        let resumption = OpenAIResumption::new(provider, None);
        
        // No errors should be considered resumable for OpenAI
        let timeout_error = CodexErr::Stream("timeout".to_string(), None);
        assert!(!resumption.is_resumable_error(&timeout_error));
        
        let server_error = CodexErr::UnexpectedStatus(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string());
        assert!(!resumption.is_resumable_error(&server_error));
    }

    #[tokio::test]
    async fn test_openai_resume_request_fails() {
        let provider = create_test_openai_provider();
        let resumption = OpenAIResumption::new(provider, None);
        let context = ResumptionContext::new("test-id".to_string(), 3);
        
        // Should fail since resumption is not supported
        let result = resumption.create_resume_request(&context, &serde_json::json!({})).await;
        assert!(result.is_err());
    }
}