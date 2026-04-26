use crate::sdk::model::{Revision, Sdk, SdkEntry};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Local SDK scanner - reads installed packages from filesystem
pub struct LocalSdkScanner {
    sdk_path: PathBuf,
}

impl LocalSdkScanner {
    pub fn new(sdk_path: PathBuf) -> Self {
        Self { sdk_path }
    }

    /// Scan SDK directory for installed packages
    pub fn scan(&self) -> Result<Sdk> {
        let mut entries = Vec::new();

        // Scan standard SDK directories
        self.scan_directory("build-tools", &mut entries)?;
        self.scan_directory("platforms", &mut entries)?;
        self.scan_directory("platform-tools", &mut entries)?;
        self.scan_directory("tools", &mut entries)?;
        self.scan_directory("cmdline-tools", &mut entries)?;
        self.scan_directory("emulator", &mut entries)?;
        self.scan_directory("ndk", &mut entries)?;
        self.scan_directory("cmake", &mut entries)?;
        self.scan_directory("sources", &mut entries)?;

        Ok(Sdk::with_entries(entries))
    }

    /// Scan a specific package directory
    fn scan_directory(&self, package_type: &str, entries: &mut Vec<SdkEntry>) -> Result<()> {
        let dir = self.sdk_path.join(package_type);

        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Parse version from directory name or source.properties
            let revision = self.parse_version(&path, name);

            // Build full package path
            let full_path = if package_type == "cmdline-tools" {
                // cmdline-tools has nested structure: cmdline-tools/{version}/
                // Also handle the bin directory case
                if path.join("bin").exists() {
                    format!("{};{}", package_type, name)
                } else {
                    // This might be a subdirectory like cmdline-tools/latest/bin
                    continue;
                }
            } else if package_type == "build-tools" || package_type == "platforms" {
                format!("{};{}", package_type, name)
            } else if package_type == "ndk" {
                format!("{};{}", package_type, name)
            } else if package_type == "cmake" {
                format!("{};{}", package_type, name)
            } else {
                package_type.to_string()
            };

            entries.push(SdkEntry::new(full_path, revision));
        }

        Ok(())
    }

    /// Parse version from directory name or source.properties
    fn parse_version(&self, path: &Path, dir_name: &str) -> Revision {
        // First try source.properties
        let source_props = path.join("source.properties");
        if source_props.exists() {
            if let Ok(content) = fs::read_to_string(&source_props) {
                for line in content.lines() {
                    if line.starts_with("Pkg.Revision=") {
                        let version = line.split('=').nth(1).unwrap_or(dir_name);
                        return Revision::parse(version.trim()).unwrap_or_else(|| Revision::new(0));
                    }
                }
            }
        }

        // Try package.xml
        let package_xml = path.join("package.xml");
        if package_xml.exists() {
            if let Ok(content) = fs::read_to_string(&package_xml) {
                // Parse revision from XML
                for line in content.lines() {
                    if line.contains("<sdk:major>") {
                        // Extract major version
                        let major = self
                            .extract_xml_value(line, "sdk:major")
                            .and_then(|v| v.parse::<i32>().ok())
                            .unwrap_or(0);

                        let minor = content
                            .lines()
                            .find(|l| l.contains("<sdk:minor>"))
                            .and_then(|l| self.extract_xml_value(l, "sdk:minor"))
                            .and_then(|v| v.parse::<i32>().ok());

                        let micro = content
                            .lines()
                            .find(|l| l.contains("<sdk:micro>"))
                            .and_then(|l| self.extract_xml_value(l, "sdk:micro"))
                            .and_then(|v| v.parse::<i32>().ok());

                        return Revision {
                            major,
                            minor,
                            micro,
                            preview: None,
                        };
                    }
                }
            }
        }

        // Fallback: parse from directory name
        Revision::parse(dir_name).unwrap_or_else(|| Revision::new(0))
    }

    /// Extract value from XML tag
    fn extract_xml_value(&self, line: &str, tag: &str) -> Option<String> {
        let start_tag = format!("<{}>", tag);
        let end_tag = format!("</{}>", tag);

        if line.contains(&start_tag) {
            let start = line.find(&start_tag)? + start_tag.len();
            let end = line.find(&end_tag)?;
            Some(line[start..end].to_string())
        } else {
            None
        }
    }

    /// Get specific package info
    pub fn get_package_info(&self, package_path: &str) -> Result<Option<SdkEntry>> {
        let parts: Vec<&str> = package_path.split(';').collect();
        let base_path = self.sdk_path.join(parts[0]);

        if parts.len() > 1 {
            // Specific version
            let version_dir = base_path.join(parts[1]);
            if version_dir.exists() {
                let revision = self.parse_version(&version_dir, parts[1]);
                Ok(Some(SdkEntry::new(package_path.to_string(), revision)))
            } else {
                Ok(None)
            }
        } else {
            // Generic package - find highest version
            let mut highest: Option<(String, Revision)> = None;

            if base_path.exists() {
                for entry in fs::read_dir(&base_path)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        let revision = self.parse_version(&path, name);

                        match &highest {
                            None => highest = Some((name.to_string(), revision)),
                            Some((_, old_rev)) => {
                                if revision.cmp(old_rev) == std::cmp::Ordering::Greater {
                                    highest = Some((name.to_string(), revision));
                                }
                            }
                        }
                    }
                }
            }

            Ok(highest.map(|(name, rev)| SdkEntry::new(format!("{};{}", parts[0], name), rev)))
        }
    }

    /// Check if package is installed
    pub fn is_installed(&self, package_path: &str) -> bool {
        let parts: Vec<&str> = package_path.split(';').collect();
        let base_path = self.sdk_path.join(parts[0]);

        if parts.len() > 1 {
            base_path.join(parts[1]).exists()
        } else {
            base_path.exists()
        }
    }

    /// Get installed package count
    pub fn count(&self) -> Result<usize> {
        let sdk = self.scan()?;
        Ok(sdk.entries.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_scanner_new() {
        let temp = tempdir().unwrap();
        let scanner = LocalSdkScanner::new(temp.path().to_path_buf());

        let sdk = scanner.scan().unwrap();
        assert_eq!(sdk.entries.len(), 0);
    }
}
