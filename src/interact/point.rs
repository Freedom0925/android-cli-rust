/// A 2D point
///
/// Based on original Point.kt from Kotlin implementation
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    /// Create a new point
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Create a point from floating point coordinates (rounded)
    pub fn from_f64(x: f64, y: f64) -> Self {
        Self {
            x: x.round() as i32,
            y: y.round() as i32,
        }
    }

    /// Get x coordinate (Kotlin: getX())
    pub fn get_x(&self) -> i32 {
        self.x
    }

    /// Get y coordinate (Kotlin: getY())
    pub fn get_y(&self) -> i32 {
        self.y
    }

    /// Get distance to another point
    pub fn distance_to(&self, other: &Point) -> f64 {
        let dx = (self.x - other.x) as f64;
        let dy = (self.y - other.y) as f64;
        (dx * dx + dy * dy).sqrt()
    }

    /// Get squared distance (faster, no sqrt)
    pub fn distance_squared_to(&self, other: &Point) -> i64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy) as i64
    }

    /// Translate point by given offset
    pub fn translate(&self, dx: i32, dy: i32) -> Point {
        Point::new(self.x + dx, self.y + dy)
    }

    /// Get midpoint between two points
    pub fn midpoint(&self, other: &Point) -> Point {
        Point::new((self.x + other.x) / 2, (self.y + other.y) / 2)
    }

    /// Check if point is inside a rectangle
    pub fn inside_rect(&self, rect: &crate::interact::Rect) -> bool {
        rect.contains_point(self.x, self.y)
    }
}

/// Display trait for toString matching Kotlin format "[x,y]"
impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", self.x, self.y)
    }
}

impl Default for Point {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation() {
        let p = Point::new(10, 20);
        assert_eq!(p.x, 10);
        assert_eq!(p.y, 20);
    }

    #[test]
    fn test_point_getters() {
        let p = Point::new(100, 200);
        assert_eq!(p.get_x(), 100);
        assert_eq!(p.get_y(), 200);
    }

    #[test]
    fn test_point_display() {
        let p = Point::new(10, 20);
        assert_eq!(format!("{}", p), "[10,20]");
    }

    #[test]
    fn test_point_from_f64() {
        let p = Point::from_f64(10.5, 20.4);
        assert_eq!(p.x, 11); // rounded
        assert_eq!(p.y, 20);
    }

    #[test]
    fn test_point_distance() {
        let p1 = Point::new(0, 0);
        let p2 = Point::new(3, 4);

        assert_eq!(p1.distance_to(&p2), 5.0); // 3-4-5 triangle
    }

    #[test]
    fn test_point_distance_squared() {
        let p1 = Point::new(0, 0);
        let p2 = Point::new(3, 4);

        assert_eq!(p1.distance_squared_to(&p2), 25);
    }

    #[test]
    fn test_point_translate() {
        let p = Point::new(10, 20);
        let translated = p.translate(5, -5);

        assert_eq!(translated.x, 15);
        assert_eq!(translated.y, 15);
    }

    #[test]
    fn test_point_midpoint() {
        let p1 = Point::new(0, 0);
        let p2 = Point::new(100, 100);

        let mid = p1.midpoint(&p2);
        assert_eq!(mid.x, 50);
        assert_eq!(mid.y, 50);
    }
}
