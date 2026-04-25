use serde::{Deserialize, Serialize};

/// KB search result from Tantivy index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbSearchResult {
    /// Relevance score (BM25)
    pub score: f32,
    /// Document URL
    pub url: String,
    /// Document title
    pub title: String,
    /// Document filepath (within KB ZIP)
    pub filepath: String,
    /// Summary/snippet
    pub summary: String,
    /// Keywords/tags
    pub keywords: String,
    /// Relative URL (if different from url)
    pub relative_url: Option<String>,
}

impl KbSearchResult {
    /// Create a new search result
    pub fn new(
        score: f32,
        url: String,
        title: String,
        filepath: String,
        summary: String,
        keywords: String,
    ) -> Self {
        Self {
            score,
            url,
            title,
            filepath,
            summary,
            keywords,
            relative_url: None,
        }
    }

    /// Get the display URL (prefer relative_url if available)
    pub fn display_url(&self) -> &str {
        self.relative_url.as_deref().unwrap_or(&self.url)
    }

    /// Format for display
    pub fn format_display(&self) -> String {
        format!(
            "[{:.2}] {} - {}",
            self.score,
            self.title,
            self.display_url()
        )
    }
}

/// KB fetch result (for fetching specific document)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbFetchResult {
    /// Document URL
    pub url: String,
    /// Document title
    pub title: String,
    /// Full content
    pub content: String,
    /// Keywords/tags
    pub keywords: String,
    /// Source filepath
    pub filepath: String,
}

impl KbFetchResult {
    /// Create a new fetch result
    pub fn new(
        url: String,
        title: String,
        content: String,
        keywords: String,
        filepath: String,
    ) -> Self {
        Self {
            url,
            title,
            content,
            keywords,
            filepath,
        }
    }

    /// Get content length
    pub fn content_length(&self) -> usize {
        self.content.len()
    }

    /// Get word count estimate
    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }

    /// Format summary (first N characters)
    pub fn format_summary(&self, max_chars: usize) -> String {
        if self.content.len() <= max_chars {
            self.content.clone()
        } else {
            format!("{}...", &self.content[..max_chars])
        }
    }
}

/// KB search query options
#[derive(Debug, Clone)]
pub struct KbSearchOptions {
    /// Maximum results to return
    pub limit: usize,
    /// Boost factor for title field
    pub title_boost: f32,
    /// Boost factor for content field
    pub content_boost: f32,
    /// Boost factor for keywords field
    pub keywords_boost: f32,
    /// Whether to include summaries
    pub include_summary: bool,
    /// Whether to highlight matches (not yet implemented)
    pub highlight: bool,
}

impl Default for KbSearchOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            title_boost: 2.0,
            content_boost: 1.0,
            keywords_boost: 1.5,
            include_summary: true,
            highlight: false,
        }
    }
}

impl KbSearchOptions {
    /// Create options with custom limit
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            ..Default::default()
        }
    }

    /// Create options for precise title search
    pub fn title_search() -> Self {
        Self {
            limit: 5,
            title_boost: 5.0,
            content_boost: 0.5,
            keywords_boost: 1.0,
            include_summary: true,
            highlight: false,
        }
    }

    /// Create options for content search
    pub fn content_search() -> Self {
        Self {
            limit: 20,
            title_boost: 1.0,
            content_boost: 3.0,
            keywords_boost: 1.0,
            include_summary: true,
            highlight: false,
        }
    }
}

/// KB search response (with metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbSearchResponse {
    /// Search results
    pub results: Vec<KbSearchResult>,
    /// Total matching documents (approximate)
    pub total: usize,
    /// Query string
    pub query: String,
    /// Time taken in milliseconds
    pub time_ms: u64,
}

impl KbSearchResponse {
    /// Create a new search response
    pub fn new(
        results: Vec<KbSearchResult>,
        total: usize,
        query: String,
        time_ms: u64,
    ) -> Self {
        Self {
            results,
            total,
            query,
            time_ms,
        }
    }

    /// Check if results are empty
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Get number of results
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Format for display
    pub fn format_display(&self) -> String {
        let mut output = format!("Found {} results for '{}' ({}ms):\n", self.total, self.query, self.time_ms);

        for (i, result) in self.results.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, result.format_display()));
            if !result.summary.is_empty() {
                output.push_str(&format!("   {}\n", result.summary));
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kb_search_result_creation() {
        let result = KbSearchResult::new(
            1.5,
            "https://example.com/doc".to_string(),
            "Test Document".to_string(),
            "docs/test.md".to_string(),
            "This is a test document".to_string(),
            "test, example".to_string(),
        );

        assert_eq!(result.score, 1.5);
        assert_eq!(result.title, "Test Document");
        assert_eq!(result.display_url(), "https://example.com/doc");
    }

    #[test]
    fn test_kb_search_result_with_relative_url() {
        let mut result = KbSearchResult::new(
            1.0,
            "https://example.com/doc".to_string(),
            "Test".to_string(),
            "test.md".to_string(),
            "Summary".to_string(),
            "test".to_string(),
        );
        result.relative_url = Some("/docs/test".to_string());

        assert_eq!(result.display_url(), "/docs/test");
    }

    #[test]
    fn test_kb_search_result_format() {
        let result = KbSearchResult::new(
            2.5,
            "/test/url".to_string(),
            "Android SDK".to_string(),
            "sdk.md".to_string(),
            "Android SDK documentation".to_string(),
            "android, sdk".to_string(),
        );

        let formatted = result.format_display();
        assert!(formatted.contains("Android SDK"));
        assert!(formatted.contains("/test/url"));
    }

    #[test]
    fn test_kb_fetch_result_creation() {
        let result = KbFetchResult::new(
            "/test".to_string(),
            "Test Doc".to_string(),
            "This is the full content of the document.".to_string(),
            "test, doc".to_string(),
            "docs/test.md".to_string(),
        );

        assert_eq!(result.url, "/test");
        assert_eq!(result.content_length(), 41);  // "This is the full content of the document." is 41 chars
        assert_eq!(result.word_count(), 8);
    }

    #[test]
    fn test_kb_fetch_result_summary() {
        let result = KbFetchResult::new(
            "/test".to_string(),
            "Test".to_string(),
            "This is a long content that needs to be summarized.".to_string(),
            "test".to_string(),
            "test.md".to_string(),
        );

        let summary = result.format_summary(20);
        assert!(summary.ends_with("..."));
        assert!(summary.len() <= 23); // 20 chars + "..."
    }

    #[test]
    fn test_kb_search_options_default() {
        let opts = KbSearchOptions::default();
        assert_eq!(opts.limit, 10);
        assert_eq!(opts.title_boost, 2.0);
        assert_eq!(opts.content_boost, 1.0);
    }

    #[test]
    fn test_kb_search_options_with_limit() {
        let opts = KbSearchOptions::with_limit(50);
        assert_eq!(opts.limit, 50);
        assert_eq!(opts.title_boost, 2.0); // Should keep default boost
    }

    #[test]
    fn test_kb_search_options_title_search() {
        let opts = KbSearchOptions::title_search();
        assert_eq!(opts.limit, 5);
        assert_eq!(opts.title_boost, 5.0);
        assert_eq!(opts.content_boost, 0.5);
    }

    #[test]
    fn test_kb_search_options_content_search() {
        let opts = KbSearchOptions::content_search();
        assert_eq!(opts.limit, 20);
        assert_eq!(opts.content_boost, 3.0);
    }

    #[test]
    fn test_kb_search_response_creation() {
        let results = vec![
            KbSearchResult::new(1.0, "/a".to_string(), "A".to_string(), "a.md".to_string(), "summary a".to_string(), "a".to_string()),
            KbSearchResult::new(0.5, "/b".to_string(), "B".to_string(), "b.md".to_string(), "summary b".to_string(), "b".to_string()),
        ];

        let response = KbSearchResponse::new(results, 2, "test".to_string(), 50);

        assert_eq!(response.len(), 2);
        assert!(!response.is_empty());
        assert_eq!(response.total, 2);
        assert_eq!(response.time_ms, 50);
    }

    #[test]
    fn test_kb_search_response_empty() {
        let response = KbSearchResponse::new(vec![], 0, "nonexistent".to_string(), 10);

        assert!(response.is_empty());
        assert_eq!(response.len(), 0);
    }

    #[test]
    fn test_kb_search_response_format() {
        let results = vec![
            KbSearchResult::new(1.5, "/doc".to_string(), "Document".to_string(), "doc.md".to_string(), "Summary text".to_string(), "doc".to_string()),
        ];

        let response = KbSearchResponse::new(results, 1, "query".to_string(), 25);

        let formatted = response.format_display();
        assert!(formatted.contains("Found 1 results"));
        assert!(formatted.contains("query"));
        assert!(formatted.contains("25ms"));
        assert!(formatted.contains("Document"));
    }
}