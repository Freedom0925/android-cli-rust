use std::path::Path;
use std::fs;
use anyhow::{Result, Context};

/// Ensure a directory exists, creating it if necessary
pub fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    }
    Ok(())
}

/// Get file name from path
pub fn file_name(path: &Path) -> Option<String> {
    path.file_name().and_then(|n| n.to_str()).map(|s| s.to_string())
}

/// Check if a path has a specific extension
pub fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some(ext)
}