//! ARM/custom SDK download from GitHub releases
//!
//! Supports downloading Android SDK from HomuHomu833/android-sdk-custom
//! which provides musl-based builds for various architectures.
//!
//! Features parallel multi-threaded download for faster downloads.

use anyhow::{Result, Context, anyhow};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Write, Read, Seek};
use std::sync::{Arc, Mutex};
use std::thread;

/// GitHub repository for custom Android SDK builds
const GITHUB_REPO: &str = "HomuHomu833/android-sdk-custom";
const GITHUB_API_URL: &str = "https://api.github.com/repos/HomuHomu833/android-sdk-custom/releases";

/// Custom SDK download URL pattern
const DOWNLOAD_URL_PATTERN: &str = "https://github.com/HomuHomu833/android-sdk-custom/releases/download/{version}/android-sdk-{arch}-linux-musl.tar.xz";

/// Default number of parallel download threads
const DEFAULT_THREADS: usize = 4;

/// Minimum chunk size for parallel download (5 MB)
const MIN_CHUNK_SIZE: u64 = 5 * 1024 * 1024;

/// Supported architectures for custom SDK
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomArch {
    Aarch64,
    Aarch64Be,
    Armhf,
    Arm,
    Armeb,
    ArmebHf,
    X86,
    X86_64,
    Riscv32,
    Riscv64,
    Loongarch64,
    Powerpc64le,
    S390x,
}

impl CustomArch {
    /// Get architecture string for download URL
    pub fn as_str(&self) -> &'static str {
        match self {
            CustomArch::Aarch64 => "aarch64",
            CustomArch::Aarch64Be => "aarch64_be",
            CustomArch::Armhf => "arm-linux-musleabihf",
            CustomArch::Arm => "arm-linux-musleabi",
            CustomArch::Armeb => "armeb-linux-musleabi",
            CustomArch::ArmebHf => "armeb-linux-musleabihf",
            CustomArch::X86 => "x86",
            CustomArch::X86_64 => "x86_64",
            CustomArch::Riscv32 => "riscv32",
            CustomArch::Riscv64 => "riscv64",
            CustomArch::Loongarch64 => "loongarch64",
            CustomArch::Powerpc64le => "powerpc64le",
            CustomArch::S390x => "s390x",
        }
    }

    /// Detect current architecture
    pub fn current() -> Option<Self> {
        match std::env::consts::ARCH {
            "aarch64" | "arm64" => Some(CustomArch::Aarch64),
            "x86_64" | "amd64" => Some(CustomArch::X86_64),
            "x86" | "i686" => Some(CustomArch::X86),
            "arm" => Some(CustomArch::Armhf),
            "riscv64" => Some(CustomArch::Riscv64),
            "loongarch64" => Some(CustomArch::Loongarch64),
            "powerpc64le" => Some(CustomArch::Powerpc64le),
            "s390x" => Some(CustomArch::S390x),
            _ => None,
        }
    }

    /// List all supported architectures
    pub fn all() -> &'static [CustomArch] {
        // Order matters: longer/more specific strings first
        // to avoid substring matching issues (x86_64 vs x86)
        &[
            CustomArch::Aarch64Be,  // aarch64_be before aarch64
            CustomArch::Aarch64,
            CustomArch::ArmebHf,    // armeb before armeb
            CustomArch::Armeb,
            CustomArch::Armhf,      // armhf contains "arm" and "arm-linux-musleabihf"
            CustomArch::Arm,        // arm-linux-musleabi
            CustomArch::X86_64,     // x86_64 before x86
            CustomArch::X86,
            CustomArch::Riscv64,    // riscv64 before riscv32
            CustomArch::Riscv32,
            CustomArch::Loongarch64,
            CustomArch::Powerpc64le,
            CustomArch::S390x,
        ]
    }
}

impl std::fmt::Display for CustomArch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Release version info
#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

/// Asset info from release
#[derive(Debug, Clone)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub arch: Option<CustomArch>,
}

impl Asset {
    /// Parse architecture from asset name
    fn parse_arch(name: &str) -> Option<CustomArch> {
        for arch in CustomArch::all() {
            if name.contains(arch.as_str()) {
                return Some(*arch);
            }
        }
        None
    }
}

/// Custom SDK downloader
pub struct CustomSdkDownloader {
    client: reqwest::blocking::Client,
    threads: usize,
}

/// Download progress tracker
struct DownloadProgress {
    total: u64,
    downloaded: u64,
    chunks_completed: usize,
    total_chunks: usize,
}

impl CustomSdkDownloader {
    pub fn new() -> Result<Self> {
        Self::with_threads(DEFAULT_THREADS)
    }

    /// Create downloader with specified number of threads
    pub fn with_threads(threads: usize) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client, threads })
    }

    /// Set number of download threads
    pub fn set_threads(&mut self, threads: usize) {
        self.threads = threads;
    }

    /// Fetch available releases from GitHub
    pub fn fetch_releases(&self) -> Result<Vec<Release>> {
        let response = self.client
            .get(GITHUB_API_URL)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .context("Failed to fetch releases from GitHub")?;

        if !response.status().is_success() {
            return Err(anyhow!("GitHub API returned status: {}", response.status()));
        }

        let body = response.text()
            .context("Failed to read response body")?;

        let releases_json: Vec<serde_json::Value> = serde_json::from_str(&body)
            .context("Failed to parse releases JSON")?;

        let releases = releases_json
            .into_iter()
            .map(|r| self.parse_release(r))
            .filter_map(|r| r)
            .collect();

        Ok(releases)
    }

    /// Parse a release from JSON
    fn parse_release(&self, json: serde_json::Value) -> Option<Release> {
        let tag_name = json["tag_name"].as_str()?.to_string();
        let version = tag_name.clone();

        let assets = json["assets"]
            .as_array()?
            .iter()
            .filter_map(|a| self.parse_asset(a))
            .collect();

        Some(Release {
            version,
            tag_name,
            assets,
        })
    }

    /// Parse an asset from JSON
    fn parse_asset(&self, json: &serde_json::Value) -> Option<Asset> {
        let name = json["name"].as_str()?.to_string();
        let url = json["browser_download_url"].as_str()?.to_string();
        let size = json["size"].as_u64()?;

        let arch = Asset::parse_arch(&name);

        Some(Asset {
            name,
            url,
            size,
            arch,
        })
    }

    /// Get download URL for specific version and architecture
    pub fn get_download_url(version: &str, arch: CustomArch) -> String {
        DOWNLOAD_URL_PATTERN
            .replace("{version}", version)
            .replace("{arch}", arch.as_str())
    }

    /// Download and extract SDK with parallel download
    pub fn download_and_extract(&self, version: &str, arch: CustomArch, sdk_path: &Path) -> Result<()> {
        let url = Self::get_download_url(version, arch);

        println!("Downloading Android SDK {} for {}...", version, arch);
        println!("URL: {}", url);
        println!("Threads: {}", self.threads);

        // Create temp directory for download
        let temp_dir = tempfile::tempdir()
            .context("Failed to create temp directory")?;

        let archive_path = temp_dir.path().join(format!("android-sdk-{}.tar.xz", arch.as_str()));

        // Get file size first
        let head_response = self.client
            .head(&url)
            .send()
            .context("Failed to check file size")?;

        if !head_response.status().is_success() {
            return Err(anyhow!("Failed to check file: {}", head_response.status()));
        }

        let total_size = head_response.content_length()
            .ok_or_else(|| anyhow!("Server did not provide file size"))?;

        // Check if server supports range requests
        let accepts_ranges = head_response.headers()
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "bytes")
            .unwrap_or(false);

        println!("File size: {:.1} MB", total_size as f64 / 1024.0 / 1024.0);
        println!("Range requests: {}", if accepts_ranges { "supported" } else { "not supported" });

        // Download with parallel chunks if supported, otherwise single-thread
        if accepts_ranges && total_size >= MIN_CHUNK_SIZE && self.threads > 1 {
            self.download_parallel(&url, &archive_path, total_size)?;
        } else {
            self.download_single(&url, &archive_path)?;
        }

        println!("Download complete!");

        // Extract tar.xz
        println!("Extracting SDK to {}...", sdk_path.display());

        // Create SDK directory if it doesn't exist
        if !sdk_path.exists() {
            fs::create_dir_all(sdk_path)
                .context("Failed to create SDK directory")?;
        }

        // Use tar command to extract (tar.xz format)
        let status = std::process::Command::new("tar")
            .arg("-xf")
            .arg(&archive_path)
            .arg("-C")
            .arg(sdk_path)
            .arg("--strip-components=1")
            .status()
            .context("Failed to run tar command")?;

        if !status.success() {
            return Err(anyhow!("Failed to extract archive"));
        }

        println!("SDK installed successfully to {}", sdk_path.display());

        Ok(())
    }

    /// Download file in parallel chunks
    fn download_parallel(&self, url: &str, output_path: &Path, total_size: u64) -> Result<()> {
        let chunk_size = (total_size / self.threads as u64).max(MIN_CHUNK_SIZE);
        let num_chunks = ((total_size / chunk_size) + 1) as usize;

        println!("Downloading {} chunks in parallel...", num_chunks);

        // Create output file
        let output_file = fs::File::create(output_path)
            .context("Failed to create output file")?;

        // Pre-allocate file space
        output_file.set_len(total_size)
            .context("Failed to pre-allocate file")?;

        let progress = Arc::new(Mutex::new(DownloadProgress {
            total: total_size,
            downloaded: 0,
            chunks_completed: 0,
            total_chunks: num_chunks,
        }));

        let output_path_arc = Arc::new(output_path.to_path_buf());
        let url_arc = Arc::new(url.to_string());
        let threads = Arc::new(self.threads);

        // Spawn download threads
        let mut handles = Vec::new();

        for chunk_idx in 0..num_chunks {
            let start = chunk_idx as u64 * chunk_size;
            let end = ((chunk_idx as u64 + 1) * chunk_size).min(total_size) - 1;

            let url = url_arc.clone();
            let output_path = output_path_arc.clone();
            let progress = progress.clone();

            let handle = thread::spawn(move || {
                download_chunk(&url, start, end, chunk_idx, &output_path, &progress)
            });

            handles.push(handle);
        }

        // Wait for all threads and check for errors
        let mut errors = Vec::new();
        for (idx, handle) in handles.into_iter().enumerate() {
            match handle.join() {
                Ok(result) => {
                    if let Err(e) = result {
                        errors.push((idx, e));
                    }
                }
                Err(_) => {
                    errors.push((idx, anyhow!("Thread panicked")));
                }
            }
        }

        if !errors.is_empty() {
            for (idx, e) in &errors {
                println!("Chunk {} failed: {}", idx, e);
            }
            return Err(anyhow!("{} chunks failed to download", errors.len()));
        }

        Ok(())
    }

    /// Single-threaded download (fallback)
    fn download_single(&self, url: &str, output_path: &Path) -> Result<()> {
        println!("Downloading with single thread...");

        let mut response = self.client
            .get(url)
            .send()
            .context("Failed to start download")?;

        if !response.status().is_success() {
            return Err(anyhow!("Download failed with status: {}", response.status()));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut file = fs::File::create(output_path)
            .context("Failed to create output file")?;

        let mut downloaded: u64 = 0;
        let mut buffer = [0u8; 8192];

        loop {
            let bytes = response.read(&mut buffer)?;
            if bytes == 0 {
                break;
            }
            file.write_all(&buffer[..bytes])?;
            downloaded += bytes as u64;

            if total_size > 0 && downloaded % (1024 * 1024 * 5) == 0 {
                let percent = (downloaded * 100 / total_size) as u32;
                println!("  Progress: {}% ({:.1} MB / {:.1} MB)",
                    percent,
                    downloaded as f64 / 1024.0 / 1024.0,
                    total_size as f64 / 1024.0 / 1024.0);
            }
        }

        println!("  Downloaded: {:.1} MB", downloaded as f64 / 1024.0 / 1024.0);

        Ok(())
    }

    /// List available versions
    pub fn list_versions(&self) -> Result<()> {
        let releases = self.fetch_releases()?;

        println!("Available Android SDK versions from {}:", GITHUB_REPO);
        println!();

        for release in releases {
            println!("Version: {}", release.version);
            println!("  Available architectures:");
            for asset in release.assets {
                if let Some(arch) = asset.arch {
                    println!("    - {} ({:.1} MB)", arch, asset.size as f64 / 1024.0 / 1024.0);
                }
            }
            println!();
        }

        Ok(())
    }

    /// Install SDK with auto-detection
    pub fn install(&self, version: Option<&str>, arch: Option<CustomArch>, sdk_path: &Path) -> Result<()> {
        // Get releases
        let releases = self.fetch_releases()?;

        if releases.is_empty() {
            return Err(anyhow!("No releases found"));
        }

        // Select version (latest by default)
        let selected_version = version.map(|v| v.to_string())
            .unwrap_or_else(|| releases[0].version.clone());

        // Select architecture (current by default)
        let selected_arch = arch.or_else(|| CustomArch::current())
            .ok_or_else(|| anyhow!("Could not detect architecture. Please specify --arch"))?;

        // Verify version exists
        let release = releases.iter()
            .find(|r| r.version == selected_version || r.tag_name == selected_version)
            .ok_or_else(|| anyhow!("Version {} not found. Available: {}",
                selected_version,
                releases.iter().map(|r| r.version.as_str()).collect::<Vec<_>>().join(", ")))?;

        // Verify architecture available for this version
        let asset = release.assets.iter()
            .find(|a| a.arch == Some(selected_arch))
            .ok_or_else(|| anyhow!("Architecture {} not available for version {}. Available: {}",
                selected_arch,
                selected_version,
                release.assets.iter()
                    .filter_map(|a| a.arch.map(|arch| arch.to_string()))
                    .collect::<Vec<_>>()
                    .join(", ")))?;

        println!("Installing Android SDK:");
        println!("  Version: {}", selected_version);
        println!("  Architecture: {}", selected_arch);
        println!("  Size: {:.1} MB", asset.size as f64 / 1024.0 / 1024.0);
        println!("  Target: {}", sdk_path.display());
        println!("  Source: https://github.com/HomuHomu833/android-sdk-custom");
        println!();

        self.download_and_extract(&selected_version, selected_arch, sdk_path)?;

        Ok(())
    }
}

/// Download a single chunk (used by parallel download threads)
fn download_chunk(
    url: &str,
    start: u64,
    end: u64,
    chunk_idx: usize,
    output_path: &Path,
    progress: &Mutex<DownloadProgress>,
) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .context("Failed to create HTTP client for chunk")?;

    let range_header = format!("bytes={}-{}", start, end);

    let response = client
        .get(url)
        .header("Range", range_header)
        .send()
        .context("Failed to request chunk")?;

    if !response.status().is_success() {
        return Err(anyhow!("Chunk {} download failed: {}", chunk_idx, response.status()));
    }

    // Open file and seek to chunk position
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(output_path)
        .context("Failed to open output file")?;

    file.seek(io::SeekFrom::Start(start))
        .context("Failed to seek to chunk position")?;

    // Download chunk data - need mutable response for reading
    let mut response = response;
    let mut buffer = [0u8; 8192];
    let mut chunk_downloaded: u64 = 0;
    let chunk_size = end - start + 1;

    loop {
        let bytes = response.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        file.write_all(&buffer[..bytes])?;
        chunk_downloaded += bytes as u64;

        // Update shared progress
        {
            let mut p = progress.lock().unwrap();
            p.downloaded += bytes as u64;

            // Print progress periodically
            if p.downloaded % (1024 * 1024 * 10) == 0 {
                let percent = (p.downloaded * 100 / p.total) as u32;
                println!("  Overall: {}% ({:.1} MB / {:.1} MB) - {} chunks done",
                    percent,
                    p.downloaded as f64 / 1024.0 / 1024.0,
                    p.total as f64 / 1024.0 / 1024.0,
                    p.chunks_completed);
            }
        }
    }

    // Mark chunk as completed
    {
        let mut p = progress.lock().unwrap();
        p.chunks_completed += 1;

        if chunk_downloaded != chunk_size {
            println!("Warning: Chunk {} downloaded {} bytes, expected {}",
                chunk_idx, chunk_downloaded, chunk_size);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_str() {
        assert_eq!(CustomArch::Aarch64.as_str(), "aarch64");
        assert_eq!(CustomArch::X86_64.as_str(), "x86_64");
    }

    #[test]
    fn test_download_url() {
        let url = CustomSdkDownloader::get_download_url("36.0.2", CustomArch::Aarch64);
        assert_eq!(url, "https://github.com/HomuHomu833/android-sdk-custom/releases/download/36.0.2/android-sdk-aarch64-linux-musl.tar.xz");
    }

    #[test]
    fn test_parse_asset_arch() {
        let arch = Asset::parse_arch("android-sdk-aarch64-linux-musl.tar.xz");
        assert_eq!(arch, Some(CustomArch::Aarch64));

        let arch = Asset::parse_arch("android-sdk-x86_64-linux-musl.tar.xz");
        assert_eq!(arch, Some(CustomArch::X86_64));
    }
}