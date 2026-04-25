use std::path::{Path, PathBuf};
use std::fs;
use std::io::{BufReader, Read, Write};
use std::time::Duration;
use anyhow::{Result, Context};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ETAG, IF_NONE_MATCH, CONTENT_LENGTH};

/// HTTP downloader with ETag caching support
pub struct Downloader {
    client: Client,
    proxy: Option<String>,
}

impl Downloader {
    /// Create a new downloader
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(30))
            .user_agent("Android-CLI-Rust/0.1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            proxy: None,
        })
    }

    /// Create downloader with proxy
    pub fn with_proxy(proxy: &str) -> Result<Self> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(30))
            .user_agent("Android-CLI-Rust/0.1.0");

        // Parse proxy URL
        if proxy.starts_with("http://") || proxy.starts_with("https://") {
            builder = builder.proxy(reqwest::Proxy::all(proxy)
                .context("Failed to configure proxy")?);
        }

        let client = builder.build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            proxy: Some(proxy.to_string()),
        })
    }

    /// Download a file, returning new ETag if changed (None if unchanged)
    pub fn download_with_etag(
        &self,
        url: &str,
        dest: &Path,
        etag_file: Option<&Path>,
    ) -> Result<Option<String>> {
        // Check existing ETag
        let existing_etag = etag_file.and_then(|f| {
            if f.exists() {
                fs::read_to_string(f).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        });

        // Build request with ETag header
        let mut headers = HeaderMap::new();
        if let Some(etag) = &existing_etag {
            headers.insert(IF_NONE_MATCH, HeaderValue::from_str(etag)
                .context("Invalid ETag header")?);
        }

        let response = self.client
            .get(url)
            .headers(headers)
            .send()
            .context("Failed to send HTTP request")?;

        // Check if unchanged (304 Not Modified)
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(None);
        }

        // Check for success
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP error: {} for URL: {}",
                response.status(),
                url
            ));
        }

        // Get new ETag
        let new_etag = response.headers()
            .get(ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Get content length for progress bar
        let content_length = response.headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        // Setup progress bar
        let pb = if let Some(len) = content_length {
            ProgressBar::new(len)
        } else {
            ProgressBar::new_spinner()
        };

        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"));

        pb.set_message("Downloading");

        // Create parent directories
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        // Download with progress - use BufReader for streaming
        let mut file = fs::File::create(dest)
            .with_context(|| format!("Failed to create file: {}", dest.display()))?;

        let mut reader = BufReader::new(response);
        let mut buffer = [0u8; 8192];
        let mut downloaded: u64 = 0;

        loop {
            let n = reader.read(&mut buffer)
                .context("Failed to read response")?;
            if n == 0 {
                break;
            }
            file.write_all(&buffer[..n])
                .context("Failed to write to file")?;
            downloaded += n as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Download complete");

        // Save new ETag
        if let (Some(etag), Some(etag_file)) = (&new_etag, etag_file) {
            fs::write(etag_file, etag)
                .with_context(|| format!("Failed to write ETag file: {}", etag_file.display()))?;
        }

        Ok(new_etag)
    }

    /// Download a file without ETag
    pub fn download(&self, url: &str, dest: &Path) -> Result<PathBuf> {
        self.download_with_etag(url, dest, None)?;
        Ok(dest.to_path_buf())
    }

    /// Fetch URL content as bytes
    pub fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let response = self.client
            .get(url)
            .send()
            .context("Failed to fetch URL")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP error: {} for URL: {}",
                response.status(),
                url
            ));
        }

        let bytes = response.bytes()
            .context("Failed to read response body")?;

        Ok(bytes.to_vec())
    }

    /// Fetch URL content as text
    pub fn fetch_text(&self, url: &str) -> Result<String> {
        let bytes = self.fetch_bytes(url)?;
        String::from_utf8(bytes)
            .context("Response is not valid UTF-8")
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new().expect("Failed to create default downloader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_downloader_create() {
        let downloader = Downloader::new();
        assert!(downloader.is_ok());
    }

    // Note: Real download tests require network access
    // They should be integration tests, not unit tests
}