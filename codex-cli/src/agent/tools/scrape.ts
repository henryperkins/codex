import type { ScrapeOptions } from "../../utils/web-scraper.js";
import type { FunctionTool } from "openai/resources/beta/assistants.mjs";

// `scrapeWebpage` needs to know the model name so it can use the correct
// tiktoken encoder when counting tokens.  The CLI already tracks the model
// that is active for the current session via the session utilities, so we
// reuse that instead of importing a non-existent helper.
import { getCurrentModel } from "../../utils/session.js";
import { scrapeWebpage } from "../../utils/web-scraper.js";

export const scrapeFunctionTool: FunctionTool = {
  type: "function",
  function: {
    name: "scrape",
    description:
      "Fetch & sanitize a web page, returning Markdown and metadata.",
    parameters: {
      type: "object",
      properties: {
        url: {
          type: "string",
          description: "The URL of the webpage to scrape",
        },
        selector: {
          oneOf: [
            { type: "string" },
            { type: "array", items: { type: "string" } },
          ],
          nullable: true,
          description: "CSS selector(s) to extract specific content",
        },
        attr: {
          type: "string",
          nullable: true,
          description: "Attribute to extract from selected elements",
        },
        truncate_tokens: {
          type: "number",
          nullable: true,
          description: "Maximum number of tokens to return",
        },
      },
      required: ["url"],
      additionalProperties: false,
    },
  },
};

export async function executeScrape(args: {
  url: string;
  selector?: string | Array<string>;
  attr?: string;
  truncate_tokens?: number;
}): Promise<string> {
  try {
    // Determine the model currently in use so that the scraper can pick the
    // matching encoder.  Fallback to a sensible default when the session has
    // not recorded a model yet (e.g. standalone `codex scrape <url>` command
    // in which no chat session is active).
    const currentModel = getCurrentModel() || "gpt-4";

    const options: ScrapeOptions = {
      url: args.url,
      selector: args.selector,
      attr: args.attr,
      truncate_tokens: args.truncate_tokens,
      model: currentModel,
      include_html_excerpt: false, // Don't include in agent responses
    };

    const result = await scrapeWebpage(options);

    // Format result for agent
    if (result.error) {
      return `Error scraping ${result.url}: ${result.error.message}`;
    }

    // Build response string
    let response = `# ${result.title || "Untitled"}\n\n`;
    response += `URL: ${result.url}\n\n`;

    if (result.meta.truncated) {
      response += `*Note: Content was truncated to fit token limit (${result.token_count} tokens)*\n\n`;
    }

    response += result.markdown;

    return response;
  } catch (error: unknown) {
    const errorMessage =
      error instanceof Error ? error.message : "Failed to scrape webpage";
    return `Error: ${errorMessage}`;
  }
}
