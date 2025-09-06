//! Resumption context and provider trait definitions.

use async_trait::async_trait;
use std::time::Duration;
use crate::error::Result;

/// Context information needed to resume a stream from a specific point.
#[derive(Debug, Clone)]
pub struct ResumptionContext {
    /// The response ID from the original request, used as the resumption anchor.
    pub response_id: String,
    
    /// The last sequence number we successfully received.
    pub last_sequence: Option<u64>,
    
    /// Number of resume attempts made so far.
    pub attempt_count: u32,
    
    /// Maximum number of resume attempts allowed.
    pub max_attempts: u32,
    
    /// Provider-specific metadata for resumption.
    pub provider_metadata: serde_json::Value,
}

impl ResumptionContext {
    pub fn new(response_id: String, max_attempts: u32) -> Self {
        Self {
            response_id,
            last_sequence: None,
            attempt_count: 0,
            max_attempts,
            provider_metadata: serde_json::Value::Null,
        }
    }
    
    pub fn can_retry(&self) -> bool {
        self.attempt_count < self.max_attempts
    }
    
    pub fn increment_attempt(&mut self) {
        self.attempt_count += 1;
    }
    
    pub fn update_sequence(&mut self, sequence: u64) {
        self.last_sequence = Some(sequence);
    }
}

/// Trait for provider-specific stream resumption capabilities.
/// 
/// Different providers (Azure, OpenAI, etc.) have different APIs for resuming
/// interrupted streams. This trait abstracts those differences.
#[async_trait]
pub trait ProviderResumption: Send + Sync {
    /// Check if this provider supports stream resumption.
    fn supports_resumption(&self) -> bool;
    
    /// Get the maximum number of resume attempts for this provider.
    fn max_resume_attempts(&self) -> u32 {
        3 // Conservative default
    }
    
    /// Get the delay before attempting a resume (for exponential backoff).
    fn resume_delay(&self, attempt: u32) -> Duration {
        // Exponential backoff: 500ms, 1s, 2s, 4s, ...
        let base_delay_ms = 500u64;
        let delay_ms = base_delay_ms * (2u64.pow(attempt.min(6))); // Cap at ~32 seconds
        Duration::from_millis(delay_ms)
    }
    
    /// Create a new stream request to resume from the given context.
    /// 
    /// This should construct a new HTTP request that tells the provider
    /// to continue streaming from where we left off.
    async fn create_resume_request(
        &self,
        context: &ResumptionContext,
        original_request_body: &serde_json::Value,
    ) -> Result<reqwest::Request>;
    
    /// Extract resumption information from a streaming response event.
    /// 
    /// This is called for each event to update our resumption context
    /// with any provider-specific tracking information (like sequence numbers).
    fn extract_resumption_info(
        &self,
        event: &crate::client_common::ResponseEvent,
        context: &mut ResumptionContext,
    );
    
    /// Determine if an error is recoverable via stream resumption.
    /// 
    /// Not all errors should trigger resumption attempts. This method
    /// helps distinguish between network/timeout errors (recoverable)
    /// and API errors (not recoverable).
    fn is_resumable_error(&self, error: &crate::error::CodexErr) -> bool;
}

/// Configuration for stream resumption behavior.
#[derive(Debug, Clone)]
pub struct StreamResumptionConfig {
    /// Maximum number of resume attempts before giving up.
    pub max_attempts: u32,
    
    /// Base delay for exponential backoff between attempts.
    pub base_delay_ms: u64,
    
    /// Maximum delay cap for exponential backoff.
    pub max_delay_ms: u64,
    
    /// Whether to enable detailed logging of resumption attempts.
    pub debug_logging: bool,
}

impl Default for StreamResumptionConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 500,
            max_delay_ms: 30_000, // 30 seconds max
            debug_logging: false,
        }
    }
}

impl StreamResumptionConfig {
    /// Create a configuration optimized for reliable networks.
    pub fn reliable_network() -> Self {
        Self {
            max_attempts: 2,
            base_delay_ms: 250,
            max_delay_ms: 5_000, // 5 seconds max
            debug_logging: false,
        }
    }
    
    /// Create a configuration optimized for unreliable networks.
    pub fn unreliable_network() -> Self {
        Self {
            max_attempts: 5,
            base_delay_ms: 1_000,
            max_delay_ms: 60_000, // 1 minute max
            debug_logging: true,
        }
    }
    
    /// Calculate the delay for a given attempt number.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = self.base_delay_ms * (2u64.pow(attempt.min(10)));
        let capped_delay_ms = delay_ms.min(self.max_delay_ms);
        Duration::from_millis(capped_delay_ms)
    }
}