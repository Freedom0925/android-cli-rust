use anyhow::{Result, Context};
use crate::sdk::model::{Sdk, SdkEntry, Revision};
use crate::http::Downloader;

/// Release channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Channel {
    Stable = 0,
    Beta = 1,
    Canary = 2,
}

impl Channel {
    pub fn from_int(i: i32) -> Self {
        match i {
            0 => Channel::Stable,
            1 => Channel::Beta,
            2 => Channel::Canary,
            _ => Channel::Stable,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "stable" => Some(Channel::Stable),
            "beta" => Some(Channel::Beta),
            "canary" => Some(Channel::Canary),
            _ => None,
        }
    }
}

/// Platform (OS)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux = 1,
    Mac = 2,
    Windows = 3,
}

impl Platform {
    pub fn from_int(i: i32) -> Self {
        match i {
            1 => Platform::Linux,
            2 => Platform::Mac,
            3 => Platform::Windows,
            _ => Platform::Linux,
        }
    }

    pub fn current() -> Self {
        match std::env::consts::OS {
            "linux" => Platform::Linux,
            "macos" => Platform::Mac,
            "windows" => Platform::Windows,
            _ => Platform::Linux,
        }
    }
}

/// Architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    X86 = 1,
    X64 = 2,
    Aarch64 = 3,
}

impl Architecture {
    pub fn from_int(i: i32) -> Self {
        match i {
            1 => Architecture::X86,
            2 => Architecture::X64,
            3 => Architecture::Aarch64,
            _ => Architecture::X64,
        }
    }

    pub fn current() -> Self {
        match std::env::consts::ARCH {
            "x86" | "i686" => Architecture::X86,
            "x86_64" | "amd64" => Architecture::X64,
            "aarch64" | "arm64" => Architecture::Aarch64,
            _ => Architecture::X64,
        }
    }
}

/// Download artifact
#[derive(Debug, Clone)]
pub struct Artifact {
    pub size: u64,
    pub checksum: String,
    pub url: String,
}

/// Archive with platform/arch info
#[derive(Debug, Clone)]
pub struct Archive {
    pub artifact: Artifact,
    pub host_os: Platform,
    pub host_arch: Architecture,
}

/// Dependency
#[derive(Debug, Clone)]
pub struct Dependency {
    pub path: String,
    pub min_revision: Option<Revision>,
}

/// Package definition from repository
#[derive(Debug, Clone)]
pub struct Package {
    pub path: String,
    pub revision: Revision,
    pub display_name: String,
    pub license_name: Option<String>,
    pub dependencies: Vec<Dependency>,
    pub channel: Channel,
    pub archives: Vec<Archive>,
    pub obsolete: bool,
}

impl Package {
    /// Find archive matching current platform
    /// On macOS aarch64, if no native arm64 archive exists, fallback to x64 (Rosetta)
    pub fn find_archive(&self, platform: Platform, arch: Architecture) -> Option<&Archive> {
        // First try exact match
        let exact = self.archives.iter().find(|a| {
            a.host_os == platform && a.host_arch == arch
        });

        if exact.is_some() {
            return exact;
        }

        // On macOS aarch64, fallback to x64 (can run via Rosetta)
        if platform == Platform::Mac && arch == Architecture::Aarch64 {
            return self.archives.iter().find(|a| {
                a.host_os == Platform::Mac && a.host_arch == Architecture::X64
            });
        }

        None
    }

    /// Check if this package matches a request
    pub fn matches(&self, path: &str, version: Option<&Revision>, channel: Channel) -> bool {
        if self.path != path {
            return false;
        }

        // Check version if specified
        if let Some(v) = version {
            if self.revision.cmp(v) != std::cmp::Ordering::Equal {
                return false;
            }
        }

        // Check channel (stable packages are always available)
        if self.channel != Channel::Stable && channel < self.channel {
            return false;
        }

        true
    }
}

/// Repository containing available packages
#[derive(Debug, Clone, Default)]
pub struct Repository {
    pub packages: Vec<Package>,
}

impl Repository {
    pub fn new() -> Self {
        Self { packages: Vec::new() }
    }

    /// Fetch repository from URL (package_list.binpb is protobuf format)
    pub fn fetch(url: &str, downloader: &Downloader) -> Result<Self> {
        let bytes = downloader.fetch_bytes(url)
            .with_context(|| format!("Failed to fetch repository from: {}", url))?;

        // Parse protobuf PackageList
        Self::parse_protobuf(&bytes)
    }

    /// Parse protobuf PackageList
    fn parse_protobuf(bytes: &[u8]) -> Result<Self> {
        // The package_list.binpb is a protobuf message
        Self::parse_package_list(bytes)
    }

    /// Parse PackageList protobuf message
    fn parse_package_list(bytes: &[u8]) -> Result<Self> {
        use std::io::Cursor;

        let mut packages = Vec::new();
        let mut cursor = Cursor::new(bytes);

        // Simple protobuf parsing - field 1 is repeated Package
        while cursor.position() < bytes.len() as u64 {
            let tag = read_varint(&mut cursor)?;
            let field_num = tag >> 3;
            let wire_type = tag & 0x7;

            if field_num == 1 && wire_type == 2 {
                // Package message
                let len = read_varint(&mut cursor)?;
                let start = cursor.position() as usize;
                let end = start + len as usize;

                if end <= bytes.len() {
                    let pkg_bytes = &bytes[start..end];
                    if let Some(pkg) = Self::parse_package(pkg_bytes) {
                        packages.push(pkg);
                    }
                }

                cursor.set_position(end as u64);
            } else {
                // Skip unknown field
                skip_field(&mut cursor, wire_type)?;
            }
        }

        Ok(Self { packages })
    }

    /// Parse a single Package protobuf message
    fn parse_package(bytes: &[u8]) -> Option<Package> {
        let mut cursor = Cursor::new(bytes);

        let mut path = String::new();
        let mut revision = Revision::new(0);
        let mut display_name = String::new();
        let mut channel = Channel::Stable;
        let mut archives = Vec::new();
        let mut dependencies = Vec::new();
        let mut obsolete = false;

        while cursor.position() < bytes.len() as u64 {
            let tag = read_varint(&mut cursor).ok()?;
            let field_num = tag >> 3;
            let wire_type = tag & 0x7;

            match field_num {
                1 => { // obsolete (bool)
                    obsolete = read_varint(&mut cursor).ok()? != 0;
                }
                2 => { // path (string)
                    path = read_string(&mut cursor).ok()?;
                }
                3 => { // revision (message)
                    let len = read_varint(&mut cursor).ok()?;
                    let start = cursor.position() as usize;
                    if start + len as usize <= bytes.len() {
                        revision = Self::parse_revision(&bytes[start..start + len as usize]);
                    }
                    cursor.set_position((start + len as usize) as u64);
                }
                4 => { // display_name (string)
                    display_name = read_string(&mut cursor).ok()?;
                }
                7 => { // channel (int32)
                    channel = Channel::from_int(read_varint(&mut cursor).ok()? as i32);
                }
                8 => { // archives (repeated message)
                    let len = read_varint(&mut cursor).ok()?;
                    let start = cursor.position() as usize;
                    if start + len as usize <= bytes.len() {
                        if let Some(archive) = Self::parse_archive(&bytes[start..start + len as usize]) {
                            archives.push(archive);
                        }
                    }
                    cursor.set_position((start + len as usize) as u64);
                }
                6 => { // dependencies (repeated message)
                    let len = read_varint(&mut cursor).ok()?;
                    let start = cursor.position() as usize;
                    if start + len as usize <= bytes.len() {
                        if let Some(dep) = Self::parse_dependency(&bytes[start..start + len as usize]) {
                            dependencies.push(dep);
                        }
                    }
                    cursor.set_position((start + len as usize) as u64);
                }
                _ => {
                    skip_field(&mut cursor, wire_type).ok();
                }
            }
        }

        if path.is_empty() {
            return None;
        }

        Some(Package {
            path,
            revision,
            display_name,
            license_name: None,
            dependencies,
            channel,
            archives,
            obsolete,
        })
    }

    /// Parse Revision protobuf message
    fn parse_revision(bytes: &[u8]) -> Revision {
        let mut cursor = Cursor::new(bytes);
        let mut major = 0;
        let mut minor = None;
        let mut micro = None;

        while cursor.position() < bytes.len() as u64 {
            let tag = read_varint(&mut cursor).unwrap_or(0);
            let field_num = tag >> 3;

            match field_num {
                1 => major = read_varint(&mut cursor).unwrap_or(0) as i32,
                2 => minor = Some(read_varint(&mut cursor).unwrap_or(0) as i32),
                3 => micro = Some(read_varint(&mut cursor).unwrap_or(0) as i32),
                _ => { let wt = tag & 0x7; skip_field(&mut cursor, wt).ok(); }
            }
        }

        Revision { major, minor, micro, preview: None }
    }

    /// Parse Archive protobuf message
    fn parse_archive(bytes: &[u8]) -> Option<Archive> {
        let mut cursor = Cursor::new(bytes);
        let mut artifact = None;
        let mut host_os = Platform::current();
        let mut host_arch = Architecture::current();

        while cursor.position() < bytes.len() as u64 {
            let tag = read_varint(&mut cursor).ok()?;
            let field_num = tag >> 3;
            let wire_type = tag & 0x7;

            match field_num {
                1 => { // artifact
                    let len = read_varint(&mut cursor).ok()?;
                    let start = cursor.position() as usize;
                    if start + len as usize <= bytes.len() {
                        artifact = Self::parse_artifact(&bytes[start..start + len as usize]);
                    }
                    cursor.set_position((start + len as usize) as u64);
                }
                2 => {
                    let os_val = read_varint(&mut cursor).ok()? as i32;
                    host_os = Platform::from_int(os_val);
                }
                3 => {
                    let arch_val = read_varint(&mut cursor).ok()? as i32;
                    host_arch = Architecture::from_int(arch_val);
                }
                _ => { skip_field(&mut cursor, wire_type).ok(); }
            }
        }

        artifact.map(|a| Archive { artifact: a, host_os, host_arch })
    }

    /// Parse Artifact protobuf message
    fn parse_artifact(bytes: &[u8]) -> Option<Artifact> {
        let mut cursor = Cursor::new(bytes);
        let mut size = 0;
        let mut checksum = String::new();
        let mut url = String::new();

        while cursor.position() < bytes.len() as u64 {
            let tag = read_varint(&mut cursor).ok()?;
            let field_num = tag >> 3;
            let wire_type = tag & 0x7;

            match field_num {
                1 => size = read_varint(&mut cursor).ok()?,
                2 => checksum = read_string(&mut cursor).ok()?,
                3 => url = read_string(&mut cursor).ok()?,
                _ => { skip_field(&mut cursor, wire_type).ok(); }
            }
        }

        Some(Artifact { size, checksum, url })
    }

    /// Parse Dependency protobuf message
    fn parse_dependency(bytes: &[u8]) -> Option<Dependency> {
        let mut cursor = Cursor::new(bytes);
        let mut path = String::new();
        let mut min_revision = None;

        while cursor.position() < bytes.len() as u64 {
            let tag = read_varint(&mut cursor).ok()?;
            let field_num = tag >> 3;
            let wire_type = tag & 0x7;

            match field_num {
                1 => path = read_string(&mut cursor).ok()?,
                2 => {
                    let len = read_varint(&mut cursor).ok()?;
                    let start = cursor.position() as usize;
                    if start + len as usize <= bytes.len() {
                        min_revision = Some(Self::parse_revision(&bytes[start..start + len as usize]));
                    }
                    cursor.set_position((start + len as usize) as u64);
                }
                _ => { skip_field(&mut cursor, wire_type).ok(); }
            }
        }

        Some(Dependency { path, min_revision })
    }

    /// Find packages matching a path pattern
    pub fn find(&self, path: &str) -> Vec<&Package> {
        self.packages.iter()
            .filter(|p| p.path == path || p.path.starts_with(&format!("{};", path)))
            .collect()
    }

    /// Find latest package for a path
    pub fn find_latest(&self, path: &str, channel: Channel) -> Option<&Package> {
        self.packages.iter()
            .filter(|p| p.path == path || p.path.starts_with(&format!("{};", path)))
            .filter(|p| p.channel <= channel || p.channel == Channel::Stable)
            .filter(|p| !p.obsolete)
            .max_by(|a, b| a.revision.cmp(&b.revision))
    }

    /// Find package by exact path with version
    pub fn find_exact(&self, path: &str, revision: &Revision) -> Option<&Package> {
        self.packages.iter()
            .find(|p| p.path == path && p.revision.cmp(revision) == std::cmp::Ordering::Equal)
    }

    /// Resolve SDK request to full package list with dependencies
    pub fn resolve(&self, request: &Sdk, channel: Channel, base_url: &str) -> Sdk {
        let mut resolved_entries = Vec::new();

        let current_platform = Platform::current();
        let current_arch = Architecture::current();

        for entry in &request.entries {
            // Find matching package
            let pkg = if entry.revision.major > 0 {
                self.find_exact(&entry.path, &entry.revision)
            } else {
                self.find_latest(&entry.path, channel)
            };

            if let Some(pkg) = pkg {
                // Debug: print available archives
                #[cfg(debug_assertions)]
                {
                    eprintln!("Package: {}", pkg.path);
                    for archive in &pkg.archives {
                        eprintln!("  Archive: os={}, arch={}", archive.host_os as i32, archive.host_arch as i32);
                    }
                    eprintln!("  Looking for: os={}, arch={}", current_platform as i32, current_arch as i32);
                }

                // Find archive for current platform
                let archive = pkg.find_archive(current_platform, current_arch);

                if let Some(archive) = archive {
                    let full_url = if archive.artifact.url.starts_with("http") {
                        archive.artifact.url.clone()
                    } else {
                        format!("{}/{}", base_url, archive.artifact.url)
                    };

                    resolved_entries.push(SdkEntry::with_archive(
                        pkg.path.clone(),
                        pkg.revision.clone(),
                        full_url,
                        archive.artifact.size,
                        archive.artifact.checksum.clone(),
                    ));

                    // Add dependencies
                    for dep in &pkg.dependencies {
                        if let Some(dep_pkg) = self.find_latest(&dep.path, channel) {
                            let dep_archive = dep_pkg.find_archive(Platform::current(), Architecture::current());
                            if let Some(dep_archive) = dep_archive {
                                let dep_url = if dep_archive.artifact.url.starts_with("http") {
                                    dep_archive.artifact.url.clone()
                                } else {
                                    format!("{}/{}", base_url, dep_archive.artifact.url)
                                };

                                resolved_entries.push(SdkEntry::with_archive(
                                    dep_pkg.path.clone(),
                                    dep_pkg.revision.clone(),
                                    dep_url,
                                    dep_archive.artifact.size,
                                    dep_archive.artifact.checksum.clone(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        Sdk::with_entries(resolved_entries)
    }

    /// List packages for display
    pub fn list(&self, installed: Option<&Sdk>, all: bool, all_versions: bool, pattern: Option<&str>, channel: Channel) {
        let packages_to_show: Vec<&Package> = if all {
            self.packages.iter()
                .filter(|p| !p.obsolete)
                .filter(|p| p.channel <= channel || p.channel == Channel::Stable)
                .collect()
        } else {
            // Show installed packages
            if let Some(sdk) = installed {
                sdk.entries.iter()
                    .filter_map(|e| self.find_latest(&e.path, channel))
                    .collect()
            } else {
                Vec::new()
            }
        };

        // Filter by pattern
        let packages_to_show: Vec<&Package> = if let Some(p) = pattern {
            packages_to_show.iter()
                .filter(|pkg| pkg.path.contains(p) || pkg.display_name.contains(p))
                .cloned()
                .collect()
        } else {
            packages_to_show
        };

        // Group by path
        let mut groups: std::collections::HashMap<String, Vec<&Package>> = std::collections::HashMap::new();
        for pkg in packages_to_show {
            groups.entry(pkg.path.clone()).or_default().push(pkg);
        }

        // Print results
        for (path, pkgs) in groups.iter() {
            if all_versions {
                println!("{}:", path);
                let mut sorted_pkgs: Vec<&Package> = pkgs.iter().cloned().collect();
                sorted_pkgs.sort_by(|a, b| b.revision.cmp(&a.revision));
                for pkg in sorted_pkgs {
                    let status = if let Some(sdk) = installed {
                        if sdk.find(path).is_some() {
                            "installed"
                        } else {
                            "available"
                        }
                    } else {
                        ""
                    };
                    println!("  {} {} {}", pkg.revision.to_string(), pkg.display_name, status);
                }
            } else {
                let latest = pkgs.iter().max_by(|a, b| a.revision.cmp(&b.revision));
                if let Some(pkg) = latest {
                    let status = if let Some(sdk) = installed {
                        if let Some(inst) = sdk.find(path) {
                            if inst.revision.cmp(&pkg.revision) == std::cmp::Ordering::Equal {
                                "installed"
                            } else if inst.revision.cmp(&pkg.revision) == std::cmp::Ordering::Less {
                                "update available"
                            } else {
                                "installed"
                            }
                        } else {
                            "available"
                        }
                    } else {
                        ""
                    };
                    println!("{}\t{}\t{}\t{}", path, pkg.revision.to_string(), pkg.display_name, status);
                }
            }
        }
    }
}

// Protobuf helper functions

use std::io::Cursor;
use std::io::Read;

fn read_varint(cursor: &mut Cursor<&[u8]>) -> Result<u64> {
    let mut result: u64 = 0;
    let mut shift = 0;

    loop {
        let mut byte = [0u8; 1];
        cursor.read_exact(&mut byte)?;
        let b = byte[0];

        result |= ((b & 0x7F) as u64) << shift;

        if b & 0x80 == 0 {
            break;
        }

        shift += 7;
        if shift >= 64 {
            return Err(anyhow::anyhow!("Varint too long"));
        }
    }

    Ok(result)
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
    let len = read_varint(cursor)?;
    let start = cursor.position() as usize;
    let end = start + len as usize;

    if end > cursor.get_ref().len() {
        return Err(anyhow::anyhow!("String length exceeds buffer"));
    }

    let s = String::from_utf8_lossy(&cursor.get_ref()[start..end]).to_string();
    cursor.set_position(end as u64);

    Ok(s)
}

fn skip_field(cursor: &mut Cursor<&[u8]>, wire_type: u64) -> Result<()> {
    match wire_type {
        0 => { read_varint(cursor)?; } // Varint
        1 => { cursor.set_position(cursor.position() + 8); } // 64-bit
        2 => { // Length-delimited
            let len = read_varint(cursor)?;
            cursor.set_position(cursor.position() + len);
        }
        5 => { cursor.set_position(cursor.position() + 4); } // 32-bit
        _ => return Err(anyhow::anyhow!("Unknown wire type: {}", wire_type)),
    }
    Ok(())
}