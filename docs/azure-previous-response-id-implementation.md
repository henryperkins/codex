# Implementation Plan for `previous_response_id` (Azure OpenAI)

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
| `previous_response_id` | string \| null | ❌ **TODO** | Chain conversations |
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
| `id` | string | **Use this for `previous_response_id`** |
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

## Overview

The key insight from the exploration is that `response_id` is already captured but **discarded** at line 2977 in `codex.rs`. The infrastructure exists, it just needs to be wired up.

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Data Flow                                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Response SSE ──► ResponseEvent::Completed { response_id } ──────┐  │
│                                                                  │  │
│                                          Currently discarded ◄───┘  │
│                                                                      │
│  ┌─────────────────── NEEDS TO BE ADDED ───────────────────────┐   │
│  │                                                              │   │
│  │  Store response_id ──► ModelClientSession.last_response_id   │   │
│  │                              │                               │   │
│  │                              ▼                               │   │
│  │  Next request: ResponsesApiRequest.previous_response_id      │   │
│  │                                                              │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Files to Modify (in order)

| File | Change |
|------|--------|
| `codex-api/src/common.rs` | Add `previous_response_id` to request struct |
| `codex-api/src/requests/responses.rs` | Add field to builder |
| `codex-api/src/endpoint/responses.rs` | Pass through in options |
| `core/src/client.rs` | Store and pass `response_id` |
| `core/src/codex.rs` | Capture `response_id` from completed event |

---

## Step 1: Add `previous_response_id` to Request Struct

**File:** `codex-rs/codex-api/src/common.rs`

```rust
#[derive(Debug, Serialize)]
pub struct ResponsesApiRequest<'a> {
    pub model: &'a str,
    pub instructions: &'a str,
    pub input: &'a [ResponseItem],
    pub tools: &'a [serde_json::Value],
    pub tool_choice: &'static str,
    pub parallel_tool_calls: bool,
    pub reasoning: Option<Reasoning>,
    pub store: bool,
    pub stream: bool,
    pub include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextControls>,
    // ADD THIS:
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
}
```

Also update `ResponseCreateWsRequest` for WebSocket support.

---

## Step 2: Add to Request Builder

**File:** `codex-rs/codex-api/src/requests/responses.rs`

```rust
#[derive(Default)]
pub struct ResponsesRequestBuilder<'a> {
    // ... existing fields ...
    previous_response_id: Option<String>,  // ADD THIS
}

impl<'a> ResponsesRequestBuilder<'a> {
    // ADD THIS METHOD:
    pub fn previous_response_id(mut self, id: Option<String>) -> Self {
        self.previous_response_id = id;
        self
    }

    pub fn build(self, provider: &Provider) -> Result<ResponsesRequest, ApiError> {
        // ... existing code ...

        let req = ResponsesApiRequest {
            model,
            instructions,
            input,
            tools,
            tool_choice: "auto",
            parallel_tool_calls: self.parallel_tool_calls,
            reasoning: self.reasoning,
            store,
            stream: true,
            include: self.include,
            prompt_cache_key: self.prompt_cache_key,
            text: self.text,
            previous_response_id: self.previous_response_id,  // ADD THIS
        };

        // ... rest of build ...
    }
}
```

---

## Step 3: Pass Through in Endpoint Options

**File:** `codex-rs/codex-api/src/endpoint/responses.rs`

```rust
pub struct ResponsesOptions {
    // ... existing fields ...
    pub previous_response_id: Option<String>,  // ADD THIS
}

// In stream_prompt():
let request = ResponsesRequestBuilder::new(model, &prompt.instructions, &prompt.input)
    .tools(&prompt.tools)
    .parallel_tool_calls(prompt.parallel_tool_calls)
    .reasoning(reasoning)
    .include(include)
    .prompt_cache_key(prompt_cache_key)
    .text(text)
    .conversation(conversation_id)
    .session_source(session_source)
    .store_override(store_override)
    .extra_headers(extra_headers)
    .compression(compression)
    .previous_response_id(options.previous_response_id.clone())  // ADD THIS
    .build(self.streaming.provider())?;
```

---

## Step 4: Store and Pass in Client Session

**File:** `codex-rs/core/src/client.rs`

```rust
pub struct ModelClientSession {
    state: Arc<ModelClientState>,
    connection: Option<ApiWebSocketConnection>,
    websocket_last_items: Vec<ResponseItem>,
    last_response_id: Option<String>,  // ADD THIS
}

impl ModelClientSession {
    // ADD THIS METHOD:
    pub fn set_last_response_id(&mut self, id: String) {
        self.last_response_id = Some(id);
    }

    // ADD THIS METHOD:
    pub fn last_response_id(&self) -> Option<&str> {
        self.last_response_id.as_deref()
    }

    // MODIFY build_responses_options():
    fn build_responses_options(&self, /* ... */) -> ResponsesOptions {
        ResponsesOptions {
            // ... existing fields ...
            previous_response_id: self.last_response_id.clone(),  // ADD THIS
        }
    }
}
```

---

## Step 5: Capture Response ID from Completed Event

**File:** `codex-rs/core/src/codex.rs` (around line 2977)

Currently:
```rust
ResponseEvent::Completed {
    response_id: _,  // DISCARDED!
    token_usage,
} => {
    sess.update_token_usage_info(&turn_context, token_usage.as_ref()).await;
    // ...
}
```

Change to:
```rust
ResponseEvent::Completed {
    response_id,
    token_usage,
} => {
    // Store the response_id for the next turn
    client_session.set_last_response_id(response_id);

    sess.update_token_usage_info(&turn_context, token_usage.as_ref()).await;
    // ...
}
```

---

## Step 6 (Optional): Azure-Only Behavior

If you want to only use `previous_response_id` for Azure providers:

**File:** `codex-rs/codex-api/src/requests/responses.rs`

```rust
pub fn build(self, provider: &Provider) -> Result<ResponsesRequest, ApiError> {
    // ... existing code ...

    // Only include previous_response_id for Azure
    let previous_response_id = if provider.is_azure_responses_endpoint() {
        self.previous_response_id
    } else {
        None
    };

    let req = ResponsesApiRequest {
        // ...
        previous_response_id,
    };

    // ...
}
```

---

## Step 7 (Optional): Optimize Input for Azure

When using `previous_response_id`, you can send only the **new** input items instead of full history:

**File:** `codex-rs/core/src/client.rs`

```rust
fn build_prompt_for_request(&self, prompt: &Prompt) -> Prompt {
    // If we have a previous_response_id, only send new items
    if self.last_response_id.is_some() && self.state.provider.is_azure() {
        Prompt {
            instructions: prompt.instructions.clone(),
            input: prompt.input.last().cloned().into_iter().collect(), // Only last item
            tools: prompt.tools.clone(),
            parallel_tool_calls: prompt.parallel_tool_calls,
            output_schema: prompt.output_schema.clone(),
        }
    } else {
        prompt.clone()
    }
}
```

⚠️ **Caution:** This optimization requires careful handling - you need to ensure the server has the full context stored.

---

## Testing Considerations

1. **Add test for Azure response ID capture:**
```rust
#[tokio::test]
async fn azure_stores_response_id_for_chaining() {
    // Mock Azure endpoint
    // Verify response_id is captured
    // Verify next request includes previous_response_id
}
```

2. **Verify backwards compatibility:** Non-Azure providers should continue to work with full history.

3. **Test conversation resume:** Verify that `previous_response_id` correctly chains conversations.

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
