use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{Schema, TEXT, STORED, Field, Value},
    Index,
    IndexWriter,
};
use zip::ZipArchive;
use std::io::Read;

use super::kb_doc::KbDocFile;
use super::kb_download::KnowledgeBaseConstants;

/// KB Indexer service - builds and queries Tantivy full-text search index
pub struct KbIndexerService {
    /// Index directory path
    index_dir: PathBuf,
    /// Tantivy index
    index: Option<Index>,
    /// Schema fields
    schema: KbSchema,
}

/// Schema field definitions
pub struct KbSchema {
    pub url: Field,
    pub title: Field,
    pub contents: Field,
    pub keywords: Field,
    pub summary: Field,
    pub filepath: Field,
    pub relative_url: Field,
    pub short_description: Field,
}

impl KbSchema {
    /// Create Tantivy schema for KB documents
    pub fn build_schema() -> (Schema, Self) {
        let mut schema_builder = Schema::builder();

        // URL - indexed and stored
        let url = schema_builder.add_text_field(KnowledgeBaseConstants::URL_FIELD, TEXT | STORED);

        // Title - indexed and stored
        let title = schema_builder.add_text_field(KnowledgeBaseConstants::TITLE_FIELD, TEXT | STORED);

        // Contents - indexed (full text search)
        let contents = schema_builder.add_text_field(KnowledgeBaseConstants::CONTENTS_FIELD, TEXT);

        // Keywords - indexed and stored
        let keywords = schema_builder.add_text_field(KnowledgeBaseConstants::KEYWORDS_FIELD, TEXT | STORED);

        // Summary - indexed and stored
        let summary = schema_builder.add_text_field(KnowledgeBaseConstants::SUMMARY_FIELD, TEXT | STORED);

        // Filepath - stored (for reference)
        let filepath = schema_builder.add_text_field(KnowledgeBaseConstants::FILEPATH_FIELD, STORED);

        // Relative URL - indexed and stored
        let relative_url = schema_builder.add_text_field(KnowledgeBaseConstants::RELATIVE_URL_FIELD, TEXT | STORED);

        // Short description - indexed and stored
        let short_description = schema_builder.add_text_field(KnowledgeBaseConstants::SHORT_DESCRIPTION_FIELD, TEXT | STORED);

        let schema = schema_builder.build();

        (schema, Self {
            url,
            title,
            contents,
            keywords,
            summary,
            filepath,
            relative_url,
            short_description,
        })
    }
}

impl KbIndexerService {
    /// Create a new KB indexer service
    pub fn new(cache_dir: PathBuf) -> Self {
        let index_dir = cache_dir.join("kb_index");
        let (_, schema) = KbSchema::build_schema();

        Self {
            index_dir,
            index: None,
            schema,
        }
    }

    /// Get index directory path
    pub fn index_dir(&self) -> &Path {
        &self.index_dir
    }

    /// Ensure index directory exists
    fn ensure_index_dir(&self) -> Result<()> {
        if !self.index_dir.exists() {
            fs::create_dir_all(&self.index_dir)
                .context("Failed to create KB index directory")?;
        }
        Ok(())
    }

    /// Initialize or open existing index
    pub fn init_index(&mut self) -> Result<()> {
        self.ensure_index_dir()?;

        let (schema, fields) = KbSchema::build_schema();
        self.schema = fields;

        // Check if index exists (meta.json in index directory root)
        let index_exists = self.index_dir.join("meta.json").exists();

        if index_exists {
            // Open existing index
            let index = Index::open_in_dir(&self.index_dir)
                .context("Failed to open existing KB index")?;
            self.index = Some(index);
        } else {
            // Create new index
            let index = Index::create_in_dir(&self.index_dir, schema)
                .context("Failed to create KB index")?;
            self.index = Some(index);
        }

        Ok(())
    }

    /// Build index from KB ZIP file
    /// Matches Google's processZipFile implementation:
    /// - Read all entries into memory
    /// - For each .md/.md.txt file, find corresponding .json metadata
    pub fn build_index_from_zip(&mut self, zip_path: &Path) -> Result<usize> {
        self.init_index()?;

        let index = self.index.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Index not initialized"))?;

        let mut writer = index.writer(50_000_000) // 50MB heap
            .context("Failed to create index writer")?;

        // Open ZIP file
        let file = fs::File::open(zip_path)
            .with_context(|| format!("Failed to open ZIP: {}", zip_path.display()))?;

        let mut archive = ZipArchive::new(file)
            .context("Failed to read ZIP archive")?;

        // First pass: read all entries into memory
        let mut all_entries: HashMap<String, Vec<u8>> = HashMap::new();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .context("Failed to get ZIP entry")?;

            let name = file.name().to_string();
            if !file.is_dir() {
                let mut content = Vec::new();
                file.read_to_end(&mut content)
                    .context("Failed to read ZIP entry content")?;
                all_entries.insert(name, content);
            }
        }

        let mut doc_count = 0;

        // Second pass: process .md and .md.txt files with their .json metadata
        for (filepath, content_bytes) in &all_entries {
            // Check if this is a markdown file (.md or .md.txt)
            if !filepath.ends_with(".md") && !filepath.ends_with(".md.txt") {
                continue;
            }

            // Read markdown content
            let md_content = String::from_utf8_lossy(content_bytes).to_string();

            // Find corresponding .json file
            let json_path = Self::replace_md_extension_to_json(filepath);
            let metadata = if let Some(json_bytes) = all_entries.get(&json_path) {
                // Parse JSON metadata
                let json_str = String::from_utf8_lossy(json_bytes);
                Self::parse_json_metadata(&json_str)
            } else {
                HashMap::new()
            };

            // Create KbDocFile
            let kb_doc = KbDocFile::with_metadata(filepath.clone(), md_content, metadata);

            // Add to index
            self.add_document(&writer, &kb_doc)?;

            doc_count += 1;

            // Commit periodically to avoid memory pressure
            if doc_count % 100 == 0 {
                writer.commit()
                    .context("Failed to commit intermediate index")?;
            }
        }

        // Final commit
        writer.commit()
            .context("Failed to commit final index")?;

        // Recreate index reader to see changes
        let reader = index.reader()
            .context("Failed to create index reader")?;
        reader.reload()
            .context("Failed to reload index reader")?;

        Ok(doc_count)
    }

    /// Replace .md or .md.txt extension with .json
    /// Matches Google's replaceMdExtensionToJson
    fn replace_md_extension_to_json(path: &str) -> String {
        if path.ends_with(".md.txt") {
            path.replace(".md.txt", ".json")
        } else if path.ends_with(".md") {
            path.replace(".md", ".json")
        } else {
            path.to_string()
        }
    }

    /// Parse JSON metadata (Map<String, String>)
    fn parse_json_metadata(json_str: &str) -> HashMap<String, String> {
        // Simple JSON parsing for flat key-value objects
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Some(obj) = json_value.as_object() {
                let mut metadata = HashMap::new();
                for (key, value) in obj {
                    if let Some(str_val) = value.as_str() {
                        metadata.insert(key.clone(), str_val.to_string());
                    }
                }
                return metadata;
            }
        }
        HashMap::new()
    }

    /// Add a document to the index
    fn add_document(&self, writer: &IndexWriter, kb_doc: &KbDocFile) -> Result<()> {
        let mut doc = doc!();

        // Add URL if present
        if let Some(url) = kb_doc.url() {
            doc.add_text(self.schema.url, url);
        }

        // Add title if present
        if let Some(title) = kb_doc.title() {
            doc.add_text(self.schema.title, title);
        }

        // Add keywords if present
        if let Some(keywords) = kb_doc.keywords() {
            doc.add_text(self.schema.keywords, keywords);
        }

        // Add filepath
        doc.add_text(self.schema.filepath, &kb_doc.filepath);

        // Add summary
        doc.add_text(self.schema.summary, &kb_doc.summary);

        // Add full contents (for full-text search)
        doc.add_text(self.schema.contents, &kb_doc.content);

        // Add relative_url from metadata if present
        if let Some(rel_url) = kb_doc.metadata.get("relative_url") {
            doc.add_text(self.schema.relative_url, rel_url);
        }

        // Add short_description from metadata if present
        if let Some(short_desc) = kb_doc.metadata.get("short_description") {
            doc.add_text(self.schema.short_description, short_desc);
        }

        writer.add_document(doc)
            .context("Failed to add document to index")?;

        Ok(())
    }

    /// Search KB documents
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let index = self.index.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Index not initialized"))?;

        let reader = index.reader()
            .context("Failed to create index reader")?;

        let searcher = reader.searcher();

        // Create query parser with multi-field search
        let query_parser = QueryParser::for_index(index, vec![
            self.schema.contents,
            self.schema.title,
            self.schema.keywords,
            self.schema.summary,
        ]);

        // Parse query
        let parsed_query = query_parser.parse_query(query)
            .map_err(|e| anyhow::anyhow!("Failed to parse query: {}", e))?;

        // Execute search with BM25 scoring
        let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(limit).order_by_score())
            .context("Failed to execute search")?;

        // Extract results
        let hits: Vec<SearchHit> = top_docs
            .into_iter()
            .map(|(score, doc_address)| {
                let doc = searcher.doc(doc_address).unwrap_or_default();
                SearchHit::from_tantivy_doc(doc, &self.schema, score)
            })
            .collect();

        Ok(hits)
    }

    /// Search with field boosting (advanced search)
    pub fn search_with_boost(&self, query: &str, title_boost: f32, content_boost: f32, limit: usize) -> Result<Vec<SearchHit>> {
        // For field boosting, we need to construct a boolean query
        // This is a simplified version - Tantivy supports complex query combinations
        let index = self.index.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Index not initialized"))?;

        let reader = index.reader()
            .context("Failed to create index reader")?;

        let searcher = reader.searcher();

        // Create query parser for title field (higher boost)
        let title_parser = QueryParser::for_index(index, vec![self.schema.title]);

        // Create query parser for contents field
        let content_parser = QueryParser::for_index(index, vec![self.schema.contents]);

        let title_query = title_parser.parse_query(query).ok();
        let content_query = content_parser.parse_query(query).ok();

        // Combine results manually with boosting
        let mut all_hits: HashMap<u32, SearchHit> = HashMap::new();

        if let Some(tq) = title_query {
            let title_docs = searcher.search(&tq, &TopDocs::with_limit(limit * 2).order_by_score()).ok().unwrap_or_default();
            for (score, addr) in title_docs {
                let boosted_score = score * title_boost;
                let doc = searcher.doc(addr).unwrap_or_default();
                let hit = SearchHit::from_tantivy_doc(doc, &self.schema, boosted_score);

                // Use doc address as key (simplified)
                all_hits.insert(addr.doc_id, hit);
            }
        }

        if let Some(cq) = content_query {
            let content_docs = searcher.search(&cq, &TopDocs::with_limit(limit * 2).order_by_score()).ok().unwrap_or_default();
            for (score, addr) in content_docs {
                let boosted_score = score * content_boost;
                let doc = searcher.doc(addr).unwrap_or_default();
                let hit = SearchHit::from_tantivy_doc(doc, &self.schema, boosted_score);

                // Merge or add
                all_hits.entry(addr.doc_id)
                    .and_modify(|existing| {
                        existing.score += boosted_score;
                    })
                    .or_insert(hit);
            }
        }

        // Sort by score and take top results
        let mut hits: Vec<SearchHit> = all_hits.into_values().collect();
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(limit);

        Ok(hits)
    }

    /// Get index statistics
    pub fn stats(&self) -> Result<IndexStats> {
        let index = self.index.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Index not initialized"))?;

        let reader = index.reader()
            .context("Failed to create index reader")?;

        let searcher = reader.searcher();

        Ok(IndexStats {
            num_docs: searcher.num_docs(),
            index_dir: self.index_dir.clone(),
        })
    }

    /// Check if index exists and is ready
    pub fn is_index_ready(&self) -> bool {
        // Check if index directory exists with tantivy meta file
        self.index_dir.exists() && self.index.is_some()
    }

    /// Clear index (delete all documents)
    pub fn clear_index(&mut self) -> Result<()> {
        if self.index_dir.exists() {
            fs::remove_dir_all(&self.index_dir)
                .context("Failed to clear index directory")?;
        }
        self.index = None;
        Ok(())
    }
}

/// A search hit result
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Relevance score (BM25)
    pub score: f32,
    /// Document URL
    pub url: Option<String>,
    /// Document title
    pub title: Option<String>,
    /// Document filepath
    pub filepath: Option<String>,
    /// Summary snippet
    pub summary: Option<String>,
    /// Keywords
    pub keywords: Option<String>,
    /// Relative URL
    pub relative_url: Option<String>,
}

impl SearchHit {
    /// Create from Tantivy document
    fn from_tantivy_doc(doc: tantivy::TantivyDocument, schema: &KbSchema, score: f32) -> Self {
        Self {
            score,
            url: doc.get_first(schema.url).and_then(|v| v.as_str().map(|s| s.to_string())),
            title: doc.get_first(schema.title).and_then(|v| v.as_str().map(|s| s.to_string())),
            filepath: doc.get_first(schema.filepath).and_then(|v| v.as_str().map(|s| s.to_string())),
            summary: doc.get_first(schema.summary).and_then(|v| v.as_str().map(|s| s.to_string())),
            keywords: doc.get_first(schema.keywords).and_then(|v| v.as_str().map(|s| s.to_string())),
            relative_url: doc.get_first(schema.relative_url).and_then(|v| v.as_str().map(|s| s.to_string())),
        }
    }
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    /// Number of documents in index
    pub num_docs: u64,
    /// Index directory path
    pub index_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_kb_schema_creation() {
        let (schema, _fields) = KbSchema::build_schema();

        // Schema should have all fields
        assert!(schema.get_field(KnowledgeBaseConstants::URL_FIELD).is_ok());
        assert!(schema.get_field(KnowledgeBaseConstants::TITLE_FIELD).is_ok());
        assert!(schema.get_field(KnowledgeBaseConstants::CONTENTS_FIELD).is_ok());
    }

    #[test]
    fn test_kb_indexer_creation() {
        let temp_dir = tempdir().unwrap();
        let indexer = KbIndexerService::new(temp_dir.path().to_path_buf());

        assert!(!indexer.is_index_ready());
    }

    #[test]
    fn test_kb_indexer_init() {
        let temp_dir = tempdir().unwrap();
        let mut indexer = KbIndexerService::new(temp_dir.path().to_path_buf());

        indexer.init_index().unwrap();
        assert!(indexer.is_index_ready());
    }

    #[test]
    fn test_kb_indexer_stats() {
        let temp_dir = tempdir().unwrap();
        let mut indexer = KbIndexerService::new(temp_dir.path().to_path_buf());

        indexer.init_index().unwrap();

        let stats = indexer.stats().unwrap();
        assert_eq!(stats.num_docs, 0);
    }

    #[test]
    fn test_add_document() {
        let temp_dir = tempdir().unwrap();
        let mut indexer = KbIndexerService::new(temp_dir.path().to_path_buf());

        indexer.init_index().unwrap();

        let index = indexer.index.as_ref().unwrap();
        let mut writer = index.writer(15_000_000).unwrap();

        let kb_doc = KbDocFile::from_markdown(
            "test.md".to_string(),
            "---\ntitle: Test Title\nurl: /test\nkeywords: android, test\n---\nTest content here".to_string()
        );

        indexer.add_document(&writer, &kb_doc).unwrap();
        writer.commit().unwrap();

        let reader = index.reader().unwrap();
        reader.reload().unwrap();

        let stats = indexer.stats().unwrap();
        assert_eq!(stats.num_docs, 1);
    }

    #[test]
    fn test_basic_search() {
        let temp_dir = tempdir().unwrap();
        let mut indexer = KbIndexerService::new(temp_dir.path().to_path_buf());

        indexer.init_index().unwrap();

        let index = indexer.index.as_ref().unwrap();
        let mut writer = index.writer(15_000_000).unwrap();

        // Add test documents
        let doc1 = KbDocFile::from_markdown(
            "doc1.md".to_string(),
            "---\ntitle: Android Basics\nurl: /android/basics\nkeywords: android, sdk\n---\nLearn about Android SDK basics and fundamentals".to_string()
        );
        indexer.add_document(&writer, &doc1).unwrap();

        let doc2 = KbDocFile::from_markdown(
            "doc2.md".to_string(),
            "---\ntitle: Kotlin Guide\nurl: /kotlin/guide\nkeywords: kotlin, programming\n---\nKotlin programming guide for Android developers".to_string()
        );
        indexer.add_document(&writer, &doc2).unwrap();

        writer.commit().unwrap();
        let reader = index.reader().unwrap();
        reader.reload().unwrap();

        // Search for "android"
        let results = indexer.search("android", 10).unwrap();
        assert!(!results.is_empty());

        // First result should be about Android (higher score)
        let first = &results[0];
        assert!(first.title.clone().unwrap_or_default().contains("Android"));
    }

    #[test]
    fn test_clear_index() {
        let temp_dir = tempdir().unwrap();
        let mut indexer = KbIndexerService::new(temp_dir.path().to_path_buf());

        indexer.init_index().unwrap();
        assert!(indexer.is_index_ready());

        indexer.clear_index().unwrap();
        assert!(!indexer.is_index_ready());
    }

    #[test]
    fn test_search_hit_creation() {
        let temp_dir = tempdir().unwrap();
        let mut indexer = KbIndexerService::new(temp_dir.path().to_path_buf());

        indexer.init_index().unwrap();

        let index = indexer.index.as_ref().unwrap();
        let mut writer = index.writer(15_000_000).unwrap();

        let kb_doc = KbDocFile::from_markdown(
            "test.md".to_string(),
            "---\ntitle: Test Title\nurl: /test/url\nkeywords: test keywords\n---\nTest content".to_string()
        );

        indexer.add_document(&writer, &kb_doc).unwrap();
        writer.commit().unwrap();

        let reader = index.reader().unwrap();
        reader.reload().unwrap();

        let results = indexer.search("test", 1).unwrap();
        assert_eq!(results.len(), 1);

        let hit = &results[0];
        assert!(hit.score > 0.0);
        assert_eq!(hit.title.as_deref(), Some("Test Title"));
        assert_eq!(hit.url.as_deref(), Some("/test/url"));
    }
}