use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::rate_limiter::{AdaptiveRateLimiter, CircuitBreaker, TokenBucket};
use tiktoken_rs::{cl100k_base, o200k_base};

/// Model-specific rate limits for Azure OpenAI
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModelRateLimits {
    pub tokens_per_minute: u32,
    pub requests_per_minute: u32,
}

impl Default for ModelRateLimits {
    fn default() -> Self {
        Self {
            tokens_per_minute: 30000,
            requests_per_minute: 300,
        }
    }
}

/// Azure OpenAI specific rate limiter
#[derive(Debug)]
pub struct AzureOpenAIRateLimiter {
    model_limits: Arc<Mutex<HashMap<String, ModelRateLimits>>>,
    token_buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
    request_buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
    circuit_breaker: CircuitBreaker,
    adaptive_limiter: AdaptiveRateLimiter,
    /// Tracks the most recent acquire context so we can attribute response
    /// headers to the right buckets.
    last_context: Arc<Mutex<Option<LimiterContext>>>,
}

#[derive(Debug, Clone)]
struct LimiterContext {
    bucket_key: String,
    model_hint: String,
}

#[allow(dead_code)]
impl AzureOpenAIRateLimiter {
    /// Create a new rate limiter with default configuration
    pub fn new() -> Self {
        Self::with_config(AzureRateLimitConfig::default())
    }

    /// Create a new rate limiter with custom configuration
    pub fn with_config(config: AzureRateLimitConfig) -> Self {
        let mut model_limits = Self::default_model_limits();

        // Log the configuration being used
        info!("Initializing Azure rate limiter with config: {:?}", config);

        // Warn about quota allocation
        warn!(
            "⚠️ Azure quota check: Ensure your deployment has sufficient quota allocation. \
            GPT-5 models may need quota increase via Azure portal if seeing frequent rate limits."
        );

        // Override with any custom limits from config
        for (model, limits) in config.custom_limits {
            model_limits.insert(model, limits);
        }

        Self {
            model_limits: Arc::new(Mutex::new(model_limits)),
            token_buckets: Arc::new(Mutex::new(HashMap::new())),
            request_buckets: Arc::new(Mutex::new(HashMap::new())),
            circuit_breaker: CircuitBreaker::new(
                config.circuit_breaker_threshold,
                2, // success threshold
                Duration::from_secs(config.circuit_breaker_timeout_secs),
            ),
            adaptive_limiter: AdaptiveRateLimiter::new(
                if config.aggressive_throttling {
                    5.0
                } else {
                    10.0
                }, // initial rate
                1.0, // min rate
                if config.aggressive_throttling {
                    30.0
                } else {
                    50.0
                }, // max rate
            ),
            last_context: Arc::new(Mutex::new(None)),
        }
    }

    /// Get default model limits
    fn default_model_limits() -> HashMap<String, ModelRateLimits> {
        let mut model_limits = HashMap::new();

        // GPT-5 models with 20,000 TPM
        model_limits.insert(
            "gpt-5".to_string(),
            ModelRateLimits {
                tokens_per_minute: 20000,
                requests_per_minute: 200,
            },
        );
        model_limits.insert(
            "gpt-5-mini".to_string(),
            ModelRateLimits {
                tokens_per_minute: 20000,
                requests_per_minute: 200,
            },
        );
        model_limits.insert(
            "gpt-5-nano".to_string(),
            ModelRateLimits {
                tokens_per_minute: 20000,
                requests_per_minute: 200,
            },
        );
        model_limits.insert(
            "gpt-5-chat".to_string(),
            ModelRateLimits {
                tokens_per_minute: 20000,
                requests_per_minute: 200,
            },
        );

        // GPT-4 models with higher limits
        model_limits.insert(
            "gpt-4o".to_string(),
            ModelRateLimits {
                tokens_per_minute: 30000,
                requests_per_minute: 300,
            },
        );
        model_limits.insert(
            "gpt-4o-mini".to_string(),
            ModelRateLimits {
                tokens_per_minute: 30000,
                requests_per_minute: 300,
            },
        );

        // GPT-4.1 models
        model_limits.insert(
            "gpt-4.1".to_string(),
            ModelRateLimits {
                tokens_per_minute: 30000,
                requests_per_minute: 300,
            },
        );
        model_limits.insert(
            "gpt-4.1-nano".to_string(),
            ModelRateLimits {
                tokens_per_minute: 30000,
                requests_per_minute: 300,
            },
        );
        model_limits.insert(
            "gpt-4.1-mini".to_string(),
            ModelRateLimits {
                tokens_per_minute: 30000,
                requests_per_minute: 300,
            },
        );

        // Reasoning models (o1, o3, o4) with conservative limits
        model_limits.insert(
            "o1".to_string(),
            ModelRateLimits {
                tokens_per_minute: 10000,
                requests_per_minute: 50,
            },
        );
        model_limits.insert(
            "o3".to_string(),
            ModelRateLimits {
                tokens_per_minute: 10000,
                requests_per_minute: 50,
            },
        );
        model_limits.insert(
            "o3-mini".to_string(),
            ModelRateLimits {
                tokens_per_minute: 15000,
                requests_per_minute: 100,
            },
        );
        model_limits.insert(
            "o4-mini".to_string(),
            ModelRateLimits {
                tokens_per_minute: 15000,
                requests_per_minute: 100,
            },
        );

        model_limits
    }

    /// Get or create token bucket for a model
    async fn get_token_bucket(&self, model: &str) -> TokenBucket {
        let mut buckets = self.token_buckets.lock().await;

        if !buckets.contains_key(model) {
            let limits = self
                .model_limits
                .lock()
                .await
                .get(model)
                .cloned()
                .unwrap_or_default();

            // Create token bucket with per-second refill rate
            let tokens_per_second = limits.tokens_per_minute as f64 / 60.0;
            let bucket = TokenBucket::new(limits.tokens_per_minute as f64, tokens_per_second);

            info!(
                "Created token bucket for {}: {} TPM ({:.1} TPS)",
                model, limits.tokens_per_minute, tokens_per_second
            );

            buckets.insert(model.to_string(), bucket.clone());
            bucket
        } else {
            buckets.get(model).unwrap().clone()
        }
    }

    /// Get or create token bucket using a deployment key with capacity derived from a model hint.
    async fn get_token_bucket_for_key(&self, bucket_key: &str, model_hint: &str) -> TokenBucket {
        let mut buckets = self.token_buckets.lock().await;
        if !buckets.contains_key(bucket_key) {
            let limits = self
                .model_limits
                .lock()
                .await
                .get(model_hint)
                .cloned()
                .unwrap_or_default();
            let tokens_per_second = limits.tokens_per_minute as f64 / 60.0;
            let bucket = TokenBucket::new(limits.tokens_per_minute as f64, tokens_per_second);
            info!(
                "Created token bucket for {} (hint {}): {} TPM ({:.1} TPS)",
                bucket_key, model_hint, limits.tokens_per_minute, tokens_per_second
            );
            buckets.insert(bucket_key.to_string(), bucket.clone());
            bucket
        } else {
            buckets.get(bucket_key).unwrap().clone()
        }
    }

    /// Get or create request bucket for a model
    async fn get_request_bucket(&self, model: &str) -> TokenBucket {
        let mut buckets = self.request_buckets.lock().await;

        if !buckets.contains_key(model) {
            let limits = self
                .model_limits
                .lock()
                .await
                .get(model)
                .cloned()
                .unwrap_or_default();

            // Create request bucket with per-second refill rate
            let requests_per_second = limits.requests_per_minute as f64 / 60.0;
            let bucket = TokenBucket::new(limits.requests_per_minute as f64, requests_per_second);

            info!(
                "Created request bucket for {}: {} RPM ({:.1} RPS)",
                model, limits.requests_per_minute, requests_per_second
            );

            buckets.insert(model.to_string(), bucket.clone());
            bucket
        } else {
            buckets.get(model).unwrap().clone()
        }
    }

    /// Get or create request bucket using a deployment key with capacity derived from a model hint.
    async fn get_request_bucket_for_key(&self, bucket_key: &str, model_hint: &str) -> TokenBucket {
        let mut buckets = self.request_buckets.lock().await;
        if !buckets.contains_key(bucket_key) {
            let limits = self
                .model_limits
                .lock()
                .await
                .get(model_hint)
                .cloned()
                .unwrap_or_default();
            let requests_per_second = limits.requests_per_minute as f64 / 60.0;
            let bucket = TokenBucket::new(limits.requests_per_minute as f64, requests_per_second);
            info!(
                "Created request bucket for {} (hint {}): {} RPM ({:.1} RPS)",
                bucket_key, model_hint, limits.requests_per_minute, requests_per_second
            );
            buckets.insert(bucket_key.to_string(), bucket.clone());
            bucket
        } else {
            buckets.get(bucket_key).unwrap().clone()
        }
    }

    /// Acquire rate limit permits for a request with adaptive pacing
    pub async fn acquire(&self, model: &str, estimated_tokens: u32) -> Result<(), String> {
        // Remember context so we can apply dynamic limits from headers later.
        *self.last_context.lock().await = Some(LimiterContext {
            bucket_key: model.to_string(),
            model_hint: model.to_string(),
        });
        // Check circuit breaker first
        if !self.circuit_breaker.is_allowed().await {
            warn!("Circuit breaker is open, rejecting request");
            return Err("Circuit breaker is open - too many failures".to_string());
        }

        // Adaptive pacing: space requests based on the dynamic rate target.
        let current_rate = self.adaptive_limiter.get_rate().await;
        if current_rate > 0.0 {
            // Minimal pacing to avoid bursts; rely on buckets for hard limits.
            let wait_s = 1.0f64 / current_rate;
            debug!(
                "Adaptive pacing delay: {:.3}s at {:.1} rps",
                wait_s, current_rate
            );
            sleep(Duration::from_secs_f64(wait_s)).await;
        }

        // Get buckets for this model
        let token_bucket = self.get_token_bucket(model).await;
        let request_bucket = self.get_request_bucket(model).await;

        // Log current state
        let available_tokens = token_bucket.available_tokens().await;
        let available_requests = request_bucket.available_tokens().await;

        debug!(
            "Model {}: Available tokens: {:.0}, Available requests: {:.0}, Requesting: {} tokens",
            model, available_tokens, available_requests, estimated_tokens
        );

        // Guard: if a single request needs more tokens than the per‑minute capacity,
        // it can never succeed. Fail fast with an actionable error.
        let capacity = self
            .model_limits
            .lock()
            .await
            .get(model)
            .cloned()
            .unwrap_or_default()
            .tokens_per_minute;
        if estimated_tokens > capacity {
            warn!(
                "Request for {} tokens exceeds per‑minute capacity for {}: {}",
                estimated_tokens, model, capacity
            );
            return Err(format!(
                "Request exceeds token capacity for {model}: {estimated_tokens} > {capacity}"
            ));
        }

        // Acquire tokens first, then request permit. This avoids consuming RPM
        // when we cannot cover tokens; once tokens are available we will wait
        // for an RPM slot, not fail and leak capacity.
        token_bucket
            .acquire(estimated_tokens as f64)
            .await
            .map_err(|e| format!("token acquire failed: {e}"))?;
        request_bucket
            .acquire(1.0)
            .await
            .map_err(|e| format!("request acquire failed: {e}"))?;

        info!(
            "Acquired permits for {}: {} tokens and 1 request",
            model, estimated_tokens
        );
        Ok(())
    }

    /// Acquire permits using a deployment key for bucketing and a model hint for capacity.
    pub async fn acquire_for_deployment(
        &self,
        deployment: &str,
        model_hint: &str,
        estimated_tokens: u32,
    ) -> Result<(), String> {
        *self.last_context.lock().await = Some(LimiterContext {
            bucket_key: deployment.to_string(),
            model_hint: model_hint.to_string(),
        });
        if !self.circuit_breaker.is_allowed().await {
            warn!("Circuit breaker is open, rejecting request");
            return Err("Circuit breaker is open - too many failures".to_string());
        }

        let current_rate = self.adaptive_limiter.get_rate().await;
        if current_rate > 0.0 {
            let wait_s = 1.0f64 / current_rate;
            debug!(
                "Adaptive pacing delay: {:.3}s at {:.1} rps (deployment {})",
                wait_s, current_rate, deployment
            );
            sleep(Duration::from_secs_f64(wait_s)).await;
        }

        let token_bucket = self.get_token_bucket_for_key(deployment, model_hint).await;
        let request_bucket = self
            .get_request_bucket_for_key(deployment, model_hint)
            .await;

        let available_tokens = token_bucket.available_tokens().await;
        let available_requests = request_bucket.available_tokens().await;
        debug!(
            "Deployment {} ({}): Available tokens: {:.0}, Available requests: {:.0}, Requesting: {} tokens",
            deployment, model_hint, available_tokens, available_requests, estimated_tokens
        );

        let capacity = self
            .model_limits
            .lock()
            .await
            .get(model_hint)
            .cloned()
            .unwrap_or_default()
            .tokens_per_minute;
        if estimated_tokens > capacity {
            warn!(
                "Request for {} tokens exceeds per‑minute capacity for {} (hint {}): {}",
                estimated_tokens, deployment, model_hint, capacity
            );
            return Err(format!(
                "Request exceeds token capacity for {deployment}: {estimated_tokens} > {capacity}"
            ));
        }

        token_bucket
            .acquire(estimated_tokens as f64)
            .await
            .map_err(|e| format!("token acquire failed: {e}"))?;
        request_bucket
            .acquire(1.0)
            .await
            .map_err(|e| format!("request acquire failed: {e}"))?;
        info!(
            "Acquired permits for deployment {} ({}): {} tokens and 1 request",
            deployment, model_hint, estimated_tokens
        );
        Ok(())
    }

    /// Update rate limits from response headers
    pub async fn update_from_response(&self, headers: &reqwest::header::HeaderMap) {
        let mut remaining_requests = None;
        let mut remaining_tokens = None;
        let mut reset_requests = None;
        let mut reset_tokens = None;
        let mut limit_requests = None;
        let mut limit_tokens = None;

        // Parse Azure OpenAI specific headers
        if let Some(val) = headers.get("x-ratelimit-remaining-requests") {
            if let Ok(s) = val.to_str() {
                remaining_requests = s.parse::<u32>().ok();
            }
        }

        if let Some(val) = headers.get("x-ratelimit-remaining-tokens") {
            if let Ok(s) = val.to_str() {
                remaining_tokens = s.parse::<u32>().ok();
            }
        }

        if let Some(val) = headers.get("x-ratelimit-reset-requests") {
            if let Ok(s) = val.to_str() {
                reset_requests = s.parse::<u64>().ok();
            }
        }

        if let Some(val) = headers.get("x-ratelimit-reset-tokens") {
            if let Ok(s) = val.to_str() {
                reset_tokens = s.parse::<u64>().ok();
            }
        }
        // Capacity limits
        if let Some(val) = headers.get("x-ratelimit-limit-requests") {
            if let Ok(s) = val.to_str() {
                limit_requests = s.parse::<u32>().ok();
            }
        }
        if let Some(val) = headers.get("x-ratelimit-limit-tokens") {
            if let Ok(s) = val.to_str() {
                limit_tokens = s.parse::<u32>().ok();
            }
        }

        // Use the most restrictive reset time
        let reset_seconds = match (reset_requests, reset_tokens) {
            (Some(r), Some(t)) => Some(r.max(t)),
            (Some(r), None) => Some(r),
            (None, Some(t)) => Some(t),
            _ => None,
        };

        // Update adaptive limiter
        self.adaptive_limiter
            .update_from_headers(remaining_requests, remaining_tokens, reset_seconds)
            .await;

        // Log the current limits
        if remaining_requests.is_some() || remaining_tokens.is_some() {
            info!(
                "Azure rate limit status - Remaining requests: {:?}, Remaining tokens: {:?}, Reset in: {:?}s",
                remaining_requests, remaining_tokens, reset_seconds
            );
        }

        // If we're very low on tokens, add extra delay and warn
        if let Some(tokens) = remaining_tokens {
            if tokens < 1000 {
                warn!(
                    "⚠️ Low token count: {} remaining. Consider reducing request frequency.",
                    tokens
                );
            }
        }

        // Also warn if low on requests
        if let Some(requests) = remaining_requests {
            if requests < 10 {
                warn!(
                    "⚠️ Low request count: {} remaining. Consider spacing out requests.",
                    requests
                );
            }
        }
        // Apply dynamic per-minute capacities when available.
        if limit_requests.is_some() || limit_tokens.is_some() {
            if let Some(ctx) = self.last_context.lock().await.clone() {
                self.apply_dynamic_limits(&ctx, limit_tokens, limit_requests)
                    .await;
            }
        }
    }

    async fn apply_dynamic_limits(
        &self,
        ctx: &LimiterContext,
        limit_tokens: Option<u32>,
        limit_requests: Option<u32>,
    ) {
        // Update defaults
        let mut limits = self
            .model_limits
            .lock()
            .await
            .get(&ctx.model_hint)
            .cloned()
            .unwrap_or_default();
        if let Some(tpm) = limit_tokens {
            limits.tokens_per_minute = tpm;
        }
        if let Some(rpm) = limit_requests {
            limits.requests_per_minute = rpm;
        }
        self.model_limits
            .lock()
            .await
            .insert(ctx.model_hint.clone(), limits.clone());

        // Resize token bucket for this key
        if let Some(tpm) = limit_tokens {
            let mut buckets = self.token_buckets.lock().await;
            if let Some(old) = buckets.get(&ctx.bucket_key).cloned() {
                let old_avail = old.available_tokens().await;
                let cap = tpm as f64;
                let rps = cap / 60.0;
                let new_bucket = TokenBucket::new(cap, rps);
                let target = old_avail.min(cap);
                let debit = (cap - target).max(0.0);
                new_bucket.force_debit(debit).await;
                buckets.insert(ctx.bucket_key.clone(), new_bucket);
                info!(
                    "Adjusted token bucket for {} (hint {}): {} TPM ({:.1} TPS)",
                    ctx.bucket_key, ctx.model_hint, tpm, rps
                );
            }
        }

        // Resize request bucket for this key
        if let Some(rpm) = limit_requests {
            let mut buckets = self.request_buckets.lock().await;
            if let Some(old) = buckets.get(&ctx.bucket_key).cloned() {
                let old_avail = old.available_tokens().await;
                let cap = rpm as f64;
                let rps = cap / 60.0;
                let new_bucket = TokenBucket::new(cap, rps);
                let target = old_avail.min(cap);
                let debit = (cap - target).max(0.0);
                new_bucket.force_debit(debit).await;
                buckets.insert(ctx.bucket_key.clone(), new_bucket);
                info!(
                    "Adjusted request bucket for {} (hint {}): {} RPM ({:.1} RPS)",
                    ctx.bucket_key, ctx.model_hint, rpm, rps
                );
            }
        }
    }

    /// Reconcile token pre‑charge with actual usage from Responses API.
    pub async fn reconcile_after_completed(
        &self,
        bucket_key: &str,
        model_hint: &str,
        estimated_tokens: u32,
        actual: crate::protocol::TokenUsage,
    ) {
        let actual_total = actual.total_tokens as i64;
        let delta = estimated_tokens as i64 - actual_total;
        let bucket = self.get_token_bucket_for_key(bucket_key, model_hint).await;
        if delta > 0 {
            bucket.refund(delta as f64).await;
            debug!(
                "Refunded {} tokens to {} (hint {})",
                delta, bucket_key, model_hint
            );
        } else if delta < 0 {
            bucket.force_debit((-delta) as f64).await;
            debug!(
                "Debited {} extra tokens from {} (hint {})",
                -delta, bucket_key, model_hint
            );
        }
    }

    /// Record successful request
    pub async fn record_success(&self) {
        self.circuit_breaker.record_success().await;
    }

    /// Record failed request
    pub async fn record_failure(&self) {
        self.circuit_breaker.record_failure().await;
    }

    /// Estimate tokens for text using tiktoken and a model-appropriate encoding.
    pub fn estimate_tokens_for_model(model: &str, text: &str) -> u32 {
        let enc = match Self::encoding_for_model(model) {
            Ok(e) => e,
            Err(_) => {
                // Fallback to cl100k_base if anything goes wrong.
                cl100k_base().expect("cl100k_base encoding must be available")
            }
        };
        // Count including special tokens to be conservative.
        enc.encode_with_special_tokens(text).len() as u32
    }

    fn encoding_for_model(model: &str) -> Result<tiktoken_rs::CoreBPE, Box<dyn std::error::Error>> {
        // Heuristic mapping – most modern OpenAI models use o200k_base; older use cl100k_base.
        if model.starts_with("gpt-4o")
            || model.starts_with("gpt-4.1")
            || model.starts_with("o1")
            || model.starts_with("o3")
            || model.starts_with("gpt-5")
        {
            Ok(o200k_base()?)
        } else {
            Ok(cl100k_base()?)
        }
    }

    /// Get current status for monitoring
    pub async fn get_status(&self, model: &str) -> RateLimiterStatus {
        let token_bucket = self.get_token_bucket(model).await;
        let request_bucket = self.get_request_bucket(model).await;

        RateLimiterStatus {
            model: model.to_string(),
            available_tokens: token_bucket.available_tokens().await as u32,
            available_requests: request_bucket.available_tokens().await as u32,
            circuit_breaker_open: !self.circuit_breaker.is_allowed().await,
            should_throttle: self.adaptive_limiter.should_throttle().await,
            current_rate: self.adaptive_limiter.get_rate().await,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RateLimiterStatus {
    pub model: String,
    pub available_tokens: u32,
    pub available_requests: u32,
    pub circuit_breaker_open: bool,
    pub should_throttle: bool,
    pub current_rate: f64,
}

/// Configuration for Azure rate limiting
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct AzureRateLimitConfig {
    pub enabled: bool,
    pub custom_limits: HashMap<String, ModelRateLimits>,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout_secs: u64,
    pub aggressive_throttling: bool,
}

impl Default for AzureRateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            custom_limits: HashMap::new(),
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout_secs: 30,
            aggressive_throttling: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gpt5_rate_limit() {
        let limiter = AzureOpenAIRateLimiter::new();

        // GPT-5 should allow up to 20k tokens
        assert!(limiter.acquire("gpt-5", 10000).await.is_ok());

        // Should reject requests over 20k
        assert!(limiter.acquire("gpt-5", 25000).await.is_err());
    }

    #[tokio::test]
    async fn test_token_estimation() {
        let text = "Hello, this is a test message.";
        // For modern models, use o200k_base; just ensure non-zero and stable for this input.
        let estimated = AzureOpenAIRateLimiter::estimate_tokens_for_model("gpt-4o", text);
        assert!(estimated > 0);
    }

    #[tokio::test]
    async fn test_model_specific_limits() {
        let limiter = AzureOpenAIRateLimiter::new();

        // GPT-4o should have different limits than GPT-5
        let gpt4_status = limiter.get_status("gpt-4o").await;
        let gpt5_status = limiter.get_status("gpt-5").await;

        // GPT-4o should have 30k tokens available initially
        assert_eq!(gpt4_status.available_tokens, 30000);

        // GPT-5 should have 20k tokens available initially
        assert_eq!(gpt5_status.available_tokens, 20000);
    }
}
