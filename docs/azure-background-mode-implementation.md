# Azure OpenAI Responses API - Background Mode Implementation Guide

## Overview

The Azure OpenAI Responses API introduces powerful background mode capabilities for long-running tasks, enabling asynchronous execution for models like o3 and o4-mini. This guide provides practical implementation examples for the Codex project.

## Key Features

1. **Asynchronous Task Execution**: Run long operations without timeout concerns
2. **Status Polling**: Check task progress (queued → in_progress → completed)
3. **Stream Resumption**: Resume interrupted streams using cursor positions
4. **Response Chaining**: Maintain conversation context across multiple requests
5. **Tool Integration**: Chain function calls with preserved context

## TypeScript Implementation

### 1. Background Task with Polling

```typescript
// background-task.ts
import OpenAI from "openai";

interface BackgroundOptions {
  maxRetries?: number;
  pollInterval?: number;
  timeout?: number;
}

export class BackgroundTaskManager {
  private client: OpenAI;
  private endpoint: string;
  private apiKey: string;
  
  constructor(endpoint: string, apiKey: string) {
    this.endpoint = endpoint;
    this.apiKey = apiKey;
    this.client = new OpenAI({
      apiKey,
      baseURL: `${endpoint}/openai/v1`,
    });
  }

  async executeWithPolling(
    model: string,
    input: string,
    options: BackgroundOptions = {}
  ) {
    const { 
      maxRetries = 100, 
      pollInterval = 2000,
      timeout = 600000 
    } = options;
    
    // Exponential backoff parameters
    let currentInterval = pollInterval;
    const maxInterval = 30000; // Max 30 seconds
    const backoffMultiplier = 1.5;
    
    // Start background task
    const response = await this.client.responses.create({
      model,
      input,
      background: true,
      store: true, // Required for background mode
    });
    
    console.log(`Background task started: ${response.id}`);
    
    const startTime = Date.now();
    let attempts = 0;
    
    // Poll until completion
    while (attempts < maxRetries) {
      if (Date.now() - startTime > timeout) {
        throw new Error("Background task timeout");
      }
      
      const status = await this.client.responses.retrieve(response.id);
      
      if (status.status === "completed") {
        return status;
      }
      
      if (status.status === "failed") {
        throw new Error(`Task failed: ${status.error}`);
      }
      
      console.log(`Status: ${status.status} (attempt ${attempts + 1}/${maxRetries})`);
      
      // Exponential backoff with jitter
      const jitter = Math.random() * 1000; // 0-1 second jitter
      await new Promise(resolve => setTimeout(resolve, currentInterval + jitter));
      
      // Increase interval for next iteration
      currentInterval = Math.min(currentInterval * backoffMultiplier, maxInterval);
      attempts++;
    }
    
    throw new Error("Max polling attempts exceeded");
  }
  
  async cancel(responseId: string) {
    // Cancel endpoint for Azure OpenAI
    const url = `${this.endpoint}/openai/v1/responses/${responseId}/cancel`;
    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${this.apiKey}`,
        'Content-Type': 'application/json',
      },
    });
    
    if (!response.ok) {
      throw new Error(`Cancel failed: ${response.statusText}`);
    }
    
    return await response.json();
  }
}
```

### 2. Streaming with Resume Capability

```typescript
// streaming-resume.ts
import OpenAI from "openai";
import { EventEmitter } from "events";

export class ResumableStream extends EventEmitter {
  private client: OpenAI;
  private endpoint: string;
  private apiKey: string;
  private responseId: string | null = null;
  private lastCursor: number | null = null;
  private buffer: string[] = [];
  
  constructor(endpoint: string, apiKey: string) {
    super();
    this.endpoint = endpoint;
    this.apiKey = apiKey;
    this.client = new OpenAI({
      apiKey,
      baseURL: `${endpoint}/openai/v1`,
    });
  }
  
  async startStream(model: string, input: string) {
    try {
      const stream = await this.client.responses.create({
        model,
        input,
        background: true,
        stream: true,
        store: true,
      }) as any;
      
      await this.processStream(stream);
    } catch (error) {
      this.emit("error", error);
      // Attempt to resume if we have a cursor
      if (this.responseId && this.lastCursor !== null) {
        await this.resume();
      }
    }
  }
  
  private async processStream(stream: any) {
    for await (const event of stream) {
      // Capture response ID and cursor
      if (event.type === "response.created") {
        this.responseId = event.response?.id ?? this.responseId;
      }
      
      if ("sequence_number" in event) {
        this.lastCursor = event.sequence_number;
      }
      
      // Emit text deltas
      if (event.type === "response.output_text.delta") {
        this.buffer.push(event.delta);
        this.emit("delta", event.delta);
      }
      
      // Handle completion
      if (event.type === "response.completed") {
        this.emit("complete", this.buffer.join(""));
      }
    }
  }
  
  async resume() {
    if (!this.responseId || this.lastCursor === null) {
      throw new Error("Cannot resume: missing response ID or cursor");
    }
    
    console.log(`Resuming from cursor ${this.lastCursor}`);
    
    const url = `${this.endpoint}/openai/v1/responses/${this.responseId}?stream=true&starting_after=${this.lastCursor}`;
    
    const response = await fetch(url, {
      headers: {
        "Authorization": `Bearer ${this.apiKey}`,
        "Content-Type": "application/json",
      },
    });
    
    if (!response.ok) {
      throw new Error(`Resume failed: ${response.statusText}`);
    }
    
    // Process resumed stream
    const reader = response.body?.getReader();
    if (!reader) {
      throw new Error("No response body available");
    }
    
    const decoder = new TextDecoder();
    
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      
      const chunk = decoder.decode(value, { stream: true });
      
      // Parse SSE events
      const lines = chunk.split('\n');
      for (const line of lines) {
        if (line.startsWith('data: ')) {
          const data = line.slice(6);
          if (data === '[DONE]') {
            this.emit("complete", this.buffer.join(""));
            return;
          }
          try {
            const event = JSON.parse(data);
            if (event.type === "response.output_text.delta") {
              this.buffer.push(event.delta);
              this.emit("delta", event.delta);
            }
            if (event.sequence_number) {
              this.lastCursor = event.sequence_number;
            }
          } catch (e) {
            // Skip invalid JSON
          }
        }
      }
    }
  }
  
  getContent() {
    return this.buffer.join("");
  }
  
  getState() {
    return {
      responseId: this.responseId,
      cursor: this.lastCursor,
      content: this.getContent(),
    };
  }
}
```

### 3. Response Chaining with Tools

```typescript
// response-chain.ts
import OpenAI from "openai";

interface Tool {
  type: "function";
  name: string;
  description: string;
  parameters: any;
}

interface FunctionCall {
  type: "function_call";
  name: string;
  call_id: string;
  arguments_json?: string;
}

export class ResponseChain {
  private client: OpenAI;
  private lastResponseId: string | null = null;
  private conversationHistory: any[] = [];
  private manualInputs: any[] = [];
  
  constructor(endpoint: string, apiKey: string) {
    this.client = new OpenAI({
      apiKey,
      baseURL: `${endpoint}/openai/v1`,
    });
  }
  
  // Method 1: Automatic chaining with previous_response_id
  async sendMessage(
    model: string,
    input: string | any[],
    tools?: Tool[],
    useManualChaining: boolean = false
  ) {
    const requestBody: any = {
      model,
      store: true,
    };
    
    if (useManualChaining) {
      // Method 2: Manual chaining by building input array
      if (typeof input === 'string') {
        this.manualInputs.push({
          type: "message",
          role: "user",
          content: input
        });
      } else {
        this.manualInputs.push(...input);
      }
      requestBody.input = this.manualInputs;
    } else {
      // Method 1: Use previous_response_id for automatic chaining
      requestBody.input = input;
      if (this.lastResponseId) {
        requestBody.previous_response_id = this.lastResponseId;
      }
    }
    
    // Add tools if provided
    if (tools) {
      requestBody.tools = tools;
    }
    
    const response = await this.client.responses.create(requestBody);
    
    // Update state for both methods
    this.lastResponseId = response.id;
    this.conversationHistory.push(response);
    
    // For manual chaining, append the response output
    if (useManualChaining && response.output) {
      this.manualInputs.push(...response.output);
    }
    
    // Check for function calls
    const functionCalls = this.extractFunctionCalls(response);
    if (functionCalls.length > 0) {
      return {
        response,
        functionCalls,
        requiresToolOutput: true,
      };
    }
    
    return {
      response,
      functionCalls: [],
      requiresToolOutput: false,
    };
  }
  
  async submitToolOutputs(
    model: string,
    toolOutputs: Array<{
      call_id: string;
      output: any;
    }>
  ) {
    const formattedOutputs = toolOutputs.map(output => ({
      type: "function_call_output",
      call_id: output.call_id,
      output: typeof output.output === "string" 
        ? output.output 
        : JSON.stringify(output.output),
    }));
    
    // Use automatic chaining for tool outputs
    return await this.sendMessage(model, formattedOutputs, undefined, false);
  }
  
  private extractFunctionCalls(response: any): FunctionCall[] {
    const calls: FunctionCall[] = [];
    
    if (response.output && Array.isArray(response.output)) {
      for (const item of response.output) {
        if (item.type === "function_call") {
          calls.push(item);
        }
      }
    }
    
    return calls;
  }
  
  getHistory() {
    return this.conversationHistory;
  }
  
  reset() {
    this.lastResponseId = null;
    this.conversationHistory = [];
  }
}
```

## Python Implementation

### Background Task Manager

```python
# background_manager.py
import os
import time
import asyncio
from typing import Optional, Dict, Any
from openai import OpenAI
from dataclasses import dataclass, field
from enum import Enum

class TaskStatus(Enum):
    QUEUED = "queued"
    IN_PROGRESS = "in_progress"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"

@dataclass
class BackgroundTask:
    id: str
    status: TaskStatus
    output_text: Optional[str] = None
    error: Optional[str] = None
    metadata: Dict[str, Any] = field(default_factory=dict)

class AzureBackgroundManager:
    def __init__(self, endpoint: str, api_key: str):
        self.client = OpenAI(
            api_key=api_key,
            base_url=f"{endpoint}/openai/v1"
        )
        self.active_tasks = {}
    
    async def execute_background(
        self,
        model: str,
        input_text: str,
        poll_interval: float = 2.0,
        max_wait: float = 600.0
    ) -> BackgroundTask:
        """Execute a background task with automatic polling."""
        
        # Start background task
        response = self.client.responses.create(
            model=model,
            input=input_text,
            background=True,
            store=True
        )
        
        task_id = response.id
        self.active_tasks[task_id] = BackgroundTask(
            id=task_id,
            status=TaskStatus(response.status)
        )
        
        # Exponential backoff parameters
        current_interval = poll_interval
        max_interval = 30.0  # Max 30 seconds
        backoff_multiplier = 1.5
        
        # Poll until completion
        start_time = time.time()
        while time.time() - start_time < max_wait:
            response = self.client.responses.retrieve(task_id)
            status = TaskStatus(response.status)
            
            if status == TaskStatus.COMPLETED:
                self.active_tasks[task_id].status = status
                self.active_tasks[task_id].output_text = response.output_text
                return self.active_tasks[task_id]
            
            elif status == TaskStatus.FAILED:
                self.active_tasks[task_id].status = status
                error_msg = getattr(response, 'error', 'Unknown error')
                if isinstance(error_msg, dict):
                    error_msg = error_msg.get('message', str(error_msg))
                self.active_tasks[task_id].error = error_msg
                raise Exception(f"Task failed: {error_msg}")
            
            # Exponential backoff with jitter
            import random
            jitter = random.random()  # 0-1 second jitter
            await asyncio.sleep(current_interval + jitter)
            
            # Increase interval for next iteration
            current_interval = min(current_interval * backoff_multiplier, max_interval)
        
        raise TimeoutError(f"Task {task_id} exceeded max wait time")
    
    def cancel_task(self, task_id: str):
        """Cancel a running background task."""
        # Use direct API call for cancel endpoint
        import requests
        
        url = f"{self.client.base_url}/responses/{task_id}/cancel"
        headers = {
            "Authorization": f"Bearer {self.client.api_key}",
            "Content-Type": "application/json"
        }
        
        response = requests.post(url, headers=headers)
        response.raise_for_status()
        
        if task_id in self.active_tasks:
            self.active_tasks[task_id].status = TaskStatus.CANCELLED
        
        return response.json()
    
    def get_task_status(self, task_id: str) -> Optional[BackgroundTask]:
        """Get current status of a task."""
        return self.active_tasks.get(task_id)
```

### Streaming with Resume

```python
# resumable_stream.py
import asyncio
import aiohttp
from typing import AsyncGenerator, Optional, Tuple
from dataclasses import dataclass

@dataclass
class StreamState:
    response_id: str
    cursor: int
    content: str

class ResumableStreamClient:
    def __init__(self, endpoint: str, api_key: str):
        self.endpoint = endpoint
        self.api_key = api_key
        self.current_state: Optional[StreamState] = None
    
    async def stream_with_resume(
        self,
        model: str,
        input_text: str,
        on_disconnect_retry: bool = True
    ) -> AsyncGenerator[str, None]:
        """Stream response with automatic resume on disconnect."""
        
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json"
        }
        
        # Initial request
        url = f"{self.endpoint}/openai/v1/responses"
        payload = {
            "model": model,
            "input": input_text,
            "background": True,
            "stream": True,
            "store": True
        }
        
        async with aiohttp.ClientSession() as session:
            try:
                async for chunk in self._stream_request(
                    session, url, headers, payload
                ):
                    yield chunk
            except aiohttp.ClientError as e:
                if on_disconnect_retry and self.current_state:
                    # Resume from last cursor
                    async for chunk in self._resume_stream(session, headers):
                        yield chunk
                else:
                    raise e
    
    async def _stream_request(
        self,
        session: aiohttp.ClientSession,
        url: str,
        headers: dict,
        payload: dict
    ) -> AsyncGenerator[str, None]:
        """Process a streaming request."""
        
        async with session.post(url, headers=headers, json=payload) as response:
            response.raise_for_status()
            
            buffer = ""
            async for line in response.content:
                line_str = line.decode('utf-8').strip()
                if not line_str:
                    continue
                
                if line_str.startswith("data: "):
                    data = line_str[6:]
                    if data == "[DONE]":
                        break
                    
                    try:
                        import json
                        event = json.loads(data)
                        
                        # Track state for resume
                        if event.get("type") == "response.created":
                            self.current_state = StreamState(
                                response_id=event["response"]["id"],
                                cursor=0,
                                content=""
                            )
                        
                        if "sequence_number" in event and self.current_state:
                            self.current_state.cursor = event["sequence_number"]
                        
                        if event.get("type") == "response.output_text.delta":
                            delta = event.get("delta", "")
                            if self.current_state:
                                self.current_state.content += delta
                            buffer += delta
                            yield delta
                    except json.JSONDecodeError:
                        continue
    
    async def _resume_stream(
        self,
        session: aiohttp.ClientSession,
        headers: dict
    ) -> AsyncGenerator[str, None]:
        """Resume streaming from last cursor position."""
        
        if not self.current_state:
            raise ValueError("No stream state to resume from")
        
        url = (
            f"{self.endpoint}/openai/v1/responses/{self.current_state.response_id}"
            f"?stream=true&starting_after={self.current_state.cursor}"
        )
        
        async with session.get(url, headers=headers) as response:
            response.raise_for_status()
            async for chunk in response.content:
                text = chunk.decode('utf-8')
                self.current_state.content += text
                yield text
```

## Rust Integration for Codex

### Background Task Support

```rust
// codex-rs/core/src/azure_background.rs
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Queued,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundResponse {
    pub id: String,
    pub status: ResponseStatus,
    pub output_text: Option<String>,
    pub error: Option<ResponseError>,
    pub usage: Option<Usage>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub message: String,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

pub struct AzureBackgroundClient {
    endpoint: String,
    api_key: String,
    client: reqwest::Client,
}

impl AzureBackgroundClient {
    pub fn new(endpoint: String, api_key: String) -> Self {
        Self {
            endpoint,
            api_key,
            client: reqwest::Client::new(),
        }
    }
    
    pub async fn create_background_task(
        &self,
        model: &str,
        input: &str,
    ) -> Result<BackgroundResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/openai/v1/responses", self.endpoint);
        
        let body = serde_json::json!({
            "model": model,
            "input": input,
            "background": true,
            "store": true,
        });
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;
        
        let background_response: BackgroundResponse = response.json().await?;
        Ok(background_response)
    }
    
    pub async fn poll_until_complete(
        &self,
        response_id: &str,
        poll_interval: Duration,
        max_attempts: u32,
    ) -> Result<BackgroundResponse, Box<dyn std::error::Error>> {
        let mut attempts = 0;
        let mut current_interval = poll_interval;
        let max_interval = Duration::from_secs(30);
        let backoff_multiplier = 1.5;
        
        loop {
            if attempts >= max_attempts {
                return Err("Max polling attempts exceeded".into());
            }
            
            let response = self.retrieve_response(response_id).await?;
            
            match response.status {
                ResponseStatus::Completed => return Ok(response),
                ResponseStatus::Failed => {
                    let error_msg = response.error
                        .map(|e| e.message)
                        .unwrap_or_else(|| "Unknown error".to_string());
                    return Err(format!("Task failed: {}", error_msg).into())
                },
                ResponseStatus::Cancelled => {
                    return Err("Task was cancelled".into())
                },
                _ => {
                    attempts += 1;
                    
                    // Exponential backoff with jitter
                    use rand::Rng;
                    let jitter = Duration::from_millis(rand::thread_rng().gen_range(0..1000));
                    sleep(current_interval + jitter).await;
                    
                    // Increase interval for next iteration
                    let new_interval = current_interval.as_secs_f64() * backoff_multiplier;
                    current_interval = Duration::from_secs_f64(new_interval.min(max_interval.as_secs_f64()));
                }
            }
        }
    }
    
    pub async fn retrieve_response(
        &self,
        response_id: &str,
    ) -> Result<BackgroundResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/openai/v1/responses/{}", self.endpoint, response_id);
        
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;
        
        let background_response: BackgroundResponse = response.json().await?;
        Ok(background_response)
    }
    
    pub async fn cancel_task(
        &self,
        response_id: &str,
    ) -> Result<BackgroundResponse, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/openai/v1/responses/{}/cancel", 
            self.endpoint, 
            response_id
        );
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;
        
        let background_response: BackgroundResponse = response.json().await?;
        Ok(background_response)
    }
}
```

### Stream Resume Support

```rust
// codex-rs/core/src/azure_stream_resume.rs
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub sequence_number: Option<u64>,
    pub delta: Option<String>,
    pub response: Option<ResponseMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct StreamState {
    pub response_id: Option<String>,
    pub last_cursor: Option<u64>,
    pub content: String,
}

pub struct ResumableStreamClient {
    endpoint: String,
    api_key: String,
    client: reqwest::Client,
    state: Arc<Mutex<StreamState>>,
}

impl ResumableStreamClient {
    pub fn new(endpoint: String, api_key: String) -> Self {
        Self {
            endpoint,
            api_key,
            client: reqwest::Client::new(),
            state: Arc::new(Mutex::new(StreamState {
                response_id: None,
                last_cursor: None,
                content: String::new(),
            })),
        }
    }
    
    pub async fn stream_with_resume(
        &self,
        model: &str,
        input: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/openai/v1/responses", self.endpoint);
        
        let body = serde_json::json!({
            "model": model,
            "input": input,
            "background": true,
            "stream": true,
            "store": true,
        });
        
        match self.start_stream(&url, body).await {
            Ok(_) => Ok(()),
            Err(e) => {
                eprintln!("Stream error: {}", e);
                // Attempt resume
                self.resume_stream().await
            }
        }
    }
    
    async fn start_stream(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let response = self.client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;
        
        let mut stream = response.bytes_stream();
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = String::from_utf8_lossy(&chunk);
            
            // Parse SSE events
            for line in text.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if data == "[DONE]" {
                        break;
                    }
                    
                    if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                        self.handle_event(event).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_event(
        &self,
        event: StreamEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.state.lock().await;
        
        // Update state based on event type
        match event.event_type.as_str() {
            "response.created" => {
                if let Some(response) = event.response {
                    state.response_id = Some(response.id);
                }
            },
            "response.output_text.delta" => {
                if let Some(delta) = event.delta {
                    state.content.push_str(&delta);
                    print!("{}", delta);
                }
            },
            _ => {}
        }
        
        // Update cursor
        if let Some(seq) = event.sequence_number {
            state.last_cursor = Some(seq);
        }
        
        Ok(())
    }
    
    async fn resume_stream(&self) -> Result<(), Box<dyn std::error::Error>> {
        let state = self.state.lock().await;
        
        let (response_id, cursor) = match (&state.response_id, state.last_cursor) {
            (Some(id), Some(cursor)) => (id.clone(), cursor),
            _ => return Err("Cannot resume: missing response ID or cursor".into()),
        };
        
        drop(state); // Release lock
        
        let url = format!(
            "{}/openai/v1/responses/{}?stream=true&starting_after={}",
            self.endpoint, response_id, cursor
        );
        
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;
        
        let mut stream = response.bytes_stream();
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = String::from_utf8_lossy(&chunk);
            
            let mut state = self.state.lock().await;
            state.content.push_str(&text);
            print!("{}", text);
        }
        
        Ok(())
    }
    
    pub async fn get_content(&self) -> String {
        let state = self.state.lock().await;
        state.content.clone()
    }
}
```

## Usage Examples

### TypeScript: Complete Background Workflow

```typescript
// example-usage.ts
async function completeWorkflow() {
  const manager = new BackgroundTaskManager(
    process.env.AZURE_OPENAI_ENDPOINT!,
    process.env.AZURE_OPENAI_API_KEY!
  );
  
  const chain = new ResponseChain(
    process.env.AZURE_OPENAI_ENDPOINT!,
    process.env.AZURE_OPENAI_API_KEY!
  );
  
  // 1. Execute background task
  const result = await manager.executeWithPolling(
    "gpt-4o",
    "Generate a comprehensive analysis of quantum computing applications",
    { pollInterval: 3000 }
  );
  
  console.log("Background task completed:", result.output_text);
  
  // 2. Chain responses with context
  const weatherTool = {
    type: "function" as const,
    name: "get_weather",
    description: "Get weather for a location",
    parameters: {
      type: "object",
      properties: {
        location: { type: "string" },
      },
      required: ["location"],
    },
  };
  
  const step1 = await chain.sendMessage(
    "gpt-4o",
    "What's the weather in Tokyo?",
    [weatherTool]
  );
  
  if (step1.requiresToolOutput) {
    // Execute function and submit results
    const toolOutputs = step1.functionCalls.map(call => ({
      call_id: call.call_id,
      output: { temperature: 22, condition: "sunny" },
    }));
    
    const step2 = await chain.submitToolOutputs("gpt-4o", toolOutputs);
    console.log("Final response:", step2.response.output_text);
  }
  
  // 3. Stream with resume
  const stream = new ResumableStream(
    process.env.AZURE_OPENAI_ENDPOINT!,
    process.env.AZURE_OPENAI_API_KEY!
  );
  
  stream.on("delta", (text) => process.stdout.write(text));
  stream.on("complete", (full) => console.log("\nStream complete"));
  stream.on("error", (err) => console.error("Stream error:", err));
  
  await stream.startStream(
    "gpt-4o",
    "Write a detailed tutorial on async programming"
  );
}
```

### Python: Async Background Tasks

```python
# example_usage.py
import os
import asyncio
from background_manager import AzureBackgroundManager
from resumable_stream import ResumableStreamClient

async def main():
    # Initialize clients
    manager = AzureBackgroundManager(
        endpoint=os.environ["AZURE_OPENAI_ENDPOINT"],
        api_key=os.environ["AZURE_OPENAI_API_KEY"]
    )
    
    stream_client = ResumableStreamClient(
        endpoint=os.environ["AZURE_OPENAI_ENDPOINT"],
        api_key=os.environ["AZURE_OPENAI_API_KEY"]
    )
    
    # Execute background task
    task = await manager.execute_background(
        model="gpt-4o",
        input_text="Analyze the impact of AI on healthcare",
        poll_interval=2.0
    )
    
    print(f"Task completed: {task.output_text[:200]}...")
    
    # Stream with resume capability
    async for chunk in stream_client.stream_with_resume(
        model="gpt-4o",
        input_text="Explain neural networks in detail"
    ):
        print(chunk, end="", flush=True)
    
    print("\nFinal content:", await stream_client.get_content())

if __name__ == "__main__":
    asyncio.run(main())
```

## Best Practices

1. **Always use `store: true`** for background tasks
2. **Implement exponential backoff** for polling to reduce API calls
3. **Store cursor positions** for stream resumption
4. **Handle all error states** (failed, cancelled, timeout)
5. **Use response chaining** to maintain conversation context
6. **Batch tool outputs** when submitting multiple function results
7. **Monitor task status** and implement proper cleanup
8. **Set reasonable timeouts** based on expected task duration

## Limitations

- Background mode requires persistent storage (`store: true`)
- Higher time-to-first-token (TTFT) in background streaming mode
- Cannot resume streams not created with `stream: true`
- Response data retained for 30 days by default
- Tool outputs must be submitted in the next request (not in-place)

## Testing Checklist

- [ ] Background task creation and polling
- [ ] Stream interruption and resume
- [ ] Response chaining with context
- [ ] Tool/function call handling
- [ ] Error recovery and retry logic
- [ ] Concurrent task management
- [ ] State persistence across disconnections