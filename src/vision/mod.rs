//! Vision module - image processing and feature detection
//!
//! Re-exports core types from interact module for vision processing

pub mod cluster;
pub mod digits;
pub mod edges;
pub mod image_utils;

// Re-export Region types from interact module
pub use crate::interact::{group_regions, MutableRegionGroup, Region, RegionGroup};

// Re-export Point and Rect from interact module (canonical implementation)
pub use crate::interact::{Point, Rect};

pub use cluster::{find_connected_clusters, ClusterDetector, PixelCluster};
pub use digits::Digits;
pub use edges::SobelEdges;
pub use image_utils::ImageUtils;
