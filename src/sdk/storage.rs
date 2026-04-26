use anyhow::{Context, Result};
use sha1::{Digest, Sha1};
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use tracing::debug;
use walkdir::WalkDir;

use crate::sdk::model::Sdk;

/// Git-like Content-Addressable Storage for SDK packages
pub struct Storage {
    /// Base path (usually .android/cli/)
    pub base_path: PathBuf,
    /// Objects directory (storage/objects/<sha>)
    pub objects_dir: PathBuf,
    /// Archives directory (storage/archives/<sha>.zip)
    pub archives_dir: PathBuf,
    /// Unzipped directory (storage/unzipped/<sha>/)
    pub unzipped_dir: PathBuf,
    /// References directory (refs/)
    pub refs_dir: PathBuf,
}

impl Storage {
    /// Create a new Storage instance
    pub fn new(base_path: PathBuf) -> Result<Self> {
        let storage = Self {
            base_path: base_path.clone(),
            objects_dir: base_path.join("storage/objects"),
            archives_dir: base_path.join("storage/archives"),
            unzipped_dir: base_path.join("storage/unzipped"),
            refs_dir: base_path.join("refs"),
        };

        storage.ensure_dirs()?;
        Ok(storage)
    }

    /// Ensure all directories exist
    fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.objects_dir).with_context(|| {
            format!(
                "Failed to create objects dir: {}",
                self.objects_dir.display()
            )
        })?;
        fs::create_dir_all(&self.archives_dir).with_context(|| {
            format!(
                "Failed to create archives dir: {}",
                self.archives_dir.display()
            )
        })?;
        fs::create_dir_all(&self.unzipped_dir).with_context(|| {
            format!(
                "Failed to create unzipped dir: {}",
                self.unzipped_dir.display()
            )
        })?;
        fs::create_dir_all(&self.refs_dir)
            .with_context(|| format!("Failed to create refs dir: {}", self.refs_dir.display()))?;
        Ok(())
    }

    /// Calculate SHA-1 hash of data
    pub fn hash(data: &[u8]) -> String {
        let mut hasher = Sha1::new();
        hasher.update(data);
        let result = hasher.finalize();
        // Convert to hex string manually for sha1 0.11 compatibility
        result.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Save an SDK index and return its SHA
    pub fn save_sdk(&self, sdk: &Sdk) -> Result<String> {
        let data = sdk.to_protobuf();
        let sha = Self::hash(&data);

        let object_path = self.objects_dir.join(&sha);
        fs::write(&object_path, &data)
            .with_context(|| format!("Failed to write SDK object: {}", object_path.display()))?;

        Ok(sha)
    }

    /// Read an SDK index by SHA
    pub fn read_sdk(&self, sha: &str) -> Result<Sdk> {
        let object_path = self.objects_dir.join(sha);

        let data = fs::read(&object_path)
            .with_context(|| format!("Failed to read SDK object: {}", object_path.display()))?;

        Sdk::from_protobuf(&data)
    }

    /// Add a reference (named pointer to a SHA)
    pub fn add_ref(&self, name: &str, sha: &str) -> Result<()> {
        let ref_path = self.refs_dir.join(name);
        fs::write(&ref_path, sha)
            .with_context(|| format!("Failed to write ref: {}", ref_path.display()))?;
        Ok(())
    }

    /// Read a reference
    pub fn read_ref(&self, name: &str) -> Result<String> {
        let ref_path = self.refs_dir.join(name);

        if !ref_path.exists() {
            return Err(anyhow::anyhow!("Ref '{}' not found", name));
        }

        let sha = fs::read_to_string(&ref_path)
            .with_context(|| format!("Failed to read ref: {}", ref_path.display()))?;

        Ok(sha.trim().to_string())
    }

    /// Save an archive (downloaded ZIP) and verify SHA
    pub fn save_archive(&self, expected_sha: &str, data: &[u8]) -> Result<PathBuf> {
        // Verify SHA matches
        let actual_sha = Self::hash(data);
        if actual_sha != expected_sha {
            return Err(anyhow::anyhow!(
                "SHA mismatch: expected {}, got {}",
                expected_sha,
                actual_sha
            ));
        }

        let archive_path = self.archives_dir.join(format!("{}.zip", expected_sha));
        fs::write(&archive_path, data)
            .with_context(|| format!("Failed to write archive: {}", archive_path.display()))?;

        Ok(archive_path)
    }

    /// Check if an archive exists
    pub fn has_archive(&self, sha: &str) -> bool {
        self.archives_dir.join(format!("{}.zip", sha)).exists()
    }

    /// Unzip an archive to the unzipped directory
    pub fn unzip(&self, sha: &str) -> Result<PathBuf> {
        let archive_path = self.archives_dir.join(format!("{}.zip", sha));
        let unzipped_path = self.unzipped_dir.join(sha);

        debug!(
            archive_path = %archive_path.display(),
            unzipped_path = %unzipped_path.display(),
            unzipped_dir = %self.unzipped_dir.display(),
            "unzip: starting"
        );

        // Check if already unzipped with content
        if unzipped_path.exists() {
            let has_content = std::fs::read_dir(&unzipped_path)
                .map(|mut dir| dir.next().is_some())
                .unwrap_or(false);
            if has_content {
                debug!("unzip: already exists with content");
                return Ok(unzipped_path);
            }
        }

        // Ensure parent directory exists
        fs::create_dir_all(&self.unzipped_dir).with_context(|| {
            format!(
                "Failed to create unzipped_dir: {}",
                self.unzipped_dir.display()
            )
        })?;

        // Create target directory
        fs::create_dir_all(&unzipped_path).with_context(|| {
            format!(
                "Failed to create unzipped_path: {}",
                unzipped_path.display()
            )
        })?;

        let file = fs::File::open(&archive_path)
            .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

        // Use BufReader for better performance and compatibility with zip crate
        let reader = BufReader::new(file);
        let mut archive = zip::ZipArchive::new(reader)
            .with_context(|| format!("Failed to read ZIP archive: {}", archive_path.display()))?;

        debug!(entries = archive.len(), "unzip: archive opened");

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => unzipped_path.join(path),
                None => continue,
            };

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }

        debug!("unzip: completed successfully");

        Ok(unzipped_path)
    }

    /// Install unzipped package to SDK path
    pub fn install_to_sdk(&self, sha: &str, sdk_path: &Path, package_path: &str) -> Result<()> {
        let unzipped_path = self.unzipped_dir.join(sha);
        let target_path = sdk_path.join(package_path);

        // Remove existing if present
        if target_path.exists() {
            fs::remove_dir_all(&target_path)?;
        }

        // Copy the unzipped content
        fs::create_dir_all(&target_path)?;

        // Find the actual content directory (skip top-level directory in ZIP)
        // ZIP structure is typically: android-<version>/<files>
        // We want to copy <files> directly to target_path
        let content_dir = fs::read_dir(&unzipped_path)?
            .next()
            .and_then(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| unzipped_path.clone());

        for entry in WalkDir::new(&content_dir) {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(&content_dir)?;
            let dest = target_path.join(relative);

            if entry.file_type().is_dir() {
                fs::create_dir_all(&dest)?;
            } else {
                fs::copy(path, &dest)?;
            }
        }

        Ok(())
    }

    /// Garbage collect - remove unreferenced objects and archives
    pub fn gc(&self) -> Result<()> {
        // Find all referenced SHAs
        let referenced: Vec<String> = self.get_all_referenced()?;

        // Clean unreferenced objects
        for entry in fs::read_dir(&self.objects_dir)? {
            let entry = entry?;
            let sha = entry.file_name().to_string_lossy().to_string();
            if !referenced.contains(&sha) {
                fs::remove_file(entry.path())?;
            }
        }

        // Clean unreferenced archives
        for entry in fs::read_dir(&self.archives_dir)? {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().to_string();
            let sha = filename.replace(".zip", "");
            if !referenced.contains(&sha) {
                fs::remove_file(entry.path())?;
            }
        }

        // Clean unreferenced unzipped directories
        for entry in fs::read_dir(&self.unzipped_dir)? {
            let entry = entry?;
            let sha = entry.file_name().to_string_lossy().to_string();
            if !referenced.contains(&sha) {
                fs::remove_dir_all(entry.path())?;
            }
        }

        Ok(())
    }

    /// Get all referenced SHAs from refs directory
    pub fn get_all_referenced(&self) -> Result<Vec<String>> {
        let mut referenced = Vec::new();

        for entry in fs::read_dir(&self.refs_dir)? {
            let entry = entry?;
            let content = fs::read_to_string(entry.path())?;
            referenced.push(content.trim().to_string());
        }

        Ok(referenced)
    }

    /// Clear all storage
    pub fn clear(&self) -> Result<()> {
        if self.objects_dir.exists() {
            fs::remove_dir_all(&self.objects_dir)?;
        }
        if self.archives_dir.exists() {
            fs::remove_dir_all(&self.archives_dir)?;
        }
        if self.unzipped_dir.exists() {
            fs::remove_dir_all(&self.unzipped_dir)?;
        }
        if self.refs_dir.exists() {
            fs::remove_dir_all(&self.refs_dir)?;
        }

        self.ensure_dirs()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::model::{Revision, SdkEntry};
    use tempfile::tempdir;

    #[test]
    fn test_storage_save_read_sdk() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();

        let sdk = Sdk::with_entries(vec![SdkEntry::new(
            "build-tools".to_string(),
            Revision::parse("34.0.0").unwrap(),
        )]);

        let sha = storage.save_sdk(&sdk).unwrap();
        assert!(!sha.is_empty());

        let read_sdk = storage.read_sdk(&sha).unwrap();
        assert_eq!(read_sdk.entries.len(), 1);
        assert_eq!(read_sdk.entries[0].path, "build-tools");
    }

    #[test]
    fn test_storage_refs() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();

        storage.add_ref("head", "abc123").unwrap();
        let sha = storage.read_ref("head").unwrap();
        assert_eq!(sha, "abc123");
    }

    #[test]
    fn test_hash() {
        let data = b"test data";
        let hash = Storage::hash(data);
        assert_eq!(hash.len(), 40); // SHA-1 produces 40 hex chars
    }

    #[test]
    fn test_hash_consistency() {
        let data = b"hello world";
        let hash1 = Storage::hash(data);
        let hash2 = Storage::hash(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_known_value() {
        // SHA-1 of empty string
        let hash = Storage::hash(b"");
        assert_eq!(hash, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn test_hash_different_inputs() {
        let hash1 = Storage::hash(b"test1");
        let hash2 = Storage::hash(b"test2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_sha_verification() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();

        let data = b"test archive content";
        let expected_sha = Storage::hash(data);

        // Save archive with correct SHA
        let result = storage.save_archive(&expected_sha, data);
        assert!(result.is_ok());
        assert!(storage.has_archive(&expected_sha));
    }

    #[test]
    fn test_sha_verification_wrong_hash() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();

        let data = b"test archive content";
        let wrong_sha = "wrong_hash_value_12345";

        // Attempt to save with wrong SHA - should fail because SHA mismatch
        let result = storage.save_archive(wrong_sha, data);
        assert!(result.is_err());

        // Verify that correct hash would work
        let correct_hash = Storage::hash(data);
        let result2 = storage.save_archive(&correct_hash, data);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_save_archive_overwrite() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();

        // Use correct SHA for each content
        let content1 = b"first content";
        let sha1 = Storage::hash(content1);
        storage.save_archive(&sha1, content1).unwrap();

        let content2 = b"second content";
        let sha2 = Storage::hash(content2);
        storage.save_archive(&sha2, content2).unwrap();

        // Each archive should have its own SHA
        assert!(storage.has_archive(&sha1));
        assert!(storage.has_archive(&sha2));

        let path1 = storage.archives_dir.join(format!("{}.zip", sha1));
        let read1 = fs::read(&path1).unwrap();
        assert_eq!(read1, content1);

        let path2 = storage.archives_dir.join(format!("{}.zip", sha2));
        let read2 = fs::read(&path2).unwrap();
        assert_eq!(read2, content2);
    }

    #[test]
    fn test_clear() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path().to_path_buf()).unwrap();

        // Add some content
        storage.add_ref("test", "value").unwrap();
        storage.clear().unwrap();

        // Ref should be gone
        assert!(storage.read_ref("test").is_err());
    }
}
