# Web Search Implementation Plan for Codex-rs

## 1. Search-Specific Model Support

### Current Gap
The system doesn't recognize or handle `gpt-4o-search-preview` or `gpt-4o-mini-search-preview` models which have special requirements.

### Implementation

```rust
// In codex-rs/core/src/model_family.rs (or similar location)

pub fn is_search_preview_model(model_slug: &str) -> bool {
    model_slug.contains("search-preview")
}

pub fn get_search_model_config(model_slug: &str) -> SearchModelConfig {
    if model_slug.contains("gpt-4o-search-preview") {
        SearchModelConfig {
            supports_deep_research: true,
            max_search_queries: 10,
            requires_web_search: true,
            context_window: 128_000,
        }
    } else if model_slug.contains("gpt-4o-mini-search-preview") {
        SearchModelConfig {
            supports_deep_research: false,
            max_search_queries: 5,
            requires_web_search: true,
            context_window: 128_000,
        }
    } else {
        SearchModelConfig::default()
    }
}
```

## 2. Context Window Enforcement

### Current Gap
No enforcement of the 128,000 token limit when web search is enabled, even for models with larger windows.

### Implementation

```rust
// In codex-rs/core/src/codex_conversation.rs

impl CodexConversation {
    pub fn get_effective_context_window(&self) -> usize {
        let base_window = self.model_family.context_window;
        
        // When web search is enabled, cap at 128k tokens
        if self.tools_config.web_search {
            std::cmp::min(base_window, 128_000)
        } else {
            base_window
        }
    }
    
    pub fn validate_context_usage(&self, current_tokens: usize) -> Result<(), CodexErr> {
        let effective_window = self.get_effective_context_window();
        
        if current_tokens > effective_window {
            return Err(CodexErr::ContextWindowExceeded {
                used: current_tokens,
                limit: effective_window,
                web_search_limited: self.tools_config.web_search,
            });
        }
        
        Ok(())
    }
}
```

## 3. Citation Rendering in TUI

### Current Gap
Citations are passed through but not rendered with clickable links or proper formatting.

### Implementation

```rust
// In codex-rs/tui/src/citation_renderer.rs (new file)

use crate::models::UrlCitation;
use ratatui::text::{Line, Span};
use ratatui::style::{Color, Modifier, Style};

pub struct CitationRenderer {
    citations: Vec<UrlCitation>,
}

impl CitationRenderer {
    pub fn new(citations: Vec<UrlCitation>) -> Self {
        Self { citations }
    }
    
    pub fn render_text_with_citations(&self, text: &str) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut current_line = Vec::new();
        let mut last_index = 0;
        
        // Sort citations by start_index
        let mut sorted_citations = self.citations.clone();
        sorted_citations.sort_by_key(|c| c.start_index);
        
        for (idx, citation) in sorted_citations.iter().enumerate() {
            // Add text before citation
            if citation.start_index > last_index {
                current_line.push(Span::raw(&text[last_index..citation.start_index]));
            }
            
            // Add citation with styling
            let citation_text = &text[citation.start_index..citation.end_index];
            let citation_span = Span::styled(
                format!("{}[{}]", citation_text, idx + 1),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED),
            );
            current_line.push(citation_span);
            
            last_index = citation.end_index;
        }
        
        // Add remaining text
        if last_index < text.len() {
            current_line.push(Span::raw(&text[last_index..]));
        }
        
        lines.push(Line::from(current_line));
        
        // Add citation list at the end
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Citations:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        
        for (idx, citation) in sorted_citations.iter().enumerate() {
            let citation_line = format!(
                "[{}] {} - {}",
                idx + 1,
                citation.title.as_ref().unwrap_or(&"Untitled".to_string()),
                citation.url
            );
            lines.push(Line::from(Span::styled(
                citation_line,
                Style::default().fg(Color::Gray),
            )));
        }
        
        lines
    }
}

// In codex-rs/tui/src/chatwidget.rs - integrate citation rendering
impl ChatWidget {
    fn render_agent_message(&mut self, msg: &AgentMessageEvent) {
        if let Some(citations) = &msg.citations {
            let renderer = CitationRenderer::new(citations.clone());
            let rendered_lines = renderer.render_text_with_citations(&msg.message);
            // Add rendered lines to the chat display
            for line in rendered_lines {
                self.add_line(line);
            }
        } else {
            // Normal message rendering
            self.add_text(&msg.message);
        }
    }
}
```

## 4. Cost Tracking

### Current Gap
No tracking of web search tool costs or token usage specific to search operations.

### Implementation

```rust
// In codex-rs/core/src/protocol.rs - extend TokenUsage

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: Option<u64>,
    pub output_tokens: u64,
    pub reasoning_output_tokens: Option<u64>,
    pub total_tokens: u64,
    
    // New fields for web search
    pub web_search_tokens: Option<u64>,
    pub web_search_queries: Option<u32>,
    pub web_search_cost: Option<f64>,
}

impl TokenUsage {
    pub fn calculate_web_search_cost(&mut self, model: &str) {
        if let Some(search_tokens) = self.web_search_tokens {
            // Pricing per 1M tokens (example rates)
            let rate = match model {
                m if m.contains("gpt-4o-search") => 2.50,
                m if m.contains("gpt-4o-mini-search") => 0.60,
                _ => 0.0,
            };
            
            self.web_search_cost = Some((search_tokens as f64 / 1_000_000.0) * rate);
        }
    }
}

// In codex-rs/core/src/client.rs - track search usage
impl ModelClient {
    fn process_web_search_response(&mut self, response: &ResponseItem) {
        if let ResponseItem::WebSearchCall { tokens_used, queries, .. } = response {
            self.usage.web_search_tokens = Some(tokens_used);
            self.usage.web_search_queries = Some(queries);
            self.usage.calculate_web_search_cost(&self.model);
        }
    }
}
```

## 5. Search Result Display

### Current Gap
No dedicated UI components for displaying search results with proper attribution.

### Implementation

```rust
// In codex-rs/core/src/protocol.rs - add search result event

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebSearchResultEvent {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub domain: String,
    pub relevance_score: f32,
}

// In codex-rs/tui/src/search_results_widget.rs (new file)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub struct SearchResultsWidget {
    results: Vec<SearchResult>,
    query: String,
}

impl SearchResultsWidget {
    pub fn new(query: String, results: Vec<SearchResult>) -> Self {
        Self { query, results }
    }
    
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(format!(" Web Search: {} ", self.query))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));
        
        let items: Vec<ListItem> = self.results
            .iter()
            .enumerate()
            .map(|(idx, result)| {
                let content = vec![
                    Line::from(vec![
                        Span::styled(
                            format!("{}. ", idx + 1),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(
                            &result.title,
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(Color::White),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(
                            &result.domain,
                            Style::default().fg(Color::Green),
                        ),
                        Span::raw(" - "),
                        Span::styled(
                            &result.url,
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(
                            &result.snippet,
                            Style::default().fg(Color::Gray),
                        ),
                    ]),
                    Line::from(""),
                ];
                
                ListItem::new(content)
            })
            .collect();
        
        let list = List::new(items)
            .block(block)
            .style(Style::default());
        
        frame.render_widget(list, area);
    }
}

// In codex-rs/exec/src/event_processor_with_human_output.rs
// Add handling for search results display

impl EventProcessor {
    fn handle_web_search_result(&mut self, event: WebSearchResultEvent) {
        // Format search results for CLI output
        println!("\n{}", "=".repeat(80));
        println!("🔍 Web Search Results for: {}", event.query);
        println!("{}", "=".repeat(80));
        
        for (idx, result) in event.results.iter().enumerate() {
            println!("\n{}. {}", idx + 1, result.title);
            println!("   📍 {}", result.domain);
            println!("   🔗 {}", result.url);
            println!("   {}", result.snippet);
        }
        
        println!("\n{}", "-".repeat(80));
        println!("Search completed at: {}", event.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
        println!("{}", "=".repeat(80));
    }
}
```

## 6. Rate Limiting

### Implementation

```rust
// In codex-rs/core/src/client.rs

use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct WebSearchRateLimiter {
    last_request: Mutex<Option<Instant>>,
    min_interval: Duration,
    requests_per_minute: u32,
    request_count: Mutex<u32>,
    window_start: Mutex<Instant>,
}

impl WebSearchRateLimiter {
    pub fn new(model_tier: &str) -> Self {
        let (rpm, min_interval_ms) = match model_tier {
            "tier-1" => (500, 120),  // 500 req/min, 120ms between
            "tier-2" => (5000, 12),  // 5000 req/min, 12ms between
            _ => (100, 600),         // Default conservative
        };
        
        Self {
            last_request: Mutex::new(None),
            min_interval: Duration::from_millis(min_interval_ms),
            requests_per_minute: rpm,
            request_count: Mutex::new(0),
            window_start: Mutex::new(Instant::now()),
        }
    }
    
    pub async fn wait_if_needed(&self) -> Result<(), CodexErr> {
        let mut last = self.last_request.lock().await;
        
        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.min_interval {
                tokio::time::sleep(self.min_interval - elapsed).await;
            }
        }
        
        // Check requests per minute
        let mut count = self.request_count.lock().await;
        let mut window = self.window_start.lock().await;
        
        if window.elapsed() > Duration::from_secs(60) {
            *count = 0;
            *window = Instant::now();
        }
        
        if *count >= self.requests_per_minute {
            return Err(CodexErr::RateLimitExceeded {
                limit: self.requests_per_minute,
                window: "1 minute".to_string(),
            });
        }
        
        *count += 1;
        *last = Some(Instant::now());
        
        Ok(())
    }
}
```

## 7. Implementation Priority

1. **Phase 1 - Core Functionality** (High Priority)
   - Context window enforcement
   - Search-specific model detection
   - Basic cost tracking

2. **Phase 2 - User Experience** (Medium Priority)
   - Citation rendering in TUI
   - Search result display widget
   - CLI search result formatting

3. **Phase 3 - Advanced Features** (Lower Priority)
   - Rate limiting implementation
   - Search result caching
   - Advanced cost analytics

## Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_search_model_detection() {
        assert!(is_search_preview_model("gpt-4o-search-preview"));
        assert!(is_search_preview_model("gpt-4o-mini-search-preview"));
        assert!(!is_search_preview_model("gpt-4o"));
    }
    
    #[test]
    fn test_context_window_enforcement() {
        let mut conv = create_test_conversation();
        conv.tools_config.web_search = true;
        conv.model_family.context_window = 200_000;
        
        assert_eq!(conv.get_effective_context_window(), 128_000);
    }
    
    #[test]
    fn test_citation_rendering() {
        let citations = vec![
            UrlCitation {
                citation_type: "url_citation".to_string(),
                start_index: 10,
                end_index: 20,
                url: "https://example.com".to_string(),
                title: Some("Example".to_string()),
            },
        ];
        
        let renderer = CitationRenderer::new(citations);
        let lines = renderer.render_text_with_citations("Some text example text here");
        
        assert!(lines.len() > 1);
        assert!(lines.iter().any(|l| l.to_string().contains("[1]")));
    }
}
```

## Migration Path

1. Start with non-breaking changes (detection, tracking)
2. Add new UI components without removing old ones
3. Gradually migrate to new citation rendering
4. Enable context enforcement with config flag first
5. Full rollout after testing period

## Configuration Updates

```toml
# In ~/.codex/config.toml

[web_search]
enabled = true
enforce_context_limit = true  # New: enforce 128k limit
show_search_costs = true      # New: display cost tracking
render_citations = true       # New: enable citation rendering

[web_search.display]
show_attribution = true       # New: always show source
max_results = 10             # New: limit displayed results
highlight_citations = true   # New: highlight cited text
```