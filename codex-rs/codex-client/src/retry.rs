use crate::error::TransportError;
use crate::request::Request;
use http::HeaderMap;
use rand::Rng;
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for retry behavior on failed HTTP requests.
///
/// Controls how many times a request should be retried and the delay between attempts.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts before giving up.
    pub max_attempts: u64,
    /// Base delay for exponential backoff calculation.
    pub base_delay: Duration,
    /// Conditions under which retries should be attempted.
    pub retry_on: RetryOn,
    /// Maximum delay to wait when server sends retry-after header.
    /// If server requests longer delay, fall back to exponential backoff.
    pub max_retry_delay: Option<Duration>,
}

/// Specifies which error conditions should trigger a retry attempt.
#[derive(Debug, Clone)]
pub struct RetryOn {
    /// Retry on HTTP 429 Too Many Requests (rate limiting).
    pub retry_429: bool,
    /// Retry on HTTP 5xx server errors.
    pub retry_5xx: bool,
    /// Retry on transport-level errors (timeouts, network failures).
    pub retry_transport: bool,
}

impl RetryOn {
    /// Determines whether a request should be retried based on the error type.
    ///
    /// Returns `false` if `attempt >= max_attempts` or if the error type
    /// is not configured for retry.
    pub fn should_retry(&self, err: &TransportError, attempt: u64, max_attempts: u64) -> bool {
        if attempt >= max_attempts {
            return false;
        }
        match err {
            TransportError::Http { status, .. } => {
                (self.retry_429 && status.as_u16() == 429)
                    || (self.retry_5xx && status.is_server_error())
            }
            TransportError::Timeout | TransportError::Network(_) => self.retry_transport,
            _ => false,
        }
    }
}

/// Calculates exponential backoff delay with jitter.
///
/// For `attempt == 0`, returns the base delay. For subsequent attempts,
/// doubles the delay each time with ±10% jitter to prevent thundering herd.
///
/// # Arguments
/// * `base` - Base delay duration
/// * `attempt` - Current attempt number (0-indexed)
pub fn backoff(base: Duration, attempt: u64) -> Duration {
    if attempt == 0 {
        return base;
    }
    let exp = 2u64.saturating_pow(attempt as u32 - 1);
    let millis = base.as_millis() as u64;
    let raw = millis.saturating_mul(exp);
    let jitter: f64 = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((raw as f64 * jitter) as u64)
}

/// Parse retry-after delay from HTTP response headers.
///
/// Follows Azure SDK precedence order:
/// 1. `retry-after-ms` - milliseconds (Azure-specific, highest precision)
/// 2. `x-ms-retry-after-ms` - milliseconds (Azure-specific alternative)
/// 3. `retry-after` - seconds (standard HTTP header)
///
/// Returns `None` if no valid retry-after header is found.
pub fn parse_retry_after_headers(headers: &HeaderMap) -> Option<Duration> {
    // Try retry-after-ms first (milliseconds)
    if let Some(value) = headers.get("retry-after-ms")
        && let Ok(s) = value.to_str()
        && let Ok(ms) = s.parse::<u64>()
    {
        return Some(Duration::from_millis(ms));
    }

    // Try x-ms-retry-after-ms (Azure alternative, milliseconds)
    if let Some(value) = headers.get("x-ms-retry-after-ms")
        && let Ok(s) = value.to_str()
        && let Ok(ms) = s.parse::<u64>()
    {
        return Some(Duration::from_millis(ms));
    }

    // Try standard retry-after header (seconds)
    if let Some(value) = headers.get("retry-after")
        && let Ok(value_str) = value.to_str()
    {
        // Try parsing as integer seconds first
        if let Ok(secs) = value_str.parse::<u64>() {
            return Some(Duration::from_secs(secs));
        }
        // Try parsing as float seconds (some servers send "1.5")
        // Guard against negative, NaN, or infinite values which panic Duration::from_secs_f64
        if let Ok(secs) = value_str.parse::<f64>()
            && secs.is_finite()
            && secs >= 0.0
        {
            return Some(Duration::from_secs_f64(secs));
        }
    }

    None
}

/// Compute the delay for a retry attempt, preferring server-provided retry-after headers.
///
/// If `headers` contains a valid retry-after value within `max_retry_delay`, use it.
/// Otherwise, fall back to exponential backoff.
fn compute_retry_delay(
    headers: Option<&HeaderMap>,
    base_delay: Duration,
    attempt: u64,
    max_retry_delay: Option<Duration>,
) -> Duration {
    if let Some(hdrs) = headers
        && let Some(server_delay) = parse_retry_after_headers(hdrs)
    {
        // Check if server delay is within acceptable bounds
        if let Some(max_delay) = max_retry_delay {
            if server_delay <= max_delay {
                return server_delay;
            }
            // Server requested too long, fall back to backoff
        } else {
            // No max configured, trust the server
            return server_delay;
        }
    }
    // Fall back to exponential backoff
    backoff(base_delay, attempt + 1)
}

/// Executes an HTTP operation with automatic retries according to the given policy.
///
/// Respects server-provided `retry-after` headers when present, falling back to
/// exponential backoff when headers are absent or exceed the configured maximum.
///
/// # Arguments
/// * `policy` - Retry configuration including max attempts and delay settings
/// * `make_req` - Factory function that creates a fresh request for each attempt
/// * `op` - Async operation that executes the request
///
/// # Returns
/// The successful response, or `TransportError::RetryLimit` if all attempts fail.
pub async fn run_with_retry<T, F, Fut>(
    policy: RetryPolicy,
    mut make_req: impl FnMut() -> Request,
    op: F,
) -> Result<T, TransportError>
where
    F: Fn(Request, u64) -> Fut,
    Fut: Future<Output = Result<T, TransportError>>,
{
    for attempt in 0..=policy.max_attempts {
        let req = make_req();
        match op(req, attempt).await {
            Ok(resp) => return Ok(resp),
            Err(ref err)
                if policy
                    .retry_on
                    .should_retry(err, attempt, policy.max_attempts) =>
            {
                // Extract headers from HTTP errors to check for retry-after
                let headers = match &err {
                    TransportError::Http { headers, .. } => headers.as_ref(),
                    _ => None,
                };
                let delay = compute_retry_delay(
                    headers,
                    policy.base_delay,
                    attempt,
                    policy.max_retry_delay,
                );
                sleep(delay).await;
            }
            Err(err) => return Err(err),
        }
    }
    Err(TransportError::RetryLimit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_retry_after_ms_header() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after-ms", HeaderValue::from_static("1500"));

        let delay = parse_retry_after_headers(&headers);
        assert_eq!(delay, Some(Duration::from_millis(1500)));
    }

    #[test]
    fn parse_x_ms_retry_after_ms_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ms-retry-after-ms", HeaderValue::from_static("2500"));

        let delay = parse_retry_after_headers(&headers);
        assert_eq!(delay, Some(Duration::from_millis(2500)));
    }

    #[test]
    fn parse_retry_after_seconds_header() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("30"));

        let delay = parse_retry_after_headers(&headers);
        assert_eq!(delay, Some(Duration::from_secs(30)));
    }

    #[test]
    fn parse_retry_after_float_seconds_header() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("1.5"));

        let delay = parse_retry_after_headers(&headers);
        assert_eq!(delay, Some(Duration::from_secs_f64(1.5)));
    }

    #[test]
    fn retry_after_ms_takes_precedence() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after-ms", HeaderValue::from_static("100"));
        headers.insert("x-ms-retry-after-ms", HeaderValue::from_static("200"));
        headers.insert("retry-after", HeaderValue::from_static("30"));

        let delay = parse_retry_after_headers(&headers);
        // retry-after-ms should win
        assert_eq!(delay, Some(Duration::from_millis(100)));
    }

    #[test]
    fn x_ms_retry_after_ms_takes_precedence_over_standard() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ms-retry-after-ms", HeaderValue::from_static("200"));
        headers.insert("retry-after", HeaderValue::from_static("30"));

        let delay = parse_retry_after_headers(&headers);
        // x-ms-retry-after-ms should win over standard retry-after
        assert_eq!(delay, Some(Duration::from_millis(200)));
    }

    #[test]
    fn no_retry_after_headers_returns_none() {
        let headers = HeaderMap::new();
        let delay = parse_retry_after_headers(&headers);
        assert_eq!(delay, None);
    }

    #[test]
    fn invalid_retry_after_value_returns_none() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("invalid"));

        let delay = parse_retry_after_headers(&headers);
        assert_eq!(delay, None);
    }

    #[test]
    fn compute_delay_uses_header_when_within_max() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after-ms", HeaderValue::from_static("500"));

        let delay = compute_retry_delay(
            Some(&headers),
            Duration::from_millis(100),
            0,
            Some(Duration::from_secs(60)),
        );
        assert_eq!(delay, Duration::from_millis(500));
    }

    #[test]
    fn compute_delay_falls_back_when_exceeds_max() {
        let mut headers = HeaderMap::new();
        // Server requests 120 seconds, but max is 60 seconds
        headers.insert("retry-after", HeaderValue::from_static("120"));

        let delay = compute_retry_delay(
            Some(&headers),
            Duration::from_millis(100),
            0,
            Some(Duration::from_secs(60)),
        );
        // Should fall back to backoff, not use 120s
        assert!(delay < Duration::from_secs(60));
    }

    #[test]
    fn compute_delay_trusts_server_when_no_max() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("120"));

        let delay = compute_retry_delay(
            Some(&headers),
            Duration::from_millis(100),
            0,
            None, // No max configured
        );
        assert_eq!(delay, Duration::from_secs(120));
    }

    #[test]
    fn compute_delay_uses_backoff_when_no_headers() {
        let delay = compute_retry_delay(
            None,
            Duration::from_millis(100),
            0,
            Some(Duration::from_secs(60)),
        );
        // Should use backoff (base * 2^0 with jitter)
        assert!(delay >= Duration::from_millis(90) && delay <= Duration::from_millis(220));
    }

    #[test]
    fn negative_retry_after_returns_none() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("-5.0"));

        let delay = parse_retry_after_headers(&headers);
        assert_eq!(delay, None);
    }

    #[test]
    fn infinite_retry_after_returns_none() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("inf"));

        let delay = parse_retry_after_headers(&headers);
        assert_eq!(delay, None);
    }

    #[test]
    fn nan_retry_after_returns_none() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("NaN"));

        let delay = parse_retry_after_headers(&headers);
        assert_eq!(delay, None);
    }

    #[test]
    fn non_utf8_high_priority_header_falls_back_to_valid_lower_priority() {
        let mut headers = HeaderMap::new();
        // Insert non-UTF8 bytes in retry-after-ms (highest priority)
        headers.insert(
            "retry-after-ms",
            HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
        );
        // Insert valid retry-after (lower priority)
        headers.insert("retry-after", HeaderValue::from_static("30"));

        let delay = parse_retry_after_headers(&headers);
        // Should fall back to retry-after since retry-after-ms is non-UTF8
        assert_eq!(delay, Some(Duration::from_secs(30)));
    }

    #[test]
    fn invalid_parse_high_priority_falls_back_to_valid_lower_priority() {
        let mut headers = HeaderMap::new();
        // Insert unparseable value in retry-after-ms (highest priority)
        headers.insert("retry-after-ms", HeaderValue::from_static("not-a-number"));
        // Insert valid x-ms-retry-after-ms (second priority)
        headers.insert("x-ms-retry-after-ms", HeaderValue::from_static("500"));

        let delay = parse_retry_after_headers(&headers);
        // Should fall back to x-ms-retry-after-ms
        assert_eq!(delay, Some(Duration::from_millis(500)));
    }
}
