use crate::http::Downloader;
use crate::sdk::model::{Revision, Sdk, SdkEntry};
use crate::sdk::protobuf::PackageList;
use anyhow::{Context, Result};
use prost::Message;
use tracing::debug;

/// Release channel (local representation, different from proto enum values)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Channel {
    Stable = 0,
    Beta = 1,
    Canary = 2,
}

impl Channel {
    /// Convert from proto Channel enum value
    /// Proto: Unspecified=0, Canary=1, Beta=2, Stable=3
    /// Local: Stable=0, Beta=1, Canary=2
    pub fn from_proto(proto_channel: crate::sdk::protobuf::Channel) -> Self {
        match proto_channel {
            crate::sdk::protobuf::Channel::Stable => Channel::Stable,
            crate::sdk::protobuf::Channel::Beta => Channel::Beta,
            crate::sdk::protobuf::Channel::Canary => Channel::Canary,
            crate::sdk::protobuf::Channel::Unspecified => Channel::Stable,
        }
    }

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

/// Platform (OS) - local representation matching proto values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux = 1,
    Mac = 2,
    Windows = 3,
}

impl Platform {
    /// Convert from proto Platform enum
    pub fn from_proto(proto_platform: crate::sdk::protobuf::Platform) -> Self {
        match proto_platform {
            crate::sdk::protobuf::Platform::Linux => Platform::Linux,
            crate::sdk::protobuf::Platform::Mac => Platform::Mac,
            crate::sdk::protobuf::Platform::Windows => Platform::Windows,
            crate::sdk::protobuf::Platform::Unspecified => Platform::Linux,
        }
    }

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

/// Architecture - local representation matching proto values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    X86 = 1,
    X64 = 2,
    Aarch64 = 3,
}

impl Architecture {
    /// Convert from proto Architecture enum
    pub fn from_proto(proto_arch: crate::sdk::protobuf::Architecture) -> Self {
        match proto_arch {
            crate::sdk::protobuf::Architecture::X64 => Architecture::X64,
            crate::sdk::protobuf::Architecture::X86 => Architecture::X86,
            crate::sdk::protobuf::Architecture::Aarch64 => Architecture::Aarch64,
            crate::sdk::protobuf::Architecture::Unspecified => Architecture::X64,
        }
    }

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
        let exact = self
            .archives
            .iter()
            .find(|a| a.host_os == platform && a.host_arch == arch);

        if exact.is_some() {
            return exact;
        }

        // On macOS aarch64, fallback to x64 (can run via Rosetta)
        if platform == Platform::Mac && arch == Architecture::Aarch64 {
            return self
                .archives
                .iter()
                .find(|a| a.host_os == Platform::Mac && a.host_arch == Architecture::X64);
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
        Self {
            packages: Vec::new(),
        }
    }

    /// Fetch repository from URL (package_list.binpb is protobuf format)
    pub fn fetch(url: &str, downloader: &Downloader) -> Result<Self> {
        let bytes = downloader
            .fetch_bytes(url)
            .with_context(|| format!("Failed to fetch repository from: {}", url))?;

        // Parse protobuf PackageList using prost
        Self::parse_protobuf(&bytes)
    }

    /// Parse protobuf PackageList using prost
    fn parse_protobuf(bytes: &[u8]) -> Result<Self> {
        let proto_list = PackageList::decode(bytes)
            .context("Failed to decode PackageList protobuf")?;

        let packages = proto_list
            .packages
            .into_iter()
            .filter_map(|proto_pkg| Self::convert_package(proto_pkg))
            .collect();

        Ok(Self { packages })
    }

    /// Convert proto Package to local Package
    fn convert_package(proto_pkg: crate::sdk::protobuf::Package) -> Option<Package> {
        if proto_pkg.path.is_empty() {
            return None;
        }

        let revision = proto_pkg
            .revision
            .map(|r| Revision {
                major: r.major,
                minor: if r.minor != 0 { Some(r.minor) } else { None },
                micro: if r.micro != 0 { Some(r.micro) } else { None },
                preview: if r.preview != 0 { Some(r.preview) } else { None },
            })
            .unwrap_or_else(|| Revision::new(0));

        let channel = Channel::from_proto(
            crate::sdk::protobuf::Channel::try_from(proto_pkg.channel).unwrap_or(
                crate::sdk::protobuf::Channel::Stable,
            ),
        );

        let archives = proto_pkg
            .archives
            .into_iter()
            .filter_map(|a| Self::convert_archive(a))
            .collect();

        let dependencies = proto_pkg
            .dependencies
            .into_iter()
            .filter_map(|d| Self::convert_dependency(d))
            .collect();

        Some(Package {
            path: proto_pkg.path,
            revision,
            display_name: proto_pkg.display_name,
            license_name: if proto_pkg.license_name.is_empty() {
                None
            } else {
                Some(proto_pkg.license_name)
            },
            dependencies,
            channel,
            archives,
            obsolete: proto_pkg.obsolete,
        })
    }

    /// Convert proto Archive to local Archive
    fn convert_archive(proto_archive: crate::sdk::protobuf::Archive) -> Option<Archive> {
        let artifact = proto_archive.artifact.map(|a| Artifact {
            size: a.size as u64,
            checksum: a.checksum,
            url: a.url,
        })?;

        let host_os = Platform::from_proto(
            crate::sdk::protobuf::Platform::try_from(proto_archive.host_os).unwrap_or(
                crate::sdk::protobuf::Platform::Linux,
            ),
        );

        let host_arch = Architecture::from_proto(
            crate::sdk::protobuf::Architecture::try_from(proto_archive.host_arch).unwrap_or(
                crate::sdk::protobuf::Architecture::X64,
            ),
        );

        Some(Archive {
            artifact,
            host_os,
            host_arch,
        })
    }

    /// Convert proto Dependency to local Dependency
    fn convert_dependency(proto_dep: crate::sdk::protobuf::Dependency) -> Option<Dependency> {
        if proto_dep.path.is_empty() {
            return None;
        }

        let min_revision = proto_dep.min_revision.map(|r| Revision {
            major: r.major,
            minor: if r.minor != 0 { Some(r.minor) } else { None },
            micro: if r.micro != 0 { Some(r.micro) } else { None },
            preview: None,
        });

        Some(Dependency {
            path: proto_dep.path,
            min_revision,
        })
    }

    /// Find packages matching a path pattern
    pub fn find(&self, path: &str) -> Vec<&Package> {
        self.packages
            .iter()
            .filter(|p| p.path == path || p.path.starts_with(&format!("{};", path)))
            .collect()
    }

    /// Find latest package for a path
    pub fn find_latest(&self, path: &str, channel: Channel) -> Option<&Package> {
        self.packages
            .iter()
            .filter(|p| p.path == path || p.path.starts_with(&format!("{};", path)))
            .filter(|p| p.channel <= channel || p.channel == Channel::Stable)
            .filter(|p| !p.obsolete)
            .max_by(|a, b| a.revision.cmp(&b.revision))
    }

    /// Find package by exact path with version
    pub fn find_exact(&self, path: &str, revision: &Revision) -> Option<&Package> {
        self.packages
            .iter()
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
                debug!(
                    package = %pkg.path,
                    archives = pkg.archives.len(),
                    looking_for_os = current_platform as i32,
                    looking_for_arch = current_arch as i32,
                    "resolve: package found"
                );

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
                            let dep_archive =
                                dep_pkg.find_archive(Platform::current(), Architecture::current());
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
    pub fn list(
        &self,
        installed: Option<&Sdk>,
        all: bool,
        all_versions: bool,
        pattern: Option<&str>,
        channel: Channel,
    ) {
        let packages_to_show: Vec<&Package> = if all {
            self.packages
                .iter()
                .filter(|p| !p.obsolete)
                .filter(|p| p.channel <= channel || p.channel == Channel::Stable)
                .collect()
        } else {
            // Show installed packages
            if let Some(sdk) = installed {
                sdk.entries
                    .iter()
                    .filter_map(|e| self.find_latest(&e.path, channel))
                    .collect()
            } else {
                Vec::new()
            }
        };

        // Filter by pattern
        let packages_to_show: Vec<&Package> = if let Some(p) = pattern {
            packages_to_show
                .iter()
                .filter(|pkg| pkg.path.contains(p) || pkg.display_name.contains(p))
                .cloned()
                .collect()
        } else {
            packages_to_show
        };

        // Group by path
        let mut groups: std::collections::HashMap<String, Vec<&Package>> =
            std::collections::HashMap::new();
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
                    println!(
                        "  {} {} {}",
                        pkg.revision.to_string(),
                        pkg.display_name,
                        status
                    );
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
                    println!(
                        "{}\t{}\t{}\t{}",
                        path,
                        pkg.revision.to_string(),
                        pkg.display_name,
                        status
                    );
                }
            }
        }
    }
}

