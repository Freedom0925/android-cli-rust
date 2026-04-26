use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique key for UI element identification
///
/// Keys are computed from parent path and resourceId/index to create
/// a unique identifier for each element in the UI hierarchy.
///
/// Format: parent_key:resourceId (if unique among siblings)
///         parent_key:resourceId-index (if resourceId duplicated)
///         parent_key:index (if no resourceId)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    /// The key value string
    pub value: String,
}

impl Key {
    /// Create an empty key
    pub fn empty() -> Self {
        Self {
            value: String::new(),
        }
    }

    /// Create a new key with the given value
    pub fn new(value: String) -> Self {
        Self { value }
    }

    /// Create a key from parent key and resource-id
    pub fn from_resource_id(parent_key: &Key, resource_id: &str) -> Self {
        // Strip Android resource-id prefix (e.g., "com.app:id/" or ":id/")
        let simplified_id = if let Some(pos) = resource_id.rfind(":id/") {
            &resource_id[pos + 4..]
        } else {
            resource_id
        };

        if parent_key.value.is_empty() {
            Self::new(simplified_id.to_string())
        } else {
            Self::new(format!("{}:{}", parent_key.value, simplified_id))
        }
    }

    /// Create a key from parent key and resource-id with index
    pub fn from_resource_id_with_index(parent_key: &Key, resource_id: &str, index: usize) -> Self {
        let simplified_id = if let Some(pos) = resource_id.rfind(":id/") {
            &resource_id[pos + 4..]
        } else {
            resource_id
        };

        if parent_key.value.is_empty() {
            Self::new(format!("{}-{}", simplified_id, index))
        } else {
            Self::new(format!("{}:{}-{}", parent_key.value, simplified_id, index))
        }
    }

    /// Create a key from parent key and sibling index
    pub fn from_index(parent_key: &Key, index: usize) -> Self {
        if parent_key.value.is_empty() {
            Self::new(format!("{}", index))
        } else {
            Self::new(format!("{}:{}", parent_key.value, index))
        }
    }

    /// Check if this key is empty
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Get the length of the key value
    pub fn len(&self) -> usize {
        self.value.len()
    }

    /// Compute hash code for this key (used in serialization)
    pub fn hash_code(&self) -> u64 {
        // Simple hash function for key value
        let mut hash: u64 = 0;
        for c in self.value.chars() {
            hash = hash * 31 + c as u64;
        }
        hash
    }

    /// Get parent key from this key
    pub fn parent(&self) -> Option<Key> {
        if self.value.is_empty() {
            return None;
        }

        // Find last ':' separator
        let last_colon = self.value.rfind(':');
        if let Some(pos) = last_colon {
            Some(Key::new(self.value[..pos].to_string()))
        } else {
            None
        }
    }

    /// Get the local part of this key (after last ':')
    pub fn local_part(&self) -> &str {
        if self.value.is_empty() {
            return "";
        }

        let last_colon = self.value.rfind(':');
        if let Some(pos) = last_colon {
            &self.value[pos + 1..]
        } else {
            &self.value
        }
    }
}

impl Default for Key {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl From<String> for Key {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for Key {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_empty() {
        let key = Key::empty();
        assert!(key.is_empty());
        assert_eq!(key.len(), 0);
        assert_eq!(key.value, "");
    }

    #[test]
    fn test_key_new() {
        let key = Key::new("test_key".to_string());
        assert!(!key.is_empty());
        assert_eq!(key.len(), 8);
        assert_eq!(key.value, "test_key");
    }

    #[test]
    fn test_key_from_str() {
        let key: Key = "my_key".into();
        assert_eq!(key.value, "my_key");
    }

    #[test]
    fn test_key_from_string() {
        let key: Key = String::from("my_key").into();
        assert_eq!(key.value, "my_key");
    }

    #[test]
    fn test_key_from_resource_id() {
        let parent = Key::empty();
        let key = Key::from_resource_id(&parent, "com.app:id/button");
        assert_eq!(key.value, "button");

        let parent = Key::new("root".to_string());
        let key = Key::from_resource_id(&parent, "com.app:id/text");
        assert_eq!(key.value, "root:text");
    }

    #[test]
    fn test_key_from_resource_id_strip_prefix() {
        let parent = Key::new("root".to_string());
        let key = Key::from_resource_id(&parent, ":id/button");
        assert_eq!(key.value, "root:button");

        // No prefix to strip
        let key = Key::from_resource_id(&parent, "plain_id");
        assert_eq!(key.value, "root:plain_id");
    }

    #[test]
    fn test_key_from_resource_id_with_index() {
        let parent = Key::empty();
        let key = Key::from_resource_id_with_index(&parent, ":id/button", 2);
        assert_eq!(key.value, "button-2");

        let parent = Key::new("root".to_string());
        let key = Key::from_resource_id_with_index(&parent, ":id/item", 1);
        assert_eq!(key.value, "root:item-1");
    }

    #[test]
    fn test_key_from_index() {
        let parent = Key::empty();
        let key = Key::from_index(&parent, 5);
        assert_eq!(key.value, "5");

        let parent = Key::new("root".to_string());
        let key = Key::from_index(&parent, 3);
        assert_eq!(key.value, "root:3");
    }

    #[test]
    fn test_key_hash_code() {
        let key1 = Key::new("test".to_string());
        let key2 = Key::new("test".to_string());
        assert_eq!(key1.hash_code(), key2.hash_code());

        let key3 = Key::new("other".to_string());
        assert_ne!(key1.hash_code(), key3.hash_code());
    }

    #[test]
    fn test_key_parent() {
        let key = Key::new("root:child:grandchild".to_string());
        let parent = key.parent();
        assert!(parent.is_some());
        assert_eq!(parent.unwrap().value, "root:child");

        let key = Key::new("simple".to_string());
        let parent = key.parent();
        assert!(parent.is_none());

        let key = Key::empty();
        let parent = key.parent();
        assert!(parent.is_none());
    }

    #[test]
    fn test_key_local_part() {
        let key = Key::new("root:child".to_string());
        assert_eq!(key.local_part(), "child");

        let key = Key::new("simple".to_string());
        assert_eq!(key.local_part(), "simple");

        let key = Key::empty();
        assert_eq!(key.local_part(), "");
    }

    #[test]
    fn test_key_display() {
        let key = Key::new("test_key".to_string());
        let display = format!("{}", key);
        assert_eq!(display, "test_key");
    }

    #[test]
    fn test_key_equality() {
        let key1 = Key::new("same".to_string());
        let key2 = Key::new("same".to_string());
        let key3 = Key::new("different".to_string());

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_key_hash_consistency() {
        use std::collections::HashSet;

        let key1 = Key::new("test".to_string());
        let key2 = Key::new("test".to_string());

        let mut set = HashSet::new();
        set.insert(key1.clone());
        assert!(set.contains(&key2));
    }

    #[test]
    fn test_key_serialization() {
        let key = Key::new("root:button".to_string());
        let json = serde_json::to_string(&key).unwrap();
        assert!(json.contains("root:button"));

        let deserialized: Key = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, key);
    }

    #[test]
    fn test_key_default() {
        let key = Key::default();
        assert!(key.is_empty());
    }
}
