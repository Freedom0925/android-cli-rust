//! Interact module - core interaction primitives
//!
//! Based on Kotlin interact package structure:
//! - Point: 2D coordinate point
//! - Rect: Rectangle bounds
//! - Region: Trait for bounds-based regions
//! - RegionGroup: Trait for hierarchical region grouping
//! - MutableRegionGroup: Concrete implementation

pub mod region_group;
pub mod rect;
pub mod point;

// Re-export from region_group (contains both trait and impl)
pub use region_group::{Region, RegionGroup, MutableRegionGroup, group_regions};
pub use rect::Rect;
pub use point::Point;