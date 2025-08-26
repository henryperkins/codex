# Azure OpenAI Rate Limiting Strategies

## Current Implementation Analysis

Based on my investigation of the codebase, here's what I found:

### Existing Rate Limit Handling

The codebase already has some rate limit handling in place:

1. **Basic Retry Logic** (in `/core/src/client.rs`):
   - Detects HTTP 429 (Too Many Requests) status codes
   - Parses `Retry-After` headers from responses
   - Extracts retry delays from error messages (e.g., "Please try again in 11.054s")
   - Uses exponential backoff with jitter when no specific retry time is provided

2. **Backoff Implementation** (in `/core/src/util.rs`):
   - Initial delay: 200ms
   - Backoff factor: 2.0
   - Includes random jitter (0.9-1.1x) to prevent thundering herd

3. **Error Handling**:
   - Properly parses rate limit errors with code `rate_limit_exceeded`
   - Extracts retry duration from error messages using regex

## Recommended Improvements for Azure OpenAI

### 1. Token Bucket Rate Limiting
Implement a token bucket algorithm to proactively prevent hitting rate limits:

```rust
struct TokenBucket {
    capacity: f64,
    tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    async fn acquire(&mut self, tokens_needed: f64) {
        self.refill();
        while self.tokens < tokens_needed {
            let wait_time = (tokens_needed - self.tokens) / self.refill_rate;
            sleep(Duration::from_secs_f64(wait_time)).await;
            self.refill();
        }
        self.tokens -= tokens_needed;
    }
    
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }
}
```

### 2. Request Queue with Priority
Implement a priority queue for requests to manage concurrent API calls:

```rust
struct RequestQueue {
    high_priority: VecDeque<Request>,
    normal_priority: VecDeque<Request>,
    max_concurrent: usize,
    active_requests: usize,
}
```

### 3. Circuit Breaker Pattern
Add circuit breaker to prevent cascading failures:

```rust
enum CircuitState {
    Closed,
    Open { until: Instant },
    HalfOpen,
}

struct CircuitBreaker {
    state: CircuitState,
    failure_threshold: u32,
    failure_count: u32,
    success_threshold: u32,
    success_count: u32,
    timeout: Duration,
}
```

### 4. Adaptive Rate Limiting
Monitor actual rate limit headers and adjust dynamically:

```rust
struct AdaptiveRateLimiter {
    // Track remaining tokens from headers
    remaining_requests: Option<u32>,
    remaining_tokens: Option<u32>,
    reset_time: Option<Instant>,
    
    // Adjust request rate based on feedback
    current_rate: f64,
    min_rate: f64,
    max_rate: f64,
}
```

### 5. Request Batching
Batch multiple small requests when possible:

```rust
struct RequestBatcher {
    pending: Vec<Request>,
    batch_size: usize,
    batch_timeout: Duration,
    last_batch: Instant,
}
```

## Azure-Specific Considerations

### Rate Limit Types
Azure OpenAI has multiple rate limit types:
- **RPM (Requests Per Minute)**: Total number of API calls
- **TPM (Tokens Per Minute)**: Total tokens processed
- **RPMD (Requests Per Minute per Deployment)**: Per-deployment limits

### Headers to Monitor
```rust
const RATE_LIMIT_HEADERS: &[&str] = &[
    "x-ratelimit-limit-requests",
    "x-ratelimit-limit-tokens", 
    "x-ratelimit-remaining-requests",
    "x-ratelimit-remaining-tokens",
    "x-ratelimit-reset-requests",
    "x-ratelimit-reset-tokens",
    "retry-after",
    "retry-after-ms",
];
```

### Best Practices

1. **Implement Tiered Retry Strategy**:
   - First retry: Use exact retry-after value if provided
   - Subsequent retries: Exponential backoff with jitter
   - Max retries: Configure based on use case (e.g., 5 for interactive, 10 for batch)

2. **Monitor and Log**:
   - Track rate limit hits per endpoint/deployment
   - Log retry attempts and success rates
   - Alert on repeated failures

3. **Deployment Strategy**:
   - Use multiple deployments for load distribution
   - Implement deployment rotation/fallback
   - Consider region-based failover

4. **Token Estimation**:
   - Pre-calculate approximate token usage
   - Reserve capacity for response tokens
   - Implement request splitting for large prompts

5. **Graceful Degradation**:
   - Fallback to smaller models when rate limited
   - Queue non-urgent requests
   - Provide user feedback on delays

## Implementation Priority

1. **High Priority** (Immediate):
   - Enhance retry logic with proper header parsing
   - Add request queuing with basic throttling
   - Improve error messages and logging

2. **Medium Priority** (Next Sprint):
   - Implement token bucket rate limiter
   - Add circuit breaker pattern
   - Create metrics/monitoring dashboard

3. **Low Priority** (Future):
   - Request batching optimization
   - Advanced deployment rotation
   - ML-based rate prediction

## Configuration Example

```toml
[azure_openai.rate_limiting]
# Token bucket settings
tokens_per_minute = 30000
requests_per_minute = 300
burst_capacity = 5000

# Retry settings
max_retries = 5
initial_retry_delay_ms = 200
max_retry_delay_ms = 60000
backoff_factor = 2.0

# Circuit breaker
failure_threshold = 5
success_threshold = 2
circuit_timeout_seconds = 30

# Queue settings
max_queue_size = 100
queue_timeout_seconds = 300
max_concurrent_requests = 10
```

## Testing Strategy

1. **Unit Tests**:
   - Test retry logic with various error responses
   - Verify backoff calculations
   - Test token bucket refill rates

2. **Integration Tests**:
   - Simulate rate limit scenarios
   - Test circuit breaker state transitions
   - Verify queue prioritization

3. **Load Tests**:
   - Stress test with concurrent requests
   - Measure throughput near rate limits
   - Test recovery after rate limit hits

## Monitoring Metrics

- `azure_openai_rate_limit_hits_total` - Counter of rate limit errors
- `azure_openai_retry_attempts_total` - Counter of retry attempts
- `azure_openai_request_queue_size` - Gauge of queued requests
- `azure_openai_circuit_breaker_state` - Current circuit breaker state
- `azure_openai_tokens_remaining` - Gauge of remaining tokens
- `azure_openai_request_latency_seconds` - Histogram of request latencies