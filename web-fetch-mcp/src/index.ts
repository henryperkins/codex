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
} from '@modelcontextprotocol/sdk/types.js';

import { loadConfig, validateConfig, getConfig } from './config.js';
import { executeFetch, getFetchInputSchema } from './tools/fetch.js';
import { executeExtract, getExtractInputSchema } from './tools/extract.js';
import { executeChunk, getChunkInputSchema } from './tools/chunk.js';
import { executeCompact, getCompactInputSchema } from './tools/compact.js';
import { closeBrowser } from './fetcher/browser-renderer.js';

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
      },
    }
  );

  // Handle list tools request
  server.setRequestHandler(ListToolsRequestSchema, async () => {
    return { tools: TOOLS };
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
