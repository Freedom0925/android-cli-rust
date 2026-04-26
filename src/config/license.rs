use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// License manager for SDK package installation
pub struct LicenseManager {
    /// Path to store accepted licenses
    licenses_path: PathBuf,
    /// Set of accepted license IDs
    accepted: HashSet<String>,
}

impl LicenseManager {
    pub fn new(android_user_home: &Path) -> Result<Self> {
        let licenses_path = android_user_home.join("cli").join("licenses");

        // Ensure directory exists
        fs::create_dir_all(&licenses_path)?;

        // Load existing accepted licenses
        let accepted = Self::load_accepted(&licenses_path)?;

        Ok(Self {
            licenses_path,
            accepted,
        })
    }

    /// Load accepted licenses from storage
    fn load_accepted(path: &Path) -> Result<HashSet<String>> {
        let mut accepted = HashSet::new();

        if !path.exists() {
            return Ok(accepted);
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path
                .extension()
                .map(|e| e == "license")
                .unwrap_or(false)
            {
                // Read license hash from file
                if let Ok(content) = fs::read_to_string(&file_path) {
                    let id = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.replace(".license", ""))
                        .unwrap_or_default();

                    if content.trim() == "accepted" {
                        accepted.insert(id);
                    }
                }
            }
        }

        Ok(accepted)
    }

    /// Check if a license is accepted
    pub fn is_accepted(&self, license_id: &str) -> bool {
        self.accepted.contains(license_id)
    }

    /// Accept a license
    pub fn accept(&mut self, license_id: &str, license_content: &str) -> Result<()> {
        // Display license and prompt for acceptance
        println!("\n{}", license_content);
        println!("\n----------------------------------------");
        println!("To install this package, you must accept the license.");
        println!("Type 'y' to accept, 'n' to reject, or 's' to skip this package.");

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let answer = input.trim().to_lowercase();

        if answer == "y" || answer == "yes" {
            // Store acceptance
            let license_file = self.licenses_path.join(format!("{}.license", license_id));
            fs::write(&license_file, "accepted")?;

            self.accepted.insert(license_id.to_string());

            println!("License accepted.");
            Ok(())
        } else if answer == "s" || answer == "skip" {
            println!("Package skipped.");
            Err(anyhow::anyhow!("License skipped"))
        } else {
            println!("License rejected. Package will not be installed.");
            Err(anyhow::anyhow!("License rejected"))
        }
    }

    /// Accept license without prompting (for --force mode)
    pub fn accept_force(&mut self, license_id: &str) -> Result<()> {
        let license_file = self.licenses_path.join(format!("{}.license", license_id));
        fs::write(&license_file, "accepted")?;

        self.accepted.insert(license_id.to_string());

        Ok(())
    }

    /// Prompt to accept all unaccepted licenses
    pub fn prompt_all(&mut self, licenses: &[(String, String)]) -> Result<()> {
        let unaccepted: Vec<_> = licenses
            .iter()
            .filter(|(id, _)| !self.is_accepted(id))
            .collect();

        if unaccepted.is_empty() {
            return Ok(());
        }

        println!("\nThe following licenses need to be accepted:");
        for (id, _) in &unaccepted {
            println!("  - {}", id);
        }

        println!("\nType 'y' to accept all, 'n' to reject, or 'i' to review individually.");

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let answer = input.trim().to_lowercase();

        if answer == "y" || answer == "yes" {
            for (id, content) in unaccepted {
                self.accept_force(id)?;
            }
            println!("All licenses accepted.");
            Ok(())
        } else if answer == "i" || answer == "individually" {
            for (id, content) in unaccepted {
                self.accept(id, content)?;
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Licenses not accepted"))
        }
    }

    /// Get license content from repository
    pub fn fetch_license_content(&self, license_name: &str) -> Result<String> {
        // Try to read from known license files
        let known_licenses = [
            ("android-sdk-license", "Android SDK License"),
            ("android-sdk-preview-license", "Android SDK Preview License"),
            ("intel-android-extra-license", "Intel Android Extra License"),
            ("mips-android-extra-license", "MIPS Android Extra License"),
            ("google-gdk-license", "Google GDK License"),
        ];

        // For now, return a standard license text
        Ok(format!(
            "License: {}\n\n\
            This package requires acceptance of the following license agreement:\n\n\
            By downloading and using this SDK package, you agree to the terms and conditions\n\
            of the applicable license agreement. Please review the license at:\n\
            https://developer.android.com/studio/terms\n\n\
            Package: {}",
            license_name, license_name
        ))
    }
}

/// Common SDK licenses
pub const SDK_LICENSE: &str = "android-sdk-license";
pub const SDK_PREVIEW_LICENSE: &str = "android-sdk-preview-license";
pub const INTEL_EXTRA_LICENSE: &str = "intel-android-extra-license";
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_license_manager_new() {
        let dir = tempdir().unwrap();
        let manager = LicenseManager::new(dir.path()).unwrap();
        assert!(!manager.is_accepted("test-license"));
    }

    #[test]
    fn test_license_accept_force() {
        let dir = tempdir().unwrap();
        let mut manager = LicenseManager::new(dir.path()).unwrap();

        manager.accept_force("test-license").unwrap();
        assert!(manager.is_accepted("test-license"));
    }

    #[test]
    fn test_license_persistence() {
        let dir = tempdir().unwrap();

        // Create and accept license
        let mut manager1 = LicenseManager::new(dir.path()).unwrap();
        manager1.accept_force("persistent-license").unwrap();

        // Create new manager - should load existing
        let manager2 = LicenseManager::new(dir.path()).unwrap();
        assert!(manager2.is_accepted("persistent-license"));
    }

    #[test]
    fn test_multiple_licenses() {
        let dir = tempdir().unwrap();
        let mut manager = LicenseManager::new(dir.path()).unwrap();

        manager.accept_force("license1").unwrap();
        manager.accept_force("license2").unwrap();
        manager.accept_force("license3").unwrap();

        assert!(manager.is_accepted("license1"));
        assert!(manager.is_accepted("license2"));
        assert!(manager.is_accepted("license3"));
        assert!(!manager.is_accepted("license4"));
    }

    #[test]
    fn test_fetch_license_content() {
        let dir = tempdir().unwrap();
        let manager = LicenseManager::new(dir.path()).unwrap();

        let content = manager
            .fetch_license_content("android-sdk-license")
            .unwrap();
        assert!(content.contains("android-sdk-license"));
    }
}
