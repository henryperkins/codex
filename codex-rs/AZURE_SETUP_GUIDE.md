# Azure OpenAI Setup Guide for Codex CLI

## Quick Setup (5 minutes)

### Step 1: Get Your Azure OpenAI Credentials

1. Go to [Azure Portal](https://portal.azure.com)
2. Navigate to your Azure OpenAI resource
3. Click on "Keys and Endpoint" in the left menu
4. Copy:
   - **KEY 1** (this will be your API key)
   - **Endpoint** (e.g., `https://myresource.openai.azure.com`)

### Step 2: Set Environment Variables

```bash
# Add to your ~/.bashrc, ~/.zshrc, or shell profile
export AZURE_OPENAI_API_KEY="paste-your-key-here"
export AZURE_OPENAI_ENDPOINT="https://your-resource.openai.azure.com"

# Reload your shell
source ~/.bashrc  # or ~/.zshrc
```

### Step 3: Find Your Deployment Names

1. In Azure Portal, go to your Azure OpenAI resource
2. Click on "Model deployments" → "Manage Deployments"
3. Note your deployment names (these are NOT the model names)
   - Example: Your GPT-5 deployment might be named "gpt-5-prod" or "my-gpt5"
   - Example: Your o3 deployment might be named "o3-deployment" or "reasoning-o3"

### Step 4: Create Codex Configuration

Create file `~/.codex/config.toml`:

```toml
# Azure OpenAI Configuration for Codex CLI

# Default to GPT-5
model_provider = "azure-responses"
model = "YOUR-GPT5-DEPLOYMENT-NAME"  # ← Replace with your actual deployment name
model_reasoning_effort = "medium"
model_reasoning_summary = "detailed"

# Profile for o3
[profiles.o3]
model_provider = "azure-responses"
model = "YOUR-O3-DEPLOYMENT-NAME"  # ← Replace with your actual deployment name
model_reasoning_effort = "high"
stream_idle_timeout_ms = 1800000  # 30 minutes

# Set default profile
profile = "default"
```

### Step 5: Test Your Setup

```bash
# Test GPT-5
codex "Hello, are you working?"

# Test o3
codex --profile o3 "What is 2+2?"

# If you see errors, check troubleshooting below
```

---

## Advanced Configuration

### Using Custom Endpoints

If your Azure endpoint is non-standard or you need specific API versions:

```toml
[model_providers.my-azure]
name = "My Azure OpenAI"
base_url = "https://custom-endpoint.openai.azure.com/openai/v1"
env_key = "AZURE_OPENAI_API_KEY"
wire_api = "responses"
query_params = { api-version = "2025-04-01-preview" }  # Specific version

model_provider = "my-azure"
model = "deployment-name"
```

### Multiple Azure Resources

If you have different models in different Azure resources:

```toml
# Resource 1: GPT-5
[model_providers.azure-eastus]
name = "Azure East US"
base_url = "https://eastus-resource.openai.azure.com/openai/v1"
env_key = "AZURE_EASTUS_API_KEY"
wire_api = "responses"
query_params = { api-version = "preview" }

# Resource 2: o3
[model_providers.azure-westus]
name = "Azure West US"
base_url = "https://westus-resource.openai.azure.com/openai/v1"
env_key = "AZURE_WESTUS_API_KEY"
wire_api = "responses"
query_params = { api-version = "preview" }

# Profiles to switch between them
[profiles.gpt5-east]
model_provider = "azure-eastus"
model = "gpt-5-deployment"

[profiles.o3-west]
model_provider = "azure-westus"
model = "o3-deployment"
```

### Performance Tuning for Different Models

```toml
# GPT-5: Balanced settings
[profiles.gpt5]
model_provider = "azure-responses"
model = "gpt-5"
model_reasoning_effort = "medium"
stream_idle_timeout_ms = 300000  # 5 minutes

# GPT-5 Quick: For fast responses
[profiles.gpt5-quick]
model_provider = "azure-responses"
model = "gpt-5"
model_reasoning_effort = "minimal"  # Fastest reasoning
stream_idle_timeout_ms = 120000  # 2 minutes

# o3: For complex reasoning
[profiles.o3]
model_provider = "azure-responses"
model = "o3"
model_reasoning_effort = "high"
stream_idle_timeout_ms = 1800000  # 30 minutes
stream_max_retries = 15

# o3-pro: For very long tasks
[profiles.o3-pro]
model_provider = "azure-responses"
model = "o3-pro"
model_reasoning_effort = "high"
stream_idle_timeout_ms = 3600000  # 60 minutes
stream_max_retries = 20
```

---

## Troubleshooting

### Common Issues and Solutions

#### 1. "API key not found" Error
```bash
# Check if environment variable is set
echo $AZURE_OPENAI_API_KEY

# If empty, set it:
export AZURE_OPENAI_API_KEY="your-key-here"
```

#### 2. "Model not found" Error
- **Cause**: Using model name instead of deployment name
- **Fix**: Use your deployment name from Azure Portal, not "gpt-5" or "o3"

```toml
# Wrong
model = "gpt-5"  # This is the model name

# Correct
model = "my-gpt5-deployment"  # This is your deployment name
```

#### 3. "Invalid API version" Error
```toml
# Try different API versions
query_params = { api-version = "preview" }  # Latest preview
# OR
query_params = { api-version = "2025-04-01-preview" }  # Specific version
# OR
query_params = { api-version = "2024-12-01-preview" }  # Older version
```

#### 4. Timeout Errors with o3/o3-pro
```toml
# Increase timeouts
stream_idle_timeout_ms = 3600000  # 60 minutes
stream_max_retries = 20
```

#### 5. "Endpoint not found" Error
```bash
# Verify your endpoint
echo $AZURE_OPENAI_ENDPOINT

# Should look like:
# https://your-resource.openai.azure.com
# NOT: https://your-resource.openai.azure.com/
# NOT: https://your-resource.openai.azure.com/openai
```

#### 6. Testing Connection
```bash
# Test with curl
curl "${AZURE_OPENAI_ENDPOINT}/openai/deployments?api-version=2023-05-15" \
  -H "api-key: ${AZURE_OPENAI_API_KEY}"

# Should list your deployments
```

---

## Command Examples

### Basic Usage
```bash
# Use default model (GPT-5)
codex "Explain quantum computing"

# Use o3 for complex reasoning
codex --profile o3 "Solve this algorithm problem..."

# Quick mode with minimal reasoning
codex --config model_reasoning_effort=minimal "What's 2+2?"

# High reasoning effort
codex --config model_reasoning_effort=high "Analyze this complex system..."
```

### Advanced Usage
```bash
# Override model on the fly
codex --model o3 --config model_reasoning_effort=high "Complex task"

# Use specific provider
codex --model-provider azure-responses --model my-deployment "Query"

# Verbose mode for debugging
codex --model gpt-5 --config model_reasoning_summary=detailed "Task"
```

---

## Verification Checklist

- [ ] Environment variables set (`AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_ENDPOINT`)
- [ ] Config file created at `~/.codex/config.toml`
- [ ] Deployment names (not model names) used in config
- [ ] Test command works: `codex "Hello"`
- [ ] Profile switching works: `codex --profile o3 "Test"`

---

## Need Help?

1. Check Azure OpenAI service status in Azure Portal
2. Verify your subscription has access to the models
3. Ensure your region supports the specific models
4. Check API quotas and limits in Azure Portal
5. Review Codex logs: `~/.codex/logs/`

## Resources

- [Azure OpenAI Documentation](https://learn.microsoft.com/azure/ai-services/openai/)
- [Codex CLI Documentation](./README.md)
- [Azure Portal](https://portal.azure.com)