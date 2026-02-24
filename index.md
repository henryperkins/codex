---
epoch: 1771887527548
modelKey: "claude-opus-4.6|github-copilot"
topic: "Governed AI Agents & Codex"



tags:
  - copilot-conversation
---

**user**: {activeNote} [[Building Governed AI Agents - A Practical Guide to Agentic Scaffolding]] [[Unlocking the Codex harness how we built the App Server]]
[Context: Notes: Clippings/Building Governed AI Agents - A Practical Guide to Agentic Scaffolding.md, Clippings/Unlocking the Codex harness how we built the App Server.md, Clippings/Unrolling the Codex agent loop.md]
[Timestamp: 2026/02/23 16:58:47]

**ai**: # Summary: Three Perspectives on AI Agent Architecture

These three notes collectively paint a comprehensive picture of how modern AI agents are designed, governed, and deployed — from the inner loop of a coding agent to enterprise-wide policy enforcement.

---

## [[Unrolling the Codex agent loop]]

This post (the active note) is a **deep technical dive into the core agent loop** powering OpenAI's Codex CLI. Key ideas:

- **The agent loop** is the iterative cycle of: user input → model inference → tool calls → observation → re-inference → assistant message. A single "turn" can involve many inference/tool iterations before the model returns a final response.
- **Prompt construction** is carefully layered: system message (from the server), tools, developer instructions, `AGENTS.md` content, environment context, and finally the user message — all composed into the `input` field sent to the Responses API.
- **Prompt caching** is critical for performance. Because each new inference call re-sends the entire conversation as a prefix, exact prefix matching enables cache hits, making sampling **linear rather than quadratic**. The team goes to great lengths to avoid breaking the prefix (e.g., stable tool ordering, appending new messages instead of modifying earlier ones).
- **Context window management** is handled via **compaction** — when the conversation grows too long, the `/responses/compact` endpoint replaces the conversation history with a compressed representation including an opaque `encrypted_content` item.
- **Zero Data Retention (ZDR)** is supported by keeping requests fully stateless (no `previous_response_id`), with encrypted reasoning content that can be decrypted server-side.

---

## [[Unlocking the Codex harness how we built the App Server]]

This companion post explains **how the Codex harness is exposed to different client surfaces** via the App Server:

- **The App Server** is a long-lived process that hosts Codex core threads and communicates with clients over a **bidirectional JSON-RPC (JSONL over stdio)** protocol. It translates between client requests and Codex core's internal event stream.
- **Three conversation primitives** define the protocol:
  - **Item** — atomic unit of I/O (message, tool execution, diff, approval) with a lifecycle: `started → delta → completed`
  - **Turn** — one unit of agent work from user input to agent completion
  - **Thread** — durable container for an ongoing session, supporting create/resume/fork/archive
- **Client integration patterns**: local apps (VS Code, Desktop) bundle the binary and talk stdio; Codex Web runs the harness in a container with SSE streaming to the browser; the TUI is being refactored to also use the App Server protocol.
- **Protocol choice guidance**: App Server for full harness access; MCP server for tool-level integration; CLI exec mode for CI/automation; and a TypeScript SDK for programmatic control.

---

## [[Building Governed AI Agents - A Practical Guide to Agentic Scaffolding]]

This OpenAI cookbook shifts focus from **how agents work internally** to **how to govern them in production**. It builds a PE firm AI assistant to demonstrate:

- **Multi-agent handoffs**: A triage agent routes to specialist agents (Deal Screening, Portfolio, Investor Relations) based on `handoff_description` — keeping each agent focused and maintainable.
- **Layered guardrails**:
  - **Agent-level** (Agents SDK `InputGuardrail`): domain-specific validation (e.g., "is this a PE question?")
  - **Client-level** (`GuardrailsOpenAI` / `GuardrailAgent`): org-wide policy enforcement — PII detection, moderation, jailbreak detection, off-topic blocking — applied automatically to every API call
- **Policy-as-code**: Governance config is packaged as a pip-installable Python package (`pe-policies`) with JSON config, versioned in Git, so any team can `pip install` instant compliance.
- **Tracing & ZDR**: `trace()` context manager for observability, with custom trace processors for ZDR-compliant organizations that can't use OpenAI's dashboard.
- **Eval-driven tuning**: The `GuardrailEval` framework measures precision/recall/F1 against labeled test data, and a `GuardrailFeedbackLoop` **automatically tunes** confidence thresholds.
- **Red teaming with Promptfoo**: Auto-generates adversarial inputs (jailbreaks, PII extraction, prompt injection with encoding evasion) to stress-test guardrails beyond static test sets.

---

## How They Connect

| Concern | Codex Agent Loop | Codex App Server | Governed AI Agents |
| --- | --- | --- | --- |
| **Focus** | Core inference & tool loop | Protocol & client integration | Governance & compliance |
| **Key Pattern** | Prompt prefix → cache hit → efficiency | Item/Turn/Thread → rich streaming UI | Policy config → automatic enforcement |
| **ZDR** | Stateless requests, encrypted reasoning | Thread persistence within container | Disable tracing or custom processors |
| **Multi-agent** | Single agent loop | Thread-per-agent in App Server | Triage → specialist handoffs |
| **Evaluation** | Prompt caching metrics | Protocol backward-compat testing | Precision/recall evals + red teaming |

Together, these notes show that building production AI agents requires thinking at three levels: the **inference loop** (how the model reasons), the **integration layer** (how clients interact), and the **governance layer** (how the organization stays safe and compliant).
[Timestamp: 2026/02/23 16:59:26]

**user**: {activeNote}
[Context: Notes: Untitled 12.md]
[Timestamp: 2026/02/23 17:01:07]

**ai**: # Codex SDK — Overview & Notes

This note documents the **TypeScript SDK** for OpenAI's Codex agent (`@openai/codex-sdk`). It provides a programmatic way to embed the Codex agent into your own applications and workflows, wrapping the `codex` CLI binary and communicating over **JSONL via stdin/stdout** — the same App Server protocol described in [[Unlocking the Codex harness how we built the App Server]].

---

## Key Concepts

| Concept | Description |
| --- | --- |
| **`Codex`** | The top-level client. Spawns the CLI process and manages its environment/config. |
| **`Thread`** | A persistent conversation session. Maps to a Codex thread (persisted in `~/.codex/sessions`). |
| **`Turn`** | One round of user input → agent work → final response. Contains `finalResponse` and `items`. |
| **`Item`** | An atomic unit of agent output (message, tool call, diff, etc.), matching the App Server's item primitive. |

---

## API Surface at a Glance

### Initialization

```typescript
const codex = new Codex({
  env: { /* custom environment */ },
  config: { /* CLI config overrides as TOML-like JSON */ },
});
```

### Conversation Flow

```typescript
// Start a new thread
const thread = codex.startThread({ workingDirectory: "/path/to/repo" });

// Buffered turn (waits for completion)
const turn = await thread.run("Diagnose the test failure");

// Streaming turn (async generator of events)
const { events } = await thread.runStreamed("Fix the issue");
for await (const event of events) { /* handle events */ }

// Continue the conversation on the same thread
await thread.run("Now add tests for the fix");

// Resume a thread from a previous session
const resumed = codex.resumeThread(savedThreadId);
```

---

## Notable Design Decisions

### 1. CLI-as-a-Service Architecture
The SDK doesn't reimplement the agent loop — it **spawns the `codex` CLI binary** and talks to it over the App Server's JSONL/stdio protocol. This means:
- You always get the same agent behavior as the CLI/VS Code/Desktop
- Updates to Codex core are picked up by updating the `@openai/codex` package
- The SDK is a thin orchestration layer, not a reimplementation

### 2. Git Requirement
Codex requires the working directory to be a **Git repository** by default. This is a safety mechanism — it ensures all agent-made file changes are recoverable via `git diff` / `git checkout`. The `skipGitRepoCheck: true` escape hatch exists but should be used with caution.

### 3. Structured Output
The `outputSchema` option enables **constrained JSON responses** — the agent's final message will conform to the provided JSON Schema. This is powerful for building pipelines where downstream code needs to parse agent output reliably. Zod integration via `zod-to-json-schema` with `target: "openAi"` is explicitly supported.

### 4. Environment Isolation
The `env` option gives full control over the CLI's environment variables, which is critical for:
- **Sandboxed hosts** (Electron, containers)
- **Multi-tenant setups** where different API keys or base URLs are needed
- The SDK still injects its own required variables (`OPENAI_BASE_URL`, `CODEX_API_KEY`) on top

### 5. Config Overrides
The `config` option maps to `--config key=value` CLI flags (TOML format). This lets you programmatically control things like:
- `show_raw_agent_reasoning: true` — surface the model's reasoning
- `sandbox_workspace_write.network_access: true` — loosen sandbox restrictions

---

## Relationship to Other Notes

- **[[Unrolling the Codex agent loop]]** — Explains the inference/tool loop that runs *inside* the CLI process this SDK spawns. The `Turn` object returned by `run()` corresponds to one full cycle of that loop.
- **[[Unlocking the Codex harness how we built the App Server]]** — The JSONL/stdio protocol the SDK uses is the App Server protocol. The streaming events from `runStreamed()` (`item.completed`, `turn.completed`) map directly to the App Server's item lifecycle primitives.
- **[[Building Governed AI Agents - A Practical Guide to Agentic Scaffolding]]** — For governance, you could wrap Codex SDK calls with guardrail layers (input validation, output filtering) similar to the `GuardrailsOpenAI` client pattern shown in that cookbook.
[Timestamp: 2026/02/23 17:01:35]

**user**: How to implement a native tool that searches an index of the current repo
[Timestamp: 2026/02/23 17:02:32]

**ai**: # Implementing a Native Repo Search Tool in Codex

Based on the architecture described in [[Unrolling the Codex agent loop]], Codex's tools are defined in the `tools` field of the Responses API request and executed by the harness when the model emits a `function_call`. There are several ways to give Codex a repo search capability, depending on how deep you want to integrate.

---

## Option 1: MCP Server (Recommended)

The most straightforward approach is to build an **MCP (Model Context Protocol) server** that exposes a search tool. Codex natively supports MCP servers configured in `~/.codex/config.toml`:

### 1. Build the MCP server

```typescript
// repo-search-server.ts
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { execSync } from "child_process";

const server = new McpServer({
  name: "repo-search",
  version: "1.0.0",
});

server.tool(
  "search_repo",
  "Search the current repository for code, symbols, or text patterns. " +
    "Use this when you need to find files, function definitions, usages, or " +
    "any text matching a pattern across the codebase.",
  {
    query: z.string().describe("The search query (regex or plain text)"),
    file_pattern: z
      .string()
      .optional()
      .describe("Glob pattern to filter files, e.g. '*.ts' or 'src/**/*.py'"),
    max_results: z
      .number()
      .optional()
      .default(20)
      .describe("Maximum number of results to return"),
  },
  async ({ query, file_pattern, max_results }) => {
    try {
      // Use ripgrep for fast indexed search
      const globArg = file_pattern ? `--glob '${file_pattern}'` : "";
      const cmd = `rg --json --max-count ${max_results} ${globArg} '${query}' .`;
      const output = execSync(cmd, {
        encoding: "utf-8",
        maxBuffer: 1024 * 1024,
        timeout: 10_000,
      });

      // Parse ripgrep JSON output into a readable format
      const matches = output
        .trim()
        .split("\n")
        .filter(Boolean)
        .map((line) => JSON.parse(line))
        .filter((entry) => entry.type === "match")
        .map((entry) => ({
          file: entry.data.path.text,
          line: entry.data.line_number,
          text: entry.data.lines.text.trim(),
        }));

      return {
        content: [
          {
            type: "text",
            text: JSON.stringify(matches, null, 2),
          },
        ],
      };
    } catch (err: any) {
      if (err.status === 1) {
        return { content: [{ type: "text", text: "No matches found." }] };
      }
      throw err;
    }
  }
);

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main();
```

### 2. Register it in Codex config

```toml
# ~/.codex/config.toml

[mcp_servers.repo-search]
command = "npx"
args = ["tsx", "/path/to/repo-search-server.ts"]
```

### 3. How Codex sees it

Per [[Unrolling the Codex agent loop]], MCP tools appear in the `tools` array sent to the Responses API just like built-in tools:

```json
{
  "type": "function",
  "name": "mcp__repo-search__search_repo",
  "description": "Search the current repository for code, symbols, or text patterns...",
  "strict": false,
  "parameters": {
    "type": "object",
    "properties": {
      "query": { "type": "string" },
      "file_pattern": { "type": "string" },
      "max_results": { "type": "number" }
    },
    "required": ["query"]
  }
}
```

The model can then call `mcp__repo-search__search_repo` and receive results as a `function_call_output` appended to the conversation `input`.

---

## Option 2: Semantic Search with an Embedding Index

For richer "find code related to X" queries, build an embedding-based index:

### 1. Index the repo

```typescript
// index-repo.ts
import { OpenAI } from "openai";
import { globSync } from "glob";
import { readFileSync, writeFileSync } from "fs";

const openai = new OpenAI();

interface ChunkRecord {
  file: string;
  startLine: number;
  text: string;
  embedding: number[];
}

async function indexRepo(repoRoot: string): Promise<ChunkRecord[]> {
  const files = globSync("**/*.{ts,js,py,rs,go,md}", {
    cwd: repoRoot,
    ignore: ["node_modules/**", ".git/**", "dist/**"],
  });

  const chunks: { file: string; startLine: number; text: string }[] = [];

  for (const file of files) {
    const content = readFileSync(`${repoRoot}/${file}`, "utf-8");
    const lines = content.split("\n");

    // Chunk into ~50-line windows with overlap
    for (let i = 0; i < lines.length; i += 40) {
      chunks.push({
        file,
        startLine: i + 1,
        text: lines.slice(i, i + 50).join("\n"),
      });
    }
  }

  // Batch embed
  const batchSize = 100;
  const records: ChunkRecord[] = [];

  for (let i = 0; i < chunks.length; i += batchSize) {
    const batch = chunks.slice(i, i + batchSize);
    const resp = await openai.embeddings.create({
      model: "text-embedding-3-small",
      input: batch.map((c) => c.text),
    });
    for (let j = 0; j < batch.length; j++) {
      records.push({ ...batch[j], embedding: resp.data[j].embedding });
    }
  }

  writeFileSync(".codex/repo-index.json", JSON.stringify(records));
  return records;
}
```

### 2. Search via cosine similarity

```typescript
// search.ts
function cosineSimilarity(a: number[], b: number[]): number {
  let dot = 0, magA = 0, magB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    magA += a[i] * a[i];
    magB += b[i] * b[i];
  }
  return dot / (Math.sqrt(magA) * Math.sqrt(magB));
}

async function searchIndex(
  query: string,
  index: ChunkRecord[],
  topK = 10
): Promise<ChunkRecord[]> {
  const openai = new OpenAI();
  const resp = await openai.embeddings.create({
    model: "text-embedding-3-small",
    input: query,
  });
  const queryEmb = resp.data[0].embedding;

  return index
    .map((record) => ({
      ...record,
      score: cosineSimilarity(queryEmb, record.embedding),
    }))
    .sort((a, b) => b.score - a.score)
    .slice(0, topK);
}
```

### 3. Expose it as an MCP tool

Wire the `searchIndex` function into the same MCP server pattern from Option 1, loading the index from `.codex/repo-index.json` on startup.

---

## Option 3: Using the SDK Programmatically

If you're embedding Codex via the TypeScript SDK (as documented in your active note), you can orchestrate search **outside** the agent loop and inject results as context:

```typescript
import { Codex } from "@openai/codex-sdk";

// Run your search
const searchResults = await searchIndex("authentication middleware", index);
const context = searchResults
  .map((r) => `// ${r.file}:${r.startLine}\n${r.text}`)
  .join("\n---\n");

// Feed results into Codex as part of the prompt
const codex = new Codex();
const thread = codex.startThread({ workingDirectory: "/path/to/repo" });

const turn = await thread.run(
  `Here are relevant code sections from the repo:\n\n${context}\n\n` +
  `Based on the above, refactor the auth middleware to support JWT tokens.`
);
```

---

## Comparison

| Approach | Pros | Cons |
| --- | --- | --- |
| **MCP Server (ripgrep)** | Fast, no index to maintain, model decides when to search | Text-only matching, no semantic understanding |
| **MCP Server (embeddings)** | Semantic "find related code," handles natural language queries | Requires building/refreshing an index, embedding API costs |
| **SDK context injection** | Full control over what the model sees, no tool overhead | Model can't search on its own; you must anticipate what's relevant |

---

## Important Considerations

### Prompt caching impact
As noted in [[Unrolling the Codex agent loop]], MCP tools must enumerate in a **stable order** to preserve prompt cache hits. If your MCP server's tool list is static (just `search_repo`), this isn't an issue. But avoid dynamically changing tool definitions mid-conversation.

### Sandbox boundaries
From the agent loop post: MCP-provided tools are **not sandboxed by Codex** — they're responsible for enforcing their own guardrails. Your search tool should:
- Restrict search to the repo root (no traversal to `/etc/passwd`)
- Set timeouts on subprocess calls
- Cap output size to avoid blowing up the context window

### Context window budget
Search results consume tokens. Keep results concise — return file paths, line numbers, and short snippets rather than full file contents. The model can always use the built-in `shell` tool to `cat` a file if it needs more context.
[Timestamp: 2026/02/23 17:03:17]

**user**: Option 2
[Timestamp: 2026/02/23 17:03:29]

**ai**: 

# Semantic Repo Search Tool via Embedding Index

A complete implementation of an MCP server that indexes your repository with embeddings and exposes semantic search to Codex.

---

## Architecture

```
┌──────────────┐       ┌──────────────────────┐       ┌──────────────┐
│  Codex CLI   │ stdio │  MCP Server          │       │  OpenAI      │
│  / SDK       │◄─────►│  repo-search         │──────►│  Embeddings  │
│              │ JSONL │                      │       │  API         │
└──────────────┘       │  ┌────────────────┐  │       └──────────────┘
                       │  │ .codex/        │  │
                       │  │ repo-index.json│  │
                       │  └────────────────┘  │
                       └──────────────────────┘
```

The server has two modes:
1. **Index** — walk the repo, chunk files, embed each chunk, write to disk
2. **Search** — embed the query, rank chunks by cosine similarity, return top-K

---

## Full Implementation

### Project setup

```bash
mkdir repo-search-mcp && cd repo-search-mcp
npm init -y
npm install @modelcontextprotocol/sdk openai glob zod
npm install -D tsx typescript @types/node
```

```json
// tsconfig.json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "Node16",
    "moduleResolution": "Node16",
    "strict": true,
    "esModuleInterop": true,
    "outDir": "dist",
    "rootDir": "src"
  },
  "include": ["src"]
}
```

### Core types and utilities

```typescript
// src/types.ts

export interface ChunkRecord {
  /** Relative file path from repo root */
  file: string;
  /** 1-based starting line number */
  startLine: number;
  /** 1-based ending line number */
  endLine: number;
  /** Raw text content of the chunk */
  text: string;
  /** Embedding vector */
  embedding: number[];
}

export interface IndexMetadata {
  /** ISO timestamp of when the index was built */
  createdAt: string;
  /** Model used for embeddings */
  model: string;
  /** Number of chunks indexed */
  chunkCount: number;
  /** Number of files indexed */
  fileCount: number;
  /** Repo root path */
  repoRoot: string;
}

export interface RepoIndex {
  metadata: IndexMetadata;
  chunks: ChunkRecord[];
}
```

```typescript
// src/utils.ts

export function cosineSimilarity(a: number[], b: number[]): number {
  let dot = 0;
  let magA = 0;
  let magB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    magA += a[i] * a[i];
    magB += b[i] * b[i];
  }
  const denom = Math.sqrt(magA) * Math.sqrt(magB);
  return denom === 0 ? 0 : dot / denom;
}

/**
 * Split file content into overlapping chunks.
 *
 * - chunkSize:  number of lines per chunk
 * - overlap:    number of lines shared between consecutive chunks
 */
export function chunkLines(
  content: string,
  chunkSize = 50,
  overlap = 10
): { startLine: number; endLine: number; text: string }[] {
  const lines = content.split("\n");
  const chunks: { startLine: number; endLine: number; text: string }[] = [];
  const step = chunkSize - overlap;

  for (let i = 0; i < lines.length; i += step) {
    const slice = lines.slice(i, i + chunkSize);
    if (slice.length === 0) break;

    chunks.push({
      startLine: i + 1,
      endLine: i + slice.length,
      text: slice.join("\n"),
    });

    // If this chunk reached the end of the file, stop
    if (i + chunkSize >= lines.length) break;
  }

  return chunks;
}
```

### Indexer

```typescript
// src/indexer.ts

import { OpenAI } from "openai";
import { globSync } from "glob";
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "fs";
import { resolve, relative } from "path";
import { chunkLines } from "./utils.js";
import type { ChunkRecord, RepoIndex } from "./types.js";

/** File extensions to index, grouped by language family */
const DEFAULT_PATTERNS = [
  "**/*.{ts,tsx,js,jsx,mjs,cjs}",
  "**/*.{py,pyi}",
  "**/*.{rs,go,java,kt,scala}",
  "**/*.{c,cpp,cc,h,hpp}",
  "**/*.{rb,ex,exs,erl}",
  "**/*.{md,mdx,txt,rst}",
  "**/*.{yaml,yml,toml,json}",
  "**/*.{sh,bash,zsh,fish}",
];

const DEFAULT_IGNORE = [
  "node_modules/**",
  ".git/**",
  "dist/**",
  "build/**",
  "target/**",
  "__pycache__/**",
  "*.min.js",
  "*.lock",
  "package-lock.json",
  ".codex/**",
];

const EMBEDDING_MODEL = "text-embedding-3-small";
const BATCH_SIZE = 100;
const MAX_CHUNK_CHARS = 8000; // stay well within token limits

export async function buildIndex(repoRoot: string): Promise<RepoIndex> {
  const openai = new OpenAI();
  const absRoot = resolve(repoRoot);

  // Discover files
  const files: string[] = [];
  for (const pattern of DEFAULT_PATTERNS) {
    const matches = globSync(pattern, {
      cwd: absRoot,
      ignore: DEFAULT_IGNORE,
      nodir: true,
    });
    files.push(...matches);
  }

  // Deduplicate
  const uniqueFiles = [...new Set(files)];
  console.error(`[indexer] Found ${uniqueFiles.length} files to index`);

  // Chunk all files
  const allChunks: Omit<ChunkRecord, "embedding">[] = [];

  for (const file of uniqueFiles) {
    try {
      const content = readFileSync(`${absRoot}/${file}`, "utf-8");

      // Skip very large files (likely generated)
      if (content.length > 500_000) {
        console.error(`[indexer] Skipping large file: ${file}`);
        continue;
      }

      const chunks = chunkLines(content, 50, 10);
      for (const chunk of chunks) {
        // Truncate oversized chunks
        const text =
          chunk.text.length > MAX_CHUNK_CHARS
            ? chunk.text.slice(0, MAX_CHUNK_CHARS)
            : chunk.text;

        allChunks.push({
          file,
          startLine: chunk.startLine,
          endLine: chunk.endLine,
          text,
        });
      }
    } catch (err) {
      console.error(`[indexer] Error reading ${file}:`, err);
    }
  }

  console.error(`[indexer] Created ${allChunks.length} chunks, embedding...`);

  // Batch embed
  const records: ChunkRecord[] = [];

  for (let i = 0; i < allChunks.length; i += BATCH_SIZE) {
    const batch = allChunks.slice(i, i + BATCH_SIZE);

    // Prefix each chunk with its file path for better embedding quality
    const inputs = batch.map(
      (c) => `File: ${c.file} (lines ${c.startLine}-${c.endLine})\n${c.text}`
    );

    const resp = await openai.embeddings.create({
      model: EMBEDDING_MODEL,
      input: inputs,
    });

    for (let j = 0; j < batch.length; j++) {
      records.push({
        ...batch[j],
        embedding: resp.data[j].embedding,
      });
    }

    console.error(
      `[indexer] Embedded ${Math.min(i + BATCH_SIZE, allChunks.length)}/${allChunks.length}`
    );
  }

  // Write index
  const index: RepoIndex = {
    metadata: {
      createdAt: new Date().toISOString(),
      model: EMBEDDING_MODEL,
      chunkCount: records.length,
      fileCount: uniqueFiles.length,
      repoRoot: absRoot,
    },
    chunks: records,
  };

  const indexDir = `${absRoot}/.codex`;
  if (!existsSync(indexDir)) {
    mkdirSync(indexDir, { recursive: true });
  }
  writeFileSync(`${indexDir}/repo-index.json`, JSON.stringify(index));

  console.error(
    `[indexer] Index written to .codex/repo-index.json ` +
      `(${records.length} chunks from ${uniqueFiles.length} files)`
  );

  return index;
}
```

### Search engine

```typescript
// src/search.ts

import { OpenAI } from "openai";
import { cosineSimilarity } from "./utils.js";
import type { ChunkRecord, RepoIndex } from "./types.js";

export interface SearchResult {
  file: string;
  startLine: number;
  endLine: number;
  score: number;
  snippet: string;
}

const EMBEDDING_MODEL = "text-embedding-3-small";

export async function searchIndex(
  query: string,
  index: RepoIndex,
  options: {
    topK?: number;
    filePattern?: string;
    minScore?: number;
  } = {}
): Promise<SearchResult[]> {
  const { topK = 10, filePattern, minScore = 0.0 } = options;
  const openai = new OpenAI();

  // Embed the query
  const resp = await openai.embeddings.create({
    model: EMBEDDING_MODEL,
    input: query,
  });
  const queryEmbedding = resp.data[0].embedding;

  // Filter chunks by file pattern if specified
  let candidates = index.chunks;
  if (filePattern) {
    const regex = globToRegex(filePattern);
    candidates = candidates.filter((c) => regex.test(c.file));
  }

  // Rank by cosine similarity
  const scored = candidates
    .map((chunk) => ({
      file: chunk.file,
      startLine: chunk.startLine,
      endLine: chunk.endLine,
      score: cosineSimilarity(queryEmbedding, chunk.embedding),
      snippet: truncateSnippet(chunk.text, 500),
    }))
    .filter((r) => r.score >= minScore)
    .sort((a, b) => b.score - a.score)
    .slice(0, topK);

  return scored;
}

/** Truncate text to maxLen chars, breaking at a newline if possible */
function truncateSnippet(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text;
  const cut = text.lastIndexOf("\n", maxLen);
  return (cut > maxLen * 0.5 ? text.slice(0, cut) : text.slice(0, maxLen)) + "\n...";
}

/** Convert a simple glob pattern to a RegExp */
function globToRegex(pattern: string): RegExp {
  const escaped = pattern
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*/g, ".*")
    .replace(/\?/g, ".");
  return new RegExp(escaped);
}
```

### MCP Server

```typescript
// src/server.ts

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { readFileSync, existsSync } from "fs";
import { resolve } from "path";
import { buildIndex } from "./indexer.js";
import { searchIndex } from "./search.js";
import type { RepoIndex } from "./types.js";

let cachedIndex: RepoIndex | null = null;

function getRepoRoot(): string {
  return process.env.REPO_ROOT || process.cwd();
}

function loadIndex(): RepoIndex | null {
  const indexPath = resolve(getRepoRoot(), ".codex/repo-index.json");
  if (!existsSync(indexPath)) return null;

  try {
    const data = readFileSync(indexPath, "utf-8");
    return JSON.parse(data) as RepoIndex;
  } catch {
    return null;
  }
}

const server = new McpServer({
  name: "repo-search",
  version: "1.0.0",
});

// ── Tool 1: Build / rebuild the index ────────────────────────────

server.tool(
  "index_repo",
  "Build or rebuild the semantic search index for the current repository. " +
    "Run this once before searching, or again after significant code changes. " +
    "This walks the repo, chunks every source file, and embeds each chunk.",
  {},
  async () => {
    const repoRoot = getRepoRoot();
    cachedIndex = await buildIndex(repoRoot);

    return {
      content: [
        {
          type: "text",
          text:
            `Index built successfully.\n` +
            `- Files indexed: ${cachedIndex.metadata.fileCount}\n` +
            `- Chunks created: ${cachedIndex.metadata.chunkCount}\n` +
            `- Model: ${cachedIndex.metadata.model}\n` +
            `- Saved to: .codex/repo-index.json`,
        },
      ],
    };
  }
);

// ── Tool 2: Semantic search ──────────────────────────────────────

server.tool(
  "search_repo",
  "Semantically search the repository for code, documentation, or concepts. " +
    "Unlike grep, this understands meaning — e.g., searching 'authentication flow' " +
    "will find OAuth handlers, login middleware, JWT validation, etc. " +
    "The repo must be indexed first (use index_repo if needed).",
  {
    query: z
      .string()
      .describe(
        "Natural language query describing what you're looking for. " +
          "Be specific: 'error handling in the payment service' works better than 'errors'."
      ),
    file_pattern: z
      .string()
      .optional()
      .describe("Glob pattern to restrict search to certain files, e.g. '*.ts' or 'src/api/*'"),
    max_results: z
      .number()
      .optional()
      .default(10)
      .describe("Maximum number of results to return (default: 10)"),
    min_score: z
      .number()
      .optional()
      .default(0.3)
      .describe("Minimum similarity score threshold 0-1 (default: 0.3)"),
  },
  async ({ query, file_pattern, max_results, min_score }) => {
    // Load index if not cached
    if (!cachedIndex) {
      cachedIndex = loadIndex();
    }
    if (!cachedIndex) {
      return {
        content: [
          {
            type: "text",
            text:
              "No index found. Run the `index_repo` tool first to build " +
              "the semantic search index.",
          },
        ],
        isError: true,
      };
    }

    const results = await searchIndex(query, cachedIndex, {
      topK: max_results,
      filePattern: file_pattern,
      minScore: min_score,
    });

    if (results.length === 0) {
      return {
        content: [
          {
            type: "text",
            text: `No results found for "${query}" above the similarity threshold (${min_score}).`,
          },
        ],
      };
    }

    // Format results for the model
    const formatted = results
      .map(
        (r, i) =>
          `### Result ${i + 1} (score: ${r.score.toFixed(3)})\n` +
          `**${r.file}** lines ${r.startLine}–${r.endLine}\n` +
          "```\n" +
          r.snippet +
          "\n```"
      )
      .join("\n\n");

    return {
      content: [
        {
          type: "text",
          text:
            `Found ${results.length} results for "${query}":\n\n` +
            formatted,
        },
      ],
    };
  }
);

// ── Tool 3: Index status ─────────────────────────────────────────

server.tool(
  "index_status",
  "Check whether a semantic search index exists and show its metadata " +
    "(when it was built, how many files/chunks it contains).",
  {},
  async () => {
    if (!cachedIndex) {
      cachedIndex = loadIndex();
    }
    if (!cachedIndex) {
      return {
        content: [
          {
            type: "text",
            text: "No index found. Run `index_repo` to create one.",
          },
        ],
      };
    }

    const m = cachedIndex.metadata;
    return {
      content: [
        {
          type: "text",
          text:
            `Index status:\n` +
            `- Created: ${m.createdAt}\n` +
            `- Files: ${m.fileCount}\n` +
            `- Chunks: ${m.chunkCount}\n` +
            `- Model: ${m.model}\n` +
            `- Repo root: ${m.repoRoot}`,
        },
      ],
    };
  }
);

// ── Start ────────────────────────────────────────────────────────

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("[repo-search] MCP server running on stdio");
}

main();
```

---

## Codex Configuration

```toml
# ~/.codex/config.toml

[mcp_servers.repo-search]
command = "npx"
args = ["tsx", "/path/to/repo-search-mcp/src/server.ts"]

# Optional: set the repo root explicitly
# env = { REPO_ROOT = "/path/to/your/project" }
```

Add the index file to `.gitignore`:

```gitignore
# .gitignore
.codex/repo-index.json
```

---

## Usage Flow

Once configured, the model sees three tools and uses them naturally:

```
You: "Find all code related to rate limiting"

Codex (thinks): I should search the repo semantically.
       → calls mcp__repo-search__search_repo({ query: "rate limiting" })

Codex: Found 7 relevant results:
       1. src/middleware/rate-limiter.ts (lines 1–50, score: 0.891)
       2. src/api/routes.ts (lines 120–145, score: 0.743)
       ...
```

If the index doesn't exist yet:

```
You: "What authentication patterns does this repo use?"

Codex (thinks): I need to search but should check the index first.
       → calls mcp__repo-search__index_status()
       ← "No index found."
       → calls mcp__repo-search__index_repo()
       ← "Index built: 342 files, 1,847 chunks"
       → calls mcp__repo-search__search_repo({ query: "authentication patterns" })
       ...
```

---

## Performance & Cost Considerations

| Factor | Detail |
| --- | --- |
| **Indexing cost** | ~$0.02 per 1M tokens with `text-embedding-3-small`. A 10K-line repo ≈ 200 chunks ≈ ~$0.001 |
| **Search cost** | One embedding call per query (~$0.00001) |
| **Index size** | ~1.5 KB per chunk (1536-dim float array). 2,000 chunks ≈ 3 MB JSON |
| **Latency** | Search: ~200ms (embedding call + in-memory sort). Indexing: depends on repo size |
| **Prompt cache impact** | The tool list is static (3 tools, stable order), so per [[Unrolling the Codex agent loop]] there's no risk of cache misses from tool enumeration |
| **Context budget** | Each result is ~5–10 lines of snippet. 10 results ≈ 500–1000 tokens — well within budget |

### Incremental re-indexing (optimization)

For large repos, you can add a fourth tool that only re-indexes changed files:

```typescript
server.tool(
  "reindex_changed",
  "Re-index only files that changed since the last index was built (uses git diff).",
  {},
  async () => {
    const since = cachedIndex?.metadata.createdAt;
    // Use git diff --name-only --since=<timestamp> to find changed files
    // Re-chunk and re-embed only those files
    // Merge into existing index
    // ...
  }
);
```
[Timestamp: 2026/02/23 17:04:48]
