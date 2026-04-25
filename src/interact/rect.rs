//! Rectangle bounds
//!
//! Based on original Rect.kt from Kotlin implementation

use super::point::Point;
use std::fmt;
use serde::{Serialize, Deserialize};

/// A rectangle defined by min/max coordinates
///
/// Matches Kotlin Rect class with:
/// - minX, minY, maxX, maxY coordinates
/// - ll (lower-left) and ur (upper-right) as Point
/// - width, height, center computed properties
/// - contains, l2Norm, merge methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl Rect {
    /// Empty rect used for merging (matches Kotlin EMPTY)
    pub fn empty() -> Self {
        Self {
            min_x: i32::MAX,
            min_y: i32::MAX,
            max_x: i32::MIN,
            max_y: i32::MIN,
        }
    }

    /// Check if rect is empty (invalid bounds)
    pub fn is_empty(&self) -> bool {
        self.min_x >= self.max_x || self.min_y >= self.max_y
    }

    /// Create a new rectangle
    pub fn new(min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Self {
        Self {
            min_x: std::cmp::min(min_x, max_x),
            min_y: std::cmp::min(min_y, max_y),
            max_x: std::cmp::max(min_x, max_x),
            max_y: std::cmp::max(min_y, max_y),
        }
    }

    /// Create a rectangle from origin and size
    pub fn from_origin_size(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self::new(x, y, x + width, y + height)
    }

    /// Get lower-left point (Kotlin: ll)
    pub fn ll(&self) -> Point {
        Point::new(self.min_x, self.min_y)
    }

    /// Get upper-right point (Kotlin: ur)
    pub fn ur(&self) -> Point {
        Point::new(self.max_x, self.max_y)
    }

    /// Get the width of the rectangle
    pub fn width(&self) -> i32 {
        self.max_x - self.min_x
    }

    /// Get the height of the rectangle
    pub fn height(&self) -> i32 {
        self.max_y - self.min_y
    }

    /// Get the center point
    pub fn center(&self) -> Point {
        Point::new(
            self.min_x + self.width() / 2,
            self.min_y + self.height() / 2,
        )
    }

    /// Get min_x (Kotlin: getMinX())
    pub fn get_min_x(&self) -> i32 {
        self.min_x
    }

    /// Get min_y (Kotlin: getMinY())
    pub fn get_min_y(&self) -> i32 {
        self.min_y
    }

    /// Get max_x (Kotlin: getMaxX())
    pub fn get_max_x(&self) -> i32 {
        self.max_x
    }

    /// Get max_y (Kotlin: getMaxY())
    pub fn get_max_y(&self) -> i32 {
        self.max_y
    }

    /// Check if a point is inside the rectangle
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.min_x && x < self.max_x &&
        y >= self.min_y && y < self.max_y
    }

    /// Check if this rect strictly contains another rect
    /// (other must be completely inside, not touching edges)
    /// Matches Kotlin contains(Rect other) method
    pub fn contains(&self, other: &Rect) -> bool {
        other.max_x < self.max_x
            && other.min_x > self.min_x
            && other.max_y < self.max_y
            && other.min_y > self.min_y
    }

    /// Check if another rectangle intersects with this one
    pub fn intersects(&self, other: &Rect) -> bool {
        self.min_x < other.max_x && self.max_x > other.min_x &&
        self.min_y < other.max_y && self.max_y > other.min_y
    }

    /// Get the intersection of two rectangles
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        if !self.intersects(other) {
            return None;
        }

        Some(Rect::new(
            std::cmp::max(self.min_x, other.min_x),
            std::cmp::max(self.min_y, other.min_y),
            std::cmp::min(self.max_x, other.max_x),
            std::cmp::min(self.max_y, other.max_y),
        ))
    }

    /// Merge two rects (union of bounds) - matches Kotlin merge method
    pub fn merge(&self, other: &Rect) -> Rect {
        Rect::new(
            self.min_x.min(other.min_x),
            self.min_y.min(other.min_y),
            self.max_x.max(other.max_x),
            self.max_y.max(other.max_y),
        )
    }

    /// Get the union of two rectangles (alias for merge)
    pub fn union(&self, other: &Rect) -> Rect {
        self.merge(other)
    }

    /// Calculate L2 norm squared (sum of squared coordinate differences)
    /// Matches Kotlin l2Norm method
    pub fn l2_norm(&self, other: &Rect) -> i32 {
        let va = [self.min_x, self.min_y, self.max_x, self.max_y];
        let vb = [other.min_x, other.min_y, other.max_x, other.max_y];

        va.iter()
            .zip(vb.iter())
            .map(|(a, b)| (a - b).pow(2))
            .sum()
    }

    /// Calculate neighbor distance (dx, dy between rects)
    /// Returns the gap between two non-overlapping rects
    pub fn neighbor_distance(&self, other: &Rect) -> (i32, i32) {
        let dx = std::cmp::max(
            0,
            std::cmp::max(self.min_x - other.max_x, other.min_x - self.max_x),
        );
        let dy = std::cmp::max(
            0,
            std::cmp::max(self.min_y - other.max_y, other.min_y - self.max_y),
        );
        (dx, dy)
    }

    /// Expand the rectangle by a given amount
    pub fn expand(&self, amount: i32) -> Rect {
        Rect::new(
            self.min_x - amount,
            self.min_y - amount,
            self.max_x + amount,
            self.max_y + amount,
        )
    }

    /// Shrink the rectangle by a given amount
    pub fn shrink(&self, amount: i32) -> Option<Rect> {
        let min_x = self.min_x + amount;
        let min_y = self.min_y + amount;
        let max_x = self.max_x - amount;
        let max_y = self.max_y - amount;

        if min_x >= max_x || min_y >= max_y {
            return None;
        }

        Some(Rect::new(min_x, min_y, max_x, max_y))
    }

    /// Get the area of the rectangle
    pub fn area(&self) -> i32 {
        self.width() * self.height()
    }

    /// Parse from bounds string "[min_x,min_y][max_x,max_y]"
    pub fn parse(s: &str) -> Option<Self> {
        let re = regex::Regex::new(r"\[(-?\d+),(-?\d+)\]\[(-?\d+),(-?\d+)\]").ok()?;
        let caps = re.captures(s)?;
        Some(Self {
            min_x: caps[1].parse().ok()?,
            min_y: caps[2].parse().ok()?,
            max_x: caps[3].parse().ok()?,
            max_y: caps[4].parse().ok()?,
        })
    }
}

/// Display trait for toString matching Kotlin format "[minX,minY][maxX,maxY]"
impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}][{},{}]", self.min_x, self.min_y, self.max_x, self.max_y)
    }
}

impl Default for Rect {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_creation() {
        let rect = Rect::new(10, 20, 50, 60);
        assert_eq!(rect.min_x, 10);
        assert_eq!(rect.min_y, 20);
        assert_eq!(rect.max_x, 50);
        assert_eq!(rect.max_y, 60);
    }

    #[test]
    fn test_rect_creation_swapped() {
        // Should auto-sort coordinates
        let rect = Rect::new(50, 60, 10, 20);
        assert_eq!(rect.min_x, 10);
        assert_eq!(rect.min_y, 20);
        assert_eq!(rect.max_x, 50);
        assert_eq!(rect.max_y, 60);
    }

    #[test]
    fn test_rect_empty() {
        let r = Rect::empty();
        assert_eq!(r.min_x, i32::MAX);
        assert_eq!(r.min_y, i32::MAX);
        assert_eq!(r.max_x, i32::MIN);
        assert_eq!(r.max_y, i32::MIN);
    }

    #[test]
    fn test_rect_dimensions() {
        let rect = Rect::new(10, 20, 50, 60);
        assert_eq!(rect.width(), 40);
        assert_eq!(rect.height(), 40);
        assert_eq!(rect.area(), 1600);
    }

    #[test]
    fn test_rect_ll_ur() {
        let r = Rect::new(10, 20, 100, 200);
        assert_eq!(r.ll(), Point::new(10, 20));
        assert_eq!(r.ur(), Point::new(100, 200));
    }

    #[test]
    fn test_rect_center() {
        let rect = Rect::new(0, 0, 100, 200);
        let c = rect.center();
        assert_eq!(c.x, 50);
        assert_eq!(c.y, 100);
    }

    #[test]
    fn test_rect_contains_point() {
        let rect = Rect::new(10, 10, 50, 50);

        assert!(rect.contains_point(20, 20));
        assert!(rect.contains_point(10, 10));
        assert!(!rect.contains_point(50, 50)); // max is exclusive
        assert!(!rect.contains_point(0, 0));
        assert!(!rect.contains_point(100, 100));
    }

    #[test]
    fn test_rect_contains_rect() {
        let outer = Rect::new(0, 0, 100, 100);
        let inner = Rect::new(10, 10, 90, 90);
        let touching = Rect::new(0, 0, 50, 50);

        assert!(outer.contains(&inner));
        assert!(!outer.contains(&touching)); // touching edges
        assert!(!inner.contains(&outer));
    }

    #[test]
    fn test_rect_merge() {
        let r1 = Rect::new(0, 0, 50, 50);
        let r2 = Rect::new(25, 25, 100, 100);
        let merged = r1.merge(&r2);

        assert_eq!(merged.min_x, 0);
        assert_eq!(merged.min_y, 0);
        assert_eq!(merged.max_x, 100);
        assert_eq!(merged.max_y, 100);
    }

    #[test]
    fn test_rect_merge_with_empty() {
        let r = Rect::new(10, 20, 50, 60);
        let empty = Rect::empty();
        let merged = r.merge(&empty);

        assert_eq!(merged.min_x, 10);
        assert_eq!(merged.min_y, 20);
        assert_eq!(merged.max_x, 50);
        assert_eq!(merged.max_y, 60);
    }

    #[test]
    fn test_rect_l2_norm() {
        let r1 = Rect::new(0, 0, 10, 10);
        let r2 = Rect::new(5, 5, 15, 15);

        // Differences: [0-5, 0-5, 10-15, 10-15] = [-5, -5, -5, -5]
        // Sum of squares: 25 + 25 + 25 + 25 = 100
        assert_eq!(r1.l2_norm(&r2), 100);
    }

    #[test]
    fn test_rect_neighbor_distance_overlap() {
        let r1 = Rect::new(0, 0, 50, 50);
        let r2 = Rect::new(25, 25, 75, 75);

        // Overlapping, distance should be 0
        let (dx, dy) = r1.neighbor_distance(&r2);
        assert_eq!(dx, 0);
        assert_eq!(dy, 0);
    }

    #[test]
    fn test_rect_neighbor_distance_separate() {
        let r1 = Rect::new(0, 0, 50, 50);
        let r2 = Rect::new(100, 100, 150, 150);

        // Separate by 50 pixels in both directions
        let (dx, dy) = r1.neighbor_distance(&r2);
        assert_eq!(dx, 50);
        assert_eq!(dy, 50);
    }

    #[test]
    fn test_rect_intersects() {
        let rect1 = Rect::new(0, 0, 100, 100);
        let rect2 = Rect::new(50, 50, 150, 150);
        let rect3 = Rect::new(200, 200, 300, 300);

        assert!(rect1.intersects(&rect2));
        assert!(!rect1.intersects(&rect3));
    }

    #[test]
    fn test_rect_intersection() {
        let rect1 = Rect::new(0, 0, 100, 100);
        let rect2 = Rect::new(50, 50, 150, 150);

        let intersection = rect1.intersection(&rect2).unwrap();
        assert_eq!(intersection.min_x, 50);
        assert_eq!(intersection.min_y, 50);
        assert_eq!(intersection.max_x, 100);
        assert_eq!(intersection.max_y, 100);
    }

    #[test]
    fn test_rect_display() {
        let r = Rect::new(10, 20, 100, 200);
        assert_eq!(format!("{}", r), "[10,20][100,200]");
    }

    #[test]
    fn test_rect_parse() {
        let r = Rect::parse("[0,0][100,200]").unwrap();
        assert_eq!(r.min_x, 0);
        assert_eq!(r.min_y, 0);
        assert_eq!(r.max_x, 100);
        assert_eq!(r.max_y, 200);
    }

    #[test]
    fn test_rect_default() {
        let r = Rect::default();
        assert_eq!(r, Rect::empty());
    }
}