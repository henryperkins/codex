use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_channel::unbounded;
use async_trait::async_trait;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpInvocation;
use codex_protocol::protocol::McpToolCallBeginEvent;
use codex_protocol::protocol::McpToolCallEndEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::SessionTask;
use super::SessionTaskContext;
use crate::SandboxState;
use crate::codex::TurnContext;
use crate::mcp::CODEX_REPO_TOOLS_MCP_SERVER_NAME;
use crate::mcp::ToolPluginProvenance;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp::repo_tools_mcp_server_config;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::mcp_connection_manager::codex_apps_tools_cache_key;
use crate::state::TaskKind;

#[derive(Clone, Copy)]
pub(crate) struct RepoIndexRefreshTask {
    force_full: bool,
}

impl RepoIndexRefreshTask {
    pub(crate) fn new(force_full: bool) -> Self {
        Self { force_full }
    }
}

#[async_trait]
impl SessionTask for RepoIndexRefreshTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    fn span_name(&self) -> &'static str {
        "session_task.repo_index_refresh"
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        turn_context: Arc<TurnContext>,
        _input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        if cancellation_token.is_cancelled() {
            return None;
        }

        let session = session.clone_session();
        session
            .services
            .session_telemetry
            .counter("codex.task.repo_index_refresh", 1, &[]);

        let event = EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn_context.sub_id.clone(),
            model_context_window: turn_context.model_context_window(),
            collaboration_mode_kind: turn_context.collaboration_mode.mode,
        });
        session.send_event(turn_context.as_ref(), event).await;

        let mut arguments = serde_json::json!({
            "repo_root": turn_context.cwd.display().to_string(),
        });
        if self.force_full {
            arguments["force_full"] = serde_json::Value::Bool(true);
        }

        let call_id = Uuid::new_v4().to_string();
        let invocation = McpInvocation {
            server: CODEX_REPO_TOOLS_MCP_SERVER_NAME.to_string(),
            tool: "repo_index_refresh".to_string(),
            arguments: Some(arguments.clone()),
        };
        session
            .send_event(
                turn_context.as_ref(),
                EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                    call_id: call_id.clone(),
                    invocation: invocation.clone(),
                }),
            )
            .await;

        let start = Instant::now();
        let result: Result<CallToolResult, String> = if let Some(repo_tools_server_config) =
            repo_tools_mcp_server_config()
        {
            let server_name = CODEX_REPO_TOOLS_MCP_SERVER_NAME.to_string();
            let server_config = HashMap::from([(server_name.clone(), repo_tools_server_config)]);
            let auth_statuses = compute_auth_statuses(
                server_config.iter(),
                turn_context.config.mcp_oauth_credentials_store_mode,
            )
            .await;
            let sandbox_state = SandboxState {
                sandbox_policy: turn_context.sandbox_policy.get().clone(),
                codex_linux_sandbox_exe: turn_context.codex_linux_sandbox_exe.clone(),
                sandbox_cwd: turn_context.cwd.clone(),
                use_legacy_landlock: turn_context.features.use_legacy_landlock(),
            };
            let (tx_event, rx_event) = unbounded();
            drop(rx_event);

            let (manager, cancel_token) = McpConnectionManager::new(
                &server_config,
                turn_context.config.mcp_oauth_credentials_store_mode,
                auth_statuses,
                &turn_context.config.permissions.approval_policy,
                tx_event,
                sandbox_state,
                turn_context.config.codex_home.clone(),
                codex_apps_tools_cache_key(None),
                ToolPluginProvenance::default(),
            )
            .await;
            let result = manager
                .call_tool(
                    CODEX_REPO_TOOLS_MCP_SERVER_NAME,
                    "repo_index_refresh",
                    Some(arguments),
                )
                .await
                .map_err(|err| format!("tool call error: {err:#}"));
            cancel_token.cancel();
            result
        } else {
            Err(concat!(
                "tool call error: could not resolve a local Codex MCP launcher; ",
                "tried `codex-mcp-server` and `codex mcp-server`."
            )
            .to_string())
        };

        let status = if result.is_ok() { "ok" } else { "error" };
        turn_context
            .session_telemetry
            .counter("codex.mcp.call", 1, &[("status", status)]);

        session
            .send_event(
                turn_context.as_ref(),
                EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                    call_id,
                    invocation,
                    duration: start.elapsed(),
                    result,
                }),
            )
            .await;
        None
    }
}
