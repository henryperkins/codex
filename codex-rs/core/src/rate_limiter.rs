use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::sleep;
use tracing::{debug, warn};

/// Token bucket implementation for rate limiting
#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: f64,
    tokens: Arc<Mutex<f64>>,
    refill_rate: f64,
    last_refill: Arc<Mutex<Instant>>,
}

impl TokenBucket {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: Arc::new(Mutex::new(capacity)),
            refill_rate,
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Acquire tokens, waiting if necessary
    pub async fn acquire(&self, tokens_needed: f64) -> Result<(), String> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 100;

        loop {
            self.refill().await;

            let mut tokens = self.tokens.lock().await;
            if *tokens >= tokens_needed {
                *tokens -= tokens_needed;
                debug!("Acquired {} tokens, {} remaining", tokens_needed, *tokens);
                return Ok(());
            }

            // Calculate wait time
            let tokens_deficit = tokens_needed - *tokens;
            let wait_time = Duration::from_secs_f64(tokens_deficit / self.refill_rate);

            drop(tokens); // Release lock before sleeping

            if attempts >= MAX_ATTEMPTS {
                return Err("Max attempts reached waiting for tokens".to_string());
            }

            debug!("Waiting {:?} for {} tokens", wait_time, tokens_needed);
            sleep(wait_time).await;
            attempts += 1;
        }
    }

    /// Refill tokens based on elapsed time
    async fn refill(&self) {
        let mut last_refill = self.last_refill.lock().await;
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill).as_secs_f64();

        let mut tokens = self.tokens.lock().await;
        let new_tokens = (*tokens + elapsed * self.refill_rate).min(self.capacity);

        if new_tokens > *tokens {
            debug!("Refilled tokens: {} -> {}", *tokens, new_tokens);
            *tokens = new_tokens;
        }

        *last_refill = now;
    }

    /// Get current token count (for monitoring)
    pub async fn available_tokens(&self) -> f64 {
        self.refill().await;
        *self.tokens.lock().await
    }

    /// Refund tokens immediately without waiting, clamped to capacity.
    pub async fn refund(&self, tokens: f64) {
        if tokens <= 0.0 {
            return;
        }
        let mut current = self.tokens.lock().await;
        *current = (*current + tokens).min(self.capacity);
    }

    /// Force a debit of tokens immediately (no waiting), clamped at zero.
    pub async fn force_debit(&self, tokens: f64) {
        if tokens <= 0.0 {
            return;
        }
        let mut current = self.tokens.lock().await;
        *current = (*current - tokens).max(0.0);
    }
}

/// Circuit breaker states
#[derive(Debug, Clone)]
pub enum CircuitState {
    Closed,
    Open { until: Instant },
    HalfOpen,
}

/// Circuit breaker for preventing cascading failures
#[derive(Debug)]
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failure_count: Arc<Mutex<u32>>,
    success_count: Arc<Mutex<u32>>,
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, success_threshold: u32, timeout: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed)),
            failure_count: Arc::new(Mutex::new(0)),
            success_count: Arc::new(Mutex::new(0)),
            failure_threshold,
            success_threshold,
            timeout,
        }
    }

    /// Check if request is allowed
    pub async fn is_allowed(&self) -> bool {
        let mut state = self.state.lock().await;

        match &*state {
            CircuitState::Closed => true,
            CircuitState::Open { until } => {
                if Instant::now() >= *until {
                    debug!("Circuit breaker transitioning to half-open");
                    *state = CircuitState::HalfOpen;
                    *self.success_count.lock().await = 0;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful request
    pub async fn record_success(&self) {
        let mut state = self.state.lock().await;

        match &*state {
            CircuitState::HalfOpen => {
                let mut success_count = self.success_count.lock().await;
                *success_count += 1;

                if *success_count >= self.success_threshold {
                    debug!("Circuit breaker closing after {} successes", *success_count);
                    *state = CircuitState::Closed;
                    *self.failure_count.lock().await = 0;
                }
            }
            CircuitState::Closed => {
                *self.failure_count.lock().await = 0;
            }
            _ => {}
        }
    }

    /// Record a failed request
    pub async fn record_failure(&self) {
        let mut state = self.state.lock().await;

        match &*state {
            CircuitState::Closed => {
                let mut failure_count = self.failure_count.lock().await;
                *failure_count += 1;

                if *failure_count >= self.failure_threshold {
                    warn!("Circuit breaker opening after {} failures", *failure_count);
                    *state = CircuitState::Open {
                        until: Instant::now() + self.timeout,
                    };
                }
            }
            CircuitState::HalfOpen => {
                warn!("Circuit breaker reopening after failure in half-open state");
                *state = CircuitState::Open {
                    until: Instant::now() + self.timeout,
                };
                *self.success_count.lock().await = 0;
            }
            _ => {}
        }
    }
}

/// Adaptive rate limiter that adjusts based on response headers
#[derive(Debug)]
pub struct AdaptiveRateLimiter {
    current_rate: Arc<Mutex<f64>>,
    min_rate: f64,
    max_rate: f64,
    remaining_requests: Arc<Mutex<Option<u32>>>,
    remaining_tokens: Arc<Mutex<Option<u32>>>,
    reset_time: Arc<Mutex<Option<Instant>>>,
}

impl AdaptiveRateLimiter {
    pub fn new(initial_rate: f64, min_rate: f64, max_rate: f64) -> Self {
        Self {
            current_rate: Arc::new(Mutex::new(initial_rate)),
            min_rate,
            max_rate,
            remaining_requests: Arc::new(Mutex::new(None)),
            remaining_tokens: Arc::new(Mutex::new(None)),
            reset_time: Arc::new(Mutex::new(None)),
        }
    }

    /// Update rate limits from response headers
    pub async fn update_from_headers(
        &self,
        remaining_requests: Option<u32>,
        remaining_tokens: Option<u32>,
        reset_after_seconds: Option<u64>,
    ) {
        if let Some(requests) = remaining_requests {
            *self.remaining_requests.lock().await = Some(requests);
        }

        if let Some(tokens) = remaining_tokens {
            *self.remaining_tokens.lock().await = Some(tokens);
        }

        if let Some(reset_seconds) = reset_after_seconds {
            *self.reset_time.lock().await =
                Some(Instant::now() + Duration::from_secs(reset_seconds));
        }

        // Adjust rate based on remaining capacity
        if let (Some(remaining), Some(reset)) = (remaining_requests, reset_after_seconds) {
            if reset > 0 {
                let suggested_rate = (remaining as f64) / (reset as f64);
                self.adjust_rate(suggested_rate).await;
            }
        }
    }

    /// Adjust the current rate within bounds
    async fn adjust_rate(&self, suggested_rate: f64) {
        let mut current_rate = self.current_rate.lock().await;
        let new_rate = suggested_rate.clamp(self.min_rate, self.max_rate);

        if (new_rate - *current_rate).abs() > 0.1 {
            debug!(
                "Adjusting rate from {} to {} requests/sec",
                *current_rate, new_rate
            );
            *current_rate = new_rate;
        }
    }

    /// Get current rate limit
    pub async fn get_rate(&self) -> f64 {
        *self.current_rate.lock().await
    }

    /// Check if we should throttle based on remaining capacity
    #[allow(dead_code)]
    pub async fn should_throttle(&self) -> bool {
        let remaining_requests = *self.remaining_requests.lock().await;
        let remaining_tokens = *self.remaining_tokens.lock().await;

        // Throttle if we're below 20% capacity
        if let Some(requests) = remaining_requests {
            if requests < 10 {
                return true;
            }
        }

        if let Some(tokens) = remaining_tokens {
            if tokens < 1000 {
                return true;
            }
        }

        false
    }
}

/// Request queue with priority support
#[allow(dead_code)]
pub struct PriorityRequestQueue<T> {
    high_priority: Arc<Mutex<VecDeque<T>>>,
    normal_priority: Arc<Mutex<VecDeque<T>>>,
    semaphore: Arc<Semaphore>,
    max_concurrent: usize,
}

#[allow(dead_code)]
impl<T> PriorityRequestQueue<T> {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            high_priority: Arc::new(Mutex::new(VecDeque::new())),
            normal_priority: Arc::new(Mutex::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent,
        }
    }

    /// Add request to queue with priority
    pub async fn enqueue(&self, request: T, high_priority: bool) {
        if high_priority {
            self.high_priority.lock().await.push_back(request);
        } else {
            self.normal_priority.lock().await.push_back(request);
        }
    }

    /// Get next request from queue
    pub async fn dequeue(&self) -> Option<T> {
        // Try high priority first
        if let Some(request) = self.high_priority.lock().await.pop_front() {
            return Some(request);
        }

        // Then normal priority
        self.normal_priority.lock().await.pop_front()
    }

    /// Execute request with concurrency control
    pub async fn execute_with_permit<F, R>(&self, f: F) -> R
    where
        F: std::future::Future<Output = R>,
    {
        let _permit = self.semaphore.acquire().await.unwrap();
        f.await
    }

    /// Get queue sizes for monitoring
    pub async fn queue_sizes(&self) -> (usize, usize) {
        let high = self.high_priority.lock().await.len();
        let normal = self.normal_priority.lock().await.len();
        (high, normal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_bucket() {
        let bucket = TokenBucket::new(10.0, 2.0); // 10 tokens, 2 per second refill

        // Should be able to acquire 5 tokens immediately
        assert!(bucket.acquire(5.0).await.is_ok());

        // Should have 5 tokens left
        assert_eq!(bucket.available_tokens().await as i32, 5);

        // Acquiring 10 more should require waiting
        let start = Instant::now();
        assert!(bucket.acquire(10.0).await.is_ok());
        let elapsed = start.elapsed();

        // Should have waited approximately 2.5 seconds (5 tokens / 2 per second)
        assert!(elapsed >= Duration::from_secs(2));
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let breaker = CircuitBreaker::new(3, 2, Duration::from_secs(1));

        // Initially closed
        assert!(breaker.is_allowed().await);

        // Record 3 failures to open the circuit
        for _ in 0..3 {
            breaker.record_failure().await;
        }

        // Should be open now
        assert!(!breaker.is_allowed().await);

        // Wait for timeout
        sleep(Duration::from_secs(1)).await;

        // Should be half-open
        assert!(breaker.is_allowed().await);

        // Record 2 successes to close
        for _ in 0..2 {
            breaker.record_success().await;
        }

        // Should be closed again
        assert!(breaker.is_allowed().await);
    }

    #[tokio::test]
    async fn test_adaptive_rate_limiter() {
        let limiter = AdaptiveRateLimiter::new(10.0, 1.0, 100.0);

        // Initial rate
        assert_eq!(limiter.get_rate().await, 10.0);

        // Update from headers suggesting lower rate
        limiter.update_from_headers(Some(20), None, Some(10)).await;

        // Rate should adjust to ~2 requests/sec
        let new_rate = limiter.get_rate().await;
        assert!(new_rate < 3.0 && new_rate >= 1.0);

        // Should throttle with low remaining
        limiter.update_from_headers(Some(5), None, None).await;
        assert!(limiter.should_throttle().await);
    }

    #[tokio::test]
    async fn test_priority_queue() {
        let queue = PriorityRequestQueue::new(2);

        // Add requests
        queue.enqueue("normal1", false).await;
        queue.enqueue("high1", true).await;
        queue.enqueue("normal2", false).await;
        queue.enqueue("high2", true).await;

        // Should dequeue high priority first
        assert_eq!(queue.dequeue().await, Some("high1"));
        assert_eq!(queue.dequeue().await, Some("high2"));
        assert_eq!(queue.dequeue().await, Some("normal1"));
        assert_eq!(queue.dequeue().await, Some("normal2"));
        assert_eq!(queue.dequeue().await, None);
    }
}
