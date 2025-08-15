use codex_core::protocol::SearchResult;
use codex_core::protocol::WebSearchResultEvent;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

/// Widget for displaying web search results
pub struct SearchResultsWidget {
    results: Vec<SearchResult>,
    query: String,
    timestamp: String,
}

impl SearchResultsWidget {
    pub fn new(event: WebSearchResultEvent) -> Self {
        Self {
            query: event.query,
            results: event.results,
            timestamp: event.timestamp,
        }
    }

    /// Render the search results widget
    pub fn render(&self, _area: Rect) -> Paragraph {
        let mut lines = Vec::new();

        // Add header
        lines.push(Line::from(vec![
            Span::styled(
                "🔍 Web Search Results: ",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Blue),
            ),
            Span::styled(
                &self.query,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::White),
            ),
        ]));

        lines.push(Line::from(""));

        if self.results.is_empty() {
            lines.push(Line::from(Span::styled(
                "No results found",
                Style::default().fg(Color::Yellow),
            )));
        } else {
            // Add results
            for (idx, result) in self.results.iter().enumerate() {
                // Result number and title
                lines.push(Line::from(vec![
                    Span::styled(format!("{}. ", idx + 1), Style::default().fg(Color::Yellow)),
                    Span::styled(
                        &result.title,
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(Color::White),
                    ),
                ]));

                // Domain and URL
                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(&result.domain, Style::default().fg(Color::Green)),
                    Span::raw(" - "),
                    Span::styled(
                        &result.url,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::UNDERLINED),
                    ),
                ]));

                // Snippet
                if !result.snippet.is_empty() {
                    lines.push(Line::from(vec![
                        Span::raw("   "),
                        Span::styled(&result.snippet, Style::default().fg(Color::Gray)),
                    ]));
                }

                // Relevance score if available
                if let Some(score) = result.relevance_score {
                    lines.push(Line::from(vec![
                        Span::raw("   "),
                        Span::styled(
                            format!("Relevance: {score:.2}"),
                            Style::default().fg(Color::Magenta),
                        ),
                    ]));
                }

                lines.push(Line::from(""));
            }
        }

        // Add footer with timestamp
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("Search completed at: "),
            Span::styled(&self.timestamp, Style::default().fg(Color::Gray)),
        ]));

        Paragraph::new(lines).block(
            Block::default()
                .title(" Web Search Results ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
    }

    /// Get the minimum height needed to display all results
    pub fn min_height(&self) -> u16 {
        let header_lines = 3; // Title + empty line + search query
        let footer_lines = 2; // Empty line + timestamp
        let result_lines = if self.results.is_empty() {
            1 // "No results found"
        } else {
            self.results
                .iter()
                .map(|result| {
                    let mut lines = 3; // Title, domain+URL, empty line
                    if !result.snippet.is_empty() {
                        lines += 1; // Snippet
                    }
                    if result.relevance_score.is_some() {
                        lines += 1; // Relevance score
                    }
                    lines
                })
                .sum::<usize>() as u16
        };

        header_lines + result_lines + footer_lines + 2 // +2 for border
    }
}

/// Format search results for CLI output (non-TUI mode)
pub fn format_search_results_for_cli(event: &WebSearchResultEvent) -> String {
    let mut output = String::new();

    output.push_str(&"=".repeat(80));
    output.push('\n');
    output.push_str(&format!("🔍 Web Search Results for: {}\n", event.query));
    output.push_str(&"=".repeat(80));
    output.push('\n');

    if event.results.is_empty() {
        output.push_str("\nNo results found.\n");
    } else {
        for (idx, result) in event.results.iter().enumerate() {
            output.push('\n');
            output.push_str(&format!("{}. {}\n", idx + 1, result.title));
            output.push_str(&format!("   📍 {}\n", result.domain));
            output.push_str(&format!("   🔗 {}\n", result.url));

            if !result.snippet.is_empty() {
                output.push_str(&format!("   {}\n", result.snippet));
            }

            if let Some(score) = result.relevance_score {
                output.push_str(&format!("   📊 Relevance: {score:.2}\n"));
            }
        }
    }

    output.push('\n');
    output.push_str(&"-".repeat(80));
    output.push('\n');
    output.push_str(&format!("Search completed at: {}\n", event.timestamp));
    output.push_str(&"=".repeat(80));
    output.push('\n');

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event() -> WebSearchResultEvent {
        WebSearchResultEvent {
            query: "test query".to_string(),
            results: vec![
                SearchResult {
                    title: "Test Result 1".to_string(),
                    url: "https://example1.com".to_string(),
                    snippet: "This is a test snippet for result 1".to_string(),
                    domain: "example1.com".to_string(),
                    relevance_score: Some(0.95),
                },
                SearchResult {
                    title: "Test Result 2".to_string(),
                    url: "https://example2.com".to_string(),
                    snippet: "This is a test snippet for result 2".to_string(),
                    domain: "example2.com".to_string(),
                    relevance_score: None,
                },
            ],
            timestamp: "2024-01-15T10:30:00Z".to_string(),
        }
    }

    #[test]
    fn test_search_results_widget_creation() {
        let event = create_test_event();
        let widget = SearchResultsWidget::new(event);

        assert_eq!(widget.query, "test query");
        assert_eq!(widget.results.len(), 2);
        assert_eq!(widget.timestamp, "2024-01-15T10:30:00Z");
    }

    #[test]
    fn test_min_height_calculation() {
        let event = create_test_event();
        let widget = SearchResultsWidget::new(event);

        let height = widget.min_height();
        assert!(height > 10); // Should need reasonable height for 2 results
    }

    #[test]
    fn test_cli_formatting() {
        let event = create_test_event();
        let formatted = format_search_results_for_cli(&event);

        assert!(formatted.contains("🔍 Web Search Results for: test query"));
        assert!(formatted.contains("Test Result 1"));
        assert!(formatted.contains("Test Result 2"));
        assert!(formatted.contains("example1.com"));
        assert!(formatted.contains("Relevance: 0.95"));
    }

    #[test]
    fn test_empty_results() {
        let empty_event = WebSearchResultEvent {
            query: "empty query".to_string(),
            results: vec![],
            timestamp: "2024-01-15T10:30:00Z".to_string(),
        };

        let formatted = format_search_results_for_cli(&empty_event);
        assert!(formatted.contains("No results found"));
    }
}
