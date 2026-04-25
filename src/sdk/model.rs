use serde::{Deserialize, Serialize};

/// Version revision number
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    pub major: i32,
    pub minor: Option<i32>,
    pub micro: Option<i32>,
    pub preview: Option<i32>,
}

impl Revision {
    pub fn new(major: i32) -> Self {
        Self {
            major,
            minor: None,
            micro: None,
            preview: None,
        }
    }

    pub fn full(major: i32, minor: i32, micro: i32) -> Self {
        Self {
            major,
            minor: Some(minor),
            micro: Some(micro),
            preview: None,
        }
    }

    /// Parse version string like "34.0.0" or "34"
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        let major = parts[0].parse::<i32>().ok()?;
        let minor = parts.get(1).and_then(|p| p.parse::<i32>().ok());
        let micro = parts.get(2).and_then(|p| p.parse::<i32>().ok());
        let preview = parts.get(3).and_then(|p| p.parse::<i32>().ok());

        Some(Self {
            major,
            minor,
            micro,
            preview,
        })
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        let mut result = self.major.to_string();
        if let Some(minor) = self.minor {
            result.push_str(&format!(".{}", minor));
        }
        if let Some(micro) = self.micro {
            result.push_str(&format!(".{}", micro));
        }
        if let Some(preview) = self.preview {
            result.push_str(&format!("-rc{}", preview));
        }
        result
    }

    /// Compare two revisions
    pub fn cmp(&self, other: &Revision) -> std::cmp::Ordering {
        // Compare major first
        let major_cmp = self.major.cmp(&other.major);
        if major_cmp != std::cmp::Ordering::Equal {
            return major_cmp;
        }

        // Compare minor (treat None as 0)
        let self_minor = self.minor.unwrap_or(0);
        let other_minor = other.minor.unwrap_or(0);
        let minor_cmp = self_minor.cmp(&other_minor);
        if minor_cmp != std::cmp::Ordering::Equal {
            return minor_cmp;
        }

        // Compare micro (treat None as 0)
        let self_micro = self.micro.unwrap_or(0);
        let other_micro = other.micro.unwrap_or(0);
        let micro_cmp = self_micro.cmp(&other_micro);
        if micro_cmp != std::cmp::Ordering::Equal {
            return micro_cmp;
        }

        // Compare preview (treat None as stable, i.e., larger than any preview)
        match (self.preview, other.preview) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater, // stable > preview
            (Some(_), None) => std::cmp::Ordering::Less,     // preview < stable
            (Some(s), Some(o)) => s.cmp(&o),
        }
    }
}

/// SDK package entry (installed package)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkEntry {
    /// Package path (e.g., "build-tools", "platforms")
    pub path: String,
    /// Version revision
    pub revision: Revision,
    /// Download URL (optional for local entries)
    pub url: Option<String>,
    /// File size in bytes
    pub size: u64,
    /// SHA-1 hash of the archive
    pub sha1: String,
}

impl SdkEntry {
    pub fn new(path: String, revision: Revision) -> Self {
        Self {
            path,
            revision,
            url: None,
            size: 0,
            sha1: String::new(),
        }
    }

    pub fn with_archive(path: String, revision: Revision, url: String, size: u64, sha1: String) -> Self {
        Self {
            path,
            revision,
            url: Some(url),
            size,
            sha1,
        }
    }
}

/// SDK index (collection of entries)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sdk {
    pub entries: Vec<SdkEntry>,
}

impl Sdk {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn with_entries(entries: Vec<SdkEntry>) -> Self {
        Self { entries }
    }

    /// Find entry by path
    pub fn find(&self, path: &str) -> Option<&SdkEntry> {
        self.entries.iter().find(|e| e.path == path)
    }

    /// Find entry by path and version
    pub fn find_with_version(&self, path: &str, revision: &Revision) -> Option<&SdkEntry> {
        self.entries.iter().find(|e| e.path == path && e.revision.cmp(&revision) == std::cmp::Ordering::Equal)
    }

    /// Delete entry by path
    pub fn delete(&self, path: &str) -> Self {
        Self {
            entries: self.entries.iter().filter(|e| e.path != path).cloned().collect(),
        }
    }

    /// Update/merge with another SDK
    pub fn update(&self, other: &Sdk) -> Self {
        let mut entries = self.entries.clone();

        for entry in &other.entries {
            // Find existing entry with same path
            let existing_idx = entries.iter().position(|e| e.path == entry.path);

            if let Some(idx) = existing_idx {
                // Replace if new version is higher
                if entry.revision.cmp(&entries[idx].revision) >= std::cmp::Ordering::Equal {
                    entries[idx] = entry.clone();
                }
            } else {
                // Add new entry
                entries.push(entry.clone());
            }
        }

        Self { entries }
    }

    /// Calculate diff between two SDKs
    /// Returns (common, changed, removed)
    /// - common: entries with same path and version in both SDKs
    /// - changed: entries in other that are new or have different version
    /// - removed: entries in self that don't exist in other
    pub fn diff(&self, other: &Sdk) -> (Sdk, Sdk, Sdk) {
        let common = Sdk::with_entries(
            self.entries.iter()
                .filter(|e| {
                    if let Some(other_entry) = other.find(&e.path) {
                        e.revision.cmp(&other_entry.revision) == std::cmp::Ordering::Equal
                    } else {
                        false
                    }
                })
                .cloned()
                .collect()
        );

        let changed = Sdk::with_entries(
            other.entries.iter()
                .filter(|e| {
                    let self_entry = self.find(&e.path);
                    match self_entry {
                        None => true, // New entry
                        Some(s) => e.revision.cmp(&s.revision) != std::cmp::Ordering::Equal, // Different version
                    }
                })
                .cloned()
                .collect()
        );

        let removed = Sdk::with_entries(
            self.entries.iter()
                .filter(|e| other.find(&e.path).is_none())
                .cloned()
                .collect()
        );

        (common, changed, removed)
    }

    /// Serialize to protobuf bytes (uses proper protobuf format)
    pub fn to_protobuf(&self) -> Vec<u8> {
        crate::sdk::protobuf::sdk_to_protobuf(self)
    }

    /// Deserialize from protobuf bytes
    pub fn from_protobuf(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        crate::sdk::protobuf::sdk_from_protobuf(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize SDK: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_revision_parse() {
        let r = Revision::parse("34.0.0").unwrap();
        assert_eq!(r.major, 34);
        assert_eq!(r.minor, Some(0));
        assert_eq!(r.micro, Some(0));

        let r2 = Revision::parse("34").unwrap();
        assert_eq!(r2.major, 34);
        assert_eq!(r2.minor, None);
    }

    #[test]
    fn test_revision_cmp() {
        let r1 = Revision::parse("34.0.0").unwrap();
        let r2 = Revision::parse("35.0.0").unwrap();
        assert_eq!(r1.cmp(&r2), std::cmp::Ordering::Less);

        let r3 = Revision::parse("34.0.0").unwrap();
        let r4 = Revision::parse("34.0.1").unwrap();
        assert_eq!(r3.cmp(&r4), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_sdk_diff() {
        let sdk1 = Sdk::with_entries(vec![
            SdkEntry::new("build-tools".to_string(), Revision::parse("34.0.0").unwrap()),
            SdkEntry::new("platforms".to_string(), Revision::parse("34").unwrap()),
        ]);

        let sdk2 = Sdk::with_entries(vec![
            SdkEntry::new("build-tools".to_string(), Revision::parse("35.0.0").unwrap()),
            SdkEntry::new("platforms".to_string(), Revision::parse("34").unwrap()),
        ]);

        let (common, changed, removed) = sdk1.diff(&sdk2);

        assert_eq!(common.entries.len(), 1); // platforms
        assert_eq!(changed.entries.len(), 1); // build-tools updated
        assert_eq!(removed.entries.len(), 0);
    }
}