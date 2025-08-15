use codex_core::UrlCitation;
use ratatui::text::Line;

/// Renders text with URL citations as inline annotations
pub struct CitationRenderer {
    citations: Vec<UrlCitation>,
}

impl CitationRenderer {
    pub fn new(citations: Vec<UrlCitation>) -> Self {
        Self { citations }
    }

    /// Render text with citations as inline annotations and append citation list
    pub fn render_text_with_citations(&self, text: &str) -> String {
        if self.citations.is_empty() {
            return text.to_string();
        }

        let mut result = String::new();
        let mut last_index = 0;

        // Sort citations by start_index to process them in order
        let mut sorted_citations = self.citations.clone();
        sorted_citations.sort_by_key(|c| c.start_index);

        for (idx, citation) in sorted_citations.iter().enumerate() {
            // Add text before citation
            if citation.start_index > last_index {
                result.push_str(&text[last_index..citation.start_index]);
            }

            // Add citation with inline reference number
            let citation_text = &text[citation.start_index..citation.end_index];
            result.push_str(&format!("{}[{}]", citation_text, idx + 1));

            last_index = citation.end_index;
        }

        // Add remaining text
        if last_index < text.len() {
            result.push_str(&text[last_index..]);
        }

        // Add citation list at the end
        result.push_str("\n\n**Citations:**\n");
        for (idx, citation) in sorted_citations.iter().enumerate() {
            let title = citation.title.as_deref().unwrap_or("Untitled");
            result.push_str(&format!("[{}] [{}]({})\n", idx + 1, title, citation.url));
        }

        result
    }

    /// Render citations for TUI with styled text (for future use with ratatui)
    pub fn render_citations_as_lines(&self, text: &str) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if self.citations.is_empty() {
            lines.push(Line::from(text.to_string()));
            return lines;
        }

        // For now, just convert the rendered text to lines
        // This can be enhanced later with proper styled text
        let rendered = self.render_text_with_citations(text);
        for line in rendered.lines() {
            lines.push(Line::from(line.to_string()));
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_citation_rendering() {
        let citations = vec![UrlCitation {
            citation_type: "url_citation".to_string(),
            start_index: 10,
            end_index: 20,
            url: "https://example.com".to_string(),
            title: Some("Example".to_string()),
        }];

        let renderer = CitationRenderer::new(citations);
        let result = renderer.render_text_with_citations("Some text example text here");

        assert!(result.contains("[1]"));
        assert!(result.contains("Citations:"));
        assert!(result.contains("Example"));
        assert!(result.contains("https://example.com"));
    }

    #[test]
    fn test_empty_citations() {
        let renderer = CitationRenderer::new(vec![]);
        let result = renderer.render_text_with_citations("Some text here");

        assert_eq!(result, "Some text here");
    }

    #[test]
    fn test_multiple_citations() {
        let citations = vec![
            UrlCitation {
                citation_type: "url_citation".to_string(),
                start_index: 0,
                end_index: 4,
                url: "https://example1.com".to_string(),
                title: Some("First".to_string()),
            },
            UrlCitation {
                citation_type: "url_citation".to_string(),
                start_index: 10,
                end_index: 14,
                url: "https://example2.com".to_string(),
                title: Some("Second".to_string()),
            },
        ];

        let renderer = CitationRenderer::new(citations);
        let result = renderer.render_text_with_citations("Some text here now");

        assert!(result.contains("[1]"));
        assert!(result.contains("[2]"));
        assert!(result.contains("First"));
        assert!(result.contains("Second"));
    }
}
