# Azure OpenAI API Specification Summary

Based on the official Azure REST API specifications from: https://raw.githubusercontent.com/Azure/azure-rest-api-specs/refs/heads/main/specification/ai/data-plane/OpenAI.v1/azure-v1-preview-generated.json

## Core API Endpoints

### 1. Chat Completions
**Path:** `/chat/completions`
**Method:** POST
**Purpose:** Create chat completions with conversational context

**Request Parameters:**
- `messages`: Array of conversation messages
- `model`: Model to use for completion
- `temperature`: Sampling temperature (0-2)
- `max_tokens`: Maximum tokens to generate
- `stream`: Boolean for streaming responses
- `top_p`: Nucleus sampling parameter
- `frequency_penalty`: Reduce repetition
- `presence_penalty`: Encourage new topics
- `stop`: Stop sequences
- `n`: Number of completions to generate
- `logit_bias`: Token probability adjustments
- `user`: User identifier for tracking

**Response Structure:**
- Standard: Returns complete response with choices array
- Streaming: Server-sent events with incremental deltas

### 2. Embeddings
**Path:** `/embeddings`
**Method:** POST
**Purpose:** Generate vector representations of text

**Request Parameters:**
- `input`: Text or array of texts to embed
- `model`: Embedding model to use
- `encoding_format`: Output format (float or base64)
- `dimensions`: Output dimensionality (model-specific)

### 3. Fine-Tuning

#### Create Job
**Path:** `/fine_tuning/jobs`
**Method:** POST

#### List Jobs
**Path:** `/fine_tuning/jobs`
**Method:** GET

#### Get Job Details
**Path:** `/fine_tuning/jobs/{job_id}`
**Method:** GET

#### Cancel Job
**Path:** `/fine_tuning/jobs/{job_id}/cancel`
**Method:** POST

#### List Job Events
**Path:** `/fine_tuning/jobs/{job_id}/events`
**Method:** GET

### 4. Image Generation
**Path:** `/images/generations`
**Method:** POST
**Purpose:** Generate images from text prompts

**Request Parameters:**
- `prompt`: Text description of desired image
- `n`: Number of images to generate
- `size`: Image dimensions
- `response_format`: url or b64_json
- `quality`: Image quality level
- `style`: Image style preferences

### 5. Models

#### List Models
**Path:** `/models`
**Method:** GET

#### Get Model
**Path:** `/models/{model_id}`
**Method:** GET

## Azure-Specific Features

### API Versioning
- All endpoints require `api-version` query parameter
- Current preview version: `2024-10-01-preview`
- Stable versions also available

### Headers
- `api-key`: Authentication key (alternative to Bearer token)
- `x-ms-client-request-id`: Request tracking
- `aoai-evals`: Preview features flag
- `x-ms-stream-options`: Streaming configuration

### Error Handling
Consistent error response structure:
```json
{
  "error": {
    "code": "error_code",
    "message": "Human-readable error message",
    "type": "error_type",
    "param": "problematic_parameter",
    "inner_error": {}
  }
}
```

### Streaming Support
- Uses Server-Sent Events (SSE) format
- Line prefix: `data: `
- Termination signal: `data: [DONE]`
- Delta updates for incremental content

### Authentication Methods
1. API Key in header: `api-key: YOUR_KEY`
2. Bearer token: `Authorization: Bearer YOUR_TOKEN`
3. Azure AD / Entra ID integration

## Key Differences from OpenAI API

1. **Explicit API Versioning**: Azure requires version in URL
2. **Azure-specific Headers**: Additional tracking and configuration headers
3. **Resource Management**: Deployment names instead of model names
4. **Authentication**: Multiple auth methods including Azure AD
5. **Regional Endpoints**: Region-specific base URLs
6. **Preview Features**: Opt-in preview capabilities via headers

## Base URL Pattern
```
https://{resource-name}.openai.azure.com/openai/deployments/{deployment-id}/{endpoint}?api-version={version}
```

## Rate Limiting
- Tokens per minute (TPM) limits
- Requests per minute (RPM) limits
- Deployment-specific quotas
- Regional capacity constraints

## Best Practices
1. Always specify API version explicitly
2. Use deployment names consistently
3. Implement exponential backoff for rate limits
4. Handle streaming disconnections gracefully
5. Track requests with client request IDs
6. Use appropriate authentication method for environment