#!/usr/bin/env node

/**
 * web-fetch-mcp
 *
 * MCP server for safe, high-signal web browsing and content fetching for LLM agents.
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ListPromptsRequestSchema,
  GetPromptRequestSchema,
  McpError,
  ErrorCode,
} from '@modelcontextprotocol/sdk/types.js';

import { loadConfig, validateConfig, getConfig } from './config.js';
import { executeFetch, getFetchInputSchema } from './tools/fetch.js';
import { executeExtract, getExtractInputSchema } from './tools/extract.js';
import { executeChunk, getChunkInputSchema } from './tools/chunk.js';
import { executeCompact, getCompactInputSchema } from './tools/compact.js';
import { closeBrowser } from './fetcher/browser-renderer.js';

const PROMPTS = [
  {
    name: 'fetch_url',
    title: 'Fetch URL',
    description: 'Fetch a URL and return the LLMPacket',
    arguments: [
      { name: 'url', description: 'The URL to fetch', required: true },
      { name: 'mode', description: 'Fetch mode: auto, http, or render', required: false },
      { name: 'extraction', description: 'Optional JSON for options.extraction', required: false },
    ],
  },
  {
    name: 'fetch_and_chunk',
    title: 'Fetch And Chunk',
    description: 'Fetch a URL, then chunk the content',
    arguments: [
      { name: 'url', description: 'The URL to fetch', required: true },
      { name: 'max_tokens', description: 'Max tokens per chunk', required: false },
      { name: 'strategy', description: 'Chunk strategy: headings_first or balanced', required: false },
    ],
  },
  {
    name: 'fetch_and_compact',
    title: 'Fetch And Compact',
    description: 'Fetch a URL, then compact the content',
    arguments: [
      { name: 'url', description: 'The URL to fetch', required: true },
      { name: 'max_tokens', description: 'Target max tokens for compaction', required: false },
      { name: 'mode', description: 'Compaction mode: structural, salience, map_reduce, question_focused', required: false },
      { name: 'question', description: 'Question for question-focused compaction', required: false },
    ],
  },
  {
    name: 'fetch_ai_search',
    title: 'Fetch With AI Search',
    description: 'Fetch a URL, upload to AI Search, and optionally run a query',
    arguments: [
      { name: 'url', description: 'The URL to fetch', required: true },
      { name: 'query', description: 'Query string for AI Search', required: true },
      { name: 'wait_ms', description: 'Wait before querying AI Search (ms)', required: false },
      { name: 'mode', description: 'AI Search mode: search or ai_search', required: false },
    ],
  },
];

const PROMPT_MAP = new Map(PROMPTS.map(prompt => [prompt.name, prompt]));

function getArgs(
  args: Record<string, string> | undefined,
  required: string[]
): Record<string, string> {
  const resolved: Record<string, string> = {};
  for (const key of required) {
    const value = args?.[key];
    if (!value || value.trim() === '') {
      throw new McpError(ErrorCode.InvalidParams, `Missing required argument: ${key}`);
    }
    resolved[key] = value;
  }

  if (args) {
    for (const [key, value] of Object.entries(args)) {
      if (value !== undefined) {
        resolved[key] = value;
      }
    }
  }

  return resolved;
}

function parseJsonArgument(value: string | undefined): unknown | undefined {
  if (!value) return undefined;
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  if ((trimmed.startsWith('{') && trimmed.endsWith('}')) ||
      (trimmed.startsWith('[') && trimmed.endsWith(']'))) {
    try {
      return JSON.parse(trimmed);
    } catch {
      return value;
    }
  }
  return value;
}

function parseNumberArgument(value: string | undefined): number | string | undefined {
  if (!value) return undefined;
  const parsed = Number(value);
  if (Number.isFinite(parsed)) {
    return parsed;
  }
  return value;
}

function buildFetchUrlPrompt(args: Record<string, string>): string {
  const url = args['url'] ?? '';
  const mode = args['mode'];
  const extraction = parseJsonArgument(args['extraction']);

  const options: Record<string, unknown> = {};
  if (mode) {
    options['mode'] = mode;
  }
  if (extraction !== undefined) {
    options['extraction'] = extraction;
  }

  const payload: Record<string, unknown> = { url };
  if (Object.keys(options).length > 0) {
    payload['options'] = options;
  }

  return [
    'Call the `fetch` tool with the following input:',
    '```json',
    JSON.stringify(payload, null, 2),
    '```',
  ].join('\n');
}

function buildFetchAndChunkPrompt(args: Record<string, string>): string {
  const url = args['url'] ?? '';
  const maxTokens = parseNumberArgument(args['max_tokens']);
  const strategy = args['strategy'];

  const options: Record<string, unknown> = {};
  if (maxTokens !== undefined) {
    options['max_tokens'] = maxTokens;
  }
  if (strategy) {
    options['strategy'] = strategy;
  }

  const chunkPayload: Record<string, unknown> = {
    packet: '<fetchResult.packet>',
  };
  if (Object.keys(options).length > 0) {
    chunkPayload['options'] = options;
  }

  return [
    '1) Call `fetch` with:',
    '```json',
    JSON.stringify({ url }, null, 2),
    '```',
    '',
    '2) Call `chunk` with the packet from step 1:',
    '```json',
    JSON.stringify(chunkPayload, null, 2),
    '```',
  ].join('\n');
}

function buildFetchAndCompactPrompt(args: Record<string, string>): string {
  const url = args['url'] ?? '';
  const maxTokens = parseNumberArgument(args['max_tokens']);
  const mode = args['mode'];
  const question = args['question'];

  const options: Record<string, unknown> = {};
  if (maxTokens !== undefined) {
    options['max_tokens'] = maxTokens;
  }
  if (mode) {
    options['mode'] = mode;
  }
  if (question) {
    options['question'] = question;
  }

  const compactPayload: Record<string, unknown> = {
    input: '<fetchResult.packet>',
  };
  if (Object.keys(options).length > 0) {
    compactPayload['options'] = options;
  }

  return [
    '1) Call `fetch` with:',
    '```json',
    JSON.stringify({ url }, null, 2),
    '```',
    '',
    '2) Call `compact` with the packet from step 1:',
    '```json',
    JSON.stringify(compactPayload, null, 2),
    '```',
  ].join('\n');
}

function buildFetchAiSearchPrompt(args: Record<string, string>): string {
  const url = args['url'] ?? '';
  const query = args['query'] ?? '';
  const waitMs = parseNumberArgument(args['wait_ms']);
  const mode = args['mode'];

  const aiSearchOptions: Record<string, unknown> = {
    enabled: true,
    query: {
      query,
      ...(mode ? { mode } : {}),
    },
  };
  if (waitMs !== undefined) {
    aiSearchOptions['wait_ms'] = waitMs;
  }

  const payload = {
    url,
    options: {
      ai_search: aiSearchOptions,
    },
  };

  return [
    'Call the `fetch` tool with AI Search enabled:',
    '```json',
    JSON.stringify(payload, null, 2),
    '```',
  ].join('\n');
}

// Tool definitions
const TOOLS = [
  {
    name: 'fetch',
    description: `Fetch and extract content from a URL. Supports HTML, JavaScript-rendered pages (SPA), Markdown, PDF, JSON, and XML/RSS feeds.

Returns an LLMPacket with:
- Normalized markdown content
- Metadata (title, author, date)
- Document outline
- Key blocks for citation
- Prompt injection detection warnings
- Optional Cloudflare R2 upload for AI Search indexing

Security: Blocks private IPs, respects robots.txt, rate limits per host.`,
    inputSchema: getFetchInputSchema(),
  },
  {
    name: 'extract',
    description: `Extract and normalize content from raw bytes or a URL.

Use this when you already have content and want to process it into an LLMPacket.
Supports all content types: HTML, Markdown, PDF, JSON, XML.`,
    inputSchema: getExtractInputSchema(),
  },
  {
    name: 'chunk',
    description: `Split an LLMPacket into semantic chunks for context-limited processing.

Chunks preserve:
- Heading boundaries (won't split mid-section)
- Code blocks (won't split mid-block)
- Logical paragraph structure

Each chunk includes heading path for context.`,
    inputSchema: getChunkInputSchema(),
  },
  {
    name: 'compact',
    description: `Intelligently compress content while preserving key information.

Compaction modes:
- structural: Remove boilerplate, keep structure
- salience: Keep high-information-density sentences
- map_reduce: Summarize chunks then merge
- question_focused: Keep content relevant to a specific question

Always preserves numbers, dates, names, definitions, and procedures.`,
    inputSchema: getCompactInputSchema(),
  },
];

/**
 * Main entry point
 */
async function main(): Promise<void> {
  // Load and validate configuration
  const config = loadConfig();
  const configErrors = validateConfig(config);

  if (configErrors.length > 0) {
    console.error('Configuration errors:');
    configErrors.forEach(err => console.error(`  - ${err}`));
    process.exit(1);
  }

  // Create MCP server
  const server = new Server(
    {
      name: 'web-fetch-mcp',
      version: '1.0.0',
    },
    {
      capabilities: {
        tools: {},
        prompts: {
          listChanged: false,
        },
      },
    }
  );

  // Handle list tools request
  server.setRequestHandler(ListToolsRequestSchema, async () => {
    return { tools: TOOLS };
  });

  server.setRequestHandler(ListPromptsRequestSchema, async () => {
    return { prompts: PROMPTS };
  });

  server.setRequestHandler(GetPromptRequestSchema, async (request) => {
    const { name, arguments: args } = request.params;
    if (!PROMPT_MAP.has(name)) {
      throw new McpError(ErrorCode.InvalidParams, `Prompt ${name} not found`);
    }

    switch (name) {
      case 'fetch_url': {
        const resolved = getArgs(args, ['url']);
        return {
          description: 'Fetch a URL and return the LLMPacket',
          messages: [
            {
              role: 'user',
              content: {
                type: 'text',
                text: buildFetchUrlPrompt(resolved),
              },
            },
          ],
        };
      }
      case 'fetch_and_chunk': {
        const resolved = getArgs(args, ['url']);
        return {
          description: 'Fetch a URL, then chunk the content',
          messages: [
            {
              role: 'user',
              content: {
                type: 'text',
                text: buildFetchAndChunkPrompt(resolved),
              },
            },
          ],
        };
      }
      case 'fetch_and_compact': {
        const resolved = getArgs(args, ['url']);
        return {
          description: 'Fetch a URL, then compact the content',
          messages: [
            {
              role: 'user',
              content: {
                type: 'text',
                text: buildFetchAndCompactPrompt(resolved),
              },
            },
          ],
        };
      }
      case 'fetch_ai_search': {
        const resolved = getArgs(args, ['url', 'query']);
        return {
          description: 'Fetch a URL, upload to AI Search, and optionally run a query',
          messages: [
            {
              role: 'user',
              content: {
                type: 'text',
                text: buildFetchAiSearchPrompt(resolved),
              },
            },
          ],
        };
      }
      default:
        throw new McpError(ErrorCode.InvalidParams, `Prompt ${name} not found`);
    }
  });

  // Handle tool calls
  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const { name, arguments: args } = request.params;

    try {
      switch (name) {
        case 'fetch': {
          const result = await executeFetch({
            url: (args as Record<string, unknown>)['url'] as string,
            options: (args as Record<string, unknown>)['options'] as Record<string, unknown> | undefined,
          });

          return {
            content: [
              {
                type: 'text',
                text: JSON.stringify(result, null, 2),
              },
            ],
          };
        }

        case 'extract': {
          const input = (args as Record<string, unknown>)['input'] as Record<string, unknown>;

          // Handle base64 raw_bytes if provided
          let rawBytes: Buffer | undefined;
          if (input['raw_bytes'] && typeof input['raw_bytes'] === 'string') {
            rawBytes = Buffer.from(input['raw_bytes'], 'base64');
          }

          const result = await executeExtract({
            input: {
              url: input['url'] as string | undefined,
              raw_bytes: rawBytes,
              content_type: input['content_type'] as string | undefined,
              canonical_url: input['canonical_url'] as string | undefined,
            },
            options: (args as Record<string, unknown>)['options'] as Record<string, unknown> | undefined,
          });

          return {
            content: [
              {
                type: 'text',
                text: JSON.stringify(result, null, 2),
              },
            ],
          };
        }

        case 'chunk': {
          const result = executeChunk({
            packet: (args as Record<string, unknown>)['packet'] as never,
            options: (args as Record<string, unknown>)['options'] as Record<string, unknown> | undefined,
          });

          return {
            content: [
              {
                type: 'text',
                text: JSON.stringify(result, null, 2),
              },
            ],
          };
        }

        case 'compact': {
          const result = executeCompact({
            input: (args as Record<string, unknown>)['input'] as never,
            options: (args as Record<string, unknown>)['options'] as Record<string, unknown> | undefined,
          });

          return {
            content: [
              {
                type: 'text',
                text: JSON.stringify(result, null, 2),
              },
            ],
          };
        }

        default:
          return {
            content: [
              {
                type: 'text',
                text: JSON.stringify({
                  success: false,
                  error: {
                    code: 'UNKNOWN_TOOL',
                    message: `Unknown tool: ${name}`,
                  },
                }),
              },
            ],
            isError: true,
          };
      }
    } catch (err) {
      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify({
              success: false,
              error: {
                code: 'TOOL_ERROR',
                message: err instanceof Error ? err.message : 'Unknown error',
              },
            }),
          },
        ],
        isError: true,
      };
    }
  });

  // Handle graceful shutdown
  const shutdown = async () => {
    console.error('Shutting down...');
    await closeBrowser();
    process.exit(0);
  };

  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);

  // Start server
  const transport = new StdioServerTransport();
  await server.connect(transport);

  console.error('web-fetch-mcp server started');
}

// Run
main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
