# Query Project Review Remediation Plan

Date: 2026-03-09

This plan covers the two findings from the uncommitted-change review:

1. Query-project result opening can resolve relative paths against stale app cwd state after the session cwd changes.
2. The `/index` qdrant diagnostics can describe the wrong root for collection-name derivation because they display a non-canonical cwd while qdrant hashes the canonical repo root.

## Finding 1: Query-project result opening uses the wrong base directory

### Goal

Make query-project result opening use the same root that the tool call used when it produced the result, instead of relying on stale app-level cwd state.

### Files to touch

- `codex-rs/tui/src/history_cell.rs`
- `codex-rs/tui/src/app.rs`
- `codex-rs/tui/src/chatwidget.rs` only if you need an existing cwd accessor instead of duplicating state lookup
- `codex-rs/tui/src/*tests*.rs` or the existing test module in `app.rs`

### Step-by-step implementation plan

1. Add a focused regression test before changing behavior.
   - Put the test next to the existing query-project key-action coverage in `codex-rs/tui/src/app.rs`.
   - Build an app/chat state where:
     - the original `app.config.cwd` is one directory,
     - the live session cwd in the chat widget is a different directory,
     - the latest completed `query_project` result returns a relative path such as `src/lib.rs`,
     - the payload includes `repo_root` matching the live session cwd.
   - Trigger the same open-result path that Alt-O uses.
   - Assert that the resolved path points under the live/root payload directory, not the original app config cwd.
   - Assert that the old `"query_project result path does not exist"` error is not emitted for the valid live-root path.

2. Carry `repo_root` through the selected-result model.
   - Extend `QueryProjectSelectedResult` in `codex-rs/tui/src/history_cell.rs` to include the parsed `repo_root`.
   - Populate it from `QueryProjectPayload` inside `selected_query_project_result()`.
   - Keep the existing guardrails:
     - return `None` when the selected path is empty,
     - preserve current line-number handling.

3. Update result-path resolution to use the result payload instead of stale app config.
   - Change `resolve_query_project_result_path()` in `codex-rs/tui/src/app.rs` to accept the selected result, not only the raw path string.
   - Resolve in this order:
     - if the result path is absolute, use it as-is,
     - else if the selected result includes a non-empty `repo_root`, join against that root,
     - else join against the live chat-widget cwd/config state, not `self.config.cwd`.
   - Use the same live session state the TUI already trusts for `/index` and status-line cwd output.

4. Keep the open-result path consistent with the review fix.
   - Update `open_selected_query_project_result()` to pass the full selected result into the resolver.
   - Keep the existing existence check and editor-launch flow unchanged after the path is resolved.

5. Improve the failure signal if the resolved file is still missing.
   - If the path still does not exist, keep the current error path but include enough context to debug quickly.
   - Preferred shape: mention both the resolved absolute path and the base root that was used.
   - Do not change the success path or editor invocation behavior.

6. Add one more narrow test for the fallback path.
   - Cover the case where `repo_root` is absent from the payload and the resolver falls back to the live session cwd.
   - This prevents the new fix from only working for structured payloads that already include `repo_root`.

### Validation steps

1. Run the new focused TUI app test(s) that exercise result opening.
2. Re-run the existing targeted query-project TUI tests:
   - `cargo test -p codex-tui completed_query_project_structured_content_renders_summary_lines`
   - `cargo test -p codex-tui completed_query_project_supports_view_toggles_and_selection`
3. If the visible text output changes, update or add the required `insta` snapshot coverage.
4. Run `just fmt` in `codex-rs`.

## Finding 2: `/index` qdrant diagnostics can describe the wrong collection root

### Goal

Make the `/index` output describe the same root semantics that the qdrant collection-name logic actually uses.

### Files to touch

- `codex-rs/tui/src/chatwidget.rs`
- `codex-rs/tui/src/chatwidget/tests.rs`
- `codex-rs/tui/src/chatwidget/snapshots/codex_tui__chatwidget__tests__slash_index_qdrant_defaults_output.snap`
- `codex-rs/docs/codex_mcp_interface.md` if the user-facing wording of the qdrant defaults changes materially

### Step-by-step implementation plan

1. Define the exact display contract first.
   - The current `/index` output mixes two concepts:
     - the live session cwd shown to the user,
     - the canonical repo root that qdrant hashes into the collection name.
   - Decide which of these should be shown explicitly.
   - Recommended contract:
     - keep showing the live session default root,
     - add a separate canonicalized qdrant collection root when backend = `Qdrant`.

2. Mirror the server-side root semantics in the TUI.
   - Read the current cwd from the same live source already used by `/index`.
   - Canonicalize that path using the same practical rules as `query_project`:
     - treat the session cwd as the default root,
     - resolve it to a canonical path before explaining qdrant collection derivation,
     - if canonicalization fails, fall back to the raw displayed cwd and mark the qdrant root as unresolved instead of lying.
   - Do not change the backend behavior in this step; this is a diagnostics-alignment fix.

3. Update the `/index` text so the labels are unambiguous.
   - Rename or clarify the existing `default-repo-root` line if needed.
   - Replace the current generic sentence about qdrant collection names with wording that points to the canonicalized collection root actually used for hashing.
   - Keep the `qdrant-collection-prefix` line because it is still useful, but make it clear that the prefix is only one part of the derived name.

4. Add a deterministic test for the new output contract.
   - Extend `slash_index_shows_qdrant_defaults_output` in `codex-rs/tui/src/chatwidget/tests.rs`.
   - Assert both:
     - the displayed default/live root,
     - the qdrant-specific root explanation line.
   - If you can produce a stable canonicalization fixture in the test environment, use it.
   - If filesystem canonicalization would make the test brittle, structure the rendering so the test can inject the already-resolved strings and snapshot only the final output.

5. Update the snapshot intentionally.
   - Regenerate the affected qdrant `/index` snapshot.
   - Review the final wording for clarity and brevity before accepting it.

6. Update docs if the user-facing behavior changed materially.
   - If `/index` now distinguishes between live cwd and canonical qdrant collection root, add a short note in `codex-rs/docs/codex_mcp_interface.md`.
   - Keep the docs tied to observable runtime behavior, not implementation detail.

### Validation steps

1. Run the targeted slash-command test:
   - `cargo test -p codex-tui slash_index_shows_qdrant_defaults_output`
2. Review the updated snapshot output directly.
3. If docs changed, read the rendered section for consistency with the final `/index` wording.
4. Run `just fmt` in `codex-rs`.

## Recommended execution order

1. Fix Finding 1 first because it is a user-facing behavior regression, not only a diagnostics issue.
2. Land the Finding 1 tests before the implementation change so the regression stays pinned.
3. Fix Finding 2 next and keep it scoped to diagnostics/documentation alignment.
4. After both are complete, run the small targeted test slice again and then decide whether broader TUI coverage is worth running.

## Completion checklist

- [ ] Result opening uses payload/live session root semantics instead of stale app cwd state.
- [ ] Query-project result-opening regression test exists and passes.
- [ ] `/index` qdrant diagnostics clearly distinguish displayed default root from canonical collection root semantics.
- [ ] Qdrant `/index` test and snapshot are updated and passing.
- [ ] `just fmt` has been run in `codex-rs`.
- [ ] Docs are updated if the final UI wording changed materially.
