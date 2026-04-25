use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{Read, Write};
use sha2::{Sha256, Digest};
use indicatif::{ProgressBar, ProgressStyle};

use super::kb_doc::KbDownloadResult;

/// Knowledge base constants
pub struct KnowledgeBaseConstants;

impl KnowledgeBaseConstants {
    /// KB ZIP download URL
    pub const KB_ZIP_URL: &str = "https://developer.android.com/static/api/kb/kb.zip";
    /// Ready file timeout in milliseconds
    pub const READY_FILE_TIMEOUT_MS: u64 = 30000;
    /// Field names
    pub const URL_FIELD: &str = "url";
    pub const RELATIVE_URL_FIELD: &str = "relative_url";
    pub const FILEPATH_FIELD: &str = "filepath";
    pub const KEYWORDS_FIELD: &str = "keywords";
    pub const SHORT_DESCRIPTION_FIELD: &str = "short_description";
    pub const TITLE_FIELD: &str = "title";
    pub const CONTENTS_FIELD: &str = "contents";
    pub const SUMMARY_FIELD: &str = "summary";
}

/// KB Download service - downloads KB ZIP from server
pub struct KBDownloadService {
    /// Cache directory for KB data
    cache_dir: PathBuf,
    /// HTTP client
    client: reqwest::blocking::Client,
}

impl KBDownloadService {
    /// Create a new KB download service
    pub fn new(cache_dir: PathBuf) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .connect_timeout(std::time::Duration::from_secs(30))
            .user_agent("Android-CLI-KB/0.1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            cache_dir,
            client,
        }
    }

    /// Ensure cache directory exists
    fn ensure_cache_dir(&self) -> Result<()> {
        if !self.cache_dir.exists() {
            fs::create_dir_all(&self.cache_dir)
                .context("Failed to create KB cache directory")?;
        }
        Ok(())
    }

    /// Get KB ZIP file path
    fn kb_zip_path(&self) -> PathBuf {
        self.cache_dir.join("kb.zip")
    }

    /// Get ready file path (signals index is ready)
    fn ready_file_path(&self) -> PathBuf {
        self.cache_dir.join("kb.ready")
    }

    /// Get or update KB ZIP file
    pub fn get_or_update_zip(&self) -> Result<KbDownloadResult> {
        self.ensure_cache_dir()?;

        let zip_path = self.kb_zip_path();

        // Check if we need to download
        let needs_download = !zip_path.exists() || self.is_zip_stale(&zip_path)?;

        if !needs_download {
            // Use existing ZIP
            let sha256 = self.compute_sha256(&zip_path)?;
            return Ok(KbDownloadResult {
                zip_path,
                sha256,
                is_new: false,
            });
        }

        // Download new ZIP
        println!("Downloading Android KB...");
        self.download_kb_zip()?;

        let sha256 = self.compute_sha256(&zip_path)?;
        Ok(KbDownloadResult {
            zip_path,
            sha256,
            is_new: true,
        })
    }

    /// Check if existing ZIP is stale (older than 24 hours)
    fn is_zip_stale(&self, zip_path: &Path) -> Result<bool> {
        if !zip_path.exists() {
            return Ok(true);
        }

        let metadata = fs::metadata(zip_path)
            .context("Failed to read ZIP metadata")?;

        let modified = metadata.modified()
            .context("Failed to get modification time")?;

        let elapsed = modified.elapsed()
            .context("Failed to calculate elapsed time")?;

        // Consider stale after 24 hours
        Ok(elapsed.as_secs() > 24 * 60 * 60)
    }

    /// Download KB ZIP from server
    fn download_kb_zip(&self) -> Result<()> {
        let zip_path = self.kb_zip_path();

        // Create temp file for download
        let temp_path = zip_path.with_extension("tmp");

        // Download with progress
        let response = self.client
            .get(KnowledgeBaseConstants::KB_ZIP_URL)
            .send()
            .context("Failed to start KB download")?;

        if !response.status().is_success() {
            bail!("KB download failed: HTTP {}", response.status());
        }

        let content_length = response.headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        let pb = if let Some(len) = content_length {
            ProgressBar::new(len)
        } else {
            ProgressBar::new_spinner()
        };

        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("#>-"),
        );

        // Download to temp file
        let mut temp_file = fs::File::create(&temp_path)
            .context("Failed to create temp file")?;

        let mut reader = std::io::BufReader::new(response);
        let mut buffer = [0u8; 8192];
        let mut downloaded: u64 = 0;

        loop {
            let n = reader.read(&mut buffer)
                .context("Failed to read response")?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut temp_file, &buffer[..n])
                .context("Failed to write to temp file")?;
            downloaded += n as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Download complete");

        // Move temp file to final location
        fs::rename(&temp_path, &zip_path)
            .context("Failed to move downloaded ZIP")?;

        println!("KB downloaded to: {}", zip_path.display());

        Ok(())
    }

    /// Compute SHA256 of a file
    fn compute_sha256(&self, path: &Path) -> Result<String> {
        let mut file = fs::File::open(path)
            .with_context(|| format!("Failed to open file: {}", path.display()))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let n = std::io::Read::read(&mut file, &mut buffer)
                .context("Failed to read file")?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Create ready file to signal index completion
    pub fn create_ready_file(&self, sha256: &str) -> Result<()> {
        let ready_path = self.ready_file_path();
        fs::write(&ready_path, sha256)
            .context("Failed to write ready file")?;
        Ok(())
    }

    /// Check if ready file exists
    pub fn is_ready(&self) -> Result<bool> {
        Ok(self.ready_file_path().exists())
    }

    /// Wait for ready file with timeout
    pub fn wait_for_ready(&self, timeout_ms: u64) -> Result<Option<String>> {
        let ready_path = self.ready_file_path();
        let start = std::time::Instant::now();

        while start.elapsed().as_millis() < timeout_ms as u128 {
            if ready_path.exists() {
                let content = fs::read_to_string(&ready_path)
                    .context("Failed to read ready file")?;
                return Ok(Some(content));
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        Ok(None)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_kb_download_service_creation() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());
        assert!(service.cache_dir.exists());
    }

    #[test]
    fn test_ensure_cache_dir() {
        let temp_dir = tempdir().unwrap();
        let new_dir = temp_dir.path().join("kb_cache");
        let service = KBDownloadService::new(new_dir.clone());

        service.ensure_cache_dir().unwrap();
        assert!(new_dir.exists());
    }

    #[test]
    fn test_kb_zip_path() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        let zip_path = service.kb_zip_path();
        assert_eq!(zip_path.file_name().unwrap(), "kb.zip");
    }

    #[test]
    fn test_ready_file_path() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        let ready_path = service.ready_file_path();
        assert_eq!(ready_path.file_name().unwrap(), "kb.ready");
    }

    #[test]
    fn test_create_and_check_ready_file() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        assert!(!service.is_ready().unwrap());

        service.create_ready_file("test_sha256").unwrap();
        assert!(service.is_ready().unwrap());
    }

    #[test]
    fn test_wait_for_ready() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        // Not ready initially
        let result = service.wait_for_ready(1000).unwrap();
        assert!(result.is_none());

        // Create ready file
        service.create_ready_file("test_sha").unwrap();

        // Should return immediately now
        let result = service.wait_for_ready(1000).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "test_sha");
    }

    #[test]
    fn test_is_zip_stale() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        // Non-existent file is stale
        let stale = service.is_zip_stale(&temp_dir.path().join("nonexistent.zip")).unwrap();
        assert!(stale);

        // Create a new file - not stale
        let zip_path = temp_dir.path().join("kb.zip");
        fs::write(&zip_path, "test content").unwrap();
        let stale = service.is_zip_stale(&zip_path).unwrap();
        assert!(!stale);
    }

    #[test]
    fn test_compute_sha256() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        // Create test file
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "hello world").unwrap();

        let sha256 = service.compute_sha256(&test_file).unwrap();
        // SHA256 of "hello world" is known
        assert_eq!(sha256.len(), 64); // SHA256 hex length
    }

    #[test]
    fn test_constants() {
        assert!(KnowledgeBaseConstants::KB_ZIP_URL.starts_with("https://"));
        assert!(!KnowledgeBaseConstants::URL_FIELD.is_empty());
    }
}