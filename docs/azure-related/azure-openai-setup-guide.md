# Azure OpenAI Setup Guide for Codex

This guide will help you configure Codex to use Azure OpenAI services, including both the Responses API and Chat Completions API.

## Prerequisites

Before starting, you'll need:
1. An Azure subscription with Azure OpenAI access
2. A deployed model in your Azure OpenAI resource
3. Your resource endpoint and API key

## Quick Start

### 1. Set Environment Variables

```bash
# Required: Your Azure OpenAI resource endpoint
export AZURE_OPENAI_ENDPOINT="https://your-resource.openai.azure.com"

# Required: Your API key
export AZURE_OPENAI_API_KEY="your-api-key-here"

# Optional: API version (defaults to "preview" for Responses API)
export AZURE_OPENAI_API_VERSION="2025-04-01-preview"
```

### 2. Configure Your Model

Edit `~/.codex/config.toml`:

```toml
# Use your Azure deployment name
model = "gpt-4o-deployment"  # Replace with your actual deployment name

# Choose the appropriate provider
model_provider_id = "azure-responses"  # or "azure-chat" for Chat Completions
```

### 3. Start Codex

```bash
codex
```

That's it! Codex will automatically use your Azure deployment.

## Detailed Configuration Options

### Using the Responses API (Recommended)

The Responses API provides advanced features like response chaining and background processing:

```toml
model = "your-deployment-name"
model_provider_id = "azure-responses"

# Optional: Override built-in settings
[model_providers.my-azure]
name = "my-azure"
base_url = "https://my-resource.openai.azure.com/openai/v1"
env_key = "AZURE_OPENAI_API_KEY"
wire_api = "responses"
auth_type = "api_key"
query_params = { api-version = "preview" }
```

### Using the Chat Completions API

For simpler deployments or compatibility:

```toml
model = "your-deployment-name"
model_provider_id = "azure-chat"

# Optional: Custom configuration
[model_providers.my-azure-chat]
name = "my-azure-chat"
base_url = "https://my-resource.openai.azure.com/openai"
env_key = "AZURE_OPENAI_API_KEY"
wire_api = "chat"
auth_type = "api_key"
query_params = { api-version = "2024-10-01-preview" }
```

## Model Capabilities

### Automatic Capability Detection

Codex automatically detects model capabilities based on your deployment name:

| Deployment Name Pattern | Capabilities |
|------------------------|--------------|
| `gpt-4o*` | Standard GPT-4o features |
| `gpt-5*` | Reasoning summaries |
| `o3*` | Reasoning summaries |
| `o4-mini*` | Reasoning summaries |
| `codex-*` | Reasoning summaries, local shell optimization |
| Custom names | Basic capabilities (no reasoning) |

### Example Configurations by Model Type

#### GPT-4o Deployment
```toml
model = "gpt-4o-prod"  # Automatically gets GPT-4o capabilities
model_provider_id = "azure-responses"
```

#### GPT-5 or O3 Deployment
```toml
model = "gpt-5-latest"  # Automatically enables reasoning
model_provider_id = "azure-responses"
```

#### Custom Deployment Name
```toml
model = "my-custom-model"  # No special capabilities
model_provider_id = "azure-responses"

# Manually specify capabilities if needed
[model_family]
slug = "my-custom-model"
family = "gpt-5"  # Inherit GPT-5 capabilities
supports_reasoning_summaries = true
```

## Advanced Features

### Response Chaining (Responses API only)

The Azure Responses API automatically chains conversations server-side:

```toml
# Response storage is enabled by default for Azure
# Previous responses are automatically linked
disable_response_storage = false  # Default
```

### Background Processing

Enable asynchronous response generation:

```bash
export CODEX_ENABLE_BACKGROUND=1
```

### Using Azure AD Authentication

Instead of API keys, use Azure Active Directory:

```toml
[model_providers.azure-ad]
name = "azure-ad"
base_url = "https://my-resource.openai.azure.com/openai/v1"
wire_api = "responses"
auth_type = "bearer"  # Use Bearer token instead of api-key
# Set up Azure AD authentication separately
```

## Regional Configuration

### Multi-Region Failover

Configure multiple Azure regions for redundancy:

```toml
# Primary region
[model_providers.azure-eastus]
name = "azure-eastus"
base_url = "https://eastus-resource.openai.azure.com/openai/v1"
env_key = "AZURE_EASTUS_KEY"
wire_api = "responses"
auth_type = "api_key"

# Fallback region
[model_providers.azure-westus]
name = "azure-westus"
base_url = "https://westus-resource.openai.azure.com/openai/v1"
env_key = "AZURE_WESTUS_KEY"
wire_api = "responses"
auth_type = "api_key"

# Use primary by default
model_provider_id = "azure-eastus"
```

## Troubleshooting

### Common Issues

#### 1. "DeploymentNotFound" Error
- **Cause**: The deployment name doesn't exist in your Azure resource
- **Fix**: Verify your deployment name in the Azure portal

#### 2. "InvalidApiVersion" Error
- **Cause**: Using an unsupported API version
- **Fix**: Use "preview" for Responses API or "2024-10-01-preview" for Chat

#### 3. Authentication Fails
- **Cause**: Incorrect API key or wrong header type
- **Fix**: Ensure you're using `azure-responses` or `azure-chat` provider (not generic OpenAI)

#### 4. Previous Response Not Found
- **Cause**: Response chaining issue
- **Fix**: Codex automatically retries without chaining; ensure `store=true`

#### 5. No Reasoning Output
- **Cause**: Model doesn't support reasoning or deployment name not recognized
- **Fix**: Use a deployment name matching a reasoning-capable model pattern

### Debug Mode

Enable detailed logging to troubleshoot issues:

```bash
RUST_LOG=debug codex
```

## Migration from OpenAI

### Minimal Changes Required

1. Change environment variables:
   ```bash
   # From OpenAI
   unset OPENAI_API_KEY
   
   # To Azure
   export AZURE_OPENAI_ENDPOINT="https://your-resource.openai.azure.com"
   export AZURE_OPENAI_API_KEY="your-key"
   ```

2. Update config.toml:
   ```toml
   # Change only the provider
   model_provider_id = "azure-responses"  # was "openai"
   model = "your-azure-deployment"  # was "gpt-4o"
   ```

### Feature Parity

Azure supports all OpenAI features:
- ✅ Streaming responses
- ✅ Function calling
- ✅ Reasoning (o3, gpt-5 models)
- ✅ Vision capabilities
- ✅ Code interpreter
- ✅ Response chaining
- ✅ Background processing

## Best Practices

1. **Use Descriptive Deployment Names**: Name deployments after their model type (e.g., "gpt-4o-prod") for automatic capability detection

2. **Enable Response Storage**: Keep `disable_response_storage = false` for optimal performance with response chaining

3. **Set Appropriate Timeouts**: For long-running tasks:
   ```toml
   [model_providers.my-azure]
   stream_idle_timeout_ms = 60000  # 60 seconds
   ```

4. **Monitor Rate Limits**: Azure has different rate limits than OpenAI; adjust retry settings:
   ```toml
   [model_providers.my-azure]
   request_max_retries = 5
   stream_max_retries = 3
   ```

## Example: Complete Configuration

Here's a production-ready Azure configuration:

```toml
# ~/.codex/config.toml

# Model configuration
model = "gpt-4o-2024-11"
model_provider_id = "azure-prod"
model_context_window = 128000
model_max_output_tokens = 16384

# Azure provider configuration
[model_providers.azure-prod]
name = "Azure Production"
base_url = "https://prod-openai.openai.azure.com/openai/v1"
env_key = "AZURE_OPENAI_API_KEY"
wire_api = "responses"
auth_type = "api_key"
query_params = { api-version = "preview" }
request_max_retries = 3
stream_max_retries = 2
stream_idle_timeout_ms = 45000

# Optional: Model family override for custom deployments
[model_family]
slug = "gpt-4o-2024-11"
family = "gpt-4o"
supports_reasoning_summaries = false
uses_local_shell_tool = false
```

## Next Steps

- Review [Azure OpenAI documentation](https://learn.microsoft.com/en-us/azure/ai-services/openai/)
- Explore [available models and regions](https://learn.microsoft.com/en-us/azure/ai-services/openai/concepts/models)
- Learn about [Azure OpenAI pricing](https://azure.microsoft.com/en-us/pricing/details/cognitive-services/openai-service/)

## Support

If you encounter issues:
1. Check this troubleshooting guide
2. Review Azure OpenAI service status
3. Enable debug logging with `RUST_LOG=debug`
4. Report issues at https://github.com/anthropics/codex/issues