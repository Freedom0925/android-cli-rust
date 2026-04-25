use crate::sdk::model::{Sdk, SdkEntry};

/// Calculate detailed diff between two SDK states
pub struct SdkDiff {
    /// Entries unchanged (same path and version)
    pub unchanged: Vec<SdkEntry>,
    /// Entries updated (same path, different version)
    pub updated: Vec<(SdkEntry, SdkEntry)>, // (old, new)
    /// Entries added (new in target)
    pub added: Vec<SdkEntry>,
    /// Entries removed (missing in target)
    pub removed: Vec<SdkEntry>,
}

impl SdkDiff {
    /// Calculate diff between two SDK states
    pub fn calculate(source: &Sdk, target: &Sdk) -> Self {
        let mut unchanged = Vec::new();
        let mut updated = Vec::new();
        let mut added = Vec::new();
        let mut removed = Vec::new();

        // Check source entries against target
        for source_entry in &source.entries {
            if let Some(target_entry) = target.find(&source_entry.path) {
                // Same path exists in target
                if source_entry.revision.cmp(&target_entry.revision) == std::cmp::Ordering::Equal {
                    // Same version - unchanged
                    unchanged.push(source_entry.clone());
                } else {
                    // Different version - updated
                    updated.push((source_entry.clone(), target_entry.clone()));
                }
            } else {
                // Path not in target - removed
                removed.push(source_entry.clone());
            }
        }

        // Check for new entries in target
        for target_entry in &target.entries {
            if source.find(&target_entry.path).is_none() {
                added.push(target_entry.clone());
            }
        }

        Self {
            unchanged,
            updated,
            added,
            removed,
        }
    }

    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        !self.updated.is_empty() || !self.added.is_empty() || !self.removed.is_empty()
    }

    /// Get total number of changes
    pub fn change_count(&self) -> usize {
        self.updated.len() + self.added.len() + self.removed.len()
    }

    /// Print diff summary
    pub fn print_summary(&self) {
        if self.unchanged.len() > 0 {
            println!("Unchanged: {} packages", self.unchanged.len());
        }
        if self.updated.len() > 0 {
            println!("Updated: {} packages", self.updated.len());
            for (old, new) in &self.updated {
                println!("  {} {} -> {}", old.path, old.revision.to_string(), new.revision.to_string());
            }
        }
        if self.added.len() > 0 {
            println!("Added: {} packages", self.added.len());
            for entry in &self.added {
                println!("  {} {}", entry.path, entry.revision.to_string());
            }
        }
        if self.removed.len() > 0 {
            println!("Removed: {} packages", self.removed.len());
            for entry in &self.removed {
                println!("  {} {}", entry.path, entry.revision.to_string());
            }
        }
    }

    /// Merge changes into a single SDK representing the operations needed
    pub fn to_operations(&self) -> SdkOperations {
        SdkOperations {
            install: self.added.clone(),
            update: self.updated.iter().map(|(_, new)| new.clone()).collect(),
            remove: self.removed.clone(),
        }
    }
}

/// Operations needed to transform source SDK to target SDK
pub struct SdkOperations {
    /// Packages to install
    pub install: Vec<SdkEntry>,
    /// Packages to update
    pub update: Vec<SdkEntry>,
    /// Packages to remove
    pub remove: Vec<SdkEntry>,
}

impl SdkOperations {
    /// Check if any operations needed
    pub fn is_empty(&self) -> bool {
        self.install.is_empty() && self.update.is_empty() && self.remove.is_empty()
    }

    /// Get total operations count
    pub fn total_count(&self) -> usize {
        self.install.len() + self.update.len() + self.remove.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::model::Revision;

    #[test]
    fn test_diff_empty() {
        let sdk1 = Sdk::with_entries(vec![]);
        let sdk2 = Sdk::with_entries(vec![]);

        let diff = SdkDiff::calculate(&sdk1, &sdk2);

        assert_eq!(diff.unchanged.len(), 0);
        assert_eq!(diff.updated.len(), 0);
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.removed.len(), 0);
        assert!(!diff.has_changes());
    }

    #[test]
    fn test_diff_add() {
        let sdk1 = Sdk::with_entries(vec![]);
        let sdk2 = Sdk::with_entries(vec![
            SdkEntry::new("build-tools".to_string(), Revision::new(34)),
        ]);

        let diff = SdkDiff::calculate(&sdk1, &sdk2);

        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 0);
        assert!(diff.has_changes());
    }

    #[test]
    fn test_diff_update() {
        let sdk1 = Sdk::with_entries(vec![
            SdkEntry::new("build-tools".to_string(), Revision::new(33)),
        ]);
        let sdk2 = Sdk::with_entries(vec![
            SdkEntry::new("build-tools".to_string(), Revision::new(34)),
        ]);

        let diff = SdkDiff::calculate(&sdk1, &sdk2);

        assert_eq!(diff.unchanged.len(), 0);
        assert_eq!(diff.updated.len(), 1);
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.removed.len(), 0);
    }
}