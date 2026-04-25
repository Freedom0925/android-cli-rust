use std::path::{Path, PathBuf};
use anyhow::{Result, Context, anyhow};
use crate::http::Downloader;
use crate::sdk::{Sdk, SdkEntry, Revision, Storage, Repository, Channel};
use crate::sdk::diff::SdkDiff;
use crate::sdk::arm_sdk::{CustomSdkDownloader, CustomArch};

/// SDK Manager - orchestrates SDK package management
pub struct SdkManager {
    storage: Storage,
    repository: Repository,
    downloader: Downloader,
    sdk_path: PathBuf,
    base_url: String,
}

/// Check if current platform needs GitHub releases fallback
fn needs_arm_fallback() -> bool {
    let platform = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // Linux ARM (aarch64) needs fallback
    platform == "linux" && arch == "aarch64"
}

impl SdkManager {
    /// Create a new SdkManager
    pub fn new(
        storage_path: PathBuf,
        sdk_path: PathBuf,
        index_url: &str,
        base_url: &str,
    ) -> Result<Self> {
        let storage = Storage::new(storage_path)
            .context("Failed to initialize storage")?;

        let downloader = Downloader::new()
            .context("Failed to create downloader")?;

        // Check if we need to use ARM fallback
        if needs_arm_fallback() {
            println!("Detected Linux ARM platform");
            println!("Official Google SDK doesn't support this architecture");
            println!("Will use GitHub releases as fallback:");
            println!("  https://github.com/HomuHomu833/android-sdk-custom");
            println!();
        }

        // Fetch repository index (may be empty/unsupported for ARM)
        let repository = Repository::fetch(index_url, &downloader)
            .context("Failed to fetch repository index")?;

        Ok(Self {
            storage,
            repository,
            downloader,
            sdk_path,
            base_url: base_url.to_string(),
        })
    }

    /// Create SdkManager with existing repository (for testing)
    pub fn with_repository(
        storage: Storage,
        repository: Repository,
        downloader: Downloader,
        sdk_path: PathBuf,
        base_url: String,
    ) -> Self {
        Self {
            storage,
            repository,
            downloader,
            sdk_path,
            base_url,
        }
    }

    /// Fetch latest repository index
    pub fn refresh(&mut self, index_url: &str) -> Result<()> {
        self.repository = Repository::fetch(index_url, &self.downloader)
            .context("Failed to refresh repository")?;
        Ok(())
    }

    /// Commit current SDK state to storage
    pub fn commit(&self) -> Result<String> {
        // Scan local SDK and create Sdk index
        let local_sdk = self.scan_local_sdk()?;

        // Save to storage and create head ref
        let sha = self.storage.save_sdk(&local_sdk)?;
        self.storage.add_ref("head", &sha)?;

        Ok(sha)
    }

    /// Scan local SDK directory for installed packages
    fn scan_local_sdk(&self) -> Result<Sdk> {
        let mut entries = Vec::new();

        // Scan common SDK directories
        let dirs_to_scan = [
            "build-tools",
            "platforms",
            "platform-tools",
            "tools",
            "cmdline-tools",
            "emulator",
            "ndk",
            "cmake",
            "sources",
        ];

        for dir in dirs_to_scan {
            let full_path = self.sdk_path.join(dir);
            if full_path.exists() {
                self.scan_package_dir(&full_path, dir, &mut entries)?;
            }
        }

        Ok(Sdk::with_entries(entries))
    }

    /// Scan a package directory for installed versions
    fn scan_package_dir(&self, dir: &Path, package_type: &str, entries: &mut Vec<SdkEntry>) -> Result<()> {
        use std::fs;

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            // Skip non-directories
            if !path.is_dir() {
                continue;
            }

            // Get version from directory name
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Parse version
            let mut revision = Revision::parse(name)
                .unwrap_or_else(|| Revision::new(0));

            // Check for package.xml or source.properties
            let source_props = path.join("source.properties");

            if source_props.exists() {
                // Read version from source.properties
                if let Ok(props) = std::fs::read_to_string(&source_props) {
                    for line in props.lines() {
                        if line.starts_with("Pkg.Revision=") {
                            let version = line.split('=').nth(1).unwrap_or(name);
                            revision = Revision::parse(version.trim())
                                .unwrap_or(revision);
                            break;
                        }
                    }
                }
            }

            // Build full package path
            let full_path = if package_type == "cmdline-tools" {
                // cmdline-tools has nested structure: cmdline-tools/{version}/bin/
                format!("{};{}", package_type, name)
            } else if package_type == "build-tools" || package_type == "platforms" {
                format!("{};{}", package_type, name)
            } else {
                package_type.to_string()
            };

            entries.push(SdkEntry::new(full_path, revision));
        }

        Ok(())
    }

    /// Install packages
    pub fn install(&self, packages: &[String], channel: Channel, force: bool) -> Result<()> {
        // Check if we should use ARM fallback
        if needs_arm_fallback() {
            return self.install_from_github_releases(packages, force);
        }

        // Normal install flow
        // Parse package specifications
        let request = self.parse_request(packages)?;

        // Resolve dependencies
        let resolved = self.repository.resolve(&request, channel, &self.base_url);

        // Check if any packages couldn't be resolved (architecture not supported)
        if resolved.entries.is_empty() {
            println!("No packages found for current architecture in official repository");
            println!("Attempting to download from GitHub releases...");

            return self.install_from_github_releases(packages, force);
        }

        // Download and install each package
        for entry in &resolved.entries {
            if !force && self.is_installed(&entry.path)? {
                println!("Package {} is already installed", entry.path);
                continue;
            }

            self.install_package(entry)?;
        }

        // Update head ref
        self.commit()?;

        // Garbage collect
        self.storage.gc()?;

        Ok(())
    }

    /// Install from GitHub releases (fallback for unsupported architectures)
    fn install_from_github_releases(&self, packages: &[String], force: bool) -> Result<()> {
        use crate::sdk::arm_sdk::CustomSdkDownloader;

        // GitHub releases provides entire SDK, not individual packages
        // So we download the latest version

        println!("Downloading Android SDK from GitHub releases...");
        println!("Source: https://github.com/HomuHomu833/android-sdk-custom");
        println!();

        let downloader = CustomSdkDownloader::new()?;

        // Parse requested packages to infer version (if specified)
        // Take the first package's version, warn if multiple different versions
        let versions: Vec<String> = packages.iter()
            .filter_map(|p| {
                let parts: Vec<&str> = p.split(';').collect();
                if parts.len() > 1 {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
            .collect();

        let version = versions.first().cloned();

        // Warn if multiple different versions specified
        if versions.len() > 1 {
            let unique_versions: Vec<&String> = versions.iter().collect::<std::collections::HashSet<_>>()
                .into_iter().collect();
            if unique_versions.len() > 1 {
                println!("Warning: Multiple versions specified in packages: {}", versions.join(", "));
                println!("Using first version: {}", version.as_ref().unwrap());
            }
        }

        // Get current architecture
        let arch = CustomArch::current()
            .ok_or_else(|| anyhow!("Could not detect architecture for GitHub download"))?;

        println!("Target architecture: {}", arch);
        println!("SDK path: {}", self.sdk_path.display());
        println!();

        // Check if SDK already exists
        if !force && self.sdk_path.exists() {
            // Check for some basic SDK directories
            if self.sdk_path.join("platform-tools").exists() {
                println!("SDK already installed at {}", self.sdk_path.display());
                println!("Use --force to reinstall");
                return Ok(());
            }
        }

        // Download and extract
        downloader.install(version.as_deref(), Some(arch), &self.sdk_path)?;

        println!();
        println!("SDK installed successfully!");
        println!("Note: This is a complete SDK bundle from GitHub releases");
        println!("      It includes platform-tools, build-tools, and other components");

        Ok(())
    }

    /// Parse package request specifications
    fn parse_request(&self, packages: &[String]) -> Result<Sdk> {
        let mut entries = Vec::new();

        for spec in packages {
            // Parse "path;version" or just "path"
            let parts: Vec<&str> = spec.split(';').collect();
            let path = parts[0].to_string();
            let revision = if parts.len() > 1 {
                Revision::parse(parts[1])
                    .with_context(|| format!("Invalid version: {}", parts[1]))?
            } else {
                Revision::new(0) // Will resolve to latest
            };

            entries.push(SdkEntry::new(path, revision));
        }

        Ok(Sdk::with_entries(entries))
    }

    /// Check if a package is already installed
    fn is_installed(&self, path: &str) -> Result<bool> {
        // Check if directory exists in SDK path
        let parts: Vec<&str> = path.split(';').collect();
        let base_path = self.sdk_path.join(parts[0]);

        if parts.len() > 1 {
            // Specific version requested
            let version_dir = base_path.join(parts[1]);
            Ok(version_dir.exists())
        } else {
            // Any version
            Ok(base_path.exists() && base_path.read_dir()?.count() > 0)
        }
    }

    /// Download and install a single package
    fn install_package(&self, entry: &SdkEntry) -> Result<()> {
        println!("Installing {} {}...", entry.path, entry.revision.to_string());

        // Get URL from entry
        let url = entry.url.as_ref()
            .context("Package has no download URL")?;

        // Download archive
        let archive_path = self.download_archive(url, &entry.sha1)?;

        // Extract and install
        self.storage.install_to_sdk(&entry.sha1, &self.sdk_path, &entry.path)?;

        // Write legacy package.xml
        self.write_package_xml(&entry)?;

        println!("Installed {} {}", entry.path, entry.revision.to_string());

        Ok(())
    }

    /// Download archive with SHA verification
    fn download_archive(&self, url: &str, expected_sha: &str) -> Result<PathBuf> {
        // Check if already downloaded
        if self.storage.has_archive(expected_sha) {
            println!("Archive {} already downloaded", expected_sha);
            return Ok(self.storage.archives_dir.join(format!("{}.zip", expected_sha)));
        }

        // Download to temp file
        let temp_dir = tempfile::tempdir()?;
        let temp_file = temp_dir.path().join("archive.zip");

        self.downloader.download(url, &temp_file)?;

        // Verify SHA
        let data = std::fs::read(&temp_file)?;
        let actual_sha = Storage::hash(&data);

        if actual_sha != expected_sha {
            return Err(anyhow::anyhow!(
                "SHA mismatch: expected {}, got {}",
                expected_sha,
                actual_sha
            ));
        }

        // Save to storage
        let archive_path = self.storage.save_archive(expected_sha, &data)?;

        Ok(archive_path)
    }

    /// Write legacy package.xml for Android Studio compatibility
    fn write_package_xml(&self, entry: &SdkEntry) -> Result<()> {
        use std::fs;

        // Determine install location
        let parts: Vec<&str> = entry.path.split(';').collect();
        let install_dir = if parts.len() > 1 {
            self.sdk_path.join(parts[0]).join(parts[1])
        } else {
            self.sdk_path.join(parts[0])
        };

        let xml_path = install_dir.join("package.xml");

        // Ensure directory exists
        fs::create_dir_all(&install_dir)?;

        // Write XML
        let xml_content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <sdk:package xmlns:sdk=\"http://schemas.android.com/sdk/android/repository/\">\n\
             <sdk:revision>\n\
               <sdk:major>{}</sdk:major>\n\
               {}{}\
             </sdk:revision>\n\
             <sdk:display-name>{}</sdk:display-name>\n\
             <sdk:archives>\n\
               <sdk:archive>\n\
                 <sdk:size>{}</sdk:size>\n\
                 <sdk:checksum type=\"sha1\">{}</sdk:checksum>\n\
               </sdk:archive>\n\
             </sdk:archives>\n\
             </sdk:package>",
            entry.revision.major,
            entry.revision.minor.map(|m| format!("<sdk:minor>{}</sdk:minor>\n", m)).unwrap_or_default(),
            entry.revision.micro.map(|m| format!("<sdk:micro>{}</sdk:micro>\n", m)).unwrap_or_default(),
            entry.path,
            entry.size,
            entry.sha1
        );

        fs::write(&xml_path, xml_content)?;

        Ok(())
    }

    /// List packages (available or installed)
    pub fn list(&self, all: bool, all_versions: bool, pattern: Option<&str>, channel: Channel) -> Result<()> {
        let installed = self.scan_local_sdk()?;

        self.repository.list(Some(&installed), all, all_versions, pattern, channel);

        Ok(())
    }

    /// Update installed packages
    pub fn update(&self, packages: Option<&[String]>, channel: Channel, force: bool) -> Result<()> {
        let installed = self.scan_local_sdk()?;

        // Determine which packages to update
        let to_update: Vec<String> = if let Some(pkgs) = packages {
            pkgs.to_vec()
        } else {
            // Update all installed packages
            installed.entries.iter().map(|e| e.path.clone()).collect()
        };

        // Find updates
        let mut updates = Vec::new();
        for path in to_update {
            let latest = self.repository.find_latest(&path, channel);
            if let Some(pkg) = latest {
                let current = installed.find(&path);
                if let Some(current) = current {
                    if pkg.revision.cmp(&current.revision) == std::cmp::Ordering::Greater {
                        println!("Update available: {} {} -> {}", path, current.revision.to_string(), pkg.revision.to_string());
                        updates.push(path);
                    } else {
                        println!("{} {} is up to date", path, current.revision.to_string());
                    }
                }
            }
        }

        if updates.is_empty() {
            println!("No updates available");
            return Ok(());
        }

        // Install updates
        self.install(&updates, channel, force)?;

        Ok(())
    }

    /// Remove packages
    pub fn remove(&self, packages: &[String]) -> Result<()> {
        let installed = self.scan_local_sdk()?;

        for spec in packages {
            let parts: Vec<&str> = spec.split(';').collect();

            // Find and remove
            let install_dir = if parts.len() > 1 {
                self.sdk_path.join(parts[0]).join(parts[1])
            } else {
                // Remove all versions
                self.sdk_path.join(parts[0])
            };

            if install_dir.exists() {
                println!("Removing {}...", spec);
                std::fs::remove_dir_all(&install_dir)
                    .with_context(|| format!("Failed to remove {}", install_dir.display()))?;
                println!("Removed {}", spec);
            } else {
                println!("Package {} is not installed", spec);
            }
        }

        // Update head ref
        self.commit()?;

        Ok(())
    }

    /// Check for updates
    pub fn status(&self, channel: Channel) -> Result<()> {
        let installed = self.scan_local_sdk()?;

        println!("Installed packages:");
        for entry in &installed.entries {
            println!("  {} {}", entry.path, entry.revision.to_string());

            // Check for updates
            let latest = self.repository.find_latest(&entry.path, channel);
            if let Some(pkg) = latest {
                if pkg.revision.cmp(&entry.revision) == std::cmp::Ordering::Greater {
                    println!("    -> Update available: {}", pkg.revision.to_string());
                }
            }
        }

        // Check remote reference
        let remote_sha = self.storage.read_ref("remote").ok();
        let head_sha = self.storage.read_ref("head").ok();

        if let (Some(remote), Some(head)) = (remote_sha, head_sha) {
            if remote != head {
                println!("\nSDK index updated since last refresh. Run 'android sdk list --all' to see changes.");
            }
        }

        Ok(())
    }

    // Hidden/internal commands implementation

    /// Fetch SDK index from repository
    pub fn fetch(&self, check: bool) -> Result<()> {
        // Save repository as SDK index
        let sdk = self.repository_to_sdk(Channel::Stable)?;
        let sha = self.storage.save_sdk(&sdk)?;

        // Store as remote reference
        self.storage.add_ref("remote", &sha)?;

        println!("Fetched SDK index: {}", sha);

        if check {
            // Check for duplicates
            let mut paths: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut duplicates = Vec::new();

            for entry in &sdk.entries {
                if paths.contains(&entry.path) {
                    duplicates.push(entry.path.clone());
                } else {
                    paths.insert(entry.path.clone());
                }
            }

            if duplicates.is_empty() {
                println!("No duplicate packages found");
            } else {
                println!("Duplicate packages found:");
                for dup in duplicates {
                    println!("  {}", dup);
                }
            }
        }

        Ok(())
    }

    /// Resolve package id and version to SHA
    pub fn resolve(&self, package: &str, channel: Channel) -> Result<()> {
        // Parse package specification
        let parts: Vec<&str> = package.split(';').collect();
        let path = parts[0].to_string();
        let revision = if parts.len() > 1 {
            Revision::parse(parts[1])
                .with_context(|| format!("Invalid version: {}", parts[1]))?
        } else {
            Revision::new(0)
        };

        // Find matching package
        let pkg = if revision.major > 0 {
            self.repository.find_exact(&path, &revision)
        } else {
            self.repository.find_latest(&path, channel)
        };

        match pkg {
            Some(p) => {
                // Find archive for current platform
                let archive = p.find_archive(
                    crate::sdk::repository::Platform::current(),
                    crate::sdk::repository::Architecture::current()
                );

                if let Some(archive) = archive {
                    println!("Package: {} {}", p.path, p.revision.to_string());
                    println!("SHA: {}", archive.artifact.checksum);
                    println!("Size: {} bytes", archive.artifact.size);
                    println!("URL: {}", archive.artifact.url);
                } else {
                    println!("No archive available for current platform");
                }
            }
            None => {
                println!("Package not found: {}", package);
            }
        }

        Ok(())
    }

    /// Materialize SDK units from SHA
    pub fn materialize(&self, sha: &str) -> Result<()> {
        // Read SDK index
        let sdk = self.storage.read_sdk(sha)?;

        println!("Materializing SDK index: {}", sha);
        println!("Packages: {}", sdk.entries.len());

        for entry in &sdk.entries {
            println!("  {} {} - SHA: {}", entry.path, entry.revision.to_string(), entry.sha1);
        }

        // Save materialized index
        let materialized_sha = self.storage.save_sdk(&sdk)?;
        println!("Materialized SHA: {}", materialized_sha);

        Ok(())
    }

    /// Download packages by SHA
    pub fn download(&self, sha: &str, url: Option<&str>) -> Result<()> {
        // Check if already downloaded
        if self.storage.has_archive(sha) {
            println!("Archive {} already downloaded", sha);
            return Ok(());
        }

        // If URL provided, download directly
        if let Some(url) = url {
            println!("Downloading from {}...", url);
            let temp_dir = tempfile::tempdir()?;
            let temp_file = temp_dir.path().join("archive.zip");

            self.downloader.download(url, &temp_file)?;

            let data = std::fs::read(&temp_file)?;
            let actual_sha = Storage::hash(&data);

            if actual_sha != sha {
                return Err(anyhow!("SHA mismatch: expected {}, got {}", sha, actual_sha));
            }

            self.storage.save_archive(sha, &data)?;
            println!("Downloaded archive: {}", sha);
        } else {
            // Try to find URL from repository
            let pkg = self.find_package_by_sha(sha);
            if let Some(pkg) = pkg {
                let archive = pkg.find_archive(
                    crate::sdk::repository::Platform::current(),
                    crate::sdk::repository::Architecture::current()
                );

                if let Some(archive) = archive {
                    let full_url = if archive.artifact.url.starts_with("http") {
                        archive.artifact.url.clone()
                    } else {
                        format!("{}/{}", self.base_url, archive.artifact.url)
                    };

                    println!("Downloading from {}...", full_url);
                    let temp_dir = tempfile::tempdir()?;
                    let temp_file = temp_dir.path().join("archive.zip");

                    self.downloader.download(&full_url, &temp_file)?;

                    let data = std::fs::read(&temp_file)?;
                    self.storage.save_archive(sha, &data)?;
                    println!("Downloaded archive: {}", sha);
                } else {
                    return Err(anyhow!("No archive available for current platform"));
                }
            } else {
                return Err(anyhow!("Cannot find package with SHA {} in repository", sha));
            }
        }

        Ok(())
    }

    /// Find package by archive SHA
    fn find_package_by_sha(&self, sha: &str) -> Option<&crate::sdk::repository::Package> {
        self.repository.packages.iter()
            .find(|p| {
                p.archives.iter().any(|a| a.artifact.checksum == sha)
            })
    }

    /// Print storage object by SHA
    pub fn show(&self, sha: &str, json: bool) -> Result<()> {
        // Try to read as SDK index
        let sdk = self.storage.read_sdk(sha)?;

        if json {
            let json_str = serde_json::to_string_pretty(&sdk)
                .context("Failed to serialize SDK to JSON")?;
            println!("{}", json_str);
        } else {
            println!("SDK Index: {}", sha);
            println!("Packages: {}", sdk.entries.len());
            for entry in &sdk.entries {
                println!("  {} {} - {} bytes", entry.path, entry.revision.to_string(), entry.size);
                if !entry.sha1.is_empty() {
                    println!("    SHA: {}", entry.sha1);
                }
                if let Some(url) = &entry.url {
                    println!("    URL: {}", url);
                }
            }
        }

        Ok(())
    }

    /// Print SHA of a reference
    pub fn show_ref(&self, ref_name: &str) -> Result<()> {
        let sha = self.storage.read_ref(ref_name)?;
        println!("{}: {}", ref_name, sha);
        Ok(())
    }

    /// Commit current SDK to storage (returns SHA)
    pub fn commit_cmd(&self) -> Result<()> {
        let sha = self.commit()?;
        println!("Committed SDK: {}", sha);
        Ok(())
    }

    /// Checkout package index and update SDK
    pub fn checkout(&self, sha: &str, force: bool) -> Result<()> {
        // Read target SDK index
        let target_sdk = self.storage.read_sdk(sha)?;

        // Get current SDK state
        let current_sdk = self.scan_local_sdk()?;

        // Calculate diff
        let diff = SdkDiff::calculate(&current_sdk, &target_sdk);

        if !force && diff.has_changes() {
            println!("Local changes detected. Use --force to checkout.");
            diff.print_summary();
            return Ok(());
        }

        println!("Checking out SDK index: {}", sha);

        // Remove packages that need to be removed
        for entry in &diff.removed {
            println!("Removing {}...", entry.path);
            self.remove_package_dir(&entry.path)?;
        }

        // Install/update packages
        for entry in &diff.added {
            self.install_from_entry(entry)?;
        }

        for (_, new_entry) in &diff.updated {
            self.install_from_entry(new_entry)?;
        }

        // Update head reference
        self.storage.add_ref("head", sha)?;

        println!("Checkout complete");
        Ok(())
    }

    /// Remove package directory
    fn remove_package_dir(&self, path: &str) -> Result<()> {
        let parts: Vec<&str> = path.split(';').collect();
        let install_dir = if parts.len() > 1 {
            self.sdk_path.join(parts[0]).join(parts[1])
        } else {
            self.sdk_path.join(parts[0])
        };

        if install_dir.exists() {
            std::fs::remove_dir_all(&install_dir)
                .with_context(|| format!("Failed to remove {}", install_dir.display()))?;
        }

        Ok(())
    }

    /// Install package from SdkEntry
    fn install_from_entry(&self, entry: &SdkEntry) -> Result<()> {
        if entry.url.is_none() || entry.sha1.is_empty() {
            println!("Skipping {} - no download info", entry.path);
            return Ok(());
        }

        println!("Installing {} {}...", entry.path, entry.revision.to_string());

        // Download if needed
        if !self.storage.has_archive(&entry.sha1) {
            self.download(&entry.sha1, entry.url.as_deref())?;
        }

        // Unzip
        self.storage.unzip(&entry.sha1)?;

        // Install to SDK
        self.storage.install_to_sdk(&entry.sha1, &self.sdk_path, &entry.path)?;

        // Write package.xml
        self.write_package_xml(entry)?;

        println!("Installed {} {}", entry.path, entry.revision.to_string());
        Ok(())
    }

    /// Install packages from index into another
    pub fn update_index(&self, source_sha: &str, target_sha: &str) -> Result<()> {
        // Read both SDKs
        let source_sdk = self.storage.read_sdk(source_sha)?;
        let target_sdk = self.storage.read_sdk(target_sha)?;

        // Update target with source
        let updated_sdk = target_sdk.update(&source_sdk);

        // Save updated index
        let new_sha = self.storage.save_sdk(&updated_sdk)?;

        println!("Updated index: {} -> {}", target_sha, new_sha);
        println!("Packages: {} -> {}", target_sdk.entries.len(), updated_sdk.entries.len());

        Ok(())
    }

    /// Diff two SDK indexes (returns three SHAs)
    pub fn diff_cmd(&self, sha1: &str, sha2: &str, verbose: bool) -> Result<()> {
        let sdk1 = self.storage.read_sdk(sha1)?;
        let sdk2 = self.storage.read_sdk(sha2)?;

        let (common, changed, removed) = sdk1.diff(&sdk2);

        // Save each part
        let common_sha = self.storage.save_sdk(&common)?;
        let changed_sha = self.storage.save_sdk(&changed)?;
        let removed_sha = self.storage.save_sdk(&removed)?;

        println!("Diff: {} vs {}", sha1, sha2);
        println!("Common: {} ({} packages)", common_sha, common.entries.len());
        println!("Changed: {} ({} packages)", changed_sha, changed.entries.len());
        println!("Removed: {} ({} packages)", removed_sha, removed.entries.len());

        if verbose {
            if !common.entries.is_empty() {
                println!("\nCommon packages:");
                for entry in &common.entries {
                    println!("  {} {}", entry.path, entry.revision.to_string());
                }
            }

            if !changed.entries.is_empty() {
                println!("\nChanged/new packages:");
                for entry in &changed.entries {
                    println!("  {} {}", entry.path, entry.revision.to_string());
                }
            }

            if !removed.entries.is_empty() {
                println!("\nRemoved packages:");
                for entry in &removed.entries {
                    println!("  {} {}", entry.path, entry.revision.to_string());
                }
            }
        }

        Ok(())
    }

    /// Remove from disk by SHA
    pub fn rm(&self, sha: &str, archive: bool, unzipped: bool) -> Result<()> {
        // Remove object
        let object_path = self.storage.objects_dir.join(sha);
        if object_path.exists() {
            std::fs::remove_file(&object_path)?;
            println!("Removed object: {}", sha);
        }

        // Remove archive if requested
        if archive {
            let archive_path = self.storage.archives_dir.join(format!("{}.zip", sha));
            if archive_path.exists() {
                std::fs::remove_file(&archive_path)?;
                println!("Removed archive: {}", sha);
            }
        }

        // Remove unzipped if requested
        if unzipped {
            let unzipped_path = self.storage.unzipped_dir.join(sha);
            if unzipped_path.exists() {
                std::fs::remove_dir_all(&unzipped_path)?;
                println!("Removed unzipped: {}", sha);
            }
        }

        Ok(())
    }

    /// Unzip downloaded packages in place
    pub fn unzip_cmd(&self, sha: &str) -> Result<()> {
        if !self.storage.has_archive(sha) {
            return Err(anyhow!("Archive not found: {}", sha));
        }

        let unzipped_path = self.storage.unzip(sha)?;
        println!("Unzipped to: {}", unzipped_path.display());

        Ok(())
    }

    /// Generate legacy package.xml files
    pub fn write_xml_cmd(&self, package: &str, sha: Option<&str>) -> Result<()> {
        // Parse package path
        let parts: Vec<&str> = package.split(';').collect();
        let path = parts[0].to_string();
        let revision = if parts.len() > 1 {
            Revision::parse(parts[1])
                .with_context(|| format!("Invalid version: {}", parts[1]))?
        } else {
            // Try to find installed version
            let installed = self.scan_local_sdk()?;
            let entry = installed.find(&path);
            if let Some(e) = entry {
                e.revision.clone()
            } else {
                Revision::new(0)
            }
        };

        // Create entry for XML generation
        let entry = SdkEntry {
            path: path.clone(),
            revision,
            url: None,
            size: 0,
            sha1: sha.map(|s| s.to_string()).unwrap_or_default(),
        };

        self.write_package_xml(&entry)?;
        println!("Generated package.xml for: {}", package);

        Ok(())
    }

    /// Delete package from index
    pub fn delete_index(&self, sha: &str, package: &str) -> Result<()> {
        // Read SDK
        let sdk = self.storage.read_sdk(sha)?;

        // Delete package
        let updated_sdk = sdk.delete(package);

        // Save updated index
        let new_sha = self.storage.save_sdk(&updated_sdk)?;

        println!("Deleted {} from index", package);
        println!("New index SHA: {}", new_sha);

        Ok(())
    }

    /// Garbage collect storage
    pub fn gc_cmd(&self, dry_run: bool, aggressive: bool) -> Result<()> {
        println!("Garbage collecting storage...");

        // Find all referenced SHAs
        let referenced: Vec<String> = self.storage.get_all_referenced()?;

        let mut objects_removed = 0;
        let mut archives_removed = 0;
        let mut unzipped_removed = 0;

        // Check objects
        for entry in std::fs::read_dir(&self.storage.objects_dir)? {
            let entry = entry?;
            let sha = entry.file_name().to_string_lossy().to_string();

            if aggressive || !referenced.contains(&sha) {
                if dry_run {
                    println!("Would remove object: {}", sha);
                } else {
                    std::fs::remove_file(entry.path())?;
                }
                objects_removed += 1;
            }
        }

        // Check archives
        for entry in std::fs::read_dir(&self.storage.archives_dir)? {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().to_string();
            let sha = filename.replace(".zip", "");

            if aggressive || !referenced.contains(&sha) {
                if dry_run {
                    println!("Would remove archive: {}", sha);
                } else {
                    std::fs::remove_file(entry.path())?;
                }
                archives_removed += 1;
            }
        }

        // Check unzipped
        for entry in std::fs::read_dir(&self.storage.unzipped_dir)? {
            let entry = entry?;
            let sha = entry.file_name().to_string_lossy().to_string();

            if aggressive || !referenced.contains(&sha) {
                if dry_run {
                    println!("Would remove unzipped: {}", sha);
                } else {
                    std::fs::remove_dir_all(entry.path())?;
                }
                unzipped_removed += 1;
            }
        }

        if dry_run {
            println!("Dry run - would remove:");
            println!("  Objects: {}", objects_removed);
            println!("  Archives: {}", archives_removed);
            println!("  Unzipped: {}", unzipped_removed);
        } else {
            println!("Removed:");
            println!("  Objects: {}", objects_removed);
            println!("  Archives: {}", archives_removed);
            println!("  Unzipped: {}", unzipped_removed);
        }

        Ok(())
    }

    /// Convert repository to SDK index
    fn repository_to_sdk(&self, channel: Channel) -> Result<Sdk> {
        let mut entries = Vec::new();

        for pkg in &self.repository.packages {
            if pkg.channel > channel && pkg.channel != Channel::Stable {
                continue;
            }

            let archive = pkg.find_archive(
                crate::sdk::repository::Platform::current(),
                crate::sdk::repository::Architecture::current()
            );

            if let Some(archive) = archive {
                let full_url = if archive.artifact.url.starts_with("http") {
                    archive.artifact.url.clone()
                } else {
                    format!("{}/{}", self.base_url, archive.artifact.url)
                };

                entries.push(SdkEntry::with_archive(
                    pkg.path.clone(),
                    pkg.revision.clone(),
                    full_url,
                    archive.artifact.size,
                    archive.artifact.checksum.clone(),
                ));
            }
        }

        Ok(Sdk::with_entries(entries))
    }
}