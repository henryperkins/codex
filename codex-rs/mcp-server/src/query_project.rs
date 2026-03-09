use anyhow::Context;
use codex_core::config::Config;
use codex_core::config::types::QueryProjectIndex;
use codex_core::config::types::QueryProjectIndexBackend;
use futures::TryStreamExt;
use globset::Glob;
use globset::GlobSet;
use globset::GlobSetBuilder;
use ignore::WalkBuilder;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::Condition;
use qdrant_client::qdrant::CreateCollectionBuilder;
use qdrant_client::qdrant::CreateFieldIndexCollectionBuilder;
use qdrant_client::qdrant::DeletePointsBuilder;
use qdrant_client::qdrant::Distance;
use qdrant_client::qdrant::FieldType;
use qdrant_client::qdrant::Filter;
use qdrant_client::qdrant::PointStruct;
use qdrant_client::qdrant::QueryPointsBuilder;
use qdrant_client::qdrant::UpsertPointsBuilder;
use qdrant_client::qdrant::VectorParamsBuilder;
use qdrant_client::qdrant::point_id::PointIdOptions;
use reqwest::StatusCode;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use rmcp::model::JsonObject;
use rmcp::model::Tool;
use schemars::JsonSchema;
use schemars::r#gen::SchemaSettings;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use sha1::Digest;
use sha1::Sha1;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::SqliteSynchronous;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::UNIX_EPOCH;
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;

const DEFAULT_LIMIT: usize = 8;
const MAX_LIMIT: usize = 200;
const DEFAULT_ALPHA: f32 = 0.6;
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_VOYAGE_EMBEDDINGS_URL: &str = "https://api.voyageai.com/v1/embeddings";
const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";
const OPENAI_BASE_URL_ENV_VAR: &str = "OPENAI_BASE_URL";
const VOYAGE_API_KEY_ENV_VAR: &str = "VOYAGE_API_KEY";
const INDEX_DIR: &str = ".codex/repo_hybrid_index";
const DB_FILE_NAME: &str = "index.sqlite";
const CHUNK_LINE_COUNT: usize = 40;
const CHUNK_LINE_OVERLAP: usize = 8;
const SNIPPET_LINE_COUNT: usize = 6;
const MAX_FILE_SIZE_BYTES: u64 = 1_500_000;
const EMBED_BATCH_SIZE: usize = 64;
const VECTOR_CANDIDATE_MULTIPLIER: usize = 8;
const LEXICAL_CANDIDATE_MULTIPLIER: usize = 8;
const FALLBACK_RG_LIMIT: usize = 2_000;
const SQLITE_BIND_CHUNK_SIZE: usize = 900;
const SQLITE_BUSY_TIMEOUT_SECS: u64 = 5;
const EMBEDDING_REQUEST_TIMEOUT_SECS: u64 = 30;
const EMBEDDING_RATE_LIMIT_RETRIES: u32 = 8;
/// Inputs longer than this are truncated to avoid exceeding the embedding
/// model's 8192-token context limit.  Code with short identifiers and heavy
/// punctuation can tokenize at close to 1 byte/token, so use a conservative
/// limit.
const EMBEDDING_MAX_INPUT_BYTES: usize = 8_000;
const EMBEDDING_CONNECT_TIMEOUT_SECS: u64 = 10;
const QUERY_LOG_PREVIEW_CHARS: usize = 96;
const METADATA_EMBEDDING_MODEL: &str = "embedding_model";
const METADATA_EMBEDDING_READY: &str = "embedding_ready";
const METADATA_VECTOR_BACKEND: &str = "vector_backend";
const METADATA_VECTOR_LAYOUT_VERSION: &str = "vector_layout_version";
const EMBEDDING_REASON_MISSING_API_KEY: &str = "missing_api_key";
const EMBEDDING_REASON_QUERY_FAILED: &str = "embedding_query_failed";
const EMBEDDING_REASON_UNAVAILABLE: &str = "embedding_unavailable";
const QDRANT_PAYLOAD_PATH_KEY: &str = "path";
const QDRANT_VECTOR_LAYOUT_VERSION: &str = "qdrant_path_cleanup_v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepoHybridSearchParams {
    /// Required natural-language query describing what to find in the repository.
    pub query: String,
    /// Maximum number of results to return. Must be > 0; capped at 200.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Optional repository root path. Defaults to the current working directory.
    #[serde(default)]
    pub repo_root: Option<String>,
    /// Optional glob filters (for example: ["src/**/*.rs", "docs/**"]).
    /// When omitted, all indexable files are considered.
    #[serde(default)]
    pub file_globs: Option<Vec<String>>,
    /// Search mode selector.
    /// `0.0` = lexical-only, `1.0` = vector-only, values in between = hybrid.
    #[serde(default = "default_alpha")]
    pub alpha: f32,
    /// Optional embedding model override. Defaults to `text-embedding-3-small`.
    #[serde(default)]
    pub embedding_model: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepoIndexRefreshParams {
    #[serde(default)]
    pub repo_root: Option<String>,
    #[serde(default)]
    pub file_globs: Option<Vec<String>>,
    #[serde(default)]
    pub embedding_model: Option<String>,
    #[serde(default)]
    pub force_full: bool,
    #[serde(default)]
    pub require_embeddings: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct RepoHybridSearchResultItem {
    pub path: String,
    pub line_range: LineRange,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct LineRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct RepoIndexRefreshStats {
    pub scanned_files: usize,
    pub updated_files: usize,
    pub removed_files: usize,
    pub indexed_chunks: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct RepoIndexWarmOutcome {
    pub repo_root: PathBuf,
    pub stats: RepoIndexRefreshStats,
    pub embedding_status: RepoEmbeddingStatus,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EmbeddingMode {
    Required,
    Skip,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct RepoEmbeddingStatus {
    pub mode_used: EmbeddingMode,
    pub ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
struct SelectedEmbeddingMode {
    mode: EmbeddingMode,
    reason: Option<&'static str>,
    require_embeddings: bool,
}

impl SelectedEmbeddingMode {
    fn status(&self, ready: bool) -> RepoEmbeddingStatus {
        RepoEmbeddingStatus {
            mode_used: self.mode,
            ready,
            reason: self.reason.map(str::to_string),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SearchBlendMode {
    LexicalOnly,
    Hybrid(f32),
    VectorOnly,
}

impl SearchBlendMode {
    fn from_alpha(alpha: f32) -> Self {
        if alpha <= f32::EPSILON {
            Self::LexicalOnly
        } else if (1.0 - alpha).abs() <= f32::EPSILON {
            Self::VectorOnly
        } else {
            Self::Hybrid(alpha)
        }
    }

    fn uses_embeddings(self) -> bool {
        !matches!(self, Self::LexicalOnly)
    }

    fn uses_lexical(self) -> bool {
        !matches!(self, Self::VectorOnly)
    }

    fn score(self, vector_score: f32, lexical_score: f32) -> f32 {
        match self {
            Self::LexicalOnly => lexical_score,
            Self::Hybrid(alpha) => alpha * vector_score + (1.0 - alpha) * lexical_score,
            Self::VectorOnly => vector_score,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmbeddingProvider {
    OpenAiCompatible,
    Voyage,
}

impl EmbeddingProvider {
    fn from_model(model: &str) -> Self {
        if model
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("voyage-")
        {
            return Self::Voyage;
        }
        Self::OpenAiCompatible
    }

    fn api_key_env_var(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => OPENAI_API_KEY_ENV_VAR,
            Self::Voyage => VOYAGE_API_KEY_ENV_VAR,
        }
    }

    fn embeddings_url(self, openai_base_url_override: Option<&str>) -> String {
        match self {
            Self::OpenAiCompatible => {
                let base_url = openai_base_url_override.map_or_else(
                    || {
                        std::env::var(OPENAI_BASE_URL_ENV_VAR)
                            .unwrap_or_else(|_| DEFAULT_OPENAI_BASE_URL.to_string())
                    },
                    str::to_owned,
                );
                let trimmed = base_url.trim_end_matches('/');
                if trimmed.ends_with("/embeddings")
                    || trimmed.contains("/embeddings?")
                    || trimmed.contains("/embeddings#")
                {
                    trimmed.to_string()
                } else {
                    format!("{trimmed}/embeddings")
                }
            }
            Self::Voyage => DEFAULT_VOYAGE_EMBEDDINGS_URL.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct ChunkDraft {
    start_line: usize,
    end_line: usize,
    content: String,
    snippet: String,
}

#[derive(Debug, Clone)]
struct ChunkRecord {
    path: String,
    start_line: usize,
    end_line: usize,
    snippet: String,
}

#[derive(Debug, Clone)]
struct SearchOutcome {
    results: Vec<RepoHybridSearchResultItem>,
    embedding_fallback_reason: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct RefreshIndexOutcome {
    stats: RepoIndexRefreshStats,
    ready: bool,
}

#[derive(Debug, Clone)]
struct ScannedFile {
    absolute_path: PathBuf,
    modified_sec: i64,
    modified_nsec: i64,
    size_bytes: i64,
}

#[derive(Debug, Clone, Copy)]
struct ExistingFile {
    modified_sec: i64,
    modified_nsec: i64,
    size_bytes: i64,
}

#[derive(Debug, Clone, Copy)]
struct StoredEmbeddingState<'a> {
    model: Option<&'a str>,
    ready: bool,
    backend: Option<&'a str>,
    backend_ready: bool,
    vector_layout_version: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FullRefreshReason {
    UserRequested,
    EmbeddingModelChanged,
    VectorBackendChanged,
    VectorCollectionMissing,
    VectorLayoutVersionChanged,
    StrictEmbeddingsNotReady,
}

pub(crate) fn create_tool_for_query_project() -> Tool {
    let schema = SchemaSettings::draft2019_09()
        .with(|settings| {
            settings.inline_subschemas = true;
            settings.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<RepoHybridSearchParams>();
    let input_schema = create_tool_input_schema(schema);
    Tool {
        name: "query_project".into(),
        title: Some("Query Project".to_string()),
        input_schema,
        output_schema: None,
        description: Some(
            "Search the current repository for relevant code snippets.\n\
             Call this before directly reading files so you start from ranked, relevant locations.\n\
             Use `query` for what you want to find, and optionally narrow with `file_globs` or `repo_root`.\n\
             `repo_root` must stay inside the current working directory.\n\
             Returns ranked matches with file path, line range, snippet, and score.\n\
             Automatically performs an incremental index refresh before searching."
                .into(),
        ),
        annotations: None,
        execution: None,
        icons: None,
        meta: None,
    }
}

pub(crate) fn create_tool_for_repo_index_refresh() -> Tool {
    let schema = SchemaSettings::draft2019_09()
        .with(|settings| {
            settings.inline_subschemas = true;
            settings.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<RepoIndexRefreshParams>();
    let input_schema = create_tool_input_schema(schema);
    Tool {
        name: "repo_index_refresh".into(),
        title: Some("Repo Index Refresh".to_string()),
        input_schema,
        output_schema: None,
        description: Some(
            "Incrementally refreshes the repository hybrid-search index (local SQLite/FTS plus configured vector backend).".into(),
        ),
        annotations: None,
        execution: None,
        icons: None,
        meta: None,
    }
}

pub(crate) async fn handle_repo_index_refresh(
    arguments: Option<JsonObject>,
    config: &Config,
) -> CallToolResult {
    let params = match arguments {
        Some(arguments) => match parse_arguments::<RepoIndexRefreshParams>(Some(arguments)) {
            Ok(params) => params,
            Err(result) => return result,
        },
        None => RepoIndexRefreshParams::default(),
    };

    let repo_root = match resolve_repo_root(params.repo_root.as_deref()) {
        Ok(repo_root) => repo_root,
        Err(err) => return call_tool_error(format!("invalid repo_root: {err}")),
    };

    let file_globs = params
        .file_globs
        .unwrap_or_else(|| config.query_project_index.file_globs.clone());
    let embedding_model = params
        .embedding_model
        .or_else(|| config.query_project_index.embedding_model.clone());
    let require_embeddings = params
        .require_embeddings
        .unwrap_or(config.query_project_index.require_embeddings);

    let outcome = match refresh_repo_index(
        repo_root,
        file_globs,
        embedding_model,
        params.force_full,
        require_embeddings,
        &config.query_project_index,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(err) => return call_tool_error(format!("index refresh failed: {err}")),
    };

    let payload = json!({
        "repo_root": outcome.repo_root.display().to_string(),
        "stats": outcome.stats,
        "embedding_status": outcome.embedding_status,
    });
    call_tool_success(payload)
}

pub(crate) async fn auto_warm_query_project_index(
    config: &Config,
) -> anyhow::Result<RepoIndexWarmOutcome> {
    let repo_root = resolve_repo_root(None)?;
    refresh_repo_index(
        repo_root,
        config.query_project_index.file_globs.clone(),
        config.query_project_index.embedding_model.clone(),
        false,
        config.query_project_index.require_embeddings,
        &config.query_project_index,
    )
    .await
}

async fn refresh_repo_index(
    repo_root: PathBuf,
    file_globs: Vec<String>,
    embedding_model: Option<String>,
    force_full: bool,
    require_embeddings: bool,
    index_config: &QueryProjectIndex,
) -> anyhow::Result<RepoIndexWarmOutcome> {
    let index = RepoHybridIndex::open(&repo_root, index_config)
        .await
        .with_context(|| format!("failed to initialize index at `{}`", repo_root.display()))?;
    let embedding_model = embedding_model_or_default(embedding_model);
    let mut embedding_mode = resolve_embedding_mode(require_embeddings, &embedding_model)?;
    let refresh_outcome = refresh_index(
        &index,
        &file_globs,
        force_full,
        embedding_model.as_str(),
        &mut embedding_mode,
    )
    .await?;

    Ok(RepoIndexWarmOutcome {
        repo_root,
        stats: refresh_outcome.stats,
        embedding_status: embedding_mode.status(refresh_outcome.ready),
    })
}

pub(crate) async fn handle_query_project(
    arguments: Option<JsonObject>,
    config: &Config,
) -> CallToolResult {
    let call_start = std::time::Instant::now();
    let params = match parse_arguments::<RepoHybridSearchParams>(arguments) {
        Ok(params) => params,
        Err(result) => {
            tracing::warn!(
                stage = "parse_arguments",
                elapsed_ms = call_start.elapsed().as_millis() as u64,
                "query_project failed"
            );
            return result;
        }
    };

    let query = params.query.trim();
    let query_preview = query_log_preview(query);
    let query_char_count = query.chars().count();
    tracing::info!(
        query_preview,
        query_char_count,
        limit = params.limit,
        alpha = params.alpha,
        "query_project called"
    );
    tracing::debug!(query, "query_project full query");
    if query.is_empty() {
        return query_project_failure(
            &call_start,
            "validate_query",
            Some(query_preview.as_str()),
            "query must not be empty",
        );
    }

    if params.limit == 0 {
        return query_project_failure(
            &call_start,
            "validate_limit",
            Some(query_preview.as_str()),
            "limit must be greater than zero",
        );
    }

    if !(0.0..=1.0).contains(&params.alpha) {
        return query_project_failure(
            &call_start,
            "validate_alpha",
            Some(query_preview.as_str()),
            "alpha must be between 0.0 and 1.0",
        );
    }

    let limit = params.limit.min(MAX_LIMIT);
    let repo_root = match resolve_repo_root(params.repo_root.as_deref()) {
        Ok(repo_root) => repo_root,
        Err(err) => {
            return query_project_failure(
                &call_start,
                "resolve_repo_root",
                Some(query_preview.as_str()),
                format!("invalid repo_root: {err}"),
            );
        }
    };

    let file_globs = params
        .file_globs
        .unwrap_or_else(|| config.query_project_index.file_globs.clone());
    let blend_mode = SearchBlendMode::from_alpha(params.alpha);
    let embedding_model = embedding_model_or_default(
        params
            .embedding_model
            .or_else(|| config.query_project_index.embedding_model.clone()),
    );
    let embedding_mode = match resolve_query_project_embedding_mode(
        blend_mode,
        config.query_project_index.require_embeddings,
        &embedding_model,
    ) {
        Ok(embedding_mode) => embedding_mode,
        Err(err) => {
            return query_project_failure(
                &call_start,
                "resolve_embedding_mode",
                Some(query_preview.as_str()),
                format!("failed to resolve embedding mode: {err}"),
            );
        }
    };

    let index = match RepoHybridIndex::open(&repo_root, &config.query_project_index).await {
        Ok(index) => index,
        Err(err) => {
            return query_project_failure(
                &call_start,
                "open_index",
                Some(query_preview.as_str()),
                format!(
                    "failed to initialize index at `{}`: {err}",
                    repo_root.display()
                ),
            );
        }
    };
    let mut embedding_mode = embedding_mode;

    let refresh_outcome = match refresh_index(
        &index,
        &file_globs,
        false,
        embedding_model.as_str(),
        &mut embedding_mode,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(err) => {
            return query_project_failure(
                &call_start,
                "refresh_index",
                Some(query_preview.as_str()),
                format!("index refresh failed: {err}"),
            );
        }
    };

    let search_outcome = match index
        .search(
            query,
            limit,
            blend_mode,
            &file_globs,
            embedding_model.clone(),
            config.query_project_index.require_embeddings,
        )
        .await
    {
        Ok(results) => results,
        Err(err) => {
            return query_project_failure(
                &call_start,
                "search",
                Some(query_preview.as_str()),
                format!("hybrid search failed: {err}"),
            );
        }
    };
    let embedding_status = if let Some(reason) = search_outcome.embedding_fallback_reason {
        RepoEmbeddingStatus {
            mode_used: embedding_mode.mode,
            ready: false,
            reason: Some(reason.to_string()),
        }
    } else {
        embedding_mode.status(refresh_outcome.ready)
    };

    let result_count = search_outcome.results.len();
    let elapsed = call_start.elapsed();
    tracing::info!(
        query_preview,
        query_char_count,
        result_count,
        elapsed_ms = elapsed.as_millis() as u64,
        "query_project completed"
    );

    let payload = json!({
        "repo_root": repo_root.display().to_string(),
        "query": query,
        "limit": limit,
        "alpha": params.alpha,
        "embedding_model": embedding_model,
        "embedding_status": embedding_status,
        "refresh": refresh_outcome.stats,
        "results": search_outcome.results,
    });
    call_tool_success(payload)
}

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

fn query_log_preview(query: &str) -> String {
    let trimmed = query.trim();
    let mut preview = trimmed
        .chars()
        .take(QUERY_LOG_PREVIEW_CHARS)
        .collect::<String>();
    if trimmed.chars().count() > QUERY_LOG_PREVIEW_CHARS {
        preview.push('…');
    }
    preview
}

fn query_project_failure(
    call_start: &std::time::Instant,
    stage: &str,
    query_preview: Option<&str>,
    message: impl Into<String>,
) -> CallToolResult {
    let message = message.into();
    let elapsed_ms = call_start.elapsed().as_millis() as u64;
    if let Some(query_preview) = query_preview {
        tracing::warn!(stage, query_preview, elapsed_ms, "query_project failed");
    } else {
        tracing::warn!(stage, elapsed_ms, "query_project failed");
    }
    call_tool_error(message)
}

fn default_alpha() -> f32 {
    DEFAULT_ALPHA
}

fn embedding_model_or_default(model: Option<String>) -> String {
    match model {
        Some(model) if !model.trim().is_empty() => model,
        _ => DEFAULT_EMBEDDING_MODEL.to_string(),
    }
}

fn resolve_embedding_mode(
    require_embeddings: bool,
    model: &str,
) -> anyhow::Result<SelectedEmbeddingMode> {
    let provider = EmbeddingProvider::from_model(model);
    let api_key = std::env::var(provider.api_key_env_var()).ok();
    resolve_embedding_mode_from_api_key(
        require_embeddings,
        api_key.as_deref(),
        provider.api_key_env_var(),
    )
}

fn resolve_embedding_mode_from_api_key(
    require_embeddings: bool,
    api_key: Option<&str>,
    api_key_env_var: &str,
) -> anyhow::Result<SelectedEmbeddingMode> {
    if api_key.is_some_and(|value| !value.trim().is_empty()) {
        return Ok(SelectedEmbeddingMode {
            mode: EmbeddingMode::Required,
            reason: None,
            require_embeddings,
        });
    }
    if require_embeddings {
        anyhow::bail!("{api_key_env_var} is required when require_embeddings=true");
    }
    Ok(SelectedEmbeddingMode {
        mode: EmbeddingMode::Skip,
        reason: Some(EMBEDDING_REASON_MISSING_API_KEY),
        require_embeddings,
    })
}

fn resolve_query_project_embedding_mode(
    blend_mode: SearchBlendMode,
    require_embeddings: bool,
    model: &str,
) -> anyhow::Result<SelectedEmbeddingMode> {
    if !blend_mode.uses_embeddings() {
        return Ok(SelectedEmbeddingMode {
            mode: EmbeddingMode::Skip,
            reason: None,
            require_embeddings: false,
        });
    }
    resolve_embedding_mode(require_embeddings, model)
}

fn full_refresh_reason(
    force_full: bool,
    embedding_mode: EmbeddingMode,
    require_embeddings: bool,
    stored_state: StoredEmbeddingState<'_>,
    embedding_model: &str,
    current_backend: &str,
) -> Option<FullRefreshReason> {
    if force_full {
        return Some(FullRefreshReason::UserRequested);
    }
    if !matches!(embedding_mode, EmbeddingMode::Required) {
        return None;
    }
    if stored_state.model != Some(embedding_model) {
        return Some(FullRefreshReason::EmbeddingModelChanged);
    }
    if stored_state.backend != Some(current_backend) {
        return Some(FullRefreshReason::VectorBackendChanged);
    }
    if !stored_state.backend_ready {
        return Some(FullRefreshReason::VectorCollectionMissing);
    }
    let current_vector_layout_version = match current_backend {
        "qdrant" => Some(QDRANT_VECTOR_LAYOUT_VERSION),
        _ => None,
    };
    if stored_state.vector_layout_version != current_vector_layout_version {
        return Some(FullRefreshReason::VectorLayoutVersionChanged);
    }
    if require_embeddings && !stored_state.ready {
        return Some(FullRefreshReason::StrictEmbeddingsNotReady);
    }
    None
}

#[cfg(test)]
fn should_force_full_refresh(
    force_full: bool,
    embedding_mode: EmbeddingMode,
    require_embeddings: bool,
    stored_state: StoredEmbeddingState<'_>,
    embedding_model: &str,
    current_backend: &str,
) -> bool {
    full_refresh_reason(
        force_full,
        embedding_mode,
        require_embeddings,
        stored_state,
        embedding_model,
        current_backend,
    )
    .is_some()
}

fn should_backfill_embeddings(embedding_mode: EmbeddingMode, stored_ready: bool) -> bool {
    matches!(embedding_mode, EmbeddingMode::Required) && !stored_ready
}

fn vector_prefilter_candidate_ids<'a>(
    blend_mode: SearchBlendMode,
    vector_backend: &VectorBackend,
    lexical_candidate_ids: &'a [i64],
) -> Option<&'a [i64]> {
    match blend_mode {
        SearchBlendMode::Hybrid(_)
            if matches!(vector_backend, VectorBackend::Local)
                && !lexical_candidate_ids.is_empty() =>
        {
            Some(lexical_candidate_ids)
        }
        SearchBlendMode::Hybrid(_) | SearchBlendMode::LexicalOnly | SearchBlendMode::VectorOnly => {
            None
        }
    }
}

fn refresh_preserves_unscanned_existing_files(
    existing_files: &HashMap<String, ExistingFile>,
    scanned_paths: &HashSet<&str>,
    glob_set: Option<&GlobSet>,
) -> bool {
    let Some(glob_set) = glob_set else {
        return false;
    };
    existing_files
        .keys()
        .any(|path| !scanned_paths.contains(path.as_str()) && !glob_set.is_match(path.as_str()))
}

fn next_embedding_ready(
    embedding_mode: EmbeddingMode,
    stored_ready: bool,
    backfill_embeddings: bool,
    preserves_unscanned_existing: bool,
    backend_ready: bool,
) -> bool {
    if !matches!(embedding_mode, EmbeddingMode::Required) {
        return false;
    }
    if !backend_ready {
        return false;
    }
    if !backfill_embeddings {
        return true;
    }
    stored_ready || !preserves_unscanned_existing
}

async fn refresh_index(
    index: &RepoHybridIndex,
    file_globs: &[String],
    force_full: bool,
    embedding_model: &str,
    embedding_mode: &mut SelectedEmbeddingMode,
) -> anyhow::Result<RefreshIndexOutcome> {
    match index
        .refresh(
            file_globs,
            force_full,
            embedding_model.to_string(),
            embedding_mode.mode,
            embedding_mode.require_embeddings,
        )
        .await
    {
        Ok(outcome) => Ok(outcome),
        Err(err)
            if matches!(embedding_mode.mode, EmbeddingMode::Required)
                && !embedding_mode.require_embeddings =>
        {
            tracing::warn!(error = %err, "embedding refresh failed; retrying without embeddings");
            let outcome = index
                .refresh(
                    file_globs,
                    force_full,
                    embedding_model.to_string(),
                    EmbeddingMode::Skip,
                    false,
                )
                .await?;
            embedding_mode.mode = EmbeddingMode::Skip;
            embedding_mode.reason = Some(EMBEDDING_REASON_QUERY_FAILED);
            Ok(outcome)
        }
        Err(err) => Err(err),
    }
}

fn parse_arguments<T>(arguments: Option<JsonObject>) -> Result<T, CallToolResult>
where
    T: for<'de> Deserialize<'de>,
{
    let Some(arguments) = arguments else {
        return Err(call_tool_error("missing tool arguments"));
    };
    serde_json::from_value::<T>(serde_json::Value::Object(arguments))
        .map_err(|err| call_tool_error(format!("failed to parse tool arguments: {err}")))
}

fn call_tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult {
        content: vec![Content::text(message.into())],
        structured_content: None,
        is_error: Some(true),
        meta: None,
    }
}

fn call_tool_success(payload: serde_json::Value) -> CallToolResult {
    let structured_content = Some(payload.clone());
    CallToolResult {
        content: vec![Content::text(payload.to_string())],
        structured_content,
        is_error: Some(false),
        meta: None,
    }
}

fn resolve_repo_root(repo_root: Option<&str>) -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir().context("failed to resolve current working directory")?;
    resolve_repo_root_from_cwd(repo_root, cwd.as_path())
}

fn resolve_repo_root_from_cwd(repo_root: Option<&str>, cwd: &Path) -> anyhow::Result<PathBuf> {
    let canonical_cwd = cwd.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize current working directory `{}`",
            cwd.display()
        )
    })?;
    let root = match repo_root {
        Some(repo_root) if !repo_root.trim().is_empty() => {
            let path = PathBuf::from(repo_root);
            if path.is_absolute() {
                path
            } else {
                canonical_cwd.join(path)
            }
        }
        _ => canonical_cwd.clone(),
    };
    let canonical_root = root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize repository root `{}`",
            root.display()
        )
    })?;
    if !canonical_root.is_dir() {
        anyhow::bail!(
            "repository root must be a directory: `{}`",
            canonical_root.display()
        );
    }
    if canonical_root != canonical_cwd && !canonical_root.starts_with(&canonical_cwd) {
        anyhow::bail!(
            "repository root `{}` must be within the current working directory `{}`",
            canonical_root.display(),
            canonical_cwd.display()
        );
    }
    Ok(canonical_root)
}

fn create_tool_input_schema(schema: schemars::schema::RootSchema) -> Arc<JsonObject> {
    let schema_value = match serde_json::to_value(schema) {
        Ok(value) => value,
        Err(err) => panic!("schema should serialize: {err}"),
    };
    let mut schema_object = match schema_value {
        serde_json::Value::Object(object) => object,
        _ => panic!("tool schema should serialize to a JSON object"),
    };

    let mut input_schema = JsonObject::new();
    for key in ["properties", "required", "type", "$defs", "definitions"] {
        if let Some(value) = schema_object.remove(key) {
            input_schema.insert(key.to_string(), value);
        }
    }
    Arc::new(input_schema)
}

struct RepoHybridIndex {
    repo_root: PathBuf,
    pool: SqlitePool,
    refresh_lock: Arc<AsyncMutex<()>>,
    embeddings_client: reqwest::Client,
    embeddings_base_url_override: Option<String>,
    embedding_api_key_override: Option<String>,
    vector_backend: VectorBackend,
}

#[derive(Clone, Debug)]
enum VectorBackend {
    Local,
    Qdrant(QdrantVectorStore),
}

#[derive(Clone)]
struct QdrantVectorStore {
    inner: QdrantVectorStoreInner,
}

#[derive(Clone)]
enum QdrantVectorStoreInner {
    Client {
        client: Qdrant,
        collection_name: String,
    },
    #[cfg(test)]
    Fake {
        collection_name: String,
        state: Arc<Mutex<FakeQdrantStoreState>>,
    },
}

impl std::fmt::Debug for QdrantVectorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QdrantVectorStore")
            .field("collection_name", &self.collection_name())
            .finish_non_exhaustive()
    }
}

impl VectorBackend {
    fn from_config(repo_root: &Path, index_config: &QueryProjectIndex) -> anyhow::Result<Self> {
        match index_config.backend {
            QueryProjectIndexBackend::Local => Ok(Self::Local),
            QueryProjectIndexBackend::Qdrant => {
                let qdrant_config = &index_config.qdrant;
                let url = qdrant_config.url.as_deref().context(
                    "query_project qdrant backend requires query_project_index.qdrant.url",
                )?;
                let timeout = Duration::from_millis(qdrant_config.timeout_ms);
                let mut builder = Qdrant::from_url(url).timeout(timeout);
                if let Ok(api_key) = std::env::var(qdrant_config.api_key_env.as_str())
                    && !api_key.trim().is_empty()
                {
                    builder = builder.api_key(api_key);
                }
                let client = builder.build().context("failed to create qdrant client")?;
                let collection_name =
                    qdrant_collection_name(repo_root, qdrant_config.collection_prefix.as_str());
                Ok(Self::Qdrant(QdrantVectorStore {
                    inner: QdrantVectorStoreInner::Client {
                        client,
                        collection_name,
                    },
                }))
            }
        }
    }

    fn qdrant_store(&self) -> Option<&QdrantVectorStore> {
        match self {
            Self::Local => None,
            Self::Qdrant(store) => Some(store),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Qdrant(_) => "qdrant",
        }
    }

    fn layout_version(&self) -> Option<&'static str> {
        match self {
            Self::Local => None,
            Self::Qdrant(_) => Some(QDRANT_VECTOR_LAYOUT_VERSION),
        }
    }
}

impl QdrantVectorStore {
    fn collection_name(&self) -> &str {
        match &self.inner {
            QdrantVectorStoreInner::Client {
                collection_name, ..
            } => collection_name,
            #[cfg(test)]
            QdrantVectorStoreInner::Fake {
                collection_name, ..
            } => collection_name,
        }
    }

    async fn collection_exists(&self) -> anyhow::Result<bool> {
        match &self.inner {
            QdrantVectorStoreInner::Client {
                client,
                collection_name,
            } => Ok(client.collection_exists(collection_name.as_str()).await?),
            #[cfg(test)]
            QdrantVectorStoreInner::Fake { state, .. } => Ok(state
                .lock()
                .map_err(|_| anyhow::anyhow!("fake qdrant state should not be poisoned"))?
                .collection_exists),
        }
    }

    async fn clear_collection(&self) -> anyhow::Result<()> {
        match &self.inner {
            QdrantVectorStoreInner::Client {
                client,
                collection_name,
            } => {
                if self.collection_exists().await? {
                    client.delete_collection(collection_name.as_str()).await?;
                }
            }
            #[cfg(test)]
            QdrantVectorStoreInner::Fake { state, .. } => {
                let mut guard = state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("fake qdrant state should not be poisoned"))?;
                guard.collection_exists = false;
                guard.points.clear();
                guard.clear_count += 1;
            }
        }
        Ok(())
    }

    async fn ensure_collection(&self, vector_size: usize, recreate: bool) -> anyhow::Result<()> {
        match &self.inner {
            QdrantVectorStoreInner::Client {
                client,
                collection_name,
            } => {
                let exists = self.collection_exists().await?;
                if recreate && exists {
                    client.delete_collection(collection_name.as_str()).await?;
                } else if exists {
                    return Ok(());
                }

                client
                    .create_collection(
                        CreateCollectionBuilder::new(collection_name.as_str()).vectors_config(
                            VectorParamsBuilder::new(vector_size as u64, Distance::Cosine),
                        ),
                    )
                    .await?;
                client
                    .create_field_index(
                        CreateFieldIndexCollectionBuilder::new(
                            collection_name.as_str(),
                            QDRANT_PAYLOAD_PATH_KEY,
                            FieldType::Keyword,
                        )
                        .wait(true),
                    )
                    .await?;
            }
            #[cfg(test)]
            QdrantVectorStoreInner::Fake { state, .. } => {
                let mut guard = state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("fake qdrant state should not be poisoned"))?;
                if recreate {
                    guard.points.clear();
                }
                guard.collection_exists = true;
                guard.ensure_dimensions.push(vector_size);
            }
        }
        Ok(())
    }

    async fn delete_path(&self, path: &str) -> anyhow::Result<()> {
        if !self.collection_exists().await? {
            return Ok(());
        }
        match &self.inner {
            QdrantVectorStoreInner::Client {
                client,
                collection_name,
            } => {
                let filter = Filter::must([Condition::matches(
                    QDRANT_PAYLOAD_PATH_KEY,
                    path.to_string(),
                )]);
                client
                    .delete_points(
                        DeletePointsBuilder::new(collection_name.as_str())
                            .points(filter)
                            .wait(true),
                    )
                    .await?;
            }
            #[cfg(test)]
            QdrantVectorStoreInner::Fake { state, .. } => {
                let mut guard = state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("fake qdrant state should not be poisoned"))?;
                guard.delete_paths.push(path.to_string());
                guard.points.retain(|_, point| point.path != path);
            }
        }
        Ok(())
    }

    async fn upsert_chunks(
        &self,
        path: &str,
        chunk_ids: &[i64],
        embeddings: &[Vec<f32>],
    ) -> anyhow::Result<()> {
        let points = chunk_ids
            .iter()
            .zip(embeddings.iter())
            .filter_map(|(chunk_id, embedding)| {
                u64::try_from(*chunk_id).ok().map(|point_id| {
                    PointStruct::new(
                        point_id,
                        embedding.clone(),
                        [(QDRANT_PAYLOAD_PATH_KEY, path.to_string().into())],
                    )
                })
            })
            .collect::<Vec<_>>();
        if points.is_empty() {
            return Ok(());
        }
        match &self.inner {
            QdrantVectorStoreInner::Client {
                client,
                collection_name,
            } => {
                client
                    .upsert_points(
                        UpsertPointsBuilder::new(collection_name.as_str(), points).wait(true),
                    )
                    .await?;
            }
            #[cfg(test)]
            QdrantVectorStoreInner::Fake { state, .. } => {
                let mut guard = state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("fake qdrant state should not be poisoned"))?;
                if !guard.collection_exists {
                    anyhow::bail!("fake qdrant collection is missing");
                }
                for (chunk_id, embedding) in chunk_ids.iter().zip(embeddings.iter()) {
                    guard.points.insert(
                        *chunk_id,
                        FakeQdrantPoint {
                            path: path.to_string(),
                            embedding: embedding.clone(),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    async fn vector_scores(
        &self,
        query_embedding: &[f32],
        limit: usize,
        glob_set: Option<&GlobSet>,
        candidate_ids: Option<&[i64]>,
    ) -> anyhow::Result<Vec<(i64, f32)>> {
        if query_embedding.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let candidate_limit = limit.saturating_mul(VECTOR_CANDIDATE_MULTIPLIER).max(limit);
        if !self.collection_exists().await? {
            return Ok(Vec::new());
        }
        let mut scores = match &self.inner {
            QdrantVectorStoreInner::Client {
                client,
                collection_name,
            } => {
                let candidate_ids = candidate_ids.map(|ids| {
                    ids.iter()
                        .filter_map(|id| u64::try_from(*id).ok())
                        .collect::<Vec<_>>()
                });
                if candidate_ids.as_ref().is_some_and(Vec::is_empty) {
                    return Ok(Vec::new());
                }

                let mut scores = Vec::with_capacity(candidate_limit);
                let mut offset = 0_u64;
                while scores.len() < candidate_limit {
                    let mut query = QueryPointsBuilder::new(collection_name.as_str())
                        .query(query_embedding.to_vec())
                        .limit(candidate_limit as u64)
                        .offset(offset)
                        .with_payload(true);

                    if let Some(candidate_ids) = candidate_ids.as_ref() {
                        query =
                            query.filter(Filter::must([Condition::has_id(candidate_ids.clone())]));
                    }

                    let response = client.query(query).await?;
                    let page_len = response.result.len();
                    if page_len == 0 {
                        break;
                    }

                    for point in response.result {
                        let Some(point_id) = point.id.and_then(|id| id.point_id_options) else {
                            continue;
                        };
                        let chunk_id = match point_id {
                            PointIdOptions::Num(id) => match i64::try_from(id) {
                                Ok(id) => id,
                                Err(_) => continue,
                            },
                            PointIdOptions::Uuid(_) => continue,
                        };
                        if let Some(glob_set) = glob_set {
                            let Some(path) = point
                                .payload
                                .get(QDRANT_PAYLOAD_PATH_KEY)
                                .and_then(|value| value.as_str())
                            else {
                                continue;
                            };
                            if !glob_set.is_match(path) {
                                continue;
                            }
                        }
                        scores.push((chunk_id, point.score));
                    }

                    if page_len < candidate_limit {
                        break;
                    }
                    offset = offset.saturating_add(page_len as u64);
                }
                scores
            }
            #[cfg(test)]
            QdrantVectorStoreInner::Fake { state, .. } => {
                let guard = state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("fake qdrant state should not be poisoned"))?;
                let candidate_ids =
                    candidate_ids.map(|ids| ids.iter().copied().collect::<HashSet<_>>());
                let mut all_scores = guard
                    .points
                    .iter()
                    .filter_map(|(chunk_id, point)| {
                        if let Some(candidate_ids) = candidate_ids.as_ref()
                            && !candidate_ids.contains(chunk_id)
                        {
                            return None;
                        }
                        Some((
                            *chunk_id,
                            cosine_similarity(query_embedding, &point.embedding),
                            point.path.clone(),
                        ))
                    })
                    .collect::<Vec<_>>();
                all_scores
                    .sort_by(|left, right| sort_score_desc(&(left.0, left.1), &(right.0, right.1)));

                let mut scores = Vec::with_capacity(candidate_limit);
                let mut offset = 0_usize;
                while scores.len() < candidate_limit {
                    let page = all_scores
                        .iter()
                        .skip(offset)
                        .take(candidate_limit)
                        .collect::<Vec<_>>();
                    if page.is_empty() {
                        break;
                    }

                    for (chunk_id, score, path) in &page {
                        if let Some(glob_set) = glob_set
                            && !glob_set.is_match(path.as_str())
                        {
                            continue;
                        }
                        scores.push((*chunk_id, *score));
                    }

                    if page.len() < candidate_limit {
                        break;
                    }
                    offset += page.len();
                }
                scores
            }
        };
        scores.sort_by(sort_score_desc);
        scores.truncate(candidate_limit);
        Ok(scores)
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct FakeQdrantPoint {
    path: String,
    embedding: Vec<f32>,
}

#[cfg(test)]
#[derive(Debug, Default)]
struct FakeQdrantStoreState {
    collection_exists: bool,
    points: HashMap<i64, FakeQdrantPoint>,
    delete_paths: Vec<String>,
    ensure_dimensions: Vec<usize>,
    clear_count: usize,
}

#[cfg(test)]
impl QdrantVectorStore {
    fn fake(collection_name: &str, state: Arc<Mutex<FakeQdrantStoreState>>) -> Self {
        Self {
            inner: QdrantVectorStoreInner::Fake {
                collection_name: collection_name.to_string(),
                state,
            },
        }
    }
}

impl RepoHybridIndex {
    async fn open(repo_root: &Path, index_config: &QueryProjectIndex) -> anyhow::Result<Self> {
        let index_dir = repo_root.join(INDEX_DIR);
        std::fs::create_dir_all(&index_dir).with_context(|| {
            format!("failed to create index directory `{}`", index_dir.display())
        })?;
        let db_path = index_dir.join(DB_FILE_NAME);
        let connect_options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(SQLITE_BUSY_TIMEOUT_SECS));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_options)
            .await
            .with_context(|| format!("failed to open SQLite DB `{}`", db_path.display()))?;
        let embeddings_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(EMBEDDING_REQUEST_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(EMBEDDING_CONNECT_TIMEOUT_SECS))
            .build()
            .context("failed to initialize embeddings client")?;
        static REFRESH_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>> =
            OnceLock::new();
        let refresh_lock = {
            let locks = REFRESH_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
            let mut guard = locks.lock().map_err(|_| {
                anyhow::anyhow!("repo index refresh lock map should not be poisoned")
            })?;
            match guard.get(repo_root) {
                Some(lock) => Arc::clone(lock),
                None => {
                    let lock = Arc::new(AsyncMutex::new(()));
                    guard.insert(repo_root.to_path_buf(), Arc::clone(&lock));
                    lock
                }
            }
        };
        let index = Self {
            repo_root: repo_root.to_path_buf(),
            pool,
            refresh_lock,
            embeddings_client,
            embeddings_base_url_override: None,
            embedding_api_key_override: None,
            vector_backend: VectorBackend::from_config(repo_root, index_config)?,
        };
        index.ensure_schema().await?;
        Ok(index)
    }

    async fn ensure_schema(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS indexed_files (
                path TEXT PRIMARY KEY,
                modified_sec INTEGER NOT NULL,
                modified_nsec INTEGER NOT NULL DEFAULT 0,
                size_bytes INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        let columns = sqlx::query("PRAGMA table_info(indexed_files)")
            .fetch_all(&self.pool)
            .await?;
        let mut has_modified_nsec = false;
        for row in &columns {
            let name: String = row.try_get("name")?;
            if name == "modified_nsec" {
                has_modified_nsec = true;
                break;
            }
        }
        if !has_modified_nsec {
            sqlx::query(
                "ALTER TABLE indexed_files ADD COLUMN modified_nsec INTEGER NOT NULL DEFAULT 0",
            )
            .execute(&self.pool)
            .await?;
        }

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                snippet TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_chunks_path ON chunks(path)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(content, path UNINDEXED, chunk_id UNINDEXED)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS index_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn load_metadata(&self, key: &str) -> anyhow::Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM index_metadata WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        let value = row
            .map(|row| row.try_get::<String, _>("value"))
            .transpose()?;
        Ok(value)
    }

    async fn set_metadata(&self, key: &str, value: &str) -> anyhow::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO index_metadata(key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn embedding_ready(&self) -> anyhow::Result<bool> {
        Ok(self
            .load_metadata(METADATA_EMBEDDING_READY)
            .await?
            .as_deref()
            .is_some_and(|value| value == "true"))
    }

    async fn vector_backend_ready(&self, indexed_chunk_count: usize) -> anyhow::Result<bool> {
        if indexed_chunk_count == 0 {
            return Ok(true);
        }
        match self.vector_backend.qdrant_store() {
            Some(vector_store) => vector_store.collection_exists().await,
            None => Ok(true),
        }
    }

    async fn refresh(
        &self,
        file_globs: &[String],
        force_full: bool,
        embedding_model: String,
        embedding_mode: EmbeddingMode,
        require_embeddings: bool,
    ) -> anyhow::Result<RefreshIndexOutcome> {
        let _refresh_guard = self.refresh_lock.lock().await;
        let is_scoped_refresh = !file_globs.is_empty();
        if force_full && is_scoped_refresh {
            anyhow::bail!(
                "force_full cannot be combined with file_globs; rerun without file_globs to rebuild the full index"
            );
        }
        let glob_set = build_glob_set(file_globs)?;
        let stored_model = self.load_metadata(METADATA_EMBEDDING_MODEL).await?;
        let stored_ready = self.embedding_ready().await?;
        let stored_backend = self.load_metadata(METADATA_VECTOR_BACKEND).await?;
        let stored_vector_layout_version =
            self.load_metadata(METADATA_VECTOR_LAYOUT_VERSION).await?;
        let stored_chunk_count = self.count_chunks().await?;
        let current_backend = self.vector_backend.name();
        let backend_ready = self.vector_backend_ready(stored_chunk_count).await?;
        let stored_state = StoredEmbeddingState {
            model: stored_model.as_deref(),
            ready: stored_ready,
            backend: stored_backend.as_deref(),
            backend_ready,
            vector_layout_version: stored_vector_layout_version
                .as_deref()
                .filter(|value| !value.is_empty()),
        };
        let force_full_reason = full_refresh_reason(
            force_full,
            embedding_mode,
            require_embeddings,
            stored_state,
            embedding_model.as_str(),
            current_backend,
        );
        let scoped_full_corpus_repair_reason =
            force_full_reason.filter(|_| is_scoped_refresh && !force_full);
        if let Some(reason) = scoped_full_corpus_repair_reason {
            tracing::info!(
                ?reason,
                repo_root = %self.repo_root.display(),
                current_backend,
                "query_project scoped refresh detected a full-corpus repair requirement; preserving existing index data and leaving embeddings not ready"
            );
            if require_embeddings {
                let reason_detail = match reason {
                    FullRefreshReason::UserRequested => "a full refresh was explicitly requested",
                    FullRefreshReason::EmbeddingModelChanged => "the embedding model changed",
                    FullRefreshReason::VectorBackendChanged => "the vector backend changed",
                    FullRefreshReason::VectorCollectionMissing => {
                        "the vector collection is missing"
                    }
                    FullRefreshReason::VectorLayoutVersionChanged => {
                        "the vector layout version changed"
                    }
                    FullRefreshReason::StrictEmbeddingsNotReady => {
                        "the index is not embedding-ready"
                    }
                };
                anyhow::bail!(
                    "scoped refresh cannot satisfy require_embeddings=true because {reason_detail}; rerun without file_globs"
                );
            }
        }
        let force_full = force_full_reason.is_some() && !is_scoped_refresh;
        if let Some(reason) = force_full_reason
            && force_full
        {
            tracing::info!(
                ?reason,
                repo_root = %self.repo_root.display(),
                current_backend,
                "query_project refresh running full rebuild"
            );
        }
        let backfill_embeddings = !force_full
            && (should_backfill_embeddings(embedding_mode, stored_ready)
                || scoped_full_corpus_repair_reason.is_some());
        let skip_vector_writes = matches!(
            scoped_full_corpus_repair_reason,
            Some(FullRefreshReason::VectorLayoutVersionChanged)
        );
        let vector_store =
            if matches!(embedding_mode, EmbeddingMode::Required) && !skip_vector_writes {
                self.vector_backend.qdrant_store()
            } else {
                None
            };
        if force_full {
            self.clear_all().await?;
            if let Some(vector_store) = vector_store {
                vector_store.clear_collection().await?;
            }
        }

        let repo_root = self.repo_root.clone();
        let scan_file_globs = file_globs.to_vec();
        let scanned_files = tokio::task::spawn_blocking(move || {
            let scan_glob_set = build_glob_set(&scan_file_globs)?;
            scan_repo(&repo_root, scan_glob_set.as_ref())
        })
        .await
        .context("repo scan task failed")??;
        let existing_files = self.load_existing_files().await?;

        let mut stats = RepoIndexRefreshStats {
            scanned_files: scanned_files.len(),
            updated_files: 0,
            removed_files: 0,
            indexed_chunks: 0,
        };
        let mut qdrant_collection_initialized = false;

        let scanned_paths: HashSet<&str> = scanned_files.keys().map(String::as_str).collect();
        let preserves_unscanned_existing = refresh_preserves_unscanned_existing_files(
            &existing_files,
            &scanned_paths,
            glob_set.as_ref(),
        );
        for (path, _existing) in existing_files
            .iter()
            .filter(|(path, _)| !scanned_paths.contains(path.as_str()))
        {
            if let Some(glob_set) = glob_set.as_ref()
                && !glob_set.is_match(path.as_str())
            {
                continue;
            }
            let mut tx = self.pool.begin().await?;
            remove_file_from_index(&mut tx, path).await?;
            if let Some(vector_store) = vector_store {
                vector_store.delete_path(path).await?;
            }
            tx.commit().await?;
            stats.removed_files += 1;
        }

        for (path, scanned) in &scanned_files {
            let unchanged = existing_files.get(path).is_some_and(|existing| {
                existing.modified_sec == scanned.modified_sec
                    && existing.modified_nsec == scanned.modified_nsec
                    && existing.size_bytes == scanned.size_bytes
            });
            if unchanged && !backfill_embeddings {
                continue;
            }

            let file_text = match read_text_file(&scanned.absolute_path).await? {
                Some(file_text) => file_text,
                None => {
                    let mut tx = self.pool.begin().await?;
                    remove_file_from_index(&mut tx, path).await?;
                    if let Some(vector_store) = vector_store {
                        vector_store.delete_path(path).await?;
                    }
                    tx.commit().await?;
                    continue;
                }
            };

            let chunks = chunk_text(&file_text);
            if chunks.is_empty() {
                let mut tx = self.pool.begin().await?;
                remove_file_from_index(&mut tx, path).await?;
                sqlx::query(
                    "INSERT OR REPLACE INTO indexed_files(path, modified_sec, modified_nsec, size_bytes) VALUES (?, ?, ?, ?)",
                )
                .bind(path)
                .bind(scanned.modified_sec)
                .bind(scanned.modified_nsec)
                .bind(scanned.size_bytes)
                .execute(&mut *tx)
                .await?;
                if let Some(vector_store) = vector_store {
                    vector_store.delete_path(path).await?;
                }
                tx.commit().await?;
                stats.updated_files += 1;
                continue;
            }

            let embeddings = if matches!(embedding_mode, EmbeddingMode::Required) {
                let inputs = chunks
                    .iter()
                    .map(|chunk| chunk.content.clone())
                    .collect::<Vec<_>>();
                self.embed_texts(&embedding_model, &inputs).await?
            } else {
                vec![Vec::new(); chunks.len()]
            };
            if embeddings.len() != chunks.len() {
                anyhow::bail!(
                    "embedding service returned {} vectors for {} chunks",
                    embeddings.len(),
                    chunks.len()
                );
            }

            let mut tx = self.pool.begin().await?;
            remove_file_from_index(&mut tx, path).await?;
            sqlx::query(
                "INSERT OR REPLACE INTO indexed_files(path, modified_sec, modified_nsec, size_bytes) VALUES (?, ?, ?, ?)",
            )
            .bind(path)
            .bind(scanned.modified_sec)
            .bind(scanned.modified_nsec)
            .bind(scanned.size_bytes)
            .execute(&mut *tx)
            .await?;

            let mut inserted_chunk_ids = Vec::with_capacity(embeddings.len());
            for (chunk, embedding) in chunks.into_iter().zip(embeddings.iter()) {
                let embedding_json = serde_json::to_string(&embedding)?;
                let content = chunk.content;
                let insert_result = sqlx::query(
                    "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(path)
                .bind(chunk.start_line as i64)
                .bind(chunk.end_line as i64)
                .bind(chunk.snippet)
                .bind(&content)
                .bind(embedding_json)
                .execute(&mut *tx)
                .await?;
                let chunk_id = insert_result.last_insert_rowid();
                inserted_chunk_ids.push(chunk_id);
                sqlx::query(
                    "INSERT INTO chunks_fts(rowid, content, path, chunk_id) VALUES (?, ?, ?, ?)",
                )
                .bind(chunk_id)
                .bind(content)
                .bind(path)
                .bind(chunk_id)
                .execute(&mut *tx)
                .await?;
            }
            if let Some(vector_store) = vector_store {
                vector_store.delete_path(path).await?;
                if !qdrant_collection_initialized
                    && let Some(dimension) = embeddings.first().map(Vec::len)
                {
                    vector_store.ensure_collection(dimension, false).await?;
                    qdrant_collection_initialized = true;
                }
                vector_store
                    .upsert_chunks(path, inserted_chunk_ids.as_slice(), embeddings.as_slice())
                    .await?;
            }
            tx.commit().await?;
            stats.updated_files += 1;
        }

        stats.indexed_chunks = self.count_chunks().await?;
        self.set_metadata(METADATA_EMBEDDING_MODEL, &embedding_model)
            .await?;
        self.set_metadata(METADATA_VECTOR_BACKEND, current_backend)
            .await?;
        let backend_ready = self.vector_backend_ready(stats.indexed_chunks).await?;
        match self.vector_backend.layout_version() {
            Some(layout_version)
                if matches!(embedding_mode, EmbeddingMode::Required)
                    && file_globs.is_empty()
                    && backend_ready =>
            {
                self.set_metadata(METADATA_VECTOR_LAYOUT_VERSION, layout_version)
                    .await?;
            }
            Some(_) => {}
            None => {
                self.set_metadata(METADATA_VECTOR_LAYOUT_VERSION, "")
                    .await?;
            }
        }
        let ready = if scoped_full_corpus_repair_reason.is_some() {
            false
        } else {
            next_embedding_ready(
                embedding_mode,
                stored_ready,
                backfill_embeddings,
                preserves_unscanned_existing,
                backend_ready,
            )
        };
        self.set_metadata(
            METADATA_EMBEDDING_READY,
            if ready { "true" } else { "false" },
        )
        .await?;
        Ok(RefreshIndexOutcome { stats, ready })
    }

    async fn clear_all(&self) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM chunks_fts")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM chunks").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM indexed_files")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn load_existing_files(&self) -> anyhow::Result<HashMap<String, ExistingFile>> {
        let rows =
            sqlx::query("SELECT path, modified_sec, modified_nsec, size_bytes FROM indexed_files")
                .fetch_all(&self.pool)
                .await?;
        let mut files = HashMap::with_capacity(rows.len());
        for row in rows {
            let path: String = row.try_get("path")?;
            let modified_sec: i64 = row.try_get("modified_sec")?;
            let modified_nsec: i64 = row.try_get("modified_nsec")?;
            let size_bytes: i64 = row.try_get("size_bytes")?;
            files.insert(
                path,
                ExistingFile {
                    modified_sec,
                    modified_nsec,
                    size_bytes,
                },
            );
        }
        Ok(files)
    }

    async fn count_chunks(&self) -> anyhow::Result<usize> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM chunks")
            .fetch_one(&self.pool)
            .await?;
        let count: i64 = row.try_get("count")?;
        Ok(count as usize)
    }

    async fn search(
        &self,
        query: &str,
        limit: usize,
        blend_mode: SearchBlendMode,
        file_globs: &[String],
        embedding_model: String,
        require_embeddings: bool,
    ) -> anyhow::Result<SearchOutcome> {
        let glob_set = build_glob_set(file_globs)?;
        let lexical_scores = if blend_mode.uses_lexical() {
            self.lexical_scores(
                query,
                limit.saturating_mul(LEXICAL_CANDIDATE_MULTIPLIER),
                glob_set.as_ref(),
            )
            .await?
        } else {
            HashMap::new()
        };
        let lexical_candidate_ids = lexical_scores.keys().copied().collect::<Vec<_>>();

        let mut vector_scores = Vec::new();
        let mut embedding_fallback_reason = None;
        if blend_mode.uses_embeddings() {
            let embedding_ready = self.embedding_ready().await?;
            let indexed_chunk_count = if embedding_ready {
                self.count_chunks().await?
            } else {
                0
            };
            let backend_ready = if embedding_ready {
                self.vector_backend_ready(indexed_chunk_count).await?
            } else {
                false
            };
            if require_embeddings && (!embedding_ready || !backend_ready) {
                anyhow::bail!("embeddings are required but the index is not embedding-ready");
            }
            if embedding_ready && backend_ready {
                match self
                    .embed_texts(&embedding_model, &[query.to_string()])
                    .await
                {
                    Ok(embeddings) => {
                        let (query_embedding, fallback_reason) =
                            query_embedding_or_fallback_reason(embeddings);
                        if let Some(query_embedding) = query_embedding {
                            let vector_prefilter = vector_prefilter_candidate_ids(
                                blend_mode,
                                &self.vector_backend,
                                lexical_candidate_ids.as_slice(),
                            );
                            match self
                                .vector_scores(
                                    &query_embedding,
                                    limit,
                                    glob_set.as_ref(),
                                    vector_prefilter,
                                )
                                .await
                            {
                                Ok(scores) => {
                                    vector_scores = scores;
                                }
                                Err(err) => {
                                    if require_embeddings {
                                        return Err(err).context(
                                            "failed to query vector backend in strict mode",
                                        );
                                    }
                                    embedding_fallback_reason = Some(EMBEDDING_REASON_QUERY_FAILED);
                                }
                            }
                        } else {
                            if require_embeddings {
                                anyhow::bail!(
                                    "embeddings are required but query embedding was empty"
                                );
                            }
                            embedding_fallback_reason = fallback_reason;
                        }
                    }
                    Err(err) => {
                        if require_embeddings {
                            return Err(err).context("failed to embed query in strict mode");
                        }
                        embedding_fallback_reason = Some(EMBEDDING_REASON_QUERY_FAILED);
                    }
                }
            } else if embedding_ready {
                embedding_fallback_reason = Some(EMBEDDING_REASON_UNAVAILABLE);
            }
        }

        if vector_scores.is_empty() && lexical_scores.is_empty() {
            return Ok(SearchOutcome {
                results: Vec::new(),
                embedding_fallback_reason,
            });
        }

        let lexical_score_pairs = lexical_scores.into_iter().collect::<Vec<_>>();
        let normalized_vector = normalize_scores(&vector_scores);
        let normalized_lexical = normalize_scores(&lexical_score_pairs);

        let mut candidate_ids = HashSet::new();
        match blend_mode {
            SearchBlendMode::LexicalOnly => {
                candidate_ids.extend(normalized_lexical.keys().copied());
            }
            SearchBlendMode::Hybrid(_) => {
                candidate_ids.extend(normalized_vector.keys().copied());
                candidate_ids.extend(normalized_lexical.keys().copied());
            }
            SearchBlendMode::VectorOnly => {
                candidate_ids.extend(normalized_vector.keys().copied());
            }
        }
        let candidate_ids = candidate_ids.into_iter().collect::<Vec<_>>();
        let chunks = self.load_chunks_by_ids(&candidate_ids).await?;

        let mut merged = candidate_ids
            .into_iter()
            .filter(|chunk_id| chunks.contains_key(chunk_id))
            .map(|chunk_id| {
                let vector_score = normalized_vector
                    .get(&chunk_id)
                    .copied()
                    .unwrap_or_default();
                let lexical_score = normalized_lexical
                    .get(&chunk_id)
                    .copied()
                    .unwrap_or_default();
                let score = blend_mode.score(vector_score, lexical_score);
                (chunk_id, score)
            })
            .collect::<Vec<_>>();
        merged.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });
        merged.truncate(limit);

        let mut results = Vec::with_capacity(merged.len());
        for (chunk_id, score) in merged {
            if let Some(chunk) = chunks.get(&chunk_id) {
                results.push(RepoHybridSearchResultItem {
                    path: chunk.path.clone(),
                    line_range: LineRange {
                        start: chunk.start_line,
                        end: chunk.end_line,
                    },
                    snippet: chunk.snippet.clone(),
                    score: round_score(score),
                });
            }
        }
        Ok(SearchOutcome {
            results,
            embedding_fallback_reason,
        })
    }

    async fn embed_texts(&self, model: &str, inputs: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let provider = EmbeddingProvider::from_model(model);
        let api_key_env_var = provider.api_key_env_var();
        let api_key = self
            .embedding_api_key_override
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .or_else(|| {
                std::env::var(api_key_env_var)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .with_context(|| {
                format!("{api_key_env_var} is required for query_project embeddings")
            })?;
        let embeddings_url = provider.embeddings_url(self.embeddings_base_url_override.as_deref());

        let mut all_embeddings = Vec::<Vec<f32>>::with_capacity(inputs.len());
        for batch in inputs.chunks(EMBED_BATCH_SIZE) {
            let clamped_batch: Vec<String> = batch
                .iter()
                .map(|s| {
                    if s.trim().is_empty() {
                        // Azure rejects empty inputs; use a single space as a
                        // minimal placeholder so the batch size stays aligned.
                        " ".to_string()
                    } else if s.len() <= EMBEDDING_MAX_INPUT_BYTES {
                        s.clone()
                    } else {
                        let mut end = EMBEDDING_MAX_INPUT_BYTES;
                        while !s.is_char_boundary(end) && end > 0 {
                            end -= 1;
                        }
                        s[..end].to_string()
                    }
                })
                .collect();
            let request_body = EmbeddingsRequestBody {
                model: model.to_string(),
                input: clamped_batch,
            };
            let mut retries = 0u32;
            let response = loop {
                let resp = self
                    .embeddings_client
                    .post(&embeddings_url)
                    .bearer_auth(&api_key)
                    .header("api-key", &api_key)
                    .json(&request_body)
                    .send()
                    .await?;
                if resp.status() == StatusCode::TOO_MANY_REQUESTS
                    && retries < EMBEDDING_RATE_LIMIT_RETRIES
                {
                    let retry_after = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(1 << retries);
                    tracing::warn!(
                        retry_after,
                        retries,
                        "embedding request rate-limited, retrying"
                    );
                    tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    retries += 1;
                    continue;
                }
                break resp;
            };
            if response.status() != StatusCode::OK {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                let empty_count = batch.iter().filter(|s| s.is_empty()).count();
                let max_len = batch
                    .iter()
                    .map(std::string::String::len)
                    .max()
                    .unwrap_or(0);
                anyhow::bail!(
                    "embedding request failed with status {status} (batch_size={}, empty_inputs={empty_count}, max_input_len={max_len}): {body}",
                    batch.len()
                );
            }

            let mut parsed = response.json::<EmbeddingsResponseBody>().await?;
            parsed.data.sort_by_key(|item| item.index);
            all_embeddings.extend(parsed.data.into_iter().map(|item| item.embedding));
        }
        Ok(all_embeddings)
    }

    async fn vector_scores(
        &self,
        query_embedding: &[f32],
        limit: usize,
        glob_set: Option<&GlobSet>,
        candidate_ids: Option<&[i64]>,
    ) -> anyhow::Result<Vec<(i64, f32)>> {
        if let Some(vector_store) = self.vector_backend.qdrant_store() {
            return vector_store
                .vector_scores(query_embedding, limit, glob_set, candidate_ids)
                .await;
        }
        self.vector_scores_local(query_embedding, limit, glob_set, candidate_ids)
            .await
    }

    async fn vector_scores_local(
        &self,
        query_embedding: &[f32],
        limit: usize,
        glob_set: Option<&GlobSet>,
        candidate_ids: Option<&[i64]>,
    ) -> anyhow::Result<Vec<(i64, f32)>> {
        let candidate_limit = limit.saturating_mul(VECTOR_CANDIDATE_MULTIPLIER).max(limit);
        let buffer_limit = candidate_limit.saturating_mul(4).max(candidate_limit);
        let mut scores = Vec::new();

        if let Some(candidate_ids) = candidate_ids {
            for id_chunk in candidate_ids.chunks(SQLITE_BIND_CHUNK_SIZE) {
                let mut builder =
                    QueryBuilder::new("SELECT id, path, embedding FROM chunks WHERE id IN (");
                let mut separated = builder.separated(", ");
                for id in id_chunk {
                    separated.push_bind(id);
                }
                separated.push_unseparated(") ORDER BY id ASC");
                let rows = builder.build().fetch_all(&self.pool).await?;
                for row in rows {
                    let id: i64 = row.try_get("id")?;
                    let path: String = row.try_get("path")?;
                    if let Some(glob_set) = glob_set
                        && !glob_set.is_match(path.as_str())
                    {
                        continue;
                    }
                    let embedding_json: String = row.try_get("embedding")?;
                    let embedding = serde_json::from_str::<Vec<f32>>(&embedding_json)
                        .with_context(|| {
                            format!("failed to parse embedding JSON for chunk id {id}")
                        })?;
                    let score = cosine_similarity(query_embedding, &embedding);
                    scores.push((id, score));
                    if scores.len() > buffer_limit {
                        scores.sort_by(sort_score_desc);
                        scores.truncate(candidate_limit);
                    }
                }
            }
        } else {
            let mut rows = sqlx::query("SELECT id, path, embedding FROM chunks ORDER BY id ASC")
                .fetch(&self.pool);
            while let Some(row) = rows.try_next().await? {
                let id: i64 = row.try_get("id")?;
                let path: String = row.try_get("path")?;
                if let Some(glob_set) = glob_set
                    && !glob_set.is_match(path.as_str())
                {
                    continue;
                }
                let embedding_json: String = row.try_get("embedding")?;
                let embedding = serde_json::from_str::<Vec<f32>>(&embedding_json)
                    .with_context(|| format!("failed to parse embedding JSON for chunk id {id}"))?;
                let score = cosine_similarity(query_embedding, &embedding);
                scores.push((id, score));
                if scores.len() > buffer_limit {
                    scores.sort_by(sort_score_desc);
                    scores.truncate(candidate_limit);
                }
            }
        }
        scores.sort_by(sort_score_desc);
        scores.truncate(candidate_limit);
        Ok(scores)
    }

    async fn load_chunks_by_ids(&self, ids: &[i64]) -> anyhow::Result<HashMap<i64, ChunkRecord>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut chunks = HashMap::new();
        for id_chunk in ids.chunks(SQLITE_BIND_CHUNK_SIZE) {
            let mut builder = QueryBuilder::new(
                "SELECT id, path, start_line, end_line, snippet FROM chunks WHERE id IN (",
            );
            let mut separated = builder.separated(", ");
            for id in id_chunk {
                separated.push_bind(id);
            }
            separated.push_unseparated(")");
            let rows = builder.build().fetch_all(&self.pool).await?;
            for row in rows {
                let id: i64 = row.try_get("id")?;
                let path: String = row.try_get("path")?;
                let start_line: i64 = row.try_get("start_line")?;
                let end_line: i64 = row.try_get("end_line")?;
                let snippet: String = row.try_get("snippet")?;
                chunks.insert(
                    id,
                    ChunkRecord {
                        path,
                        start_line: start_line as usize,
                        end_line: end_line as usize,
                        snippet,
                    },
                );
            }
        }
        Ok(chunks)
    }

    async fn lexical_scores(
        &self,
        query: &str,
        limit: usize,
        glob_set: Option<&GlobSet>,
    ) -> anyhow::Result<HashMap<i64, f32>> {
        let query_for_fts = to_fts_query(query);
        let rows = sqlx::query(
            "SELECT chunk_id, path, bm25(chunks_fts) AS rank FROM chunks_fts WHERE chunks_fts MATCH ? ORDER BY rank LIMIT ?",
        )
        .bind(query_for_fts)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await;

        match rows {
            Ok(rows) => {
                let mut scores = HashMap::with_capacity(rows.len());
                for row in rows {
                    let chunk_id: i64 = row.try_get("chunk_id")?;
                    let rank: f64 = row.try_get("rank")?;
                    if let Some(glob_set) = glob_set {
                        let path: String = row.try_get("path")?;
                        if !glob_set.is_match(path.as_str()) {
                            continue;
                        }
                    }
                    scores.insert(chunk_id, -(rank as f32));
                }
                Ok(scores)
            }
            Err(_) => {
                self.lexical_scores_with_ripgrep(query, limit, glob_set)
                    .await
            }
        }
    }

    async fn lexical_scores_with_ripgrep(
        &self,
        query: &str,
        limit: usize,
        glob_set: Option<&GlobSet>,
    ) -> anyhow::Result<HashMap<i64, f32>> {
        let mut command = Command::new("rg");
        command
            .current_dir(&self.repo_root)
            .arg("--line-number")
            .arg("--no-heading")
            .arg("--color")
            .arg("never")
            .arg("--max-count")
            .arg(FALLBACK_RG_LIMIT.to_string())
            .arg("--")
            .arg(query)
            .arg(".");
        let output = match command.output().await {
            Ok(output) => output,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Ok(HashMap::new());
            }
            Err(err) => return Err(err.into()),
        };
        if output.status.code() == Some(1) {
            return Ok(HashMap::new());
        }
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ripgrep lexical fallback failed: {stderr}");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut scores_by_chunk = HashMap::<i64, f32>::new();
        for line in stdout.lines() {
            let mut parts = line.splitn(3, ':');
            let Some(path) = parts.next() else {
                continue;
            };
            let Some(line_number_raw) = parts.next() else {
                continue;
            };
            let Ok(line_number) = line_number_raw.parse::<i64>() else {
                continue;
            };

            let normalized_path = normalize_rel_path(path);
            if let Some(glob_set) = glob_set
                && !glob_set.is_match(normalized_path.as_str())
            {
                continue;
            }

            let row = sqlx::query(
                "SELECT id FROM chunks WHERE path = ? AND start_line <= ? AND end_line >= ? LIMIT 1",
            )
            .bind(&normalized_path)
            .bind(line_number)
            .bind(line_number)
            .fetch_optional(&self.pool)
            .await?;
            let Some(row) = row else {
                continue;
            };
            let chunk_id: i64 = row.try_get("id")?;
            *scores_by_chunk.entry(chunk_id).or_insert(0.0) += 1.0;
        }

        let mut score_pairs = scores_by_chunk.into_iter().collect::<Vec<_>>();
        score_pairs.sort_by(sort_score_desc);
        score_pairs.truncate(limit);
        Ok(score_pairs.into_iter().collect())
    }
}

async fn remove_file_from_index(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    path: &str,
) -> anyhow::Result<usize> {
    sqlx::query("DELETE FROM chunks_fts WHERE rowid IN (SELECT id FROM chunks WHERE path = ?)")
        .bind(path)
        .execute(&mut **tx)
        .await?;
    let removed_chunk_count = sqlx::query("DELETE FROM chunks WHERE path = ?")
        .bind(path)
        .execute(&mut **tx)
        .await?
        .rows_affected() as usize;
    sqlx::query("DELETE FROM indexed_files WHERE path = ?")
        .bind(path)
        .execute(&mut **tx)
        .await?;
    Ok(removed_chunk_count)
}

fn build_glob_set(file_globs: &[String]) -> anyhow::Result<Option<GlobSet>> {
    if file_globs.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    let mut has_globs = false;
    for file_glob in file_globs {
        let trimmed = file_glob.trim();
        if trimmed.is_empty() {
            continue;
        }
        builder.add(Glob::new(trimmed).with_context(|| format!("invalid glob `{trimmed}`"))?);
        has_globs = true;
    }
    if !has_globs {
        return Ok(None);
    }
    Ok(Some(builder.build()?))
}

fn scan_repo(
    repo_root: &Path,
    glob_set: Option<&GlobSet>,
) -> anyhow::Result<HashMap<String, ScannedFile>> {
    let mut files = HashMap::new();
    let mut walker = WalkBuilder::new(repo_root);
    walker
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .parents(true)
        .require_git(false);

    for entry in walker.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }

        let absolute_path = entry.path().to_path_buf();
        let relative_path = match absolute_path.strip_prefix(repo_root) {
            Ok(relative_path) => normalize_rel_path(relative_path.to_string_lossy().as_ref()),
            Err(_) => continue,
        };
        if should_skip_index_path(relative_path.as_str()) {
            continue;
        }
        if let Some(glob_set) = glob_set
            && !glob_set.is_match(relative_path.as_str())
        {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.len() > MAX_FILE_SIZE_BYTES {
            continue;
        }
        let (modified_sec, modified_nsec) = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| (duration.as_secs() as i64, duration.subsec_nanos() as i64))
            .unwrap_or((0, 0));
        files.insert(
            relative_path,
            ScannedFile {
                absolute_path,
                modified_sec,
                modified_nsec,
                size_bytes: metadata.len() as i64,
            },
        );
    }
    Ok(files)
}

fn should_skip_index_path(path: &str) -> bool {
    path.starts_with(".git/")
        || path == ".git"
        || path.starts_with("target/")
        || path.starts_with("node_modules/")
        || path.starts_with(".codex/repo_hybrid_index/")
}

fn qdrant_collection_name(repo_root: &Path, prefix: &str) -> String {
    let mut sanitized_prefix = prefix
        .trim()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => ch,
            _ => '_',
        })
        .collect::<String>();
    if sanitized_prefix.is_empty() {
        sanitized_prefix = "codex_repo_".to_string();
    }
    if !sanitized_prefix.ends_with('_') && !sanitized_prefix.ends_with('-') {
        sanitized_prefix.push('_');
    }
    if sanitized_prefix.len() > 64 {
        sanitized_prefix.truncate(64);
    }
    let mut hasher = Sha1::new();
    hasher.update(repo_root.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    format!("{sanitized_prefix}{digest:x}")
}

async fn read_text_file(path: &Path) -> anyhow::Result<Option<String>> {
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("failed to read file `{}`", path.display()))?;
    if bytes.contains(&0) {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&bytes).to_string()))
}

fn chunk_text(file_text: &str) -> Vec<ChunkDraft> {
    let lines = file_text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Vec::new();
    }

    let step = CHUNK_LINE_COUNT.saturating_sub(CHUNK_LINE_OVERLAP).max(1);
    let mut start_index = 0;
    let mut chunks = Vec::new();

    while start_index < lines.len() {
        let end_index = (start_index + CHUNK_LINE_COUNT).min(lines.len());
        let chunk_lines = &lines[start_index..end_index];
        let snippet = chunk_lines
            .iter()
            .take(SNIPPET_LINE_COUNT)
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        let content = chunk_lines.join("\n");
        chunks.push(ChunkDraft {
            start_line: start_index + 1,
            end_line: end_index,
            content,
            snippet,
        });
        if end_index == lines.len() {
            break;
        }
        start_index += step;
    }

    chunks
}

fn normalize_rel_path(path: &str) -> String {
    path.trim_start_matches("./").replace('\\', "/")
}

fn to_fts_query(query: &str) -> String {
    let terms = query
        .split_whitespace()
        .map(|part| part.trim_matches('"'))
        .filter(|part| !part.is_empty())
        .map(|part| format!("\"{part}\""))
        .collect::<Vec<_>>();
    if terms.is_empty() {
        "\"\"".to_string()
    } else {
        terms.join(" AND ")
    }
}

fn normalize_scores(scores: &[(i64, f32)]) -> HashMap<i64, f32> {
    if scores.is_empty() {
        return HashMap::new();
    }
    let (min_score, max_score) = scores.iter().fold(
        (f32::MAX, f32::MIN),
        |(min_score, max_score), (_, score)| (min_score.min(*score), max_score.max(*score)),
    );
    if (max_score - min_score).abs() < f32::EPSILON {
        return scores.iter().map(|(id, _)| (*id, 1.0)).collect();
    }
    scores
        .iter()
        .map(|(id, score)| (*id, (*score - min_score) / (max_score - min_score)))
        .collect()
}

fn query_embedding_or_fallback_reason(
    mut embeddings: Vec<Vec<f32>>,
) -> (Option<Vec<f32>>, Option<&'static str>) {
    match embeddings.pop() {
        Some(embedding) => (Some(embedding), None),
        None => (None, Some(EMBEDDING_REASON_QUERY_FAILED)),
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || right.is_empty() || left.len() != right.len() {
        return 0.0;
    }
    let (dot, norm_left, norm_right) =
        left.iter()
            .zip(right.iter())
            .fold((0.0_f32, 0.0_f32, 0.0_f32), |acc, (left, right)| {
                let (dot, norm_left, norm_right) = acc;
                (
                    dot + (left * right),
                    norm_left + (left * left),
                    norm_right + (right * right),
                )
            });
    if norm_left <= f32::EPSILON || norm_right <= f32::EPSILON {
        return 0.0;
    }
    dot / (norm_left.sqrt() * norm_right.sqrt())
}

fn round_score(score: f32) -> f32 {
    (score * 10_000.0).round() / 10_000.0
}

fn sort_score_desc(left: &(i64, f32), right: &(i64, f32)) -> Ordering {
    right
        .1
        .partial_cmp(&left.1)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.0.cmp(&right.0))
}

#[derive(Serialize)]
struct EmbeddingsRequestBody {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingsResponseBody {
    data: Vec<EmbeddingItem>,
}

#[derive(Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
    index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering as AtomicOrdering;
    use tempfile::tempdir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::Respond;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    struct EmbeddingsSeqResponder {
        responses: Vec<Vec<Vec<f32>>>,
        next_response: AtomicUsize,
    }

    impl Respond for EmbeddingsSeqResponder {
        fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
            let index = self.next_response.fetch_add(1, AtomicOrdering::SeqCst);
            let embeddings = self
                .responses
                .get(index)
                .unwrap_or_else(|| panic!("missing embedding response for request {index}"));
            let body = json!({
                "data": embeddings
                    .iter()
                    .enumerate()
                    .map(|(embedding_index, embedding)| {
                        json!({
                            "embedding": embedding,
                            "index": embedding_index,
                        })
                    })
                    .collect::<Vec<_>>(),
            });
            ResponseTemplate::new(200).set_body_json(body)
        }
    }

    async fn mount_openai_embeddings(server: &MockServer, responses: Vec<Vec<Vec<f32>>>) {
        let responder = EmbeddingsSeqResponder {
            next_response: AtomicUsize::new(0),
            responses,
        };
        let expected_calls = responder.responses.len() as u64;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(responder)
            .expect(expected_calls)
            .mount(server)
            .await;
    }

    async fn open_index_with_fake_qdrant(
        repo_root: &Path,
        state: Arc<Mutex<FakeQdrantStoreState>>,
    ) -> RepoHybridIndex {
        let mut index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        index.vector_backend =
            VectorBackend::Qdrant(QdrantVectorStore::fake("test-collection", state));
        index
    }

    async fn mark_qdrant_embedding_ready(index: &RepoHybridIndex, model: &str) {
        index
            .set_metadata(METADATA_EMBEDDING_READY, "true")
            .await
            .expect("set embedding ready");
        index
            .set_metadata(METADATA_EMBEDDING_MODEL, model)
            .await
            .expect("set embedding model");
        index
            .set_metadata(METADATA_VECTOR_BACKEND, "qdrant")
            .await
            .expect("set vector backend");
        index
            .set_metadata(METADATA_VECTOR_LAYOUT_VERSION, QDRANT_VECTOR_LAYOUT_VERSION)
            .await
            .expect("set vector layout version");
    }

    #[test]
    fn chunk_text_splits_with_overlap() {
        let file_text = (1..=65)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_text(&file_text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 40);
        assert_eq!(chunks[1].start_line, 33);
        assert_eq!(chunks[1].end_line, 65);
    }

    #[test]
    fn normalize_rel_path_strips_dot_prefix_and_backslashes() {
        assert_eq!(normalize_rel_path("./src\\main.rs"), "src/main.rs");
    }

    #[test]
    fn qdrant_collection_name_is_deterministic_and_sanitized() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        std::fs::create_dir_all(&repo_root).expect("create repo dir");

        let first = qdrant_collection_name(&repo_root, " codex repo ");
        let second = qdrant_collection_name(&repo_root, " codex repo ");

        assert_eq!(first, second);
        assert!(first.starts_with("codex_repo_"));
        assert!(
            first
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        );
    }

    #[test]
    fn qdrant_backend_requires_url() {
        let index_config = QueryProjectIndex {
            backend: QueryProjectIndexBackend::Qdrant,
            ..Default::default()
        };
        let err = VectorBackend::from_config(Path::new("/tmp/repo"), &index_config)
            .expect_err("missing qdrant url should error");
        assert!(
            err.to_string().contains("query_project_index.qdrant.url"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn vector_backend_local_returns_none_for_qdrant_store() {
        assert!(VectorBackend::Local.qdrant_store().is_none());
    }

    #[test]
    fn normalize_scores_handles_constant_values() {
        let normalized = normalize_scores(&[(1, 2.0), (2, 2.0)]);
        assert_eq!(normalized.get(&1).copied(), Some(1.0));
        assert_eq!(normalized.get(&2).copied(), Some(1.0));
    }

    #[test]
    fn query_embedding_or_fallback_reason_returns_embedding_when_present() {
        let (embedding, reason) =
            query_embedding_or_fallback_reason(vec![vec![0.25_f32, 0.75_f32]]);
        assert_eq!(embedding, Some(vec![0.25_f32, 0.75_f32]));
        assert_eq!(reason, None);
    }

    #[test]
    fn query_embedding_or_fallback_reason_sets_reason_when_missing() {
        let (embedding, reason) = query_embedding_or_fallback_reason(Vec::new());
        assert_eq!(embedding, None);
        assert_eq!(reason, Some(EMBEDDING_REASON_QUERY_FAILED));
    }

    #[test]
    fn query_log_preview_truncates_and_adds_ellipsis() {
        let long_query = "x".repeat(QUERY_LOG_PREVIEW_CHARS + 4);
        let preview = query_log_preview(&long_query);
        assert_eq!(preview.chars().count(), QUERY_LOG_PREVIEW_CHARS + 1);
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn build_glob_set_ignores_blank_entries() {
        let glob_set = build_glob_set(&["  ".to_string(), "\n".to_string()]).expect("glob set");
        assert!(glob_set.is_none());
    }

    #[test]
    fn resolve_repo_root_rejects_paths_outside_cwd() {
        let temp = tempdir().expect("tempdir");
        let cwd = temp.path().join("cwd");
        let inside = cwd.join("repo");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&inside).expect("create inside dir");
        std::fs::create_dir_all(&outside).expect("create outside dir");

        let inside_resolved = resolve_repo_root_from_cwd(Some("repo"), cwd.as_path())
            .expect("inside path should resolve");
        assert_eq!(
            inside_resolved,
            inside.canonicalize().expect("inside canonical")
        );

        let err = resolve_repo_root_from_cwd(Some("../outside"), cwd.as_path())
            .expect_err("outside path should be rejected");
        assert!(
            err.to_string()
                .contains("must be within the current working directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_embedding_mode_uses_required_when_api_key_is_present() {
        let mode =
            resolve_embedding_mode_from_api_key(false, Some("test-key"), OPENAI_API_KEY_ENV_VAR)
                .expect("mode should resolve");
        assert_eq!(mode.mode, EmbeddingMode::Required);
        assert_eq!(mode.reason, None);
        assert!(!mode.require_embeddings);
        assert_eq!(mode.status(true).ready, true);
    }

    #[test]
    fn resolve_embedding_mode_defaults_to_skip_without_api_key() {
        let mode = resolve_embedding_mode_from_api_key(false, None, OPENAI_API_KEY_ENV_VAR)
            .expect("mode should resolve to skip");
        assert_eq!(mode.mode, EmbeddingMode::Skip);
        assert_eq!(mode.reason, Some(EMBEDDING_REASON_MISSING_API_KEY));
        assert!(!mode.require_embeddings);
        assert_eq!(mode.status(false).ready, false);
    }

    #[test]
    fn resolve_embedding_mode_marks_strict_mode_when_embeddings_are_required() {
        let mode =
            resolve_embedding_mode_from_api_key(true, Some("test-key"), OPENAI_API_KEY_ENV_VAR)
                .expect("mode should resolve");
        assert_eq!(mode.mode, EmbeddingMode::Required);
        assert_eq!(mode.reason, None);
        assert!(mode.require_embeddings);
        assert_eq!(mode.status(true).ready, true);
    }

    #[test]
    fn lexical_only_queries_bypass_embedding_mode_resolution() {
        let mode = resolve_query_project_embedding_mode(
            SearchBlendMode::LexicalOnly,
            true,
            DEFAULT_EMBEDDING_MODEL,
        )
        .expect("lexical-only mode should bypass embedding resolution");
        assert_eq!(mode.mode, EmbeddingMode::Skip);
        assert_eq!(mode.reason, None);
        assert!(!mode.require_embeddings);
    }

    #[test]
    fn repo_index_refresh_params_default_accepts_missing_arguments() {
        let params = RepoIndexRefreshParams::default();
        assert_eq!(params.repo_root, None);
        assert_eq!(params.file_globs, None);
        assert_eq!(params.embedding_model, None);
        assert!(!params.force_full);
        assert_eq!(params.require_embeddings, None);
    }

    #[test]
    fn force_full_refresh_disabled_for_not_ready_optional_embeddings() {
        let should_force = should_force_full_refresh(
            false,
            EmbeddingMode::Required,
            false,
            StoredEmbeddingState {
                model: Some(DEFAULT_EMBEDDING_MODEL),
                ready: false,
                backend: Some("local"),
                backend_ready: true,
                vector_layout_version: None,
            },
            DEFAULT_EMBEDDING_MODEL,
            "local",
        );
        assert!(!should_force);
    }

    #[test]
    fn force_full_refresh_stays_disabled_when_already_ready() {
        let should_force = should_force_full_refresh(
            false,
            EmbeddingMode::Required,
            true,
            StoredEmbeddingState {
                model: Some(DEFAULT_EMBEDDING_MODEL),
                ready: true,
                backend: Some("local"),
                backend_ready: true,
                vector_layout_version: None,
            },
            DEFAULT_EMBEDDING_MODEL,
            "local",
        );
        assert!(!should_force);
    }

    #[test]
    fn force_full_refresh_is_enabled_for_strict_missing_embeddings() {
        let should_force = should_force_full_refresh(
            false,
            EmbeddingMode::Required,
            true,
            StoredEmbeddingState {
                model: Some(DEFAULT_EMBEDDING_MODEL),
                ready: false,
                backend: Some("local"),
                backend_ready: true,
                vector_layout_version: None,
            },
            DEFAULT_EMBEDDING_MODEL,
            "local",
        );
        assert!(should_force);
    }

    #[test]
    fn force_full_refresh_is_enabled_when_embedding_model_changes() {
        let should_force = should_force_full_refresh(
            false,
            EmbeddingMode::Required,
            false,
            StoredEmbeddingState {
                model: Some("text-embedding-3-small"),
                ready: false,
                backend: Some("local"),
                backend_ready: true,
                vector_layout_version: None,
            },
            "text-embedding-3-large",
            "local",
        );
        assert!(should_force);
    }

    #[test]
    fn force_full_refresh_is_enabled_when_vector_backend_changes() {
        let should_force = should_force_full_refresh(
            false,
            EmbeddingMode::Required,
            false,
            StoredEmbeddingState {
                model: Some(DEFAULT_EMBEDDING_MODEL),
                ready: true,
                backend: Some("local"),
                backend_ready: true,
                vector_layout_version: None,
            },
            DEFAULT_EMBEDDING_MODEL,
            "qdrant",
        );
        assert!(should_force);
    }

    #[test]
    fn force_full_refresh_disabled_when_backend_matches() {
        let should_force = should_force_full_refresh(
            false,
            EmbeddingMode::Required,
            false,
            StoredEmbeddingState {
                model: Some(DEFAULT_EMBEDDING_MODEL),
                ready: true,
                backend: Some("qdrant"),
                backend_ready: true,
                vector_layout_version: Some(QDRANT_VECTOR_LAYOUT_VERSION),
            },
            DEFAULT_EMBEDDING_MODEL,
            "qdrant",
        );
        assert!(!should_force);
    }

    #[test]
    fn force_full_refresh_is_enabled_when_vector_backend_is_unavailable() {
        let should_force = should_force_full_refresh(
            false,
            EmbeddingMode::Required,
            false,
            StoredEmbeddingState {
                model: Some(DEFAULT_EMBEDDING_MODEL),
                ready: true,
                backend: Some("qdrant"),
                backend_ready: false,
                vector_layout_version: Some(QDRANT_VECTOR_LAYOUT_VERSION),
            },
            DEFAULT_EMBEDDING_MODEL,
            "qdrant",
        );
        assert!(should_force);
    }

    #[test]
    fn force_full_refresh_is_enabled_when_qdrant_layout_version_changes() {
        let should_force = should_force_full_refresh(
            false,
            EmbeddingMode::Required,
            false,
            StoredEmbeddingState {
                model: Some(DEFAULT_EMBEDDING_MODEL),
                ready: true,
                backend: Some("qdrant"),
                backend_ready: true,
                vector_layout_version: None,
            },
            DEFAULT_EMBEDDING_MODEL,
            "qdrant",
        );
        assert!(should_force);
    }

    #[test]
    fn backfill_embeddings_when_required_mode_is_not_ready() {
        assert!(should_backfill_embeddings(EmbeddingMode::Required, false));
    }

    #[test]
    fn skip_mode_does_not_backfill_embeddings() {
        assert!(!should_backfill_embeddings(EmbeddingMode::Skip, false));
    }

    #[test]
    fn required_mode_does_not_backfill_when_already_ready() {
        assert!(!should_backfill_embeddings(EmbeddingMode::Required, true));
    }

    #[test]
    fn vector_prefilter_disabled_for_embedding_only_alpha() {
        let candidates = vec![1_i64, 2_i64];
        assert_eq!(
            vector_prefilter_candidate_ids(
                SearchBlendMode::VectorOnly,
                &VectorBackend::Local,
                candidates.as_slice(),
            ),
            None
        );
    }

    #[test]
    fn vector_prefilter_enabled_for_local_hybrid_candidates() {
        let candidates = vec![1_i64, 2_i64];
        assert_eq!(
            vector_prefilter_candidate_ids(
                SearchBlendMode::Hybrid(0.6),
                &VectorBackend::Local,
                candidates.as_slice(),
            ),
            Some(candidates.as_slice())
        );
    }

    #[test]
    fn vector_prefilter_disabled_for_qdrant_hybrid_candidates() {
        let candidates = vec![1_i64, 2_i64];
        let backend = VectorBackend::Qdrant(QdrantVectorStore::fake(
            "test-collection",
            Arc::new(Mutex::new(FakeQdrantStoreState::default())),
        ));
        assert_eq!(
            vector_prefilter_candidate_ids(
                SearchBlendMode::Hybrid(0.6),
                &backend,
                candidates.as_slice(),
            ),
            None
        );
    }

    #[test]
    fn resolve_embedding_mode_requires_api_key_in_strict_mode() {
        let err = resolve_embedding_mode_from_api_key(true, None, OPENAI_API_KEY_ENV_VAR)
            .expect_err("strict mode should fail without api key");
        assert_eq!(
            err.to_string(),
            "OPENAI_API_KEY is required when require_embeddings=true"
        );
    }

    #[test]
    fn embedding_provider_uses_voyage_for_voyage_models() {
        assert_eq!(
            EmbeddingProvider::from_model("voyage-3-large"),
            EmbeddingProvider::Voyage
        );
        assert_eq!(
            EmbeddingProvider::from_model("text-embedding-3-small"),
            EmbeddingProvider::OpenAiCompatible
        );
    }

    #[test]
    fn resolve_embedding_mode_requires_voyage_api_key_in_strict_mode() {
        let err = resolve_embedding_mode_from_api_key(true, None, VOYAGE_API_KEY_ENV_VAR)
            .expect_err("strict mode should fail without voyage api key");
        assert_eq!(
            err.to_string(),
            "VOYAGE_API_KEY is required when require_embeddings=true"
        );
    }

    #[tokio::test]
    async fn refresh_with_globs_preserves_unmatched_files() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::create_dir_all(repo_root.join("src")).expect("create src dir");
        std::fs::write(repo_root.join("src/a.txt"), "alpha").expect("write a.txt");
        std::fs::write(repo_root.join("src/b.txt"), "beta").expect("write b.txt");

        let index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("initial refresh");
        let outcome = index
            .refresh(
                &["src/a.txt".to_string()],
                false,
                "model".to_string(),
                EmbeddingMode::Skip,
                false,
            )
            .await
            .expect("glob refresh");
        let files = index.load_existing_files().await.expect("load files");

        assert_eq!(outcome.stats.removed_files, 0);
        assert!(!outcome.ready);
        assert!(files.contains_key("src/a.txt"));
        assert!(files.contains_key("src/b.txt"));
    }

    #[tokio::test]
    async fn refresh_with_skip_mode_does_not_rebuild_unchanged_files() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        let mode = resolve_embedding_mode_from_api_key(false, None, OPENAI_API_KEY_ENV_VAR)
            .expect("mode should resolve to skip");
        index
            .refresh(
                &[],
                false,
                "model".to_string(),
                mode.mode,
                mode.require_embeddings,
            )
            .await
            .expect("initial refresh");
        let second = index
            .refresh(
                &[],
                false,
                "model".to_string(),
                mode.mode,
                mode.require_embeddings,
            )
            .await
            .expect("second refresh");

        assert_eq!(second.stats.updated_files, 0);
        assert_eq!(second.stats.removed_files, 0);
        assert!(!second.ready);
    }

    #[tokio::test]
    async fn scoped_force_full_refresh_is_rejected() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        let err = index
            .refresh(
                &["README.md".to_string()],
                true,
                "model".to_string(),
                EmbeddingMode::Skip,
                false,
            )
            .await
            .expect_err("scoped force_full should fail");

        assert!(
            err.to_string()
                .contains("force_full cannot be combined with file_globs"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn scoped_refresh_with_missing_qdrant_collection_preserves_existing_files() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::create_dir_all(repo_root.join("src")).expect("create src dir");
        std::fs::write(repo_root.join("src/a.txt"), "alpha").expect("write a.txt");
        std::fs::write(repo_root.join("src/b.txt"), "beta").expect("write b.txt");

        let server = MockServer::start().await;
        mount_openai_embeddings(&server, vec![vec![vec![1.0_f32, 0.0_f32]]]).await;

        let state = Arc::new(Mutex::new(FakeQdrantStoreState::default()));
        let mut index = open_index_with_fake_qdrant(repo_root, Arc::clone(&state)).await;
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("initial refresh");
        mark_qdrant_embedding_ready(&index, "model").await;

        let outcome = index
            .refresh(
                &["src/a.txt".to_string()],
                false,
                "model".to_string(),
                EmbeddingMode::Required,
                false,
            )
            .await
            .expect("scoped refresh should preserve existing files");
        let files = index.load_existing_files().await.expect("load files");
        let chunk_count = index.count_chunks().await.expect("count chunks");
        let state = state.lock().expect("lock fake qdrant state");

        assert_eq!(outcome.stats.updated_files, 1);
        assert_eq!(outcome.stats.removed_files, 0);
        assert!(!outcome.ready);
        assert!(files.contains_key("src/a.txt"));
        assert!(files.contains_key("src/b.txt"));
        assert_eq!(chunk_count, 2);
        assert_eq!(state.clear_count, 0);
        assert!(state.collection_exists);
    }

    #[tokio::test]
    async fn scoped_strict_refresh_fails_when_full_corpus_repair_is_needed() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::create_dir_all(repo_root.join("src")).expect("create src dir");
        std::fs::write(repo_root.join("src/a.txt"), "alpha").expect("write a.txt");

        let state = Arc::new(Mutex::new(FakeQdrantStoreState::default()));
        let index = open_index_with_fake_qdrant(repo_root, Arc::clone(&state)).await;
        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("initial refresh");
        mark_qdrant_embedding_ready(&index, "model").await;

        let err = index
            .refresh(
                &["src/a.txt".to_string()],
                false,
                "model".to_string(),
                EmbeddingMode::Required,
                true,
            )
            .await
            .expect_err("strict scoped refresh should fail");

        assert!(
            err.to_string()
                .contains("scoped refresh cannot satisfy require_embeddings=true"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn scoped_refresh_with_stale_qdrant_layout_preserves_existing_files() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::create_dir_all(repo_root.join("src")).expect("create src dir");
        std::fs::write(repo_root.join("src/a.txt"), "alpha").expect("write a.txt");
        std::fs::write(repo_root.join("src/b.txt"), "beta").expect("write b.txt");

        let server = MockServer::start().await;
        mount_openai_embeddings(&server, vec![vec![vec![1.0_f32, 0.0_f32]]]).await;

        let state = Arc::new(Mutex::new(FakeQdrantStoreState {
            collection_exists: true,
            ..Default::default()
        }));
        let mut index = open_index_with_fake_qdrant(repo_root, Arc::clone(&state)).await;
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("initial refresh");
        index
            .set_metadata(METADATA_EMBEDDING_READY, "true")
            .await
            .expect("set embedding ready");
        index
            .set_metadata(METADATA_EMBEDDING_MODEL, "model")
            .await
            .expect("set embedding model");
        index
            .set_metadata(METADATA_VECTOR_BACKEND, "qdrant")
            .await
            .expect("set vector backend");

        state.lock().expect("lock fake qdrant state").points.insert(
            9_999,
            FakeQdrantPoint {
                path: "stale.rs".to_string(),
                embedding: vec![1.0_f32, 0.0_f32],
            },
        );

        let outcome = index
            .refresh(
                &["src/a.txt".to_string()],
                false,
                "model".to_string(),
                EmbeddingMode::Required,
                false,
            )
            .await
            .expect("scoped refresh should preserve existing files");
        let files = index.load_existing_files().await.expect("load files");
        let chunk_count = index.count_chunks().await.expect("count chunks");
        let vector_layout_version = index
            .load_metadata(METADATA_VECTOR_LAYOUT_VERSION)
            .await
            .expect("load vector layout version");
        let state = state.lock().expect("lock fake qdrant state");

        assert_eq!(outcome.stats.updated_files, 1);
        assert_eq!(outcome.stats.removed_files, 0);
        assert!(!outcome.ready);
        assert!(files.contains_key("src/a.txt"));
        assert!(files.contains_key("src/b.txt"));
        assert_eq!(chunk_count, 2);
        assert_eq!(state.clear_count, 0);
        assert!(state.delete_paths.is_empty());
        assert!(state.points.contains_key(&9_999));
        assert_eq!(vector_layout_version, None);
    }

    #[tokio::test]
    async fn scoped_required_refresh_keeps_index_not_ready_until_full_backfill() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::create_dir_all(repo_root.join("src")).expect("create src dir");
        std::fs::write(repo_root.join("src/a.txt"), "alpha").expect("write a.txt");
        std::fs::write(repo_root.join("src/b.txt"), "beta").expect("write b.txt");

        let server = MockServer::start().await;
        mount_openai_embeddings(
            &server,
            vec![
                vec![vec![1.0_f32, 0.0_f32]],
                vec![vec![1.0_f32, 0.0_f32]],
                vec![vec![1.0_f32, 0.0_f32]],
            ],
        )
        .await;

        let mut index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("initial skip refresh");
        assert!(
            !index
                .embedding_ready()
                .await
                .expect("embedding ready after skip")
        );

        let partial = index
            .refresh(
                &["src/a.txt".to_string()],
                false,
                "model".to_string(),
                EmbeddingMode::Required,
                false,
            )
            .await
            .expect("partial required refresh");
        assert_eq!(partial.stats.updated_files, 1);
        assert!(!partial.ready);
        assert!(
            !index
                .embedding_ready()
                .await
                .expect("embedding ready after partial")
        );

        let full = index
            .refresh(
                &[],
                false,
                "model".to_string(),
                EmbeddingMode::Required,
                false,
            )
            .await
            .expect("full required refresh");
        assert_eq!(full.stats.updated_files, 2);
        assert!(full.ready);
        assert!(
            index
                .embedding_ready()
                .await
                .expect("embedding ready after full")
        );
    }

    #[tokio::test]
    async fn lexical_only_search_skips_embedding_checks_and_requests() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let server = MockServer::start().await;
        mount_openai_embeddings(&server, Vec::new()).await;

        let mut index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());
        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("refresh");

        let outcome = index
            .search(
                "needle",
                5,
                SearchBlendMode::LexicalOnly,
                &[],
                "model".to_string(),
                true,
            )
            .await
            .expect("lexical-only search");

        assert_eq!(outcome.embedding_fallback_reason, None);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "README.md");
    }

    #[tokio::test]
    async fn vector_only_search_can_return_non_lexical_vector_hits() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        let server = MockServer::start().await;
        mount_openai_embeddings(&server, vec![vec![vec![1.0_f32, 0.0_f32]]]).await;

        let mut index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        let mut tx = index.pool.begin().await.expect("begin tx");
        let lexical_insert = sqlx::query(
            "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("lexical.rs")
        .bind(1_i64)
        .bind(1_i64)
        .bind("needle lexical")
        .bind("needle lexical")
        .bind("[0.0, 1.0]")
        .execute(&mut *tx)
        .await
        .expect("insert lexical chunk");
        let lexical_id = lexical_insert.last_insert_rowid();
        sqlx::query("INSERT INTO chunks_fts(rowid, content, path, chunk_id) VALUES (?, ?, ?, ?)")
            .bind(lexical_id)
            .bind("needle lexical")
            .bind("lexical.rs")
            .bind(lexical_id)
            .execute(&mut *tx)
            .await
            .expect("insert lexical fts row");

        let vector_insert = sqlx::query(
            "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("vector.rs")
        .bind(1_i64)
        .bind(1_i64)
        .bind("vector hit")
        .bind("something unrelated")
        .bind("[1.0, 0.0]")
        .execute(&mut *tx)
        .await
        .expect("insert vector chunk");
        let vector_id = vector_insert.last_insert_rowid();
        sqlx::query("INSERT INTO chunks_fts(rowid, content, path, chunk_id) VALUES (?, ?, ?, ?)")
            .bind(vector_id)
            .bind("something unrelated")
            .bind("vector.rs")
            .bind(vector_id)
            .execute(&mut *tx)
            .await
            .expect("insert vector fts row");
        tx.commit().await.expect("commit");

        index
            .set_metadata(METADATA_EMBEDDING_READY, "true")
            .await
            .expect("set embedding ready");
        index
            .set_metadata(METADATA_EMBEDDING_MODEL, "model")
            .await
            .expect("set embedding model");
        index
            .set_metadata(METADATA_VECTOR_BACKEND, "local")
            .await
            .expect("set vector backend");

        let outcome = index
            .search(
                "needle",
                1,
                SearchBlendMode::VectorOnly,
                &[],
                "model".to_string(),
                false,
            )
            .await
            .expect("vector-only search");

        assert_eq!(outcome.embedding_fallback_reason, None);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "vector.rs");
    }

    #[tokio::test]
    async fn hybrid_search_preserves_semantic_only_hits() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        let server = MockServer::start().await;
        mount_openai_embeddings(&server, vec![vec![vec![1.0_f32, 0.0_f32]]]).await;

        let state = Arc::new(Mutex::new(FakeQdrantStoreState {
            collection_exists: true,
            ..Default::default()
        }));
        let mut index = open_index_with_fake_qdrant(repo_root, Arc::clone(&state)).await;
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        let mut tx = index.pool.begin().await.expect("begin tx");
        let lexical_insert = sqlx::query(
            "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("lexical.rs")
        .bind(1_i64)
        .bind(1_i64)
        .bind("needle lexical")
        .bind("needle lexical")
        .bind("[0.0, 1.0]")
        .execute(&mut *tx)
        .await
        .expect("insert lexical chunk");
        let lexical_id = lexical_insert.last_insert_rowid();
        sqlx::query("INSERT INTO chunks_fts(rowid, content, path, chunk_id) VALUES (?, ?, ?, ?)")
            .bind(lexical_id)
            .bind("needle lexical")
            .bind("lexical.rs")
            .bind(lexical_id)
            .execute(&mut *tx)
            .await
            .expect("insert lexical fts row");

        let vector_insert = sqlx::query(
            "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("vector.rs")
        .bind(1_i64)
        .bind(1_i64)
        .bind("vector hit")
        .bind("something unrelated")
        .bind("[1.0, 0.0]")
        .execute(&mut *tx)
        .await
        .expect("insert vector chunk");
        let vector_id = vector_insert.last_insert_rowid();
        sqlx::query("INSERT INTO chunks_fts(rowid, content, path, chunk_id) VALUES (?, ?, ?, ?)")
            .bind(vector_id)
            .bind("something unrelated")
            .bind("vector.rs")
            .bind(vector_id)
            .execute(&mut *tx)
            .await
            .expect("insert vector fts row");
        tx.commit().await.expect("commit");

        mark_qdrant_embedding_ready(&index, "model").await;
        state
            .lock()
            .expect("lock fake qdrant state")
            .points
            .extend([
                (
                    lexical_id,
                    FakeQdrantPoint {
                        path: "lexical.rs".to_string(),
                        embedding: vec![0.0_f32, 1.0_f32],
                    },
                ),
                (
                    vector_id,
                    FakeQdrantPoint {
                        path: "vector.rs".to_string(),
                        embedding: vec![1.0_f32, 0.0_f32],
                    },
                ),
            ]);

        let outcome = index
            .search(
                "needle",
                2,
                SearchBlendMode::Hybrid(0.6),
                &[],
                "model".to_string(),
                false,
            )
            .await
            .expect("hybrid search");

        assert_eq!(outcome.embedding_fallback_reason, None);
        assert_eq!(outcome.results.len(), 2);
        assert!(
            outcome
                .results
                .iter()
                .any(|result| result.path == "lexical.rs"),
            "expected lexical hit in results: {outcome:?}"
        );
        assert!(
            outcome
                .results
                .iter()
                .any(|result| result.path == "vector.rs"),
            "expected semantic-only hit in results: {outcome:?}"
        );
    }

    #[tokio::test]
    async fn local_hybrid_search_prefilters_to_lexical_candidates() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        let server = MockServer::start().await;
        mount_openai_embeddings(&server, vec![vec![vec![1.0_f32, 0.0_f32]]]).await;

        let mut index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        let mut tx = index.pool.begin().await.expect("begin tx");
        let lexical_insert = sqlx::query(
            "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("good.rs")
        .bind(1_i64)
        .bind(1_i64)
        .bind("needle lexical")
        .bind("needle lexical")
        .bind("[1.0, 0.0]")
        .execute(&mut *tx)
        .await
        .expect("insert lexical chunk");
        let lexical_id = lexical_insert.last_insert_rowid();
        sqlx::query("INSERT INTO chunks_fts(rowid, content, path, chunk_id) VALUES (?, ?, ?, ?)")
            .bind(lexical_id)
            .bind("needle lexical")
            .bind("good.rs")
            .bind(lexical_id)
            .execute(&mut *tx)
            .await
            .expect("insert lexical fts row");

        sqlx::query(
            "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("broken.rs")
        .bind(1_i64)
        .bind(1_i64)
        .bind("broken")
        .bind("something unrelated")
        .bind("not-json")
        .execute(&mut *tx)
        .await
        .expect("insert broken chunk");
        tx.commit().await.expect("commit");

        index
            .set_metadata(METADATA_EMBEDDING_READY, "true")
            .await
            .expect("set embedding ready");
        index
            .set_metadata(METADATA_EMBEDDING_MODEL, "model")
            .await
            .expect("set embedding model");
        index
            .set_metadata(METADATA_VECTOR_BACKEND, "local")
            .await
            .expect("set vector backend");
        index
            .set_metadata(METADATA_VECTOR_LAYOUT_VERSION, "")
            .await
            .expect("set vector layout version");

        let outcome = index
            .search(
                "needle",
                1,
                SearchBlendMode::Hybrid(0.6),
                &[],
                "model".to_string(),
                false,
            )
            .await
            .expect("hybrid search should stay within lexical candidates");

        assert_eq!(outcome.embedding_fallback_reason, None);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "good.rs");
    }

    #[tokio::test]
    async fn search_falls_back_to_lexical_when_embeddings_disabled() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("refresh");
        let outcome = index
            .search(
                "needle",
                5,
                SearchBlendMode::Hybrid(0.6),
                &[],
                "model".to_string(),
                false,
            )
            .await
            .expect("search");

        assert!(
            outcome
                .results
                .iter()
                .any(|result| result.snippet.contains("needle")),
            "expected lexical match in results: {outcome:?}"
        );
        assert_eq!(outcome.embedding_fallback_reason, None);
    }

    #[tokio::test]
    async fn search_falls_back_when_qdrant_collection_is_missing() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let state = Arc::new(Mutex::new(FakeQdrantStoreState::default()));
        let index = open_index_with_fake_qdrant(repo_root, Arc::clone(&state)).await;
        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("refresh");
        mark_qdrant_embedding_ready(&index, "model").await;

        let outcome = index
            .search(
                "needle",
                5,
                SearchBlendMode::Hybrid(0.6),
                &[],
                "model".to_string(),
                false,
            )
            .await
            .expect("search with missing qdrant collection");

        assert_eq!(
            outcome.embedding_fallback_reason,
            Some(EMBEDDING_REASON_UNAVAILABLE)
        );
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "README.md");

        let err = index
            .search(
                "needle",
                5,
                SearchBlendMode::Hybrid(0.6),
                &[],
                "model".to_string(),
                true,
            )
            .await
            .expect_err("strict mode should fail when qdrant collection is missing");
        assert!(
            err.to_string()
                .contains("embeddings are required but the index is not embedding-ready"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn strict_search_fails_without_embedding_ready_index() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("refresh");

        let err = index
            .search(
                "needle",
                5,
                SearchBlendMode::Hybrid(0.5),
                &[],
                "model".to_string(),
                true,
            )
            .await
            .expect_err("strict mode should fail when embeddings are unavailable");
        assert!(
            err.to_string()
                .contains("embeddings are required but the index is not embedding-ready"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn search_filters_missing_qdrant_chunk_ids_before_limiting() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        let server = MockServer::start().await;
        mount_openai_embeddings(&server, vec![vec![vec![1.0_f32, 0.0_f32]]]).await;

        let state = Arc::new(Mutex::new(FakeQdrantStoreState {
            collection_exists: true,
            ..Default::default()
        }));
        let mut index = open_index_with_fake_qdrant(repo_root, Arc::clone(&state)).await;
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        let mut tx = index.pool.begin().await.expect("begin tx");
        let fresh_insert = sqlx::query(
            "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("fresh.rs")
        .bind(1_i64)
        .bind(1_i64)
        .bind("fresh")
        .bind("fresh")
        .bind("[0.5, 0.5]")
        .execute(&mut *tx)
        .await
        .expect("insert fresh chunk");
        let fresh_id = fresh_insert.last_insert_rowid();
        tx.commit().await.expect("commit");
        mark_qdrant_embedding_ready(&index, "model").await;

        state
            .lock()
            .expect("lock fake qdrant state")
            .points
            .extend([
                (
                    9_999,
                    FakeQdrantPoint {
                        path: "stale.rs".to_string(),
                        embedding: vec![1.0_f32, 0.0_f32],
                    },
                ),
                (
                    fresh_id,
                    FakeQdrantPoint {
                        path: "fresh.rs".to_string(),
                        embedding: vec![0.5_f32, 0.5_f32],
                    },
                ),
            ]);

        let outcome = index
            .search(
                "needle",
                1,
                SearchBlendMode::VectorOnly,
                &[],
                "model".to_string(),
                false,
            )
            .await
            .expect("vector-only search");

        assert_eq!(outcome.embedding_fallback_reason, None);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "fresh.rs");
    }

    #[tokio::test]
    async fn qdrant_vector_search_pages_until_glob_filtered_matches_are_found() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        let server = MockServer::start().await;
        mount_openai_embeddings(&server, vec![vec![vec![1.0_f32, 0.0_f32]]]).await;

        let state = Arc::new(Mutex::new(FakeQdrantStoreState {
            collection_exists: true,
            ..Default::default()
        }));
        let mut index = open_index_with_fake_qdrant(repo_root, Arc::clone(&state)).await;
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        let mut tx = index.pool.begin().await.expect("begin tx");
        let mut out_of_scope_ids = Vec::new();
        for idx in 0..VECTOR_CANDIDATE_MULTIPLIER {
            let path = format!("vendor/out-{idx}.rs");
            let insert = sqlx::query(
                "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&path)
            .bind(1_i64)
            .bind(1_i64)
            .bind("out-of-scope")
            .bind("out-of-scope")
            .bind("[1.0, 0.0]")
            .execute(&mut *tx)
            .await
            .expect("insert out-of-scope chunk");
            out_of_scope_ids.push((insert.last_insert_rowid(), path));
        }

        let in_scope_insert = sqlx::query(
            "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("src/in-scope.rs")
        .bind(1_i64)
        .bind(1_i64)
        .bind("in-scope")
        .bind("in-scope")
        .bind("[0.9, 0.1]")
        .execute(&mut *tx)
        .await
        .expect("insert in-scope chunk");
        let in_scope_id = in_scope_insert.last_insert_rowid();
        tx.commit().await.expect("commit");

        mark_qdrant_embedding_ready(&index, "model").await;

        {
            let mut state = state.lock().expect("lock fake qdrant state");
            for (chunk_id, path) in out_of_scope_ids {
                state.points.insert(
                    chunk_id,
                    FakeQdrantPoint {
                        path,
                        embedding: vec![1.0_f32, 0.0_f32],
                    },
                );
            }
            state.points.insert(
                in_scope_id,
                FakeQdrantPoint {
                    path: "src/in-scope.rs".to_string(),
                    embedding: vec![0.9_f32, 0.1_f32],
                },
            );
        }

        let outcome = index
            .search(
                "needle",
                1,
                SearchBlendMode::VectorOnly,
                &["src/*.rs".to_string()],
                "model".to_string(),
                false,
            )
            .await
            .expect("vector-only search with glob filter");

        assert_eq!(outcome.embedding_fallback_reason, None);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "src/in-scope.rs");
    }

    #[tokio::test]
    async fn refresh_replaces_existing_qdrant_points_for_updated_files() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "first version").expect("write README");

        let server = MockServer::start().await;
        mount_openai_embeddings(
            &server,
            vec![vec![vec![1.0_f32, 0.0_f32]], vec![vec![0.5_f32, 0.5_f32]]],
        )
        .await;

        let state = Arc::new(Mutex::new(FakeQdrantStoreState::default()));
        let mut index = open_index_with_fake_qdrant(repo_root, Arc::clone(&state)).await;
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        index
            .refresh(
                &[],
                false,
                "model".to_string(),
                EmbeddingMode::Required,
                false,
            )
            .await
            .expect("initial refresh");
        let first_point_ids = state
            .lock()
            .expect("lock fake qdrant state")
            .points
            .keys()
            .copied()
            .collect::<Vec<_>>();

        std::thread::sleep(Duration::from_millis(5));
        std::fs::write(repo_root.join("README.md"), "second version with more text")
            .expect("rewrite README");

        index
            .refresh(
                &[],
                false,
                "model".to_string(),
                EmbeddingMode::Required,
                false,
            )
            .await
            .expect("second refresh");

        let guard = state.lock().expect("lock fake qdrant state");
        assert_eq!(guard.delete_paths, vec!["README.md".to_string()]);
        assert_eq!(guard.points.len(), 1);
        assert!(
            first_point_ids
                .iter()
                .all(|point_id| !guard.points.contains_key(point_id)),
            "stale point ids should have been removed: {:?}",
            guard.points.keys().collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn refresh_rebuilds_when_qdrant_collection_is_missing() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let server = MockServer::start().await;
        mount_openai_embeddings(&server, vec![vec![vec![1.0_f32, 0.0_f32]]]).await;

        let state = Arc::new(Mutex::new(FakeQdrantStoreState::default()));
        let mut index = open_index_with_fake_qdrant(repo_root, Arc::clone(&state)).await;
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("initial lexical refresh");
        mark_qdrant_embedding_ready(&index, "model").await;

        let outcome = index
            .refresh(
                &[],
                false,
                "model".to_string(),
                EmbeddingMode::Required,
                false,
            )
            .await
            .expect("required refresh should rebuild missing collection");

        let guard = state.lock().expect("lock fake qdrant state");
        assert!(guard.collection_exists);
        assert_eq!(guard.ensure_dimensions, vec![2]);
        assert_eq!(outcome.stats.updated_files, 1);
        assert!(outcome.ready);
    }

    #[tokio::test]
    async fn refresh_rebuilds_old_qdrant_layout_and_restores_search_recall() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let server = MockServer::start().await;
        mount_openai_embeddings(
            &server,
            vec![vec![vec![1.0_f32, 0.0_f32]], vec![vec![1.0_f32, 0.0_f32]]],
        )
        .await;

        let state = Arc::new(Mutex::new(FakeQdrantStoreState {
            collection_exists: true,
            ..Default::default()
        }));
        let mut index = open_index_with_fake_qdrant(repo_root, Arc::clone(&state)).await;
        index.embeddings_base_url_override = Some(format!("{}/v1", server.uri()));
        index.embedding_api_key_override = Some("test-key".to_string());

        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip, false)
            .await
            .expect("initial lexical refresh");
        index
            .set_metadata(METADATA_EMBEDDING_READY, "true")
            .await
            .expect("set embedding ready");
        index
            .set_metadata(METADATA_EMBEDDING_MODEL, "model")
            .await
            .expect("set embedding model");
        index
            .set_metadata(METADATA_VECTOR_BACKEND, "qdrant")
            .await
            .expect("set vector backend");

        {
            let mut guard = state.lock().expect("lock fake qdrant state");
            for stale_id in 10_000_i64..10_000_i64 + (VECTOR_CANDIDATE_MULTIPLIER as i64 + 4) {
                guard.points.insert(
                    stale_id,
                    FakeQdrantPoint {
                        path: format!("stale-{stale_id}.rs"),
                        embedding: vec![1.0_f32, 0.0_f32],
                    },
                );
            }
        }

        let outcome = index
            .refresh(
                &[],
                false,
                "model".to_string(),
                EmbeddingMode::Required,
                false,
            )
            .await
            .expect("required refresh should rebuild old qdrant layout");

        {
            let guard = state.lock().expect("lock fake qdrant state");
            assert_eq!(guard.clear_count, 1);
            assert_eq!(guard.points.len(), 1);
        }
        assert_eq!(outcome.stats.updated_files, 1);
        assert!(outcome.ready);

        assert_eq!(
            index
                .load_metadata(METADATA_VECTOR_LAYOUT_VERSION)
                .await
                .expect("load vector layout version"),
            Some(QDRANT_VECTOR_LAYOUT_VERSION.to_string())
        );

        let search_outcome = index
            .search(
                "needle",
                1,
                SearchBlendMode::VectorOnly,
                &[],
                "model".to_string(),
                false,
            )
            .await
            .expect("vector-only search after layout rebuild");

        assert_eq!(search_outcome.embedding_fallback_reason, None);
        assert_eq!(search_outcome.results.len(), 1);
        assert_eq!(search_outcome.results[0].path, "README.md");
    }

    #[tokio::test]
    async fn auto_warm_query_project_index_is_incremental() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path().to_path_buf();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let index_config = QueryProjectIndex::default();
        let first =
            refresh_repo_index(repo_root.clone(), vec![], None, false, false, &index_config)
                .await
                .expect("first warm");
        let second = refresh_repo_index(repo_root, vec![], None, false, false, &index_config)
            .await
            .expect("second warm");

        assert_eq!(first.stats.updated_files, 1);
        assert_eq!(second.stats.updated_files, 0);
        assert_eq!(second.stats.removed_files, 0);
        assert_eq!(
            first.embedding_status.ready, second.embedding_status.ready,
            "embedding ready status should be consistent across incremental refreshes"
        );
    }

    /// Manual integration test: populates the Qdrant index for a real repo
    /// and verifies the collection exists with points.
    ///
    /// Requires:
    /// - `AZURE_OPENAI_API_KEY` (for embeddings via Azure OpenAI)
    /// - `QDRANT_API_KEY` (for vector storage)
    /// - Qdrant cluster reachable at the configured URL
    ///
    /// Run with: `cargo test -p codex-mcp-server --lib -- --ignored populate_qdrant_index_for_repo`
    #[tokio::test]
    #[ignore]
    async fn populate_qdrant_index_for_repo() {
        // Both ring and aws-lc-rs are in the dep tree, so rustls cannot
        // auto-select.  Install ring explicitly before the qdrant client
        // tries to open a TLS connection.
        codex_utils_rustls_provider::ensure_rustls_crypto_provider();

        let repo_root = PathBuf::from("/home/azureuser/codex");
        let embedding_model = "text-embedding-3-small";
        let index_config = QueryProjectIndex {
            backend: QueryProjectIndexBackend::Qdrant,
            auto_warm: true,
            require_embeddings: true,
            embedding_model: Some(embedding_model.to_string()),
            file_globs: vec![],
            qdrant: codex_core::config::types::QueryProjectIndexQdrant {
                url: Some(
                    "https://f2e5a36d-0c8b-4f74-a174-e53bc60baab9.eastus-0.azure.cloud.qdrant.io:6334".to_string(),
                ),
                api_key_env: "QDRANT_API_KEY".to_string(),
                collection_prefix: "codex_repo_".to_string(),
                timeout_ms: 10_000,
            },
        };

        // Use the Azure OpenAI deployment-path embeddings endpoint.
        let azure_api_key =
            std::env::var("AZURE_OPENAI_API_KEY").expect("AZURE_OPENAI_API_KEY must be set");
        let azure_embeddings_url = format!(
            "https://fifteenmodels.openai.azure.com/openai/deployments/{embedding_model}/embeddings?api-version=2025-04-01-preview"
        );

        // Open the index directly so we can set the Azure overrides before
        // calling refresh (refresh_repo_index doesn't expose override knobs).
        let mut index = RepoHybridIndex::open(&repo_root, &index_config)
            .await
            .expect("open index");
        index.embeddings_base_url_override = Some(azure_embeddings_url);
        index.embedding_api_key_override = Some(azure_api_key);

        let mut embedding_mode = SelectedEmbeddingMode {
            mode: EmbeddingMode::Required,
            reason: None,
            require_embeddings: true,
        };
        let refresh_outcome = refresh_index(
            &index,
            &[],
            true, // force_full
            embedding_model,
            &mut embedding_mode,
        )
        .await
        .expect("refresh should succeed");
        let outcome = RepoIndexWarmOutcome {
            repo_root: repo_root.clone(),
            stats: refresh_outcome.stats,
            embedding_status: embedding_mode.status(refresh_outcome.ready),
        };

        tracing::info!("Refresh outcome: {outcome:#?}");
        assert!(
            outcome.stats.scanned_files > 0,
            "expected scanned files > 0, got {}",
            outcome.stats.scanned_files
        );
        assert!(
            outcome.stats.indexed_chunks > 0,
            "expected indexed chunks > 0, got {}",
            outcome.stats.indexed_chunks
        );
        assert!(
            outcome.embedding_status.ready,
            "expected embedding_status.ready = true"
        );

        // Verify the Qdrant collection was created.
        let store = index
            .vector_backend
            .qdrant_store()
            .expect("should have qdrant store");
        let exists = store
            .collection_exists()
            .await
            .expect("collection_exists check");
        assert!(exists, "Qdrant collection should exist after refresh");

        tracing::info!(
            "Collection '{}' exists with {} indexed chunks",
            store.collection_name(),
            outcome.stats.indexed_chunks
        );

        // Run a search to confirm end-to-end (reuse the same index
        // so the Azure embeddings overrides remain active).
        let search_outcome = index
            .search(
                "tool dispatch handler",
                5,
                SearchBlendMode::Hybrid(0.6),
                &[],
                embedding_model.to_string(),
                true,
            )
            .await
            .expect("search should succeed");
        assert!(
            !search_outcome.results.is_empty(),
            "expected search results, got none"
        );
        tracing::info!("Search returned {} results:", search_outcome.results.len());
        for result in &search_outcome.results {
            tracing::info!(
                "  {path}:{start}-{end} (score {score:.4})",
                path = result.path,
                start = result.line_range.start,
                end = result.line_range.end,
                score = result.score,
            );
        }
    }

    #[tokio::test]
    async fn vector_scores_with_candidates_avoids_full_table_scan() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        let index = RepoHybridIndex::open(repo_root, &QueryProjectIndex::default())
            .await
            .expect("open index");
        let mut tx = index.pool.begin().await.expect("begin tx");

        let good_insert = sqlx::query(
            "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("good.rs")
        .bind(1_i64)
        .bind(1_i64)
        .bind("good")
        .bind("good")
        .bind("[1.0, 0.0]")
        .execute(&mut *tx)
        .await
        .expect("insert good chunk");
        let good_id = good_insert.last_insert_rowid();

        sqlx::query(
            "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("bad.rs")
        .bind(1_i64)
        .bind(1_i64)
        .bind("bad")
        .bind("bad")
        .bind("not-json")
        .execute(&mut *tx)
        .await
        .expect("insert bad chunk");
        tx.commit().await.expect("commit");

        let scores = index
            .vector_scores(&[1.0, 0.0], 5, None, Some(&[good_id]))
            .await
            .expect("vector scores should only read candidate rows");

        assert_eq!(scores, vec![(good_id, 1.0)]);
    }
}
