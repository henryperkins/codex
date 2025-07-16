import type { Tiktoken } from "tiktoken";

import iconv from "iconv-lite";
import { JSDOM } from "jsdom";
import { AbortController } from "node:abort-controller";
import { URL } from "node:url";
import { Readability } from "readability";
import sanitizeHtml from "sanitize-html";
import { encoding_for_model, get_encoding } from "tiktoken";
import TurndownService from "turndown";
import { fetch, Agent } from "undici";

export interface ScrapeResult {
  url: string;
  title: string | null;
  markdown: string;
  text: string;
  html_excerpt?: string;
  token_count: number;
  meta: {
    model: string;
    truncated: boolean;
    fetched_at: string;
    fetch_ms: number;
  };
  error?: {
    code: string;
    message: string;
  };
}

export interface ScrapeOptions {
  url: string;
  selector?: string | Array<string>;
  attr?: string;
  truncate_tokens?: number;
  model?: string;
  include_html_excerpt?: boolean;
  no_readability?: boolean;
}

const MAX_REDIRECTS = 5;
const MAX_BODY_SIZE = 10 * 1024 * 1024; // 10 MB
const FETCH_TIMEOUT = 10000; // 10 seconds
const CHUNK_TIMEOUT = 3000; // 3 seconds per 64KB
const MAX_DOM_NODES = 100000;
const MAX_DATA_URI_SIZE = 50 * 1024; // 50 KB
const MAX_CONCURRENCY = 2;

let activeFetches = 0;

class ConcurrencyError extends Error {
  constructor() {
    super("Too many concurrent fetches");
    this.name = "ConcurrencyError";
  }
}

const encoderCache = new Map<string, Tiktoken>();

function getEncoder(model: string): Tiktoken {
  const cached = encoderCache.get(model);
  if (cached) {
    return cached;
  }

  try {
    const encoder = encoding_for_model(
      model as Parameters<typeof encoding_for_model>[0],
    );
    encoderCache.set(model, encoder);
    return encoder;
  } catch {
    // Fallback to cl100k_base for unknown models
    const encoder = get_encoding("cl100k_base");
    encoderCache.set(model, encoder);
    return encoder;
  }
}

function normalizeUrl(inputUrl: string): string {
  // Add https:// if no protocol specified
  let url = inputUrl;
  if (!url.match(/^https?:\/\//i)) {
    url = "https://" + url;
  }

  // Validate URL
  try {
    new URL(url);
    return url;
  } catch {
    throw new Error("Invalid URL format");
  }
}

async function fetchWithSafety(url: string): Promise<{
  body: string;
  contentType: string;
  finalUrl: string;
  fetchMs: number;
}> {
  if (activeFetches >= MAX_CONCURRENCY) {
    throw new ConcurrencyError();
  }

  activeFetches++;
  const startTime = Date.now();

  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), FETCH_TIMEOUT);

    const response = await fetch(url, {
      method: "GET",
      headers: {
        "Accept":
          "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.1",
        "User-Agent":
          "codex-cli-web-scraper/1.0 (+https://github.com/openai/codex)",
      },
      redirect: "follow",
      maxRedirections: MAX_REDIRECTS,
      signal: controller.signal as AbortSignal,
      dispatcher: new Agent({
        bodyTimeout: FETCH_TIMEOUT,
        headersTimeout: FETCH_TIMEOUT,
      }),
    });

    clearTimeout(timeout);

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const contentType = response.headers.get("content-type") || "";

    // Check if content is HTML-like
    if (!contentType.includes("html") && !contentType.includes("xml")) {
      // Try to detect HTML by reading first 512 bytes
      const reader = response.body?.getReader();
      if (!reader) {
        throw new Error("No response body");
      }

      const { value } = await reader.read();
      reader.releaseLock();

      const preview = new TextDecoder().decode(
        value?.slice(0, 512) || new Uint8Array(),
      );
      if (!preview.match(/<html|<!doctype\s+html/i)) {
        throw new Error("Response is not HTML");
      }
    }

    // Stream body with size limit
    let bodySize = 0;
    const chunks: Array<Uint8Array> = [];
    const reader = response.body?.getReader();
    if (!reader) {
      throw new Error("No response body");
    }

    const chunkTimeout = setTimeout(() => controller.abort(), CHUNK_TIMEOUT);

    // eslint-disable-next-line no-constant-condition
    while (true) {
      // eslint-disable-next-line no-await-in-loop
      const { done, value } = await reader.read();
      if (done) {
        break;
      }

      bodySize += value.length;
      if (bodySize > MAX_BODY_SIZE) {
        reader.releaseLock();
        throw new Error("Response body too large");
      }

      chunks.push(value);

      // Reset chunk timeout
      clearTimeout(chunkTimeout);
      setTimeout(() => controller.abort(), CHUNK_TIMEOUT);
    }

    clearTimeout(chunkTimeout);

    // Combine chunks
    const bodyBuffer = Buffer.concat(chunks);

    // Detect and convert encoding
    let body: string;
    const charset =
      contentType.match(/charset=([^;]+)/)?.[1]?.toLowerCase() || "utf-8";

    if (charset !== "utf-8" && iconv.encodingExists(charset)) {
      body = iconv.decode(bodyBuffer, charset);
    } else {
      body = bodyBuffer.toString("utf-8");
    }

    const fetchMs = Date.now() - startTime;

    return {
      body,
      contentType,
      finalUrl: response.url,
      fetchMs,
    };
  } finally {
    activeFetches--;
  }
}

function sanitizeAndPrepareHtml(inputHtml: string): string {
  // Remove script and style tags and their contents
  let html = inputHtml.replace(
    /<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi,
    "",
  );
  html = html.replace(/<style\b[^<]*(?:(?!<\/style>)<[^<]*)*<\/style>/gi, "");

  // Sanitize HTML
  return sanitizeHtml(html, {
    allowedTags: sanitizeHtml.defaults.allowedTags.concat([
      "img",
      "h1",
      "h2",
      "h3",
      "h4",
      "h5",
      "h6",
    ]),
    allowedAttributes: {
      ...sanitizeHtml.defaults.allowedAttributes,
      img: ["src", "alt", "title"],
    },
    transformTags: {
      img: (tagName, attribs) => {
        // Remove large data URIs
        if (
          attribs.src?.startsWith("data:") &&
          attribs.src.length > MAX_DATA_URI_SIZE
        ) {
          return {
            tagName: "span",
            attribs: {},
            text: "[Image removed: data URI too large]",
          };
        }
        return { tagName, attribs };
      },
    },
  });
}

function extractContent(
  dom: JSDOM,
  options: ScrapeOptions,
): { title: string | null; content: string; isReadability: boolean } {
  const document = dom.window.document;

  // Try user-specified selectors first
  if (options.selector) {
    const selectors = Array.isArray(options.selector)
      ? options.selector
      : [options.selector];
    for (const selector of selectors) {
      try {
        const elements = document.querySelectorAll(selector);
        if (elements.length > 0) {
          let content = "";
          elements.forEach((el) => {
            if (options.attr && el.hasAttribute(options.attr)) {
              content += el.getAttribute(options.attr) + "\n";
            } else {
              content += el.textContent + "\n";
            }
          });

          const title =
            document.querySelector("title")?.textContent?.trim() || null;
          return { title, content: content.trim(), isReadability: false };
        }
      } catch (e) {
        // Invalid selector, continue
      }
    }
  }

  // Try Readability unless disabled
  if (!options.no_readability) {
    try {
      const reader = new Readability(document);
      const article = reader.parse();

      if (article?.content) {
        // Create a new DOM from the article content to extract text
        const articleDom = new JSDOM(article.content);
        const content = articleDom.window.document.body.textContent || "";

        return {
          title: article.title || null,
          content: content.trim(),
          isReadability: true,
        };
      }
    } catch (e) {
      // Readability failed, fall through
    }
  }

  // Fallback to body text
  const content = document.body?.textContent || "";
  const title = document.querySelector("title")?.textContent?.trim() || null;

  return { title, content: content.trim(), isReadability: false };
}

function convertToMarkdown(html: string): string {
  const turndown = new TurndownService({
    headingStyle: "atx",
    codeBlockStyle: "fenced",
    bulletListMarker: "-",
  });

  // Add custom rules
  turndown.addRule("dataUri", {
    filter: ["img"],
    replacement: (content, node) => {
      const img = node as HTMLImageElement;
      if (img.src?.startsWith("data:")) {
        return "[Image: data URI]";
      }
      const alt = img.alt || "Image";
      return `![${alt}](${img.src})`;
    },
  });

  let markdown = turndown.turndown(html);

  // Post-process markdown
  // Escape triple backticks
  markdown = markdown.replace(/```+/g, (match) => {
    return "`" + "\u200B".repeat(match.length - 1) + "`";
  });

  // Wrap long lines
  const lines = markdown.split("\n");
  const wrappedLines = lines.map((line) => {
    if (line.length <= 120) {
      return line;
    }

    // Don't wrap code blocks or tables
    if (line.startsWith("    ") || line.includes("|")) {
      return line;
    }

    const words = line.split(" ");
    const wrapped: Array<string> = [];
    let current = "";

    for (const word of words) {
      if ((current + " " + word).length > 120) {
        wrapped.push(current);
        current = word;
      } else {
        current = current ? current + " " + word : word;
      }
    }

    if (current) {
      wrapped.push(current);
    }
    return wrapped.join("\n");
  });

  return wrappedLines.join("\n");
}

function truncateToTokenLimit(
  text: string,
  maxTokens: number,
  encoder: Tiktoken,
): { text: string; truncated: boolean } {
  try {
    const tokens = encoder.encode(text);

    if (tokens.length <= maxTokens) {
      return { text, truncated: false };
    }

    // Truncate at paragraph level
    const paragraphs = text.split(/\n\n+/);
    let result = "";
    let currentTokens = 0;

    for (const paragraph of paragraphs) {
      const paragraphTokens = encoder.encode(paragraph).length;

      if (currentTokens + paragraphTokens > maxTokens) {
        result += "\n\n*(truncated)*";
        break;
      }

      result += (result ? "\n\n" : "") + paragraph;
      currentTokens += paragraphTokens;
    }

    return { text: result, truncated: true };
  } catch {
    // Fallback to character-based truncation
    const estimatedMaxChars = maxTokens * 4;
    if (text.length <= estimatedMaxChars) {
      return { text, truncated: false };
    }

    return {
      text: text.substring(0, estimatedMaxChars) + "\n\n*(truncated)*",
      truncated: true,
    };
  }
}

export async function scrapeWebpage(
  options: ScrapeOptions,
): Promise<ScrapeResult> {
  const startTime = Date.now();
  const model = options.model || "gpt-4";

  try {
    // Normalize and validate URL
    const normalizedUrl = normalizeUrl(options.url);

    // Fetch the webpage
    const { body, finalUrl } = await fetchWithSafety(normalizedUrl);

    // Prepare HTML excerpt if requested
    const htmlExcerpt = options.include_html_excerpt
      ? body.substring(0, 2048)
      : undefined;

    // Sanitize HTML
    const sanitizedHtml = sanitizeAndPrepareHtml(body);

    // Create DOM
    const dom = new JSDOM(sanitizedHtml, {
      url: finalUrl,
      runScripts: "outside-only",
      resources: "usable",
    });

    // Check DOM size
    const nodeCount = dom.window.document.querySelectorAll("*").length;
    if (nodeCount > MAX_DOM_NODES) {
      dom.window.close();
      throw new Error("DOM too large");
    }

    // Extract content
    const { title, content, isReadability } = extractContent(dom, options);

    // Get HTML for markdown conversion
    let htmlForMarkdown: string;
    if (isReadability) {
      // Re-run Readability to get HTML content
      const reader = new Readability(dom.window.document);
      const article = reader.parse();
      htmlForMarkdown = article?.content || sanitizedHtml;
    } else if (options.selector) {
      // Get HTML from selected elements
      const selectors = Array.isArray(options.selector)
        ? options.selector
        : [options.selector];
      htmlForMarkdown = "";
      for (const selector of selectors) {
        const elements = dom.window.document.querySelectorAll(selector);
        elements.forEach((el) => {
          htmlForMarkdown += el.innerHTML + "\n";
        });
      }
    } else {
      htmlForMarkdown = dom.window.document.body.innerHTML;
    }

    // Clean up DOM
    dom.window.close();

    // Convert to markdown
    const markdown = convertToMarkdown(htmlForMarkdown);

    // Get token encoder
    const encoder = getEncoder(model);

    // Determine max tokens
    const providerLimits: Record<string, number> = {
      "gpt-4": 128000,
      "gpt-3.5-turbo": 16000,
      "claude-3": 200000,
    };

    const providerLimit = providerLimits[model] || 100000;
    const maxTokens = Math.min(
      options.truncate_tokens || Infinity,
      Math.floor(providerLimit * 0.8),
    );

    // Truncate if needed
    const { text: truncatedMarkdown, truncated: markdownTruncated } =
      truncateToTokenLimit(markdown, maxTokens, encoder);
    const { text: truncatedText, truncated: textTruncated } =
      truncateToTokenLimit(content, maxTokens, encoder);

    // Count final tokens
    const tokenCount = encoder.encode(truncatedMarkdown).length;

    return {
      url: finalUrl,
      title,
      markdown: truncatedMarkdown,
      text: truncatedText,
      html_excerpt: htmlExcerpt,
      token_count: tokenCount,
      meta: {
        model,
        truncated: markdownTruncated || textTruncated,
        fetched_at: new Date().toISOString(),
        fetch_ms: Date.now() - startTime,
      },
    };
  } catch (error: unknown) {
    const errorObj = error instanceof Error ? error : new Error(String(error));
    let code = "UNKNOWN_ERROR";
    let message = errorObj.message || "Unknown error occurred";

    if (errorObj instanceof ConcurrencyError) {
      code = "CONCURRENCY_LIMIT";
      message = "Too many concurrent requests. Please try again later.";
    } else if (errorObj.name === "AbortError") {
      code = "TIMEOUT";
      message = "Request timed out";
    } else if (message.includes("HTTP")) {
      code = "HTTP_ERROR";
    } else if (message.includes("too large")) {
      code = "SIZE_LIMIT";
    } else if (message.includes("not HTML")) {
      code = "NOT_HTML";
    } else if (message.includes("Invalid URL")) {
      code = "INVALID_URL";
    }

    return {
      url: options.url,
      title: null,
      markdown: "",
      text: "",
      token_count: 0,
      meta: {
        model: options.model || "gpt-4",
        truncated: false,
        fetched_at: new Date().toISOString(),
        fetch_ms: Date.now() - startTime,
      },
      error: {
        code,
        message,
      },
    };
  }
}

// Cleanup function for encoder cache
export function cleanupEncoders(): void {
  encoderCache.forEach((encoder) => encoder.free());
  encoderCache.clear();
}
