use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Knowledge base document structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbDoc {
    /// URL of the document
    pub url: String,
    /// Title of the document
    pub title: String,
    /// Short description/summary
    pub summary: String,
    /// Keywords for search
    pub keywords: String,
    /// Full content
    pub content: String,
}

/// KB document file structure (from ZIP archive)
#[derive(Debug, Clone)]
pub struct KbDocFile {
    /// File path within ZIP
    pub filepath: String,
    /// Content summary (first N lines)
    pub summary: String,
    /// Full content
    pub content: String,
    /// Metadata from frontmatter or header
    pub metadata: HashMap<String, String>,
}

impl KbDocFile {
    /// Parse a document file from markdown content
    pub fn from_markdown(filepath: String, content: String) -> Self {
        let (metadata, body) = Self::parse_frontmatter(&content);

        // Extract summary from first few lines
        let summary = Self::extract_summary(&body);

        Self {
            filepath,
            summary,
            content: body,
            metadata,
        }
    }

    /// Parse YAML frontmatter from content
    fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
        let mut metadata = HashMap::new();

        // Check for YAML frontmatter (--- delimited)
        if content.starts_with("---") {
            let parts: Vec<&str> = content.splitn(3, "---").collect();
            if parts.len() >= 3 {
                // Parse YAML metadata
                let yaml_content = parts[1].trim();
                for line in yaml_content.lines() {
                    if let Some((key, value)) = line.split_once(':') {
                        let key = key.trim();
                        let value = value.trim();
                        // Remove quotes if present
                        let value = value.trim_matches('"').trim_matches('\'');
                        metadata.insert(key.to_string(), value.to_string());
                    }
                }
                return (metadata, parts[2].trim().to_string());
            }
        }

        (metadata, content.to_string())
    }

    /// Extract summary from content (first non-empty paragraph)
    fn extract_summary(content: &str) -> String {
        let mut summary_lines = Vec::new();
        let mut in_paragraph = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines and headers
            if trimmed.is_empty() {
                if in_paragraph && summary_lines.len() >= 3 {
                    break;
                }
                in_paragraph = false;
                continue;
            }

            if trimmed.starts_with('#') {
                continue;
            }

            in_paragraph = true;
            summary_lines.push(trimmed);

            if summary_lines.len() >= 5 {
                break;
            }
        }

        summary_lines.join(" ").chars().take(200).collect()
    }

    /// Get URL from metadata
    pub fn url(&self) -> Option<&str> {
        self.metadata.get("url").map(|s| s.as_str())
            .or_else(|| self.metadata.get("relative_url").map(|s| s.as_str()))
    }

    /// Get title from metadata
    pub fn title(&self) -> Option<&str> {
        self.metadata.get("title").map(|s| s.as_str())
    }

    /// Get keywords from metadata
    pub fn keywords(&self) -> Option<&str> {
        self.metadata.get("keywords").map(|s| s.as_str())
            .or_else(|| self.metadata.get("tags").map(|s| s.as_str()))
    }
}

/// KB download result
#[derive(Debug, Clone)]
pub struct KbDownloadResult {
    /// Path to downloaded ZIP file
    pub zip_path: std::path::PathBuf,
    /// SHA256 hash of downloaded content
    pub sha256: String,
    /// Whether this was a new download
    pub is_new: bool,
}

/// KB index state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KbIndexState {
    /// Index is initializing
    Initializing,
    /// Index is ready for searching
    Ready,
    /// Index has an error
    Error,
}

/// Content of sentinel file for KB index
///
/// The sentinel file tracks the state of the KB index,
/// including any errors during indexing and the hash of the indexed ZIP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelFileContent {
    /// Whether indexing had an error
    pub index_error: bool,
    /// SHA256 hash of the KB ZIP
    pub zip_hash: Option<String>,
}

impl SentinelFileContent {
    /// Create a new sentinel file content
    pub fn new(index_error: bool, zip_hash: Option<String>) -> Self {
        Self { index_error, zip_hash }
    }

    /// Create a sentinel indicating successful indexing
    pub fn success(zip_hash: String) -> Self {
        Self::new(false, Some(zip_hash))
    }

    /// Create a sentinel indicating indexing error
    pub fn error() -> Self {
        Self::new(true, None)
    }

    /// Check if indexing was successful
    pub fn is_success(&self) -> bool {
        !self.index_error
    }

    /// Check if indexing had an error
    pub fn has_error(&self) -> bool {
        self.index_error
    }

    /// Parse from JSON content
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize to pretty JSON
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

impl Default for SentinelFileContent {
    fn default() -> Self {
        Self::new(false, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = "---\ntitle: Test Doc\nurl: /test\n---\nContent here";
        let (metadata, body) = KbDocFile::parse_frontmatter(content);

        assert_eq!(metadata.get("title").unwrap(), "Test Doc");
        assert_eq!(metadata.get("url").unwrap(), "/test");
        assert_eq!(body, "Content here");
    }

    #[test]
    fn test_parse_frontmatter_with_quotes() {
        let content = "---\ntitle: \"Quoted Title\"\n---\nBody";
        let (metadata, body) = KbDocFile::parse_frontmatter(content);

        assert_eq!(metadata.get("title").unwrap(), "Quoted Title");
        assert_eq!(body, "Body");
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "Just plain content\nNo frontmatter";
        let (metadata, body) = KbDocFile::parse_frontmatter(content);

        assert!(metadata.is_empty());
        assert_eq!(body, content);
    }

    #[test]
    fn test_extract_summary() {
        let content = "# Title\n\nFirst paragraph line.\nSecond line.\n\n# Another header\nMore content";
        let summary = KbDocFile::extract_summary(content);

        assert!(summary.contains("First paragraph"));
        assert!(!summary.contains("# Title"));
    }

    #[test]
    fn test_kb_doc_file_creation() {
        let content = "---\ntitle: Test\nurl: /test\nkeywords: android, cli\n---\nBody content";
        let doc = KbDocFile::from_markdown("test.md".to_string(), content.to_string());

        assert_eq!(doc.filepath, "test.md");
        assert_eq!(doc.title().unwrap(), "Test");
        assert_eq!(doc.url().unwrap(), "/test");
        assert_eq!(doc.keywords().unwrap(), "android, cli");
    }

    #[test]
    fn test_sentinel_file_content_success() {
        let sentinel = SentinelFileContent::success("abc123".to_string());
        assert!(sentinel.is_success());
        assert!(!sentinel.has_error());
        assert_eq!(sentinel.zip_hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_sentinel_file_content_error() {
        let sentinel = SentinelFileContent::error();
        assert!(!sentinel.is_success());
        assert!(sentinel.has_error());
        assert!(sentinel.zip_hash.is_none());
    }

    #[test]
    fn test_sentinel_file_content_default() {
        let sentinel = SentinelFileContent::default();
        assert!(sentinel.is_success());
        assert!(sentinel.zip_hash.is_none());
    }

    #[test]
    fn test_sentinel_file_content_serialization() {
        let sentinel = SentinelFileContent::success("hash123".to_string());
        let json = sentinel.to_json().unwrap();
        assert!(json.contains("index_error"));
        assert!(json.contains("hash123"));

        let deserialized: SentinelFileContent = SentinelFileContent::from_json(&json).unwrap();
        assert_eq!(deserialized.index_error, false);
        assert_eq!(deserialized.zip_hash, Some("hash123".to_string()));
    }

    #[test]
    fn test_sentinel_file_content_pretty_json() {
        let sentinel = SentinelFileContent::error();
        let json = sentinel.to_json_pretty().unwrap();
        assert!(json.contains("index_error"));
        assert!(json.contains("true"));
    }
}