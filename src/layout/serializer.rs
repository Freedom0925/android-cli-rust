use serde_json::{json, Value};
use std::collections::HashMap;

use crate::layout::{UiNode, Key};
use crate::vision::Rect;

/// Element serializer for JSON output
///
/// Serializes UI elements in a format similar to the original Kotlin ElementSerializer:
/// - text (if not blank)
/// - content-desc (if not blank)
/// - interactions (as array)
/// - state (as array)
/// - center (Point coordinates)
/// - bounds (for scrollable elements)
/// - resource-id (simplified, strip ":id/")
/// - key (hash code)
/// - off-screen flag
pub struct ElementSerializer {
    /// Whether to recursively serialize children
    recursive: bool,
}

impl Default for ElementSerializer {
    fn default() -> Self {
        Self { recursive: true }
    }
}

impl ElementSerializer {
    /// Create a new element serializer
    pub fn new(recursive: bool) -> Self {
        Self { recursive }
    }

    /// Create a non-recursive serializer
    pub fn flat() -> Self {
        Self { recursive: false }
    }

    /// Serialize a single node to JSON object
    pub fn to_json_object(node: &UiNode) -> Value {
        let mut obj = serde_json::Map::new();

        // Add text if not blank
        if let Some(text) = node.get_text() {
            obj.insert("text".to_string(), json!(text));
        }

        // Add content-desc if not blank
        if let Some(desc) = node.get_content_desc() {
            obj.insert("content-desc".to_string(), json!(desc));
        }

        // Add interactions
        if !node.interactions.is_empty() {
            let interactions: Vec<&str> = node.interactions.iter()
                .map(|s| s.as_str())
                .collect();
            obj.insert("interactions".to_string(), json!(interactions));
        }

        // Add state
        if !node.state.is_empty() {
            let state: Vec<&str> = node.state.iter()
                .map(|s| s.as_str())
                .collect();
            obj.insert("state".to_string(), json!(state));
        }

        // Add center if bounds available
        if let Some((cx, cy)) = node.get_center() {
            obj.insert("center".to_string(), json!({
                "x": cx,
                "y": cy
            }));
        }

        // Add bounds for scrollable elements
        if node.is_scrollable() {
            if let Some((min_x, min_y, max_x, max_y)) = node.get_bounds() {
                obj.insert("bounds".to_string(), json!({
                    "min_x": min_x,
                    "min_y": min_y,
                    "max_x": max_x,
                    "max_y": max_y
                }));
            }
        }

        // Add simplified resource-id
        if let Some(resource_id) = node.get_resource_id() {
            let simplified = simplify_resource_id(resource_id);
            obj.insert("resource-id".to_string(), json!(simplified));
        }

        // Add key hash
        obj.insert("key".to_string(), json!(node.key.hash_code()));

        // Add class if available
        if let Some(class) = node.get_class() {
            obj.insert("class".to_string(), json!(class));
        }

        // Add enabled/clickable flags
        if node.is_clickable() {
            obj.insert("clickable".to_string(), json!(true));
        }

        if !node.is_enabled() {
            obj.insert("enabled".to_string(), json!(false));
        }

        Value::Object(obj)
    }

    /// Serialize node tree to JSON array
    pub fn serialize_tree(root: &UiNode) -> Value {
        let mut elements = Vec::new();
        serialize_recursive(root, &mut elements);
        json!(elements)
    }

    /// Serialize flattened map to JSON array
    pub fn serialize_flat_map(map: &HashMap<Key, UiNode>) -> Value {
        let elements: Vec<Value> = map.iter()
            .map(|(_, node)| Self::to_json_object(node))
            .collect();
        json!(elements)
    }
}

/// Recursively serialize nodes
fn serialize_recursive(node: &UiNode, elements: &mut Vec<Value>) {
    elements.push(ElementSerializer::to_json_object(node));
    for child in &node.children {
        serialize_recursive(child, elements);
    }
}

/// Simplify resource-id (strip Android package prefix)
fn simplify_resource_id(resource_id: &str) -> String {
    // Look for :id/ pattern
    if let Some(pos) = resource_id.rfind(":id/") {
        return resource_id[pos + 4..].to_string();
    }
    resource_id.to_string()
}

/// Element diff serializer for comparing UI trees
///
/// Compares two UI trees and produces a JSON object showing:
/// - added: elements that appear in current but not in previous
/// - modified: elements that exist in both but have changed attributes
pub struct ElementDiffSerializer {
    /// Flattened previous tree
    old_map: HashMap<Key, UiNode>,
}

impl ElementDiffSerializer {
    /// Create a diff serializer from previous tree
    pub fn new(old_tree: &UiNode) -> Self {
        Self {
            old_map: UiNode::flatten(old_tree),
        }
    }

    /// Compare current tree with previous and produce diff JSON
    pub fn serialize(&self, current: &UiNode) -> Value {
        let current_map = UiNode::flatten(current);

        let mut added = Vec::new();
        let mut modified = Vec::new();

        // Find added and modified
        for (key, node) in &current_map {
            if let Some(old_node) = self.old_map.get(key) {
                // Check if modified
                if !old_node.has_same_attributes(node) {
                    modified.push(ElementSerializer::to_json_object(node));
                }
            } else {
                // Added
                added.push(ElementSerializer::to_json_object(node));
            }
        }

        // Find removed
        let removed: Vec<Value> = self.old_map.iter()
            .filter(|(key, _)| !current_map.contains_key(key))
            .map(|(_, node)| ElementSerializer::to_json_object(node))
            .collect();

        json!({
            "added": added,
            "modified": modified,
            "removed": removed
        })
    }

    /// Get summary counts
    pub fn summary(&self, current: &UiNode) -> DiffSummary {
        let current_map = UiNode::flatten(current);

        let added = current_map.iter()
            .filter(|(key, _)| !self.old_map.contains_key(key))
            .count();

        let modified = current_map.iter()
            .filter(|(key, node)| {
                self.old_map.get(key)
                    .map(|old| !old.has_same_attributes(node))
                    .unwrap_or(false)
            })
            .count();

        let removed = self.old_map.iter()
            .filter(|(key, _)| !current_map.contains_key(key))
            .count();

        DiffSummary {
            added,
            modified,
            removed,
        }
    }
}

/// Diff summary counts
#[derive(Debug, Clone)]
pub struct DiffSummary {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
}

impl DiffSummary {
    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        self.added > 0 || self.modified > 0 || self.removed > 0
    }

    /// Format for display
    pub fn format(&self) -> String {
        format!(
            "Added: {}, Modified: {}, Removed: {}",
            self.added, self.modified, self.removed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Key;
    use crate::vision::Rect;
    use std::collections::HashSet;

    fn create_test_node(text: &str, resource_id: &str, clickable: bool) -> UiNode {
        let mut interactions = HashSet::new();
        if clickable {
            interactions.insert("clickable".to_string());
        }

        UiNode {
            clazz: "android.widget.Button".to_string(),
            text: text.to_string(),
            resource_id: resource_id.to_string(),
            content_desc: String::new(),
            index: 0,
            interactions,
            state: HashSet::new(),
            bounds: Rect::new(0, 0, 100, 100),
            key: Key::new("root".to_string()), // Set a valid key for tests
            children: Vec::new(),
        }
    }

    #[test]
    fn test_simplify_resource_id() {
        assert_eq!(simplify_resource_id("com.app:id/button"), "button");
        assert_eq!(simplify_resource_id(":id/text"), "text");
        assert_eq!(simplify_resource_id("plain_id"), "plain_id");
    }

    #[test]
    fn test_element_serializer_to_json() {
        let node = create_test_node("Click Me", "com.app:id/btn", true);
        let json = ElementSerializer::to_json_object(&node);

        assert!(json.is_object());
        let obj = json.as_object().unwrap();
        assert_eq!(obj.get("text").unwrap(), "Click Me");
        assert_eq!(obj.get("resource-id").unwrap(), "btn");
        assert!(obj.contains_key("center"));
        assert!(obj.contains_key("clickable"));
    }

    #[test]
    fn test_element_serializer_empty_text() {
        let node = create_test_node("", "com.app:id/btn", false);
        let json = ElementSerializer::to_json_object(&node);

        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("text")); // Empty text should not be serialized
    }

    #[test]
    fn test_element_serializer_scrollable_bounds() {
        let mut interactions = HashSet::new();
        interactions.insert("scrollable".to_string());

        let node = UiNode {
            clazz: String::new(),
            text: String::new(),
            resource_id: String::new(),
            content_desc: String::new(),
            index: 0,
            interactions,
            state: HashSet::new(),
            bounds: Rect::new(0, 0, 100, 200),
            key: Key::empty(),
            children: Vec::new(),
        };

        let json = ElementSerializer::to_json_object(&node);
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("bounds"));
    }

    #[test]
    fn test_element_serializer_tree() {
        let child = create_test_node("Child", "com.app:id/child", false);
        let parent = UiNode {
            clazz: "android.widget.LinearLayout".to_string(),
            text: "Parent".to_string(),
            resource_id: String::new(),
            content_desc: String::new(),
            index: 0,
            interactions: HashSet::new(),
            state: HashSet::new(),
            bounds: Rect::new(0, 0, 100, 100),
            key: Key::empty(),
            children: vec![child],
        };

        let json = ElementSerializer::serialize_tree(&parent);
        assert!(json.is_array());
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 2); // Parent + child
    }

    #[test]
    fn test_diff_serializer_new() {
        let old = create_test_node("Old", "com.app:id/text", false);
        let diff = ElementDiffSerializer::new(&old);

        assert!(!diff.old_map.is_empty());
    }

    #[test]
    fn test_diff_serializer_no_changes() {
        let node = create_test_node("Same", "com.app:id/text", false);
        let diff = ElementDiffSerializer::new(&node);

        let result = diff.serialize(&node);
        let obj = result.as_object().unwrap();

        let added = obj.get("added").unwrap().as_array().unwrap();
        let modified = obj.get("modified").unwrap().as_array().unwrap();
        let removed = obj.get("removed").unwrap().as_array().unwrap();

        assert!(added.is_empty());
        assert!(modified.is_empty());
        assert!(removed.is_empty());
    }

    #[test]
    fn test_diff_serializer_summary() {
        let old = create_test_node("Old", "com.app:id/text", false);
        let new = create_test_node("New", "com.app:id/text", false);

        let diff = ElementDiffSerializer::new(&old);
        let summary = diff.summary(&new);

        assert!(summary.has_changes());
        assert_eq!(summary.modified, 1);
    }

    #[test]
    fn test_diff_summary_format() {
        let summary = DiffSummary {
            added: 2,
            modified: 1,
            removed: 3,
        };

        let formatted = summary.format();
        assert!(formatted.contains("Added: 2"));
        assert!(formatted.contains("Modified: 1"));
        assert!(formatted.contains("Removed: 3"));
    }

    #[test]
    fn test_element_serializer_interactions() {
        let mut interactions = HashSet::new();
        interactions.insert("clickable".to_string());
        interactions.insert("long-clickable".to_string());

        let node = UiNode {
            clazz: String::new(),
            text: String::new(),
            resource_id: String::new(),
            content_desc: String::new(),
            index: 0,
            interactions,
            state: HashSet::new(),
            bounds: Rect::new(0, 0, 100, 100),
            key: Key::empty(),
            children: Vec::new(),
        };

        let json = ElementSerializer::to_json_object(&node);
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("interactions"));

        let interactions = obj.get("interactions").unwrap().as_array().unwrap();
        assert_eq!(interactions.len(), 2);
    }

    #[test]
    fn test_element_serializer_state() {
        let mut state = HashSet::new();
        state.insert("checked".to_string());
        state.insert("focused".to_string());

        let node = UiNode {
            clazz: String::new(),
            text: String::new(),
            resource_id: String::new(),
            content_desc: String::new(),
            index: 0,
            interactions: HashSet::new(),
            state,
            bounds: Rect::new(0, 0, 100, 100),
            key: Key::empty(),
            children: Vec::new(),
        };

        let json = ElementSerializer::to_json_object(&node);
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("state"));

        let state = obj.get("state").unwrap().as_array().unwrap();
        assert_eq!(state.len(), 2);
    }
}