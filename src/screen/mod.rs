use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;
use anyhow::{Result, Context, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::vision::{Rect, ImageUtils, PixelCluster, Region, MutableRegionGroup, group_regions, SobelEdges, find_connected_clusters};
use image::{DynamicImage, ImageBuffer, Rgba, Luma};
use std::io::Write;

/// PNG IEND chunk marker (end of PNG data)
/// Format: length (4 bytes) + type "IEND" (4 bytes) + CRC (4 bytes)
const PNG_IEND_MARKER: [u8; 12] = [0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130];

/// Default threshold for merging nearby clusters
const DEFAULT_CLUSTER_MERGE_THRESHOLD: i32 = 10;

/// Green color for annotation boxes
const GREEN_COLOR: Rgba<u8> = Rgba([0, 255, 0, 255]);

/// Feature info for annotated screenshots
/// Represents a labeled region in the screenshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureInfo {
    /// Label number (corresponds to #N placeholder)
    pub label: u32,
    /// Bounding rectangle
    pub bounds: Bounds,
}

impl FeatureInfo {
    /// Create a new feature info
    pub fn new(label: u32, bounds: Bounds) -> Self {
        Self { label, bounds }
    }

    /// Get center coordinates as string (comma-separated for ADB commands)
    pub fn center_string(&self) -> String {
        format!("{},{}", self.bounds.center_x(), self.bounds.center_y())
    }
}

/// Validate device identifier to prevent command injection
/// Device IDs should only contain alphanumeric characters, hyphens, and underscores
fn validate_device_id(device: &str) -> Result<()> {
    // Device IDs: emulator-5554, serial numbers, IP:port
    // Valid pattern: alphanumeric, hyphens, underscores, dots, colons (for IP:port)
    for c in device.chars() {
        if !c.is_alphanumeric() && c != '-' && c != '_' && c != '.' && c != ':' {
            bail!("Invalid device identifier '{}': contains forbidden character '{}'. \
                  Device IDs must only contain alphanumeric characters, hyphens, underscores, dots, or colons.",
                  device, c);
        }
    }
    // Additional check: no shell metacharacters
    let forbidden_patterns = [";", "|", "&", "$", "`", "(", ")", "<", ">", "\n", "\r"];
    for pattern in forbidden_patterns {
        if device.contains(pattern) {
            bail!("Invalid device identifier '{}': contains shell metacharacter '{}'", device, pattern);
        }
    }
    Ok(())
}

/// Screen command operations
pub struct ScreenCommand {
    sdk_path: PathBuf,
    /// Threshold for merging nearby clusters (default 10)
    cluster_merge_threshold: i32,
    /// Enable debug mode to output intermediate images
    debug: bool,
}

/// UI element for annotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiElement {
    pub index: i32,
    pub text: String,
    pub resource_id: String,
    pub class: String,
    pub package: String,
    pub bounds: Bounds,
    pub clickable: bool,
    pub enabled: bool,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Bounds {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn center_x(&self) -> i32 {
        (self.left + self.right) / 2
    }

    pub fn center_y(&self) -> i32 {
        (self.top + self.bottom) / 2
    }
}

/// Resolve command for coordinate substitution
pub struct ResolveCommand;

impl ScreenCommand {
    pub fn new(sdk_path: &PathBuf) -> Result<Self> {
        Ok(Self {
            sdk_path: sdk_path.clone(),
            cluster_merge_threshold: DEFAULT_CLUSTER_MERGE_THRESHOLD,
            debug: false,
        })
    }

    /// Create with custom parameters
    pub fn with_options(sdk_path: &PathBuf, cluster_merge_threshold: i32, debug: bool) -> Self {
        Self {
            sdk_path: sdk_path.clone(),
            cluster_merge_threshold,
            debug,
        }
    }

    /// Set cluster merge threshold
    pub fn set_cluster_merge_threshold(&mut self, threshold: i32) {
        self.cluster_merge_threshold = threshold;
    }

    /// Set debug mode
    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }

    /// Detect features with depth-based filtering
    ///
    /// Based on ScreenCommand.detectFeatures from Kotlin:
    /// Uses groupRegions to create hierarchical region groups,
    /// then filters by depth-based size thresholds:
    /// - depth 0: width > 5 && height > 5
    /// - depth 1: width > 10 && height > 10
    /// - depth 2: width > 20 && height > 2
    /// - default: width > 48 && height > 48
    pub fn detect_features(&self, clusters: &[PixelCluster]) -> Vec<FeatureInfo> {
        let threshold_sq = self.cluster_merge_threshold * self.cluster_merge_threshold;

        // Neighbors predicate: within threshold distance (matches Kotlin detectFeatures$neighbors)
        // Formula: dx² + dy² < threshold²
        let neighbors = |a: &PixelCluster, b: &PixelCluster| {
            let a_bounds = a.bounds();
            let b_bounds = b.bounds();
            let (dx, dy) = a_bounds.neighbor_distance(&b_bounds);
            dx * dx + dy * dy < threshold_sq
        };

        // Parent predicate: find smallest cluster that contains this one (matches Kotlin detectFeatures$parent)
        let parent = |cluster: &PixelCluster| {
            let cluster_bounds = cluster.bounds();
            clusters.iter()
                .filter(|c| {
                    let c_bounds = c.bounds();
                    c_bounds != cluster_bounds && c_bounds.contains(&cluster_bounds)
                })
                .min_by_key(|c| c.bounds().width() * c.bounds().height())
                .cloned()  // Return owned PixelCluster
        };

        // Group regions using hierarchical grouping algorithm (matches Kotlin RegionKt.groupRegions)
        let groups: Vec<MutableRegionGroup<PixelCluster>> = group_regions(clusters, neighbors, parent);

        // Filter by depth-based size thresholds (matches Kotlin when block)
        let features: Vec<FeatureInfo> = groups
            .into_iter()
            .enumerate()
            .filter_map(|(i, group)| {
                let bounds = group.get_bounds();
                let width = bounds.width();
                let height = bounds.height();
                let depth = group.get_depth();

                // Apply depth-based filtering
                let passes = match depth {
                    0 => width > 5 && height > 5,
                    1 => width > 10 && height > 10,
                    2 => width > 20 && height > 2,
                    _ => width > 48 && height > 48,
                };

                if passes {
                    Some(FeatureInfo::new(i as u32, Bounds {
                        left: bounds.min_x,
                        top: bounds.min_y,
                        right: bounds.max_x,
                        bottom: bounds.max_y,
                    }))
                } else {
                    None
                }
            })
            .collect();

        features
    }

    /// Draw labeled regions on image
    pub fn draw_labeled_regions(
        img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        features: &[FeatureInfo],
    ) {
        for (idx, feature) in features.iter().enumerate() {
            let rect = Rect::new(
                feature.bounds.left,
                feature.bounds.top,
                feature.bounds.right,
                feature.bounds.bottom,
            );

            // Draw rectangle border
            ImageUtils::safe_set_pixel(img, rect.min_x, rect.min_y, GREEN_COLOR);
            for x in rect.min_x..rect.max_x {
                ImageUtils::safe_set_pixel(img, x, rect.min_y, GREEN_COLOR);
                ImageUtils::safe_set_pixel(img, x, rect.max_y - 1, GREEN_COLOR);
            }
            for y in rect.min_y..rect.max_y {
                ImageUtils::safe_set_pixel(img, rect.min_x, y, GREEN_COLOR);
                ImageUtils::safe_set_pixel(img, rect.max_x - 1, y, GREEN_COLOR);
            }

            // Draw label number above the box
            let label_y = rect.min_y - 22;
            if label_y >= 0 {
                crate::vision::digits::Digits::draw_number_on_buffer(
                    img,
                    rect.min_x,
                    label_y,
                    idx as u32,
                    GREEN_COLOR,
                    4,
                ).ok();
            }
        }
    }

    /// Highlight clusters on image
    pub fn highlight_clusters(
        img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        clusters: &[PixelCluster],
    ) {
        for (i, cluster) in clusters.iter().enumerate() {
            let color = generate_color(i);
            for point in cluster.get_pixels() {
                ImageUtils::safe_set_pixel(img, point.x, point.y, color);
            }
        }
    }

    /// Write debug images
    pub fn write_debug_images(
        &self,
        output_path: &Path,
        screen: &DynamicImage,
        greyscale: &ImageBuffer<Luma<u8>, Vec<u8>>,
        edges: &ImageBuffer<Luma<u8>, Vec<u8>>,
        clusters: &[PixelCluster],
    ) -> Result<()> {
        let debug_dir = output_path.parent()
            .unwrap_or(Path::new("."))
            .join("debug");

        fs::create_dir_all(&debug_dir)?;

        // Save original screenshot
        let screenshot_path = debug_dir.join("screenshot.png");
        screen.save(&screenshot_path)?;

        // Save greyscale
        let greyscale_path = debug_dir.join("greyscale.png");
        greyscale.save(&greyscale_path)?;

        // Save sobel edges
        let edges_path = debug_dir.join("sobel-edges.png");
        edges.save(&edges_path)?;

        // Save highlighted clusters
        let mut clusters_img = screen.to_rgba8();
        Self::highlight_clusters(&mut clusters_img, clusters);
        let clusters_path = debug_dir.join("clusters.png");
        clusters_img.save(&clusters_path)?;

        println!("Debug images saved to: {}", debug_dir.display());
        Ok(())
    }

    /// Append JSON data to PNG file
    pub fn append_json_to_png(png_path: &Path, json: &str) -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(png_path)?;

        file.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Get ADB path
    fn adb_path(&self) -> PathBuf {
        self.sdk_path.join("platform-tools").join("adb")
    }

    /// Capture screenshot with vision-based feature detection
    /// Uses Sobel edge detection + clustering (matches Kotlin ScreenCommand.annotate)
    pub fn capture_with_features(
        &self,
        device: Option<&str>,
        output: Option<&str>,
    ) -> Result<()> {
        use crate::vision::{SobelEdges, find_connected_clusters};

        let adb = self.adb_path();

        // Validate device ID if provided
        if let Some(d) = device {
            validate_device_id(d)?;
        }

        // Build device selection args
        let device_args: Vec<String> = device
            .map(|d| vec!["-s".to_string(), d.to_string()])
            .unwrap_or_default();

        // Capture screenshot
        println!("Capturing screenshot...");

        let mut cmd = Command::new(&adb);
        for arg in &device_args {
            cmd.arg(arg);
        }
        cmd.args(["exec-out", "screencap", "-p"]);

        let output_data = cmd.output()
            .context("Failed to capture screenshot")?;

        if !output_data.status.success() {
            bail!("Screenshot capture failed: {}", String::from_utf8_lossy(&output_data.stderr));
        }

        // Load image
        let screen = image::load_from_memory(&output_data.stdout)
            .context("Failed to parse screenshot image")?;

        println!("Processing image for feature detection...");

        // Convert to grayscale (matches Kotlin ImageUtils.toGreyscale)
        let greyscale = ImageUtils::to_grayscale(&screen);

        // Sobel edge detection with Otsu threshold (threshold=-1)
        let edges = SobelEdges::sobel_edges_with_threshold(&screen, -1);

        // Find clusters (matches Kotlin ClustersKt.findClusters)
        let clusters = find_connected_clusters(&edges);
        println!("Found {} edge clusters", clusters.len());

        // Detect features with depth filtering
        let features = self.detect_features(&clusters);
        println!("Detected {} features", features.len());

        // Debug output if enabled
        if self.debug {
            let output_path = match output {
                Some(path) => PathBuf::from(path),
                None => PathBuf::from(format!("screenshot_{}.png", chrono_timestamp())),
            };
            self.write_debug_images(&output_path, &screen, &greyscale, &edges, &clusters)?;
        }

        // Draw labeled regions
        let mut annotated = screen.to_rgba8();
        Self::draw_labeled_regions(&mut annotated, &features);

        // Determine output filename
        let output_path = match output {
            Some(path) => PathBuf::from(path),
            None => PathBuf::from(format!("screenshot_features_{}.png", chrono_timestamp())),
        };

        // Save annotated image
        annotated.save(&output_path)
            .context("Failed to save annotated image")?;

        // Append feature JSON to PNG (matches Kotlin Files.write with APPEND)
        let feature_json = serde_json::to_string(&features)
            .context("Failed to serialize features")?;
        Self::append_json_to_png(&output_path, &feature_json)?;

        println!("Annotated screenshot saved to: {}", output_path.display());
        Ok(())
    }

    /// Capture screenshot from device
    pub fn capture(
        &mut self,
        device: Option<&str>,
        output: Option<&str>,
        annotate: bool,
        cluster_merge_threshold: i32,
        debug: bool,
    ) -> Result<()> {
        // Set parameters
        self.cluster_merge_threshold = cluster_merge_threshold;
        self.debug = debug;

        if annotate {
            // Use vision-based approach (matches Kotlin ScreenCommand)
            // Sobel edge detection + clustering for feature detection
            return self.capture_with_features(device, output);
        }

        // Simple screenshot without annotation
        let adb = self.adb_path();

        // Validate device ID if provided
        if let Some(d) = device {
            validate_device_id(d)?;
        }

        // Build device selection args
        let device_args: Vec<String> = device
            .map(|d| vec!["-s".to_string(), d.to_string()])
            .unwrap_or_default();

        // Capture screenshot
        println!("Capturing screenshot...");

        // Get screenshot as PNG
        let mut cmd = Command::new(&adb);
        for arg in &device_args {
            cmd.arg(arg);
        }
        cmd.args(["exec-out", "screencap", "-p"]);

        let output_data = cmd.output()
            .context("Failed to capture screenshot")?;

        if !output_data.status.success() {
            bail!("Screenshot capture failed: {}", String::from_utf8_lossy(&output_data.stderr));
        }

        // Determine output filename
        let output_path = match output {
            Some(path) => PathBuf::from(path),
            None => PathBuf::from(format!("screenshot_{}.png", chrono_timestamp())),
        };

        std::fs::write(&output_path, &output_data.stdout)
            .with_context(|| format!("Failed to write screenshot to {}", output_path.display()))?;

        println!("Screenshot saved to: {}", output_path.display());
        Ok(())
    }

    /// Dump UI hierarchy from device
    fn dump_ui_hierarchy(&self, device: Option<&str>) -> Result<Vec<UiElement>> {
        let adb = self.adb_path();
        let remote_path = "/sdcard/window_dump.xml";

        // Validate device ID if provided
        if let Some(d) = device {
            validate_device_id(d)?;
        }

        // Build device selection args
        let device_args: Vec<String> = device
            .map(|d| vec!["-s".to_string(), d.to_string()])
            .unwrap_or_default();

        // Dump UI hierarchy
        let mut cmd = Command::new(&adb);
        for arg in &device_args {
            cmd.arg(arg);
        }
        cmd.args(["shell", "uiautomator", "dump", remote_path]);

        let output = cmd.output()
            .context("Failed to dump UI hierarchy")?;

        if !output.status.success() {
            bail!("UI dump failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        // Pull the dump file
        let mut pull_cmd = Command::new(&adb);
        for arg in &device_args {
            pull_cmd.arg(arg);
        }
        pull_cmd.args(["shell", "cat", remote_path]);

        let xml_output = pull_cmd.output()
            .context("Failed to read UI dump")?;

        let xml_content = String::from_utf8_lossy(&xml_output.stdout);

        // Parse XML to extract elements
        let elements = parse_ui_dump(&xml_content)?;

        Ok(elements)
    }

    /// Annotate screenshot with UI element bounding boxes
    fn annotate_screenshot(&self, png_data: Vec<u8>, elements: Vec<UiElement>) -> Result<Vec<u8>> {
        // Load image from PNG data
        let img = image::load_from_memory(&png_data)
            .context("Failed to parse screenshot image")?;

        // Convert to RGBA for drawing
        let mut img_rgba = img.to_rgba8();

        // Create annotation metadata
        let annotations: Vec<AnnotationData> = elements.iter().enumerate().map(|(idx, el)| {
            AnnotationData {
                index: idx + 1,
                text: el.text.clone(),
                resource_id: el.resource_id.clone(),
                class: el.class.clone(),
                bounds: el.bounds.clone(),
                clickable: el.clickable,
            }
        }).collect();

        // Draw labeled regions on image
        let features: Vec<FeatureInfo> = annotations.iter()
            .map(|a| FeatureInfo::new(a.index as u32, a.bounds.clone()))
            .collect();

        Self::draw_labeled_regions(&mut img_rgba, &features);

        // Embed JSON into PNG for resolve functionality
        let json = serde_json::to_string(&annotations)
            .context("Failed to serialize annotations")?;

        // Convert to PNG bytes
        let mut cursor = std::io::Cursor::new(Vec::new());
        img_rgba.write_to(&mut cursor, image::ImageFormat::Png)
            .context("Failed to encode annotated image")?;
        let mut output = cursor.into_inner();

        // Append JSON after IEND chunk
        output.extend_from_slice(json.as_bytes());

        // Save annotation file separately for reference
        let annotation_path = PathBuf::from("screenshot_elements.json");
        let json_pretty = serde_json::to_string_pretty(&annotations)
            .context("Failed to serialize annotations")?;
        std::fs::write(&annotation_path, &json_pretty)
            .context("Failed to write annotation file")?;

        println!("UI elements saved to: {}", annotation_path.display());
        println!("Found {} interactive elements", annotations.len());

        Ok(output)
    }
}

impl ResolveCommand {
    /// Parse annotated image and substitute placeholders with coordinates
    pub fn resolve(screenshot: &str, string: &str) -> Result<String> {
        // Try PNG parsing first (for embedded JSON)
        let screenshot_path = PathBuf::from(screenshot);

        if screenshot_path.exists() {
            let png_data = fs::read(&screenshot_path)
                .with_context(|| format!("Failed to read screenshot: {}", screenshot))?;

            // Try to extract embedded JSON from PNG
            if let Ok(features) = extract_png_embedded_json(&png_data) {
                return Self::resolve_with_features(string, &features);
            }
        }

        // Fallback to annotation file
        let annotation_path = screenshot_path.with_extension("json");

        if !annotation_path.exists() {
            bail!(
                "Annotation file not found: {}. Run 'android screen capture --annotate' first.",
                annotation_path.display()
            );
        }

        let json_content = std::fs::read_to_string(&annotation_path)
            .with_context(|| format!("Failed to read {}", annotation_path.display()))?;

        let annotations: Vec<AnnotationData> = serde_json::from_str(&json_content)
            .context("Failed to parse annotation file")?;

        // Convert annotations to features
        let features: Vec<FeatureInfo> = annotations
            .iter()
            .map(|a| FeatureInfo::new(a.index as u32, a.bounds.clone()))
            .collect();

        Self::resolve_with_features(string, &features)
    }

    /// Resolve placeholders using feature info
    fn resolve_with_features(string: &str, features: &[FeatureInfo]) -> Result<String> {
        // Build feature map by label
        let feature_map: HashMap<u32, &FeatureInfo> = features
            .iter()
            .map(|f| (f.label, f))
            .collect();

        // Replace #N placeholders with coordinates
        let re = regex::Regex::new(r"#(\d+)").unwrap();
        let result = re.replace_all(string, |caps: &regex::Captures| {
            if let Some(idx_str) = caps.get(1) {
                if let Ok(label) = idx_str.as_str().parse::<u32>() {
                    if let Some(feature) = feature_map.get(&label) {
                        return feature.center_string();
                    }
                }
            }
            caps.get(0).unwrap().as_str().to_string()
        }).to_string();

        Ok(result)
    }

    /// Extract feature info from PNG file with embedded JSON
    pub fn extract_features_from_png(png_path: &Path) -> Result<Vec<FeatureInfo>> {
        let png_data = fs::read(png_path)
            .with_context(|| format!("Failed to read PNG: {}", png_path.display()))?;

        extract_png_embedded_json(&png_data)
    }
}

/// Extract embedded JSON data from PNG file
///
/// PNG files may have JSON data appended after the IEND chunk.
/// This function finds the IEND marker and parses any JSON after it.
fn extract_png_embedded_json(png_data: &[u8]) -> Result<Vec<FeatureInfo>> {
    // Find IEND marker
    let json_start = find_png_iend_end(png_data)?;

    // Extract JSON after IEND
    let json_bytes = &png_data[json_start..];
    let json_str = String::from_utf8_lossy(json_bytes);

    // Trim any trailing data after JSON
    let json_end = json_str.rfind(']').map(|p| p + 1)
        .or_else(|| json_str.rfind('}').map(|p| p + 1))
        .unwrap_or(json_str.len());

    let json_content = &json_str[..json_end];

    // Parse JSON
    let features: Vec<FeatureInfo> = serde_json::from_str(json_content)
        .context("Failed to parse embedded JSON from PNG")?;

    Ok(features)
}

/// Find the end of PNG IEND chunk
fn find_png_iend_end(data: &[u8]) -> Result<usize> {
    if data.len() < PNG_IEND_MARKER.len() {
        bail!("Data too small to contain PNG IEND marker");
    }

    // Scan for IEND marker
    let max_scan = data.len() - PNG_IEND_MARKER.len();
    for i in 0..max_scan {
        if data[i..i + PNG_IEND_MARKER.len()] == PNG_IEND_MARKER {
            return Ok(i + PNG_IEND_MARKER.len());
        }
    }

    bail!("No PNG IEND marker found - file may not be a valid PNG or has no embedded data")
}

/// Annotation data for JSON serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnnotationData {
    index: usize,
    text: String,
    resource_id: String,
    class: String,
    bounds: Bounds,
    clickable: bool,
}

/// Parse UI dump XML to extract elements
fn parse_ui_dump(xml: &str) -> Result<Vec<UiElement>> {
    use crate::layout::build_tree;

    // Use the layout module's tree parser for proper XML handling
    let root = build_tree(xml)?;

    // Flatten the tree and extract interactive elements
    let mut elements = Vec::new();
    let mut index = 0;
    collect_interactive_elements(&root, &mut elements, &mut index);

    Ok(elements)
}

/// Recursively collect interactive elements from UI tree
fn collect_interactive_elements(node: &crate::layout::UiNode, elements: &mut Vec<UiElement>, index: &mut i32) {
    // Include elements that are clickable, have text, or have resource_id
    if node.interactions.contains("clickable")
        || !node.text.is_empty()
        || !node.resource_id.is_empty() {

        elements.push(UiElement {
            index: *index,
            text: node.text.clone(),
            resource_id: node.resource_id.clone(),
            class: node.clazz.clone(),
            package: "".to_string(), // Package info not available in layout tree
            bounds: Bounds {
                left: node.bounds.min_x,
                top: node.bounds.min_y,
                right: node.bounds.max_x,
                bottom: node.bounds.max_y,
            },
            clickable: node.interactions.contains("clickable"),
            enabled: true,
            visible: true,
        });
        *index += 1;
    }

    // Recurse into children
    for child in &node.children {
        collect_interactive_elements(child, elements, index);
    }
}

/// Parse a single node element from XML line
fn parse_node_element(line: &str, index: i32) -> Result<UiElement> {
    let text = extract_attr(line, "text").unwrap_or_default();
    let resource_id = extract_attr(line, "resource-id").unwrap_or_default();
    let class = extract_attr(line, "class").unwrap_or_default();
    let package = extract_attr(line, "package").unwrap_or_default();
    let clickable = extract_attr(line, "clickable")
        .map(|v| v == "true")
        .unwrap_or(false);
    let enabled = extract_attr(line, "enabled")
        .map(|v| v == "true")
        .unwrap_or(true);
    let bounds_str = extract_attr(line, "bounds").unwrap_or_default();

    let bounds = parse_bounds(&bounds_str)?;

    Ok(UiElement {
        index,
        text,
        resource_id,
        class,
        package,
        bounds,
        clickable,
        enabled,
        visible: true,
    })
}

/// Extract attribute value from XML string
fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = line.find(&pattern) {
        let start = start + pattern.len();
        if let Some(end) = line[start..].find('"') {
            return Some(line[start..start + end].to_string());
        }
    }
    None
}

/// Parse bounds string like "[0,0][1080,2400]"
fn parse_bounds(s: &str) -> Result<Bounds> {
    // Format: [left,top][right,bottom] - handle negative coordinates
    let re = regex::Regex::new(r"\[(-?\d+),(-?\d+)\]\[(-?\d+),(-?\d+)\]")?;
    if let Some(caps) = re.captures(s) {
        Ok(Bounds {
            left: caps[1].parse().unwrap_or(0),
            top: caps[2].parse().unwrap_or(0),
            right: caps[3].parse().unwrap_or(0),
            bottom: caps[4].parse().unwrap_or(0),
        })
    } else {
        Ok(Bounds {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        })
    }
}

/// Generate timestamp for filenames
fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", timestamp)
}

/// Generate a distinct color for cluster highlighting
fn generate_color(index: usize) -> Rgba<u8> {
    // Use HSV-like color generation for distinct colors
    let hue = (index * 37) % 360; // 37 is prime, gives good distribution
    let saturation = 200;
    let value = 255;

    // Convert HSV to RGB (simplified)
    let h = hue as f64 / 60.0;
    let c = value as f64 * saturation as f64 / 255.0 / 255.0;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());

    let (r, g, b) = match hue / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    Rgba([
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
        255,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bounds() {
        let bounds = parse_bounds("[100,200][500,800]").unwrap();
        assert_eq!(bounds.left, 100);
        assert_eq!(bounds.top, 200);
        assert_eq!(bounds.right, 500);
        assert_eq!(bounds.bottom, 800);
    }

    #[test]
    fn test_bounds_center() {
        let bounds = Bounds {
            left: 0,
            top: 0,
            right: 100,
            bottom: 200,
        };
        assert_eq!(bounds.center_x(), 50);
        assert_eq!(bounds.center_y(), 100);
    }

    #[test]
    fn test_bounds_dimensions() {
        let bounds = Bounds {
            left: 10,
            top: 20,
            right: 110,
            bottom: 220,
        };
        assert_eq!(bounds.width(), 100);
        assert_eq!(bounds.height(), 200);
    }

    #[test]
    fn test_extract_attr() {
        let line = r#"<node text="Hello" resource-id="com.app:id/text" clickable="true"/>"#;
        assert_eq!(extract_attr(line, "text"), Some("Hello".to_string()));
        assert_eq!(extract_attr(line, "resource-id"), Some("com.app:id/text".to_string()));
        assert_eq!(extract_attr(line, "clickable"), Some("true".to_string()));
    }

    #[test]
    fn test_parse_node_element() {
        let line = r#"<node text="Click Me" resource-id="com.app:id/button" class="android.widget.Button" clickable="true" bounds="[100,200][300,280]"/>"#;
        let element = parse_node_element(line, 0).unwrap();
        assert_eq!(element.text, "Click Me");
        assert_eq!(element.resource_id, "com.app:id/button");
        assert_eq!(element.class, "android.widget.Button");
        assert!(element.clickable);
        assert_eq!(element.bounds.left, 100);
        assert_eq!(element.bounds.right, 300);
    }

    #[test]
    fn test_bounds_center_calculation() {
        // Test center calculation for various bounds
        let bounds = Bounds {
            left: 0,
            top: 0,
            right: 100,
            bottom: 200,
        };
        assert_eq!(bounds.center_x(), 50);
        assert_eq!(bounds.center_y(), 100);

        // Test with non-zero origin
        let bounds2 = Bounds {
            left: 100,
            top: 200,
            right: 300,
            bottom: 600,
        };
        assert_eq!(bounds2.center_x(), 200);
        assert_eq!(bounds2.center_y(), 400);

        // Test with odd dimensions
        let bounds3 = Bounds {
            left: 10,
            top: 15,
            right: 21,
            bottom: 27,
        };
        assert_eq!(bounds3.center_x(), 15); // (10 + 21) / 2 = 15
        assert_eq!(bounds3.center_y(), 21); // (15 + 27) / 2 = 21
    }

    #[test]
    fn test_ui_element_bounds() {
        let element = UiElement {
            index: 1,
            text: "Button".to_string(),
            resource_id: "com.app:id/btn".to_string(),
            class: "android.widget.Button".to_string(),
            package: "com.app".to_string(),
            bounds: Bounds {
                left: 50,
                top: 100,
                right: 250,
                bottom: 200,
            },
            clickable: true,
            enabled: true,
            visible: true,
        };

        assert_eq!(element.index, 1);
        assert_eq!(element.bounds.width(), 200);
        assert_eq!(element.bounds.height(), 100);
        assert_eq!(element.bounds.center_x(), 150);
        assert_eq!(element.bounds.center_y(), 150);
    }

    #[test]
    fn test_ui_element_serialization() {
        let element = UiElement {
            index: 0,
            text: "Hello".to_string(),
            resource_id: "com.test:id/text".to_string(),
            class: "android.widget.TextView".to_string(),
            package: "com.test".to_string(),
            bounds: Bounds {
                left: 0,
                top: 0,
                right: 100,
                bottom: 50,
            },
            clickable: false,
            enabled: true,
            visible: true,
        };

        // Test JSON serialization
        let json = serde_json::to_string(&element).unwrap();
        assert!(json.contains("\"text\":\"Hello\""));
        assert!(json.contains("\"class\":\"android.widget.TextView\""));

        // Test JSON deserialization
        let deserialized: UiElement = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text, element.text);
        assert_eq!(deserialized.bounds.left, element.bounds.left);
    }

    #[test]
    fn test_resolve_command_placeholder_substitution() {
        // Create a temporary annotation file
        let temp_dir = tempfile::tempdir().unwrap();
        let screenshot_path = temp_dir.path().join("screenshot.png");
        let annotation_path = temp_dir.path().join("screenshot.json");

        let annotations = vec![
            AnnotationData {
                index: 1,
                text: "Button 1".to_string(),
                resource_id: "com.app:id/btn1".to_string(),
                class: "android.widget.Button".to_string(),
                bounds: Bounds {
                    left: 0,
                    top: 0,
                    right: 100,
                    bottom: 100,
                },
                clickable: true,
            },
            AnnotationData {
                index: 2,
                text: "Button 2".to_string(),
                resource_id: "com.app:id/btn2".to_string(),
                class: "android.widget.Button".to_string(),
                bounds: Bounds {
                    left: 100,
                    top: 100,
                    right: 300,
                    bottom: 300,
                },
                clickable: true,
            },
        ];

        // Write annotation file
        let json = serde_json::to_string_pretty(&annotations).unwrap();
        std::fs::write(&annotation_path, &json).unwrap();

        // Create an empty screenshot file (the content doesn't matter for this test)
        std::fs::write(&screenshot_path, "").unwrap();

        // Test placeholder substitution
        let result = ResolveCommand::resolve(
            screenshot_path.to_str().unwrap(),
            "tap #1 && tap #2",
        );

        // This should work since we have the annotation file
        assert!(result.is_ok());
        let resolved = result.unwrap();
        // #1 should be replaced with "50,50" (center of [0,0][100,100])
        // #2 should be replaced with "200,200" (center of [100,100][300,300])
        assert!(resolved.contains("50,50"));
        assert!(resolved.contains("200,200"));
        assert!(!resolved.contains("#1"));
        assert!(!resolved.contains("#2"));
    }

    #[test]
    fn test_resolve_command_with_multiple_placeholders() {
        let temp_dir = tempfile::tempdir().unwrap();
        let screenshot_path = temp_dir.path().join("screen.png");
        let annotation_path = temp_dir.path().join("screen.json");

        let annotations = vec![
            AnnotationData {
                index: 1,
                text: "".to_string(),
                resource_id: "com.app:id/edit".to_string(),
                class: "android.widget.EditText".to_string(),
                bounds: Bounds {
                    left: 10,
                    top: 20,
                    right: 310,
                    bottom: 70,
                },
                clickable: true,
            },
            AnnotationData {
                index: 5,
                text: "Submit".to_string(),
                resource_id: "com.app:id/submit".to_string(),
                class: "android.widget.Button".to_string(),
                bounds: Bounds {
                    left: 0,
                    top: 200,
                    right: 200,
                    bottom: 280,
                },
                clickable: true,
            },
        ];

        std::fs::write(&annotation_path, serde_json::to_string_pretty(&annotations).unwrap()).unwrap();
        std::fs::write(&screenshot_path, "").unwrap();

        let result = ResolveCommand::resolve(
            screenshot_path.to_str().unwrap(),
            "input #1 'hello' && tap #5",
        );

        assert!(result.is_ok());
        let resolved = result.unwrap();
        // #1 center: (10+310)/2=160, (20+70)/2=45
        assert!(resolved.contains("160,45"));
        // #5 center: (0+200)/2=100, (200+280)/2=240
        assert!(resolved.contains("100,240"));
    }

    #[test]
    fn test_resolve_command_missing_annotation_file() {
        let result = ResolveCommand::resolve(
            "/nonexistent/path/screenshot.png",
            "tap #1",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_screen_command_parse_bounds_from_dump() {
        // Use actual uiautomator format (single line, no whitespace)
        let xml_dump = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?><hierarchy rotation="0"><node index="0" text="" resource-id="" class="FrameLayout" package="com.app" bounds="[0,0][1080,2400]" clickable="false"><node index="1" text="Hello" resource-id="com.app:id/title" class="TextView" package="com.app" bounds="[100,200][500,300]" clickable="false"><node index="2" text="Click Me" resource-id="com.app:id/button" class="android.widget.Button" package="com.app" bounds="[100,500][400,600]" clickable="true"/></node></node></hierarchy>"#;

        let elements = parse_ui_dump(xml_dump).unwrap();

        // Should have parsed the interactive/text elements
        assert!(!elements.is_empty());

        // Check that we have elements with correct bounds
        let button_element = elements.iter().find(|e| e.resource_id == "com.app:id/button");
        assert!(button_element.is_some());

        let button = button_element.unwrap();
        assert_eq!(button.text, "Click Me");
        assert_eq!(button.bounds.left, 100);
        assert_eq!(button.bounds.top, 500);
        assert_eq!(button.bounds.right, 400);
        assert_eq!(button.bounds.bottom, 600);
        assert!(button.clickable);
    }

    #[test]
    fn test_parse_bounds_edge_cases() {
        // Test with valid bounds
        let result = parse_bounds("[0,0][1000,2000]").unwrap();
        assert_eq!(result.left, 0);
        assert_eq!(result.top, 0);
        assert_eq!(result.right, 1000);
        assert_eq!(result.bottom, 2000);

        // Test with empty bounds (should return zeros)
        let result = parse_bounds("").unwrap();
        assert_eq!(result.left, 0);
        assert_eq!(result.top, 0);
        assert_eq!(result.right, 0);
        assert_eq!(result.bottom, 0);

        // Test with invalid format (should return zeros)
        let result = parse_bounds("invalid").unwrap();
        assert_eq!(result.left, 0);
        assert_eq!(result.right, 0);
    }

    #[test]
    fn test_coordinate_formatting() {
        // Test that coordinates are formatted correctly for ADB
        let bounds = Bounds {
            left: 123,
            top: 456,
            right: 789,
            bottom: 1011,
        };

        let center_x = bounds.center_x();
        let center_y = bounds.center_y();

        // Center should be (456, 733)
        assert_eq!(center_x, 456);
        assert_eq!(center_y, 733);

        // Format as coordinate string
        let coord_string = format!("{},{}", center_x, center_y);
        assert_eq!(coord_string, "456,733");
    }

    #[test]
    fn test_parse_ui_dump_with_special_characters() {
        let xml_dump = r#"<node text="Hello &amp; World" resource-id="com.app:id/text" class="TextView" bounds="[0,0][100,50]" clickable="false"/>"#;

        let elements = parse_ui_dump(xml_dump).unwrap();
        // Note: The simple parser doesn't handle XML entities, but shouldn't crash
        assert!(!elements.is_empty());
    }

    #[test]
    fn test_parse_ui_dump_filtering() {
        // Test that elements with text, resource-id, or clickable are included
        let xml_dump = r#"<node text="" resource-id="" class="FrameLayout" bounds="[0,0][100,100]" clickable="false">
            <node text="Title" resource-id="" class="TextView" bounds="[0,100][100,200]" clickable="false"/>
            <node text="" resource-id="com.app:id/btn" class="Button" bounds="[0,200][100,300]" clickable="true"/>
            <node text="" resource-id="" class="View" bounds="[0,300][100,400]" clickable="false"/>
        </node>"#;

        let elements = parse_ui_dump(xml_dump).unwrap();

        // Should include elements with text, resource-id, or clickable
        // The simple parser looks for clickable elements, non-empty text, or non-empty resource-id
        assert!(!elements.is_empty());
    }

    #[test]
    fn test_extract_attr_edge_cases() {
        // Test extraction from complex attributes
        let line = r#"<node text="Value with spaces" resource-id="com.app:id/test" empty="" clickable="true"/>"#;
        assert_eq!(extract_attr(line, "text"), Some("Value with spaces".to_string()));
        assert_eq!(extract_attr(line, "resource-id"), Some("com.app:id/test".to_string()));
        assert_eq!(extract_attr(line, "empty"), Some("".to_string()));
        assert_eq!(extract_attr(line, "clickable"), Some("true".to_string()));

        // Test missing attribute
        assert_eq!(extract_attr(line, "nonexistent"), None);
    }

    #[test]
    fn test_parse_node_element_with_all_attributes() {
        let line = r#"<node index="0" text="Test" resource-id="com.app:id/test" class="android.widget.Button" package="com.test.app" clickable="true" enabled="true" bounds="[10,20][110,120]"/>"#;
        let element = parse_node_element(line, 0).unwrap();

        assert_eq!(element.index, 0);
        assert_eq!(element.text, "Test");
        assert_eq!(element.resource_id, "com.app:id/test");
        assert_eq!(element.class, "android.widget.Button");
        assert_eq!(element.package, "com.test.app");
        assert!(element.clickable);
        assert!(element.enabled);
        assert!(element.visible);
        assert_eq!(element.bounds.left, 10);
        assert_eq!(element.bounds.top, 20);
        assert_eq!(element.bounds.right, 110);
        assert_eq!(element.bounds.bottom, 120);
    }

    #[test]
    fn test_parse_node_element_enabled_default() {
        // Test that enabled defaults to true when not specified
        let line = r#"<node text="" resource-id="" class="View" bounds="[0,0][100,100]"/>"#;
        let element = parse_node_element(line, 0).unwrap();
        assert!(element.enabled);

        // Test explicit enabled="false"
        let line_disabled = r#"<node text="" resource-id="" class="View" bounds="[0,0][100,100]" enabled="false"/>"#;
        let element_disabled = parse_node_element(line_disabled, 0).unwrap();
        assert!(!element_disabled.enabled);
    }

    #[test]
    fn test_annotation_data_serialization() {
        let annotation = AnnotationData {
            index: 1,
            text: "Submit Button".to_string(),
            resource_id: "com.app:id/submit".to_string(),
            class: "android.widget.Button".to_string(),
            bounds: Bounds {
                left: 50,
                top: 100,
                right: 250,
                bottom: 200,
            },
            clickable: true,
        };

        let json = serde_json::to_string(&annotation).unwrap();
        let deserialized: AnnotationData = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.index, 1);
        assert_eq!(deserialized.text, "Submit Button");
        assert_eq!(deserialized.bounds.center_x(), 150);
        assert_eq!(deserialized.bounds.center_y(), 150);
    }
}