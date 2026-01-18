# Resources Plan

## Previous Response
- Resources are not advertised or handled; the server only declares `tools` and `prompts`. `web-fetch-mcp/src/index.ts:316`
- `LLMPacket` already has the full context payload you’d want to expose as a resource: `source_id`, metadata, content, warnings, hashes, plus optional `raw_excerpt` and `screenshot_base64`. `web-fetch-mcp/src/types.ts:176`
- `source_id` is derived from canonical URL + day-level timestamp + content hash, so IDs rotate daily (good for freshness, but not stable across days). `web-fetch-mcp/src/processing/normalizer.ts:432`
- In-memory caching exists only for raw HTTP fetch results and robots; there’s no packet store to back `resources/read`. `web-fetch-mcp/src/utils/cache.ts:1`, `web-fetch-mcp/src/fetcher/http-fetcher.ts:73`
- `format.output === "raw"` returns before normalization, so a resource cache would not get populated for raw-only requests unless you add explicit handling. `web-fetch-mcp/src/tools/fetch.ts:294`

Tailored Resource Design
- Scope/IDs: Use `source_id` as the resource key; it already aligns with chunk/compact and ties to normalized content. `web-fetch-mcp/src/processing/normalizer.ts:432`
- Resource store: Add a `ResourceStore` built on `SimpleCache` to hold recent `LLMPacket` plus optional `NormalizedContent`, raw bytes, and screenshot. Reuse `CACHE_TTL_S` for TTL and cap size (e.g., 100) to match the existing fetch cache. `web-fetch-mcp/src/utils/cache.ts:12`
- URIs & templates: Custom scheme `webfetch://`.
  - `webfetch://packet/{source_id}` → `application/json` `LLMPacket`
  - `webfetch://content/{source_id}` → `text/markdown` `packet.content`
  - `webfetch://normalized/{source_id}` → `application/json` `NormalizedContent`
  - `webfetch://screenshot/{source_id}` → `image/png` `blob` (from `screenshot_base64`)
  - Optional: `webfetch://raw/{source_id}` → raw body + `mimeType` if you decide to store raw bytes
- resources/list: List only `packet` resources (keeps the list small). Use `title` from `metadata.title` or fallback to `canonical_url`, `description` with host + `retrieved_at`, `annotations.lastModified` from `retrieved_at`, and `size` as `content.length`.
- resources/read: Parse the URI, lookup `source_id`, return `text`/`blob` with correct `mimeType`. Return JSON-RPC `-32002` when missing; validate scheme strictly to avoid accidental network fetches.
- Capabilities & notifications: Advertise `resources: { listChanged: true }` and emit `notifications/resources/list_changed` when a packet is stored or evicted. Skip `subscribe` initially.
- Raw-only requests: Default to no resource when `format.output === "raw"`; if you want raw resources, add an explicit opt-in and accept the memory tradeoff (up to `MAX_BYTES` per entry).

If you want me to implement this, I can:
1) Add the resource store + list/read/templates handlers.
2) Wire resource capture into `fetch`/`extract`.
3) Add vitest coverage for list/read/error cases.

## Plan
1) Add a ResourceStore and helpers
   - Create a new module (for example `web-fetch-mcp/src/resources/store.ts`) that wraps `SimpleCache` and stores entries keyed by `source_id`.
   - Define a `ResourceEntry` that includes: `packet` (required), `normalized` (optional), `raw` (optional), `screenshot` (optional), and `retrieved_at` for annotations.
   - Implement methods:
     - `set(entry)` to insert and return whether this is a new key.
     - `get(sourceId)` to fetch a non-expired entry.
     - `list()` to return entries in a stable order (for example, newest first by `retrieved_at`).
   - Use `CACHE_TTL_S` as TTL and set `maxSize` to 100 (mirrors fetch cache). Add a small helper to prune expired entries on list/read.
   - Add a URI parser/formatter helper (for example `web-fetch-mcp/src/resources/uri.ts`) to validate `webfetch://{type}/{source_id}` and reject anything else.

2) Add MCP resources capability + list/read/templates handlers
   - In `web-fetch-mcp/src/index.ts`, add the `resources` capability with `{ listChanged: true }`.
   - Add handlers for:
     - `resources/list` to expose `webfetch://packet/{source_id}` resources only, with `name`, `title`, `description`, `mimeType: "application/json"`, `size`, and `annotations.lastModified` from `retrieved_at`.
     - `resources/read` to resolve URIs to:
       - `packet` (JSON `text`)
       - `content` (markdown `text`)
       - `normalized` (JSON `text`)
       - `screenshot` (base64 `blob`, `mimeType: "image/png"`)
       - Optional: `raw` (base64 `blob`, `mimeType` from raw)
     - `resources/templates/list` to advertise the URI patterns for packet/content/normalized/screenshot (and optional raw).
   - For missing resources, throw `McpError` with `ErrorCode.ResourceNotFound` (or `-32002`) and include `{ uri }` in `data`.
   - Emit `notifications/resources/list_changed` whenever a new entry is added (and optionally on eviction).

3) Wire capture into fetch/extract
   - In `web-fetch-mcp/src/tools/fetch.ts`, after a successful normalization, store a resource entry with:
     - `packet`, optional `normalized`, optional `raw` (if you decide to keep raw bytes), optional `screenshot`.
     - Skip storing when `format.output === "raw"` unless you add explicit raw-only storage.
   - In `web-fetch-mcp/src/tools/extract.ts`, after successful normalization from `raw_bytes`, store a resource entry (packet + normalized when requested).
   - Ensure `source_id` is the key so chunk/compact outputs can be tied back to the resource.

4) Add vitest coverage for list/read/error cases
   - Add unit tests (for example `web-fetch-mcp/tests/unit/resources.test.ts`) that exercise:
     - `resources/list` returns the expected resource metadata after inserting a packet.
     - `resources/read` returns correct content/mimeType for packet/content/normalized/screenshot.
     - Unknown/malformed URIs return `-32002` with `uri` in error data.
   - If you separate handlers into testable functions, test those directly instead of spinning up an MCP server.
   - Run `npm test` to confirm coverage.

## Implementation Tasks
1) Add a resources module
   - Create `web-fetch-mcp/src/resources/store.ts` with:
     - `ResourceEntry` (packet, normalized?, raw?, screenshot?, retrieved_at).
     - `ResourceStore` wrapper over `SimpleCache` with `set/get/list`.
     - A singleton accessor (for example `getResourceStore(ttlMs)`).
   - Create `web-fetch-mcp/src/resources/uri.ts` with:
     - `parseResourceUri(uri)` → `{ kind, sourceId }` or error.
     - `buildResourceUri(kind, sourceId)` to keep format consistent.
   - Optional: `web-fetch-mcp/src/resources/types.ts` for shared enums and helpers.

2) Implement MCP resource handlers
   - Update `web-fetch-mcp/src/index.ts`:
     - Add `resources: { listChanged: true }` under `capabilities`.
     - Import resource helpers/store.
     - Register `resources/list`, `resources/read`, and `resources/templates/list` handlers.
   - Handler behaviors:
     - `resources/list`: iterate `ResourceStore.list()` and map to MCP `Resource` objects using `packet` metadata.
     - `resources/read`: parse URI, look up entry, return correct `text` or `blob`.
     - `resources/templates/list`: return URI templates for packet/content/normalized/screenshot (and optional raw).
     - On missing or malformed URIs, throw `McpError` with resource-not-found code and `{ uri }` in `data`.

3) Wire resource capture into tools
   - Update `web-fetch-mcp/src/tools/fetch.ts`:
     - After a successful normalize (and before return), write to `ResourceStore`.
     - Capture `packet`, `normalized` (when requested), and `screenshot`.
     - Skip storing when `format.output === "raw"` unless explicitly enabled.
   - Update `web-fetch-mcp/src/tools/extract.ts`:
     - After a successful normalize from `raw_bytes`, store the `packet` (and `normalized` if requested).

4) Emit list-changed notifications
   - In `resources/store.ts` (or `index.ts`), detect when `set` inserts a new `source_id`.
   - On new insert, emit `notifications/resources/list_changed` from the MCP server.
   - If eviction is implemented, emit list-changed on evict as well.

5) Add tests
   - Add `web-fetch-mcp/tests/unit/resources.test.ts` (or split tests by module):
     - Store/list ordering, TTL expiry, and list metadata mapping.
     - `resources/read` for packet/content/normalized/screenshot.
     - Resource-not-found error for unknown IDs and invalid URIs.
   - If handlers are extracted into helper functions, test those without spinning up a server.

6) Validation
   - Run `npm test` in `web-fetch-mcp`.
   - If behavior changes are user-facing, update `web-fetch-mcp/README.md` to add a “Resources” section and examples.

## Checklist
- [ ] Create `web-fetch-mcp/src/resources/store.ts` with `ResourceStore` + `ResourceEntry`.
- [ ] Create `web-fetch-mcp/src/resources/uri.ts` with parse/build helpers.
- [ ] Add `resources` capability and MCP handlers in `web-fetch-mcp/src/index.ts`.
- [ ] Wire resource capture into `web-fetch-mcp/src/tools/fetch.ts`.
- [ ] Wire resource capture into `web-fetch-mcp/src/tools/extract.ts`.
- [ ] Emit `notifications/resources/list_changed` on new resource insertion (and evictions if applicable).
- [ ] Add unit tests for resource list/read/errors.
- [ ] Run `npm test` in `web-fetch-mcp`.
- [ ] Update `web-fetch-mcp/README.md` with a Resources section (if needed).
