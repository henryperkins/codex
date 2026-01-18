use crate::azure::is_azure_base_url;
use codex_client::Request;
use codex_client::RequestCompression;
use codex_client::RetryOn;
use codex_client::RetryPolicy;
use http::Method;
use http::header::HeaderMap;
use std::collections::HashMap;
use std::time::Duration;

/// Wire-level APIs supported by a `Provider`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireApi {
    Responses,
    Chat,
    Compact,
}

/// High-level retry configuration for a provider.
///
/// This is converted into a `RetryPolicy` used by `codex-client` to drive
/// transport-level retries for both unary and streaming calls.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u64,
    pub base_delay: Duration,
    pub retry_429: bool,
    pub retry_5xx: bool,
    pub retry_transport: bool,
    /// Maximum delay to honor from server retry-after headers.
    /// If the server requests a longer delay, fall back to exponential backoff.
    pub max_retry_delay: Option<Duration>,
}

impl RetryConfig {
    pub fn to_policy(&self) -> RetryPolicy {
        RetryPolicy {
            max_attempts: self.max_attempts,
            base_delay: self.base_delay,
            retry_on: RetryOn {
                retry_429: self.retry_429,
                retry_5xx: self.retry_5xx,
                retry_transport: self.retry_transport,
            },
            max_retry_delay: self.max_retry_delay,
        }
    }
}

/// HTTP endpoint configuration used to talk to a concrete API deployment.
///
/// Encapsulates base URL, default headers, query params, retry policy, and
/// stream idle timeout, plus helper methods for building requests.
#[derive(Debug, Clone)]
pub struct Provider {
    pub name: String,
    pub base_url: String,
    pub query_params: Option<HashMap<String, String>>,
    pub wire: WireApi,
    pub headers: HeaderMap,
    pub retry: RetryConfig,
    pub stream_idle_timeout: Duration,
}

impl Provider {
    pub fn url_for_path(&self, path: &str) -> String {
        let path = path.trim_start_matches('/');

        // Split base URL into path and existing query string, if any
        let (base_path, existing_query) = if let Some(idx) = self.base_url.find('?') {
            let (p, q) = self.base_url.split_at(idx);
            (p.trim_end_matches('/'), Some(&q[1..])) // q[1..] to skip the '?'
        } else {
            (self.base_url.trim_end_matches('/'), None)
        };

        // Build the path portion
        let mut url = if path.is_empty() {
            base_path.to_string()
        } else {
            format!("{base_path}/{path}")
        };

        // Collect all query params: existing from base URL + explicit query_params
        let mut all_params: Vec<String> = Vec::new();

        // Include existing query string from base URL (already encoded)
        if let Some(qs) = existing_query
            && !qs.is_empty()
        {
            all_params.push(qs.to_string());
        }

        // Add explicit query params (with encoding)
        if let Some(params) = &self.query_params
            && !params.is_empty()
        {
            let qs = params
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");
            all_params.push(qs);
        }

        // Append combined query string
        if !all_params.is_empty() {
            url.push('?');
            url.push_str(&all_params.join("&"));
        }

        url
    }

    pub fn build_request(&self, method: Method, path: &str) -> Request {
        Request {
            method,
            url: self.url_for_path(path),
            headers: self.headers.clone(),
            body: None,
            compression: RequestCompression::None,
            timeout: None,
        }
    }

    pub fn is_azure_responses_endpoint(&self) -> bool {
        if self.wire != WireApi::Responses {
            return false;
        }

        if self.name.eq_ignore_ascii_case("azure") {
            return true;
        }

        is_azure_base_url(&self.base_url)
    }
}
