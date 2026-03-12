use std::sync::Arc;

use async_trait::async_trait;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::SessionTask;
use super::SessionTaskContext;
use crate::codex::TurnContext;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_tool_call::handle_mcp_tool_call;
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

        let arguments = if self.force_full {
            serde_json::json!({ "force_full": true }).to_string()
        } else {
            String::new()
        };
        let _ = handle_mcp_tool_call(
            Arc::clone(&session),
            &turn_context,
            Uuid::new_v4().to_string(),
            CODEX_APPS_MCP_SERVER_NAME.to_string(),
            "repo_index_refresh".to_string(),
            arguments,
        )
        .await;
        None
    }
}
