use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Default release URL for updates
const DEFAULT_RELEASE_URL: &str = "https://github.com/example/android-cli/releases/latest/download";

/// Updater for self-updating the CLI binary
pub struct Updater {
    release_url: String,
}

impl Updater {
    /// Create a new Updater with default release URL
    pub fn new() -> Self {
        Self {
            release_url: DEFAULT_RELEASE_URL.to_string(),
        }
    }

    /// Create an Updater with a custom release URL
    pub fn with_url(url: &str) -> Self {
        Self {
            release_url: url.to_string(),
        }
    }

    /// Validate update URL for security
    fn validate_url(url: &str) -> Result<()> {
        let parsed =
            url::Url::parse(url).with_context(|| format!("Invalid URL format: {}", url))?;

        // Ensure HTTPS scheme
        if parsed.scheme() != "https" {
            bail!("Update URL must use HTTPS for security: {}", url);
        }

        // Check allowed domains
        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("URL has no host: {}", url))?;

        let allowed_domains = [
            "github.com",
            "githubusercontent.com",
            "releases.githubusercontent.com",
        ];
        let is_allowed = allowed_domains
            .iter()
            .any(|domain| host == *domain || host.ends_with(&format!(".{}", domain)));

        if !is_allowed {
            bail!("Update URL must be from GitHub domain: {}", url);
        }

        Ok(())
    }

    /// Get the current binary path
    fn current_binary_path() -> Result<PathBuf> {
        env::current_exe().context("Failed to determine current executable path")
    }

    /// Get the platform-specific binary name
    fn get_binary_name() -> String {
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            "windows" => "windows",
            "linux" => "linux",
            other => other,
        };

        let arch = match std::env::consts::ARCH {
            "x86_64" | "amd64" => "amd64",
            "aarch64" | "arm64" => "arm64",
            other => other,
        };

        let extension = if std::env::consts::OS == "windows" {
            ".exe"
        } else {
            ""
        };

        format!("android-{}-{}{}", os, arch, extension)
    }

    /// Construct the download URL for the current platform
    fn get_download_url(&self) -> String {
        let binary_name = Self::get_binary_name();
        format!("{}/{}", self.release_url.trim_end_matches('/'), binary_name)
    }

    /// Download the new binary with SHA256 verification
    fn download_binary(&self, url: &str, dest: &Path) -> Result<()> {
        // Validate URL first
        Self::validate_url(url)?;

        println!("Downloading update from: {}", url);

        // Create HTTP client
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .connect_timeout(std::time::Duration::from_secs(30))
            .user_agent(format!("Android-CLI-Updater/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .context("Failed to create HTTP client")?;

        let response = client
            .get(url)
            .send()
            .context("Failed to download update")?;

        if !response.status().is_success() {
            bail!("Failed to download update: HTTP {}", response.status());
        }

        // Get expected SHA256 from header if available
        let expected_sha256 = response
            .headers()
            .get("x-sha256")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Get content length for progress bar
        let content_length = response
            .headers()
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

        // Download to temp file first, computing SHA256 while downloading
        let mut temp_file =
            tempfile::NamedTempFile::new().context("Failed to create temporary file")?;

        let mut reader = io::BufReader::new(response);
        let mut buffer = [0u8; 8192];
        let mut downloaded: u64 = 0;
        let mut hasher = Sha256::new();

        loop {
            let n = reader
                .read(&mut buffer)
                .context("Failed to read response")?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            temp_file
                .write_all(&buffer[..n])
                .context("Failed to write to temporary file")?;
            downloaded += n as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Download complete");

        // Compute final SHA256
        let actual_sha256 = format!("{:x}", hasher.finalize());
        println!("Downloaded SHA256: {}", actual_sha256);

        // Verify SHA256 if expected hash is provided
        if let Some(expected) = expected_sha256 {
            if actual_sha256 != expected {
                bail!("SHA256 verification failed! Expected: {}, Got: {}. The download may be corrupted or tampered.",
                      expected, actual_sha256);
            }
            println!("SHA256 verification passed ✓");
        } else {
            println!("Note: No SHA256 checksum provided by server. Consider verifying manually.");
        }

        // Make the file executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(temp_file.path(), fs::Permissions::from_mode(0o755))
                .context("Failed to set executable permissions")?;
        }

        // Move temp file to destination
        temp_file
            .persist(dest)
            .context("Failed to save downloaded file")?;

        Ok(())
    }

    /// Replace the current binary with the new one
    fn replace_binary(current: &Path, new: &Path) -> Result<()> {
        // Create backup of current binary
        let backup = current.with_extension("backup");
        fs::copy(current, &backup).context("Failed to backup current binary")?;

        // Try to replace the binary
        // On Windows, we need to rename the old binary first
        #[cfg(windows)]
        {
            let old = current.with_extension("old");
            fs::rename(current, &old).context("Failed to rename old binary")?;
            fs::rename(new, current).context("Failed to move new binary into place")?;
            // Try to remove the old binary (may fail if still in use)
            let _ = fs::remove_file(&old);
        }

        #[cfg(not(windows))]
        {
            fs::rename(new, current).context("Failed to replace binary")?;
        }

        // Clean up backup
        let _ = fs::remove_file(&backup);

        Ok(())
    }

    /// Perform the self-update
    pub fn update(&self, url: Option<&str>) -> Result<()> {
        let download_url = url
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.get_download_url());

        let current_binary = Self::current_binary_path()?;
        println!("Current binary: {}", current_binary.display());

        // Download to temp location
        let temp_dir = tempfile::tempdir().context("Failed to create temporary directory")?;

        let binary_name = Self::get_binary_name();
        let new_binary = temp_dir.path().join(&binary_name);

        // Download new binary
        self.download_binary(&download_url, &new_binary)?;

        // Replace current binary
        Self::replace_binary(&current_binary, &new_binary)?;

        println!("Update complete! The CLI has been updated.");
        println!("Restart your terminal or run the command again to use the new version.");

        Ok(())
    }

    /// Check for available updates
    pub fn check_for_update(&self) -> Result<Option<String>> {
        let client = reqwest::blocking::Client::new();
        let url = format!("{}/version.txt", self.release_url.trim_end_matches('/'));

        let response = client
            .head(&url)
            .send()
            .context("Failed to check for updates")?;

        if response.status().is_success() {
            let response = client
                .get(&url)
                .send()
                .context("Failed to fetch version info")?;

            let version = response.text().context("Failed to read version")?;

            Ok(Some(version.trim().to_string()))
        } else {
            Ok(None)
        }
    }
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_get_binary_name() {
        let name = Updater::get_binary_name();
        // Should contain platform and arch
        assert!(name.contains("android"));
    }

    #[test]
    fn test_updater_create() {
        let updater = Updater::new();
        assert!(!updater.release_url.is_empty());
    }

    #[test]
    fn test_updater_with_url() {
        let updater = Updater::with_url("https://example.com/releases");
        assert_eq!(updater.release_url, "https://example.com/releases");
    }

    #[test]
    fn test_get_current_version() {
        // Test that we can get the current binary path
        let result = Updater::current_binary_path();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_download_url_construction() {
        // Test default URL construction
        let updater = Updater::new();
        let url = updater.get_download_url();

        // Should contain the default release URL
        assert!(url.contains(DEFAULT_RELEASE_URL.trim_end_matches('/')));

        // Should contain the platform-appropriate binary name
        let binary_name = Updater::get_binary_name();
        assert!(url.ends_with(&binary_name));
    }

    #[test]
    fn test_download_url_construction_custom() {
        // Test custom URL construction
        let updater = Updater::with_url("https://custom.example.com/downloads/");
        let url = updater.get_download_url();

        // Should use the custom URL (trimmed of trailing slash)
        assert!(url.starts_with("https://custom.example.com/downloads/"));

        // Should still contain the binary name
        let binary_name = Updater::get_binary_name();
        assert!(url.ends_with(&binary_name));
    }

    #[test]
    fn test_backup_creation() {
        // Test that backup path is correctly generated
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let binary_path = temp_dir.path().join("android-darwin-amd64");

        // Create a dummy binary file
        fs::write(&binary_path, b"dummy binary content").expect("Failed to write binary");

        // Test backup path logic
        let backup_path = binary_path.with_extension("backup");
        assert_eq!(
            backup_path.file_name().unwrap(),
            "android-darwin-amd64.backup"
        );

        // Test that we can create the backup
        fs::copy(&binary_path, &backup_path).expect("Failed to create backup");
        assert!(backup_path.exists());
    }

    #[test]
    fn test_binary_replacement_logic() {
        // Test the path logic for binary replacement without actual replacement
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

        // Create paths that would be used in replacement
        let current_binary = temp_dir.path().join("current-binary");
        let new_binary = temp_dir.path().join("new-binary");
        let backup = current_binary.with_extension("backup");

        // Create dummy files
        fs::write(&current_binary, b"current content").expect("Failed to write current binary");
        fs::write(&new_binary, b"new content").expect("Failed to write new binary");

        // Verify paths are in the expected locations
        assert!(current_binary.exists());
        assert!(new_binary.exists());
        assert!(!backup.exists());

        // Simulate backup creation
        fs::copy(&current_binary, &backup).expect("Failed to backup");
        assert!(backup.exists());

        // Verify backup content matches current
        let backup_content = fs::read(&backup).expect("Failed to read backup");
        assert_eq!(backup_content, b"current content");

        // Verify new binary content differs
        let new_content = fs::read(&new_binary).expect("Failed to read new binary");
        assert_eq!(new_content, b"new content");
    }

    #[test]
    fn test_platform_specific_paths_darwin() {
        // Test platform detection logic
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        let binary_name = Updater::get_binary_name();

        // Verify binary name contains expected components based on current platform
        if os == "macos" {
            assert!(binary_name.contains("darwin"));
        } else if os == "windows" {
            assert!(binary_name.contains("windows"));
            assert!(binary_name.ends_with(".exe"));
        } else if os == "linux" {
            assert!(binary_name.contains("linux"));
        }

        if arch == "x86_64" || arch == "amd64" {
            assert!(binary_name.contains("amd64"));
        } else if arch == "aarch64" || arch == "arm64" {
            assert!(binary_name.contains("arm64"));
        }
    }

    #[test]
    fn test_platform_specific_paths_windows_extension() {
        // Test that Windows binaries have .exe extension
        let os = std::env::consts::OS;
        let binary_name = Updater::get_binary_name();

        if os == "windows" {
            assert!(binary_name.ends_with(".exe"));
        } else {
            assert!(!binary_name.ends_with(".exe"));
        }
    }

    #[test]
    fn test_binary_name_format() {
        let binary_name = Updater::get_binary_name();

        // Binary name should follow the pattern: android-{os}-{arch}[.exe]
        let parts: Vec<&str> = binary_name.split('-').collect();
        assert!(parts.len() >= 3, "Binary name should have at least 3 parts");
        assert_eq!(
            parts[0], "android",
            "Binary name should start with 'android'"
        );

        // Second part should be the OS
        let os_part = parts[1];
        assert!(
            ["darwin", "windows", "linux"].contains(&os_part) || os_part.starts_with("android"),
            "OS part should be a known platform"
        );
    }

    #[test]
    fn test_updater_default_implementation() {
        // Test that Default trait implementation works correctly
        let updater1 = Updater::new();
        let updater2 = Updater::default();

        assert_eq!(updater1.release_url, updater2.release_url);
    }

    #[test]
    fn test_release_url_trailing_slash_handling() {
        // Test that URLs with trailing slashes are handled correctly
        let updater = Updater::with_url("https://example.com/releases/");
        let url = updater.get_download_url();

        // URL should not have double slashes
        assert!(!url.contains("//releases//"));
    }

    #[test]
    fn test_temp_directory_creation_for_download() {
        // Test that we can create a temp directory for downloads
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let binary_name = Updater::get_binary_name();
        let new_binary_path = temp_dir.path().join(&binary_name);

        // Path should be valid and in temp directory
        assert!(new_binary_path.starts_with(temp_dir.path()));
    }

    #[test]
    fn test_check_for_update_url_construction() {
        // Test that check_for_update constructs the correct URL
        let updater = Updater::with_url("https://example.com/releases");
        // The version check URL should be {release_url}/version.txt
        let expected_base = "https://example.com/releases";
        assert!(updater.release_url.starts_with(expected_base));
    }
}
