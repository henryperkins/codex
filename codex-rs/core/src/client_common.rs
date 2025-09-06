use crate::error::Result;
use crate::model_family::ModelFamily;
use crate::openai_tools::OpenAiTool;
use crate::protocol::TokenUsage;
use codex_apply_patch::APPLY_PATCH_TOOL_INSTRUCTIONS;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::config_types::Verbosity as VerbosityConfig;
use codex_protocol::models::ResponseItem;
use futures::Stream;
use serde::Serialize;
use std::borrow::Cow;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

/// The `instructions` field in the payload sent to a model should always start
/// with this content.
const BASE_INSTRUCTIONS: &str = include_str!("../prompt.md");

/// API request payload for a single model turn
#[derive(Default, Debug, Clone)]
pub struct Prompt {
    /// Conversation context input items.
    pub input: Vec<ResponseItem>,

    /// Tools available to the model, including additional tools sourced from
    /// external MCP servers.
    pub(crate) tools: Vec<OpenAiTool>,

    /// Optional override for the built-in BASE_INSTRUCTIONS.
    pub base_instructions_override: Option<String>,

    /// Optional `previous_response_id` used for response chaining on providers
    /// that support it (e.g., Azure Responses API). When present, the client
    /// will forward it in the request payload.
    pub previous_response_id: Option<String>,
}

impl Prompt {
    pub(crate) fn get_full_instructions(&self, model: &ModelFamily) -> Cow<'_, str> {
        let base = self
            .base_instructions_override
            .as_deref()
            .unwrap_or(BASE_INSTRUCTIONS);
        let mut sections: Vec<&str> = vec![base];

        // When there are no custom instructions, add apply_patch_tool_instructions if either:
        // - the model needs special instructions (4.1), or
        // - there is no apply_patch tool present
        let is_apply_patch_tool_present = self.tools.iter().any(|tool| match tool {
            OpenAiTool::Function(f) => f.name == "apply_patch",
            OpenAiTool::Freeform(f) => f.name == "apply_patch",
            _ => false,
        });
        if self.base_instructions_override.is_none()
            && (model.needs_special_apply_patch_instructions || !is_apply_patch_tool_present)
        {
            sections.push(APPLY_PATCH_TOOL_INSTRUCTIONS);
        }
        Cow::Owned(sections.join("\n"))
    }

    pub(crate) fn get_formatted_input(&self) -> Vec<ResponseItem> {
        // Sanitize the input before serialization:
        // - Drop `Reasoning` items entirely. These are server-emitted with
        //   immutable ids (e.g., `rs_…`). Re-sending them can trigger
        //   duplicate-id validation errors on providers that store response
        //   items (e.g., Azure Responses API).
        // - Clear server-assigned ids from items that may carry them
        //   (messages, function/custom/shell calls). These ids are minted by
        //   the server and are unique within its stored transcript. When
        //   chaining with `previous_response_id`, re-sending items that still
        //   have their original ids will trip validation with errors like
        //   "Duplicate item found with id fc_…". Clearing the ids keeps the
        //   semantic content while avoiding duplicate-id conflicts.
        self.input
            .iter()
            .filter_map(|item| match item {
                ResponseItem::Reasoning { .. } => None,
                ResponseItem::Message { role, content, .. } => Some(ResponseItem::Message {
                    id: None,
                    role: role.clone(),
                    content: content.clone(),
                }),
                ResponseItem::FunctionCall {
                    id: _,
                    name,
                    arguments,
                    call_id,
                } => Some(ResponseItem::FunctionCall {
                    id: None,
                    name: name.clone(),
                    arguments: arguments.clone(),
                    call_id: call_id.clone(),
                }),
                ResponseItem::LocalShellCall {
                    id: _,
                    call_id,
                    status,
                    action,
                } => Some(ResponseItem::LocalShellCall {
                    id: None,
                    call_id: call_id.clone(),
                    status: status.clone(),
                    action: action.clone(),
                }),
                ResponseItem::CustomToolCall {
                    id: _,
                    status,
                    call_id,
                    name,
                    input,
                } => Some(ResponseItem::CustomToolCall {
                    id: None,
                    status: status.clone(),
                    call_id: call_id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                }),
                other => Some(other.clone()),
            })
            .collect()
    }
}

#[derive(Debug)]
pub enum ResponseEvent {
    Created,
    OutputItemDone(ResponseItem),
    Completed {
        response_id: String,
        token_usage: Option<TokenUsage>,
    },
    Incomplete {
        response_id: String,
        _reason: Option<String>,
    },
    OutputTextDelta(String),
    ReasoningSummaryDelta(String),
    ReasoningContentDelta(String),
    ReasoningSummaryPartAdded,
    WebSearchCallBegin {
        call_id: String,
    },
    
    // Azure Responses API specific events
    Queued,
    InProgress,
    Failed(serde_json::Value),
    Error(serde_json::Value),
    
    // Output events with indices and IDs for Azure
    OutputItemAdded {
        output_index: u32,
        item: serde_json::Value,
    },
    OutputTextDeltaIndexed {
        output_index: u32,
        content_index: u32,
        item_id: String,
        delta: String,
    },
    OutputTextDone {
        output_index: u32,
        content_index: u32,
        item_id: String,
        text: String,
    },
    RefusalDelta {
        output_index: u32,
        content_index: u32,
        item_id: String,
        delta: String,
    },
    RefusalDone {
        output_index: u32,
        content_index: u32,
        item_id: String,
        refusal: String,
    },
    
    // Reasoning events
    ReasoningDelta {
        delta: String,
    },
    ReasoningDone {
        reasoning: String,
    },
    
    // Generic event for unknown/future events
    Unknown {
        event_type: String,
        payload: serde_json::Value,
    },
}

#[derive(Debug, Serialize)]
pub(crate) struct Reasoning {
    pub(crate) effort: ReasoningEffortConfig,
    pub(crate) summary: ReasoningSummaryConfig,
}

/// Controls under the `text` field in the Responses API for GPT-5.
#[derive(Debug, Serialize, Default, Clone, Copy)]
pub(crate) struct TextControls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) verbosity: Option<OpenAiVerbosity>,
}

#[derive(Debug, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OpenAiVerbosity {
    Low,
    #[default]
    Medium,
    High,
}

impl From<VerbosityConfig> for OpenAiVerbosity {
    fn from(v: VerbosityConfig) -> Self {
        match v {
            VerbosityConfig::Low => OpenAiVerbosity::Low,
            VerbosityConfig::Medium => OpenAiVerbosity::Medium,
            VerbosityConfig::High => OpenAiVerbosity::High,
        }
    }
}

/// Request object that is serialized as JSON and POST'ed when using the
/// Responses API.
#[derive(Debug, Serialize)]
pub(crate) struct ResponsesApiRequest<'a> {
    pub(crate) model: &'a str,
    pub(crate) instructions: &'a str,
    // TODO(mbolin): ResponseItem::Other should not be serialized. Currently,
    // we code defensively to avoid this case, but perhaps we should use a
    // separate enum for serialization.
    pub(crate) input: &'a Vec<ResponseItem>,
    pub(crate) tools: &'a [serde_json::Value],
    pub(crate) tool_choice: &'static str,
    pub(crate) parallel_tool_calls: bool,
    // Match OpenAI's client behavior: only include `reasoning` when supported
    // by the model family. When unsupported, omit the field entirely rather
    // than sending `null` so providers (including Azure) don't see a stray key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning: Option<Reasoning>,
    pub(crate) store: bool,
    pub(crate) stream: bool,
    pub(crate) include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<TextControls>,
}

pub(crate) fn create_reasoning_param_for_request(
    model_family: &ModelFamily,
    effort: ReasoningEffortConfig,
    summary: ReasoningSummaryConfig,
) -> Option<Reasoning> {
    if model_family.supports_reasoning_summaries {
        Some(Reasoning { effort, summary })
    } else {
        None
    }
}

pub(crate) fn create_text_param_for_request(
    verbosity: Option<VerbosityConfig>,
) -> Option<TextControls> {
    verbosity.map(|v| TextControls {
        verbosity: Some(v.into()),
    })
}

pub struct ResponseStream {
    pub(crate) rx_event: mpsc::Receiver<Result<ResponseEvent>>,
}

impl Stream for ResponseStream {
    type Item = Result<ResponseEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use crate::model_family::find_family_for_model;

    use super::*;

    #[test]
    fn get_full_instructions_no_user_content() {
        let prompt = Prompt {
            ..Default::default()
        };
        let expected = format!("{BASE_INSTRUCTIONS}\n{APPLY_PATCH_TOOL_INSTRUCTIONS}");
        let model_family = find_family_for_model("gpt-4.1").expect("known model slug");
        let full = prompt.get_full_instructions(&model_family);
        assert_eq!(full, expected);
    }

    #[test]
    fn serializes_text_verbosity_when_set() {
        let input: Vec<ResponseItem> = vec![];
        let tools: Vec<serde_json::Value> = vec![];
        let req = ResponsesApiRequest {
            model: "gpt-5",
            instructions: "i",
            input: &input,
            tools: &tools,
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: vec![],
            prompt_cache_key: None,
            text: Some(TextControls {
                verbosity: Some(OpenAiVerbosity::Low),
            }),
        };

        let v = serde_json::to_value(&req).expect("json");
        assert_eq!(
            v.get("text")
                .and_then(|t| t.get("verbosity"))
                .and_then(|s| s.as_str()),
            Some("low")
        );
    }

    #[test]
    fn omits_text_when_not_set() {
        let input: Vec<ResponseItem> = vec![];
        let tools: Vec<serde_json::Value> = vec![];
        let req = ResponsesApiRequest {
            model: "gpt-5",
            instructions: "i",
            input: &input,
            tools: &tools,
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: vec![],
            prompt_cache_key: None,
            text: None,
        };

        let v = serde_json::to_value(&req).expect("json");
        assert!(v.get("text").is_none());
    }

    #[test]
    fn formatted_input_clears_server_assigned_ids_and_drops_reasoning() {
        use codex_protocol::models::ContentItem;
        use codex_protocol::models::LocalShellAction;
        use codex_protocol::models::LocalShellExecAction;
        use codex_protocol::models::LocalShellStatus;
        use codex_protocol::models::ResponseItem;

        let prompt = Prompt {
            input: vec![
                // Server-emitted reasoning should be dropped entirely.
                ResponseItem::Reasoning {
                    id: "rs_123".to_string(),
                    summary: vec![],
                    content: None,
                    encrypted_content: None,
                },
                // Message id should be cleared.
                ResponseItem::Message {
                    id: Some("msg_1".to_string()),
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "hello".to_string(),
                    }],
                },
                // Function call id should be cleared but call_id preserved.
                ResponseItem::FunctionCall {
                    id: Some("fc_1".to_string()),
                    name: "shell".to_string(),
                    arguments: "{}".to_string(),
                    call_id: "call_1".to_string(),
                },
                // Local shell call id should be cleared; call_id/status/action preserved.
                ResponseItem::LocalShellCall {
                    id: Some("ls_1".to_string()),
                    call_id: Some("call_2".to_string()),
                    status: LocalShellStatus::InProgress,
                    action: LocalShellAction::Exec(LocalShellExecAction {
                        command: vec!["echo".to_string(), "hi".to_string()],
                        timeout_ms: None,
                        working_directory: None,
                        env: None,
                        user: None,
                    }),
                },
                // Custom tool call id should be cleared.
                ResponseItem::CustomToolCall {
                    id: Some("ctc_1".to_string()),
                    status: Some("in_progress".to_string()),
                    call_id: "call_3".to_string(),
                    name: "custom.tool".to_string(),
                    input: "{}".to_string(),
                },
            ],
            ..Default::default()
        };

        let formatted = prompt.get_formatted_input();

        // Reasoning item dropped; remaining 4 entries.
        assert_eq!(formatted.len(), 4);

        // Message id cleared
        match &formatted[0] {
            ResponseItem::Message { id, role, content } => {
                assert!(id.is_none());
                assert_eq!(role, "assistant");
                assert!(matches!(content[0], ContentItem::OutputText { .. }));
            }
            _ => panic!("unexpected variant for formatted[0]"),
        }

        // Function call id cleared, call_id intact
        match &formatted[1] {
            ResponseItem::FunctionCall {
                id,
                name,
                arguments,
                call_id,
            } => {
                assert!(id.is_none());
                assert_eq!(name, "shell");
                assert_eq!(arguments, "{}");
                assert_eq!(call_id, "call_1");
            }
            _ => panic!("unexpected variant for formatted[1]"),
        }

        // Local shell call id cleared, other fields preserved
        match &formatted[2] {
            ResponseItem::LocalShellCall {
                id,
                call_id,
                status,
                action,
            } => {
                assert!(id.is_none());
                assert_eq!(call_id.as_deref(), Some("call_2"));
                assert!(matches!(status, LocalShellStatus::InProgress));
                assert!(matches!(action, LocalShellAction::Exec(_)));
            }
            _ => panic!("unexpected variant for formatted[2]"),
        }

        // Custom tool call id cleared, other fields preserved
        match &formatted[3] {
            ResponseItem::CustomToolCall {
                id,
                status,
                call_id,
                name,
                input,
            } => {
                assert!(id.is_none());
                assert_eq!(status.as_deref(), Some("in_progress"));
                assert_eq!(call_id, "call_3");
                assert_eq!(name, "custom.tool");
                assert_eq!(input, "{}");
            }
            _ => panic!("unexpected variant for formatted[3]"),
        }
    }
}
