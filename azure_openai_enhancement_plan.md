# Azure OpenAI Enhancement Plan for Codex

## Executive Summary

This document outlines a phased enhancement plan for the Azure OpenAI integration in Codex. The plan builds upon the existing robust streaming implementation to add missing Response API features while maintaining backward compatibility and code quality.

## Current State Analysis

### ✅ What's Already Working
- **Streaming response creation** via POST to `/responses` endpoint with `stream: true`
- **Full tool calling support** including function calls, web search, and custom tools
- **Reasoning model support** with configurable effort levels (minimal/low/medium/high)
- **Comprehensive error handling** with Azure-specific error parsing and retry logic
- **Authentication** supporting both API keys and Microsoft Entra ID
- **Token usage tracking** via Azure-specific headers

### ❌ Current Limitations
1. Cannot create non-streaming responses (`stream: false`)
2. No support for background/async processing (`background: true`)
3. Cannot use `previous_response_id` for stateless conversation chaining
4. Forced to use `store: true` due to Azure API limitations
5. Missing query parameters on GET endpoints (security and filtering options)

## Implementation Plan

### Phase 1: Non-Streaming Response Support (Priority: High)
**Estimated effort: 2-3 days**

#### 1.1 Add Synchronous Response Creation

**File: `codex-rs/core/src/client.rs`**

Add new method alongside existing `stream_responses`:

```rust
/// Creates a non-streaming response via the Responses API.
async fn create_response_sync(&self, prompt: &Prompt) -> Result<Response> {
    let auth_manager = self.auth_manager.clone();

    // Reuse existing payload construction logic
    let full_instructions = prompt.get_full_instructions(&self.config.model_family);
    let tools_json = create_tools_json_for_responses_api(&prompt.tools)?;
    let reasoning = create_reasoning_param_for_request(
        &self.config.model_family,
        self.effort,
        self.summary,
    );

    let payload = ResponsesApiRequest {
        model: &self.config.model,
        instructions: &full_instructions,
        input: &prompt.get_formatted_input(),
        tools: &tools_json,
        tool_choice: "auto",
        parallel_tool_calls: false,
        reasoning,
        store: self.provider.is_azure_responses_endpoint(), // Azure workaround
        stream: false,  // KEY DIFFERENCE: Non-streaming
        include: vec![],
        prompt_cache_key: Some(self.conversation_id.to_string()),
        text: create_text_param_for_request(self.config.model_verbosity),
    };

    // Send request and parse Response
    let response = self.send_request_with_retry(payload, auth_manager).await?;
    let azure_response = response.json::<codex_openai_schema::Response>().await?;

    Ok(azure_response)
}
```

#### 1.2 Update Public API

**File: `codex-rs/core/src/client.rs`**

Add response options struct and update public method:

```rust
#[derive(Debug, Default)]
pub struct ResponseOptions {
    pub stream: bool,
    pub background: bool,  // For Phase 2
    pub previous_response_id: Option<String>,  // For Phase 3
}

pub async fn create_response(&self, prompt: &Prompt, options: ResponseOptions) -> Result<ResponseResult> {
    match (options.stream, options.background) {
        (true, false) => Ok(ResponseResult::Stream(self.stream_responses(prompt).await?)),
        (false, false) => Ok(ResponseResult::Sync(self.create_response_sync(prompt).await?)),
        (_, true) => self.create_background_response(prompt, options).await,  // Phase 2
    }
}

pub enum ResponseResult {
    Stream(ResponseStream),
    Sync(Response),
    Background(BackgroundResponse),  // Phase 2
}
```

### Phase 2: Background Processing Support (Priority: Medium)
**Estimated effort: 3-4 days**

#### 2.1 Add Background Response Creation

**File: `codex-rs/core/src/azure.rs`**

Add new functions for background processing:

```rust
/// Creates a background response that can be polled for completion.
pub async fn create_background_response(
    provider: &ModelProviderInfo,
    client: &reqwest::Client,
    auth_manager: &Option<Arc<AuthManager>>,
    request: CreateResponseRequest,
) -> Result<BackgroundResponse> {
    let mut modified_request = request;
    modified_request.background = Some(true);
    modified_request.store = Some(true);  // Required for background

    // Send request - will return immediately with status
    let response = send_azure_request(provider, client, auth_manager, modified_request).await?;

    Ok(BackgroundResponse {
        id: response.id,
        status: response.status,
    })
}

/// Polls a background response for completion.
pub async fn poll_background_response(
    provider: &ModelProviderInfo,
    client: &reqwest::Client,
    auth_manager: &Option<Arc<AuthManager>>,
    response_id: &str,
) -> Result<Response> {
    // Reuse existing get_response with polling logic
    loop {
        let response = get_response(provider, client, auth_manager, response_id).await?;

        match response.status.as_deref() {
            Some("completed") | Some("failed") | Some("cancelled") => return Ok(response),
            Some("queued") | Some("in_progress") => {
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            _ => return Err(CodexErr::Azure(AzureError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "unknown_status".to_string(),
                message: format!("Unknown response status: {:?}", response.status),
                request_id: None,
            })),
        }
    }
}

/// Cancels an in-progress background response.
pub async fn cancel_background_response(
    provider: &ModelProviderInfo,
    client: &reqwest::Client,
    auth_manager: &Option<Arc<AuthManager>>,
    response_id: &str,
) -> Result<Response> {
    let auth = auth_manager.as_ref().and_then(|m| m.auth());
    let url = build_azure_url(provider, &auth, &format!("{}/cancel", response_id));

    let mut builder = client.post(&url);
    if let Some(auth) = auth.as_ref() {
        builder = builder.bearer_auth(auth.get_token().await?);
    }
    builder = provider.apply_http_headers(builder);

    let res = builder.send().await?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        let azure_err = parse_azure_error(body, status, res.headers());
        return Err(CodexErr::Azure(azure_err));
    }

    Ok(res.json::<Response>().await?)
}
```

#### 2.2 Add Background Response Struct

**File: `codex-rs/core/src/client_common.rs`**

```rust
#[derive(Debug)]
pub struct BackgroundResponse {
    pub id: String,
    pub status: String,
}

impl BackgroundResponse {
    pub async fn wait_for_completion(
        self,
        client: &Client
    ) -> Result<Response> {
        client.poll_background_response(&self.id).await
    }

    pub async fn cancel(self, client: &Client) -> Result<Response> {
        client.cancel_background_response(&self.id).await
    }
}
```

### Phase 3: Stateless Conversation Chaining (Priority: Medium)
**Estimated effort: 2 days**

#### 3.1 Update ResponsesApiRequest

**File: `codex-rs/core/src/client_common.rs`**

```rust
#[derive(Debug, Serialize)]
pub(crate) struct ResponsesApiRequest<'a> {
    // ... existing fields ...

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) previous_response_id: Option<String>,  // NEW FIELD
}
```

#### 3.2 Update Prompt Structure

**File: `codex-rs/core/src/client_common.rs`**

```rust
#[derive(Default, Debug, Clone)]
pub struct Prompt {
    pub input: Vec<ResponseItem>,
    pub(crate) tools: Vec<OpenAiTool>,
    pub base_instructions_override: Option<String>,
    pub previous_response_id: Option<String>,  // NEW FIELD
}
```

#### 3.3 Modify Request Construction

**File: `codex-rs/core/src/client.rs`**

Update both `stream_responses` and `create_response_sync`:

```rust
let payload = ResponsesApiRequest {
    // ... existing fields ...
    previous_response_id: prompt.previous_response_id.clone(),  // ADD THIS
};
```

### Phase 4: Enhanced GET Operations (Priority: Low)
**Estimated effort: 1 day**

#### 4.1 Update get_response with Query Parameters

**File: `codex-rs/core/src/azure.rs`**

```rust
pub async fn get_response_enhanced(
    provider: &ModelProviderInfo,
    client: &reqwest::Client,
    auth_manager: &Option<Arc<AuthManager>>,
    response_id: &str,
    include_obfuscation: Option<bool>,
    include: Option<Vec<String>>,
) -> Result<Response> {
    let auth = auth_manager.as_ref().and_then(|m| m.auth());
    let mut url = build_azure_url(provider, &auth, response_id);

    // Add query parameters
    if let Some(obfuscation) = include_obfuscation {
        url.push_str(&format!("&include_obfuscation={}", obfuscation));
    }

    if let Some(fields) = include {
        for field in fields {
            url.push_str(&format!("&include[]={}", urlencoding::encode(&field)));
        }
    }

    // Rest of implementation remains the same...
}
```

#### 4.2 Add Pagination for Input Items

**File: `codex-rs/core/src/azure.rs`**

```rust
pub struct InputItemsOptions {
    pub limit: Option<u32>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub order: Option<String>,
}

pub async fn get_response_input_items_paginated(
    provider: &ModelProviderInfo,
    client: &reqwest::Client,
    auth_manager: &Option<Arc<AuthManager>>,
    response_id: &str,
    options: InputItemsOptions,
) -> Result<ResponseInputItemsList> {
    let auth = auth_manager.as_ref().and_then(|m| m.auth());
    let mut url = build_azure_url(provider, &auth, &format!("{}/input_items", response_id));

    // Add pagination parameters
    if let Some(limit) = options.limit {
        url.push_str(&format!("&limit={}", limit));
    }
    if let Some(after) = options.after {
        url.push_str(&format!("&after={}", urlencoding::encode(&after)));
    }
    if let Some(before) = options.before {
        url.push_str(&format!("&before={}", urlencoding::encode(&before)));
    }
    if let Some(order) = options.order {
        url.push_str(&format!("&order={}", order));
    }

    // Execute request with existing retry logic...
}
```

### Phase 5: Azure-Specific Improvements (Priority: Low)
**Estimated effort: 2 days**

#### 5.1 Fix Store Workaround

**File: `codex-rs/core/src/client.rs`**

Add API version detection:

```rust
fn should_force_store(&self) -> bool {
    // Only force store: true for older Azure API versions
    if !self.provider.is_azure_responses_endpoint() {
        return false;
    }

    // Check if base_url contains newer API version
    let has_v1_endpoint = self.provider.base_url
        .as_deref()
        .map(|b| b.contains("/openai/v1"))
        .unwrap_or(false);

    !has_v1_endpoint  // Force store only for legacy endpoints
}
```

#### 5.2 Enhanced Header Capture

**File: `codex-rs/core/src/azure.rs`**

Add response metadata structure:

```rust
#[derive(Debug)]
pub struct AzureResponseMetadata {
    pub request_id: Option<String>,
    pub model_id: Option<String>,
    pub usage: Option<serde_json::Value>,
}

fn extract_azure_metadata(headers: &HeaderMap) -> AzureResponseMetadata {
    AzureResponseMetadata {
        request_id: headers.get("x-ms-request-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from),
        model_id: headers.get("x-ms-model-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from),
        usage: headers.get("azure-openai-usage")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| serde_json::from_str(v).ok()),
    }
}
```

## Testing Strategy

### Unit Tests

Add tests for each new function in their respective modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sync_response_creation() {
        // Test non-streaming response
    }

    #[tokio::test]
    async fn test_background_response_polling() {
        // Test background response lifecycle
    }

    #[tokio::test]
    async fn test_stateless_chaining() {
        // Test previous_response_id handling
    }
}
```

### Integration Tests

Create `codex-rs/core/tests/azure_integration.rs`:

```rust
#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_azure_full_flow() {
    let client = create_test_client();

    // Test sync response
    let prompt = create_test_prompt();
    let response = client.create_response(&prompt, ResponseOptions {
        stream: false,
        ..Default::default()
    }).await.unwrap();

    // Test background response
    let bg_response = client.create_response(&prompt, ResponseOptions {
        background: true,
        ..Default::default()
    }).await.unwrap();

    // Test chaining
    let follow_up = create_follow_up_prompt(response.id);
    let chained = client.create_response(&follow_up, ResponseOptions {
        stream: true,
        ..Default::default()
    }).await.unwrap();
}
```

## Migration Guide

### For Existing Code

No breaking changes - existing streaming code continues to work:

```rust
// Old code - still works
let stream = client.stream(&prompt).await?;

// New alternative for sync responses
let response = client.create_response(&prompt, ResponseOptions {
    stream: false,
    ..Default::default()
}).await?;
```

### Configuration Updates

No configuration changes required. New features use existing auth and provider settings.

## Rollout Plan

1. **Phase 1** (Week 1): Implement and test non-streaming responses
2. **Phase 2** (Week 2): Add background processing support
3. **Phase 3** (Week 3): Implement stateless chaining
4. **Phase 4-5** (Week 4): Enhanced GET operations and Azure optimizations
5. **Documentation** (Ongoing): Update docs as features are added

## Success Metrics

- ✅ All existing tests pass
- ✅ New features have >80% test coverage
- ✅ No performance regression in streaming responses
- ✅ Background responses handle long-running tasks (>1 minute)
- ✅ Stateless chaining reduces memory usage for long conversations

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Azure API changes | Use versioned endpoints, handle unknown fields gracefully |
| Breaking existing code | All changes are additive, existing APIs unchanged |
| Rate limiting | Reuse existing retry logic with exponential backoff |
| Authentication issues | Leverage existing AuthManager token refresh |

## Dependencies

- No new external dependencies required
- Uses existing `reqwest`, `serde_json`, and `tokio` infrastructure
- Reuses `codex_openai_schema` types where applicable

## Future Considerations

After these enhancements are complete, consider:

1. **Container API** for persistent Code Interpreter sessions
2. **Embeddings API** if vector operations become necessary
3. **OpenAPI code generation** to auto-update from Azure spec
4. **Metrics collection** using Azure Application Insights

## Conclusion

This plan provides a systematic approach to enhancing Azure OpenAI support while maintaining code quality and backward compatibility. The phased approach allows for incremental delivery and testing, reducing risk while providing value at each stage.