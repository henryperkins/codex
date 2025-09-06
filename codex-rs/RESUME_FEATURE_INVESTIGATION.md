# Resume Feature Investigation

## Summary

There are **3 distinct types of resume functionality** in Codex, with different maturity levels:

## 1. 🔬 **Session Resume** (Experimental)
- **Status**: Experimental/undocumented
- **Config**: `experimental_resume = "/path/to/rollout-file.jsonl"`  
- **Implementation**: `conversation_manager.rs:resume_conversation_from_rollout()`
- **Purpose**: Resume entire conversations from saved session files
- **Storage Format**: JSONL files in `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`
- **UI Support**: TUI has `--resume` and `--continue` flags

### How it Works:
```rust
// In conversation_manager.rs:68-70
if let Some(resume_path) = config.experimental_resume.as_ref() {
    let initial_history = RolloutRecorder::get_rollout_history(resume_path).await?;
    let CodexSpawnOk { codex, session_id } = Codex::spawn(config, auth_manager, initial_history).await?;
    // ... continue with resumed state
}
```

## 2. 🚫 **Stream Resumption** (REMOVED)
- **Status**: ⚠️ **Removed in upstream merge** 
- **Previous Implementation**: Found in `client.rs.pre-merge-backup`
- **Purpose**: Auto-retry dropped streaming connections using `response_id` + sequence numbers
- **Removal**: Likely removed for simplification/performance

### What Was Removed:
```rust
// From client.rs.pre-merge-backup:991-1025
let mut response_id_for_resume: Option<String> = None;
let mut resume_attempts: u64 = 0;
let max_resume_retries = resume_ctx.as_ref().map(|c| c.provider.stream_max_retries()).unwrap_or(0);

// Auto-retry logic on stream errors
if resume_ctx.is_some() 
    && response_id_for_resume.is_some()
    && last_sequence_number.is_some()
    && resume_attempts < max_resume_retries 
{
    let new_stream = attempt_resume(&ctx.client, &ctx.provider, &ctx.auth_manager,
        response_id_for_resume.as_ref().unwrap(), last_sequence_number.unwrap()).await;
    resume_attempts += 1;
    stream = new_stream.eventsource();
    continue; // Resume from where we left off
}
```

## 3. ✅ **Response Chaining** (Available)
- **Status**: Production-ready in our `AdvancedFeatures` system
- **Purpose**: Continue conversations using `previous_response_id` from API responses
- **Implementation**: Via our modular `advanced_features.rs` system
- **Provider Support**: OpenAI (requires storage), Azure (works without storage)

## Current State Analysis

### What's Working ✅
1. **Session Resume**: Experimental but functional
   - Can resume entire conversations from JSONL rollout files
   - UI integration exists (`--resume`, `--continue`)
   - Stable storage format with pagination

2. **Response Chaining**: Our modular implementation
   - Provider-aware configuration
   - Auto-validation and dependency resolution  
   - Full test coverage

### What Was Lost ❌  
1. **Stream Resumption**: Advanced network resilience removed
   - No more automatic retry on dropped streaming connections
   - Lost sequence-based resumption with `response_id`
   - Lost Azure-specific stream resumption capabilities

## Impact on Our Advanced Features

### Good News ✅
- Our `AdvancedFeatures` architecture is **still valid**
- `enable_stream_resumption` flag can be **re-implemented** independently
- Session resume (`experimental_resume`) works orthogonally to our system

### Update Needed ⚠️
The `enable_stream_resumption` feature in our `AdvancedFeatures` is currently **aspirational** - the actual implementation was removed upstream.

## Recommendations

### 1. **Update Documentation**
Mark `enable_stream_resumption` as "planned feature" rather than "currently supported":

```rust
/// Enable stream resumption on network failures.
/// Provides resilience for long-running streaming responses.
/// Note: Implementation removed in upstream merge - marked for re-implementation.
pub enable_stream_resumption: bool,
```

### 2. **Consider Re-Implementation**  
The removed stream resumption code shows a sophisticated implementation that could be:
- Re-integrated as an optional advanced feature
- Made provider-agnostic
- Enhanced with exponential backoff

### 3. **Integration Opportunity**
Connect session resume with our advanced features:
```rust
pub struct AdvancedFeatures {
    // Existing features...
    
    /// Enable session resume from rollout files
    pub enable_session_resume: bool,
    
    /// Default rollout directory for session storage
    pub session_storage_path: Option<PathBuf>,
}
```

## File Locations

- **Session Resume**: `core/src/conversation_manager.rs` (lines 68-140)
- **Rollout Storage**: `core/src/rollout/` directory
- **Config**: `core/src/config.rs` (`experimental_resume` field)
- **Removed Stream Resume**: `core/src/client.rs.pre-merge-backup` (lines 991-1050)
- **Our Features**: `core/src/advanced_features.rs`

## Conclusion

The resume functionality is **partially available** but underwent **significant changes**:

- ✅ **Session resume**: Experimental but working
- ❌ **Stream resumption**: Removed in upstream merge  
- ✅ **Response chaining**: Our modular implementation works

Our `AdvancedFeatures` system provides a good foundation to **selectively re-implement** the removed stream resumption functionality as an opt-in feature.