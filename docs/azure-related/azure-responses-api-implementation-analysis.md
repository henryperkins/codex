# Azure Responses API Implementation Analysis

## Overview
Codex has implemented support for the Azure OpenAI Responses API, allowing for asynchronous/background processing of requests.

## Implemented Endpoints

### 1. Create Response (POST /responses)
- **Location**: `codex-rs/core/src/client.rs:302-315`
- **Implementation**: Sends `ResponsesApiRequest` with model, messages, tools, and optional `previous_response_id`
- **Features**:
  - Supports background mode (`background: true`)
  - Allows response chaining via `previous_response_id`
  - Handles both SSE streaming and background polling modes

### 2. Get Response (GET /responses/{id})
- **Location**: `codex-rs/core/src/client.rs:731-830` (poll_until_complete)
- **Implementation**: Polls response status until completion
- **Features**:
  - Exponential backoff polling (attempts at 10ms, 50ms, 100ms intervals)
  - Handles status states: queued, in_progress, completed, failed, canceled
  - Extracts output items and usage data on completion

### 3. Cancel Response (POST /responses/{id}/cancel for Azure, DELETE /responses/{id} for others)
- **Location**: `codex-rs/core/src/client.rs:613-645`
- **Implementation**: Platform-aware cancellation
- **Features**:
  - Azure uses POST to `/cancel` endpoint
  - Other providers use DELETE method
  - Preserves query parameters for Azure

### 4. Delete Response (Not explicitly implemented)
- **Missing**: No explicit DELETE endpoint for removing completed responses

### 5. List Input Items (Not implemented)
- **Missing**: `GET /responses/{response_id}/input_items` endpoint not found

## Key Implementation Details

### Response Chaining
- Tracks `last_response_id` for automatic chaining (`client.rs:74-76`)
- Updates after each completed response (`client.rs:157-161`)
- Uses `previous_response_id` in requests when available

### Background Processing Flow
1. Send request with `background: true`
2. Receive response with `id` and `status: "queued"`
3. Poll `GET /responses/{id}` until status is `completed`
4. Extract output items and usage from completed response

### Azure-Specific Handling
- Different cancel endpoint (`POST /cancel` vs `DELETE`)
- API key header (`api-key`) instead of Bearer token for some deployments
- Query parameter preservation for API versioning

## Gaps Identified

### Missing Endpoints
1. **Delete Response**: No implementation for `DELETE /responses/{response_id}`
2. **List Input Items**: No implementation for `GET /responses/{response_id}/input_items`

### Potential Enhancements
1. No explicit list responses endpoint (if exists in spec)
2. No batch response operations
3. No response metadata update capabilities

## Compatibility Status
✅ **Core functionality implemented**: Create, retrieve, and cancel responses
✅ **Background mode support**: Full polling until completion
✅ **Response chaining**: Via `previous_response_id`
⚠️ **Partial implementation**: Missing delete and input items endpoints
✅ **Azure-specific adaptations**: Proper auth headers and cancel endpoint

## Recommendation
The implementation covers the essential Responses API functionality needed for background processing and response chaining. The missing endpoints (delete response, list input items) appear to be less critical for core functionality but could be added for completeness.