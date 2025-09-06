# Azure OpenAI Responses API - Complete Implementation Guide

Comprehensive guide combining Background Mode, Response Chaining, Parallel Tool Calling, and Structured Outputs.

## Table of Contents
1. [Setup & Authentication](#setup--authentication)
2. [Structured Outputs with Pydantic](#structured-outputs-with-pydantic)
3. [Parallel Tool Calling](#parallel-tool-calling)
4. [Background Mode with Chaining](#background-mode-with-chaining)
5. [Complete Production Implementation](#complete-production-implementation)

## Setup & Authentication

### Python Setup with Entra ID (Recommended)
```python
from openai import AzureOpenAI
from azure.identity import DefaultAzureCredential, get_bearer_token_provider
from pydantic import BaseModel, Field
from typing import List, Optional, Dict, Any
from enum import Enum
import asyncio
import json
from datetime import datetime

# Authentication
token_provider = get_bearer_token_provider(
    DefaultAzureCredential(), 
    "https://cognitiveservices.azure.com/.default"
)

client = AzureOpenAI(
    azure_endpoint="https://YOUR-RESOURCE-NAME.openai.azure.com",
    azure_ad_token_provider=token_provider,
    api_version="2024-08-01-preview",  # Required for structured outputs
)
```

### TypeScript Setup
```typescript
import OpenAI from "openai";
import { z } from "zod";

const client = new OpenAI({
  apiKey: process.env.AZURE_OPENAI_API_KEY,
  baseURL: "https://YOUR-RESOURCE-NAME.openai.azure.com/openai/v1/",
  defaultHeaders: {
    "api-version": "2024-08-01-preview"
  }
});
```

## Structured Outputs with Pydantic

### Define Response Schemas
```python
from pydantic import BaseModel, Field
from typing import List, Optional, Literal
from datetime import datetime

# Weather response schema
class WeatherInfo(BaseModel):
    location: str = Field(description="City name")
    temperature: float = Field(description="Temperature in celsius")
    condition: Literal["sunny", "cloudy", "rainy", "snowy"]
    humidity: int = Field(ge=0, le=100)
    wind_speed: float = Field(description="Wind speed in km/h")
    
class WeatherResponse(BaseModel):
    forecasts: List[WeatherInfo]
    generated_at: str = Field(default_factory=lambda: datetime.now().isoformat())
    units: Literal["metric", "imperial"] = "metric"

# Task execution schema
class TaskStep(BaseModel):
    step_number: int
    action: str
    description: str
    status: Literal["pending", "in_progress", "completed", "failed"]
    error_message: Optional[str] = None

class TaskExecutionPlan(BaseModel):
    task_name: str
    total_steps: int
    estimated_duration_minutes: int
    steps: List[TaskStep]
    dependencies: List[str] = Field(default_factory=list)
    
# Analysis result schema  
class AnalysisResult(BaseModel):
    summary: str = Field(max_length=500)
    key_findings: List[str] = Field(max_items=10)
    confidence_score: float = Field(ge=0, le=1)
    recommendations: List[str]
    data_quality: Literal["high", "medium", "low"]
    metadata: Dict[str, Any] = Field(default_factory=dict)
```

## Parallel Tool Calling

### Define Multiple Tools
```python
# Tool definitions
tools = [
    {
        "type": "function",
        "name": "get_weather",
        "description": "Get current weather for multiple locations",
        "parameters": {
            "type": "object",
            "properties": {
                "locations": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of city names"
                },
                "units": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"],
                    "default": "celsius"
                }
            },
            "required": ["locations"],
            "additionalProperties": False
        }
    },
    {
        "type": "function",
        "name": "search_flights",
        "description": "Search for flights between cities",
        "parameters": {
            "type": "object",
            "properties": {
                "origin": {"type": "string"},
                "destination": {"type": "string"},
                "date": {"type": "string", "format": "date"},
                "return_date": {"type": "string", "format": "date"},
                "passengers": {"type": "integer", "minimum": 1}
            },
            "required": ["origin", "destination", "date"],
            "additionalProperties": False
        }
    },
    {
        "type": "function",
        "name": "book_hotel",
        "description": "Book hotel accommodations",
        "parameters": {
            "type": "object",
            "properties": {
                "city": {"type": "string"},
                "check_in": {"type": "string", "format": "date"},
                "check_out": {"type": "string", "format": "date"},
                "guests": {"type": "integer", "minimum": 1},
                "room_type": {
                    "type": "string",
                    "enum": ["single", "double", "suite"]
                }
            },
            "required": ["city", "check_in", "check_out"],
            "additionalProperties": False
        }
    }
]

# Tool implementations
async def get_weather(locations: List[str], units: str = "celsius") -> WeatherResponse:
    """Simulate weather API calls"""
    forecasts = []
    for location in locations:
        # Simulate API call
        await asyncio.sleep(0.1)
        forecasts.append(WeatherInfo(
            location=location,
            temperature=22.5 if units == "celsius" else 72.5,
            condition="sunny",
            humidity=65,
            wind_speed=12.3
        ))
    return WeatherResponse(forecasts=forecasts, units="metric" if units == "celsius" else "imperial")

async def search_flights(origin: str, destination: str, date: str, **kwargs):
    """Simulate flight search"""
    await asyncio.sleep(0.2)
    return {
        "flights": [
            {"flight_number": "AA123", "price": 450, "duration": "3h 45m"},
            {"flight_number": "UA456", "price": 425, "duration": "4h 10m"}
        ]
    }

async def book_hotel(city: str, check_in: str, check_out: str, **kwargs):
    """Simulate hotel booking"""
    await asyncio.sleep(0.15)
    return {
        "confirmation_number": "HTL-" + city.upper()[:3] + "-12345",
        "hotel_name": f"{city} Grand Hotel",
        "total_price": 750
    }

# Tool executor with parallel processing
async def execute_tool_calls(tool_calls):
    """Execute multiple tool calls in parallel"""
    tasks = []
    
    for tool_call in tool_calls:
        function_name = tool_call.function.name if hasattr(tool_call, 'function') else tool_call.get('name')
        function_args = json.loads(
            tool_call.function.arguments if hasattr(tool_call, 'function') 
            else tool_call.get('arguments_json', '{}')
        )
        
        # Map function names to implementations
        if function_name == "get_weather":
            task = get_weather(**function_args)
        elif function_name == "search_flights":
            task = search_flights(**function_args)
        elif function_name == "book_hotel":
            task = book_hotel(**function_args)
        else:
            continue
            
        tasks.append({
            "task": task,
            "tool_call": tool_call,
            "function_name": function_name
        })
    
    # Execute all tasks in parallel using asyncio.gather
    task_coroutines = [task_info["task"] for task_info in tasks]
    results_data = await asyncio.gather(*task_coroutines)
    
    # Format results
    results = []
    for task_info, result in zip(tasks, results_data):
        results.append({
            "tool_call_id": task_info["tool_call"].id if hasattr(task_info["tool_call"], 'id') 
                           else task_info["tool_call"].get('call_id'),
            "function_name": task_info["function_name"],
            "output": result.model_dump_json() if isinstance(result, BaseModel) else json.dumps(result)
        })
    
    return results
```

## Background Mode with Chaining

### Advanced Background Task Manager
```python
class AdvancedBackgroundManager:
    def __init__(self, client: AzureOpenAI):
        self.client = client
        self.active_tasks = {}
        self.response_chains = {}
    
    def _format_response_schema(self, schema: type[BaseModel]) -> dict:
        """Convert Pydantic model to proper response format"""
        if not schema:
            return None
        return {
            "type": "json_schema",
            "json_schema": {
                "name": schema.__name__,
                "strict": True,
                "schema": schema.model_json_schema()
            }
        }
        
    async def create_background_chain(
        self,
        model: str,
        initial_input: str,
        tools: Optional[List] = None,
        response_format: Optional[type[BaseModel]] = None,
        max_steps: int = 5
    ):
        """Create a background task with response chaining capability"""
        
        # Start initial background task
        initial_response = await self.client.responses.create(
            model=model,
            input=initial_input,
            tools=tools,
            background=True,
            stream=False,
            store=True,
            parallel_tool_calls=True if tools else None,
            response_format=self._format_response_schema(response_format) if response_format else None
        )
        
        chain_id = f"chain_{initial_response.id}"
        self.response_chains[chain_id] = {
            "responses": [initial_response.id],
            "status": "active",
            "created_at": datetime.now(),
            "max_steps": max_steps,
            "current_step": 1
        }
        
        return chain_id, initial_response
    
    async def continue_chain(
        self,
        chain_id: str,
        user_input: str,
        tools: Optional[List] = None,
        response_format: Optional[type[BaseModel]] = None
    ):
        """Continue a response chain with a new message"""
        
        if chain_id not in self.response_chains:
            raise ValueError(f"Chain {chain_id} not found")
        
        chain = self.response_chains[chain_id]
        if chain["current_step"] >= chain["max_steps"]:
            raise ValueError(f"Chain {chain_id} has reached maximum steps")
        
        # Get the last response ID
        last_response_id = chain["responses"][-1]
        
        # Create chained response
        new_response = await self.client.responses.create(
            model="gpt-4o",
            previous_response_id=last_response_id,
            input=[{"role": "user", "content": user_input}],
            tools=tools,
            background=True,
            store=True,
            parallel_tool_calls=True if tools else None,
            response_format=self._format_response_schema(response_format) if response_format else None
        )
        
        chain["responses"].append(new_response.id)
        chain["current_step"] += 1
        
        return new_response
    
    async def poll_with_retry(
        self,
        response_id: str,
        max_attempts: int = 100,
        initial_interval: float = 2.0
    ):
        """Poll a background task with exponential backoff"""
        
        current_interval = initial_interval
        max_interval = 30.0
        backoff_multiplier = 1.5
        
        for attempt in range(max_attempts):
            try:
                response = self.client.responses.retrieve(response_id)
                
                if response.status == "completed":
                    return response
                elif response.status == "failed":
                    raise Exception(f"Task failed: {response.error}")
                elif response.status == "cancelled":
                    raise Exception("Task was cancelled")
                
                # Exponential backoff with jitter
                import random
                jitter = random.random()
                await asyncio.sleep(current_interval + jitter)
                current_interval = min(current_interval * backoff_multiplier, max_interval)
                
            except Exception as e:
                if attempt == max_attempts - 1:
                    raise
                await asyncio.sleep(current_interval)
        
        raise TimeoutError(f"Response {response_id} polling timeout")
```

## Complete Production Implementation

### Full-Featured Response Handler
```python
class AzureResponseHandler:
    """Production-ready handler for Azure OpenAI Responses API"""
    
    def __init__(self, client: AzureOpenAI):
        self.client = client
        self.background_manager = AdvancedBackgroundManager(client)
    
    def _format_response_schema(self, schema: type[BaseModel]) -> dict:
        """Convert Pydantic model to proper response format"""
        if not schema:
            return None
        return {
            "type": "json_schema",
            "json_schema": {
                "name": schema.__name__,
                "strict": True,
                "schema": schema.model_json_schema()
            }
        }
    
    def _get_auth_token(self) -> str:
        """Get authentication token from client"""
        if hasattr(self.client, '_azure_ad_token_provider'):
            return self.client._azure_ad_token_provider()
        return ""
        
    async def process_with_tools_and_structured_output(
        self,
        model: str,
        user_message: str,
        tools: List[Dict],
        output_schema: type[BaseModel],
        use_background: bool = False,
        enable_chaining: bool = True,
        previous_response_id: Optional[str] = None
    ):
        """
        Process a request with parallel tool calling and structured output
        """
        
        # Build request parameters
        request_params = {
            "model": model,
            "tools": tools,
            "parallel_tool_calls": True,
            "response_format": self._format_response_schema(output_schema) if output_schema else None,
            "store": enable_chaining,
        }
        
        # Add input based on chaining preference
        if previous_response_id and enable_chaining:
            request_params["previous_response_id"] = previous_response_id
            request_params["input"] = [{"role": "user", "content": user_message}]
        else:
            request_params["input"] = user_message
        
        # Add background mode if requested
        if use_background:
            request_params["background"] = True
        
        # Create initial response
        response = self.client.responses.create(**request_params)
        
        # If background, poll for completion
        if use_background:
            response = await self.background_manager.poll_with_retry(response.id)
        
        # Process tool calls if present
        if hasattr(response, 'output') and response.output:
            tool_results = await self._process_tool_calls(response.output)
            
            if tool_results:
                # Submit tool outputs
                tool_response = self.client.responses.create(
                    model=model,
                    previous_response_id=response.id,
                    input=tool_results,
                    response_format=output_schema,
                    store=enable_chaining,
                    background=use_background
                )
                
                if use_background:
                    tool_response = await self.background_manager.poll_with_retry(tool_response.id)
                
                return tool_response
        
        return response
    
    async def _process_tool_calls(self, output_items):
        """Process and execute tool calls from response output"""
        
        tool_calls = []
        for item in output_items:
            if hasattr(item, 'type') and item.type == "function_call":
                tool_calls.append(item)
            elif isinstance(item, dict) and item.get('type') == "function_call":
                tool_calls.append(item)
        
        if not tool_calls:
            return None
        
        # Execute tools in parallel
        results = await execute_tool_calls(tool_calls)
        
        # Format for submission
        formatted_results = []
        for result in results:
            formatted_results.append({
                "type": "function_call_output",
                "call_id": result["tool_call_id"],
                "output": result["output"]
            })
        
        return formatted_results
    
    async def streaming_with_resume(
        self,
        model: str,
        input_text: str,
        tools: Optional[List] = None,
        response_format: Optional[type[BaseModel]] = None
    ):
        """Stream response with resume capability"""
        
        response_id = None
        last_cursor = None
        full_content = []
        
        try:
            # Start streaming
            stream = await self.client.responses.create(
                model=model,
                input=input_text,
                tools=tools,
                stream=True,
                background=True,
                store=True,
                response_format=self._format_response_schema(response_format) if response_format else None
            )
            
            # Handle streaming properly
            for event in stream:
                # Track response ID and cursor
                if hasattr(event, 'response') and hasattr(event.response, 'id'):
                    response_id = event.response.id
                
                if hasattr(event, 'sequence_number'):
                    last_cursor = event.sequence_number
                
                # Process event
                if hasattr(event, 'type'):
                    if event.type == 'response.output_text.delta':
                        full_content.append(event.delta)
                        yield event.delta
                    elif event.type == 'response.function_call':
                        # Handle function calls during streaming
                        yield {"type": "function_call", "data": event}
                    elif event.type == 'response.completed':
                        yield {"type": "completed", "content": "".join(full_content)}
                        
        except Exception as e:
            # Attempt to resume if we have necessary info
            if response_id and last_cursor is not None:
                async for chunk in self._resume_stream(response_id, last_cursor):
                    yield chunk
            else:
                raise e
    
    async def _resume_stream(self, response_id: str, starting_cursor: int):
        """Resume a interrupted stream"""
        
        import aiohttp
        
        # Extract endpoint and API key properly
        endpoint = self.client.azure_endpoint if hasattr(self.client, 'azure_endpoint') else self.client.base_url
        api_key = self.client.api_key if hasattr(self.client, 'api_key') else None
        
        url = (
            f"{endpoint}/openai/v1/responses/{response_id}"
            f"?stream=true&starting_after={starting_cursor}"
        )
        
        headers = {
            "Content-Type": "application/json"
        }
        
        # Add appropriate authorization
        if api_key:
            headers["api-key"] = api_key
        else:
            # Use token provider if available
            headers["Authorization"] = "Bearer " + self._get_auth_token()
        
        async with aiohttp.ClientSession() as session:
            async with session.get(url, headers=headers) as response:
                async for line in response.content:
                    line_str = line.decode('utf-8').strip()
                    if line_str.startswith('data: '):
                        data = line_str[6:]
                        if data != '[DONE]':
                            try:
                                event = json.loads(data)
                                if event.get('type') == 'response.output_text.delta':
                                    yield event.get('delta', '')
                            except json.JSONDecodeError:
                                continue
```

### Example Usage: Travel Planning Assistant
```python
async def travel_planning_example():
    """
    Complete example combining all features:
    - Structured outputs
    - Parallel tool calling
    - Background processing
    - Response chaining
    """
    
    handler = AzureResponseHandler(client)
    
    # Define structured output for travel plan
    class TravelItinerary(BaseModel):
        destination: str
        duration_days: int
        flights: List[Dict[str, Any]]
        hotels: List[Dict[str, Any]]
        activities: List[str]
        total_estimated_cost: float
        weather_forecast: Optional[WeatherResponse] = None
    
    # Step 1: Initial planning request with tools
    print("Planning your trip to Tokyo...")
    
    initial_response = await handler.process_with_tools_and_structured_output(
        model="gpt-4o",
        user_message="Plan a 5-day trip to Tokyo for 2 people in April. Include flights from San Francisco, hotels, and weather information.",
        tools=tools,
        output_schema=TravelItinerary,
        use_background=True,
        enable_chaining=True
    )
    
    print(f"Initial plan created: {initial_response.id}")
    
    # Step 2: Refine with additional requirements (chained)
    refined_response = await handler.process_with_tools_and_structured_output(
        model="gpt-4o",
        user_message="Please adjust the plan to include more cultural activities and find budget-friendly options. Keep total cost under $3000.",
        tools=tools,
        output_schema=TravelItinerary,
        use_background=True,
        enable_chaining=True,
        previous_response_id=initial_response.id
    )
    
    # Parse structured output
    if refined_response.output_text:
        try:
            itinerary = TravelItinerary.model_validate_json(refined_response.output_text)
            print(f"\nFinal Itinerary:")
            print(f"- Destination: {itinerary.destination}")
            print(f"- Duration: {itinerary.duration_days} days")
            print(f"- Estimated Cost: ${itinerary.total_estimated_cost}")
            print(f"- Activities: {', '.join(itinerary.activities[:3])}...")
        except Exception as e:
            print(f"Error parsing response: {e}")
    
    # Step 3: Stream detailed recommendations
    print("\nStreaming detailed recommendations...")
    
    async for chunk in handler.streaming_with_resume(
        model="gpt-4o",
        input_text="Provide detailed day-by-day recommendations for the Tokyo trip, including restaurant suggestions and transportation tips.",
        response_format=None  # No structured output for streaming
    ):
        if isinstance(chunk, str):
            print(chunk, end='', flush=True)
        elif isinstance(chunk, dict) and chunk.get('type') == 'completed':
            print("\n\nRecommendations complete!")

# Run the example
if __name__ == "__main__":
    asyncio.run(travel_planning_example())
```

## TypeScript Implementation

```typescript
// azure-responses-complete.ts
import OpenAI from "openai";
import { z } from "zod";

// Structured output schemas using Zod
const WeatherInfoSchema = z.object({
  location: z.string(),
  temperature: z.number(),
  condition: z.enum(["sunny", "cloudy", "rainy", "snowy"]),
  humidity: z.number().min(0).max(100),
  windSpeed: z.number()
});

const TaskExecutionSchema = z.object({
  taskName: z.string(),
  totalSteps: z.number(),
  estimatedDuration: z.number(),
  steps: z.array(z.object({
    stepNumber: z.number(),
    action: z.string(),
    status: z.enum(["pending", "in_progress", "completed", "failed"]),
    errorMessage: z.string().optional()
  }))
});

// Advanced Response Handler
class AzureResponseHandler {
  private client: OpenAI;
  private responseChains: Map<string, any> = new Map();
  
  constructor(endpoint: string, apiKey: string) {
    this.client = new OpenAI({
      apiKey,
      baseURL: `${endpoint}/openai/v1/`,
      defaultHeaders: {
        "api-version": "2024-08-01-preview"
      }
    });
  }
  
  async processWithToolsAndStructuredOutput<T>(
    model: string,
    userMessage: string,
    tools: any[],
    outputSchema?: z.ZodType<T>,
    options: {
      useBackground?: boolean;
      enableChaining?: boolean;
      previousResponseId?: string;
    } = {}
  ) {
    const requestParams: any = {
      model,
      tools,
      parallel_tool_calls: true,
      store: options.enableChaining ?? true,
    };
    
    // Add structured output if provided
    if (outputSchema) {
      requestParams.response_format = {
        type: "json_schema",
        json_schema: {
          name: "response",
          strict: true,
          schema: zodToJsonSchema(outputSchema)
        }
      };
    }
    
    // Handle chaining
    if (options.previousResponseId && options.enableChaining) {
      requestParams.previous_response_id = options.previousResponseId;
      requestParams.input = [{ role: "user", content: userMessage }];
    } else {
      requestParams.input = userMessage;
    }
    
    // Add background mode
    if (options.useBackground) {
      requestParams.background = true;
    }
    
    // Create response
    let response = await this.client.responses.create(requestParams);
    
    // Poll if background
    if (options.useBackground) {
      response = await this.pollWithRetry(response.id);
    }
    
    // Process tool calls
    if (response.output && Array.isArray(response.output)) {
      const toolResults = await this.processToolCalls(response.output);
      
      if (toolResults.length > 0) {
        const toolResponse = await this.client.responses.create({
          model,
          previous_response_id: response.id,
          input: toolResults,
          store: options.enableChaining ?? true,
          background: options.useBackground
        });
        
        if (options.useBackground) {
          return await this.pollWithRetry(toolResponse.id);
        }
        
        return toolResponse;
      }
    }
    
    // Parse structured output if schema provided
    if (outputSchema && response.output_text) {
      try {
        const parsed = JSON.parse(response.output_text);
        return {
          ...response,
          parsed: outputSchema.parse(parsed)
        };
      } catch (e) {
        console.error("Failed to parse structured output:", e);
      }
    }
    
    return response;
  }
  
  private async pollWithRetry(
    responseId: string,
    maxAttempts: number = 100
  ) {
    let currentInterval = 2000;
    const maxInterval = 30000;
    const backoffMultiplier = 1.5;
    
    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      const response = await this.client.responses.retrieve(responseId);
      
      if (response.status === "completed") {
        return response;
      } else if (response.status === "failed") {
        throw new Error(`Task failed: ${response.error}`);
      } else if (response.status === "cancelled") {
        throw new Error("Task was cancelled");
      }
      
      // Exponential backoff with jitter
      const jitter = Math.random() * 1000;
      await new Promise(resolve => setTimeout(resolve, currentInterval + jitter));
      currentInterval = Math.min(currentInterval * backoffMultiplier, maxInterval);
    }
    
    throw new Error(`Polling timeout for response ${responseId}`);
  }
  
  private async processToolCalls(output: any[]) {
    const toolCalls = output.filter(
      item => item.type === "function_call"
    );
    
    if (toolCalls.length === 0) return [];
    
    // Execute tools in parallel
    const results = await Promise.all(
      toolCalls.map(async (call) => {
        const result = await this.executeToolCall(call);
        return {
          type: "function_call_output",
          call_id: call.call_id,
          output: JSON.stringify(result)
        };
      })
    );
    
    return results;
  }
  
  private async executeToolCall(call: any) {
    const args = JSON.parse(call.arguments_json || "{}");
    
    // Implement your tool functions here
    switch (call.name) {
      case "get_weather":
        return this.getWeather(args);
      case "search_flights":
        return this.searchFlights(args);
      case "book_hotel":
        return this.bookHotel(args);
      default:
        throw new Error(`Unknown tool: ${call.name}`);
    }
  }
  
  private async getWeather(args: any) {
    // Simulate weather API
    await new Promise(resolve => setTimeout(resolve, 100));
    return {
      location: args.locations[0],
      temperature: 22.5,
      condition: "sunny",
      humidity: 65,
      windSpeed: 12.3
    };
  }
  
  private async searchFlights(args: any) {
    // Simulate flight search
    await new Promise(resolve => setTimeout(resolve, 200));
    return {
      flights: [
        { flightNumber: "AA123", price: 450, duration: "3h 45m" },
        { flightNumber: "UA456", price: 425, duration: "4h 10m" }
      ]
    };
  }
  
  private async bookHotel(args: any) {
    // Simulate hotel booking
    await new Promise(resolve => setTimeout(resolve, 150));
    return {
      confirmationNumber: `HTL-${args.city.toUpperCase().slice(0, 3)}-12345`,
      hotelName: `${args.city} Grand Hotel`,
      totalPrice: 750
    };
  }
}

// Helper function to convert Zod schema to JSON Schema
function zodToJsonSchema(schema: z.ZodType<any>): any {
  // Simplified conversion - use a library like zod-to-json-schema in production
  return {
    type: "object",
    properties: {},
    required: [],
    additionalProperties: false
  };
}

// Usage example
async function main() {
  const handler = new AzureResponseHandler(
    process.env.AZURE_OPENAI_ENDPOINT!,
    process.env.AZURE_OPENAI_API_KEY!
  );
  
  // Example: Complex travel planning with all features
  const response = await handler.processWithToolsAndStructuredOutput(
    "gpt-4o",
    "Plan a 5-day trip to Tokyo with flights, hotels, and weather info",
    [
      /* tool definitions */
    ],
    TaskExecutionSchema,
    {
      useBackground: true,
      enableChaining: true
    }
  );
  
  console.log("Response:", response);
}

export { AzureResponseHandler, WeatherInfoSchema, TaskExecutionSchema };
```

## Best Practices

### 1. Error Handling
```python
class RobustResponseHandler:
    async def safe_execute(self, func, *args, **kwargs):
        """Execute with retry and error handling"""
        max_retries = 3
        retry_delay = 1.0
        
        for attempt in range(max_retries):
            try:
                return await func(*args, **kwargs)
            except Exception as e:
                if attempt == max_retries - 1:
                    raise
                
                # Log error
                print(f"Attempt {attempt + 1} failed: {e}")
                
                # Exponential backoff
                await asyncio.sleep(retry_delay * (2 ** attempt))
        
        raise Exception("Max retries exceeded")
```

### 2. Security for Tool Calling
```python
class SecureToolExecutor:
    def __init__(self, allowed_tools: Set[str], require_confirmation: bool = False):
        self.allowed_tools = allowed_tools
        self.require_confirmation = require_confirmation
    
    async def execute_tool(self, tool_name: str, args: Dict):
        """Execute tool with security checks"""
        
        # Check if tool is allowed
        if tool_name not in self.allowed_tools:
            raise PermissionError(f"Tool {tool_name} is not allowed")
        
        # Validate arguments
        self._validate_args(tool_name, args)
        
        # Require user confirmation for sensitive operations
        if self.require_confirmation and self._is_sensitive(tool_name):
            if not await self._get_user_confirmation(tool_name, args):
                raise PermissionError("User declined tool execution")
        
        # Execute with sandboxing
        return await self._sandboxed_execute(tool_name, args)
    
    def _validate_args(self, tool_name: str, args: Dict):
        """Validate tool arguments"""
        # Implement validation logic
        pass
    
    def _is_sensitive(self, tool_name: str) -> bool:
        """Check if tool performs sensitive operations"""
        sensitive_tools = {"book_hotel", "make_payment", "delete_data"}
        return tool_name in sensitive_tools
```

### 3. Monitoring and Logging
```python
import logging
from datetime import datetime

class MonitoredResponseHandler:
    def __init__(self, client, logger=None):
        self.client = client
        self.logger = logger or logging.getLogger(__name__)
        self.metrics = {
            "total_requests": 0,
            "successful_requests": 0,
            "failed_requests": 0,
            "total_tokens": 0,
            "average_latency": 0
        }
    
    async def create_response_with_monitoring(self, **kwargs):
        """Create response with monitoring"""
        start_time = datetime.now()
        
        try:
            response = await self.client.responses.create(**kwargs)
            
            # Update metrics
            self.metrics["total_requests"] += 1
            self.metrics["successful_requests"] += 1
            
            if hasattr(response, 'usage'):
                self.metrics["total_tokens"] += response.usage.total_tokens
            
            # Log success
            self.logger.info(f"Response created: {response.id}")
            
            return response
            
        except Exception as e:
            self.metrics["failed_requests"] += 1
            self.logger.error(f"Response creation failed: {e}")
            raise
        
        finally:
            # Calculate latency
            latency = (datetime.now() - start_time).total_seconds()
            self._update_average_latency(latency)
    
    def _update_average_latency(self, new_latency: float):
        """Update rolling average latency"""
        n = self.metrics["total_requests"]
        if n == 0:
            self.metrics["average_latency"] = new_latency
        else:
            avg = self.metrics["average_latency"]
            self.metrics["average_latency"] = (avg * (n - 1) + new_latency) / n
```

## Summary

This comprehensive implementation combines:

1. **Structured Outputs**: Type-safe responses with Pydantic/Zod schemas
2. **Parallel Tool Calling**: Execute multiple functions simultaneously
3. **Background Mode**: Long-running tasks with polling and retry
4. **Response Chaining**: Maintain conversation context across requests
5. **Error Handling**: Robust retry logic and error recovery
6. **Security**: Tool execution validation and sandboxing
7. **Monitoring**: Metrics collection and logging

Key advantages:
- Type safety with structured outputs
- Improved performance with parallel tool execution
- Reliability with background mode and retry logic
- Context preservation with response chaining
- Production-ready error handling and monitoring