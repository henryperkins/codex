use std::pin::Pin;
use std::sync::Arc;

use codex_protocol::config_types::ModeKind;
use codex_protocol::items::TurnItem;
use codex_utils_stream_parser::strip_citations;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::error::Result;
use crate::function_tool::FunctionCallError;
use crate::memories::citations::get_thread_id_from_citations;
use crate::parse_turn_item;
use crate::state_db;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::router::ToolRouter;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_utils_stream_parser::strip_proposed_plan_blocks;
use futures::Future;
use tracing::debug;
use tracing::instrument;
use uuid::Uuid;

fn strip_hidden_assistant_markup(text: &str, plan_mode: bool) -> String {
    let (without_citations, _) = strip_citations(text);
    if plan_mode {
        strip_proposed_plan_blocks(&without_citations)
    } else {
        without_citations
    }
}

pub(crate) fn raw_assistant_output_text_from_item(item: &ResponseItem) -> Option<String> {
    if let ResponseItem::Message { role, content, .. } = item
        && role == "assistant"
    {
        let combined = content
            .iter()
            .filter_map(|ci| match ci {
                codex_protocol::models::ContentItem::OutputText { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        return Some(combined);
    }
    None
}

/// Persist a completed model response item and record any cited memory usage.
pub(crate) async fn record_completed_response_item(
    sess: &Session,
    turn_context: &TurnContext,
    item: &ResponseItem,
) {
    sess.record_conversation_items(turn_context, std::slice::from_ref(item))
        .await;
    maybe_mark_thread_memory_mode_polluted_from_web_search(sess, turn_context, item).await;
    record_stage1_output_usage_for_completed_item(turn_context, item).await;
}

async fn maybe_mark_thread_memory_mode_polluted_from_web_search(
    sess: &Session,
    turn_context: &TurnContext,
    item: &ResponseItem,
) {
    if !turn_context
        .config
        .memories
        .no_memories_if_mcp_or_web_search
        || !matches!(item, ResponseItem::WebSearchCall { .. })
    {
        return;
    }
    state_db::mark_thread_memory_mode_polluted(
        sess.services.state_db.as_deref(),
        sess.conversation_id,
        "record_completed_response_item",
    )
    .await;
}

async fn record_stage1_output_usage_for_completed_item(
    turn_context: &TurnContext,
    item: &ResponseItem,
) {
    let Some(raw_text) = raw_assistant_output_text_from_item(item) else {
        return;
    };

    let (_, citations) = strip_citations(&raw_text);
    let thread_ids = get_thread_id_from_citations(citations);
    if thread_ids.is_empty() {
        return;
    }

    if let Some(db) = state_db::get_state_db(turn_context.config.as_ref(), None).await {
        let _ = db.record_stage1_output_usage(&thread_ids).await;
    }
}

/// Handle a completed output item from the model stream, recording it and
/// queuing any tool execution futures. This records items immediately so
/// history and rollout stay in sync even if the turn is later cancelled.
pub(crate) type InFlightFuture<'f> =
    Pin<Box<dyn Future<Output = Result<ResponseInputItem>> + Send + 'f>>;

#[derive(Default)]
pub(crate) struct OutputItemResult {
    pub last_agent_message: Option<String>,
    pub needs_follow_up: bool,
    pub tool_future: Option<InFlightFuture<'static>>,
}

pub(crate) struct HandleOutputCtx {
    pub sess: Arc<Session>,
    pub turn_context: Arc<TurnContext>,
    pub tool_runtime: ToolCallRuntime,
    pub cancellation_token: CancellationToken,
}

#[instrument(level = "trace", skip_all)]
pub(crate) async fn handle_output_item_done(
    ctx: &mut HandleOutputCtx,
    item: ResponseItem,
    previously_active_item: Option<TurnItem>,
) -> Result<OutputItemResult> {
    let mut output = OutputItemResult::default();
    let plan_mode = ctx.turn_context.collaboration_mode.mode == ModeKind::Plan;

    match ToolRouter::build_tool_call(ctx.sess.as_ref(), item.clone()).await {
        // The model emitted a tool call; log it, persist the item immediately, and queue the tool execution.
        Ok(Some(call)) => {
            let payload_preview = call.payload.log_payload().into_owned();
            tracing::info!(
                thread_id = %ctx.sess.conversation_id,
                "ToolCall: {} {}",
                call.tool_name,
                payload_preview
            );

            record_completed_response_item(ctx.sess.as_ref(), ctx.turn_context.as_ref(), &item)
                .await;

            let cancellation_token = ctx.cancellation_token.child_token();
            let tool_future: InFlightFuture<'static> = Box::pin(
                ctx.tool_runtime
                    .clone()
                    .handle_tool_call(call, cancellation_token),
            );

            output.needs_follow_up = true;
            output.tool_future = Some(tool_future);
        }
        // No tool call: convert messages/reasoning into turn items and mark them as complete.
        Ok(None) => {
            if let Some(turn_item) = handle_non_tool_response_item(&item, plan_mode) {
                if previously_active_item.is_none() {
                    let mut started_item = turn_item.clone();
                    if let TurnItem::ImageGeneration(item) = &mut started_item {
                        item.status = "in_progress".to_string();
                        item.revised_prompt = None;
                        item.result.clear();
                    }
                    ctx.sess
                        .emit_turn_item_started(&ctx.turn_context, &started_item)
                        .await;
                }

                ctx.sess
                    .emit_turn_item_completed(&ctx.turn_context, turn_item)
                    .await;
            }

            record_completed_response_item(ctx.sess.as_ref(), ctx.turn_context.as_ref(), &item)
                .await;
            let last_agent_message = last_assistant_message_from_item(&item, plan_mode);

            output.last_agent_message = last_agent_message;
        }
        // Guardrail: the model issued a LocalShellCall without an id; surface the error back into history.
        Err(FunctionCallError::MissingLocalShellCallId) => {
            let msg = "LocalShellCall without call_id or id";
            ctx.turn_context
                .otel_manager
                .log_tool_failed("local_shell", msg);
            tracing::error!(msg);

            for response_item in tool_error_response_items(&item, msg.to_string()) {
                ctx.sess
                    .record_conversation_items(
                        &ctx.turn_context,
                        std::slice::from_ref(&response_item),
                    )
                    .await;
            }

            output.needs_follow_up = true;
        }
        // The tool request should be answered directly (or was denied); push that response into the transcript.
        Err(FunctionCallError::RespondToModel(message)) => {
            record_completed_response_item(ctx.sess.as_ref(), ctx.turn_context.as_ref(), &item)
                .await;
            for response_item in tool_error_response_items(&item, message) {
                ctx.sess
                    .record_conversation_items(
                        &ctx.turn_context,
                        std::slice::from_ref(&response_item),
                    )
                    .await;
            }

            output.needs_follow_up = true;
        }
        // A fatal error occurred; surface it back into history.
        Err(FunctionCallError::Fatal(message)) => {
            return Err(CodexErr::Fatal(message));
        }
    }

    Ok(output)
}

pub(crate) fn handle_non_tool_response_item(
    item: &ResponseItem,
    plan_mode: bool,
) -> Option<TurnItem> {
    debug!(?item, "Output item");

    match item {
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. } => {
            let mut turn_item = parse_turn_item(item)?;
            if let TurnItem::AgentMessage(agent_message) = &mut turn_item {
                let combined = agent_message
                    .content
                    .iter()
                    .map(|entry| match entry {
                        codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
                    })
                    .collect::<String>();
                let stripped = strip_hidden_assistant_markup(&combined, plan_mode);
                agent_message.content =
                    vec![codex_protocol::items::AgentMessageContent::Text { text: stripped }];
            }
            Some(turn_item)
        }
        ResponseItem::FunctionCallOutput { .. } | ResponseItem::CustomToolCallOutput { .. } => {
            debug!("unexpected tool output from stream");
            None
        }
        _ => None,
    }
}

pub(crate) fn last_assistant_message_from_item(
    item: &ResponseItem,
    plan_mode: bool,
) -> Option<String> {
    if let Some(combined) = raw_assistant_output_text_from_item(item) {
        if combined.is_empty() {
            return None;
        }
        let stripped = strip_hidden_assistant_markup(&combined, plan_mode);
        if stripped.trim().is_empty() {
            return None;
        }
        return Some(stripped);
    }
    None
}

fn tool_error_response_items(item: &ResponseItem, message: String) -> Vec<ResponseItem> {
    let output_payload = FunctionCallOutputPayload {
        body: FunctionCallOutputBody::Text(message),
        ..Default::default()
    };

    match item {
        ResponseItem::FunctionCall { call_id, .. }
        | ResponseItem::CustomToolCall { call_id, .. } => {
            vec![ResponseItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output: output_payload,
            }]
        }
        ResponseItem::LocalShellCall {
            id,
            call_id,
            status,
            action,
        } => {
            let fallback_call_id = call_id
                .clone()
                .or_else(|| id.clone())
                .unwrap_or_else(|| format!("local_shell_fallback_{}", Uuid::new_v4()));
            vec![
                ResponseItem::LocalShellCall {
                    id: None,
                    call_id: Some(fallback_call_id.clone()),
                    status: status.clone(),
                    action: action.clone(),
                },
                ResponseItem::FunctionCallOutput {
                    call_id: fallback_call_id,
                    output: output_payload,
                },
            ]
        }
        _ => {
            tracing::error!(?item, "tool error returned for non-tool response item");
            let fallback_call_id = format!("tool_error_fallback_{}", Uuid::new_v4());
            vec![ResponseItem::FunctionCallOutput {
                call_id: fallback_call_id,
                output: output_payload,
            }]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::handle_non_tool_response_item;
    use super::last_assistant_message_from_item;
    use super::tool_error_response_items;
    use crate::context_manager::ContextManager;
    use crate::truncate::TruncationPolicy;
    use codex_protocol::items::TurnItem;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::LocalShellAction;
    use codex_protocol::models::LocalShellExecAction;
    use codex_protocol::models::LocalShellStatus;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::openai_models::default_input_modalities;
    use pretty_assertions::assert_eq;

    fn assistant_output_text(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: Some("msg-1".to_string()),
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
            end_turn: Some(true),
            phase: None,
        }
    }

    #[test]
    fn handle_non_tool_response_item_strips_citations_from_assistant_message() {
        let item = assistant_output_text("hello<oai-mem-citation>doc1</oai-mem-citation> world");

        let turn_item =
            handle_non_tool_response_item(&item, false).expect("assistant message should parse");

        let TurnItem::AgentMessage(agent_message) = turn_item else {
            panic!("expected agent message");
        };
        let text = agent_message
            .content
            .iter()
            .map(|entry| match entry {
                codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
            })
            .collect::<String>();
        assert_eq!(text, "hello world");
    }

    #[test]
    fn last_assistant_message_from_item_strips_citations_and_plan_blocks() {
        let item = assistant_output_text(
            "before<oai-mem-citation>doc1</oai-mem-citation>\n<proposed_plan>\n- x\n</proposed_plan>\nafter",
        );

        let message = last_assistant_message_from_item(&item, true)
            .expect("assistant text should remain after stripping");

        assert_eq!(message, "before\nafter");
    }

    #[test]
    fn last_assistant_message_from_item_returns_none_for_citation_only_message() {
        let item = assistant_output_text("<oai-mem-citation>doc1</oai-mem-citation>");

        assert_eq!(last_assistant_message_from_item(&item, false), None);
    }

    #[test]
    fn last_assistant_message_from_item_returns_none_for_plan_only_hidden_message() {
        let item = assistant_output_text("<proposed_plan>\n- x\n</proposed_plan>");

        assert_eq!(last_assistant_message_from_item(&item, true), None);
    }

    #[test]
    fn tool_error_response_items_uses_existing_function_call_id() {
        let call = ResponseItem::FunctionCall {
            id: None,
            name: "shell".to_string(),
            arguments: "{}".to_string(),
            call_id: "call-123".to_string(),
        };

        let items = tool_error_response_items(&call, "failure".to_string());
        assert_eq!(
            items,
            vec![ResponseItem::FunctionCallOutput {
                call_id: "call-123".to_string(),
                output: codex_protocol::models::FunctionCallOutputPayload {
                    body: codex_protocol::models::FunctionCallOutputBody::Text(
                        "failure".to_string(),
                    ),
                    success: None,
                },
            }]
        );
    }

    #[test]
    fn function_call_error_output_survives_prompt_normalization() {
        let call = ResponseItem::FunctionCall {
            id: None,
            name: "shell".to_string(),
            arguments: "{}".to_string(),
            call_id: "call-123".to_string(),
        };
        let output_items = tool_error_response_items(&call, "failure".to_string());

        let mut history = ContextManager::new();
        history.record_items([&call], TruncationPolicy::Tokens(10_000));
        history.record_items(output_items.iter(), TruncationPolicy::Tokens(10_000));
        let prompt_items = history.for_prompt(&default_input_modalities());

        assert_eq!(prompt_items.len(), 2);
        assert_eq!(prompt_items[0], call);
        assert_eq!(
            prompt_items[1],
            ResponseItem::FunctionCallOutput {
                call_id: "call-123".to_string(),
                output: codex_protocol::models::FunctionCallOutputPayload {
                    body: codex_protocol::models::FunctionCallOutputBody::Text(
                        "failure".to_string(),
                    ),
                    success: None,
                },
            }
        );
    }

    #[test]
    fn tool_error_response_items_synthesizes_pair_for_missing_local_shell_call_id() {
        let local_shell = ResponseItem::LocalShellCall {
            id: None,
            call_id: None,
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["/bin/echo".to_string(), "hello".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
        };

        let items = tool_error_response_items(&local_shell, "tool failed".to_string());
        assert_eq!(items.len(), 2);

        let ResponseItem::LocalShellCall {
            id: synthetic_id,
            call_id: synthetic_call_id,
            status,
            action,
        } = &items[0]
        else {
            panic!("expected synthesized local_shell_call");
        };
        assert_eq!(synthetic_id, &None);
        assert_eq!(status, &LocalShellStatus::Completed);
        assert_eq!(
            action,
            &LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["/bin/echo".to_string(), "hello".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            })
        );
        let Some(synthetic_call_id) = synthetic_call_id.clone() else {
            panic!("synthesized local_shell_call should have call_id");
        };
        assert!(!synthetic_call_id.is_empty());

        let ResponseItem::FunctionCallOutput { call_id, output } = &items[1] else {
            panic!("expected function_call_output");
        };
        assert_eq!(call_id, &synthetic_call_id);
        assert_eq!(
            output,
            &codex_protocol::models::FunctionCallOutputPayload {
                body: codex_protocol::models::FunctionCallOutputBody::Text(
                    "tool failed".to_string(),
                ),
                success: None,
            }
        );
    }

    #[test]
    fn synthesized_local_shell_error_output_survives_prompt_normalization() {
        let local_shell = ResponseItem::LocalShellCall {
            id: None,
            call_id: None,
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["/bin/echo".to_string(), "hello".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
        };

        let synthesized = tool_error_response_items(&local_shell, "tool failed".to_string());
        let mut history = ContextManager::new();
        history.record_items(synthesized.iter(), TruncationPolicy::Tokens(10_000));
        let prompt_items = history.for_prompt(&default_input_modalities());

        assert_eq!(prompt_items.len(), 2);

        let ResponseItem::LocalShellCall {
            call_id: Some(call_id),
            ..
        } = &prompt_items[0]
        else {
            panic!("expected local_shell_call first");
        };

        let ResponseItem::FunctionCallOutput {
            call_id: output_call_id,
            ..
        } = &prompt_items[1]
        else {
            panic!("expected function_call_output second");
        };
        assert_eq!(output_call_id, call_id);
    }
}
