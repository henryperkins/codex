# Azure OpenAI Integration Gaps — Implementation Plan

## Context

- Canonical doc: `codex-rs/docs/azure-responses.md` (treat as source of truth).
- Today: Codex supports Azure via built‑in providers (`azure-responses`, `azure-chat`) using the generic Responses/Chat clients.
- A dedicated Azure module (`core/src/azure/`) exists (types/auth/client/deployments/error) but is not currently compiled or wired into Codex’s request path.

## Objectives

- Solidify Azure behavior with the existing `azure-responses` provider (no breaking CLI changes).
- Close the most visible gaps (auth correctness, error clarity, optional chaining).
- Add optional Azure‑only features in a controlled, opt‑in manner.

## Non‑Goals (For Now)

- Fully switching Codex to the custom `AzureClient`.
- Introducing `azure-identity` dependency unless explicitly approved.
- Redesigning CLI UX for background tasks or file uploads.

## Approach

- Continue to use the generic Responses client for `azure-responses`.
- Add Azure‑specific behaviors guarded by provider id (`model_provider_id` starts with `azure-`).
- Keep all changes scoped to `codex-core`.

---

## P0 — Ship Immediately (Minimal, High‑Impact)

- **Auth Header (Azure API Key)**
  - Ensure Azure API key is sent via `api-key` header (not `Authorization: Bearer`) for `azure-responses` / `azure-chat`.
  - Implementation: in `core/src/model_provider_info.rs`, set:
    - `env_key = None`
    - `env_http_headers = { "api-key" = "AZURE_OPENAI_API_KEY" }`
  - Rationale: Azure API key must be provided in `api-key`; Bearer is used for Azure AD.

- **SSE Status Precheck**
  - Before parsing SSE, explicitly check HTTP status. For non‑2xx, read the body and surface a descriptive error.
  - Location: `core/src/client.rs` right after `.send()` and before creating the event stream.

- **Tests**
  - Wiremock tests verifying:
    - `azure-responses` hits `/openai/v1/responses?api-version=...` and includes `api-key`.
    - `azure-chat` hits `/openai/chat/completions?api-version=...`.
    - Non‑2xx responses (401/429/400) are surfaced as clear errors instead of opaque SSE failures.

---

## P1 — Doc Parity Enhancements (Medium Scope)

- **Previous Response Chaining**
  - Track last `response_id` when `ResponseEvent::Completed` fires.
  - Include `previous_response_id` on the next request if `store == true` (omit when `disable_response_storage == true` / ZDR).
  - Changes:
    - `core/src/client_common.rs`: add optional `previous_response_id` field to `ResponsesApiRequest`.
    - `core/src/client.rs`: persist the last completed id per session and include conditionally.
  - Tests:
    - First turn omits `previous_response_id`.
    - Second turn (stored) includes it and server validates it.

- **Azure Error Decoding (Clearer Messages)**
  - For providers starting with `azure-`, parse common Azure error shapes and produce friendly messages:
    - Content filter violation
    - Invalid `api-version`
    - Deployment not found
    - Rate limit/quota with `retry-after` hints
  - Location: error handling path in `core/src/client.rs` after non‑2xx responses.
  - Tests: Wiremock fixtures to assert message clarity.

- **Deployment Resolution (Optional, Non‑Breaking)**
  - Support env‑driven mapping for Azure (e.g., `AZURE_DEPLOY_GPT_5=prod-gpt5`).
  - If present and provider is `azure-responses`, map `model` → deployment name; otherwise use `model` verbatim.
  - Tests: Mapping honored when env var is set.

---

## P2 — Optional Azure‑Only Features (Opt‑In, Low Blast Radius)

- **Background Tasks** — Implemented
  - “background: true” request mode supported for Azure providers.
  - Polling mode added with periodic status events and final completion; opt‑in via env
    - `CODEX_ENABLE_BACKGROUND=1`
    - `CODEX_BACKGROUND_MODE=poll` (optional `CODEX_BACKGROUND_POLL_INTERVAL_MS`)
  - Streaming background continues to work when `stream=true`.

- **Tool Output Types (Parse, Don’t Break)**
  - Extend output parsing to recognize `image_generation_call`, `code_interpreter_call`, `mcp_approval_request`.
  - Render minimal, readable summaries instead of falling back to `Other`.
  - Location: `core/src/protocol/models.rs` parsing + `core/src/client.rs` event routing.
  - Tests: JSON fixtures for these types.

- **Azure AD / CLI Auth (Lightweight)** — Implemented
  - `AZURE_OPENAI_AUTH_TOKEN` is used as Bearer for Azure providers when present.
  - `AZURE_USE_CLI=1` shells out to `az account get-access-token` to acquire a token when no explicit token is set.
  - No `azure-identity` dependency added.

- **File Uploads (Internal Helper)**
  - Add a thin helper to POST `/v1/files` with `purpose`, honoring Azure auth mode.
  - Expose only internally until a public UX is agreed.

---

## Code Touch Points

- `core/src/model_provider_info.rs`
  - Adjust Azure built‑ins to use `env_http_headers = { "api-key" = "AZURE_OPENAI_API_KEY" }`.
  - Keep `api-version` query param for Azure providers.

- `core/src/client_common.rs`
  - Add `previous_response_id: Option<String>` to `ResponsesApiRequest`.

- `core/src/client.rs`
  - Non‑2xx SSE precheck and error surfaces.
  - Include `previous_response_id` when appropriate (stored sessions).
  - Azure error decoding path (provider id starts with `azure-`).
  - Optional: background mode request flag and basic poller.

- `core/src/protocol/models.rs`
  - Recognize and minimally render `image_generation_call`, `code_interpreter_call`, `mcp_approval_request`.

- Tests (`core/tests/suite/` + new azure cases)
  - Header routing (`api-key`) and endpoints for `azure-responses` / `azure-chat`.
  - Non‑2xx errors readability.
  - Chaining behavior.
  - Optional: deployment mapping, new output types.

---

## Validation & Tooling

- Formatting: `just fmt` (in `codex-rs`).
- Lints: `just fix -p codex-core`.
- Tests:
  - `cargo test -p codex-core`
  - Workspace: `cargo test --all-features` (ensure .env is not injected into tests if present in repo)

---

## Rollout

1) PR 1 (P0): Auth header + SSE status precheck + tests (safe, high‑impact).
2) PR 2 (P1): previous_response_id + Azure error decoding + optional deployment mapping.
3) PR 3 (P2): background tasks, extended tool outputs, optional AAD/CLI, file uploads (behind flags).

---

## Risks & Mitigations

- **Auth header change regressions**
  - Scoped to Azure providers only; tests enforce correct header and endpoint.

- **Chaining logic regressions**
  - Gated on stored sessions; ZDR path explicitly omits the field; tests cover first/next turn behavior.

- **Error decoding brittleness**
  - Only activates for `azure-` providers; falls back to generic error on parse failure.

---

## Open Questions

- Do we want to wire the custom `core/src/azure/` client now or keep the generic client approach and revisit later?
- Deployment mapping source: env‑only or also allow a small inline TOML mapping under `azure-responses`?
- Preferred direction for Azure AD / CLI support: lightweight env/CLI options now vs. full `azure-identity` later?

---

## Acceptance Criteria

- Azure requests (built‑in providers) authenticate with `api-key`, succeed against Azure endpoints, and fail with clear messages on common Azure errors.
- Optional chaining works for stored sessions without breaking ZDR.
- Tests pass on `codex-core` with added Azure coverage.

---

## Appendix: What Stays The Same For Users

- Use built‑in providers:
  - `model_provider = "azure-responses"` for Responses API
  - `model_provider = "azure-chat"` for Chat Completions
- Set:
  - `AZURE_OPENAI_API_KEY`
  - `AZURE_OPENAI_ENDPOINT` (e.g., `https://myresource.openai.azure.com`)
- `model` must be your deployment name unless optional mapping is configured.
