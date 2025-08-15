//! Types used to define the fields of [`crate::config::Config`].

// Note this file should generally be restricted to simple struct/enum
// definitions that do not contain business logic.

use std::collections::HashMap;
use std::path::PathBuf;
use strum_macros::Display;
use wildmatch::WildMatchPattern;

use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct McpServerConfig {
    pub command: String,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialEq)]
pub enum UriBasedFileOpener {
    #[serde(rename = "vscode")]
    VsCode,

    #[serde(rename = "vscode-insiders")]
    VsCodeInsiders,

    #[serde(rename = "windsurf")]
    Windsurf,

    #[serde(rename = "cursor")]
    Cursor,

    /// Option to disable the URI-based file opener.
    #[serde(rename = "none")]
    None,
}

impl UriBasedFileOpener {
    pub fn get_scheme(&self) -> Option<&str> {
        match self {
            UriBasedFileOpener::VsCode => Some("vscode"),
            UriBasedFileOpener::VsCodeInsiders => Some("vscode-insiders"),
            UriBasedFileOpener::Windsurf => Some("windsurf"),
            UriBasedFileOpener::Cursor => Some("cursor"),
            UriBasedFileOpener::None => None,
        }
    }
}

/// Settings that govern if and what will be written to `~/.codex/history.jsonl`.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct History {
    /// If true, history entries will not be written to disk.
    pub persistence: HistoryPersistence,

    /// If set, the maximum size of the history file in bytes.
    /// TODO(mbolin): Not currently honored.
    pub max_bytes: Option<usize>,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryPersistence {
    /// Save all history entries to disk.
    #[default]
    SaveAll,
    /// Do not write history to disk.
    None,
}

/// Collection of settings that are specific to the TUI.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Tui {}

/// Web search context size options
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WebSearchContextSize {
    Low,
    #[default]
    Medium,
    High,
}

/// User location configuration for web search
#[derive(Deserialize, Debug, Clone, PartialEq, Default, Serialize)]
pub struct WebSearchUserLocation {
    /// Two-letter ISO country code (e.g., "US")
    pub country: Option<String>,
    /// City name (e.g., "San Francisco")
    pub city: Option<String>,
    /// Region/state name (e.g., "California")
    pub region: Option<String>,
    /// IANA timezone (e.g., "America/Los_Angeles")
    pub timezone: Option<String>,
}

/// Configuration for web search tool
#[derive(Deserialize, Debug, Clone, PartialEq, Default, Serialize)]
pub struct WebSearchSettings {
    /// Whether web search is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Search context size
    #[serde(default)]
    pub context_size: WebSearchContextSize,

    /// User location for search refinement
    #[serde(default)]
    pub user_location: Option<WebSearchUserLocation>,

    /// If true, force tool_choice to {"type": "web_search_preview"} when using
    /// the Responses API so the model prioritizes web search. Defaults to false.
    #[serde(default)]
    pub force_tool_choice: bool,

    /// Optional dated version for the web search tool (e.g., "2025_03_11" or
    /// "2025-03-11"). When set, Codex will request the corresponding
    /// versioned tool (e.g., "web_search_preview_2025_03_11"). If unset,
    /// falls back to the generic "web_search_preview" identifier.
    #[serde(default)]
    pub tool_version: Option<String>,

    /// Whether to enforce the 128k token context limit when web search is enabled
    /// Defaults to true for compatibility with web search requirements
    #[serde(default = "default_enforce_context_limit")]
    pub enforce_context_limit: bool,

    /// Whether to display cost tracking for web search operations
    #[serde(default)]
    pub show_search_costs: bool,

    /// Whether to render citations with enhanced formatting (inline annotations)
    #[serde(default = "default_render_citations")]
    pub render_citations: bool,

    /// Display options for search results
    #[serde(default)]
    pub display: WebSearchDisplaySettings,

    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limiting: WebSearchRateLimitSettings,
}

/// Display configuration for web search results
#[derive(Deserialize, Debug, Clone, PartialEq, Default, Serialize)]
pub struct WebSearchDisplaySettings {
    /// Whether to always show source attribution for search results
    #[serde(default = "default_show_attribution")]
    pub show_attribution: bool,

    /// Maximum number of search results to display (0 = unlimited)
    #[serde(default = "default_max_results")]
    pub max_results: u32,

    /// Whether to highlight cited text in search results
    #[serde(default = "default_highlight_citations")]
    pub highlight_citations: bool,

    /// Whether to show relevance scores when available
    #[serde(default)]
    pub show_relevance_scores: bool,
}

/// Rate limiting configuration for web search
#[derive(Deserialize, Debug, Clone, PartialEq, Default, Serialize)]
pub struct WebSearchRateLimitSettings {
    /// Whether to enable rate limiting (recommended)
    #[serde(default = "default_enable_rate_limiting")]
    pub enabled: bool,

    /// Custom rate limit tier ("tier-1", "tier-2", "tier-3", or "default")
    /// If not specified, will be auto-detected based on model
    #[serde(default)]
    pub tier: Option<String>,

    /// Custom requests per minute limit (overrides tier setting)
    #[serde(default)]
    pub requests_per_minute: Option<u32>,

    /// Custom minimum interval between requests in milliseconds
    #[serde(default)]
    pub min_interval_ms: Option<u64>,
}

// Default value functions
fn default_enforce_context_limit() -> bool {
    true
}

fn default_render_citations() -> bool {
    true
}

fn default_show_attribution() -> bool {
    true
}

fn default_max_results() -> u32 {
    10
}

fn default_highlight_citations() -> bool {
    true
}

fn default_enable_rate_limiting() -> bool {
    true
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Default, Serialize, Display)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SandboxMode {
    #[serde(rename = "read-only")]
    #[default]
    ReadOnly,

    #[serde(rename = "workspace-write")]
    WorkspaceWrite,

    #[serde(rename = "danger-full-access")]
    DangerFullAccess,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct SandboxWorkspaceWrite {
    #[serde(default)]
    pub writable_roots: Vec<PathBuf>,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub exclude_tmpdir_env_var: bool,
    #[serde(default)]
    pub exclude_slash_tmp: bool,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ShellEnvironmentPolicyInherit {
    /// "Core" environment variables for the platform. On UNIX, this would
    /// include HOME, LOGNAME, PATH, SHELL, and USER, among others.
    Core,

    /// Inherits the full environment from the parent process.
    #[default]
    All,

    /// Do not inherit any environment variables from the parent process.
    None,
}

/// Policy for building the `env` when spawning a process via either the
/// `shell` or `local_shell` tool.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ShellEnvironmentPolicyToml {
    pub inherit: Option<ShellEnvironmentPolicyInherit>,

    pub ignore_default_excludes: Option<bool>,

    /// List of regular expressions.
    pub exclude: Option<Vec<String>>,

    pub r#set: Option<HashMap<String, String>>,

    /// List of regular expressions.
    pub include_only: Option<Vec<String>>,

    pub experimental_use_profile: Option<bool>,
}

pub type EnvironmentVariablePattern = WildMatchPattern<'*', '?'>;

/// Deriving the `env` based on this policy works as follows:
/// 1. Create an initial map based on the `inherit` policy.
/// 2. If `ignore_default_excludes` is false, filter the map using the default
///    exclude pattern(s), which are: `"*KEY*"` and `"*TOKEN*"`.
/// 3. If `exclude` is not empty, filter the map using the provided patterns.
/// 4. Insert any entries from `r#set` into the map.
/// 5. If non-empty, filter the map using the `include_only` patterns.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShellEnvironmentPolicy {
    /// Starting point when building the environment.
    pub inherit: ShellEnvironmentPolicyInherit,

    /// True to skip the check to exclude default environment variables that
    /// contain "KEY" or "TOKEN" in their name.
    pub ignore_default_excludes: bool,

    /// Environment variable names to exclude from the environment.
    pub exclude: Vec<EnvironmentVariablePattern>,

    /// (key, value) pairs to insert in the environment.
    pub r#set: HashMap<String, String>,

    /// Environment variable names to retain in the environment.
    pub include_only: Vec<EnvironmentVariablePattern>,

    /// If true, the shell profile will be used to run the command.
    pub use_profile: bool,
}

impl From<ShellEnvironmentPolicyToml> for ShellEnvironmentPolicy {
    fn from(toml: ShellEnvironmentPolicyToml) -> Self {
        // Default to inheriting the full environment when not specified.
        let inherit = toml.inherit.unwrap_or(ShellEnvironmentPolicyInherit::All);
        let ignore_default_excludes = toml.ignore_default_excludes.unwrap_or(false);
        let exclude = toml
            .exclude
            .unwrap_or_default()
            .into_iter()
            .map(|s| EnvironmentVariablePattern::new_case_insensitive(&s))
            .collect();
        let r#set = toml.r#set.unwrap_or_default();
        let include_only = toml
            .include_only
            .unwrap_or_default()
            .into_iter()
            .map(|s| EnvironmentVariablePattern::new_case_insensitive(&s))
            .collect();
        let use_profile = toml.experimental_use_profile.unwrap_or(false);

        Self {
            inherit,
            ignore_default_excludes,
            exclude,
            r#set,
            include_only,
            use_profile,
        }
    }
}

/// See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#get-started-with-reasoning
#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ReasoningEffort {
    Minimal,
    Low,
    #[default]
    Medium,
    High,
    /// Option to disable reasoning.
    None,
}

/// A summary of the reasoning performed by the model. This can be useful for
/// debugging and understanding the model's reasoning process.
/// See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#reasoning-summaries
#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ReasoningSummary {
    #[default]
    Auto,
    Concise,
    Detailed,
    /// Option to disable reasoning summaries.
    None,
}

// Conversions from protocol enums to core config enums used where protocol
// values are supplied by clients and core needs its internal representations.
impl From<codex_protocol::config_types::ReasoningEffort> for ReasoningEffort {
    fn from(v: codex_protocol::config_types::ReasoningEffort) -> Self {
        match v {
            codex_protocol::config_types::ReasoningEffort::Low => ReasoningEffort::Low,
            codex_protocol::config_types::ReasoningEffort::Medium => ReasoningEffort::Medium,
            codex_protocol::config_types::ReasoningEffort::High => ReasoningEffort::High,
            codex_protocol::config_types::ReasoningEffort::None => ReasoningEffort::None,
        }
    }
}

impl From<codex_protocol::config_types::ReasoningSummary> for ReasoningSummary {
    fn from(v: codex_protocol::config_types::ReasoningSummary) -> Self {
        match v {
            codex_protocol::config_types::ReasoningSummary::Auto => ReasoningSummary::Auto,
            codex_protocol::config_types::ReasoningSummary::Concise => ReasoningSummary::Concise,
            codex_protocol::config_types::ReasoningSummary::Detailed => ReasoningSummary::Detailed,
            codex_protocol::config_types::ReasoningSummary::None => ReasoningSummary::None,
        }
    }
}
