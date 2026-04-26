use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{Read, Write};
use sha2::{Sha256, Digest};
use indicatif::{ProgressBar, ProgressStyle};

use super::kb_doc::KbDownloadResult;

/// Knowledge base constants - matches Google's KnowledgeBaseConstants.kt
pub struct KnowledgeBaseConstants;

impl KnowledgeBaseConstants {
    /// KB ZIP download URL (from https://dl.google.com/dac/dac_kb.zip)
    pub const KB_ZIP_URL: &str = "https://dl.google.com/dac/dac_kb.zip";
    /// Ready file timeout in milliseconds
    pub const READY_FILE_TIMEOUT_MS: u64 = 30000;
    /// Ready file poll interval in milliseconds
    pub const READY_FILE_POLL_INTERVAL_MS: u64 = 500;
    /// Update interval in days (check for updates every 7 days)
    pub const UPDATE_INTERVAL_DAYS: u64 = 7;
    /// Field names
    pub const URL_FIELD: &str = "url";
    pub const RELATIVE_URL_FIELD: &str = "relative_url";
    pub const FILEPATH_FIELD: &str = "filepath";
    pub const FILENAME_FIELD: &str = "filename";
    pub const KEYWORDS_FIELD: &str = "keywords";
    pub const SHORT_DESCRIPTION_FIELD: &str = "short_description";
    pub const TITLE_FIELD: &str = "title";
    pub const CONTENTS_FIELD: &str = "contents";
    pub const SUMMARY_FIELD: &str = "summary";
    /// Search parameters
    pub const MAX_RESULTS_SEARCH: usize = 10;
    pub const MAX_RESULTS_SEARCH_AND_RETRIEVE: usize = 2;
    pub const TIMEOUT_MS: u64 = 20000;
    /// Boost factors for search
    pub const TITLE_BOOST: f32 = 10.0;
    pub const PHRASE_BOOST: f32 = 10.0;
    pub const MIN_OR_MATCHES: f32 = 0.4;
    /// Sentinel file name
    pub const SENTINEL_FILE_NAME: &str = "index_ready.json";
}

/// KB Download service - downloads KB ZIP from Google server
/// Matches Google's KBDownloadServiceImpl.kt
pub struct KBDownloadService {
    /// Storage directory for KB data (kbzip)
    storage_dir: PathBuf,
    /// HTTP client
    client: reqwest::blocking::Client,
    /// Path to the ZIP file (dac.zip)
    zip_file: PathBuf,
    /// Path to the ETag file (dac.etag)
    etag_file: PathBuf,
    /// Path to the timestamp file (last_update_check.timestamp)
    timestamp_file: PathBuf,
}

impl KBDownloadService {
    /// Create a new KB download service
    /// Matches Google's DocsCLI which uses .android/cli/docs/kbzip for storage
    pub fn new(storage_dir: PathBuf) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .connect_timeout(std::time::Duration::from_secs(30))
            .user_agent("Android-CLI-KB/0.1.0")
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .expect("Failed to create HTTP client");

        let zip_file = storage_dir.join("dac.zip");
        let etag_file = storage_dir.join("dac.etag");
        let timestamp_file = storage_dir.join("last_update_check.timestamp");

        Self {
            storage_dir,
            client,
            zip_file,
            etag_file,
            timestamp_file,
        }
    }

    /// Ensure storage directory exists
    fn ensure_storage_dir(&self) -> Result<()> {
        if !self.storage_dir.exists() {
            fs::create_dir_all(&self.storage_dir)
                .context("Failed to create KB storage directory")?;
        }
        Ok(())
    }

    /// Get or update KB ZIP file
    /// Matches Google's getOrUpdateZip implementation
    pub fn get_or_update_zip(&self) -> Result<KbDownloadResult> {
        self.ensure_storage_dir()?;

        // Check if we need to download or update
        let needs_download = !self.zip_file.exists() || self.should_check_for_update()?;

        if !needs_download {
            // Use existing ZIP
            let sha256 = self.compute_sha256(&self.zip_file)?;
            return Ok(KbDownloadResult {
                zip_path: self.zip_file.clone(),
                sha256,
                is_new: false,
            });
        }

        // Check for update using HEAD request and ETag
        if self.zip_file.exists() {
            self.check_for_update()?;

            // After check_for_update, the ZIP may have been updated
            if self.zip_file.exists() {
                let sha256 = self.compute_sha256(&self.zip_file)?;
                return Ok(KbDownloadResult {
                    zip_path: self.zip_file.clone(),
                    sha256,
                    is_new: true,
                });
            }
        }

        // Download new ZIP
        println!("Downloading Android Knowledge Base...");
        self.download_zip()?;

        // Update timestamp
        self.update_timestamp()?;

        let sha256 = self.compute_sha256(&self.zip_file)?;
        Ok(KbDownloadResult {
            zip_path: self.zip_file.clone(),
            sha256,
            is_new: true,
        })
    }

    /// Check if we should check for update (every UPDATE_INTERVAL_DAYS days)
    /// Matches Google's shouldCheckForUpdate implementation
    fn should_check_for_update(&self) -> Result<bool> {
        if !self.timestamp_file.exists() {
            return Ok(true);
        }

        // Try to read and parse timestamp, return true on error
        let check_result: Result<bool, anyhow::Error> = (|| {
            let last_check_str = fs::read_to_string(&self.timestamp_file)?;
            let last_check_millis: u64 = last_check_str.trim().parse()?;
            let update_interval_millis = KnowledgeBaseConstants::UPDATE_INTERVAL_DAYS * 24 * 60 * 60 * 1000;
            Ok(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis() as u64 - last_check_millis > update_interval_millis)
        })();

        match check_result {
            Ok(should_check) => Ok(should_check),
            Err(_) => Ok(true),
        }
    }

    /// Update timestamp file
    fn update_timestamp(&self) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();
        fs::write(&self.timestamp_file, now.to_string())
            .context("Failed to write timestamp file")?;
        Ok(())
    }

    /// Check for update using HEAD request and ETag
    /// Matches Google's checkForUpdate implementation
    fn check_for_update(&self) -> Result<()> {
        let response = self.client
            .head(KnowledgeBaseConstants::KB_ZIP_URL)
            .send()
            .context("Failed to check for KB updates")?;

        if !response.status().is_success() {
            eprintln!("Failed to check for updates. Status: {}", response.status());
            return Ok(());
        }

        let remote_etag = response.headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let local_etag = if self.etag_file.exists() {
            fs::read_to_string(&self.etag_file)?.trim().to_string()
        } else {
            "".to_string()
        };

        // If ETag matches, no update needed
        if let Some(etag) = &remote_etag {
            if etag == &local_etag {
                self.update_timestamp()?;
                return Ok(());
            }
        }

        println!("New version available. Downloading...");

        // Download to temp files first
        let temp_zip = self.storage_dir.join("dac_update.zip");
        let temp_etag = self.storage_dir.join("dac_update.etag");

        self.download_zip_to(&temp_zip, &temp_etag)?;

        // Atomically move to final location
        fs::rename(&temp_zip, &self.zip_file)
            .context("Failed to move updated ZIP")?;
        fs::rename(&temp_etag, &self.etag_file)
            .context("Failed to move updated ETag")?;

        println!("Knowledge Base zip updated successfully.");
        self.update_timestamp()?;

        Ok(())
    }

    /// Download KB ZIP from server
    fn download_zip(&self) -> Result<()> {
        self.download_zip_to(&self.zip_file, &self.etag_file)?;
        Ok(())
    }

    /// Download KB ZIP to specified paths
    fn download_zip_to(&self, target_zip: &Path, target_etag: &Path) -> Result<()> {
        // Get ETag via HEAD request
        let head_response = self.client
            .head(KnowledgeBaseConstants::KB_ZIP_URL)
            .send()
            .context("Failed to get KB metadata")?;

        let remote_etag = head_response.headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

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

        // Download to file
        let mut temp_file = fs::File::create(target_zip)
            .context("Failed to create ZIP file")?;

        let mut reader = std::io::BufReader::new(response);
        let mut buffer = [0u8; 8192];
        let mut downloaded: u64 = 0;

        loop {
            let n = reader.read(&mut buffer)
                .context("Failed to read response")?;
            if n == 0 {
                break;
            }
            temp_file.write_all(&buffer[..n])
                .context("Failed to write to file")?;
            downloaded += n as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Download complete");

        // Save ETag
        if let Some(etag) = remote_etag {
            fs::write(target_etag, etag)
                .context("Failed to write ETag file")?;
        }

        println!("KB downloaded to: {}", target_zip.display());

        Ok(())
    }

    /// Compute SHA256 of a file
    fn compute_sha256(&self, path: &Path) -> Result<String> {
        let mut file = fs::File::open(path)
            .with_context(|| format!("Failed to open file: {}", path.display()))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let n = file.read(&mut buffer)
                .context("Failed to read file")?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Create ready file (index_ready.json) to signal index completion
    pub fn create_ready_file(&self, sha256: &str) -> Result<()> {
        let ready_path = self.storage_dir.join(KnowledgeBaseConstants::SENTINEL_FILE_NAME);
        fs::write(&ready_path, sha256)
            .context("Failed to write ready file")?;
        Ok(())
    }

    /// Check if ready file exists
    pub fn is_ready(&self) -> Result<bool> {
        Ok(self.storage_dir.join(KnowledgeBaseConstants::SENTINEL_FILE_NAME).exists())
    }

    /// Wait for ready file with timeout
    pub fn wait_for_ready(&self, timeout_ms: u64) -> Result<Option<String>> {
        let ready_path = self.storage_dir.join(KnowledgeBaseConstants::SENTINEL_FILE_NAME);
        let start = std::time::Instant::now();

        while start.elapsed().as_millis() < timeout_ms as u128 {
            if ready_path.exists() {
                let content = fs::read_to_string(&ready_path)
                    .context("Failed to read ready file")?;
                return Ok(Some(content));
            }
            std::thread::sleep(std::time::Duration::from_millis(
                KnowledgeBaseConstants::READY_FILE_POLL_INTERVAL_MS
            ));
        }

        Ok(None)
    }

    /// Get storage directory
    pub fn storage_dir(&self) -> &Path {
        &self.storage_dir
    }

    /// Get ZIP file path
    pub fn zip_path(&self) -> &Path {
        &self.zip_file
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
        assert!(service.storage_dir.exists());
    }

    #[test]
    fn test_ensure_storage_dir() {
        let temp_dir = tempdir().unwrap();
        let new_dir = temp_dir.path().join("kb_cache");
        let service = KBDownloadService::new(new_dir.clone());

        service.ensure_storage_dir().unwrap();
        assert!(new_dir.exists());
    }

    #[test]
    fn test_zip_file_path() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        let zip_path = service.zip_file;
        assert_eq!(zip_path.file_name().unwrap(), "dac.zip");
    }

    #[test]
    fn test_etag_file_path() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        assert_eq!(service.etag_file.file_name().unwrap(), "dac.etag");
    }

    #[test]
    fn test_timestamp_file_path() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        assert_eq!(service.timestamp_file.file_name().unwrap(), "last_update_check.timestamp");
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
    fn test_should_check_for_update_no_file() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        // No timestamp file means should check
        let should_check = service.should_check_for_update().unwrap();
        assert!(should_check);
    }

    #[test]
    fn test_should_check_for_update_recent() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        // Write current timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        fs::write(&service.timestamp_file, now.to_string()).unwrap();

        // Recent timestamp means no need to check
        let should_check = service.should_check_for_update().unwrap();
        assert!(!should_check);
    }

    #[test]
    fn test_should_check_for_update_old() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        // Write old timestamp (10 days ago)
        let old_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() - (10 * 24 * 60 * 60 * 1000);
        fs::write(&service.timestamp_file, old_time.to_string()).unwrap();

        // Old timestamp means should check
        let should_check = service.should_check_for_update().unwrap();
        assert!(should_check);
    }

    #[test]
    fn test_update_timestamp() {
        let temp_dir = tempdir().unwrap();
        let service = KBDownloadService::new(temp_dir.path().to_path_buf());

        service.update_timestamp().unwrap();
        assert!(service.timestamp_file.exists());

        let content = fs::read_to_string(&service.timestamp_file).unwrap();
        let _: u64 = content.parse().unwrap(); // Should be valid number
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
        assert!(KnowledgeBaseConstants::KB_ZIP_URL.starts_with("https://dl.google.com/dac/"));
        assert!(!KnowledgeBaseConstants::URL_FIELD.is_empty());
        assert_eq!(KnowledgeBaseConstants::MAX_RESULTS_SEARCH, 10);
        assert_eq!(KnowledgeBaseConstants::TITLE_BOOST, 10.0);
        assert_eq!(KnowledgeBaseConstants::SENTINEL_FILE_NAME, "index_ready.json");
    }
}