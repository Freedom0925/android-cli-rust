//! Interact module - core interaction primitives
//!
//! Based on Kotlin interact package structure:
//! - Point: 2D coordinate point
//! - Rect: Rectangle bounds
//! - Region: Trait for bounds-based regions
//! - RegionGroup: Trait for hierarchical region grouping
//! - MutableRegionGroup: Concrete implementation

pub mod point;
pub mod rect;
pub mod region_group;

// Re-export from region_group (contains both trait and impl)
pub use point::Point;
pub use rect::Rect;
pub use region_group::{group_regions, MutableRegionGroup, Region, RegionGroup};
