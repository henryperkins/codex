# Codex TUI Current State (September 16, 2025)

## Layout Overview
- `tui/src/app.rs` drives the application event loop and owns the root layout: history stream rendered by `ChatWidget`, bottom composer via `BottomPane`, and optional pager overlays for transcripts or diffs.
- The viewport is inline (not alt-screen) and relies on `tui/src/tui.rs` to manage raw mode, bracketed paste, and frame scheduling while leaving terminal scrollback intact.
- Rendering happens in response to `TuiEvent::Draw`, where `ChatWidget` decides desired height and cursor placement before the frame renders.

## Conversation History & Timeline
- History is modeled as a list of `HistoryCell` implementations in `tui/src/history_cell.rs`; each cell knows how to render itself within the available width.
- `ChatWidget` (`tui/src/chatwidget.rs`) adds new cells for user prompts, agent output, command execution, MCP tool calls, web searches, diffs, errors, and reasoning breadcrumbs.
- Streaming answers use `tui/src/streaming/controller.rs` to gate newline commits and drive an animated "commit" ticker until the turn finalizes.
- Session metadata (model, cwd, reasoning effort) is inserted as a boxed header cell when a session configures, using the structured border renderer in `SessionHeaderHistoryCell`.

## Bottom Pane & Composer
- `BottomPane` (`tui/src/bottom_pane/mod.rs`) owns the lower section, toggling between the interactive composer and modal views.
- The `ChatComposer` (`tui/src/bottom_pane/chat_composer.rs`) provides the multi-line textarea, slash-command palette, `@` file search, queued attachments, token usage display, and context hints.
- A status indicator (`tui/src/status_indicator_widget.rs`) appears above the composer while work is in progress, showing shimmering headers, elapsed time, queue reminders, and Esc-interrupt guidance.
- Paste bursts, history recall, and command/file popups have their own helpers under `tui/src/bottom_pane/` (e.g., `paste_burst.rs`, `command_popup.rs`, `file_search_popup.rs`).

## Approvals, Notifications & Overlays
- Approval flows for commands and patches use the modal hierarchy in `tui/src/user_approval_widget.rs` and `tui/src/bottom_pane/approval_modal_view.rs`, capturing input until the user decides.
- Non-blocking toast notifications are triggered through `ChatWidget::notify` and surfaced by `tui::Tui::notify`, respecting per-config notification settings.
- Long-form or alternate views (diffs, transcripts, static text) appear via the pager overlay system in `tui/src/pager_overlay.rs`, offering scroll and key hints without leaving inline mode.

## Streaming Lifecycle & Turn Management
- `ChatWidget` coordinates turn state: it queues user messages while the model runs, manages `StreamController` lifecycle, and flushes active exec cells when transitions occur.
- Command execution history shows spinner-to-checkmark transitions, captured stdout/stderr, and apply-patch results, with failure cases rendered in red.
- Token usage updates (`EventMsg::TokenCount`) propagate both to bottom-pane hints and stored session state for later display.

## Rendering Details & Markdown Support
- Markdown is parsed in `tui/src/markdown_render.rs`, supporting headings, emphasis, block quotes, lists, code blocks (with optional fenced language tags), and inline links/citations.
- Bash command highlighting is implemented in `tui/src/render/highlight.rs`; other languages fall back to plain styling.
- The shimmer effect in `tui/src/shimmer.rs` powers animated headers (status widget today) and adapts to RGB or basic color terminals.
- Wrapping helpers in `tui/src/wrapping.rs` and line utilities in `tui/src/render/line_utils.rs` keep table-free text aligned with viewport width.

## Supporting Systems
- File search orchestration (`tui/src/file_search.rs`) debounces `@` queries, cancels stale searches, and streams results back to the composer.
- Resume and onboarding flows live in `tui/src/resume_picker.rs` and `tui/src/onboarding/`, providing first-run guidance and session restoration.
- Session headers update through `tui/src/chatwidget/session_header.rs`, while notifications and backtrack state live in `tui/src/app_backtrack.rs` and related modules.

This overview captures the on-disk implementation and user-visible behavior as of September 16, 2025.
