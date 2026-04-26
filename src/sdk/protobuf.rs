// Include prost-generated protobuf types
// This file bridges the generated types with our model types

// Import generated types with renamed aliases to avoid conflict
#[allow(dead_code)]
mod proto_types {
    // Include the generated protobuf types
    // The prost build generates these in OUT_DIR/android.sdk.rs
    include!(concat!(env!("OUT_DIR"), "/android.sdk.rs"));
}

// Import generated types for internal conversions
use proto_types::Revision as RevisionProto;
use proto_types::Sdk as SdkProto;
use proto_types::SdkEntry as SdkEntryProto;

// Export proto types for use in repository.rs
pub use proto_types::{
    Architecture, Artifact, Archive, Channel, Dependency, Package, PackageList, Platform,
};

/// Helper methods for Platform enum
impl Platform {
    /// Get current platform from system
    pub fn current() -> Self {
        match std::env::consts::OS {
            "macos" => Platform::Mac,
            "windows" => Platform::Windows,
            _ => Platform::Linux,
        }
    }
}

/// Helper methods for Architecture enum
impl Architecture {
    /// Get current architecture from system
    pub fn current() -> Self {
        match std::env::consts::ARCH {
            "x86" | "i686" => Architecture::X86,
            "x86_64" | "amd64" => Architecture::X64,
            "aarch64" | "arm64" => Architecture::Aarch64,
            _ => Architecture::X64,
        }
    }
}

use crate::sdk::model::{Revision, Sdk, SdkEntry};
use prost::Message;

impl From<&Revision> for RevisionProto {
    fn from(rev: &Revision) -> Self {
        RevisionProto {
            major: rev.major,
            minor: rev.minor.unwrap_or(0),
            micro: rev.micro.unwrap_or(0),
            preview: rev.preview.unwrap_or(0),
        }
    }
}

impl From<&RevisionProto> for Revision {
    fn from(proto: &RevisionProto) -> Self {
        Revision {
            major: proto.major,
            minor: if proto.minor != 0 {
                Some(proto.minor)
            } else {
                None
            },
            micro: if proto.micro != 0 {
                Some(proto.micro)
            } else {
                None
            },
            preview: if proto.preview != 0 {
                Some(proto.preview)
            } else {
                None
            },
        }
    }
}

impl From<&SdkEntry> for SdkEntryProto {
    fn from(entry: &SdkEntry) -> Self {
        SdkEntryProto {
            path: entry.path.clone(),
            revision: Some(RevisionProto::from(&entry.revision)),
            url: entry.url.clone().unwrap_or_default(),
            size: entry.size as i64,
            sha1: entry.sha1.clone(),
        }
    }
}

impl From<&SdkEntryProto> for SdkEntry {
    fn from(proto: &SdkEntryProto) -> Self {
        SdkEntry {
            path: proto.path.clone(),
            revision: proto
                .revision
                .as_ref()
                .map(|r| Revision::from(r))
                .unwrap_or_else(|| Revision::new(0)),
            url: if proto.url.is_empty() {
                None
            } else {
                Some(proto.url.clone())
            },
            size: proto.size as u64,
            sha1: proto.sha1.clone(),
        }
    }
}

impl From<&Sdk> for SdkProto {
    fn from(sdk: &Sdk) -> Self {
        SdkProto {
            entries: sdk.entries.iter().map(|e| SdkEntryProto::from(e)).collect(),
        }
    }
}

impl From<&SdkProto> for Sdk {
    fn from(proto: &SdkProto) -> Self {
        Sdk {
            entries: proto.entries.iter().map(|e| SdkEntry::from(e)).collect(),
        }
    }
}

/// Serialize SDK to protobuf bytes
pub fn sdk_to_protobuf(sdk: &Sdk) -> Vec<u8> {
    let proto = SdkProto::from(sdk);
    proto.encode_to_vec()
}

/// Deserialize SDK from protobuf bytes
pub fn sdk_from_protobuf(bytes: &[u8]) -> Result<Sdk, prost::DecodeError> {
    let proto = SdkProto::decode(bytes)?;
    Ok(Sdk::from(&proto))
}

/// Serialize Revision to protobuf bytes
pub fn revision_to_protobuf(rev: &Revision) -> Vec<u8> {
    let proto = RevisionProto::from(rev);
    proto.encode_to_vec()
}

/// Deserialize Revision from protobuf bytes
pub fn revision_from_protobuf(bytes: &[u8]) -> Result<Revision, prost::DecodeError> {
    let proto = RevisionProto::decode(bytes)?;
    Ok(Revision::from(&proto))
}

/// Deserialize PackageList from protobuf bytes
pub fn package_list_from_protobuf(bytes: &[u8]) -> Result<PackageList, prost::DecodeError> {
    PackageList::decode(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::model::{Revision, Sdk, SdkEntry};

    #[test]
    fn test_revision_protobuf_roundtrip() {
        let rev = Revision::full(34, 1, 3); // Use non-zero values to avoid protobuf default ambiguity
        let bytes = revision_to_protobuf(&rev);
        let parsed = revision_from_protobuf(&bytes).unwrap();
        assert_eq!(rev.major, parsed.major);
        assert_eq!(rev.minor, parsed.minor);
        assert_eq!(rev.micro, parsed.micro);
    }

    #[test]
    fn test_revision_protobuf_simple() {
        let rev = Revision::new(35);
        let bytes = revision_to_protobuf(&rev);
        let parsed = revision_from_protobuf(&bytes).unwrap();
        assert_eq!(rev.major, parsed.major);
    }

    #[test]
    fn test_sdk_protobuf_roundtrip() {
        let sdk = Sdk::with_entries(vec![
            SdkEntry::new(
                "build-tools".to_string(),
                Revision::parse("34.0.0").unwrap(),
            ),
            SdkEntry::new("platforms".to_string(), Revision::parse("34").unwrap()),
        ]);

        let bytes = sdk_to_protobuf(&sdk);
        let parsed = sdk_from_protobuf(&bytes).unwrap();

        assert_eq!(sdk.entries.len(), parsed.entries.len());
        assert_eq!(sdk.entries[0].path, parsed.entries[0].path);
    }

    #[test]
    fn test_sdk_protobuf_empty() {
        let sdk = Sdk::new();
        let bytes = sdk_to_protobuf(&sdk);
        let parsed = sdk_from_protobuf(&bytes).unwrap();
        assert_eq!(parsed.entries.len(), 0);
    }

    #[test]
    fn test_sdk_entry_with_url() {
        let entry = SdkEntry::with_archive(
            "build-tools;34.0.0".to_string(),
            Revision::parse("34.0.0").unwrap(),
            "https://example.com/file.zip".to_string(),
            1024000,
            "abc123".to_string(),
        );

        let bytes = sdk_to_protobuf(&Sdk::with_entries(vec![entry.clone()]));
        let parsed = sdk_from_protobuf(&bytes).unwrap();

        assert_eq!(
            parsed.entries[0].url,
            Some("https://example.com/file.zip".to_string())
        );
        assert_eq!(parsed.entries[0].size, 1024000);
        assert_eq!(parsed.entries[0].sha1, "abc123");
    }

    #[test]
    fn test_protobuf_bytes_non_empty() {
        let rev = Revision::new(34);
        let bytes = revision_to_protobuf(&rev);
        assert!(!bytes.is_empty());
    }
}
