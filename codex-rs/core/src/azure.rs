//! Azure-specific helper APIs built on top of the existing Codex `Client`.
//!
//! These functions call the two *read* endpoints that Azure exposes for the
//! Responses API:
//!   • `GET /responses/{id}`
//!   • `GET /responses/{id}/input_items`
//!
//! The request/response types are generated from `docs/v1preview.json` and live
//! in the `codex-openai-schema` crate. We deliberately *do not* use these types
//! when talking to the OpenAI-hosted Responses API to avoid accidental drift.

use std::sync::Arc;

use codex_openai_schema::Response;
use codex_openai_schema::ResponseInputItemsList;

use crate::auth::AuthManager;
use crate::auth::CodexAuth;
use crate::error::{AzureError, CodexErr, Result};
use crate::model_provider_info::ModelProviderInfo;
use crate::util::backoff;
use reqwest::header::{HeaderMap, RETRY_AFTER};

/// Builds a full Azure OpenAI URL for a specific resource path that needs a
/// `{response_id}` segment inserted *before* the query string.
fn build_azure_url(provider: &ModelProviderInfo, auth: &Option<CodexAuth>, suffix: &str) -> String {
    // provider.get_full_url() already ends with "/responses?..."
    let base_with_query = provider.get_full_url(auth);

    // Separate query string so we can insert suffix before it.
    let (base, query) = match base_with_query.split_once('?') {
        Some((b, q)) => (b.to_string(), format!("?{q}")),
        None => (base_with_query.clone(), String::new()),
    };

    let mut url = base;
    if !url.ends_with('/') {
        url.push('/');
    }

    url.push_str(suffix.trim_start_matches('/'));
    url.push_str(&query);
    url
}

/// Returns a delay based on Retry-After headers or falls back to exponential backoff.
fn calc_retry_delay(headers: &HeaderMap, attempt: u64) -> std::time::Duration {
    // Azure may send either `Retry-After` (secs) or `Retry-After-Ms` (ms)
    if let Some(delay_ms) = headers
        .get("retry-after-ms")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        return std::time::Duration::from_millis(delay_ms);
    }

    if let Some(delay_secs) = headers
        .get(RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        return std::time::Duration::from_secs(delay_secs);
    }

    backoff(attempt)
}

/// Helper to parse Azure error body {"error": {"code": ..., "message": ...}}
fn parse_azure_error(body: String, status: reqwest::StatusCode, headers: &HeaderMap) -> AzureError {
    #[derive(serde::Deserialize)]
    struct InnerError {
        code: Option<String>,
        message: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        error: Option<InnerError>,
    }

    let (code, message) = match serde_json::from_str::<ErrorBody>(&body) {
        Ok(err_body) => {
            let code = err_body
                .error
                .as_ref()
                .and_then(|e| e.code.clone())
                .unwrap_or_else(|| "unknown".into());
            let message = err_body
                .error
                .as_ref()
                .and_then(|e| e.message.clone())
                .unwrap_or_else(|| body.clone());
            (code, message)
        }
        Err(_) => ("unknown".into(), body.clone()),
    };

    let request_id = headers
        .get("azure-openai-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    AzureError {
        status,
        code,
        message,
        request_id,
    }
}

/// Fetches the **final** response object for a given response ID.
pub async fn get_response(
    provider: &ModelProviderInfo,
    client: &reqwest::Client,
    auth_manager: &Option<Arc<AuthManager>>,
    response_id: &str,
) -> Result<Response> {
    let auth = auth_manager.as_ref().and_then(|m| m.auth());
    let url = build_azure_url(provider, &auth, response_id);

    let max_retries = provider.request_max_retries();
    let mut attempt = 0;

    loop {
        attempt += 1;

        let mut builder = client.get(url.clone());
        if let Some(auth) = auth.as_ref() {
            builder = builder.bearer_auth(auth.get_token().await?);
        }
        builder = provider.apply_http_headers(builder);

        let user_agent_val = format!("codex-cli/{}", env!("CARGO_PKG_VERSION"));
        builder = builder.header("x-ms-useragent", user_agent_val);

        match builder.send().await {
            Ok(res) => {
                if res.status().is_success() {
                    let headers = res.headers().clone();
                    let mut resp = res.json::<Response>().await.map_err(CodexErr::Reqwest)?;
                    if let Some(hdr_val) = headers.get("azure-openai-usage")
                        && let Ok(raw) = hdr_val.to_str()
                        && let Ok(val) = serde_json::from_str::<serde_json::Value>(raw)
                    {
                        resp.extra.insert("azure_openai_usage_header".into(), val);
                    }
                    return Ok(resp);
                }

                let status = res.status();
                let headers_clone = res.headers().clone();

                let should_retry =
                    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error();

                if should_retry && attempt <= max_retries {
                    let delay = calc_retry_delay(res.headers(), attempt);
                    tokio::time::sleep(delay).await;
                    continue;
                }

                let body = res.text().await.unwrap_or_default();
                let azure_err = parse_azure_error(body, status, &headers_clone);
                return Err(CodexErr::Azure(azure_err));
            }
            Err(e) => {
                if attempt > max_retries {
                    return Err(CodexErr::Reqwest(e));
                }
                let delay = backoff(attempt);
                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Fetches the list of **input items** the user sent for a given response.
pub async fn get_response_input_items(
    provider: &ModelProviderInfo,
    client: &reqwest::Client,
    auth_manager: &Option<Arc<AuthManager>>,
    response_id: &str,
) -> Result<ResponseInputItemsList> {
    let auth = auth_manager.as_ref().and_then(|m| m.auth());
    let suffix = format!("{response_id}/input_items");
    let url = build_azure_url(provider, &auth, &suffix);

    let max_retries = provider.request_max_retries();
    let mut attempt = 0;

    loop {
        attempt += 1;

        let mut builder = client.get(url.clone());
        if let Some(auth) = auth.as_ref() {
            builder = builder.bearer_auth(auth.get_token().await?);
        }
        builder = provider.apply_http_headers(builder);

        let user_agent_val = format!("codex-cli/{}", env!("CARGO_PKG_VERSION"));
        builder = builder.header("x-ms-useragent", user_agent_val);

        match builder.send().await {
            Ok(res) => {
                if res.status().is_success() {
                    let list = res
                        .json::<ResponseInputItemsList>()
                        .await
                        .map_err(CodexErr::Reqwest)?;
                    return Ok(list);
                }

                let status = res.status();
                let headers_clone = res.headers().clone();
                let should_retry =
                    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error();

                if should_retry && attempt <= max_retries {
                    let delay = calc_retry_delay(res.headers(), attempt);
                    tokio::time::sleep(delay).await;
                    continue;
                }

                let body = res.text().await.unwrap_or_default();
                let azure_err = parse_azure_error(body, status, &headers_clone);
                return Err(CodexErr::Azure(azure_err));
            }
            Err(e) => {
                if attempt > max_retries {
                    return Err(CodexErr::Reqwest(e));
                }
                let delay = backoff(attempt);
                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Deletes a stored response by ID.  Azure's API returns HTTP 204 on success.
pub async fn delete_response(
    provider: &ModelProviderInfo,
    client: &reqwest::Client,
    auth_manager: &Option<Arc<AuthManager>>,
    response_id: &str,
) -> Result<()> {
    let auth = auth_manager.as_ref().and_then(|m| m.auth());
    let url = build_azure_url(provider, &auth, response_id);

    let max_retries = provider.request_max_retries();
    let mut attempt = 0;

    loop {
        attempt += 1;

        let mut builder = client.delete(url.clone());
        if let Some(auth) = auth.as_ref() {
            builder = builder.bearer_auth(auth.get_token().await?);
        }
        builder = provider.apply_http_headers(builder);

        let user_agent_val = format!("codex-cli/{}", env!("CARGO_PKG_VERSION"));
        builder = builder.header("x-ms-useragent", user_agent_val);

        match builder.send().await {
            Ok(res) => {
                if res.status().is_success() || res.status() == reqwest::StatusCode::NO_CONTENT {
                    return Ok(());
                }

                let status = res.status();
                let headers_clone = res.headers().clone();
                let should_retry =
                    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error();

                if should_retry && attempt <= max_retries {
                    let delay = calc_retry_delay(&headers_clone, attempt);
                    tokio::time::sleep(delay).await;
                    continue;
                }

                let body = res.text().await.unwrap_or_default();
                let azure_err = parse_azure_error(body, status, &headers_clone);
                return Err(CodexErr::Azure(azure_err));
            }
            Err(e) => {
                if attempt > max_retries {
                    return Err(CodexErr::Reqwest(e));
                }
                let delay = crate::util::backoff(attempt);
                tokio::time::sleep(delay).await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests (offline, no network calls)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    use crate::model_provider_info::WireApi;

    #[test]
    fn build_url_inserts_suffix_before_query() {
        let provider = ModelProviderInfo {
            name: "Azure".into(),
            base_url: Some("https://example.openai.azure.com/openai/v1".into()),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: Some(maplit::hashmap! {
                "api-version".to_string() => "2025-04-01-preview".to_string(),
            }),
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
        };

        let url = build_azure_url(&provider, &None, "abc123");
        assert_eq!(
            url,
            "https://example.openai.azure.com/openai/v1/responses/abc123?api-version=2025-04-01-preview"
        );
    }
}
