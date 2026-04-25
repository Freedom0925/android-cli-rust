//! Layout module - UI hierarchy dump and analysis
//!
//! Based on Kotlin UIElement.java and LayoutCommand.kt

use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;
use anyhow::{Result, Context, bail};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

pub mod key;
pub mod serializer;

pub use key::Key;
pub use serializer::{ElementSerializer, ElementDiffSerializer, DiffSummary};

// Import Region trait from interact module
use crate::interact::Region;
use crate::vision::Rect;

/// Maximum recursion depth for UI hierarchy parsing
const MAX_PARSE_DEPTH: i32 = 100;

/// Interaction attributes (matches Kotlin UIElement.interactionAttrs)
const INTERACTION_ATTRS: [&str; 6] = [
    "checkable", "clickable", "focusable", "scrollable", "long-clickable", "password",
];

/// State attributes (matches Kotlin UIElement.stateAttrs)
const STATE_ATTRS: [&str; 3] = ["checked", "focused", "selected"];

/// Validate device identifier
fn validate_device_id(device: &str) -> Result<()> {
    for c in device.chars() {
        if !c.is_alphanumeric() && c != '-' && c != '_' && c != '.' && c != ':' {
            bail!("Invalid device identifier '{}'", device);
        }
    }
    Ok(())
}

/// Layout command for UI hierarchy dump
pub struct LayoutCommand {
    sdk_path: PathBuf,
}

/// UI Element - matches Kotlin UIElement structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiNode {
    /// Class name
    #[serde(rename = "class")]
    pub clazz: String,
    /// Text content
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub text: String,
    /// Resource ID
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub resource_id: String,
    /// Content description
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub content_desc: String,
    /// Index among siblings
    pub index: i32,
    /// Interaction capabilities
    #[serde(skip_serializing_if = "HashSet::is_empty", default)]
    pub interactions: HashSet<String>,
    /// State flags
    #[serde(skip_serializing_if = "HashSet::is_empty", default)]
    pub state: HashSet<String>,
    /// Bounding rectangle
    pub bounds: Rect,
    /// Unique key
    #[serde(skip_serializing_if = "Key::is_empty", default)]
    pub key: Key,
    /// Children
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<UiNode>,
}

impl Default for UiNode {
    fn default() -> Self {
        Self {
            clazz: String::new(),
            text: String::new(),
            resource_id: String::new(),
            content_desc: String::new(),
            index: 0,
            interactions: HashSet::new(),
            state: HashSet::new(),
            bounds: Rect::empty(),
            key: Key::empty(),
            children: Vec::new(),
        }
    }
}

impl UiNode {
    /// hasSameAttributes - complete comparison (matches Kotlin)
    pub fn has_same_attributes(&self, other: &UiNode) -> bool {
        self.resource_id == other.resource_id
            && self.index == other.index
            && self.clazz == other.clazz
            && self.text == other.text
            && self.content_desc == other.content_desc
            && self.interactions == other.interactions
            && self.state == other.state
            && self.bounds == other.bounds
    }

    /// computeKey - matches Kotlin UIElement.computeKey()
    /// sibling_index: the position of this node in the siblings list
    pub fn compute_key(&self, parent_key: &Key, siblings: &[UiNode], sibling_index: usize) -> Key {
        if parent_key.is_empty() && siblings.is_empty() {
            return Key::new("root".to_string());
        }

        if !self.resource_id.is_empty() {
            let duplicates = siblings.iter()
                .filter(|s| s.resource_id == self.resource_id)
                .count();

            if duplicates > 1 {
                // Count how many nodes with same resource_id appear before this one
                let mut dup_index = 0;
                for (i, sibling) in siblings.iter().enumerate() {
                    if sibling.resource_id == self.resource_id {
                        if i == sibling_index { break; }
                        dup_index += 1;
                    }
                }
                Key::from_resource_id_with_index(parent_key, &self.resource_id, dup_index as usize)
            } else {
                Key::from_resource_id(parent_key, &self.resource_id)
            }
        } else {
            Key::from_index(parent_key, sibling_index)
        }
    }

    /// flatten - matches Kotlin UIElementKt.flatten
    pub fn flatten(root: &UiNode) -> HashMap<Key, UiNode> {
        let mut map = HashMap::new();
        let mut stack = VecDeque::new();
        stack.push_back(root.clone());

        while let Some(cur) = stack.pop_front() {
            if !cur.key.is_empty() {
                map.insert(cur.key.clone(), cur.clone());
            }
            for child in &cur.children {
                stack.push_back(child.clone());
            }
        }
        map
    }

    // Getter methods (for serializer compatibility)

    /// Get text if not empty
    pub fn get_text(&self) -> Option<&str> {
        if self.text.is_empty() { None } else { Some(&self.text) }
    }

    /// Get content description if not empty
    pub fn get_content_desc(&self) -> Option<&str> {
        if self.content_desc.is_empty() { None } else { Some(&self.content_desc) }
    }

    /// Get resource ID if not empty
    pub fn get_resource_id(&self) -> Option<&str> {
        if self.resource_id.is_empty() { None } else { Some(&self.resource_id) }
    }

    /// Get class name if not empty
    pub fn get_class(&self) -> Option<&str> {
        if self.clazz.is_empty() { None } else { Some(&self.clazz) }
    }

    /// Get center coordinates
    pub fn get_center(&self) -> Option<(i32, i32)> {
        let w = self.bounds.max_x - self.bounds.min_x;
        let h = self.bounds.max_y - self.bounds.min_y;
        if w > 0 && h > 0 {
            Some((self.bounds.min_x + w / 2, self.bounds.min_y + h / 2))
        } else {
            None
        }
    }

    /// Get bounds as tuple
    pub fn get_bounds(&self) -> Option<(i32, i32, i32, i32)> {
        if self.bounds.is_empty() {
            None
        } else {
            Some((self.bounds.min_x, self.bounds.min_y, self.bounds.max_x, self.bounds.max_y))
        }
    }

    /// Check if scrollable
    pub fn is_scrollable(&self) -> bool {
        self.interactions.contains("scrollable")
    }

    /// Check if clickable
    pub fn is_clickable(&self) -> bool {
        self.interactions.contains("clickable")
    }

    /// Check if enabled (based on state)
    pub fn is_enabled(&self) -> bool {
        !self.state.contains("disabled")
    }
}

impl Region for UiNode {
    fn bounds(&self) -> Rect { self.bounds }
}

/// Layout dump result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutDump {
    pub root: UiNode,
    pub device: Option<String>,
    pub timestamp: String,
    pub node_count: i32,
}

/// Layout diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutDiff {
    pub added: Vec<LayoutChange>,
    pub removed: Vec<LayoutChange>,
    pub modified: Vec<LayoutChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutChange {
    pub change_type: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<String>,
}

impl LayoutCommand {
    pub fn new(sdk_path: &PathBuf) -> Result<Self> {
        Ok(Self { sdk_path: sdk_path.clone() })
    }

    fn adb_path(&self) -> PathBuf {
        self.sdk_path.join("platform-tools").join("adb")
    }

    /// Dump UI hierarchy
    pub fn dump(&self, device: Option<&str>, output: Option<&str>, diff: bool, pretty: bool) -> Result<()> {
        if let Some(d) = device { validate_device_id(d)?; }

        let device_args: Vec<String> = device
            .map(|d| vec!["-s".to_string(), d.to_string()])
            .unwrap_or_default();

        // Get previous dump if diff mode
        let previous = if diff {
            self.read_previous_dump(device, &device_args)?
        } else {
            None
        };

        // Execute uiautomator dump
        let mut cmd = Command::new(self.adb_path());
        for arg in &device_args { cmd.arg(arg); }
        cmd.args(["shell", "uiautomator", "dump", "--compressed", "/sdcard/window_dump.xml"]);
        let result = cmd.output().context("Failed to execute uiautomator dump")?;

        if !result.status.success() {
            bail!("UI dump failed: {}", String::from_utf8_lossy(&result.stderr));
        }

        // Read dump file
        let mut cmd = Command::new(self.adb_path());
        for arg in &device_args { cmd.arg(arg); }
        cmd.args(["shell", "cat", "/sdcard/window_dump.xml"]);
        let xml_output = cmd.output().context("Failed to read dump file")?;

        let xml = String::from_utf8(xml_output.stdout).context("Invalid UTF-8 in dump")?;

        // Build tree using stack algorithm (matches Kotlin buildTree)
        let mut root = build_tree(&xml)?;

        // Compute all keys
        compute_all_keys(&mut root);

        // Serialize with diff if requested
        if diff {
            if let Some(prev) = previous {
                let diff_serializer = ElementDiffSerializer::new(&prev);
                let diff_json = diff_serializer.serialize(&root);

                let json = if pretty {
                    serde_json::to_string_pretty(&diff_json)?
                } else {
                    serde_json::to_string(&diff_json)?
                };

                // Output summary
                let summary = diff_serializer.summary(&root);
                println!("{}", summary.format());

                // Output diff JSON
                if let Some(path) = output {
                    fs::write(path, &json)?;
                    println!("Diff written to {}", path);
                } else {
                    println!("{}", json);
                }

                return Ok(());
            } else {
                println!("No previous dump found, showing current layout");
            }
        }

        // Serialize
        let json = if pretty {
            serde_json::to_string_pretty(&root)?
        } else {
            serde_json::to_string(&root)?
        };

        // Output
        if let Some(path) = output {
            fs::write(path, &json)?;
            println!("Layout written to {}", path);
        } else {
            println!("{}", json);
        }

        Ok(())
    }

    fn read_previous_dump(&self, device: Option<&str>, device_args: &[String]) -> Result<Option<UiNode>> {
        let mut cmd = Command::new(self.adb_path());
        for arg in device_args { cmd.arg(arg); }
        cmd.args(["shell", "cat", "/sdcard/window_dump.xml"]);

        // Return None if command fails
        let result = match cmd.output() {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };

        if result.status.success() {
            let xml = match String::from_utf8(result.stdout) {
                Ok(s) => s,
                Err(_) => return Ok(None),
            };
            if let Ok(mut root) = build_tree(&xml) {
                compute_all_keys(&mut root);
                return Ok(Some(root));
            }
        }
        Ok(None)
    }
}

/// Build tree using stack algorithm (matches Kotlin UIElement.buildTree)
fn build_tree(xml: &str) -> Result<UiNode> {
    let start = xml.find("<node")
        .ok_or_else(|| anyhow::anyhow!("No node found in XML"))?;

    // Parse first element
    let (first_element, first_end) = parse_single_element(xml, start)?;

    // Stack-based tree building with parent indices
    // Stack entry: (parent_index in elements list, xml_position)
    let mut stack: VecDeque<(usize, usize)> = VecDeque::new();
    let mut elements: Vec<UiNode> = vec![first_element.clone()];

    // Add children of first element (index 0) to stack
    let children = find_child_positions(xml, start, first_end)?;
    for child_pos in children {
        stack.push_back((0, child_pos)); // parent index = 0 (first element)
    }

    // Process stack
    while let Some((parent_index, xml_pos)) = stack.pop_front() {
        let (element, element_end) = parse_single_element(xml, xml_pos)?;

        // Add element to list and get its index
        let element_index = elements.len();
        elements.push(element.clone());

        // Add child to parent using the known parent index
        elements[parent_index].children.push(element.clone());

        // Add this element's children to stack
        let child_positions = find_child_positions(xml, xml_pos, element_end)?;
        for child_pos in child_positions {
            stack.push_back((element_index, child_pos));
        }
    }

    // Return root (first element)
    Ok(elements.into_iter().next().unwrap_or_default())
}

/// Parse single element from XML (matches Kotlin parseFromXml)
fn parse_single_element(xml: &str, start: usize) -> Result<(UiNode, usize)> {
    let tag_end = find_tag_end(xml, start)?;
    let tag_content = &xml[start..tag_end];

    // Parse attributes
    let attrs = parse_attributes(tag_content)?;

    // Extract interactions and state
    let interactions = extract_interactions(&attrs);
    let state = extract_state(&attrs);

    // Parse bounds
    let bounds = attrs.get("bounds")
        .and_then(|b| parse_bounds(b))
        .map(|(min_x, min_y, max_x, max_y)| Rect::new(min_x, min_y, max_x, max_y))
        .unwrap_or(Rect::empty());

    let node = UiNode {
        clazz: attrs.get("class").cloned().unwrap_or_default(),
        text: attrs.get("text").cloned().unwrap_or_default(),
        resource_id: attrs.get("resource-id").cloned().unwrap_or_default(),
        content_desc: attrs.get("content-desc").cloned().unwrap_or_default(),
        index: attrs.get("index").and_then(|v| v.parse().ok()).unwrap_or(0),
        interactions,
        state,
        bounds,
        key: Key::empty(),
        children: Vec::new(),
    };

    Ok((node, tag_end))
}

/// Find child node positions within a parent
fn find_child_positions(xml: &str, parent_start: usize, parent_end: usize) -> Result<Vec<usize>> {
    let mut positions = Vec::new();
    let mut pos = parent_end;
    let mut depth = 1;

    while pos < parent_end {
        if xml[pos..].starts_with("<node") {
            positions.push(pos);
            // Find this node's end
            let node_end = find_node_end(xml, pos)?;
            pos = node_end;
        } else if xml[pos..].starts_with("</node>") {
            depth -= 1;
            pos += 7;
            if depth == 0 { break; }
        } else {
            pos += 1;
        }
    }

    Ok(positions)
}

/// Compute all keys after tree is built (matches Kotlin buildTree final loop)
fn compute_all_keys(root: &mut UiNode) {
    // First compute root's key
    root.key = Key::new("root".to_string());

    // Then compute keys for all children recursively
    let siblings = root.children.clone();
    let root_key = root.key.clone();
    compute_keys_recursive(root, &root_key, &siblings);
}

fn compute_keys_recursive(parent: &mut UiNode, parent_key: &Key, siblings: &[UiNode]) {
    for (sibling_index, child) in parent.children.iter_mut().enumerate() {
        // Compute key for this child using position in siblings
        child.key = child.compute_key(parent_key, siblings, sibling_index);

        // Clone for recursive call
        let child_key = child.key.clone();
        let grandchildren = child.children.clone();

        // Recursively compute keys for this child's children
        compute_keys_recursive(child, &child_key, &grandchildren);
    }
}

/// Parse attributes from tag content
fn parse_attributes(tag: &str) -> Result<HashMap<String, String>> {
    let mut attrs = HashMap::new();
    let re = regex::Regex::new(r#"(\w+)="([^"]*)""#)?;
    for cap in re.captures_iter(tag) {
        attrs.insert(cap[1].to_string(), cap[2].to_string());
    }
    Ok(attrs)
}

/// Extract interactions from attributes
fn extract_interactions(attrs: &HashMap<String, String>) -> HashSet<String> {
    INTERACTION_ATTRS.iter()
        .filter(|&attr| attrs.get::<str>(attr).map(|v| v == "true").unwrap_or(false))
        .map(|s| s.to_string())
        .collect()
}

/// Extract state from attributes
fn extract_state(attrs: &HashMap<String, String>) -> HashSet<String> {
    STATE_ATTRS.iter()
        .filter(|&attr| attrs.get::<str>(attr).map(|v| v == "true").unwrap_or(false))
        .map(|s| s.to_string())
        .collect()
}

/// Parse bounds string
fn parse_bounds(bounds: &str) -> Option<(i32, i32, i32, i32)> {
    let re = regex::Regex::new(r"\[(-?\d+),(-?\d+)\]\[(-?\d+),(-?\d+)\]").ok()?;
    let caps = re.captures(bounds)?;
    Some((
        caps[1].parse().ok()?,
        caps[2].parse().ok()?,
        caps[3].parse().ok()?,
        caps[4].parse().ok()?,
    ))
}

/// Find end of opening tag
fn find_tag_end(xml: &str, start: usize) -> Result<usize> {
    let mut pos = start;
    while pos < xml.len() {
        if xml[pos..].starts_with("/>") { return Ok(pos + 2); }
        if xml[pos..].starts_with(">") { return Ok(pos + 1); }
        pos += 1;
    }
    bail!("Tag end not found")
}

/// Find end of complete node
fn find_node_end(xml: &str, start: usize) -> Result<usize> {
    let tag_end = find_tag_end(xml, start)?;
    if xml[start..tag_end].ends_with("/>") { return Ok(tag_end); }

    let mut pos = tag_end;
    let mut depth = 1;

    while pos < xml.len() {
        if xml[pos..].starts_with("<node") {
            let next_end = find_tag_end(xml, pos)?;
            if !xml[pos..next_end].ends_with("/>") { depth += 1; }
            pos = next_end;
        } else if xml[pos..].starts_with("</node>") {
            depth -= 1;
            pos += 7;
            if depth == 0 { return Ok(pos); }
        } else {
            pos += 1;
        }
    }
    bail!("Node end not found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bounds() {
        let bounds = parse_bounds("[0,0][100,200]").unwrap();
        assert_eq!(bounds, (0, 0, 100, 200));

        let bounds = parse_bounds("[10,20][50,60]").unwrap();
        assert_eq!(bounds, (10, 20, 50, 60));
    }

    #[test]
    fn test_extract_interactions() {
        let mut attrs = HashMap::new();
        attrs.insert("clickable".to_string(), "true".to_string());
        attrs.insert("focusable".to_string(), "false".to_string());

        let interactions = extract_interactions(&attrs);
        assert!(interactions.contains("clickable"));
        assert!(!interactions.contains("focusable"));
    }

    #[test]
    fn test_uinode_default() {
        let node = UiNode::default();
        assert!(node.clazz.is_empty());
        assert!(node.text.is_empty());
        assert!(node.resource_id.is_empty());
    }

    #[test]
    fn test_uinode_has_same_attributes() {
        let node1 = UiNode {
            clazz: "Button".to_string(),
            text: "Click".to_string(),
            resource_id: "btn".to_string(),
            content_desc: "".to_string(),
            index: 0,
            interactions: HashSet::new(),
            state: HashSet::new(),
            bounds: Rect::new(0, 0, 100, 50),
            key: Key::empty(),
            children: Vec::new(),
        };

        let node2 = node1.clone();
        assert!(node1.has_same_attributes(&node2));

        let node3 = UiNode { text: "Different".to_string(), ..node1.clone() };
        assert!(!node1.has_same_attributes(&node3));
    }
}