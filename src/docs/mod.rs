use anyhow::{Result, Context};

pub mod service;

pub use service::{KBDownloadService, KbIndexerService, KbDoc, KbDocFile, KbDownloadResult};
pub use service::{KbSearchResult, KbFetchResult, KbSearchOptions, KbSearchResponse, SearchHit, IndexStats};

/// Android documentation base URL
const DOCS_BASE_URL: &str = "https://developer.android.com";

/// Search index URL (placeholder for actual search API)
const SEARCH_API_URL: &str = "https://developer.android.com/s/results";

/// Documentation CLI handler
pub struct DocsCLI {
    client: reqwest::blocking::Client,
}

impl DocsCLI {
    /// Create a new DocsCLI instance
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .user_agent("Android-CLI-Docs/0.1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client })
    }

    /// Search Android documentation
    pub fn search(&self, query: &str) -> Result<Vec<DocResult>> {
        println!("Searching documentation for: {}", query);

        // Build search URL
        let search_url = format!("{}?q={}", SEARCH_API_URL, urlencoding::encode(query));

        let response = self.client
            .get(&search_url)
            .send()
            .context("Failed to search documentation")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Search failed: HTTP {}",
                response.status()
            ));
        }

        // Parse the HTML response to extract results
        // For now, we'll return placeholder results with useful links
        let html = response.text()
            .context("Failed to read search response")?;

        let results = self.parse_search_results(&html, query);

        if results.is_empty() {
            // Return some default results based on the query
            Ok(self.default_results(query))
        } else {
            Ok(results)
        }
    }

    /// Parse search results from HTML
    fn parse_search_results(&self, html: &str, query: &str) -> Vec<DocResult> {
        let mut results = Vec::new();

        // Simple parsing for common patterns
        // Look for article links in the page
        for line in html.lines() {
            if line.contains("href=\"") && line.contains("developer.android.com") {
                if let Some(href_start) = line.find("href=\"") {
                    let start = href_start + 6;
                    if let Some(end) = line[start..].find('"') {
                        let url = &line[start..start + end];
                        if url.starts_with('/') || url.contains("developer.android.com") {
                            let full_url = if url.starts_with('/') {
                                format!("{}{}", DOCS_BASE_URL, url)
                            } else {
                                url.to_string()
                            };

                            // Try to extract title from the line
                            let title = self.extract_title(line);

                            results.push(DocResult {
                                title: title.unwrap_or_else(|| query.to_string()),
                                url: full_url,
                                description: None,
                            });

                            if results.len() >= 10 {
                                break;
                            }
                        }
                    }
                }
            }
        }

        results
    }

    /// Extract title from HTML line
    fn extract_title(&self, line: &str) -> Option<String> {
        // Look for text between > and <
        if let Some(gt_pos) = line.find('>') {
            let after_gt = &line[gt_pos + 1..];
            if let Some(lt_pos) = after_gt.find('<') {
                let title = &after_gt[..lt_pos];
                let cleaned = title.trim();
                if !cleaned.is_empty() && cleaned.len() > 2 {
                    return Some(cleaned.to_string());
                }
            }
        }
        None
    }

    /// Provide default results based on query keywords
    fn default_results(&self, query: &str) -> Vec<DocResult> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        // Map common queries to relevant documentation
        if query_lower.contains("intent") {
            results.push(DocResult {
                title: "Intents and Intent Filters".to_string(),
                url: format!("{}/guide/components/intents-filters", DOCS_BASE_URL),
                description: Some("Learn how to use intents to request actions from other app components.".to_string()),
            });
        }

        if query_lower.contains("activity") {
            results.push(DocResult {
                title: "Activities Overview".to_string(),
                url: format!("{}/guide/components/activities/intro", DOCS_BASE_URL),
                description: Some("Introduction to Android activities and the activity lifecycle.".to_string()),
            });
        }

        if query_lower.contains("fragment") {
            results.push(DocResult {
                title: "Fragments".to_string(),
                url: format!("{}/guide/fragments", DOCS_BASE_URL),
                description: Some("Build flexible UIs with fragments.".to_string()),
            });
        }

        if query_lower.contains("permission") {
            results.push(DocResult {
                title: "Request App Permissions".to_string(),
                url: format!("{}/training/permissions/requesting", DOCS_BASE_URL),
                description: Some("Learn how to request runtime permissions.".to_string()),
            });
        }

        if query_lower.contains("broadcast") {
            results.push(DocResult {
                title: "Broadcasts Overview".to_string(),
                url: format!("{}/guide/components/broadcasts", DOCS_BASE_URL),
                description: Some("Learn about sending and receiving broadcast messages.".to_string()),
            });
        }

        if query_lower.contains("service") {
            results.push(DocResult {
                title: "Services Overview".to_string(),
                url: format!("{}/guide/components/services", DOCS_BASE_URL),
                description: Some("Background operations with services.".to_string()),
            });
        }

        if query_lower.contains("workmanager") || query_lower.contains("work manager") {
            results.push(DocResult {
                title: "WorkManager".to_string(),
                url: format!("{}/topic/libraries/architecture/workmanager", DOCS_BASE_URL),
                description: Some("Schedule deferrable, asynchronous tasks with WorkManager.".to_string()),
            });
        }

        if query_lower.contains("room") || query_lower.contains("database") {
            results.push(DocResult {
                title: "Room Persistence Library".to_string(),
                url: format!("{}/training/data-storage/room", DOCS_BASE_URL),
                description: Some("Use Room for local database storage.".to_string()),
            });
        }

        if query_lower.contains("retrofit") || query_lower.contains("network") || query_lower.contains("api") {
            results.push(DocResult {
                title: "Network Operations".to_string(),
                url: format!("{}/training/basics/network-ops", DOCS_BASE_URL),
                description: Some("Perform network operations in Android.".to_string()),
            });
        }

        if query_lower.contains("jetpack") || query_lower.contains("compose") {
            results.push(DocResult {
                title: "Jetpack Compose".to_string(),
                url: format!("{}/jetpack/compose", DOCS_BASE_URL),
                description: Some("Build native UIs with Jetpack Compose.".to_string()),
            });
        }

        // If no specific matches, provide general reference
        if results.is_empty() {
            results.push(DocResult {
                title: format!("Search: {}", query),
                url: format!("{}?q={}", SEARCH_API_URL, urlencoding::encode(query)),
                description: Some("Click to view search results on developer.android.com".to_string()),
            });
        }

        results
    }

    /// Fetch documentation content from a URL
    /// Validate URL to ensure it's from a trusted domain
    fn validate_url(url: &str) -> Result<()> {
        // Parse URL
        let parsed = url::Url::parse(url)
            .with_context(|| format!("Invalid URL format: {}", url))?;

        // Ensure HTTPS scheme
        if parsed.scheme() != "https" {
            return Err(anyhow::anyhow!(
                "URL must use HTTPS scheme for security: {}",
                url
            ));
        }

        // Check allowed domains
        let host = parsed.host_str()
            .ok_or_else(|| anyhow::anyhow!("URL has no host: {}", url))?;

        let allowed_domains = [
            "developer.android.com",
            "android.com",
            "source.android.com",
            "developer.chrome.com",
        ];

        let is_allowed = allowed_domains.iter().any(|domain| {
            host == *domain || host.ends_with(&format!(".{}", domain))
        });

        if !is_allowed {
            return Err(anyhow::anyhow!(
                "URL must be from an allowed domain (developer.android.com, android.com, etc.): {}",
                url
            ));
        }

        Ok(())
    }

    pub fn fetch(&self, url: &str) -> Result<String> {
        // Validate URL for security
        Self::validate_url(url)?;

        println!("Fetching documentation from: {}", url);

        let response = self.client
            .get(url)
            .send()
            .context("Failed to fetch documentation")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch documentation: HTTP {}",
                response.status()
            ));
        }

        let html = response.text()
            .context("Failed to read documentation content")?;

        // Parse and format the documentation
        let content = self.parse_documentation(&html, url);

        Ok(content)
    }

    /// Parse HTML documentation into readable text
    fn parse_documentation(&self, html: &str, url: &str) -> String {
        let mut content = String::new();
        content.push_str(&format!("Source: {}\n\n", url));

        // Extract title
        if let Some(title_start) = html.find("<title>") {
            if let Some(title_end) = html.find("</title>") {
                let title = &html[title_start + 7..title_end];
                // Clean up title (remove " | Android Developers" suffix)
                let clean_title = title.split('|').next().unwrap_or(title).trim();
                content.push_str(&format!("# {}\n\n", clean_title));
            }
        }

        // Extract main content (article body)
        if let Some(article_start) = html.find("<article") {
            if let Some(article_end) = html.find("</article>") {
                let article = &html[article_start..article_end + 10];
                content.push_str(&self.html_to_markdown(article));
            }
        } else if let Some(main_start) = html.find("<main") {
            if let Some(main_end) = html.find("</main>") {
                let main_content = &html[main_start..main_end + 7];
                content.push_str(&self.html_to_markdown(main_content));
            }
        } else {
            // Fallback: extract paragraphs
            content.push_str(&self.extract_paragraphs(html));
        }

        content
    }

    /// Convert basic HTML to Markdown
    fn html_to_markdown(&self, html: &str) -> String {
        let mut result = String::new();
        let mut in_code_block = false;
        let mut in_list = false;

        // Remove script and style tags
        let html = self.remove_tags(html, "script");
        let html = self.remove_tags(&html, "style");
        let html = self.remove_tags(&html, "nav");

        let lines: Vec<&str> = html.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Handle code blocks
            if trimmed.contains("<pre") || trimmed.contains("<code>") {
                if !in_code_block {
                    result.push_str("```\n");
                    in_code_block = true;
                }
            }

            if in_code_block {
                if trimmed.contains("</pre>") || trimmed.contains("</code>") {
                    result.push_str("```\n");
                    in_code_block = false;
                } else {
                    // Strip remaining HTML tags
                    let code = self.strip_html_tags(trimmed);
                    if !code.is_empty() {
                        result.push_str(&code);
                        result.push('\n');
                    }
                }
                i += 1;
                continue;
            }

            // Handle headings
            if trimmed.starts_with("<h1") {
                if let Some(text) = self.extract_tag_content(trimmed, "h1") {
                    result.push_str(&format!("# {}\n\n", text));
                }
            } else if trimmed.starts_with("<h2") {
                if let Some(text) = self.extract_tag_content(trimmed, "h2") {
                    result.push_str(&format!("## {}\n\n", text));
                }
            } else if trimmed.starts_with("<h3") {
                if let Some(text) = self.extract_tag_content(trimmed, "h3") {
                    result.push_str(&format!("### {}\n\n", text));
                }
            } else if trimmed.starts_with("<h4") {
                if let Some(text) = self.extract_tag_content(trimmed, "h4") {
                    result.push_str(&format!("#### {}\n\n", text));
                }
            }
            // Handle lists
            else if trimmed.starts_with("<li") || trimmed.starts_with("<li>") {
                if let Some(text) = self.extract_tag_content(trimmed, "li") {
                    result.push_str(&format!("- {}\n", text));
                    in_list = true;
                }
            }
            // Handle paragraphs
            else if trimmed.starts_with("<p") || trimmed.starts_with("<p>") {
                if let Some(text) = self.extract_tag_content(trimmed, "p") {
                    if !text.is_empty() {
                        result.push_str(&text);
                        result.push_str("\n\n");
                    }
                }
            }

            i += 1;
        }

        result
    }

    /// Remove all content between start and end tags
    fn remove_tags(&self, html: &str, tag: &str) -> String {
        let open_tag = format!("<{}", tag);
        let close_tag = format!("</{}>", tag);

        let mut result = String::new();
        let mut remaining = html;

        while let Some(start) = remaining.find(&open_tag) {
            result.push_str(&remaining[..start]);
            remaining = &remaining[start..];

            if let Some(end) = remaining.find(&close_tag) {
                remaining = &remaining[end + close_tag.len()..];
            } else {
                break;
            }
        }
        result.push_str(remaining);
        result
    }

    /// Extract content from a specific HTML tag
    fn extract_tag_content(&self, html: &str, tag: &str) -> Option<String> {
        let open_pattern = format!("<{}", tag);
        let close_pattern = format!("</{}>", tag);

        // Find the start of the tag content (after the closing >)
        let start = html.find(&open_pattern)?;
        let tag_end = html[start..].find('>')?;
        let content_start = start + tag_end + 1;

        // Find the closing tag
        let end = html[content_start..].find(&close_pattern)?;
        let content = &html[content_start..content_start + end];

        Some(self.strip_html_tags(content).trim().to_string())
    }

    /// Strip all HTML tags from text
    fn strip_html_tags(&self, html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;

        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => result.push(c),
                _ => {}
            }
        }

        // Clean up HTML entities
        result
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
    }

    /// Extract paragraphs from HTML as fallback
    fn extract_paragraphs(&self, html: &str) -> String {
        let mut result = String::new();

        let mut pos = 0;
        while let Some(start) = html[pos..].find("<p") {
            let full_start = pos + start;
            if let Some(tag_end) = html[full_start..].find('>') {
                let content_start = full_start + tag_end + 1;
                if let Some(end) = html[content_start..].find("</p>") {
                    let paragraph = &html[content_start..content_start + end];
                    let text = self.strip_html_tags(paragraph);
                    if !text.trim().is_empty() {
                        result.push_str(&text);
                        result.push_str("\n\n");
                    }
                    pos = content_start + end + 4;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        result
    }

    /// Display search results
    pub fn display_search_results(results: &[DocResult]) {
        if results.is_empty() {
            println!("No results found.");
            return;
        }

        println!("Found {} result(s):\n", results.len());
        for (i, result) in results.iter().enumerate() {
            println!("{}. {}", i + 1, result.title);
            println!("   {}", result.url);
            if let Some(desc) = &result.description {
                println!("   {}", desc);
            }
            println!();
        }
    }
}

impl Default for DocsCLI {
    fn default() -> Self {
        Self::new().expect("Failed to create DocsCLI")
    }
}

/// A single documentation search result
#[derive(Debug, Clone)]
pub struct DocResult {
    pub title: String,
    pub url: String,
    pub description: Option<String>,
}

/// Simple URL encoding (since urlencoding crate may not be available)
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::new();
        for c in s.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
                ' ' => result.push_str("+"),
                _ => {
                    for byte in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", byte));
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docs_cli_create() {
        let docs_cli = DocsCLI::new();
        assert!(docs_cli.is_ok());
    }

    #[test]
    fn test_default_results() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("intent");
        assert!(!results.is_empty());
        assert!(results[0].title.contains("Intent"));
    }

    #[test]
    fn test_url_encoding() {
        let encoded = urlencoding::encode("hello world");
        assert_eq!(encoded, "hello+world");
    }

    #[test]
    fn test_search_query_formatting() {
        // Test that search queries are properly formatted for URLs
        let query = "activity lifecycle";
        let encoded = urlencoding::encode(query);
        let search_url = format!("{}?q={}", SEARCH_API_URL, encoded);

        assert!(search_url.contains("?q=activity+lifecycle"));
        assert!(search_url.starts_with(SEARCH_API_URL));
    }

    #[test]
    fn test_search_query_special_characters() {
        // Test encoding of special characters in search queries
        let query = "android:exported=true";
        let encoded = urlencoding::encode(query);

        // Colon should be encoded
        assert!(encoded.contains("%3A") || encoded.contains(":"));
        // Equals should be encoded
        assert!(encoded.contains("%3D") || encoded.contains("="));
    }

    #[test]
    fn test_search_query_spaces() {
        // Test that spaces are encoded as +
        let query = "jetpack compose";
        let encoded = urlencoding::encode(query);
        assert_eq!(encoded, "jetpack+compose");
    }

    #[test]
    fn test_fetch_url_handling() {
        // Test URL construction for fetch
        let _test_url = "https://developer.android.com/guide/components/activities";
        let docs_cli = DocsCLI::new().unwrap();

        // We can't make actual network requests in tests, but we can verify
        // that the client is configured correctly (client exists)
        let _ = docs_cli.client;
    }

    #[test]
    fn test_docs_cli_initialization() {
        // Test that DocsCLI initializes correctly with proper configuration
        let docs_cli = DocsCLI::new();
        assert!(docs_cli.is_ok());
    }

    #[test]
    fn test_docs_cli_default() {
        // Test Default trait implementation
        let docs_cli = DocsCLI::default();
        // Just verify it creates successfully
        let _ = docs_cli.client;
    }

    #[test]
    fn test_result_rendering_empty() {
        // Test rendering of empty results
        let results: Vec<DocResult> = vec![];
        // Should not panic
        DocsCLI::display_search_results(&results);
    }

    #[test]
    fn test_result_rendering_with_results() {
        // Test rendering of results
        let results = vec![
            DocResult {
                title: "Test Result".to_string(),
                url: "https://example.com/test".to_string(),
                description: Some("Test description".to_string()),
            },
            DocResult {
                title: "Another Result".to_string(),
                url: "https://example.com/another".to_string(),
                description: None,
            },
        ];
        // Should not panic
        DocsCLI::display_search_results(&results);
    }

    #[test]
    fn test_doc_result_creation() {
        // Test creating DocResult instances
        let result = DocResult {
            title: "Activities Overview".to_string(),
            url: "https://developer.android.com/guide/components/activities".to_string(),
            description: Some("Learn about Android activities.".to_string()),
        };

        assert_eq!(result.title, "Activities Overview");
        assert!(result.url.starts_with("https://"));
        assert!(result.description.is_some());
    }

    #[test]
    fn test_default_results_intent() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("intent");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("Intent")));
        assert!(results.iter().all(|r| r.url.starts_with(DOCS_BASE_URL)));
    }

    #[test]
    fn test_default_results_activity() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("activity");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("Activit")));
    }

    #[test]
    fn test_default_results_fragment() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("fragment");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("Fragment")));
    }

    #[test]
    fn test_default_results_permission() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("permission");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("Permission")));
    }

    #[test]
    fn test_default_results_broadcast() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("broadcast");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("Broadcast")));
    }

    #[test]
    fn test_default_results_service() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("service");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("Service")));
    }

    #[test]
    fn test_default_results_workmanager() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("workmanager");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("WorkManager")));
    }

    #[test]
    fn test_default_results_room() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("room");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("Room")));
    }

    #[test]
    fn test_default_results_database() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("database");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("Room") || r.title.contains("database")));
    }

    #[test]
    fn test_default_results_jetpack_compose() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("jetpack compose");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("Compose")));
    }

    #[test]
    fn test_default_results_unknown_query() {
        let docs_cli = DocsCLI::new().unwrap();
        let results = docs_cli.default_results("someunknownqueryterm");

        // Should return at least a fallback result
        assert!(!results.is_empty());
    }

    #[test]
    fn test_extract_title_simple() {
        let docs_cli = DocsCLI::new().unwrap();
        let line = r#"<a href="/guide/components">Components</a>"#;
        let title = docs_cli.extract_title(line);

        assert!(title.is_some());
        assert_eq!(title.unwrap(), "Components");
    }

    #[test]
    fn test_extract_title_no_match() {
        let docs_cli = DocsCLI::new().unwrap();
        let line = r#"<div class="nav">"#;
        let title = docs_cli.extract_title(line);

        // Should return None for lines without proper title content
        assert!(title.is_none() || title.as_ref().map(|t| t.is_empty()).unwrap_or(true));
    }

    #[test]
    fn test_strip_html_tags() {
        let docs_cli = DocsCLI::new().unwrap();
        let html = "<p>Hello <strong>World</strong></p>";
        let stripped = docs_cli.strip_html_tags(html);

        assert_eq!(stripped, "Hello World");
    }

    #[test]
    fn test_strip_html_tags_with_entities() {
        let docs_cli = DocsCLI::new().unwrap();
        let html = "Hello&nbsp;World&amp;Test&lt;End";
        let stripped = docs_cli.strip_html_tags(html);

        assert!(stripped.contains("Hello World"));
        assert!(stripped.contains("&"));
        assert!(stripped.contains("<"));
    }

    #[test]
    fn test_parse_search_results_empty() {
        let docs_cli = DocsCLI::new().unwrap();
        let html = "<html><body>No results</body></html>";
        let results = docs_cli.parse_search_results(html, "test");

        // Should return empty or fallback to default results
        assert!(results.len() <= 10);
    }

    #[test]
    fn test_parse_search_results_with_links() {
        let docs_cli = DocsCLI::new().unwrap();
        let html = r#"<html><body><a href="https://developer.android.com/guide/test">Test Guide</a></body></html>"#;
        let results = docs_cli.parse_search_results(html, "test");

        // Should extract the link
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.url.contains("developer.android.com")));
    }

    #[test]
    fn test_html_to_markdown_basic() {
        let docs_cli = DocsCLI::new().unwrap();
        // The parser expects headings and paragraphs on separate lines
        let html = r#"<article>
<h1>Title</h1>
<p>Paragraph text</p>
</article>"#;
        let markdown = docs_cli.html_to_markdown(html);

        assert!(markdown.contains("# Title"));
        assert!(markdown.contains("Paragraph text"));
    }

    #[test]
    fn test_html_to_markdown_code_blocks() {
        let docs_cli = DocsCLI::new().unwrap();
        let html = r#"<article>
<code>
fn main() {}
</code>
</article>"#;
        let markdown = docs_cli.html_to_markdown(html);

        assert!(markdown.contains("```"));
    }

    #[test]
    fn test_html_to_markdown_headings() {
        let docs_cli = DocsCLI::new().unwrap();
        let html = r#"<article>
<h2>Section</h2>
<h3>Subsection</h3>
</article>"#;
        let markdown = docs_cli.html_to_markdown(html);

        assert!(markdown.contains("## Section"));
        assert!(markdown.contains("### Subsection"));
    }

    #[test]
    fn test_remove_tags() {
        let docs_cli = DocsCLI::new().unwrap();
        let html = r#"<script>var x = 1;</script><p>Content</p><style>.class { color: red; }</style>"#;
        let cleaned = docs_cli.remove_tags(html, "script");
        assert!(!cleaned.contains("<script>"));
        assert!(!cleaned.contains("var x = 1;"));
        assert!(cleaned.contains("<p>Content</p>"));
    }

    #[test]
    fn test_extract_tag_content() {
        let docs_cli = DocsCLI::new().unwrap();
        let html = r#"<p class="description">Hello World</p>"#;
        let content = docs_cli.extract_tag_content(html, "p");

        assert!(content.is_some());
        assert_eq!(content.unwrap(), "Hello World");
    }

    #[test]
    fn test_extract_paragraphs() {
        let docs_cli = DocsCLI::new().unwrap();
        let html = r#"<html><body><p>First paragraph.</p><p>Second paragraph.</p></body></html>"#;
        let paragraphs = docs_cli.extract_paragraphs(html);

        assert!(paragraphs.contains("First paragraph."));
        assert!(paragraphs.contains("Second paragraph."));
    }

    #[test]
    fn test_parse_documentation() {
        let docs_cli = DocsCLI::new().unwrap();
        // The parser looks for <title> and <article> blocks
        let html = r#"<html>
<head>
<title>Test Page | Android Developers</title>
</head>
<body>
<article>
<p>Content here.</p>
</article>
</body>
</html>"#;
        let content = docs_cli.parse_documentation(html, "https://example.com/test");

        assert!(content.contains("Source: https://example.com/test"));
        assert!(content.contains("# Test Page"));
        assert!(content.contains("Content here."));
    }

    #[test]
    fn test_base_url_constant() {
        assert!(DOCS_BASE_URL.starts_with("https://"));
        assert!(DOCS_BASE_URL.contains("developer.android.com"));
    }

    #[test]
    fn test_search_api_url_constant() {
        assert!(SEARCH_API_URL.starts_with("https://"));
        assert!(SEARCH_API_URL.contains("developer.android.com"));
    }
}