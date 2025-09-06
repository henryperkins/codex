# Response Chaining & Background Tasks Architecture

## Overview

This document outlines a modular, decoupled architecture for response chaining, background tasks, and storage functionality in Codex. The design allows developers to opt-in to advanced features without complexity when not needed.

## Current State Analysis

### What Upstream Kept
- ✅ **Resume functionality** - Full session restoration from .jsonl files
- ✅ **Azure stream resumption** - Network resilience for long-running streams  
- ✅ **Rollout recording** - Persistent conversation logging
- ✅ **Background processing infrastructure** - Task spawning and management

### What Upstream Removed
- ❌ **OpenAI response chaining** - `previous_response_id` context linking
- ❌ **Response storage** - Hardcoded `store: false` for performance
- ❌ **Background request mode** - Async processing with polling

## Proposed Modular Architecture

### 1. Feature Flags System

```rust
// Core configuration
#[derive(Debug, Clone)]
pub struct AdvancedFeatures {
    pub enable_response_chaining: bool,
    pub enable_background_processing: bool,
    pub enable_response_storage: bool,
    pub enable_stream_resumption: bool,
}

impl Default for AdvancedFeatures {
    fn default() -> Self {
        Self {
            enable_response_chaining: false,    // Performance-first default
            enable_background_processing: false,
            enable_response_storage: false,
            enable_stream_resumption: true,     // Keep network resilience
        }
    }
}
```

### 2. Storage Strategy Interface

```rust
pub trait ResponseStorage: Send + Sync {
    async fn should_store(&self, request: &ResponseRequest) -> bool;
    async fn store_response(&self, response: &Response) -> Result<String>;
    async fn get_previous_response(&self, id: &str) -> Result<Option<Response>>;
}

// Default: no storage (upstream behavior)
pub struct NoStorage;
impl ResponseStorage for NoStorage {
    async fn should_store(&self, _request: &ResponseRequest) -> bool { false }
    async fn store_response(&self, _response: &Response) -> Result<String> { 
        Err(CodexErr::StorageDisabled) 
    }
    async fn get_previous_response(&self, _id: &str) -> Result<Option<Response>> { 
        Ok(None) 
    }
}

// OpenAI-compatible storage
pub struct OpenAIStorage {
    // Implementation for providers that need response chaining
}

// Azure-style temporary storage  
pub struct AzureStorage {
    // Implementation for stream resumption only
}
```

### 3. Response Chaining Module

```rust
pub mod response_chaining {
    use super::*;
    
    pub struct ChainingManager<S: ResponseStorage> {
        storage: S,
        enabled: bool,
    }
    
    impl<S: ResponseStorage> ChainingManager<S> {
        pub fn new(storage: S, enabled: bool) -> Self {
            Self { storage, enabled }
        }
        
        pub async fn prepare_request(&self, request: &mut ResponseRequest) -> Result<()> {
            if !self.enabled {
                return Ok(());
            }
            
            // Add previous_response_id if chaining is needed
            if let Some(prev_id) = &request.context.previous_response_id {
                if self.storage.should_store(request).await {
                    request.payload.previous_response_id = Some(prev_id.clone());
                    request.payload.store = true;
                }
            }
            Ok(())
        }
    }
}
```

### 4. Background Processing Module

```rust
pub mod background_tasks {
    use super::*;
    
    pub struct BackgroundProcessor {
        enabled: bool,
        polling_config: PollingConfig,
    }
    
    pub struct PollingConfig {
        pub initial_delay: Duration,
        pub max_retries: u32,
        pub backoff_multiplier: f64,
    }
    
    impl BackgroundProcessor {
        pub async fn process_request(&self, request: RequestSpec) -> Result<ResponseStream> {
            if !self.enabled {
                return self.process_synchronously(request).await;
            }
            
            // Check if request should go to background
            if self.should_background(&request).await {
                self.process_async_with_polling(request).await
            } else {
                self.process_synchronously(request).await
            }
        }
        
        async fn should_background(&self, request: &RequestSpec) -> bool {
            // Heuristics: long requests, tool-heavy, etc.
            request.estimated_complexity() > ComplexityThreshold::High
        }
    }
}
```

### 5. Provider-Specific Adapters

```rust
pub mod provider_adapters {
    use super::*;
    
    pub trait ProviderCapabilities {
        fn supports_response_chaining(&self) -> bool;
        fn supports_background_processing(&self) -> bool;
        fn supports_stream_resumption(&self) -> bool;
        fn requires_response_storage_for_chaining(&self) -> bool;
    }
    
    pub struct OpenAIAdapter;
    impl ProviderCapabilities for OpenAIAdapter {
        fn supports_response_chaining(&self) -> bool { true }
        fn supports_background_processing(&self) -> bool { false }
        fn supports_stream_resumption(&self) -> bool { false }
        fn requires_response_storage_for_chaining(&self) -> bool { true }
    }
    
    pub struct AzureAdapter;
    impl ProviderCapabilities for AzureAdapter {
        fn supports_response_chaining(&self) -> bool { true }
        fn supports_background_processing(&self) -> bool { true }
        fn supports_stream_resumption(&self) -> bool { true }
        fn requires_response_storage_for_chaining(&self) -> bool { false }
    }
}
```

### 6. Developer-Friendly Configuration

```toml
# Default: Simple and fast (upstream behavior)
[advanced_features]
response_chaining = false
background_processing = false  
response_storage = false

# OpenAI with context chaining
[profiles.openai-with-context]
model_provider = "openai"
[profiles.openai-with-context.advanced_features]
response_chaining = true
response_storage = true  # Auto-enabled for OpenAI chaining

# Azure with background tasks
[profiles.azure-background]  
model_provider = "azure-responses"
[profiles.azure-background.advanced_features]
background_processing = true
stream_resumption = true
```

### 7. Usage Examples

#### Simple Usage (Default)
```rust
// Just works like upstream - no complexity
let client = ModelClient::new(config);
let response = client.stream(prompt).await?;
```

#### Advanced Usage with Response Chaining
```rust
// Opt-in to response chaining
let mut features = AdvancedFeatures::default();
features.enable_response_chaining = true;
features.enable_response_storage = true;

let client = ModelClient::with_features(config, features);

// Now previous_response_id works automatically
let prompt = Prompt {
    input: "Continue the conversation",
    previous_response_id: Some("resp_123".to_string()),
    ..Default::default()
};
let response = client.stream(prompt).await?;
```

#### Background Processing
```rust
// Enable background processing for long-running tasks
let features = AdvancedFeatures {
    enable_background_processing: true,
    enable_response_storage: true,  // Required for background
    ..Default::default()
};

let client = ModelClient::with_features(config, features);

// Long requests automatically go to background
let complex_prompt = Prompt {
    input: "Analyze this large codebase and provide detailed recommendations",
    tools: vec![/* many tools */],
    ..Default::default()
};

// Returns immediately with polling stream
let response_stream = client.stream(complex_prompt).await?;
```

## Implementation Strategy

### Phase 1: Core Infrastructure
1. **Extract storage interface** from existing code
2. **Create feature flags system** in Config
3. **Modularize response chaining** logic
4. **Add provider capability detection**

### Phase 2: Advanced Features
1. **Implement background processor** module
2. **Create storage implementations** for different providers
3. **Add developer configuration** options
4. **Create usage examples** and documentation

### Phase 3: Integration
1. **Integrate with existing client** code
2. **Maintain backward compatibility** with upstream
3. **Add comprehensive tests** for all feature combinations
4. **Performance benchmarking** to validate defaults

## Benefits

### For Developers
- 🚀 **Zero complexity by default** - matches upstream performance
- 🔧 **Opt-in complexity** - only pay for features you use  
- 📝 **Clear configuration** - explicit feature enabling
- 🔄 **Gradual adoption** - can enable features incrementally

### For Maintainers  
- 🧩 **Modular code** - features are isolated and testable
- 🔒 **Backward compatible** - upstream changes integrate cleanly
- 📊 **Performance clarity** - impact of features is measurable
- 🎯 **Focused debugging** - issues are contained to specific modules

### For Users
- ⚡ **Fast by default** - no performance penalty for basic usage
- 🎛️ **Power when needed** - advanced features available when required
- 📚 **Clear mental model** - understand what each feature costs
- 🔄 **Easy migration** - from simple to advanced usage

## Migration Path

This architecture allows for:
1. **Immediate merge** of upstream changes (no conflicts)
2. **Gradual implementation** of modular features  
3. **Zero disruption** to existing users
4. **Clear upgrade path** for users who need advanced features

The result is a codebase that respects upstream's performance decisions while providing advanced capabilities for users who need them.