//! Provider-specific stream resumption implementations.

mod azure;
mod fallback;
mod openai;

pub use azure::AzureResumption;
pub use fallback::NoResumption;
pub use openai::OpenAIResumption;

use super::context::ProviderResumption;
use crate::AuthManager;
use crate::model_provider_info::ModelProviderInfo;
use std::sync::Arc;

/// Concrete enum for provider-specific resumption implementations.
/// This avoids the trait object issues with async methods.
#[derive(Debug)]
pub enum ResumptionProvider {
    Azure(AzureResumption),
    OpenAI(OpenAIResumption),
    None(NoResumption),
}

impl ResumptionProvider {
    /// Check if this provider supports stream resumption.
    pub fn supports_resumption(&self) -> bool {
        match self {
            ResumptionProvider::Azure(provider) => provider.supports_resumption(),
            ResumptionProvider::OpenAI(provider) => provider.supports_resumption(),
            ResumptionProvider::None(provider) => provider.supports_resumption(),
        }
    }

    /// Get the maximum number of resume attempts for this provider.
    pub fn max_resume_attempts(&self) -> u32 {
        match self {
            ResumptionProvider::Azure(provider) => provider.max_resume_attempts(),
            ResumptionProvider::OpenAI(provider) => provider.max_resume_attempts(),
            ResumptionProvider::None(provider) => provider.max_resume_attempts(),
        }
    }

    /// Determine if an error is recoverable via stream resumption.
    pub fn is_resumable_error(&self, error: &crate::error::CodexErr) -> bool {
        match self {
            ResumptionProvider::Azure(provider) => provider.is_resumable_error(error),
            ResumptionProvider::OpenAI(provider) => provider.is_resumable_error(error),
            ResumptionProvider::None(provider) => provider.is_resumable_error(error),
        }
    }

    /// Extract resumption information from a streaming response event.
    pub fn extract_resumption_info(
        &self,
        event: &crate::client_common::ResponseEvent,
        context: &mut super::context::ResumptionContext,
    ) {
        match self {
            ResumptionProvider::Azure(provider) => provider.extract_resumption_info(event, context),
            ResumptionProvider::OpenAI(provider) => {
                provider.extract_resumption_info(event, context)
            }
            ResumptionProvider::None(provider) => provider.extract_resumption_info(event, context),
        }
    }

    /// Get the delay before attempting a resume (for exponential backoff).
    pub fn resume_delay(&self, attempt: u32) -> std::time::Duration {
        match self {
            ResumptionProvider::Azure(provider) => provider.resume_delay(attempt),
            ResumptionProvider::OpenAI(provider) => provider.resume_delay(attempt),
            ResumptionProvider::None(provider) => provider.resume_delay(attempt),
        }
    }

    /// Create a new stream request to resume from the given context.
    pub async fn create_resume_request(
        &self,
        context: &super::context::ResumptionContext,
        original_request_body: &serde_json::Value,
    ) -> crate::error::Result<reqwest::Request> {
        match self {
            ResumptionProvider::Azure(provider) => {
                provider
                    .create_resume_request(context, original_request_body)
                    .await
            }
            ResumptionProvider::OpenAI(provider) => {
                provider
                    .create_resume_request(context, original_request_body)
                    .await
            }
            ResumptionProvider::None(provider) => {
                provider
                    .create_resume_request(context, original_request_body)
                    .await
            }
        }
    }
}

/// Factory function to create the appropriate resumption provider based on the model provider.
pub fn create_provider_resumption(
    provider: &ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
) -> ResumptionProvider {
    // Detect provider type based on base URL and other characteristics
    if is_azure_provider(provider) {
        ResumptionProvider::Azure(AzureResumption::new(provider.clone(), auth_manager))
    } else if is_openai_provider(provider) {
        ResumptionProvider::OpenAI(OpenAIResumption::new(provider.clone(), auth_manager))
    } else {
        // Unknown provider or resumption not supported
        ResumptionProvider::None(NoResumption)
    }
}

/// Detect if a provider is Azure-based.
fn is_azure_provider(provider: &ModelProviderInfo) -> bool {
    // Check base URL for Azure patterns
    if let Some(base_url) = &provider.base_url {
        return base_url.contains("azure.com")
            || base_url.contains(".azure.")
            || base_url.contains("openai.azure.com");
    }

    // Check other characteristics
    provider.name.to_lowercase().contains("azure")
        || provider
            .env_key
            .as_ref()
            .map(|k| k.contains("AZURE"))
            .unwrap_or(false)
}

/// Detect if a provider is OpenAI-based.
fn is_openai_provider(provider: &ModelProviderInfo) -> bool {
    // Check base URL for OpenAI patterns
    if let Some(base_url) = &provider.base_url {
        return base_url.contains("api.openai.com") || base_url.contains("openai.com/v1");
    }

    // Check other characteristics
    provider.requires_openai_auth
        || provider
            .env_key
            .as_ref()
            .map(|k| k.contains("OPENAI"))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_provider_info::WireApi;

    fn create_test_provider(name: &str, base_url: &str, env_key: &str) -> ModelProviderInfo {
        ModelProviderInfo {
            name: name.to_string(),
            base_url: Some(base_url.to_string()),
            env_key: Some(env_key.to_string()),
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

    #[test]
    fn test_azure_provider_detection() {
        let azure_provider = create_test_provider(
            "Azure OpenAI",
            "https://myresource.openai.azure.com",
            "AZURE_OPENAI_API_KEY",
        );

        assert!(is_azure_provider(&azure_provider));
        assert!(!is_openai_provider(&azure_provider));
    }

    #[test]
    fn test_openai_provider_detection() {
        let openai_provider =
            create_test_provider("OpenAI", "https://api.openai.com", "OPENAI_API_KEY");

        assert!(!is_azure_provider(&openai_provider));
        assert!(is_openai_provider(&openai_provider));
    }

    #[test]
    fn test_unknown_provider() {
        let custom_provider = create_test_provider(
            "Custom LLM",
            "https://custom-llm.example.com",
            "CUSTOM_API_KEY",
        );

        assert!(!is_azure_provider(&custom_provider));
        assert!(!is_openai_provider(&custom_provider));
    }

    #[test]
    fn test_provider_factory() {
        let azure_provider = create_test_provider(
            "Azure OpenAI",
            "https://test.openai.azure.com",
            "AZURE_OPENAI_API_KEY",
        );

        let resumption = create_provider_resumption(&azure_provider, None);
        // Azure supports stream resumption using starting_after parameter
        assert!(resumption.supports_resumption());
    }
}
