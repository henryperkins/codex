//! Fallback no-op resumption provider for unsupported providers.

use async_trait::async_trait;
use crate::stream_resumption::context::{ProviderResumption, ResumptionContext};
use crate::client_common::ResponseEvent;
use crate::error::{Result, CodexErr};

/// No-operation resumption provider for providers that don't support stream resumption.
///
/// This is used as a safe fallback when stream resumption is enabled but the
/// provider doesn't support it. It gracefully disables resumption without breaking
/// the streaming functionality.
#[derive(Debug)]
pub struct NoResumption;

#[async_trait]
impl ProviderResumption for NoResumption {
    fn supports_resumption(&self) -> bool {
        false
    }
    
    fn max_resume_attempts(&self) -> u32 {
        0 // No attempts allowed
    }
    
    async fn create_resume_request(
        &self,
        _context: &ResumptionContext,
        _original_request_body: &serde_json::Value,
    ) -> Result<reqwest::Request> {
        Err(CodexErr::InternalServerError) // Should never be called
    }
    
    fn extract_resumption_info(
        &self,
        _event: &ResponseEvent,
        _context: &mut ResumptionContext,
    ) {
        // No-op: nothing to extract
    }
    
    fn is_resumable_error(&self, _error: &CodexErr) -> bool {
        false // No errors are resumable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_resumption_provider() {
        let provider = NoResumption;
        
        // Should not support resumption
        assert!(!provider.supports_resumption());
        
        // Should have no retry attempts
        assert_eq!(provider.max_resume_attempts(), 0);
        
        // Should not consider any error resumable
        let error = CodexErr::InternalServerError;
        assert!(!provider.is_resumable_error(&error));
    }
}