# Verification of Azure OpenAI Enhancement Plan vs Current Codebase

Date: 2025-09-19

This document captures the gap analysis between the **Azure OpenAI Enhancement Plan** (`azure_openai_enhancement_plan.md`) and the current state of the Codex codebase and documentation (commit checked-out in the workspace).

## Summary

| Area | Plan Status | Implementation Status | Notes |
|------|-------------|-----------------------|-------|
| **Streaming responses** | Described as _already working_ | Implemented in `ModelClient::stream_responses` | ✅ Matches plan’s premise. |
| **Phase 1 – Non-streaming responses** | Introduces `create_response_sync`, `ResponseOptions`, `ResponseResult`, `stream: false` flow | **Not implemented** | `ResponsesApiRequest` hard-codes `stream: true`; none of the proposed types exist. |
| **Phase 2 – Background processing** | Adds background creation, polling, cancellation helpers | **Not implemented** | No matching functions in `azure.rs` or elsewhere. |
| **Phase 3 – Stateless chaining (`previous_response_id`)** | Adds field to `ResponsesApiRequest` and `Prompt` | **Not implemented and currently disallowed** | Tests explicitly assert that `previous_response_id` is *not* sent. |
| **Phase 4 – Enhanced GET operations** | Adds extra query params & pagination helpers | **Not implemented** | `get_response` / `get_response_input_items` only support basic GET. |
| **Phase 5 – Azure-specific refinements** | (a) Conditional `store` workaround<br>(b) Rich header capture (`x-ms-request-id`, `x-ms-model-id`, `azure-openai-usage`) | (a) **Partially present** – workaround **always** forces `store: true`.<br>(b) **Partial** – only captures `azure-openai-usage`. | Additional logic required to complete both items. |

## Detailed Findings

### 1. Streaming Support
`codex-rs/core/src/client.rs` implements `stream_responses` which aligns with the plan’s “✅ Streaming response creation” bullet. No action required.

### 2. Non-Streaming Responses
Searches for `create_response_sync`, `ResponseOptions`, and `ResponseResult` returned **no code** outside the plan file. Therefore, the synchronous flow has not been started.

### 3. Background Processing
`codex-rs/core/src/azure.rs` lacks any background-related functions (`create_background_response`, `poll_background_response`, `cancel_background_response`). No other modules contain similar logic.

### 4. Stateless Conversation Chaining
Neither `Prompt` nor `ResponsesApiRequest` structures include `previous_response_id`. Tests (`codex-rs/core/tests/suite/client.rs`) assert that the field must **not** be present, confirming current behaviour conflicts with the planned change.

### 5. Enhanced GET Operations
Current helper functions only fetch full response objects and input items lists without pagination or filter options. Pagination structs and query-param building code suggested by the plan are absent.

### 6. Azure-Specific Improvements
*Store workaround*: The client always forces `store: true` for Azure, regardless of base URL version. The conditional logic proposed by the plan is missing.

*Header capture*: Only the `azure-openai-usage` header is attached to the returned `Response`. The additional `x-ms-request-id` and `x-ms-model-id` headers remain unexposed.

### 7. Documentation Alignment
`docs/azure_responses.md` documents the extra headers but **does not** cover non-streaming, background, or chaining features—consistent with the current implementation, not the enhancement plan.

## Conclusion

The enhancement plan accurately outlines functionality that is **not yet present** in the repository. Implementing Phases 1-4 and completing the Phase 5 refinements would require new code across `core`, `client_common`, and `azure` modules plus accompanying tests and doc updates.

