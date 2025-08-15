use crate::error::CodexErr;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;

/// Rate limiter for web search requests
pub struct WebSearchRateLimiter {
    last_request: Mutex<Option<Instant>>,
    min_interval: Duration,
    requests_per_minute: u32,
    request_count: Mutex<u32>,
    window_start: Mutex<Instant>,
}

impl WebSearchRateLimiter {
    /// Create a new rate limiter based on model tier
    pub fn new(model_tier: &str) -> Self {
        let (rpm, min_interval_ms) = match model_tier {
            "tier-1" => (500, 120), // 500 req/min, 120ms between requests
            "tier-2" => (5000, 12), // 5000 req/min, 12ms between requests
            "tier-3" => (10000, 6), // 10000 req/min, 6ms between requests
            _ => (100, 600),        // Default conservative: 100 req/min, 600ms between
        };

        Self {
            last_request: Mutex::new(None),
            min_interval: Duration::from_millis(min_interval_ms),
            requests_per_minute: rpm,
            request_count: Mutex::new(0),
            window_start: Mutex::new(Instant::now()),
        }
    }

    /// Create a rate limiter for a specific model
    pub fn for_model(model: &str) -> Self {
        let tier = if model.contains("gpt-4o") {
            "tier-2" // Higher tier for GPT-4o models
        } else if model.contains("gpt-4") {
            "tier-1" // Standard tier for GPT-4 models
        } else if model.contains("gpt-3.5") {
            "tier-1" // Standard tier for GPT-3.5
        } else {
            "default" // Conservative default
        };

        Self::new(tier)
    }

    /// Wait if needed to respect rate limits
    pub async fn wait_if_needed(&self) -> Result<(), CodexErr> {
        // Check minimum interval between requests
        let mut last = self.last_request.lock().await;

        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.min_interval {
                let sleep_duration = self.min_interval - elapsed;
                tokio::time::sleep(sleep_duration).await;
            }
        }

        // Check requests per minute limit
        let mut count = self.request_count.lock().await;
        let mut window = self.window_start.lock().await;

        // Reset window if it's been more than a minute
        if window.elapsed() > Duration::from_secs(60) {
            *count = 0;
            *window = Instant::now();
        }

        // Check if we've exceeded the rate limit
        if *count >= self.requests_per_minute {
            return Err(CodexErr::RateLimitExceeded {
                limit: self.requests_per_minute,
                window: "1 minute".to_string(),
            });
        }

        // Update counters
        *count += 1;
        *last = Some(Instant::now());

        Ok(())
    }

    /// Get current rate limit status
    pub async fn get_status(&self) -> RateLimitStatus {
        let count = self.request_count.lock().await;
        let window = self.window_start.lock().await;
        let last = self.last_request.lock().await;

        let window_remaining = Duration::from_secs(60).saturating_sub(window.elapsed());
        let next_request_available = if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.min_interval {
                Some(self.min_interval - elapsed)
            } else {
                None
            }
        } else {
            None
        };

        RateLimitStatus {
            requests_used: *count,
            requests_limit: self.requests_per_minute,
            window_reset_in: window_remaining,
            next_request_available,
        }
    }

    /// Check if a request would be allowed without actually making it
    pub async fn would_allow_request(&self) -> bool {
        let count = self.request_count.lock().await;
        let window = self.window_start.lock().await;
        let last = self.last_request.lock().await;

        // Check if window has expired
        let window_expired = window.elapsed() > Duration::from_secs(60);

        // Check per-minute limit
        if !window_expired && *count >= self.requests_per_minute {
            return false;
        }

        // Check minimum interval
        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.min_interval {
                return false;
            }
        }

        true
    }
}

/// Status information about current rate limiting
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    pub requests_used: u32,
    pub requests_limit: u32,
    pub window_reset_in: Duration,
    pub next_request_available: Option<Duration>,
}

impl RateLimitStatus {
    /// Get a human-readable description of the rate limit status
    pub fn description(&self) -> String {
        let mut parts = Vec::new();

        parts.push(format!(
            "Requests: {}/{}",
            self.requests_used, self.requests_limit
        ));

        if self.window_reset_in > Duration::from_secs(0) {
            parts.push(format!(
                "Window resets in: {}s",
                self.window_reset_in.as_secs()
            ));
        }

        if let Some(next_available) = self.next_request_available {
            parts.push(format!("Next request in: {}ms", next_available.as_millis()));
        }

        parts.join(", ")
    }

    /// Check if we're approaching the rate limit
    pub fn is_approaching_limit(&self) -> bool {
        let usage_ratio = self.requests_used as f32 / self.requests_limit as f32;
        usage_ratio >= 0.8 // 80% of limit used (inclusive)
    }

    /// Check if we're currently rate limited
    pub fn is_rate_limited(&self) -> bool {
        self.requests_used >= self.requests_limit || self.next_request_available.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_rate_limiter_creation() {
        let limiter = WebSearchRateLimiter::new("tier-1");
        assert_eq!(limiter.requests_per_minute, 500);
        assert_eq!(limiter.min_interval, Duration::from_millis(120));

        let status = limiter.get_status().await;
        assert_eq!(status.requests_used, 0);
        assert_eq!(status.requests_limit, 500);
    }

    #[tokio::test]
    async fn test_model_specific_limiter() {
        let gpt4o_limiter = WebSearchRateLimiter::for_model("gpt-4o-search-preview");
        assert_eq!(gpt4o_limiter.requests_per_minute, 5000); // tier-2

        let gpt4_limiter = WebSearchRateLimiter::for_model("gpt-4");
        assert_eq!(gpt4_limiter.requests_per_minute, 500); // tier-1

        let unknown_limiter = WebSearchRateLimiter::for_model("unknown-model");
        assert_eq!(unknown_limiter.requests_per_minute, 100); // default
    }

    #[tokio::test]
    async fn test_basic_rate_limiting() {
        let limiter = WebSearchRateLimiter::new("test");

        // First request should succeed
        assert!(limiter.wait_if_needed().await.is_ok());

        let status = limiter.get_status().await;
        assert_eq!(status.requests_used, 1);
    }

    #[tokio::test]
    async fn test_minimum_interval() {
        let limiter = WebSearchRateLimiter::new("tier-1"); // 120ms interval

        // First request
        let start = Instant::now();
        assert!(limiter.wait_if_needed().await.is_ok());

        // Second request should wait
        assert!(limiter.wait_if_needed().await.is_ok());
        let elapsed = start.elapsed();

        // Should have waited at least the minimum interval
        assert!(elapsed >= Duration::from_millis(120));
    }

    #[tokio::test]
    async fn test_would_allow_request() {
        let limiter = WebSearchRateLimiter::new("test");

        // Should allow first request
        assert!(limiter.would_allow_request().await);

        // Make a request
        assert!(limiter.wait_if_needed().await.is_ok());

        // Immediately after, should not allow due to min interval
        assert!(!limiter.would_allow_request().await);

        // Wait until the next request is allowed, then it should pass
        let status = limiter.get_status().await;
        if let Some(delay) = status.next_request_available {
            tokio::time::sleep(delay).await;
        }
        assert!(limiter.would_allow_request().await);
    }

    #[tokio::test]
    async fn test_rate_limit_status() {
        let limiter = WebSearchRateLimiter::new("tier-1");

        // Make some requests
        for _ in 0..3 {
            assert!(limiter.wait_if_needed().await.is_ok());
        }

        let status = limiter.get_status().await;
        assert_eq!(status.requests_used, 3);
        assert_eq!(status.requests_limit, 500);
        assert!(!status.is_approaching_limit()); // 3/500 is not approaching limit

        let description = status.description();
        assert!(description.contains("Requests: 3/500"));
    }

    #[tokio::test]
    async fn test_approaching_limit() {
        // Create a limiter with very low limit for testing
        let limiter = WebSearchRateLimiter {
            last_request: Mutex::new(None),
            min_interval: Duration::from_millis(1),
            requests_per_minute: 5,
            request_count: Mutex::new(4), // 4/5 = 80%
            window_start: Mutex::new(Instant::now()),
        };

        let status = limiter.get_status().await;
        assert!(status.is_approaching_limit()); // 4/5 = 80%
    }
}
