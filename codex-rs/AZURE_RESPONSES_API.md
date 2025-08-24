# Azure OpenAI Responses API Integration

This document describes the complete implementation of Azure OpenAI Responses API support in Codex CLI.

## Overview

Azure OpenAI provides two distinct APIs:
1. **Chat Completions API** - Standard chat interface compatible with OpenAI's Chat API
2. **Responses API (Preview)** - Stateful, advanced API with support for reasoning models, background tasks, and enhanced features

## Implementation Summary

### Built-in Providers Added

Two new built-in providers have been added to Codex CLI:

1. **`azure-responses`** - Azure Responses API provider for stateful conversations
2. **`azure-chat`** - Azure Chat Completions API provider for standard chat

### Key Files Modified

- `core/src/model_provider_info.rs` - Added Azure provider implementations
- `config.md` - Updated documentation with Azure configuration examples

## Configuration

### Quick Start

#### Using Built-in Azure Responses API Provider
```bash
# Set environment variables
export AZURE_OPENAI_API_KEY="your-api-key"
export AZURE_OPENAI_ENDPOINT="https://your-resource.openai.azure.com"

# Use with Codex CLI
codex --model-provider azure-responses --model gpt-5 "Your prompt here"
```

#### Using Built-in Azure Chat Provider
```bash
# Set environment variables
export AZURE_OPENAI_API_KEY="your-api-key"
export AZURE_OPENAI_ENDPOINT="https://your-resource.openai.azure.com"

# Use with Codex CLI
codex --model-provider azure-chat --model gpt-4o "Your prompt here"
```

### Configuration File (config.toml)

#### Simple Configuration
```toml
# Use built-in Azure Responses API
model_provider = "azure-responses"
model = "gpt-5"
model_reasoning_effort = "medium"
model_reasoning_summary = "detailed"
```

#### Custom Configuration
```toml
[model_providers.my-azure]
name = "My Azure Responses API"
base_url = "https://myresource.openai.azure.com/openai/v1"
env_key = "AZURE_OPENAI_API_KEY"
wire_api = "responses"
query_params = { api-version = "preview" }
stream_max_retries = 10
stream_idle_timeout_ms = 600000  # 10 minutes for long-running models
```

## Features Supported

### Azure Responses API Features
- ✅ Stateful conversations with `previous_response_id`
- ✅ Reasoning models (gpt-5, o3, o4-mini)
- ✅ Reasoning effort control (minimal, low, medium, high)
- ✅ Reasoning summaries
- ✅ Background tasks for long-running operations
- ✅ Extended context windows (up to 400K tokens)
- ✅ Streaming support
- ✅ Function/tool calling
- ✅ Image input/generation
- ✅ Code interpreter
- ✅ MCP (Model Context Protocol) support

### Network Configuration
- Configurable retry logic for failed requests
- Adjustable streaming timeouts for long-running models
- Query parameter support for API versioning

## API Version Support

The implementation supports multiple API versions:
- `preview` - Latest preview features
- `2025-04-01-preview` - Specific preview version
- Custom versions via configuration

## Testing

Comprehensive tests have been added to verify:
- Provider deserialization from TOML
- URL construction with query parameters
- Wire API selection (Responses vs Chat)
- Network configuration options

Run tests with:
```bash
cargo test --package codex-core --lib model_provider_info::tests
```

## Usage Examples

### Basic Usage with Reasoning Models
```bash
# Using GPT-5 with reasoning
codex --model-provider azure-responses \
      --model gpt-5 \
      --config model_reasoning_effort=high \
      "Solve this complex problem..."
```

### Long-Running Tasks with o3-pro
```toml
# config.toml for o3-pro
model_provider = "azure-responses"
model = "o3-pro"
stream_idle_timeout_ms = 1800000  # 30 minutes
model_reasoning_effort = "high"
```

### Switching Between APIs
```bash
# Use Responses API for reasoning
codex --model-provider azure-responses --model gpt-5 "Complex reasoning task"

# Use Chat API for simple queries
codex --model-provider azure-chat --model gpt-4o "Simple question"
```

## Environment Variables

Required:
- `AZURE_OPENAI_API_KEY` - Your Azure OpenAI API key
- `AZURE_OPENAI_ENDPOINT` - Your Azure OpenAI resource endpoint

Optional:
- `OPENAI_BASE_URL` - Can be used to override the OpenAI provider to use Azure

## Architecture Notes

### Design Decisions
1. **Plugin Architecture**: Azure providers are implemented as configurable plugins, not hard-coded
2. **API Key Authentication**: Uses standard API key authentication (Azure AD not required)
3. **Dual Provider Support**: Separate providers for Responses and Chat APIs to maintain clarity
4. **Environment Variable Support**: Follows Azure's standard environment variable conventions

### Wire Protocol Selection
- `WireApi::Responses` - Routes to `/v1/responses` endpoint
- `WireApi::Chat` - Routes to `/v1/chat/completions` endpoint

### URL Construction
The system automatically constructs proper Azure URLs:
- Base URL + `/responses` + query parameters (for Responses API)
- Base URL + `/chat/completions` + query parameters (for Chat API)

## Limitations

1. **Azure AD Authentication**: Not implemented - uses API keys only
2. **Deployment Names**: No special handling for Azure deployment names vs model names
3. **Azure-Specific Features**: Some Azure-specific features may require additional configuration

## Future Enhancements

Potential improvements for future versions:
1. Azure AD authentication support
2. Automatic deployment name mapping
3. Azure-specific error handling
4. Integration with Azure AI Studio
5. Support for Azure content filters

## Troubleshooting

### Common Issues

1. **Invalid API Version**
   - Ensure you're using a supported API version
   - Try `"preview"` for latest features

2. **Timeout Errors with Reasoning Models**
   - Increase `stream_idle_timeout_ms` for long-running models
   - Consider using background mode for o3-pro

3. **Authentication Failures**
   - Verify `AZURE_OPENAI_API_KEY` is set correctly
   - Check endpoint URL includes proper resource name

## References

- [Azure OpenAI Responses API Documentation](https://learn.microsoft.com/azure/ai-services/openai/reference-responses)
- [Azure OpenAI Reasoning Models](https://learn.microsoft.com/azure/ai-services/openai/concepts/reasoning)
- [Codex CLI Configuration Guide](./config.md)