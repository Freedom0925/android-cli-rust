use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

pub mod service;

use service::kb_download::KnowledgeBaseConstants;
pub use service::{
    IndexStats, KbFetchResult, KbSearchOptions, KbSearchResponse, KbSearchResult, SearchHit,
};
pub use service::{KBDownloadService, KbDoc, KbDocFile, KbDownloadResult, KbIndexerService};

/// KB storage directory (matches Google's .android/cli/docs/kbzip)
const KB_STORAGE_DIR: &str = "docs/kbzip";
/// KB index directory (matches Google's .android/cli/docs/index)
const KB_INDEX_DIR: &str = "docs/index";

/// Documentation CLI handler - uses KB local index
/// Matches Google's DocsCLI.kt implementation
pub struct DocsCLI {
    /// Storage directory for KB data (kbzip)
    storage_dir: PathBuf,
    /// Index directory for Tantivy index
    #[allow(dead_code)]
    index_dir: PathBuf,
    /// KB download service
    download_service: KBDownloadService,
    /// KB indexer service
    indexer_service: KbIndexerService,
}

impl DocsCLI {
    /// Create a new DocsCLI instance
    /// Matches Google's DocsCLI which uses:
    /// - storageDir: .android/cli/docs/kbzip
    /// - indexDir: .android/cli/docs/index
    pub fn new() -> Result<Self> {
        let cli_dir = Self::get_cli_dir()?;

        let storage_dir = cli_dir.join(KB_STORAGE_DIR);
        let index_dir = cli_dir.join(KB_INDEX_DIR);

        let download_service = KBDownloadService::new(storage_dir.clone());
        let indexer_service = KbIndexerService::new(index_dir.clone());

        Ok(Self {
            storage_dir,
            index_dir,
            download_service,
            indexer_service,
        })
    }

    /// Create DocsCLI with custom directories (for testing)
    pub fn with_dirs(storage_dir: PathBuf, index_dir: PathBuf) -> Result<Self> {
        let download_service = KBDownloadService::new(storage_dir.clone());
        let indexer_service = KbIndexerService::new(index_dir.clone());

        Ok(Self {
            storage_dir,
            index_dir,
            download_service,
            indexer_service,
        })
    }

    /// Get default CLI directory (.android/cli)
    fn get_cli_dir() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

        Ok(home.join(".android").join("cli"))
    }

    /// Ensure KB is ready (download and index if needed)
    pub fn ensure_kb_ready(&mut self) -> Result<()> {
        // Check if KB is ready (index_ready.json exists)
        if self.download_service.is_ready()? {
            // KB is already indexed
            self.indexer_service.init_index()?;
            return Ok(());
        }

        // Download KB ZIP (or use existing)
        let result = self.download_service.get_or_update_zip()?;

        // Always build index if ready file doesn't exist
        // (even if ZIP was already downloaded)
        println!("Building search index (this may take a moment)...");
        let doc_count = self
            .indexer_service
            .build_index_from_zip(&result.zip_path)?;
        println!("Indexed {} documents", doc_count);

        // Create ready file (index_ready.json)
        self.download_service.create_ready_file(&result.sha256)?;

        Ok(())
    }

    /// Search Android documentation using KB index
    pub fn search(&mut self, query: &str) -> Result<Vec<DocResult>> {
        println!("Searching documentation for: {}", query);

        // Ensure KB is ready
        self.ensure_kb_ready()?;

        let start = Instant::now();

        // Search using KB indexer with default limit (10)
        let hits = self
            .indexer_service
            .search(query, KnowledgeBaseConstants::MAX_RESULTS_SEARCH)?;

        let elapsed = start.elapsed();

        if hits.is_empty() {
            println!("No results found ({}ms)", elapsed.as_millis());
            return Ok(vec![]);
        }

        println!("Found {} results ({}ms)", hits.len(), elapsed.as_millis());

        // Convert SearchHit to DocResult
        let results: Vec<DocResult> = hits
            .into_iter()
            .map(|hit| DocResult {
                title: hit.title.unwrap_or_else(|| "Untitled".to_string()),
                url: hit
                    .url
                    .unwrap_or_else(|| hit.filepath.clone().unwrap_or_default()),
                description: hit.summary,
                score: Some(hit.score),
                filepath: hit.filepath,
            })
            .collect();

        Ok(results)
    }

    /// Search with custom options
    pub fn search_with_options(
        &mut self,
        query: &str,
        options: &KbSearchOptions,
    ) -> Result<Vec<DocResult>> {
        self.ensure_kb_ready()?;

        let start = Instant::now();

        // Use boosted search
        let hits = self.indexer_service.search_with_boost(
            query,
            options.title_boost,
            options.content_boost,
            options.limit,
        )?;

        let elapsed = start.elapsed();

        println!("Found {} results ({}ms)", hits.len(), elapsed.as_millis());

        let results: Vec<DocResult> = hits
            .into_iter()
            .map(|hit| DocResult {
                title: hit.title.unwrap_or_else(|| "Untitled".to_string()),
                url: hit
                    .url
                    .unwrap_or_else(|| hit.filepath.clone().unwrap_or_default()),
                description: hit.summary,
                score: Some(hit.score),
                filepath: hit.filepath,
            })
            .collect();

        Ok(results)
    }

    /// Get KB index statistics
    pub fn stats(&mut self) -> Result<IndexStats> {
        self.ensure_kb_ready()?;
        self.indexer_service.stats()
    }

    /// Clear KB cache (re-download on next search)
    pub fn clear_cache(&mut self) -> Result<()> {
        // Clear index
        self.indexer_service.clear_index()?;

        // Remove KB ZIP (dac.zip)
        let zip_path = self.storage_dir.join("dac.zip");
        if zip_path.exists() {
            fs::remove_file(&zip_path)?;
        }

        // Remove ETag file (dac.etag)
        let etag_path = self.storage_dir.join("dac.etag");
        if etag_path.exists() {
            fs::remove_file(&etag_path)?;
        }

        // Remove timestamp file
        let timestamp_path = self.storage_dir.join("last_update_check.timestamp");
        if timestamp_path.exists() {
            fs::remove_file(&timestamp_path)?;
        }

        // Remove ready file (index_ready.json)
        let ready_path = self
            .storage_dir
            .join(KnowledgeBaseConstants::SENTINEL_FILE_NAME);
        if ready_path.exists() {
            fs::remove_file(&ready_path)?;
        }

        println!("KB cache cleared");
        Ok(())
    }

    /// Check if KB index is ready
    pub fn is_ready(&self) -> bool {
        self.download_service.is_ready().unwrap_or(false)
    }

    /// Display search results
    pub fn display_search_results(results: &[DocResult]) {
        if results.is_empty() {
            println!("No results found.");
            return;
        }

        println!("Found {} result(s):\n", results.len());
        for (i, result) in results.iter().enumerate() {
            let score_str = result
                .score
                .map(|s| format!("[{:.2}] ", s))
                .unwrap_or_default();

            println!("{}. {}{}", i + 1, score_str, result.title);
            println!("   {}", result.url);
            if let Some(desc) = &result.description {
                if !desc.is_empty() {
                    println!("   {}", desc);
                }
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
    /// Relevance score (BM25)
    pub score: Option<f32>,
    /// Source filepath in KB ZIP
    pub filepath: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_docs_cli_create() {
        let temp_dir = tempdir().unwrap();
        let storage_dir = temp_dir.path().join("kbzip");
        let index_dir = temp_dir.path().join("index");
        let docs_cli = DocsCLI::with_dirs(storage_dir, index_dir);
        assert!(docs_cli.is_ok());
    }

    #[test]
    fn test_docs_cli_default_dirs() {
        let cli_dir = DocsCLI::get_cli_dir();
        assert!(cli_dir.is_ok());
        let path = cli_dir.unwrap();
        assert!(path.to_string_lossy().contains(".android"));
        assert!(path.to_string_lossy().contains("cli"));
    }

    #[test]
    fn test_doc_result_creation() {
        let result = DocResult {
            title: "Activities Overview".to_string(),
            url: "https://developer.android.com/guide/components/activities".to_string(),
            description: Some("Learn about Android activities.".to_string()),
            score: Some(1.5),
            filepath: Some("docs/activities.md".to_string()),
        };

        assert_eq!(result.title, "Activities Overview");
        assert!(result.url.starts_with("https://"));
        assert!(result.score.is_some());
    }

    #[test]
    fn test_display_search_results_empty() {
        let results: Vec<DocResult> = vec![];
        DocsCLI::display_search_results(&results);
    }

    #[test]
    fn test_display_search_results_with_results() {
        let results = vec![
            DocResult {
                title: "Test Result".to_string(),
                url: "https://example.com/test".to_string(),
                description: Some("Test description".to_string()),
                score: Some(2.5),
                filepath: None,
            },
            DocResult {
                title: "Another Result".to_string(),
                url: "https://example.com/another".to_string(),
                description: None,
                score: None,
                filepath: Some("docs/another.md".to_string()),
            },
        ];
        DocsCLI::display_search_results(&results);
    }

    #[test]
    fn test_is_ready_false_initially() {
        let temp_dir = tempdir().unwrap();
        let storage_dir = temp_dir.path().join("kbzip");
        let index_dir = temp_dir.path().join("index");
        let docs_cli = DocsCLI::with_dirs(storage_dir, index_dir).unwrap();
        assert!(!docs_cli.is_ready());
    }

    #[test]
    fn test_clear_cache() {
        let temp_dir = tempdir().unwrap();
        let storage_dir = temp_dir.path().join("kbzip");
        let index_dir = temp_dir.path().join("index");
        let mut docs_cli = DocsCLI::with_dirs(storage_dir, index_dir).unwrap();
        docs_cli.clear_cache().unwrap();
        assert!(!docs_cli.is_ready());
    }
}
