user
Let me get clear I want to make it clear that this tool will be used as a way for the
model two receive contacts in the form of data extracted from a webpage URL and
possibly then formatted or converted into presentable markdown. The data extracted
needs to be tokenized and sanitized, as well as probably converted into Jason or
whatever the API will expect this tool should also work for more than one provider and
should also accept any urlending in.com or beginning with HTTP or HTtps

codex
Understood — the scraper isn’t just a convenience CLI command; it is a tool meant to
supply clean, size-bounded context for any LLM provider Codex can talk to (OpenAI,
Azure OpenAI, Anthropic, etc.).
Below is an updated design that satisfies these stricter requirements.

──────────────────────────────────────────────

    1. Overall data contract
       ──────────────────────────────────────────────
       The scraper returns a single JSON object with this shape:

    {
      url:           string,          // final URL after redirects
      title:         string | null,
      markdown:      string,          // sanitised, readable, ready to embed
      text:          string,          // same as markdown but without MD markup
      html_excerpt?: string,          // optional – first X chars original HTML
      token_count:   number           // #tokens in `markdown` for current model
    }

• Client code (AgentLoop) embeds that JSON directly in the input items it
sends to the LLM.
• In CLI “stand-alone” mode (codex scrape …) the tool pretty-prints the same
JSON to stdout or writes it to the requested file.

────────────────────────────────────────────── 2. Sanitisation & readability pipeline
──────────────────────────────────────────────
a. Fetch
• Follow up to 5 redirects, 10 MB max, 10 s timeout.
• Reject URLs that do not match ^https?:// or a top-level domain part
shorter than two characters. (You asked for “ending in .com” but we
shouldn’t arbitrarily block .org, .io, …)

b. Strip dangerous markup
• Use sanitize-html to remove <script>, <style>, inline JS, tracking
attributes (onmouseover, …).

c. Extract human-readable text
• @mozilla/readability on the DOM → best-guess “article” content
(fallback: body.innerText).
• Collapse whitespace, normalise newlines.

d. Convert to Markdown
• turndown → produce reasonably formatted Markdown (links preserved).
• Truncate to MAX_CHARS (~30 k) before tokenisation to avoid runaway
memory.

e. Tokenise
• Use @dqbd/tiktoken (already used elsewhere in Codex) with the current
session’s model to count tokens.
• If token_count > LIMIT (e.g. 8 k) hard-trim tail paragraphs until
under limit, add “(truncated)” notice at end.

────────────────────────────────────────────── 3. Multi-provider compatibility
──────────────────────────────────────────────
• The returned JSON is provider-agnostic.
• AgentLoop already branches on provider when sending requests; we only need
to ensure that whatever object we attach respects the individual SDK’s
expected input schema:

– OpenAI: { role: "user", content: [{ type: "text", text: "…" }] }
– Anthropic: {"type":"text","text":"…"} inside messages.

We therefore augment AgentLoop.buildInputItems():

    if (item.tool === "scrape") {
      const md = item.data.markdown;
      if (providerIsAnthropic) { …wrap md… } else { …openai style… }
    }

No change is needed elsewhere.

────────────────────────────────────────────── 4. CLI flags (superset of earlier plan)
──────────────────────────────────────────────
codex scrape <url>
-o, --output <file> Write JSON to file instead of stdout
--html-excerpt Include first 2 kB raw HTML in JSON
--truncate <tokens> Custom max-token limit (default: 8000)
--no-readability Skip readability; dump full page
--agent-tool-call Do not print JSON – emit a tool_call
wrapper so the output can be piped into the
next codex invocation (advanced use)

────────────────────────────────────────────── 5. Implementation modules / files
──────────────────────────────────────────────
• package.json – add dependencies: sanitize-html, @mozilla/readability,
jsdom, turndown, (tiktoken is already present).

• src/utils/web-scraper.ts ← fetch + sanitise + markdown + token count
• src/utils/tokenizer.ts ← thin wrapper around tiktoken (already exists)
• src/agent/tools/scrape.ts ← registers FunctionTool, calls scraper util
• src/cli.tsx ← recognise “scrape” sub-command

────────────────────────────────────────────── 6. Provider integration test matrix
──────────────────────────────────────────────

┌──────────────┬─────────────┬───────────────────────┬────────────────┐
│ Provider │ Max context │ Tokeniser name │ Status │
├──────────────┼─────────────┼───────────────────────┼────────────────┤
│ openai o4 │ 128 k │ o200k_base │ supported │
├──────────────┼─────────────┼───────────────────────┼────────────────┤
│ openai 3.5 │ 16 k │ cl100k_base │ supported │
├──────────────┼─────────────┼───────────────────────┼────────────────┤
│ Azure openai │ mirror │ (same as model above) │ supported │
├──────────────┼─────────────┼───────────────────────┼────────────────┤
│ anthropic v3 │ 200 k │ anthropic_tokens_2023 │ supported soon │
└──────────────┴─────────────┴───────────────────────┴────────────────┘

Tokenizer wrapper chooses the correct encoding based on model argument already
present in the CLI flags.

────────────────────────────────────────────── 7. Security considerations
──────────────────────────────────────────────
• Network access obeys CODEX_SANDBOX_NETWORK_DISABLED=1 → throws
“network-disabled” error.
• Maximum download size and timeout (#2a).
• HTML sanitiser removes JS, inline event handlers, javascript: URLs.

────────────────────────────────────────────── 8. Work plan (incremental)
──────────────────────────────────────────────

    1. Add dependencies & implement `web-scraper.ts` in isolation.
    2. Extend CLI command.
    3. Wire the function-tool so the model can invoke `scrape` autonomously.
    4. Update README / docs / CHANGELOG.
    5. Later: add Markdown → “semantic chunk” splitter for vector-store powering
       RAG workflows (outside current scope).

This design gives Codex a robust, provider-agnostic way to ingest web page
content, ensuring it is safe, size-bounded, and ready to drop directly into
an LLM request.

user
analyze deeper still lookig for edge cases

codex
Below is a “stress-test” style audit of the yet-to-be-written scraper.
For each stage (fetch → parse → markdown → tokenise → hand-off) we list edge-cases you
must guard against and the concrete mitigations that fit Codex’s existing architecture.

──────────────────────────────────

    1. URL & network layer
       ──────────────────────────────────
       Edge-cases
       1.1  Scheme missing / unsupported (`example.com`, `ftp://…`, `file://…`).
       1.2  Redirect loops or > 5 hops.
       1.3  TLS problems: self-signed, expired cert.
       1.4  Non-HTML MIME types (PDF, image/*, octet-stream).
       1.5  Very large responses or decompressed size > 10 MB.
       1.6  Gzip/Brotli bombs (small download inflates huge).
       1.7  Slow‐loris / stall – server sends bytes too slowly.
       1.8  Network disabled inside CI or tests (`CODEX_SANDBOX_NETWORK_DISABLED=1`).
       1.9  Proxy config (`HTTPS_PROXY`) already used by AgentLoop.
       1.10 IPV6-only URLs.
       1.11 Internationalised domain names (IDN).

Mitigations
• Require ^https?:// via regex; prepend https:// if user typed bare domain.
• Use fetch() with { redirect: "follow", follow: 5 } and abort on 30× loop.
• AbortController: 10 s total timeout and 3 s per 64 kB chunk read.
• Check content-type and first 512 B sniff for <html before parsing.
• Track decompressed bytes; hard-abort when exceeding 10 MiB.
• Honour HTTPS_PROXY automatically via WHATWG fetch agent.
• If CODEX_SANDBOX_NETWORK_DISABLED ⇒ throw explicit ERR_NETWORK_DISABLED.
• IDN: call new URL(); Node handles punycode.
• Return rich error JSON { error: { code, message } } with non-zero exit code.

────────────────────────────────── 2. HTML sanitisation / DOM creation
──────────────────────────────────
Edge-cases
2.1 Malformed HTML fragments (no <html>).
2.2 Activating JS in JSDOM (<script> runs by default!).
2.3 Heavy DOM (> 100 000 nodes) ⇒ memory blow-up.
2.4 Embedded <svg><script> or inline event handlers.
2.5 Data-URI images > 1 MB inside src.
2.6 Non-UTF-8 encodings (meta charset=iso-8859-1).
2.7 Encoded entities &, &#x202A; mis-decoded.

Mitigations
• Feed jsdom with { runScripts: "outside-only", resources: "usable" } so no JS
executes.
• Use iconv-lite to transcode on content-type charset if not UTF-8 (<=1 MB).
• Stream DOM creation? JSDOM requires full buffer; rely on 10 MB cap.
• sanitize-html (allowTags = basic inline/structural) strip scripts, event attrs,
data-URIs bigger than 50 kB.
• Always call dom.window.close() to release memory.

────────────────────────────────── 3. Readability & selection
──────────────────────────────────
Edge-cases
3.1 Pages without clear article structure (Readability returns null).
3.2 CSS selector yields 0 nodes or 10 000 nodes.
3.3 Duplicate nodes across multiple selectors.
3.4 Mixing --attr with block-level selector (returns long text).
3.5 Selector/attr string injection (nothing extreme, but still validate).

Mitigations
• Fallback order: user selector(s) → Readability article → body.innerText.
• De-duplicate text by hash.
• Hard-limit combined plain-text length before markdown 100 k chars.
• Validate selector with css-what parse; reject if parse error.
• Validate attr matches /^[a-z][a-z0-9-_:]{0,30}$/i.

────────────────────────────────── 4. Markdown conversion
──────────────────────────────────
Edge-cases
4.1 Extremely long lines (tables, minified JSON).
4.2 Nested lists explode bullet depth (>10).
4.3 Existing triple back-ticks break chat formatting.
4.4 Image alt text contains ![ injection.
4.5 Links with markdown syntax inside title.
4.6 Non-Latin scripts turning to ? after turndown.

Mitigations
• Pre-wrap long lines at 120 chars before turndown.
• After turndown, replace consecutive back-ticks >= 3 with two back-ticks + U+200B ZERO
WIDTH SPACE.
• Use turndown options { headingStyle: "atx", codeBlockStyle: "fenced" }.
• Fallback: if turndown throws, return plaintext instead of failing.

────────────────────────────────── 5. Tokenisation & size control
──────────────────────────────────
Edge-cases
5.1 Unknown model → no tiktoken mapping.
5.2 Provider whose encoding differs subtly (Anthropic).
5.3 Content still > provider context limit after truncation.
5.4 Counting tokens on very large string O(n) CPU.

Mitigations
• Map model->encoding with same helper table that model-utils.ts uses; if
unknown, fallback to cl100k_base estimate (Math.ceil(chars/4)).
• Configurable maxTokens CLI flag; default 8 k, but also MIN(providerLimit, 0.8×
providerLimit).
• Truncate in paragraph chunks until token count ≤ limit.
• Cache tiktoken encoder per encoding to reuse.

────────────────────────────────── 6. JSON output & tool-schema
──────────────────────────────────
Edge-cases
6.1 Non-serialisable value inside scraper result.
6.2 Output JSON > few MB – model cost blow-up.
6.3 Unicode inside JSON not preserved (surrogate pairs).
6.4 Markdown containing ```json delimiter confusion when user pastes.

Mitigations
• JSON.stringify with replacer to drop functions / window objects.
• Enforce total serialized length <2 MB (about 0.5 M tokens) and report if cut.
• Use JSON.stringify(obj, null, 2) for CLI but send raw string (not code-block-wrapped)
as tool-return.
• Document in README: “When embedding markdown into a chat prompt, wrap with triple
back-ticks yourself.”

Tool JSON schema (for AgentLoop):

    {
      name: "scrape",
      description: "Fetches a web page and returns cleaned markdown and metadata.",
      parameters: {
        type: "object",
        properties: {
          url: { type: "string" },
          selector: { type: ["string","array"], items: { type: "string" }, nullable:

true },
attr: { type: "string", nullable: true },
truncate_tokens: { type: "number", nullable: true }
},
required: ["url"],
additionalProperties: false
}
}

────────────────────────────────── 7. Concurrency, cancellation, memory
──────────────────────────────────
Edge-cases
7.1 User presses ESC during scraper run (AgentLoop cancel).
7.2 Multiple scrape tool calls in parallel.
7.3 Memory pressure when 2-3 huge pages are processed at once (>200 MB RAM).

Mitigations
• Every scrape run receives an AbortSignal; pass to fetch and break parsing early.
• Keep a global SCRAPE_SEMAPHORE with max = 2 concurrent; subsequent calls return 429
error JSON.
• Ensure import('node:os').freemem() check; if <100 MB, refuse new scrape.

────────────────────────────────── 8. File output & CLI
──────────────────────────────────
Edge-cases
8.1 User passes --output ../../etc/passwd.
8.2 File exists → overwrite without prompt.
8.3 Invalid UTF-8 when writing.

Mitigations
• Resolve path via path.resolve and refuse when outside CWD if not explicitly approved
with --force-outside.
• Fail if file exists unless --force.
• Explicit fs.writeFileSync(out, data, "utf8").

────────────────────────────────── 9. Legal & ethical notes
──────────────────────────────────
• No robots.txt honouring yet – document that user is responsible.
• Include a User-Agent: codex-cli-web-scraper/⟨version⟩
(+https://github.com/openai/codex-cli) to be transparent.

────────────────────────────────── 10. Library-specific quirks
──────────────────────────────────
• @mozilla/readability expects document.location.href; must set it on JSDOM.
• sanitize-html strips data- attributes by default; keep them if selector might need
them? Acceptable trade-off: drop.
• turndown can hang on tables with thousands of rows; wrap call in
Promise.race([turndownPromise, timeout]).

──────────────────────────────────
Wrap-up
──────────────────────────────────
Addressing these edge-cases early will keep the scraper:

    1. Safe (no code execution, no huge memory, no path escape).
    2. Predictable in token budget for **any** LLM provider Codex supports.
    3. User-friendly with meaningful, machine-parsable error messages.

Once the mitigations above are folded into implementation & unit tests, the scraper
