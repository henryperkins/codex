//! Stream resumption module for automatic recovery from network failures.
//!
//! This module provides transparent stream resumption capabilities that can recover
//! from network interruptions during long-running streaming API responses. It's designed
//! to be completely modular and non-invasive to upstream code changes.

mod context;
mod wrapper;
mod providers;

pub use context::{ResumptionContext, ProviderResumption};
pub use wrapper::{ResumableStream, ResumableResponseStream};
pub use providers::{create_provider_resumption, ResumptionProvider, NoResumption};

use crate::advanced_features::AdvancedFeatures;
use crate::client_common::ResponseStream;
use crate::model_provider_info::ModelProviderInfo;

/// Main entry point for enabling stream resumption on any ResponseStream.
///
/// This is the only function that external code needs to use. It automatically
/// detects provider capabilities and wraps the stream with resumption logic
/// if enabled in the advanced features configuration.
pub fn maybe_enable_resumption(
    stream: ResponseStream,
    provider: &ModelProviderInfo,
    features: &AdvancedFeatures,
) -> ResponseStream {
    if !features.enable_stream_resumption {
        // Zero overhead when disabled - return stream as-is
        return stream;
    }

    // Wrap with resumption capabilities
    let resumable = ResumableStream::new(stream, provider, features);
    resumable.into_response_stream()
}

/// Check if a provider supports stream resumption.
pub fn provider_supports_resumption(provider: &ModelProviderInfo) -> bool {
    // For now, primarily Azure-based providers
    provider.is_probably_azure() || 
    provider.base_url.as_ref()
        .map(|url| url.contains("azure") || url.contains("openai.azure.com"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advanced_features::AdvancedFeatures;
    use crate::model_provider_info::{ModelProviderInfo, WireApi};

    #[test]
    fn test_provider_resumption_detection() {
        let azure_provider = ModelProviderInfo {
            name: "Azure OpenAI".to_string(),
            base_url: Some("https://myresource.openai.azure.com".to_string()),
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
        };

        assert!(provider_supports_resumption(&azure_provider));

        let openai_provider = ModelProviderInfo {
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
            stream_max_retries: Some(0), // No stream resumption
            stream_idle_timeout_ms: Some(30000),
            requires_openai_auth: true,
        };

        assert!(!provider_supports_resumption(&openai_provider));
    }

    #[test]
    fn test_resumption_enabled_by_default() {
        let features = AdvancedFeatures::default();
        assert!(features.enable_stream_resumption); // Network resilience is enabled by default
        
        // Provider-optimized configs should also have it enabled
        let azure_features = AdvancedFeatures::azure_optimized();
        assert!(azure_features.enable_stream_resumption);
        
        let openai_features = AdvancedFeatures::openai_optimized();
        assert!(openai_features.enable_stream_resumption);
    }
}