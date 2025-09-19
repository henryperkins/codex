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
use crate::error::CodexErr;
use crate::error::Result;
use crate::model_provider_info::ModelProviderInfo;
use crate::util::backoff;
use reqwest::header::RETRY_AFTER;
use std::time::Duration;

/// Builds a full Azure OpenAI URL for a specific resource path that needs a
/// `{response_id}` segment inserted *before* the query string.
fn build_azure_url(provider: &ModelProviderInfo, auth: &Option<CodexAuth>, suffix: &str) -> String {
    // Example `base` value: https://<resource>.openai.azure.com/openai/v1/responses?api-version=2025-04-01-preview
    let base = provider.get_full_url(auth);

    let mut url = url::Url::parse(&base).expect("provider.get_full_url should return valid URL");

    {
        let mut segments = url
            .path_segments_mut()
            .expect("URL should be able to modify path segments");

        for seg in suffix.trim_start_matches('/').split('/') {
            if !seg.is_empty() {
                segments.push(seg);
            }
        }
    }

    url.to_string()
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

                // Azure-specific error code
                let az_code = res
                    .headers()
                    .get("x-ms-error-code")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();

                let should_retry =
                    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error();

                if should_retry && attempt <= max_retries {
                    let delay = res
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(Duration::from_secs)
                        .unwrap_or_else(|| backoff(attempt));
                    tokio::time::sleep(delay).await;
                    continue;
                }

                let body = res.text().await.unwrap_or_default();
                let msg = if az_code.is_empty() {
                    body
                } else {
                    format!("{body} (azure error code: {az_code})")
                };
                return Err(CodexErr::UnexpectedStatus(status, msg));
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
                let should_retry =
                    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error();

                if should_retry && attempt <= max_retries {
                    let delay = res
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(Duration::from_secs)
                        .unwrap_or_else(|| backoff(attempt));
                    tokio::time::sleep(delay).await;
                    continue;
                }

                let body = res.text().await.unwrap_or_default();
                return Err(CodexErr::UnexpectedStatus(status, body));
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
