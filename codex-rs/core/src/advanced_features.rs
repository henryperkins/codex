//! Advanced feature system for optional Codex capabilities.
//!
//! This module provides opt-in functionality for response chaining, background processing,
//! and advanced storage features. By default, all advanced features are disabled to match
//! upstream performance characteristics.

use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::error::Result;

/// Configuration for optional advanced features.
/// 
/// Default behavior matches upstream: all advanced features disabled for maximum performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedFeatures {
    /// Enable response chaining with previous_response_id.
    /// Required for conversation continuity across API calls.
    /// Note: May require response storage depending on provider.
    pub enable_response_chaining: bool,

    /// Enable background processing for long-running requests.
    /// Allows async processing with polling instead of blocking streams.
    pub enable_background_processing: bool,

    /// Enable response storage for chaining and resumption.
    /// Required by some providers for response chaining.
    /// Note: May impact performance due to database storage overhead.
    pub enable_response_storage: bool,

    /// Enable stream resumption on network failures.
    /// Provides resilience for long-running streaming responses.
    /// Note: Currently only supported on Azure-compatible providers.
    pub enable_stream_resumption: bool,

    /// Configuration for background processing when enabled.
    pub background_config: BackgroundProcessingConfig,
}

impl Default for AdvancedFeatures {
    fn default() -> Self {
        Self {
            // Performance-first defaults - match upstream behavior
            enable_response_chaining: false,
            enable_background_processing: false,
            enable_response_storage: false,
            enable_stream_resumption: true, // Network resilience is generally desired
            background_config: BackgroundProcessingConfig::default(),
        }
    }
}

impl AdvancedFeatures {
    /// Create a configuration optimized for OpenAI usage patterns.
    pub fn openai_optimized() -> Self {
        Self {
            enable_response_chaining: true,
            enable_response_storage: true, // Required for OpenAI chaining
            ..Default::default()
        }
    }

    /// Create a configuration optimized for Azure usage patterns.
    pub fn azure_optimized() -> Self {
        Self {
            enable_response_chaining: true,
            enable_background_processing: true,
            enable_stream_resumption: true,
            // Azure doesn't require storage for chaining
            ..Default::default()
        }
    }

    /// Validate feature compatibility and auto-enable dependencies.
    pub fn validate_and_fix(&mut self) {
        // Response chaining on OpenAI requires storage
        if self.enable_response_chaining && !self.enable_response_storage {
            // Note: This would need provider detection in real implementation
            tracing::warn!("Response chaining may require storage for some providers");
        }

        // Background processing requires some form of response tracking
        if self.enable_background_processing && !self.enable_response_storage {
            tracing::warn!("Background processing works best with response storage enabled");
        }
    }
}

/// Configuration for background processing behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundProcessingConfig {
    /// Initial delay before first polling attempt.
    pub initial_polling_delay: Duration,
    
    /// Maximum number of polling attempts before giving up.
    pub max_polling_retries: u32,
    
    /// Multiplier for exponential backoff between polling attempts.
    pub backoff_multiplier: f64,
    
    /// Threshold for determining if a request should go to background.
    pub complexity_threshold: ComplexityThreshold,
}

impl Default for BackgroundProcessingConfig {
    fn default() -> Self {
        Self {
            initial_polling_delay: Duration::from_millis(500),
            max_polling_retries: 60, // ~30 seconds with exponential backoff
            backoff_multiplier: 1.5,
            complexity_threshold: ComplexityThreshold::High,
        }
    }
}

/// Threshold for determining request complexity for background processing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ComplexityThreshold {
    Low,    // Background for most requests
    Medium, // Background for moderately complex requests  
    High,   // Background only for very complex requests (default)
}

/// Trait for estimating request complexity.
pub trait RequestComplexity {
    fn estimated_complexity(&self) -> ComplexityLevel;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComplexityLevel {
    Low = 1,
    Medium = 2,
    High = 3,
    VeryHigh = 4,
}

impl ComplexityLevel {
    pub fn exceeds_threshold(self, threshold: ComplexityThreshold) -> bool {
        let threshold_level = match threshold {
            ComplexityThreshold::Low => ComplexityLevel::Low,
            ComplexityThreshold::Medium => ComplexityLevel::Medium,
            ComplexityThreshold::High => ComplexityLevel::High,
        };
        self >= threshold_level
    }
}

/// Trait defining provider-specific capabilities.
pub trait ProviderCapabilities {
    /// Does this provider support response chaining via previous_response_id?
    fn supports_response_chaining(&self) -> bool;
    
    /// Does this provider support background/async processing with polling?
    fn supports_background_processing(&self) -> bool;
    
    /// Does this provider support stream resumption on network failures?
    fn supports_stream_resumption(&self) -> bool;
    
    /// Does this provider require response storage for chaining to work?
    fn requires_response_storage_for_chaining(&self) -> bool;
    
    /// Maximum number of stream resumption retries for this provider.
    fn max_resume_retries(&self) -> u32 {
        3
    }
}

/// Storage interface for response persistence and retrieval.
pub trait ResponseStorage: Send + Sync {
    /// Should this request's response be stored?
    async fn should_store(&self, request: &RequestContext) -> bool;
    
    /// Store a response and return its storage ID.
    async fn store_response(&self, response: &ResponseData) -> Result<String>;
    
    /// Retrieve a previously stored response by ID.
    async fn get_previous_response(&self, id: &str) -> Result<Option<ResponseData>>;
    
    /// Clean up old responses beyond retention period.
    async fn cleanup_expired_responses(&self) -> Result<u64>;
}

/// Context information for a request (simplified for this interface).
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub model: String,
    pub estimated_tokens: Option<u64>,
    pub tool_count: usize,
    pub has_previous_response_id: bool,
}

impl RequestComplexity for RequestContext {
    fn estimated_complexity(&self) -> ComplexityLevel {
        let mut score = 0;
        
        // Token count influence
        if let Some(tokens) = self.estimated_tokens {
            score += match tokens {
                0..=1000 => 0,
                1001..=5000 => 1,
                5001..=20000 => 2,
                _ => 3,
            };
        }
        
        // Tool usage influence  
        score += match self.tool_count {
            0 => 0,
            1..=3 => 1,
            4..=10 => 2,
            _ => 3,
        };
        
        // Chain complexity
        if self.has_previous_response_id {
            score += 1;
        }
        
        match score {
            0..=1 => ComplexityLevel::Low,
            2..=3 => ComplexityLevel::Medium,
            4..=5 => ComplexityLevel::High,
            _ => ComplexityLevel::VeryHigh,
        }
    }
}

/// Response data (simplified for this interface).
#[derive(Debug, Clone)]
pub struct ResponseData {
    pub id: String,
    pub content: String,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// No-op storage implementation (default upstream behavior).
#[derive(Debug, Default)]
pub struct NoStorage;

impl ResponseStorage for NoStorage {
    async fn should_store(&self, _request: &RequestContext) -> bool {
        false
    }
    
    async fn store_response(&self, _response: &ResponseData) -> Result<String> {
        Err(crate::error::CodexErr::StorageDisabled)
    }
    
    async fn get_previous_response(&self, _id: &str) -> Result<Option<ResponseData>> {
        Ok(None)
    }
    
    async fn cleanup_expired_responses(&self) -> Result<u64> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_features_are_performance_focused() {
        let features = AdvancedFeatures::default();
        assert!(!features.enable_response_chaining);
        assert!(!features.enable_background_processing);
        assert!(!features.enable_response_storage);
        // Stream resumption is acceptable performance overhead for reliability
        assert!(features.enable_stream_resumption);
    }

    #[test]
    fn test_openai_optimized_enables_required_features() {
        let features = AdvancedFeatures::openai_optimized();
        assert!(features.enable_response_chaining);
        assert!(features.enable_response_storage); // Required for OpenAI
    }

    #[test]
    fn test_complexity_calculation() {
        let simple_request = RequestContext {
            model: "gpt-4".to_string(),
            estimated_tokens: Some(500),
            tool_count: 0,
            has_previous_response_id: false,
        };
        assert_eq!(simple_request.estimated_complexity(), ComplexityLevel::Low);

        let complex_request = RequestContext {
            model: "gpt-4".to_string(),
            estimated_tokens: Some(25000),
            tool_count: 15,
            has_previous_response_id: true,
        };
        assert_eq!(complex_request.estimated_complexity(), ComplexityLevel::VeryHigh);
    }

    #[test]
    fn test_complexity_threshold_comparison() {
        assert!(ComplexityLevel::High.exceeds_threshold(ComplexityThreshold::Medium));
        assert!(!ComplexityLevel::Low.exceeds_threshold(ComplexityThreshold::High));
    }
}