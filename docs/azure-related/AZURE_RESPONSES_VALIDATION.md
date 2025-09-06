# Azure Responses API Validation

Based on [Microsoft's Azure OpenAI Responses API documentation](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/responses?tabs=python-key#chaining-responses-together), our advanced features architecture has been validated and refined.

## Key Findings from Azure Documentation

### 1. **Response Chaining Without Storage**
Azure supports `previous_response_id` **without requiring** `store=true`:
```python
# This works on Azure
second_response = client.responses.create(
    model="gpt-4o",
    previous_response_id=response.id,  # Context maintained without storage
    input=[{"role": "user", "content": "Explain this at a college freshman level"}]
)
```

### 2. **Storage Required Only for Background Processing**  
The `store=true` parameter is **only required** for background/async processing:
```python
response = client.responses.create(
    model="gpt-4o", 
    store=true,  # Required for background mode
    # ... other params
)
```

### 3. **Context Preservation**
Even without explicit re-sharing, the model maintains full conversational context when using `previous_response_id`.

## Architecture Validation ✅

Our implementation perfectly aligns with Azure's behavior:

### Before (Incorrect Assumption)
```rust
pub fn azure_optimized() -> Self {
    Self {
        enable_response_chaining: true,
        enable_background_processing: true, 
        enable_stream_resumption: true,
        enable_response_storage: false, // ❌ Wrong - background needs storage
        ..Default::default()
    }
}
```

### After (Azure-Validated)
```rust
pub fn azure_optimized() -> Self {
    Self {
        enable_response_chaining: true,
        enable_background_processing: true,
        enable_stream_resumption: true, 
        enable_response_storage: true, // ✅ Required for background processing
        ..Default::default()
    }
}
```

## Auto-Validation Logic ✅

Our `validate_and_fix()` method now correctly handles Azure requirements:

```rust
pub fn validate_and_fix(&mut self) {
    // Response chaining on OpenAI requires storage, but Azure supports it without storage
    if self.enable_response_chaining && !self.enable_response_storage {
        tracing::warn!("Response chaining may require storage for OpenAI providers");
    }

    // Background processing REQUIRES storage on Azure (store=true)
    if self.enable_background_processing && !self.enable_response_storage {
        tracing::warn!("Background processing requires storage (store=true) - auto-enabling");
        self.enable_response_storage = true; // ✅ Auto-fix dependency
    }
}
```

## Provider Comparison

| Feature | OpenAI | Azure | Our Implementation |
|---------|--------|--------|-------------------|
| Response Chaining | Requires `store=true` | Works without storage | ✅ Provider-aware warnings |
| Background Processing | Not documented | Requires `store=true` | ✅ Auto-enables storage |
| Stream Resumption | Unknown | Supported | ✅ Enabled by default |
| Context Preservation | Via storage | Via `previous_response_id` | ✅ Abstracted via traits |

## Test Coverage ✅

All functionality is tested with 5 passing tests:
- Default performance-first behavior
- OpenAI-specific optimizations
- Azure-specific optimizations  
- Custom configurations
- Auto-validation logic

## Usage Examples

### Azure Response Chaining (Basic)
```rust
// Chaining without background processing - no storage needed
let features = AdvancedFeatures {
    enable_response_chaining: true,
    enable_background_processing: false,
    enable_response_storage: false,    // OK for Azure chaining
    enable_stream_resumption: true,
    ..Default::default()
};
```

### Azure Background Processing
```rust
// Background processing auto-enables storage
let mut features = AdvancedFeatures {
    enable_background_processing: true,
    enable_response_storage: false,
    ..Default::default()
};
features.validate_and_fix(); // Auto-enables storage
assert!(features.enable_response_storage); // Now true
```

### Recommended: Use Preset
```rust
// Simplest approach - use the validated preset
let client = ModelClient::new(config, auth, provider, effort, summary, session_id)
    .with_advanced_features(AdvancedFeatures::azure_optimized());
```

## Resolution of Original Issue ✅

The original `"previous_response_not_found"` error is now addressable through:

1. **Modular opt-in**: Advanced features disabled by default for performance
2. **Provider-specific presets**: `azure_optimized()` and `openai_optimized()` 
3. **Auto-validation**: Dependencies auto-enabled with warnings
4. **Easy integration**: Builder pattern with `.with_advanced_features()`

The architecture provides a clean path from the performance-first defaults to full Azure Responses API compatibility.