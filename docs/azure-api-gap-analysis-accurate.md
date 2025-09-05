# Azure OpenAI API Gap Analysis - Accurate Report

## Summary
After thorough verification, Codex has substantial Azure OpenAI support with most core features working. The main gaps are in stream resumption and some GPT-5 specific parameters.

## ✅ WORKING Features

### 1. Azure Authentication
- **Correctly uses `api-key` header** for Azure (not Bearer) - `model_provider_info.rs:141-142`
- **API version parameter** properly appended - `model_provider_info.rs:374-378`
- **Provider detection** via `is_probably_azure()` - `model_provider_info.rs:215-226`

### 2. Background Mode
- **Fully functional** when `CODEX_ENABLE_BACKGROUND=1` - `client.rs:230-233`
- **Polling implemented** via `BackgroundTaskManager` - `client.rs:654-802`
- **Response retrieval works** via `get_response_url()` - `model_provider_info.rs:193-210`
- **Proper status handling** (queued, in_progress, completed, failed) - `client.rs:710-766`

### 3. Response Chaining
- **Tracks `last_response_id`** - `client.rs:75,95`
- **Updates on completion** - `client.rs:349-352, 410-413`
- **Uses for `previous_response_id`** - `client.rs:221-225`

### 4. Azure Event Handling
- **Reasoning deltas** handled - `client.rs:913-920, 976-983`
- **Preamble deltas** handled - `client.rs:922-929`
- **Output item streaming** works - `client.rs:948-959`

### 5. Error Handling
- **Azure error parsing** implemented - `client.rs:805-826`
- **Handles Azure error structure** with code/message/innererror

### 6. GPT-5 Features (Mostly Working)
- **`verbosity`** parameter - FULLY IMPLEMENTED
  - Low/Medium/High options - `config_types.rs:222-227`
  - Only applied for GPT-5 family - `client.rs:206-216`
- **`reasoning_effort`** - MOSTLY IMPLEMENTED
  - Has Low/Medium/High/None - `config_types.rs:193-200`
  - Missing: `minimal` option (GPT-5 specific)
- **`lark_tool`** - IMPLEMENTED
  - Full Lark grammar support - `tool_apply_patch.rs:31-49`

### 7. Other Features
- **`include` array** for encrypted reasoning - `client.rs:197-201`
- **`parallel_tool_calls`** set to false (conservative) - `client.rs:248`
- **`store` parameter** properly managed - `client.rs:185,235-240`
- **Built-in Azure providers** configured - `model_provider_info.rs:356-390`

## ❌ ACTUAL GAPS

### 1. Stream Resumption Not Implemented
**Evidence**: 
- `ResumeCtx` struct created (`client.rs:828-833`) and passed (`client.rs:326-334`)
- But parameter prefixed with `_` (unused) - `client.rs:839`
- `sequence_number` field exists (`client.rs:590`) but never accessed
- No implementation of resume with `?starting_after={cursor}`

**Impact**: Cannot resume interrupted streams

### 2. GPT-5 Parameter Gaps
**Missing**:
- `minimal` reasoning effort option (enum only has Low/Medium/High/None)
- `tool_choice` as array (hardcoded to `"auto"` - `client.rs:247`)
- Cannot specify multiple allowed tools

### 3. Unused Code
- `azure_responses_api.rs` file exists but never imported (no `mod azure_responses_api`)
- Appears to be alternative implementation that was never integrated

### 4. Header Behavior (Not Actually Wrong)
- Code EXCLUDES `OpenAI-Beta` header for Azure (`client.rs:279-281`)
- This appears intentional - no evidence Azure requires this header

### 5. Minor Issues
- `prompt_cache_key` sent but may not be Azure-compliant - `client.rs:255`
- No multi-region failover support
- No MCP approval flow handling

## ⚠️ NUANCED Behaviors

### Store Parameter Logic
```rust
// Disabled for ChatGPT auth mode (intentional)
let mut store = prompt.store && auth_mode != Some(AuthMode::ChatGPT);

// Forced on for background mode
if background == Some(true) && !store {
    warn!("background=true requires store=true; forcing store=true");
    store = true;
}
```
This is complex but appears correct for the different auth modes.

### Polling Intervals
- 50ms sleep between status checks (`client.rs:713`) - brief pause, not main interval
- Actual backoff: `backoff(attempt).min(Duration::from_secs(5))` (`client.rs:769`)
- Reasonable approach, not too aggressive

## Verification Notes

### Claims I Was Wrong About
1. **API Version** - Can't verify if `2025-04-01-preview` is wrong without checking Azure
2. **Response Retrieval** - IS implemented, I was wrong
3. **Headers** - Azure handling appears correct
4. **Background Mode** - Works fine

### Confirmed Issues
1. **Stream resumption** - Infrastructure exists but unused
2. **Some GPT-5 parameters** - Missing `minimal` effort and array tool_choice
3. **Orphaned code** - azure_responses_api.rs disconnected

## Recommendations

### Priority 1: Enable Stream Resumption
- Use the existing `sequence_number` field
- Implement resume logic in `process_sse()`
- Add endpoint for `GET /responses/{id}?stream=true&starting_after={cursor}`

### Priority 2: Complete GPT-5 Support
- Add `minimal` to `ReasoningEffort` enum
- Allow `tool_choice` to accept arrays
- Test with actual GPT-5 deployments

### Priority 3: Clean Up
- Either integrate or remove `azure_responses_api.rs`
- Document Azure-specific behaviors

## Overall Assessment

**Codex's Azure support is ~85% complete**. Core functionality works:
- Authentication ✅
- Background mode ✅
- Response chaining ✅
- Error handling ✅
- Most GPT-5 features ✅

Main gaps are:
- Stream resumption (infrastructure exists, just needs connecting)
- Some GPT-5 parameter options
- Orphaned alternative implementation

The codebase shows good Azure awareness with proper auth headers, error parsing, and provider detection. Most "gaps" I initially identified were either wrong or minor.