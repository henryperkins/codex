//! Azure OpenAI-specific utilities.
//!
//! Centralizes Azure endpoint detection and request handling to ensure
//! consistent behavior across SSE and WebSocket paths.

use codex_protocol::models::ResponseItem;
use serde_json::Value;

/// Domain markers that identify Azure OpenAI endpoints.
///
/// These patterns are checked (case-insensitively) against the base URL
/// to auto-detect Azure deployments.
const AZURE_DOMAIN_MARKERS: &[&str] = &[
    "openai.azure.",
    "cognitiveservices.azure.",
    "aoai.azure.",
    "azure-api.",
    "azurefd.",
    "windows.net/openai",
];

/// Returns true if the given base URL appears to be an Azure OpenAI endpoint.
pub fn is_azure_base_url(base_url: &str) -> bool {
    let base_lower = base_url.to_ascii_lowercase();
    AZURE_DOMAIN_MARKERS
        .iter()
        .any(|marker| base_lower.contains(marker))
}

/// Attaches item IDs to a JSON request payload for Azure Responses API.
///
/// Azure requires item IDs to be present in the request for response chaining
/// to work correctly. The `ResponseItem` struct has `skip_serializing` on ID
/// fields, so this function patches the serialized JSON to include them.
pub fn attach_item_ids_to_json(payload_json: &mut Value, original_items: &[ResponseItem]) {
    let Some(input_value) = payload_json.get_mut("input") else {
        return;
    };
    let Value::Array(items) = input_value else {
        return;
    };

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
            "https://foo.cognitiveservices.azure.cn/openai",
            "https://foo.aoai.azure.com/openai",
            "https://foo.openai.azure-api.net/openai",
            "https://foo.z01.azurefd.net/",
            "https://myaccount.blob.core.windows.net/openai/something",
        ];
        for url in positive_cases {
            assert!(is_azure_base_url(url), "expected {url} to be Azure");
        }

        let negative_cases = [
            "https://api.openai.com/v1",
            "https://example.com/openai",
            "https://myproxy.azurewebsites.net/openai",
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
}
