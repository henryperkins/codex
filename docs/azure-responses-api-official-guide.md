# Azure OpenAI Responses API - Official Implementation Guide

Based on Microsoft's official documentation for Azure AI Foundry OpenAI service.

## Authentication Setup

### Using API Key (Python)
```python
import os
from openai import OpenAI

client = OpenAI(
    api_key=os.getenv("AZURE_OPENAI_API_KEY"),
    base_url="https://YOUR-RESOURCE-NAME.openai.azure.com/openai/v1/",
)
```

### Using Microsoft Entra ID (Python)
```python
from openai import AzureOpenAI
from azure.identity import DefaultAzureCredential, get_bearer_token_provider

token_provider = get_bearer_token_provider(
    DefaultAzureCredential(), 
    "https://cognitiveservices.azure.com/.default"
)

client = AzureOpenAI(
    azure_endpoint="https://YOUR-RESOURCE-NAME.openai.azure.com",
    azure_ad_token_provider=token_provider,
    api_version="preview",
)
```

## Core Response Operations

### 1. Create Response
```python
response = client.responses.create(   
    model="gpt-4o",  # Your deployment name
    input="This is a test.",
)

print(response.model_dump_json(indent=2))
```

### 2. Retrieve Response
```python
response = client.responses.retrieve("resp_67cb61fa3a448190bcf2c42d96f0d1a8")
print(response)
```

### 3. Delete Response
```python
response = client.responses.delete("resp_67cb61fa3a448190bcf2c42d96f0d1a8")
print(response)
```

## Response Chaining - Two Methods

### Method 1: Automatic Chaining with previous_response_id

```python
# First response
response = client.responses.create(
    model="gpt-4o",
    input="Define and explain the concept of catastrophic forgetting?"
)

# Chain using previous_response_id
second_response = client.responses.create(
    model="gpt-4o",
    previous_response_id=response.id,
    input=[{
        "role": "user", 
        "content": "Explain this at a level that could be understood by a college freshman"
    }]
)

print(second_response.model_dump_json(indent=2))
```

### Method 2: Manual Chaining by Building Input Array

```python
# Initialize conversation
inputs = [{
    "type": "message", 
    "role": "user", 
    "content": "Define and explain the concept of catastrophic forgetting?"
}]

# First response
response = client.responses.create(
    model="gpt-4o",
    input=inputs
)

# Manually append response to inputs
inputs += response.output

# Add next user message
inputs.append({
    "role": "user", 
    "type": "message", 
    "content": "Explain this at a level that could be understood by a college freshman"
})

# Second response with full context
second_response = client.responses.create(
    model="gpt-4o",
    input=inputs
)

print(second_response.model_dump_json(indent=2))
```

## Streaming Responses

### Basic Streaming
```python
response = client.responses.create(
    input="This is a test",
    model="gpt-4o",
    stream=True
)

for event in response:
    if event.type == 'response.output_text.delta':
        print(event.delta, end='')
```

## Function Calling

```python
# Define function/tool
response = client.responses.create(
    model="gpt-4o",
    tools=[{
        "type": "function",
        "name": "get_weather",
        "description": "Get the weather for a location",
        "parameters": {
            "type": "object",
            "properties": {
                "location": {"type": "string"},
            },
            "required": ["location"],
        },
    }],
    input=[{"role": "user", "content": "What's the weather in San Francisco?"}],
)

print(response.model_dump_json(indent=2))

# Process function calls and provide outputs
input = []
for output in response.output:
    if output.type == "function_call":
        if output.name == "get_weather":
            input.append({
                "type": "function_call_output",
                "call_id": output.call_id,
                "output": '{"temperature": "70 degrees"}',
            })

# Submit function outputs with chaining
second_response = client.responses.create(
    model="gpt-4o",
    previous_response_id=response.id,
    input=input
)

print(second_response.model_dump_json(indent=2))
```

## Background Mode

### Create Background Task
```python
# Start background task
resp = client.responses.create(
    model="o3",
    input="Write a very long novel about otters in space.",
    background=True,
    store=True  # Required for background mode
)

print(f"Background task ID: {resp.id}")
print(f"Initial status: {resp.status}")
```

### Poll Background Task
```python
from time import sleep

# Poll until complete
while resp.status in {"queued", "in_progress"}:
    print(f"Current status: {resp.status}")
    sleep(2)
    resp = client.responses.retrieve(resp.id)

print(f"Final status: {resp.status}")
if resp.status == "completed":
    print(f"Output:\n{resp.output_text}")
```

### Cancel Background Task
```python
resp = client.responses.cancel("resp_123")
print(f"Cancelled. Final status: {resp.status}")
```

### Stream Background Response
```python
# Create background stream
stream = client.responses.create(
    model="o3",
    input="Write a very long novel about otters in space.",
    background=True,
    stream=True,
    store=True
)

# Track cursor for resume capability
cursor = None
for event in stream:
    print(event)
    if hasattr(event, 'sequence_number'):
        cursor = event.sequence_number
```

## TypeScript/JavaScript Implementation

### Setup
```typescript
import OpenAI from "openai";

const client = new OpenAI({
  apiKey: process.env.AZURE_OPENAI_API_KEY,
  baseURL: "https://YOUR-RESOURCE-NAME.openai.azure.com/openai/v1/"
});
```

### Response Chaining (TypeScript)
```typescript
// Method 1: Using previous_response_id
const response = await client.responses.create({
  model: "gpt-4o",
  input: "Define catastrophic forgetting"
});

const secondResponse = await client.responses.create({
  model: "gpt-4o",
  previous_response_id: response.id,
  input: [{
    role: "user",
    content: "Explain this simply"
  }]
});

// Method 2: Manual chaining
let inputs = [{
  type: "message",
  role: "user", 
  content: "Define catastrophic forgetting"
}];

const firstResp = await client.responses.create({
  model: "gpt-4o",
  input: inputs
});

inputs = [...inputs, ...firstResp.output];
inputs.push({
  role: "user",
  type: "message",
  content: "Explain this simply"
});

const secondResp = await client.responses.create({
  model: "gpt-4o",
  input: inputs
});
```

## Best Practices

### 1. Response Chaining
- Use `previous_response_id` for simple conversation continuation
- Use manual input building for more control over context
- Always set `store: true` when chaining responses

### 2. Background Tasks
- Always use `store: true` for background mode
- Implement exponential backoff when polling
- Save cursor position for stream resumption
- Handle all status states: queued, in_progress, completed, failed, cancelled

### 3. Error Handling
```python
try:
    response = client.responses.create(
        model="gpt-4o",
        input="Your input here"
    )
except Exception as e:
    print(f"Error creating response: {e}")
    # Implement retry logic with exponential backoff
```

### 4. Streaming Best Practices
```python
def process_stream_with_error_handling(stream):
    try:
        for event in stream:
            if event.type == 'response.output_text.delta':
                yield event.delta
            elif event.type == 'response.error':
                raise Exception(f"Stream error: {event.error}")
    except Exception as e:
        print(f"Stream processing error: {e}")
        # Implement resume logic if needed
```

## Common Patterns

### Conversation with Memory
```python
class ConversationManager:
    def __init__(self, client, model):
        self.client = client
        self.model = model
        self.last_response_id = None
    
    def send_message(self, message):
        request = {
            "model": self.model,
            "input": [{"role": "user", "content": message}]
        }
        
        if self.last_response_id:
            request["previous_response_id"] = self.last_response_id
        
        response = self.client.responses.create(**request)
        self.last_response_id = response.id
        return response.output_text
```

### Background Task Manager
```python
class BackgroundTaskManager:
    def __init__(self, client):
        self.client = client
        self.tasks = {}
    
    async def create_task(self, model, input_text):
        response = self.client.responses.create(
            model=model,
            input=input_text,
            background=True,
            store=True
        )
        self.tasks[response.id] = response
        return response.id
    
    async def wait_for_completion(self, task_id, poll_interval=2):
        import asyncio
        
        while True:
            response = self.client.responses.retrieve(task_id)
            self.tasks[task_id] = response
            
            if response.status in {"completed", "failed", "cancelled"}:
                return response
            
            await asyncio.sleep(poll_interval)
```

## Important Notes

1. **Data Retention**: Response objects are retained for 30 days by default
2. **API Version**: Use `api-version=preview` for latest features
3. **Authentication**: Token-based auth is recommended over API keys
4. **Model Support**: Available for gpt-4o, gpt-4.1, o3, o4-mini and other models
5. **Region Availability**: Check specific region support for Responses API

## Error Response Format
```json
{
  "error": {
    "message": "Error description",
    "type": "error_type",
    "code": "error_code"
  }
}
```

## Response Object Structure
```json
{
  "id": "resp_xxx",
  "status": "completed",
  "model": "gpt-4o",
  "created_at": 1234567890,
  "output": [
    {
      "type": "message",
      "role": "assistant",
      "content": [...]
    }
  ],
  "output_text": "The response text",
  "usage": {
    "input_tokens": 100,
    "output_tokens": 200,
    "total_tokens": 300
  },
  "metadata": {},
  "previous_response_id": "resp_yyy"
}
```