Based on my deep analysis, here's a refined implementation plan with specific
  details and edge cases:

  Refined Azure OpenAI Responses API Implementation Plan

  Critical Insights from Analysis

  The codebase is already 70% ready for Azure integration. Key findings:
  - ✅ Provider framework supports Azure requirements (query params, headers,
  custom URLs)
  - ✅ Test case already demonstrates Azure configuration works
  - ❌ Authentication uses wrong header format for Azure
  - ❌ No response chaining support (critical for Azure Responses API)
  - ❌ Missing background task polling infrastructure
  - ❌ No built-in Azure provider definitions

  Phase 1: Authentication Fix (Immediate Priority)

  File: codex-rs/core/src/model_provider_info.rs

  // Current bug at line 155:
  headers.insert("Authorization", format!("Bearer {}", api_key));

  // Need to add provider type detection:
  enum AuthType {
      Bearer,      // Standard OpenAI
      ApiKey,      // Azure OpenAI
  }

  // Modify create_request_builder() to:
  match self.auth_type() {
      AuthType::ApiKey => headers.insert("api-key", api_key),
      AuthType::Bearer => headers.insert("Authorization", format!("Bearer {}",
  api_key)),
  }

  Edge Cases:
  - Azure AD tokens still use Bearer format
  - Must detect based on provider name or config flag
  - Fallback to Bearer if uncertain

  Phase 2: Response Chaining Implementation

  Files to modify:
  - codex-rs/core/src/wire_api/responses.rs - Add previous_response_id field
  - codex-rs/core/src/conversation_manager.rs - Track response IDs

  New Fields Required:
  pub struct CreateResponsesRequest {
      pub previous_response_id: Option<String>,  // NEW
      pub store: Option<bool>,                   // NEW - Azure defaults to true
      pub background: Option<bool>,               // NEW - For async operations
      // ... existing fields
  }

  State Management:
  - Store response IDs in conversation history
  - Map Azure response IDs to internal conversation state
  - Handle response retrieval via GET endpoint

  Phase 3: Background Task Polling

  New Component: BackgroundTaskManager

  pub struct BackgroundTaskManager {
      tasks: Arc<RwLock<HashMap<String, BackgroundTask>>>,
      client: reqwest::Client,
  }

  pub struct BackgroundTask {
      response_id: String,
      status: TaskStatus,
      poll_interval: Duration,
      created_at: Instant,
  }

  pub enum TaskStatus {
      Queued,
      InProgress,
      Completed(ResponseData),
      Failed(String),
      Cancelled,
  }

  Implementation Notes:
  - Poll /v1/responses/{id} endpoint
  - Exponential backoff for polling intervals
  - Timeout after configurable duration
  - Support cancellation via DELETE endpoint

  Phase 4: Azure-Specific Wire Protocol

  New SSE Events for Azure Responses API:
  // Add to process_sse() in client.rs
  match event_type {
      "response.reasoning.delta" => { /* Reasoning tokens */ },
      "response.preamble.delta" => { /* Planning before tools */ },
      "response.image_generation_call.partial_image" => { /* Partial images */ },
      "response.mcp_approval_request" => { /* MCP tool approval */ },
      // ... existing events
  }

  Azure-Specific Response Fields:
  - reasoning_effort: low/medium/high
  - verbosity: low/medium/high (GPT-5 models)
  - truncation: auto/disabled
  - include[]: Array of additional output data

  Phase 5: Built-in Provider Definitions

  Add to built_in_model_providers() in model_provider_info.rs:

  // Azure Responses API provider
  providers.insert("azure-responses".to_string(), ModelProviderInfo {
      name: "azure-responses".to_string(),
      base_url: None, // Requires user configuration
      env_key: Some("AZURE_OPENAI_API_KEY".to_string()),
      wire_api: WireApi::Responses,
      query_params: Some(HashMap::from([
          ("api-version".to_string(), "2025-04-01-preview".to_string())
      ])),
      requires_openai_auth: false,
      auth_type: AuthType::ApiKey, // NEW field
      // ... other fields
  });

  // Azure Chat Completions provider (fallback)
  providers.insert("azure-chat".to_string(), ModelProviderInfo {
      name: "azure-chat".to_string(),
      base_url: None,
      env_key: Some("AZURE_OPENAI_API_KEY".to_string()),
      wire_api: WireApi::Chat,
      query_params: Some(HashMap::from([
          ("api-version".to_string(), "2024-10-01-preview".to_string())
      ])),
      requires_openai_auth: false,
      auth_type: AuthType::ApiKey,
      // ... other fields
  });

  Phase 6: Model Mapping and Validation

  Azure Deployment Mapping:
  pub struct AzureModelMapping {
      deployment_name: String,
      openai_model: String,
      capabilities: ModelCapabilities,
  }

  pub struct ModelCapabilities {
      supports_reasoning: bool,
      supports_vision: bool,
      supports_tools: bool,
      max_context: usize,
      supports_background: bool,
  }

  Validation Logic:
  - Check region availability
  - Validate API version compatibility
  - Verify model capabilities match request

  Phase 7: Error Handling Enhancement

  Azure-Specific Error Types:
  pub enum AzureError {
      ContentFilterViolation { category: String, severity: String },
      DeploymentNotFound { deployment: String },
      RegionNotSupported { region: String },
      ApiVersionMismatch { required: String, provided: String },
      QuotaExceeded { reset_at: Option<Instant> },
  }

  Error Response Parsing:
  - Azure returns different error structure than OpenAI
  - Content filter errors need special handling
  - Rate limiting uses different headers

  Edge Cases and Subtle Changes

  1. Streaming Differences:
    - Azure may chunk SSE events differently
    - Handle partial JSON in event data
    - Azure might send keepalive events
  2. Tool Calling Variations:
    - Azure's function schema validation is stricter
    - Custom tools (lark_tool) only work with GPT-5 models
    - MCP tool approval flow differs
  3. Authentication Complexity:
    - API key vs Azure AD token detection
    - Managed identity requires token refresh
    - Service principal needs certificate handling
  4. Regional Failover:
    - Support multiple Azure regions for redundancy
    - Automatic failover on region outage
    - Load balancing across regions
  5. Response Storage:
    - Azure defaults store=true, OpenAI defaults store=false
    - Zero data retention requires special handling
    - Response expiration after 30 days
  6. Model-Specific Behaviors:
    - O-series models don't support system messages with reasoning
    - GPT-5 models have unique parameters (verbosity, preamble)
    - Reasoning models require different token counting

  Testing Strategy

  Unit Tests:
  #[test]
  fn test_azure_auth_header() {
      // Verify api-key header for Azure providers
  }

  #[test]
  fn test_response_chaining() {
      // Test previous_response_id handling
  }

  #[test]
  fn test_background_polling() {
      // Test async task lifecycle
  }

  Integration Tests:
  - Mock Azure OpenAI server with proper SSE events
  - Test all Azure-specific parameters
  - Verify error handling for Azure errors

  Migration Path

  1. Phase 1 Release: Authentication fix only (backward compatible)
  2. Phase 2 Release: Add Azure providers without breaking changes
  3. Phase 3 Release: Full Responses API features
  4. Documentation: Migration guide for existing Azure users

  Configuration Examples

  Basic Azure Setup:
  [model_providers.my-azure]
  name = "my-azure"
  type = "azure-responses"  # Uses built-in Azure provider
  base_url = "https://myresource.openai.azure.com/openai"
  env_key = "AZURE_OPENAI_API_KEY"

  [models.gpt-5-deployment]
  provider = "my-azure"
  deployment_name = "gpt-5-prod"
  openai_model = "gpt-5"

  Advanced Features:
  [model_providers.azure-advanced]
  name = "azure-advanced"
  type = "azure-responses"
  base_url = "https://myresource.openai.azure.com/openai"
  auth_type = "azure_ad"  # Use Azure AD auth
  query_params = { api-version = "2025-04-01-preview", deployment-id = "custom" }
  http_headers = { "x-ms-region" = "eastus2" }
  request_max_retries = 5
  stream_idle_timeout_ms = 30000

  This refined plan addresses all the subtle issues discovered in the analysis
  while maintaining backward compatibility and leveraging existing infrastructure.