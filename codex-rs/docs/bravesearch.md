
<think>The user wants me to create a comprehensive quick reference guide specifically for LLMs to use when configuring MCP servers with the Brave Search API. This should include all the important schemas, response types, etc.

Looking at the notes provided, I have comprehensive information about:
1. Brave Search API endpoints and authentication
2. Query parameters for both Web Search and Local Search APIs
3. Request headers
4. Response headers (rate limiting)
5. Response objects and models (extensive list)
6. MCP Client Tutorial information
7. Model Context Protocol overview

I should create a guide that:
- Is specifically tailored for LLMs to understand and use
- Includes complete schemas and response types
- Shows how to configure MCP servers with Brave Search
- Provides practical examples
- Is structured as a quick reference

The guide should combine the Brave Search API information with the MCP configuration details from the MCP Client Tutorial note.undefined</think># MCP + Brave Search API: Quick Reference for LLMs

*Comprehensive guide for configuring Model Context Protocol servers with Brave Search API*

---

## 🚀 Quick Setup

### MCP Server Configuration
```json
{
  "mcpServers": {
    "brave-search": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"],
      "env": {
        "BRAVE_API_KEY": "your_brave_api_key_here"
      }
    }
  }
}
```

### Environment Setup
```bash
# Get API key from: https://api-dashboard.search.brave.com/app/keys
export BRAVE_API_KEY="your_api_key"
export ANTHROPIC_API_KEY="your_claude_key"  # or other LLM provider
```

---

## 📡 API Endpoints & Authentication

### Primary Endpoints
```
Web Search:    https://api.search.brave.com/res/v1/web/search
Local POIs:    https://api.search.brave.com/res/v1/local/pois
Descriptions:  https://api.search.brave.com/res/v1/local/descriptions
Summarizer:    https://api.search.brave.com/res/v1/summarizer/search
```

### Authentication Header
```
X-Subscription-Token: <YOUR_API_KEY>
```

---

## 🔧 MCP Tool Functions Available

When configured, the MCP server exposes these tools to LLMs:

### `brave_web_search`
- **Purpose**: General web search with comprehensive results
- **Parameters**:
  - `query` (required): Search string
  - `count`: Results per page (1-20, default 20)
  - `result_filter`: Comma-separated types (web,news,videos,locations,etc.)
  - `country`: 2-letter country code (default "US")
  - `safesearch`: "off"|"moderate"|"strict"

### `brave_local_search`
- **Purpose**: Get detailed location information
- **Parameters**:
  - `ids`: Array of location IDs (from web search results)
  - `max_results`: Up to 20 per request

---

## 📊 Complete Response Schema Reference

### WebSearchApiResponse
```typescript
interface WebSearchApiResponse {
  type: "search"
  query?: Query                    // Query processing info
  web?: Search                     // Web search results
  news?: News                      // News articles
  videos?: Videos                  // Video content
  locations?: Locations            // Local businesses/places
  infobox?: GraphInfobox          // Entity information
  discussions?: Discussions        // Forum posts
  faq?: FAQ                       // Q&A pairs
  mixed?: MixedResponse           // Result ranking
  summarizer?: Summarizer         // Summary key
  rich?: RichCallbackInfo         // Rich content hints
}
```

### Core Result Types

#### Search Results (Web)
```typescript
interface Search {
  type: "search"
  results: SearchResult[]
  family_friendly: boolean
}

interface SearchResult {
  type: "search_result"
  title: string
  url: string
  description?: string
  age?: string                    // ISO date string
  language: string
  meta_url?: MetaUrl
  thumbnail?: Thumbnail
  extra_snippets?: string[]       // Additional excerpts
  // Rich data (when available)
  video?: VideoData
  product?: Product
  article?: Article
  rating?: Rating
  recipe?: Recipe
  // ... many other specialized types
}
```

#### News Results
```typescript
interface News {
  type: "news"
  results: NewsResult[]
  mutated_by_goggles?: boolean
}

interface NewsResult {
  title: string
  url: string
  description?: string
  source?: string
  breaking: boolean
  is_live: boolean
  age?: string
  thumbnail?: Thumbnail
  meta_url?: MetaUrl
}
```

#### Location Results
```typescript
interface Locations {
  type: "locations"
  results: LocationResult[]
}

interface LocationResult {
  type: "location_result"
  id?: string                     // Temp ID for Local API (8hr validity)
  title: string
  url: string
  coordinates?: [number, number]  // [lat, lng]
  postal_address?: PostalAddress
  opening_hours?: OpeningHours
  contact?: Contact
  price_range?: string
  rating?: Rating
  distance?: Unit
  serves_cuisine?: string[]
  categories?: string[]
  timezone?: string              // IANA timezone
}
```

#### Video Results
```typescript
interface Videos {
  type: "videos"
  results: VideoResult[]
}

interface VideoResult {
  type: "video_result"
  title: string
  url: string
  video: VideoData
  thumbnail?: Thumbnail
  age?: string
}

interface VideoData {
  duration?: string              // "HH:MM:SS" or "MM:SS"
  views?: string
  creator?: string
  publisher?: string
  thumbnail?: Thumbnail
  tags?: string[]
  requires_subscription?: boolean
}
```

### Specialized Data Types

#### Infobox (Entity Information)
```typescript
interface GraphInfobox {
  type: "graph"
  results: GenericInfobox | QAInfobox | InfoboxPlace | InfoboxWithLocation | EntityInfobox
}

interface GenericInfobox {
  type: "infobox"
  subtype: "generic"
  title: string
  description?: string
  thumbnail?: Thumbnail
  attributes?: string[][]         // [["Key", "Value"], ...]
  website_url?: string
  ratings?: Rating[]
}
```

#### FAQ Results
```typescript
interface FAQ {
  type: "faq"
  results: QA[]
}

interface QA {
  question: string
  answer: string
  title: string
  url: string
  meta_url?: MetaUrl
}
```

#### Discussions (Forums)
```typescript
interface Discussions {
  type: "search"
  results: DiscussionResult[]
}

interface DiscussionResult {
  type: "discussion"
  title: string
  url: string
  data?: ForumData
}

interface ForumData {
  forum_name: string
  num_answers?: number
  score?: string
  title?: string
  question?: string
  top_comment?: string
}
```

### Supporting Data Types

#### Address & Contact
```typescript
interface PostalAddress {
  type: "PostalAddress"
  streetAddress?: string
  addressLocality?: string
  addressRegion?: string
  postalCode?: string
  country?: string
  displayAddress: string
}

interface Contact {
  email?: string
  telephone?: string
}

interface OpeningHours {
  current_day?: DayOpeningHours[]
  days?: DayOpeningHours[][]
}

interface DayOpeningHours {
  abbr_name: string              // "Mon", "Tue", etc.
  full_name: string              // "Monday", "Tuesday", etc.
  opens: string                  // "09:00" (24hr format)
  closes: string                 // "17:00"
}
```

#### Media & Metadata
```typescript
interface Thumbnail {
  src: string                    // Served URL
  original?: string              // Original URL
}

interface MetaUrl {
  scheme: string                 // "https"
  netloc: string                 // "example.com"
  hostname?: string              // "example.com"
  favicon: string                // Favicon URL
  path: string                   // "/path/to/page"
}

interface Rating {
  ratingValue: number
  bestRating: number
  reviewCount?: number
  profile?: Profile
  is_tripadvisor?: boolean
}
```

#### Rich Content Types
```typescript
interface Product {
  type: "Product"
  name: string
  category?: string
  price: string
  thumbnail: Thumbnail
  description?: string
  rating?: Rating
  offers?: Offer[]
}

interface Recipe {
  title: string
  description: string
  thumbnail: Thumbnail
  url: string
  domain: string
  time?: string                  // "30 minutes"
  prep_time?: string
  cook_time?: string
  ingredients?: string
  instructions?: HowTo[]
  servings?: number
  calories?: number
  rating?: Rating
}

interface Article {
  author?: Person[]
  date?: string                  // Publication date
  publisher?: Organization
  thumbnail?: Thumbnail
  isAccessibleForFree?: boolean
}
```

### Query Processing Info
```typescript
interface Query {
  original: string               // User's original query
  altered?: string               // Spell-corrected query
  safesearch?: boolean
  is_navigational?: boolean      // Query for specific site
  is_geolocal?: boolean         // Location-relevant query
  is_trending?: boolean
  is_news_breaking?: boolean
  country?: string
  language?: Language
  lat?: string                   // Detected latitude
  long?: string                  // Detected longitude
  city?: string                  // Detected city
  state?: string                 // Detected state
}
```

---

## 🎛️ Query Parameters Reference

### Web Search Parameters
```typescript
interface WebSearchParams {
  // Required
  query: string                 // Search query (max 400 chars, 50 words)

  // Localization
  country?: string               // "US", "UK", etc. (default: "US")
  search_lang?: string           // "en", "es", etc. (default: "en")
  ui_lang?: string              // "en-US", "es-ES", etc. (default: "en-US")

  // Pagination (applies only to web results)
  count?: number                 // 1-20 (default: 10)
  offset?: number               // 0-9 (default: 0)

  // Filtering
  safesearch?: "off" | "moderate" | "strict"  // Default: "moderate"
  freshness?: "pd" | "pw" | "pm" | "py" | "YYYY-MM-DDtoYYYY-MM-DD"
  result_filter?: (
    | 'web'
    | 'news'
    | 'videos'
    | 'locations'
    | 'discussions'
    | 'faq'
    | 'infobox'
    | 'query'
    | 'summarizer'
    | 'rich'
  )[]                            // Default: ['web', 'query']

  // Features
  text_decorations?: boolean     // Include highlighting (default: true)
  spellcheck?: boolean          // Enable spell correction (default: true)
  extra_snippets?: boolean      // Up to 5 additional excerpts
  summary?: boolean             // Enable summarizer key generation

  // Customization
  goggles?: string[]            // Custom re-ranking rules
  units?: "metric" | "imperial" // Measurement units
}
```

### Local Search Parameters
```typescript
// `brave_local_search` re-uses the same parameters as `brave_web_search` and
// expects a regular search query. The tool will automatically detect
// locations from the web-search response and then enrich them with the Local
// API. Therefore its accepted parameters mirror `WebSearchParams` above.
type LocalSearchParams = WebSearchParams;

// (Legacy) Direct POI look-up by ID is no longer exposed at the MCP layer, but
// you can still call the Brave Local API yourself with:
//   GET /res/v1/local/pois?ids=<comma-separated-ids>
```

---

## 📡 Request Headers

### Required Headers
```typescript
{
  "X-Subscription-Token": string // Your API key
}
```

### Recommended Headers
```typescript
{
  "Accept": "application/json",
  "Accept-Encoding": "gzip",
  "User-Agent": string          // Realistic browser UA
}
```

### Location Enhancement Headers
```typescript
{
  "X-Loc-Lat": string,          // "-90.0" to "90.0"
  "X-Loc-Long": string,         // "-180.0" to "180.0"
  "X-Loc-Timezone": string,     // "America/New_York"
  "X-Loc-City": string,         // "San Francisco"
  "X-Loc-State": string,        // "CA"
  "X-Loc-Country": string,      // "US"
  "X-Loc-Postal-Code": string   // "94105"
}
```

---

## 📈 Rate Limiting & Response Headers

### Rate Limit Headers
```typescript
{
  "X-RateLimit-Limit": string,      // "1, 15000" (req/sec, req/month)
  "X-RateLimit-Policy": string,     // "1;w=1, 15000;w=2592000"
  "X-RateLimit-Remaining": string,  // "1, 1000"
  "X-RateLimit-Reset": string       // "1, 1419704" (seconds)
}
```

### Interpretation
- Only successful requests count toward usage
- Monitor remaining quota to avoid 429 errors
- Implement backoff strategies for rate limiting

---

## 🔄 Common Workflows for MCP Integration

### 1. Basic Web Search
```javascript
// LLM calls via MCP
const results = await brave_web_search({
  query: "climate change impacts",
  result_filter: "web,news",
  count: 10
});

// Response includes web.results[] and news.results[]
```

### 2. Local Business Search
```javascript
// Step 1: Find locations
const searchResults = await brave_web_search({
  query: "italian restaurants in San Francisco",
  result_filter: "locations"
});

// Step 2: Get detailed info (if locations found)
if (searchResults.locations?.results?.length > 0) {
  const locationIds = searchResults.locations.results.map(r => r.id);
  const detailedInfo = await brave_local_search({
    ids: locationIds
  });
}
```

### 3. News & Current Events
```javascript
const currentNews = await brave_web_search({
  query: "latest AI developments",
  result_filter: "news,web",
  freshness: "pd",  // Past day
  count: 15
});
```

### 4. Research & FAQ
```javascript
const researchResults = await brave_web_search({
  query: "how does quantum computing work",
  result_filter: "web,faq,infobox",
  extra_snippets: true
});
```

---

## ⚙️ MCP Server Configuration Examples

### Spring AI (Java)
```yaml
spring:
  ai:
    mcp:
      client:
        enabled: true
        name: brave-search-client
        type: SYNC
        stdio:
          servers-configuration: classpath:/mcp-servers-config.json
        toolcallback:
          enabled: true
```

### Node.js/TypeScript
```json
{
  "mcpServers": {
    "brave-search": {
      "command": "node",
      "args": ["./mcp-server-brave.js"],
      "env": {
        "BRAVE_API_KEY": "${BRAVE_API_KEY}"
      }
    }
  }
}
```

### Python
```json
{
  "mcpServers": {
    "brave-search": {
      "command": "python",
      "args": ["-m", "mcp_server_brave"],
      "env": {
        "BRAVE_API_KEY": "${BRAVE_API_KEY}"
      }
    }
  }
}
```

---

## 🚨 Error Handling & Best Practices

### Common HTTP Status Codes
```typescript
{
  400: "Bad Request - Invalid parameters",
  401: "Unauthorized - Invalid API key",
  403: "Forbidden - Insufficient plan/quota",
  429: "Rate Limited - Too many requests",
  500: "Server Error - Retry later"
}
```

### Best Practices for LLMs
1. **Always check for rate limits** in response headers
2. **Use result_filter** to get only needed data types
3. **Cache location IDs** (valid for ~8 hours)
4. **Implement retry logic** with exponential backoff
5. **Use specific queries** for better results
6. **Include location headers** for geo-relevant searches
7. **Batch local requests** (up to 20 IDs per call)

### Error Response Example
```typescript
interface ErrorResponse {
  error: {
    code: number,
    message: string,
    type: string
  }
}
```

---

## 🔍 Advanced Features

### Custom Re-ranking with Goggles
```javascript
const customResults = await brave_web_search({
  query: "programming tutorials",
  goggles: ["!site:pinterest.com", "site:github.com boost:2"]
});
```

### Summarization Workflow
```javascript
// Enable summary in search
const searchResults = await brave_web_search({
  query: "renewable energy trends",
  summary: true
});

// Use summary key if available
if (searchResults.summarizer?.key) {
  const summary = await fetch(`https://api.search.brave.com/res/v1/summarizer/search?key=${searchResults.summarizer.key}`, {
    headers: { "X-Subscription-Token": "API_KEY" }
  });
}
```

---

## 📋 Quick Checklist for MCP Setup

- [ ] Get Brave Search API key from dashboard
- [ ] Install MCP server: `npx @modelcontextprotocol/server-brave-search`
- [ ] Configure environment variables
- [ ] Set up MCP servers configuration JSON
- [ ] Test basic search functionality
- [ ] Implement error handling and rate limiting
- [ ] Configure result filtering for your use case

---

*This guide provides complete schema references and configuration examples for integrating Brave Search API with Model Context Protocol servers. All response types and parameters are included for comprehensive LLM understanding.*

#### Sources:

- [[Brave Search - API]]
- [[Brave Search - API 5]]
- [[Brave Search - API 4]]
- [[Brave Search - API 1]]
- [[Brave Search - API 3]]
- [[Brave Search - API 2]]
- [[Brave Search API Comprehensive Guide]]
- [[MCP Client Tutorial]]
- [[Model Context Protocol Overview and Specification]]  
