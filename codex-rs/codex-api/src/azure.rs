//! Azure OpenAI-specific utilities.
//!
//! Centralizes Azure endpoint detection and request handling to ensure
//! consistent behavior across SSE and WebSocket paths.

use codex_protocol::models::ResponseItem;
use serde_json::Value;

/// Domain suffixes that identify legitimate Azure OpenAI endpoints.
///
/// These patterns are checked (case-insensitively) against the hostname
/// to auto-detect Azure deployments. Using host-based matching prevents
/// false positives from non-Azure proxies running on Azure infrastructure.
const AZURE_DOMAIN_SUFFIXES: &[&str] = &[
    ".openai.azure.com",
    ".openai.azure.us",
    ".openai.azure.cn",
    ".cognitiveservices.azure.com",
    ".cognitiveservices.azure.us",
    ".cognitiveservices.azure.cn",
    ".aoai.azure.com",
];

/// Returns true if the given base URL appears to be an Azure OpenAI endpoint.
///
/// Uses host-based matching to avoid misclassifying non-Azure proxies that
/// run on Azure Front Door, APIM, or CDN infrastructure.
pub fn is_azure_base_url(base_url: &str) -> bool {
    let Ok(url) = url::Url::parse(base_url) else {
        // Fallback for unparseable URLs: check for azure markers in the string
        let base_lower = base_url.to_ascii_lowercase();
        return base_lower.contains("openai.azure.") || base_lower.contains("cognitiveservices.azure.");
    };

    let Some(host) = url.host_str() else {
        return false;
    };

    let host_lower = host.to_ascii_lowercase();
    AZURE_DOMAIN_SUFFIXES
        .iter()
        .any(|suffix| host_lower.ends_with(suffix))
}

/// Attaches item IDs to a JSON request payload for Azure Responses API.
///
/// Azure requires item IDs to be present in the request for response chaining
/// to work correctly. The `ResponseItem` struct has `skip_serializing` on ID
/// fields, so this function patches the serialized JSON to include them.
///
/// # Panics
/// Panics if the length of the serialized items array doesn't match the
/// original_items length, which would indicate a filtering or reordering bug.
pub fn attach_item_ids_to_json(payload_json: &mut Value, original_items: &[ResponseItem]) {
    let Some(input_value) = payload_json.get_mut("input") else {
        return;
    };
    let Value::Array(items) = input_value else {
        return;
    };

    // SAFETY: This function assumes 1:1 ordering between serialized input and
    // original_items. If future changes introduce filtering or reordering,
    // this check will catch the mismatch early.
    if items.len() != original_items.len() {
        panic!(
            "attach_item_ids_to_json: length mismatch - serialized {} items but have {} original items. \
             This indicates a filtering/reordering bug that will break Azure chaining.",
            items.len(),
            original_items.len()
        );
    }

    for (value, item) in items.iter_mut().zip(original_items.iter()) {
        if let Some(id) = extract_item_id(item)
            && !id.is_empty()
            && let Some(obj) = value.as_object_mut()
        {
            obj.insert("id".to_string(), Value::String(id.to_string()));
        }
    }
}

/// Extracts the ID from a ResponseItem if present.
fn extract_item_id(item: &ResponseItem) -> Option<&str> {
    match item {
        ResponseItem::Reasoning { id, .. } => Some(id.as_str()),
        ResponseItem::Message { id: Some(id), .. } => Some(id.as_str()),
        ResponseItem::WebSearchCall { id: Some(id), .. } => Some(id.as_str()),
        ResponseItem::FunctionCall { id: Some(id), .. } => Some(id.as_str()),
        ResponseItem::LocalShellCall { id: Some(id), .. } => Some(id.as_str()),
        ResponseItem::CustomToolCall { id: Some(id), .. } => Some(id.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn detects_azure_base_urls() {
        let positive_cases = [
            "https://foo.openai.azure.com/openai",
            "https://foo.openai.azure.us/openai/deployments/bar",
            "https://foo.cognitiveservices.azure.com/openai",
            "https://foo.cognitiveservices.azure.cn/openai",
            "https://foo.aoai.azure.com/openai",
        ];
        for url in positive_cases {
            assert!(is_azure_base_url(url), "expected {url} to be Azure");
        }

        let negative_cases = [
            "https://api.openai.com/v1",
            "https://example.com/openai",
            "https://myproxy.azurewebsites.net/openai",
            // Azure Front Door/CDN/APIM are not automatically detected as Azure
            // to avoid misclassifying non-Azure proxies on Azure infrastructure
            "https://foo.openai.azure-api.net/openai",
            "https://foo.z01.azurefd.net/",
            "https://myaccount.blob.core.windows.net/openai/something",
        ];
        for url in negative_cases {
            assert!(!is_azure_base_url(url), "expected {url} not to be Azure");
        }
    }

    #[test]
    fn attach_item_ids_patches_json() {
        use codex_protocol::models::ContentItem;

        let items = vec![
            ResponseItem::Message {
                id: Some("msg-1".into()),
                role: "assistant".into(),
                content: vec![ContentItem::OutputText {
                    text: "hello".into(),
                }],
            },
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "world".into(),
                }],
            },
        ];

        let mut payload = serde_json::json!({
            "model": "gpt-4",
            "input": [
                {"type": "message", "role": "assistant", "content": []},
                {"type": "message", "role": "user", "content": []}
            ]
        });

        attach_item_ids_to_json(&mut payload, &items);

        let input = payload.get("input").unwrap().as_array().unwrap();
        assert_eq!(input[0].get("id"), Some(&Value::String("msg-1".into())));
        assert_eq!(input[1].get("id"), None);
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn attach_item_ids_panics_on_length_mismatch() {
        use codex_protocol::models::ContentItem;

        let items = vec![
            ResponseItem::Message {
                id: Some("msg-1".into()),
                role: "assistant".into(),
                content: vec![ContentItem::OutputText {
                    text: "hello".into(),
                }],
            },
            ResponseItem::Message {
                id: Some("msg-2".into()),
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "world".into(),
                }],
            },
        ];

        // Mismatch: 3 items in JSON but only 2 in original_items
        let mut payload = serde_json::json!({
            "model": "gpt-4",
            "input": [
                {"type": "message", "role": "assistant", "content": []},
                {"type": "message", "role": "user", "content": []},
                {"type": "message", "role": "user", "content": []}
            ]
        });

        // Should panic due to length mismatch
        attach_item_ids_to_json(&mut payload, &items);
    }
}
