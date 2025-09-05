// Azure OpenAI Responses API - Response Chaining Examples
// Based on official Microsoft documentation

import OpenAI from "openai";

// Initialize client
const client = new OpenAI({
  apiKey: process.env.AZURE_OPENAI_API_KEY!,
  baseURL: `${process.env.AZURE_OPENAI_ENDPOINT}/openai/v1/`,
});

const model = process.env.AZURE_OPENAI_DEPLOYMENT || "gpt-4o";

// ============================================
// Method 1: Automatic Chaining with previous_response_id
// ============================================
async function automaticChaining() {
  console.log("=== Method 1: Automatic Chaining ===\n");
  
  // First response
  const response = await client.responses.create({
    model,
    input: "Define and explain the concept of catastrophic forgetting?",
    store: true, // Required for chaining
  });
  
  console.log("First response:", response.output_text?.substring(0, 200) + "...\n");
  
  // Second response - automatically includes context from first
  const secondResponse = await client.responses.create({
    model,
    previous_response_id: response.id, // This links to the previous response
    input: [{
      role: "user",
      content: "Explain this at a level that could be understood by a college freshman"
    }],
    store: true,
  });
  
  console.log("Second response (with context):", secondResponse.output_text);
  
  return { firstId: response.id, secondId: secondResponse.id };
}

// ============================================
// Method 2: Manual Chaining by Building Input Array
// ============================================
async function manualChaining() {
  console.log("\n=== Method 2: Manual Chaining ===\n");
  
  // Initialize conversation with first user message
  let inputs: any[] = [{
    type: "message",
    role: "user",
    content: "Define and explain the concept of catastrophic forgetting?"
  }];
  
  // First response
  const response = await client.responses.create({
    model,
    input: inputs,
    store: false, // Can be false since we're managing state manually
  });
  
  console.log("First response:", response.output_text?.substring(0, 200) + "...\n");
  
  // Manually append the model's response to inputs
  inputs = [...inputs, ...response.output];
  
  // Add next user message
  inputs.push({
    role: "user",
    type: "message",
    content: "Explain this at a level that could be understood by a college freshman"
  });
  
  // Second response with full conversation context
  const secondResponse = await client.responses.create({
    model,
    input: inputs, // Contains full conversation history
    store: false,
  });
  
  console.log("Second response (with manual context):", secondResponse.output_text);
  
  return { inputs, responses: [response, secondResponse] };
}

// ============================================
// Advanced: Chaining with Function Calling
// ============================================
async function chainingWithFunctions() {
  console.log("\n=== Chaining with Function Calls ===\n");
  
  // Define a weather function
  const weatherTool = {
    type: "function" as const,
    name: "get_weather",
    description: "Get the current weather for a location",
    parameters: {
      type: "object",
      properties: {
        location: { type: "string", description: "City name" },
        unit: { type: "string", enum: ["celsius", "fahrenheit"] }
      },
      required: ["location"],
    },
  };
  
  // First request with function
  const response = await client.responses.create({
    model,
    tools: [weatherTool],
    input: [{
      role: "user",
      content: "What's the weather like in San Francisco and Tokyo?"
    }],
    store: true,
  });
  
  console.log("Function calls requested:", response.output);
  
  // Process function calls
  const toolOutputs: any[] = [];
  for (const output of response.output || []) {
    if (output.type === "function_call" && output.name === "get_weather") {
      const args = JSON.parse(output.arguments_json || "{}");
      console.log(`Calling get_weather for: ${args.location}`);
      
      // Simulate weather API call
      const weatherData = {
        location: args.location,
        temperature: args.location === "San Francisco" ? 65 : 72,
        condition: "sunny",
        unit: args.unit || "fahrenheit"
      };
      
      toolOutputs.push({
        type: "function_call_output",
        call_id: output.call_id,
        output: JSON.stringify(weatherData),
      });
    }
  }
  
  // Submit function outputs using previous_response_id for chaining
  const finalResponse = await client.responses.create({
    model,
    previous_response_id: response.id, // Chain to maintain context
    input: toolOutputs,
    store: true,
  });
  
  console.log("\nFinal response with weather data:", finalResponse.output_text);
  
  return { initialResponse: response, finalResponse };
}

// ============================================
// Comparison: Both Methods Side by Side
// ============================================
async function compareChainMethods() {
  console.log("\n=== Comparing Both Chaining Methods ===\n");
  
  const testInput = "What is machine learning?";
  const followUp = "Give me a practical example";
  
  // Method 1: Automatic
  console.log("Automatic Chaining:");
  const auto1 = await client.responses.create({
    model,
    input: testInput,
    store: true,
  });
  
  const auto2 = await client.responses.create({
    model,
    previous_response_id: auto1.id,
    input: [{ role: "user", content: followUp }],
    store: true,
  });
  
  console.log("- Uses previous_response_id:", auto1.id);
  console.log("- Server manages context automatically");
  console.log("- Response:", auto2.output_text?.substring(0, 100) + "...\n");
  
  // Method 2: Manual
  console.log("Manual Chaining:");
  let manualInputs = [{ type: "message", role: "user", content: testInput }];
  
  const manual1 = await client.responses.create({
    model,
    input: manualInputs,
  });
  
  manualInputs = [
    ...manualInputs,
    ...manual1.output,
    { role: "user", type: "message", content: followUp }
  ];
  
  const manual2 = await client.responses.create({
    model,
    input: manualInputs,
  });
  
  console.log("- Manually builds input array");
  console.log("- Client manages context");
  console.log("- Response:", manual2.output_text?.substring(0, 100) + "...");
}

// ============================================
// Helper: Clean Response Output for Display
// ============================================
function cleanOutput(response: any): any {
  return {
    id: response.id,
    status: response.status,
    model: response.model,
    output_text: response.output_text?.substring(0, 150) + "...",
    usage: response.usage,
    previous_response_id: response.previous_response_id,
  };
}

// ============================================
// Main Execution
// ============================================
async function main() {
  try {
    console.log("Azure OpenAI Responses API - Chaining Examples");
    console.log("=" .repeat(50) + "\n");
    
    // Run examples
    await automaticChaining();
    await manualChaining();
    await chainingWithFunctions();
    await compareChainMethods();
    
    console.log("\n" + "=".repeat(50));
    console.log("Examples completed successfully!");
    
  } catch (error) {
    console.error("Error:", error);
  }
}

// Run if executed directly
if (require.main === module) {
  main();
}

// Export for use in other modules
export {
  automaticChaining,
  manualChaining,
  chainingWithFunctions,
  compareChainMethods,
  cleanOutput
};