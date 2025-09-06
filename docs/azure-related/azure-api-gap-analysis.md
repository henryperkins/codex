# Azure OpenAI API Gap Analysis Report

## Executive Summary

After comprehensive analysis of the Codex application's Azure OpenAI integration against Microsoft's official documentation, I've identified critical gaps and misalignments that affect functionality, compliance, and performance.

## Current Implementation Status

### ✅ Correctly Implemented
1. **API Key Authentication** - Properly uses `api-key` header for Azure (not Bearer)
2. **API Version Parameter** - Correctly appends `api-version` query parameter
3. **Provider Detection** - Has `is_probably_azure()` method for Azure-specific behavior
4. **Error Parsing** - Custom `parse_azure_error_message()` for Azure error format
5. **Built-in Providers** - Includes both `azure-responses` and `azure-chat` configurations
6. **Background Polling** - Has `BackgroundTaskManager` for polling background tasks
7. **Response Chaining** - Tracks `last_response_id` for conversation continuity

### ❌ Critical Gaps & Misalignments

## 1. Header Management Issues

### OpenAI-Beta Header Misuse
**Location**: `client.rs:280`
```rust
if !self.provider.is_probably_azure() {
    req_builder = req_builder.header("OpenAI-Beta", "responses=experimental");
}
```
**Issue**: The code EXCLUDES the OpenAI-Beta header for Azure, but Azure documentation shows this header IS required for preview features.
**Impact**: Missing capabilities for Azure preview features

### Missing Azure-Specific Headers
**Not Implemented**:
- `x-ms-region` - For region-specific routing
- `x-ms-useragent` - For Azure telemetry
- No support for deployment-specific headers

## 2. Store Parameter Handling

### Incorrect Default Behavior
**Location**: `client.rs:185`
```rust
let mut store = prompt.store && auth_mode != Some(AuthMode::ChatGPT);
```
**Issue**: Azure defaults to `store=true`, OpenAI defaults to `store=false`
**Documentation**: Azure requires `store=true` for background mode and response chaining
**Impact**: Features may fail silently when store is incorrectly set to false

### Forced Store for Background
**Location**: `client.rs:237-239`
```rust
if background == Some(true) && !store {
    warn!("background=true requires store=true; forcing store=true");
    store = true;
}
```
**Good**: Correctly enforces the requirement
**Bad**: Only logs a warning instead of treating as configuration error

## 3. Background Mode Implementation

### Polling Interval Issues
**Location**: `client.rs:713, 769`
```rust
tokio::time::sleep(Duration::from_millis(50)).await; // Too aggressive
let delay = backoff(attempt).min(Duration::from_secs(5)); // Capped too low
```
**Issue**: 50ms initial polling is too aggressive for Azure
**Recommendation**: Azure docs suggest 2-5 second initial intervals with exponential backoff

### Missing Resume Capability
**Location**: `client.rs:839`
```rust
_resume_ctx: Option<ResumeCtx>, // Parameter exists but UNUSED
```
**Issue**: Resume context is passed but never implemented
**Impact**: Cannot resume interrupted streams as Azure documentation demonstrates

### No Cursor Tracking
**Missing**: No `sequence_number` tracking for stream resumption
**Required**: Azure returns `sequence_number` in events for `?starting_after={cursor}` resume

## 4. Model-Specific Features Not Handled

### Missing Azure-Specific Parameters
**Not Implemented**:
- `reasoning_effort` (low/medium/high) - for o-series models
- `verbosity` - for GPT-5 models  
- `truncation` - for response length control
- `include[]` array - for selective data inclusion
- `prompt_cache_key` - sent but not Azure-compliant

### Unsupported Event Types
**Location**: `client.rs:912-928`
```rust
"response.reasoning.delta" => { /* Handled */ }
"response.preamble.delta" => { /* Handled */ }
// Missing:
// - response.image_generation_call.partial_image
// - response.mcp_approval_request
// - response.function_call.arguments.delta
```

## 5. Response Chaining Limitations

### Previous Response ID Management
**Location**: `client.rs:221-224`
```rust
let previous_response_id = if store {
    prompt.previous_response_id.clone()
} else {
    None
};
```
**Issue**: Only chains when store=true, but Azure REQUIRES store=true for chaining
**Missing**: No validation that previous_response_id exists on Azure

### No Response Retrieval
**Missing**: No implementation of `GET /v1/responses/{id}` for retrieving stored responses
**Impact**: Cannot retrieve conversation history or resume sessions

## 6. Tool Calling Discrepancies

### Function Output Format
**Location**: `azure_responses_api.rs:523-529`
```rust
serde_json::json!({
    "type": "function_call_output",
    "call_id": call_id,
    "output": output.to_string(), // String conversion
})
```
**Issue**: Azure expects raw JSON for output, not stringified
**Documentation**: Shows `"output": JSON` not `"output": "string"`

### No Parallel Tool Call Flag
**Missing**: `parallel_tool_calls` parameter not sent to Azure
**Impact**: Sequential execution only, missing performance optimization

## 7. Security & Compliance

### Zero Data Retention (ZDR) Not Handled
**Missing**: No support for `reasoning.encrypted_content` in stateless mode
**Required**: Azure ZDR deployments need encrypted reasoning for context preservation

### MCP Tool Approval Flow
**Not Implemented**: No handling of `mcp_approval_request` events
**Security Risk**: Tools execute without user approval when required

## 8. Error Handling Gaps

### Incomplete Azure Error Parsing
**Location**: `client.rs:805-826`
```rust
fn parse_azure_error_message(body: &str) -> Option<String> {
    // Only handles basic error structure
    // Missing: content_filter, rate_limit, deployment errors
}
```
**Missing Error Types**:
- Content filter violations
- Deployment not found
- Region not supported  
- Quota exceeded with reset time

### No Retry-After Header Support for Azure
**Location**: `client.rs:428-432`
```rust
let retry_after_secs = res.headers()
    .get(reqwest::header::RETRY_AFTER)
    // Azure uses different header names
```
**Issue**: Azure may use `x-ms-retry-after` or `x-ratelimit-reset-after`

## 9. Streaming Issues

### SSE Event Parsing
**Issue**: No handling of Azure's chunked JSON in SSE events
**Impact**: May fail on partial JSON payloads

### Keep-Alive Events
**Missing**: No filtering of Azure keep-alive events
**Result**: May process empty events as errors

## 10. Configuration Issues

### API Version Hardcoded
**Location**: `model_provider_info.rs:378`
```rust
.unwrap_or_else(|| "2025-04-01-preview".to_string())
```
**Issue**: Hardcoded future version that doesn't exist yet
**Current**: Should be `2024-10-01-preview` or `2024-08-01-preview`

### No Region Failover
**Missing**: No support for multiple Azure regions
**Impact**: No redundancy on region outages

## Severity Assessment

### 🔴 Critical (Breaks Functionality)
1. Resume capability not implemented despite infrastructure
2. Wrong API version hardcoded
3. Store parameter default incorrect
4. Response retrieval missing

### 🟡 Major (Degrades Experience)
1. Polling intervals too aggressive
2. Missing Azure-specific parameters
3. No parallel tool calling
4. Incomplete error handling

### 🟢 Minor (Best Practice)
1. Header organization could be cleaner
2. More robust provider detection
3. Better logging for Azure-specific paths

## Recommendations

### Immediate Actions
1. Fix API version to `2024-10-01-preview`
2. Implement stream resume with cursor tracking
3. Default `store=true` for Azure providers
4. Add response retrieval endpoint support

### Short-term Improvements
1. Implement Azure-specific event types
2. Add parallel tool calling support
3. Enhance error parsing for Azure formats
4. Adjust polling intervals per Azure guidelines

### Long-term Enhancements
1. Full ZDR support with encrypted reasoning
2. Multi-region failover capability
3. MCP tool approval flow
4. Comprehensive Azure error taxonomy

## Testing Gaps

### Missing Test Coverage
- No tests for stream resumption
- No tests for Azure error formats
- No background polling tests with Azure
- No response chaining validation
- No tests for store parameter behavior

### Test Recommendations
1. Mock Azure SSE server with all event types
2. Test background task lifecycle
3. Validate response chaining with previous_response_id
4. Test all Azure-specific error scenarios
5. Verify header application for Azure

## Compliance Notes

### Microsoft Requirements Not Met
1. **Telemetry**: No Azure-specific user agent or tracking
2. **Rate Limiting**: Incorrect header parsing for Azure limits
3. **Security**: MCP approval flow not implemented
4. **Data Residency**: No region pinning support

### Potential Azure Certification Issues
- Missing required preview headers
- Incorrect default parameters
- Incomplete error handling
- No support for Azure-specific features

## Conclusion

While Codex has basic Azure support, it lacks critical features required for production Azure OpenAI deployments. The most severe issues are:

1. **Stream resumption not working** despite partial implementation
2. **Incorrect API version** pointing to non-existent future version  
3. **Store parameter mishandling** breaking Azure-specific features
4. **Missing response retrieval** preventing conversation management

These gaps mean Codex cannot fully utilize Azure OpenAI's Responses API capabilities, particularly for production scenarios requiring reliability, compliance, and advanced features like reasoning models or background processing.

## Appendix: Code Locations

Key files requiring updates:
- `client.rs`: Main client implementation (headers, streaming, polling)
- `model_provider_info.rs`: Provider configuration (API version, defaults)
- `client_common.rs`: Request/response structures (missing fields)
- `azure_responses_api.rs`: Dedicated Azure module (partially implemented, unused)

The `azure_responses_api.rs` file appears to be a work-in-progress that implements many required features but is not integrated into the main codebase.