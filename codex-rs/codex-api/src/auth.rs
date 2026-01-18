use codex_client::Request;

use crate::is_azure_base_url;

/// Provides bearer and account identity information for API requests.
///
/// Implementations should be cheap and non-blocking; any asynchronous
/// refresh or I/O should be handled by higher layers before requests
/// reach this interface.
pub trait AuthProvider: Send + Sync {
    fn bearer_token(&self) -> Option<String>;
    fn account_id(&self) -> Option<String> {
        None
    }
}

/// Adds authentication headers to a request.
///
/// For Azure endpoints, the token is sent as `api-key` header.
/// For other endpoints, it's sent as `Authorization: Bearer <token>`.
pub(crate) fn add_auth_headers<A: AuthProvider>(
    auth: &A,
    mut req: Request,
    is_azure: bool,
) -> Request {
    if let Some(token) = auth.bearer_token() {
        if is_azure {
            // Azure OpenAI uses api-key header
            if let Ok(header) = HeaderValue::from_str(&token) {
                let _ = req.headers.insert("api-key", header);
            }
        } else {
            // Standard OpenAI uses Authorization: Bearer
            if let Ok(header) = format!("Bearer {token}").parse() {
                let _ = req.headers.insert(http::header::AUTHORIZATION, header);
            }
        }
    }
    if let Some(account_id) = auth.account_id()
        && let Ok(header) = account_id.parse()
    {
        let _ = req.headers.insert("ChatGPT-Account-ID", header);
    }
    req
}

/// Determines if the given base URL is an Azure endpoint.
pub(crate) fn is_azure_endpoint(provider_name: &str, base_url: &str) -> bool {
    provider_name.eq_ignore_ascii_case("azure") || is_azure_base_url(base_url)
}
