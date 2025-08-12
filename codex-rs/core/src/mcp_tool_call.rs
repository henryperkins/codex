use std::time::Duration;
use std::time::Instant;

use tracing::error;
use tracing::info;

use crate::codex::Session;
use crate::models::FunctionCallOutputPayload;
use crate::models::ResponseInputItem;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::McpInvocation;
use crate::protocol::McpToolCallBeginEvent;
use crate::protocol::McpToolCallEndEvent;

/// Handles the specified tool call dispatches the appropriate
/// `McpToolCallBegin` and `McpToolCallEnd` events to the `Session`.
pub(crate) async fn handle_mcp_tool_call(
    sess: &Session,
    sub_id: &str,
    call_id: String,
    server: String,
    tool_name: String,
    arguments: String,
    timeout: Option<Duration>,
) -> ResponseInputItem {
    // Parse the `arguments` as JSON. An empty string is OK, but invalid JSON
    // is not.
    let arguments_value = if arguments.trim().is_empty() {
        None
    } else {
        match serde_json::from_str::<serde_json::Value>(&arguments) {
            Ok(value) => Some(value),
            Err(e) => {
                error!("failed to parse tool call arguments: {e}");
                return ResponseInputItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output: FunctionCallOutputPayload {
                        content: format!("err: {e}"),
                        success: Some(false),
                    },
                };
            }
        }
    };

    let invocation = McpInvocation {
        server: server.clone(),
        tool: tool_name.clone(),
        arguments: arguments_value.clone(),
    };

    let tool_call_begin_event = EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
        call_id: call_id.clone(),
        invocation: invocation.clone(),
    });
    notify_mcp_tool_call_event(sess, sub_id, tool_call_begin_event).await;

    let start = Instant::now();

    // Make the MCP call cancellable via per-turn cancellation token
    let cancel = sess.get_turn_cancel(sub_id);
    let result = tokio::select! {
        res = sess.call_tool(&server, &tool_name, arguments_value.clone(), timeout) => {
            res.map_err(|e| format!("tool call error: {e}"))
        }
        _ = cancel.notified() => {
            Err("tool call cancelled".to_string())
        }
    };

    let duration = start.elapsed();
    let success = match &result {
        Ok(r) => r.is_error != Some(true),
        Err(_) => false,
    };
    info!(
        "MCP tool call finished: server={} tool={} duration_ms={} success={}",
        server,
        tool_name,
        duration.as_millis(),
        success
    );
    let tool_call_end_event = EventMsg::McpToolCallEnd(McpToolCallEndEvent {
        call_id: call_id.clone(),
        invocation,
        duration,
        result: result.clone(),
    });

    notify_mcp_tool_call_event(sess, sub_id, tool_call_end_event.clone()).await;

    ResponseInputItem::McpToolCallOutput { call_id, result }
}

async fn notify_mcp_tool_call_event(sess: &Session, sub_id: &str, event: EventMsg) {
    sess.send_event(Event {
        id: sub_id.to_string(),
        msg: event,
    })
    .await;
}
