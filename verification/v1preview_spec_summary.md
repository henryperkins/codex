# Azure OpenAI `v1preview.json` – Responses API Highlights

This note summarises the key information in `docs/v1preview.json` that is specifically relevant to the **Responses API** (the part already consumed by Codex).

---

## 1. Endpoints

| HTTP | Path | Purpose |
|------|------|---------|
| **POST** | `/responses` | Create a *new* response. Supports both streaming & non-streaming, synchronous & background execution. |
| **GET** | `/responses/{response_id}` | Fetch the *final* response object (polling). |
| **DELETE** | `/responses/{response_id}` | Delete a stored response. |
| **GET** | `/responses/{response_id}/input_items` | List the user-supplied input items for a response. |

**Observation:** No explicit `/cancel` endpoint for responses exists in the `v1preview` spec – background jobs appear to be cancellable via `DELETE /responses/{response_id}`.

---

## 2. Create-Response Request Schema (`AzureCreateResponse`)

Important fields (Codex currently uses only a subset):

| Field | Type | Description | Present in Codex? |
|-------|------|-------------|-------------------|
| `model` | string | Target deployment name | ✅ (`Config::model`) |
| `instructions` | string | System / developer messages | ✅ via `Prompt::get_full_instructions` |
| `input` | object | List of `input_items` | ✅ (`Prompt::get_formatted_input`) |
| `tools` | array | Tool definitions | ✅ (converted by `create_tools_json_for_responses_api`) |
| `tool_choice` | object/enum | Tool execution control | ✅ (`"auto"`) |
| `parallel_tool_calls` | boolean | Allow parallel tool invocations | ✅ (hard-coded `false`) |
| **`stream`** | boolean | Return SSE stream instead of full JSON | **Always `true` in Codex** |
| **`background`** | boolean | Run in background, poll later | **Not exposed** |
| **`previous_response_id`** | string | Enable stateless conversation chaining | **Not exposed** (tests assert absence) |
| **`store`** | boolean | Persist response object | ✅ but force-set to `true` for Azure |
| `include` | array | Extra data to include (e.g. `reasoning.encrypted_content`) | ✅ when reasoning is requested |
| `reasoning` | object | Effort/summary controls | ✅ optional |
| `text` | object | Verbosity options (gpt-5 family) | ✅ optional |

Other optional knobs (temperature, top_p, truncation, metadata, etc.) are **not surfaced** in the client yet.

---

## 3. Response Object Enrichments

The spec documents additional HTTP response headers that Azure returns:

* `azure-openai-usage` – token counts (JSON string)
* `x-ms-request-id` – unique request identifier
* `x-ms-model-id` – resolved deployment name

Codex currently parses **only** `azure-openai-usage` and stores it under `extra["azure_openai_usage_header"]`. The two other headers are ignored; the plan proposes capturing them.

---

## 4. Input-Items List (`GET /responses/{id}/input_items`)

This endpoint supports standard list pagination parameters:

* `limit` (integer)
* `after` / `before` (cursor strings)
* `order` (`asc` / `desc`)

Codex presently calls this endpoint but **does not** expose those query parameters to callers.

---

## 5. Behavioural Notes From Spec

1. `background: true` **requires** `store: true` – therefore background tasks are always persisted.
2. If `stream: true` *and* `background: true` are both set, the spec warns of potential performance degradation – matching the plan’s risk notes.
3. When `previous_response_id` is supplied *and* new `instructions` are provided, the new instructions replace those from the previous response; otherwise, the old instructions are reused.

---

## 6. Implications for Codex Implementation

* **Non-streaming path** → Implement `create_response_sync` (`stream: false`).
* **Background jobs** → Implement creation + polling; deletion doubles as cancellation.
* **Conversation chaining** → Add `previous_response_id` plumbing to `Prompt` & request payload.
* **Pagination & filters** → Extend Azure helpers to accept `limit`, `after`, `before`, `order`, and `include_obfuscation` where applicable.
* **Header capture** → Parse & expose `x-ms-request-id`, `x-ms-model-id` in `ModelClient::azure_get_response`.

---

### Quick Reference – Field Support Matrix (Current vs Spec)

| Field | Spec | Codex Today | Plan | Gap? |
|-------|------|------------|------|------|
| `stream` | yes | always `true` | switchable | ✅ gap |
| `background` | yes | n/a | supported | ✅ gap |
| `previous_response_id` | yes | n/a | supported | ✅ gap |
| `include` | yes | partial | no change | — |
| `store` | yes | forced `true` | conditional | ⚠ improvement |
| Header capture | yes | usage only | full | ✅ gap |

---

## 7. Next Steps

1. Prototype `create_response_sync` and optionally re-use existing retry logic.
2. Introduce a `ResponseOptions` struct to cleanly expose `stream`, `background`, and later `previous_response_id`.
3. Expand `azure.rs` with pagination-aware helper wrappers.
4. Update tests and snapshots accordingly.

