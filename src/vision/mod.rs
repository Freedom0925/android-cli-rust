//! Vision module - image processing and feature detection
//!
//! Re-exports core types from interact module for vision processing

pub mod image_utils;
pub mod digits;
pub mod cluster;
pub mod edges;

// Re-export Region types from interact module
pub use crate::interact::{Region, RegionGroup, MutableRegionGroup, group_regions};

// Re-export Point and Rect from interact module (canonical implementation)
pub use crate::interact::{Point, Rect};

pub use image_utils::ImageUtils;
pub use digits::Digits;
pub use cluster::{PixelCluster, ClusterDetector, find_connected_clusters};
pub use edges::SobelEdges;