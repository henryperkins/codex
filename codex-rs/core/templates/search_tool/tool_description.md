# Apps (Connectors) tool discovery

Searches over apps/connectors tool metadata with BM25 and exposes matching tools for the next model call.

Tools of the apps ({{app_names}}) are hidden until you search for them with this tool (`tool_search`).

When the request needs one of these connectors and you don't already have the required tools from it, use this tool to load them. For the apps mentioned above, always prefer `tool_search` over `list_mcp_resources` or `list_mcp_resource_templates` for tool discovery.

Follow this workflow:

1. Call `tool_search` with:
   - `query` (required): focused terms that describe the app capability you need.
   - `limit` (optional): maximum number of tools to return (default `8`).
2. Use the returned `tools` namespaces to decide which Apps tools are relevant.
3. Matching tools are returned in the `tool_search_output` for the next model call. Re-run `tool_search` if you need a different app capability.

Notes:
- Core tools remain available without searching.
- If you are unsure, start with `limit` between 5 and 10 to see a broader set of app tools.
- `query` is matched against Apps tool metadata fields:
  - `name`
  - `tool_name`
  - `server_name`
  - `title`
  - `description`
  - `connector_name`
  - input schema property keys (`input_keys`)
- Use `tool_search` when the user asks to work with one of apps ({{app_names}}) and the exact tool name is not already known.
- If the user names an Apps tool directly (for example `mcp__codex_apps__calendar-create-event`) and it is not already available, do not run that name as a shell command. First call `tool_search` with that tool name, then invoke the matching `mcp__codex_apps__...` tool.
- If the needed app is already explicit in the prompt (for example `[$app-name](app://{connector_id})`) or already present in the current `tools` list, you can call that tool directly.
- Do not use `tool_search` for non-apps/local tasks (filesystem, repo search, or shell-only workflows) or anything not related to {{app_names}}.
