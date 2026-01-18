# Azure OpenAI `previous_response_id` Implementation

## Status

✅ **Implemented** - The `previous_response_id` feature for the Azure OpenAI Responses API is implemented and working for both HTTP/SSE and WebSocket connections.

## Azure Responses API Reference

Based on the Azure OpenAPI spec, the following endpoints and fields are supported:

### Endpoints
| Endpoint | Method | Description |
|----------|--------|-------------|
| `/responses` | POST | Create a response |
| `/responses/{response_id}` | GET | Retrieve a response |
| `/responses/{response_id}` | DELETE | Delete a response |
| `/responses/{response_id}/cancel` | POST | Cancel a background response |
| `/responses/{response_id}/input_items` | GET | List input items |

### Supported Request Fields (Azure)
| Field | Type | Codex Sends | Notes |
|-------|------|-------------|-------|
| `model` | string | ✅ | Required |
| `input` | array | ✅ | Via `prompt` field |
| `instructions` | string \| array | ✅ | System instructions |
| `tools` | array | ✅ | ToolsArray |
| `tool_choice` | string | ✅ | |
| `parallel_tool_calls` | boolean | ✅ | Default: true |
| `reasoning` | object | ✅ | `{effort, summary}` |
| `store` | boolean | ✅ | |
| `stream` | boolean | ✅ | |
| `previous_response_id` | string \| null | ✅ | Chain conversations |
| `background` | boolean \| null | ❌ | Long-running tasks |
| `prompt_cache_key` | string | ✅ | Prompt caching |
| `prompt_cache_retention` | "in-memory" \| "24h" | ❌ | Cache duration |
| `text` | ResponseTextParam | ✅ | Verbosity/format |
| `temperature` | number | ❌ | Default: 1 |
| `top_p` | number | ❌ | Default: 1 |
| `max_output_tokens` | integer | ❌ | |
| `max_tool_calls` | integer | ❌ | |
| `truncation` | "auto" \| "disabled" | ❌ | Default: "disabled" |
| `metadata` | object | ❌ | |
| `safety_identifier` | string | ❌ | User abuse detection |

### Response Fields (Azure)
| Field | Type | Notes |
|-------|------|-------|
| `id` | string | **Used for `previous_response_id` in next request** |
| `object` | "response" | Always "response" |
| `status` | enum | "completed", "failed", "in_progress", "cancelled", "queued", "incomplete" |
| `created_at` | integer | Unix timestamp |
| `output` | array | OutputItem array |
| `output_text` | string \| null | Convenience field |
| `usage` | object | Token usage |
| `error` | object \| null | Error details |
| `content_filters` | array | **Azure-specific**: RAI filter results |
| `previous_response_id` | string \| null | Echoed back if provided |

---

## How It Works

The `previous_response_id` feature enables efficient conversation chaining for Azure OpenAI by allowing the server to maintain conversation state. When a `previous_response_id` is included in a request, Azure can retrieve the previous conversation context from its storage instead of requiring the client to resend all previous messages.

### Data Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Data Flow                                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Response SSE ──► ResponseEvent::Completed { response_id }          │
│                                      │                               │
│                                      ▼                               │
│                   ModelClientSession.last_response_id                │
│                                      │                               │
│                                      ▼                               │
│              Next request: ResponsesApiRequest.previous_response_id  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Implementation Details

The feature is implemented across several files:

| File | Implementation |
|------|----------------|
| `codex-api/src/common.rs` | `ResponsesApiRequest` includes `previous_response_id` field |
| `codex-api/src/requests/responses.rs` | `ResponsesRequestBuilder` supports `.previous_response_id()` |
| `codex-api/src/endpoint/responses.rs` | `ResponsesOptions` passes through `previous_response_id` |
| `core/src/client.rs` | `ModelClientSession` stores and provides `last_response_id` |
| `core/src/codex.rs` | Captures `response_id` from `ResponseEvent::Completed` |

### Azure-Only Behavior

The `previous_response_id` field is only included in requests when:
1. The provider is detected as an Azure endpoint (via `provider.is_azure_responses_endpoint()`)
2. A previous response ID is available from the last completed response

### Benefits

1. **Reduced bandwidth**: Only new input items need to be sent, not the entire conversation history
2. **Lower latency**: Smaller request payloads result in faster transmission
3. **Token efficiency**: Azure doesn't need to reprocess the entire conversation context
4. **Consistency**: Azure maintains the canonical conversation state

### Testing

Azure `previous_response_id` chaining is tested in:
- `codex-rs/core/tests/suite/client.rs::azure_previous_response_id_only_sends_new_items` (SSE)
- `codex-rs/core/tests/suite/client_websockets.rs::azure_websocket_includes_previous_response_id_and_item_ids` (WebSocket)
- Integration tests verify that:
  - First request does NOT include `previous_response_id`
  - Subsequent requests DO include `previous_response_id` from the previous response
  - Only new items are sent in follow-up requests (not the full history)
  - Item IDs are correctly attached for Azure requests

---

## Additional Azure Features (Future Enhancements)

Based on the Azure OpenAPI spec, these additional features could be implemented:

### 1. Response Retrieval API

Azure supports retrieving stored responses:

```rust
// New endpoint: GET /responses/{response_id}
pub async fn retrieve_response(&self, response_id: &str) -> Result<Response, ApiError> {
    let url = format!("{}/responses/{}", self.base_url, response_id);
    // GET request with optional stream=true, starting_after for resume
}
```

**Use cases:**
- Resume interrupted sessions
- Retrieve conversation history from server
- Background task polling

### 2. Response Deletion API

```rust
// New endpoint: DELETE /responses/{response_id}
pub async fn delete_response(&self, response_id: &str) -> Result<(), ApiError> {
    let url = format!("{}/responses/{}", self.base_url, response_id);
    // DELETE request
}
```

**Use case:** Clean up stored responses for privacy/compliance.

### 3. Background Mode

For long-running tasks (reasoning models like o3):

```rust
#[derive(Debug, Serialize)]
pub struct ResponsesApiRequest<'a> {
    // ... existing fields ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
}
```

**Workflow:**
1. Send request with `background: true`
2. Receive immediate response with `status: "queued"` or `"in_progress"`
3. Poll `GET /responses/{id}` until `status: "completed"`
4. Or use `GET /responses/{id}?stream=true&starting_after={seq}` to resume streaming

### 4. Cancel Background Response

```rust
// New endpoint: POST /responses/{response_id}/cancel
pub async fn cancel_response(&self, response_id: &str) -> Result<Response, ApiError> {
    let url = format!("{}/responses/{}/cancel", self.base_url, response_id);
    // POST request
}
```

### 5. List Input Items

```rust
// New endpoint: GET /responses/{response_id}/input_items
pub async fn list_input_items(
    &self,
    response_id: &str,
    limit: Option<u32>,
    order: Option<&str>,  // "asc" | "desc"
    after: Option<&str>,
    before: Option<&str>,
) -> Result<InputItemList, ApiError> {
    // GET request with pagination
}
```

### 6. Prompt Cache Retention

Azure supports configuring cache duration:

```rust
#[derive(Debug, Serialize)]
pub struct ResponsesApiRequest<'a> {
    // ... existing fields ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<PromptCacheRetention>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptCacheRetention {
    InMemory,
    #[serde(rename = "24h")]
    TwentyFourHours,
}
```

### 7. Content Filters (Azure-Specific)

Azure responses include RAI content filter results:

```rust
#[derive(Debug, Deserialize)]
pub struct AzureContentFilter {
    pub hate: Option<ContentFilterResult>,
    pub self_harm: Option<ContentFilterResult>,
    pub sexual: Option<ContentFilterResult>,
    pub violence: Option<ContentFilterResult>,
}

#[derive(Debug, Deserialize)]
pub struct ContentFilterResult {
    pub filtered: bool,
    pub severity: String,  // "safe", "low", "medium", "high"
}
```

---

## Configuration Example

Example `config.toml` for Azure OpenAI with Responses API:

```toml
[model_providers.azure]
name = "Azure OpenAI"
base_url = "https://your-resource.openai.azure.com/openai/v1"
env_key = "AZURE_OPENAI_API_KEY"
wire_api = "responses"
query_params = { "api-version" = "v1" }

# Azure-specific settings
[model_providers.azure.azure]
use_previous_response_id = true
prompt_cache_retention = "24h"
```
