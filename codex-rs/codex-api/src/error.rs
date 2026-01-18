use crate::rate_limits::RateLimitError;
use codex_client::TransportError;
use http::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Transport(TransportError),
    #[error("api error {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("stream error: {0}")]
    Stream(String),
    #[error("context window exceeded")]
    ContextWindowExceeded,
    #[error("quota exceeded")]
    QuotaExceeded,
    #[error("usage not included")]
    UsageNotIncluded,
    #[error("previous response chain broken: {message}")]
    PreviousResponseChainBroken { message: String },
    #[error("retryable error: {message}")]
    Retryable {
        message: String,
        delay: Option<Duration>,
    },
    #[error("rate limit: {0}")]
    RateLimit(String),
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },
}

impl From<RateLimitError> for ApiError {
    fn from(err: RateLimitError) -> Self {
        Self::RateLimit(err.to_string())
    }
}

impl ApiError {
    pub fn from_bad_request_body(body: &str) -> Option<Self> {
        #[derive(Deserialize)]
        struct ErrorEnvelope {
            error: Option<ErrorDetail>,
        }

        #[derive(Deserialize)]
        struct ErrorDetail {
            r#type: Option<String>,
            param: Option<String>,
            message: Option<String>,
        }

        let envelope: ErrorEnvelope = serde_json::from_str(body).ok()?;
        let error = envelope.error?;
        if error.r#type.as_deref() != Some("invalid_request_error") {
            return None;
        }

        let msg = error.message.as_deref().unwrap_or("");
        let msg_lower = msg.to_ascii_lowercase();
        let param = error.param.as_deref().unwrap_or("");

        if param == "previous_response_id"
            || (msg_lower.contains("previous") && msg_lower.contains("not found"))
            || (param.starts_with("input") && msg_lower.contains("not found"))
            || (param == "input" && msg_lower.contains("duplicate item"))
            // Catch "No tool output found for custom tool call" errors which indicate
            // the server is expecting tool outputs from a previous response in the chain.
            || (param == "input" && msg_lower.contains("no tool output found"))
            // Catch Azure Responses API "Function call output is missing for call id" errors
            // which occur when previous_response_id references a response with pending tool
            // calls but the continuation request doesn't include the required tool outputs.
            || (param == "input" && msg_lower.contains("output is missing"))
        {
            return Some(ApiError::PreviousResponseChainBroken {
                message: error.message.unwrap_or_default(),
            });
        }

        None
    }
}

impl From<TransportError> for ApiError {
    fn from(err: TransportError) -> Self {
        match err {
            TransportError::Http {
                status,
                url,
                headers,
                body,
            } => {
                if status == StatusCode::BAD_REQUEST
                    && let Some(body_str) = body.as_deref()
                    && let Some(chain_error) = ApiError::from_bad_request_body(body_str)
                {
                    return chain_error;
                }
                ApiError::Transport(TransportError::Http {
                    status,
                    url,
                    headers,
                    body,
                })
            }
            other => ApiError::Transport(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn detects_previous_response_chain_error_from_param() {
        let body = r#"{"error":{"type":"invalid_request_error","param":"previous_response_id","message":"Item with id resp-1 not found."}}"#;
        let result = ApiError::from_bad_request_body(body).expect("chain error");
        let ApiError::PreviousResponseChainBroken { message } = result else {
            panic!("expected PreviousResponseChainBroken");
        };
        assert_eq!(message, "Item with id resp-1 not found.");
    }

    #[test]
    fn ignores_non_chain_bad_request_errors() {
        let body = r#"{"error":{"type":"invalid_request_error","param":"max_output_tokens","message":"Invalid value."}}"#;
        assert!(ApiError::from_bad_request_body(body).is_none());
    }

    #[test]
    fn detects_duplicate_item_error() {
        let body = r#"{"error":{"message":"Duplicate item found with id rs_0c7d29236204b9c4006969d4c31de48190927f45b58f0b179a. Remove duplicate items from your input and try again.","type":"invalid_request_error","param":"input","code":null}}"#;
        let result = ApiError::from_bad_request_body(body).expect("chain error");
        let ApiError::PreviousResponseChainBroken { message } = result else {
            panic!("expected PreviousResponseChainBroken");
        };
        assert!(message.contains("Duplicate item found"));
    }

    #[test]
    fn detects_missing_tool_output_error() {
        let body = r#"{"error":{"message":"No tool output found for custom tool call call_fkAS1VFErJlgn1HWNhgd4VPH.","type":"invalid_request_error","param":"input","code":null}}"#;
        let result = ApiError::from_bad_request_body(body).expect("chain error");
        let ApiError::PreviousResponseChainBroken { message } = result else {
            panic!("expected PreviousResponseChainBroken");
        };
        assert!(message.contains("No tool output found"));
    }

    #[test]
    fn detects_function_call_output_missing_error() {
        // Azure Responses API returns this format when previous_response_id references
        // a response with pending tool calls but no tool outputs are provided.
        let body = r#"{"error":{"message":"Function call output is missing for call id call-ABC123.","type":"invalid_request_error","param":"input","code":null}}"#;
        let result = ApiError::from_bad_request_body(body).expect("chain error");
        let ApiError::PreviousResponseChainBroken { message } = result else {
            panic!("expected PreviousResponseChainBroken");
        };
        assert!(message.contains("output is missing"));
    }
}
