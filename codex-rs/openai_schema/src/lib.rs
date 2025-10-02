//! Auto-generated subset of the Azure OpenAI *v1 Preview* schema, limited to
//! the three `/responses` endpoints that are also available on the public
//! OpenAI *Responses* API. These types were generated from `docs/v1preview.json`
//! and then hand-trimmed for clarity. They intentionally cover *only* the
//! request/response payloads that Codex currently sends/receives. When we
//! integrate additional endpoints, this module can be regenerated from the
//! OpenAPI spec.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

// ---------------------------------------------------------------------------
//  POST /responses  (create a new response – can be streamed)
// ---------------------------------------------------------------------------

/// Request payload for `POST /responses`.
///
/// This struct closely mirrors the `AzureCreateResponse` schema from the
/// v1preview spec, but only includes the fields that are either required by
/// the spec or currently used by Codex-RS. Unknown/extra fields are captured
/// via `extra` so we stay forward-compatible with future spec changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResponseRequest {
    /// The model deployment name (e.g. "gpt-4o-preview").
    pub model: String,

    /// Full instructions for the model. Optional per the spec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// Array of input `ResponseItem`s. Codex currently uses the inline form
    /// rather than the file-based form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input: Vec<Value>,

    /// List of tool definitions available to the model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Value>,

    /// How the model should pick a tool. Either "auto" or an explicit choice.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,

    /// Whether the model may call multiple tools in parallel.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub parallel_tool_calls: bool,

    /// Reasoning effort & summary controls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Value>,

    /// Store the response for later retrieval (`true` for Azure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,

    /// Whether to stream the response as server-sent events.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,

    /// Include list (e.g. ["reasoning.encrypted_content"]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,

    /// Cache key used by Codex to thread multiple prompts together.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,

    /// Text formatting controls (verbosity, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Value>,

    /// Any future/unknown fields – keeps us forward-compatible without code-gen.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, Value>,
}

// ---------------------------------------------------------------------------
//  GET /responses/{id}
// ---------------------------------------------------------------------------

/// A single response object returned by `GET /responses/{id}`.
/// The schema (`AzureResponse`) is large; we keep only the common fields used
/// by Codex today and capture everything else in `extra` for forward-compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    #[serde(rename = "object")]
    pub object_type: String,
    pub created_at: u64,
    pub model: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Vec<ResponseItem>>, // usually a single text item

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,

    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, Value>,
}

// ---------------------------------------------------------------------------
//  GET /responses/{id}/input_items
// ---------------------------------------------------------------------------

/// Returned by `GET /responses/{id}/input_items`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseInputItemsList {
    pub data: Vec<ResponseItem>,

    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, Value>,
}

// ---------------------------------------------------------------------------
//  Shared sub-schemas
// ---------------------------------------------------------------------------

/// Mirrors `OpenAI.ResponseItem` but simplified.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseItem {
    Text {
        text: String,
    },

    /// Represents an arbitrary function/tool call. We leave the payload open.
    Tool {
        id: String,
        #[serde(flatten)]
        other: Value,
    },

    /// Catch-all for item types we don't explicitly model yet.
    #[serde(other)]
    Other,
}

// Helper so we can use `std::ops::Not` in field attrs above.
trait Not {
    fn not(&self) -> bool;
}

impl Not for bool {
    #[inline]
    fn not(&self) -> bool {
        !*self
    }
}
