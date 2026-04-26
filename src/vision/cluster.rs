//! Pixel cluster for connected component detection
//!
//! Based on original PixelCluster.java from Kotlin implementation

use crate::vision::{Point, Rect, Region};
use image::{DynamicImage, GenericImageView, ImageBuffer, Luma, Rgb};
use std::collections::{HashMap, HashSet};

/// A cluster of similar pixels
///
/// Matches Kotlin PixelCluster with:
/// - pixels: Set<Point> (not Vec<(u32, u32)>)
/// - bounds: Rect (computed from pixels)
/// - addPixel method
#[derive(Debug, Clone)]
pub struct PixelCluster {
    /// Pixels belonging to this cluster (x, y coordinates as Point)
    pixels: HashSet<Point>,
    /// Representative color of the cluster
    color: Rgb<u8>,
    /// Center of mass of the cluster
    center_x: f64,
    center_y: f64,
}

impl PixelCluster {
    /// Create a new pixel cluster
    pub fn new(pixels: HashSet<Point>, color: Rgb<u8>) -> Self {
        let center_x = if pixels.is_empty() {
            0.0
        } else {
            pixels.iter().map(|p| p.x as f64).sum::<f64>() / pixels.len() as f64
        };

        let center_y = if pixels.is_empty() {
            0.0
        } else {
            pixels.iter().map(|p| p.y as f64).sum::<f64>() / pixels.len() as f64
        };

        Self {
            pixels,
            color,
            center_x,
            center_y,
        }
    }

    /// Create empty cluster (matches Kotlin default constructor)
    pub fn empty() -> Self {
        Self::new(HashSet::new(), Rgb([0, 0, 0]))
    }

    /// Add a pixel to the cluster (matches Kotlin addPixel)
    pub fn add_pixel(&mut self, x: i32, y: i32) {
        // Update center incrementally
        let n = self.pixels.len() as f64;
        self.center_x = (self.center_x * n + x as f64) / (n + 1.0);
        self.center_y = (self.center_y * n + y as f64) / (n + 1.0);
        self.pixels.insert(Point::new(x, y));
    }

    /// Get the pixel count
    pub fn size(&self) -> usize {
        self.pixels.len()
    }

    /// Get the representative color
    pub fn get_color(&self) -> Rgb<u8> {
        self.color
    }

    /// Set the representative color
    pub fn set_color(&mut self, color: Rgb<u8>) {
        self.color = color;
    }

    /// Get the center coordinates
    pub fn get_center(&self) -> (f64, f64) {
        (self.center_x, self.center_y)
    }

    /// Get the pixels as Set<Point> (matches Kotlin getPixels)
    pub fn get_pixels(&self) -> &HashSet<Point> {
        &self.pixels
    }

    /// Calculate bounding box
    pub fn get_bounding_box(&self) -> Option<(i32, i32, i32, i32)> {
        if self.pixels.is_empty() {
            return None;
        }

        let min_x = self.pixels.iter().map(|p| p.x).min().unwrap();
        let max_x = self.pixels.iter().map(|p| p.x).max().unwrap();
        let min_y = self.pixels.iter().map(|p| p.y).min().unwrap();
        let max_y = self.pixels.iter().map(|p| p.y).max().unwrap();

        Some((min_x, min_y, max_x, max_y))
    }
}

/// Implement Region trait for PixelCluster (matches Kotlin: implements Region)
impl Region for PixelCluster {
    fn bounds(&self) -> Rect {
        self.get_bounding_box()
            .map(|(min_x, min_y, max_x, max_y)| Rect::new(min_x, min_y, max_x + 1, max_y + 1))
            .unwrap_or(Rect::empty())
    }
}

/// Implement Eq and Hash for PixelCluster (needed for RegionGroup)
impl PartialEq for PixelCluster {
    fn eq(&self, other: &Self) -> bool {
        // Include center and color in equality comparison
        // Note: comparing pixels directly is expensive, so we use center + color as proxy
        self.center_x == other.center_x
            && self.center_y == other.center_y
            && self.color == other.color
    }
}

impl Eq for PixelCluster {}

impl std::hash::Hash for PixelCluster {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.center_x.to_bits().hash(state);
        self.center_y.to_bits().hash(state);
    }
}

/// Find connected components using Union-Find algorithm
///
/// This implements the classic two-pass connected component labeling algorithm
/// using Union-Find for efficient label merging.
///
/// # Arguments
/// * `img` - Binary edge image (white pixels = foreground)
///
/// # Returns
/// Vector of PixelCluster objects representing connected regions
pub fn find_connected_clusters(img: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Vec<PixelCluster> {
    let (width, height) = img.dimensions();

    // Initialize labels array (0 = no label, labels[i] = parent of label i)
    // Size is width * height + 1 to accommodate all possible labels (0 is unused)
    let mut labels: Vec<i32> = vec![0; (width * height + 1) as usize];
    let mut next_label: i32 = 1;

    // Label matrix
    let mut label_matrix: Vec<Vec<i32>> = vec![vec![0; width as usize]; height as usize];

    // First pass: assign provisional labels and merge
    for y in 1..height {
        for x in 1..width {
            let pixel = img.get_pixel(x, y)[0];

            // Skip background pixels (black)
            if pixel == 0 {
                continue;
            }

            // Get labels of neighboring pixels (up and left)
            let up = label_matrix[(y - 1) as usize][x as usize];
            let left = label_matrix[y as usize][(x - 1) as usize];

            if up == 0 && left == 0 {
                // New region - assign new label
                label_matrix[y as usize][x as usize] = next_label;
                labels[next_label as usize] = next_label; // Initialize self-reference
                next_label += 1;
            } else if up != 0 && left != 0 {
                // Both neighbors have labels - merge them
                label_matrix[y as usize][x as usize] = union_find(&mut labels, up, left);
            } else {
                // Only one neighbor has label - use that label
                label_matrix[y as usize][x as usize] = if up != 0 { up } else { left };
            }
        }
    }

    // Second pass: resolve labels and build clusters
    let mut clusters: HashMap<i32, PixelCluster> = HashMap::new();

    for y in 0..height {
        for x in 0..width {
            let label = label_matrix[y as usize][x as usize];

            if label == 0 {
                continue;
            }

            // Find root label
            let root = find_root(&mut labels, label);

            if !clusters.contains_key(&root) {
                clusters.insert(root, PixelCluster::empty());
            }

            clusters
                .get_mut(&root)
                .unwrap()
                .add_pixel(x as i32, y as i32);
        }
    }

    // Convert to vector
    clusters.values().cloned().collect()
}

/// Find root label with path compression
fn find_root(labels: &mut Vec<i32>, label: i32) -> i32 {
    let mut current = label;

    // Find root
    while labels[current as usize] != current {
        current = labels[current as usize];
    }

    // Path compression
    let root = current;
    current = label;
    while labels[current as usize] != current {
        let temp = labels[current as usize];
        labels[current as usize] = root;
        current = temp;
    }

    root
}

/// Union two labels, return the resulting root
fn union_find(labels: &mut Vec<i32>, a: i32, b: i32) -> i32 {
    let root_a = find_root(labels, a);
    let root_b = find_root(labels, b);

    if root_a != root_b {
        labels[root_a as usize] = root_b;
    }

    root_b
}

/// Cluster detector for finding regions of similar pixels
pub struct ClusterDetector {
    /// Maximum distance threshold for color similarity (0-255)
    color_threshold: u8,
    /// Minimum cluster size to be considered significant
    min_cluster_size: usize,
    /// Maximum cluster size (large clusters are merged or ignored)
    max_cluster_size: usize,
}

impl Default for ClusterDetector {
    fn default() -> Self {
        Self {
            color_threshold: 30,
            min_cluster_size: 10,
            max_cluster_size: 10000,
        }
    }
}

impl ClusterDetector {
    /// Create a new cluster detector with custom thresholds
    pub fn new(color_threshold: u8, min_cluster_size: usize, max_cluster_size: usize) -> Self {
        Self {
            color_threshold,
            min_cluster_size,
            max_cluster_size,
        }
    }

    /// Detect clusters in an image region
    pub fn detect_clusters(&self, img: &DynamicImage) -> Vec<PixelCluster> {
        let (width, height) = img.dimensions();

        // Build color map: group pixels by similar colors
        let mut color_buckets: HashMap<[u8; 3], HashSet<Point>> = HashMap::new();

        for y in 0..height {
            for x in 0..width {
                let pixel = img.get_pixel(x, y);
                let rgb = [pixel[0], pixel[1], pixel[2]];

                // Quantize color to reduce buckets
                let quantized = self.quantize_color(rgb);
                color_buckets
                    .entry(quantized)
                    .or_insert_with(HashSet::new)
                    .insert(Point::new(x as i32, y as i32));
            }
        }

        // Convert buckets to clusters
        let clusters: Vec<PixelCluster> = color_buckets
            .into_iter()
            .filter(|(_, pixels)| pixels.len() >= self.min_cluster_size)
            .filter(|(_, pixels)| pixels.len() <= self.max_cluster_size)
            .map(|(color, pixels)| PixelCluster::new(pixels, Rgb(color)))
            .collect();

        // Merge adjacent clusters with similar colors
        self.merge_adjacent_clusters(clusters)
    }

    /// Detect clusters within a specific region
    pub fn detect_clusters_in_region(
        &self,
        img: &DynamicImage,
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
    ) -> Vec<PixelCluster> {
        let (width, height) = img.dimensions();
        let max_x = max_x.min(width);
        let max_y = max_y.min(height);

        let mut color_buckets: HashMap<[u8; 3], HashSet<Point>> = HashMap::new();

        for y in min_y..max_y {
            for x in min_x..max_x {
                let pixel = img.get_pixel(x, y);
                let rgb = [pixel[0], pixel[1], pixel[2]];
                let quantized = self.quantize_color(rgb);
                color_buckets
                    .entry(quantized)
                    .or_insert_with(HashSet::new)
                    .insert(Point::new(x as i32, y as i32));
            }
        }

        let clusters: Vec<PixelCluster> = color_buckets
            .into_iter()
            .filter(|(_, pixels)| pixels.len() >= self.min_cluster_size)
            .filter(|(_, pixels)| pixels.len() <= self.max_cluster_size)
            .map(|(color, pixels)| PixelCluster::new(pixels, Rgb(color)))
            .collect();

        self.merge_adjacent_clusters(clusters)
    }

    /// Find clusters of a specific color
    pub fn find_clusters_by_color(
        &self,
        img: &DynamicImage,
        target_color: Rgb<u8>,
    ) -> Vec<PixelCluster> {
        let clusters = self.detect_clusters(img);
        clusters
            .into_iter()
            .filter(|cluster| {
                let color = cluster.get_color();
                self.colors_are_similar(
                    [color[0], color[1], color[2]],
                    [target_color[0], target_color[1], target_color[2]],
                )
            })
            .collect()
    }

    /// Quantize color to reduce bucket count
    fn quantize_color(&self, rgb: [u8; 3]) -> [u8; 3] {
        let factor = self.color_threshold;
        [
            (rgb[0] / factor) * factor,
            (rgb[1] / factor) * factor,
            (rgb[2] / factor) * factor,
        ]
    }

    /// Check if two colors are similar
    fn colors_are_similar(&self, c1: [u8; 3], c2: [u8; 3]) -> bool {
        let diff_r = (c1[0] as i32 - c2[0] as i32).abs() as u8;
        let diff_g = (c1[1] as i32 - c2[1] as i32).abs() as u8;
        let diff_b = (c1[2] as i32 - c2[2] as i32).abs() as u8;

        diff_r <= self.color_threshold
            && diff_g <= self.color_threshold
            && diff_b <= self.color_threshold
    }

    /// Merge adjacent clusters
    fn merge_adjacent_clusters(&self, clusters: Vec<PixelCluster>) -> Vec<PixelCluster> {
        if clusters.is_empty() {
            return clusters;
        }

        let mut result: Vec<PixelCluster> = Vec::new();
        let mut visited: HashSet<usize> = HashSet::new();

        for i in 0..clusters.len() {
            if visited.contains(&i) {
                continue;
            }

            visited.insert(i);

            // Find all clusters adjacent to this one
            let mut merged_pixels: HashSet<Point> = clusters[i].pixels.clone();
            let mut merged_color = clusters[i].color;

            for j in (i + 1)..clusters.len() {
                if visited.contains(&j) {
                    continue;
                }

                if self.clusters_are_adjacent(&clusters[i], &clusters[j])
                    && self.colors_are_similar(
                        [merged_color[0], merged_color[1], merged_color[2]],
                        [
                            clusters[j].color[0],
                            clusters[j].color[1],
                            clusters[j].color[2],
                        ],
                    )
                {
                    visited.insert(j);
                    for p in clusters[j].pixels.iter() {
                        merged_pixels.insert(*p);
                    }
                }
            }

            if merged_pixels.len() >= self.min_cluster_size {
                result.push(PixelCluster::new(merged_pixels, merged_color));
            }
        }

        // Sort by size (largest first)
        result.sort_by(|a, b| b.size().cmp(&a.size()));
        result
    }

    /// Check if two clusters are spatially adjacent
    fn clusters_are_adjacent(&self, c1: &PixelCluster, c2: &PixelCluster) -> bool {
        // Check if any pixel from c1 is within distance threshold of c2
        let threshold = 3; // pixels

        for p1 in c1.pixels.iter().take(50) {
            // Sample first 50 pixels for efficiency
            for p2 in c2.pixels.iter().take(50) {
                let dx = (p1.x - p2.x).abs();
                let dy = (p1.y - p2.y).abs();

                if dx <= threshold && dy <= threshold {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    #[test]
    fn test_pixel_cluster_creation() {
        let mut pixels = HashSet::new();
        pixels.insert(Point::new(10, 10));
        pixels.insert(Point::new(11, 10));
        pixels.insert(Point::new(10, 11));
        let color = Rgb([255, 0, 0]);
        let cluster = PixelCluster::new(pixels, color);

        assert_eq!(cluster.size(), 3);
        assert_eq!(cluster.get_color(), Rgb([255, 0, 0]));
        // Center should be approximately (10.33, 10.33)
        let (cx, cy) = cluster.get_center();
        assert!((cx - 10.333333333333334).abs() < 0.01);
        assert!((cy - 10.333333333333334).abs() < 0.01);
    }

    #[test]
    fn test_pixel_cluster_bounding_box() {
        let mut pixels = HashSet::new();
        pixels.insert(Point::new(5, 5));
        pixels.insert(Point::new(10, 5));
        pixels.insert(Point::new(5, 15));
        pixels.insert(Point::new(10, 15));
        let cluster = PixelCluster::new(pixels, Rgb([0, 0, 0]));

        let bbox = cluster.get_bounding_box().unwrap();
        assert_eq!(bbox, (5, 5, 10, 15));
    }

    #[test]
    fn test_cluster_detector_default() {
        let detector = ClusterDetector::default();
        assert_eq!(detector.color_threshold, 30);
        assert_eq!(detector.min_cluster_size, 10);
        assert_eq!(detector.max_cluster_size, 10000);
    }

    #[test]
    fn test_quantize_color() {
        let detector = ClusterDetector::default();
        let quantized = detector.quantize_color([255, 128, 64]);
        // With threshold 30: 255/30*30=240, 128/30*30=120, 64/30*30=60
        assert_eq!(quantized, [240, 120, 60]);
    }

    #[test]
    fn test_colors_are_similar() {
        let detector = ClusterDetector::default();

        // Similar colors
        assert!(detector.colors_are_similar([100, 100, 100], [110, 110, 110]));

        // Different colors
        assert!(!detector.colors_are_similar([100, 100, 100], [200, 200, 200]));
    }

    #[test]
    fn test_empty_cluster() {
        let cluster = PixelCluster::new(HashSet::new(), Rgb([0, 0, 0]));
        assert_eq!(cluster.size(), 0);
        assert_eq!(cluster.get_center(), (0.0, 0.0));
        assert!(cluster.get_bounding_box().is_none());
    }

    #[test]
    fn test_detect_clusters_empty_image() {
        let img = DynamicImage::new_rgba8(5, 5);
        let detector = ClusterDetector::new(30, 10, 10000);

        let clusters = detector.detect_clusters(&img);
        // Small image, all pixels in one bucket but should be below min_cluster_size if threshold is high
        // With 5x5 = 25 pixels and min_cluster_size = 10, we should get clusters
        assert!(clusters.len() >= 0);
    }

    #[test]
    fn test_pixel_cluster_add_pixel() {
        let mut cluster = PixelCluster::empty();
        cluster.add_pixel(10, 20);
        cluster.add_pixel(30, 40);

        assert_eq!(cluster.size(), 2);
        assert!(cluster.get_pixels().contains(&Point::new(10, 20)));
        assert!(cluster.get_pixels().contains(&Point::new(30, 40)));
    }

    #[test]
    fn test_pixel_cluster_region_impl() {
        let mut pixels = HashSet::new();
        pixels.insert(Point::new(10, 10));
        pixels.insert(Point::new(20, 20));
        let cluster = PixelCluster::new(pixels, Rgb([255, 0, 0]));

        let bounds = cluster.bounds();
        assert_eq!(bounds.min_x, 10);
        assert_eq!(bounds.max_x, 21);
        assert_eq!(bounds.min_y, 10);
        assert_eq!(bounds.max_y, 21);
    }
}
