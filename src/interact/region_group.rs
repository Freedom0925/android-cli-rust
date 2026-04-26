//! RegionGroup trait and MutableRegionGroup implementation
//!
//! Based on original RegionGroup.java and MutableRegionGroup.java from Kotlin implementation

use crate::vision::Rect;
use std::collections::{HashMap, HashSet, VecDeque};

/// Region trait - represents a region with bounds
pub trait Region {
    /// Get the bounding rectangle of this region
    fn bounds(&self) -> Rect;
}

/// RegionGroup trait - groups regions hierarchically
///
/// This is the interface matching the Kotlin RegionGroup interface.
/// It extends Region (has bounds) and groups multiple regions together.
pub trait RegionGroup<T: Region + Eq + std::hash::Hash + Clone>: Region {
    /// Get the regions in this group
    fn regions(&self) -> &HashSet<T>;

    /// Get parent group (if any)
    fn parent(&self) -> Option<&dyn RegionGroup<T>>;

    /// Get depth level in hierarchy
    fn depth(&self) -> i32;

    /// Get child groups
    fn children(&self) -> &HashSet<Box<dyn RegionGroup<T>>>;
}

/// MutableRegionGroup - concrete implementation for building hierarchy
///
/// Based on MutableRegionGroup.java from the Kotlin implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableRegionGroup<T: Region + Eq + std::hash::Hash + Clone> {
    regions: HashSet<T>,
    bounds: Rect,
    parent: Option<Box<MutableRegionGroup<T>>>,
    depth: i32,
    children: Vec<Box<MutableRegionGroup<T>>>,
}

impl<T: Region + Eq + std::hash::Hash + Clone> std::hash::Hash for MutableRegionGroup<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.bounds.hash(state);
        self.depth.hash(state);
        // Hash regions by iterating and hashing each
        for region in &self.regions {
            region.hash(state);
        }
    }
}

impl<T: Region + Eq + std::hash::Hash + Clone> MutableRegionGroup<T> {
    /// Create a new region group
    pub fn new(
        regions: HashSet<T>,
        bounds: Rect,
        parent: Option<Box<MutableRegionGroup<T>>>,
        depth: i32,
    ) -> Self {
        Self {
            regions,
            bounds,
            parent,
            depth,
            children: Vec::new(),
        }
    }

    /// Get regions
    pub fn get_regions(&self) -> &HashSet<T> {
        &self.regions
    }

    /// Get bounds
    pub fn get_bounds(&self) -> Rect {
        self.bounds
    }

    /// Get depth
    pub fn get_depth(&self) -> i32 {
        self.depth
    }

    /// Get parent reference
    pub fn get_parent(&self) -> Option<&MutableRegionGroup<T>> {
        self.parent.as_ref().map(|p| p.as_ref())
    }

    /// Get children
    pub fn get_children(&self) -> &Vec<Box<MutableRegionGroup<T>>> {
        &self.children
    }

    /// Add a child group
    pub fn add_child(&mut self, child: Box<MutableRegionGroup<T>>) {
        self.children.push(child);
    }
}

impl<T: Region + Eq + std::hash::Hash + Clone> Region for MutableRegionGroup<T> {
    fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Group regions hierarchically based on neighbor relationships and parent function
///
/// This is the core algorithm from RegionKt.groupRegions():
/// 1. Build parent-child relationships
/// 2. Find root nodes (nodes without parents)
/// 3. BFS traversal, merging adjacent regions at each level
/// 4. Build hierarchy tree
pub fn group_regions<T: Region + Eq + std::hash::Hash + Clone>(
    regions: &[T],
    neighbors: impl Fn(&T, &T) -> bool,
    parent: impl Fn(&T) -> Option<T>,
) -> Vec<MutableRegionGroup<T>> {
    // Build parent and children maps
    let mut parents: HashMap<T, T> = HashMap::new();
    let mut children: HashMap<T, Vec<T>> = HashMap::new();

    for region in regions {
        if let Some(p) = parent(region) {
            parents.insert(region.clone(), p.clone());
            children
                .entry(p)
                .or_insert_with(Vec::new)
                .push(region.clone());
        }
    }

    // Find roots (regions without parents)
    let roots: Vec<T> = regions
        .iter()
        .filter(|r| !parents.contains_key(r))
        .cloned()
        .collect();

    // BFS to build hierarchy
    let mut groups: Vec<MutableRegionGroup<T>> = Vec::new();
    let mut queue: VecDeque<(Option<Box<MutableRegionGroup<T>>>, Vec<T>)> = VecDeque::new();

    queue.push_back((None, roots));

    while let Some((parent_group, current_regions)) = queue.pop_front() {
        // Merge adjacent regions
        let merged_sets = merge_regions(&current_regions, &neighbors);

        for set in merged_sets {
            // Calculate bounds by merging all region bounds
            let bounds = set
                .iter()
                .map(|r| r.bounds())
                .fold(Rect::empty(), |acc, b| acc.merge(&b));

            // Determine depth
            let depth = parent_group.as_ref().map(|p| p.depth + 1).unwrap_or(0);

            // Create new group
            let new_group = Box::new(MutableRegionGroup::new(
                set.clone(),
                bounds,
                parent_group.clone(),
                depth,
            ));

            groups.push((*new_group).clone());

            // Add to parent's children if parent exists
            // Note: We can't modify parent_group here due to ownership

            // Get children of regions in this set
            let group_children: Vec<T> = set
                .iter()
                .flat_map(|r| children.get(r).cloned().unwrap_or_default())
                .collect();

            if !group_children.is_empty() {
                queue.push_back((Some(new_group), group_children));
            }
        }
    }

    groups
}

/// Merge regions that are neighbors (connected components)
fn merge_regions<T: Region + Eq + std::hash::Hash + Clone>(
    regions: &[T],
    neighbors: impl Fn(&T, &T) -> bool,
) -> Vec<HashSet<T>> {
    // Build adjacency graph
    let mut graph: HashMap<T, HashSet<T>> = HashMap::new();

    for (i, region) in regions.iter().enumerate() {
        for j in (i + 1)..regions.len() {
            let other = &regions[j];
            if neighbors(region, other) {
                graph
                    .entry(region.clone())
                    .or_insert_with(HashSet::new)
                    .insert(other.clone());
                graph
                    .entry(other.clone())
                    .or_insert_with(HashSet::new)
                    .insert(region.clone());
            }
        }
    }

    // Find connected components using BFS
    let mut groups: Vec<HashSet<T>> = Vec::new();
    let mut visited: HashSet<T> = HashSet::new();

    for region in regions {
        if !visited.contains(region) {
            let mut component: HashSet<T> = HashSet::new();
            let mut queue: VecDeque<T> = VecDeque::new();

            queue.push_back(region.clone());

            while let Some(current) = queue.pop_front() {
                if !visited.contains(&current) {
                    component.insert(current.clone());
                    visited.insert(current.clone());

                    if let Some(neighbors_set) = graph.get(&current) {
                        for neighbor in neighbors_set {
                            if !visited.contains(neighbor) {
                                queue.push_back(neighbor.clone());
                            }
                        }
                    }
                }
            }

            groups.push(component);
        }
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test region implementation
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestRegion {
        id: i32,
        bounds: Rect,
    }

    impl Region for TestRegion {
        fn bounds(&self) -> Rect {
            self.bounds
        }
    }

    #[test]
    fn test_rect_merge_in_group() {
        let r1 = TestRegion {
            id: 1,
            bounds: Rect::new(0, 0, 50, 50),
        };
        let r2 = TestRegion {
            id: 2,
            bounds: Rect::new(25, 25, 100, 100),
        };

        let set = HashSet::from([r1.clone(), r2.clone()]);
        let bounds = set
            .iter()
            .map(|r| r.bounds())
            .fold(Rect::empty(), |acc, b| acc.merge(&b));

        assert_eq!(bounds.min_x, 0);
        assert_eq!(bounds.min_y, 0);
        assert_eq!(bounds.max_x, 100);
        assert_eq!(bounds.max_y, 100);
    }

    #[test]
    fn test_merge_regions_no_neighbors() {
        let r1 = TestRegion {
            id: 1,
            bounds: Rect::new(0, 0, 10, 10),
        };
        let r2 = TestRegion {
            id: 2,
            bounds: Rect::new(100, 100, 110, 110),
        };

        let regions = vec![r1, r2];
        let merged = merge_regions(&regions, |a, b| {
            let (dx, dy) = a.bounds().neighbor_distance(&b.bounds());
            dx < 5 && dy < 5
        });

        // Should have 2 separate groups (not neighbors)
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_merge_regions_with_neighbors() {
        let r1 = TestRegion {
            id: 1,
            bounds: Rect::new(0, 0, 10, 10),
        };
        let r2 = TestRegion {
            id: 2,
            bounds: Rect::new(8, 8, 18, 18),
        };
        let r3 = TestRegion {
            id: 3,
            bounds: Rect::new(16, 16, 26, 26),
        };

        let regions = vec![r1.clone(), r2.clone(), r3.clone()];
        let merged = merge_regions(&regions, |a, b| {
            let (dx, dy) = a.bounds().neighbor_distance(&b.bounds());
            dx <= 2 && dy <= 2
        });

        // r1 and r2 are neighbors (gap of 0), r2 and r3 are neighbors (gap of 0)
        // Should merge into one group
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].len(), 3);
    }

    #[test]
    fn test_group_regions_simple() {
        let r1 = TestRegion {
            id: 1,
            bounds: Rect::new(0, 0, 100, 100),
        };
        let r2 = TestRegion {
            id: 2,
            bounds: Rect::new(10, 10, 50, 50),
        };

        // r2 is child of r1 (contained)
        let regions = vec![r1.clone(), r2.clone()];

        let groups = group_regions(
            &regions,
            |_a, _b| false, // No merging
            |r| {
                if r.id == 2 {
                    Some(r1.clone())
                } else {
                    None
                }
            },
        );

        // Should have 2 groups (root + child)
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_mutable_region_group_creation() {
        let r = TestRegion {
            id: 1,
            bounds: Rect::new(0, 0, 50, 50),
        };
        let set = HashSet::from([r]);

        let group = MutableRegionGroup::new(set.clone(), Rect::new(0, 0, 50, 50), None, 0);

        assert_eq!(group.get_depth(), 0);
        assert_eq!(group.get_regions().len(), 1);
        assert!(group.get_parent().is_none());
    }

    #[test]
    fn test_region_trait_bounds() {
        let r = TestRegion {
            id: 1,
            bounds: Rect::new(10, 20, 50, 60),
        };
        assert_eq!(r.bounds().min_x, 10);
        assert_eq!(r.bounds().max_y, 60);
    }
}
