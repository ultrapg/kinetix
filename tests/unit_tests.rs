use kinetix::config::{Config, LayoutType};
use kinetix::engine::drag_detect::{DragDetector, DropZone};
use kinetix::engine::layouts::{DwindleLayout, LayoutStrategy, MasterStackLayout};
use kinetix::engine::tree::{Direction, Node};
use kinetix::state::{Geometry, StateManager};
use std::collections::HashMap;

#[test]
fn test_geometry_center_and_contains() {
    let geom = Geometry::new(100, 100, 400, 300);
    assert_eq!(geom.center(), (300, 250));
    assert!(geom.contains_point(150, 150));
    assert!(!geom.contains_point(50, 50));
    assert!(!geom.contains_point(500, 400));
}

#[test]
fn test_state_manager_tileable_filters() {
    let sm = StateManager::new();
    assert!(!sm.is_tileable("plasmashell", None));
    assert!(!sm.is_tileable("kinetix", None));
    assert!(!sm.is_tileable("yakuake", None));
    assert!(!sm.is_tileable("some-app", Some("DIALOG")));
    assert!(sm.is_tileable("firefox", Some("NORMAL")));
}

#[test]
fn test_dwindle_layout_computation() {
    let layout = DwindleLayout;
    let screen = Geometry::new(0, 0, 1920, 1080);
    let windows = vec![1, 2, 3];

    let geoms = layout.compute_layout(&windows, screen, 10, 5);
    assert_eq!(geoms.len(), 3);
    assert!(geoms.contains_key(&1));
    assert!(geoms.contains_key(&2));
    assert!(geoms.contains_key(&3));
}

#[test]
fn test_master_stack_layout_computation() {
    let layout = MasterStackLayout { master_ratio: 0.6 };
    let screen = Geometry::new(0, 0, 1000, 1000);
    let windows = vec![1, 2, 3];

    let geoms = layout.compute_layout(&windows, screen, 0, 0);
    assert_eq!(geoms.len(), 3);
    assert_eq!(geoms[&1].width, 600);
    assert_eq!(geoms[&2].width, 400);
    assert_eq!(geoms[&3].width, 400);
}

#[test]
fn test_drag_detector_edge_and_center() {
    let detector = DragDetector::new();
    let screen = Geometry::new(0, 0, 1920, 1080);
    let mut tiled = HashMap::new();
    tiled.insert(100, Geometry::new(100, 100, 800, 600));

    // Top edge drop
    let drop_top = detector.detect_drop_zone(500, 10, screen, &tiled, 200);
    assert_eq!(drop_top, DropZone::EdgeTop);

    // Center swap drop
    let drop_center = detector.detect_drop_zone(400, 300, screen, &tiled, 200);
    assert_eq!(drop_center, DropZone::CenterSwap { target_id: 100 });
}

#[test]
fn test_bsp_tree_insert_remove_swap() {
    let mut root = Node::new_window(1, Geometry::new(0, 0, 100, 100));
    root.insert(2, Geometry::new(0, 0, 100, 100), None);

    let mut geoms = HashMap::new();
    root.compute_geometries(Geometry::new(0, 0, 1000, 1000), 0, &mut geoms);
    assert_eq!(geoms.len(), 2);

    root.swap(1, 2);
    let mut geoms_swapped = HashMap::new();
    root.compute_geometries(Geometry::new(0, 0, 1000, 1000), 0, &mut geoms_swapped);
    assert_eq!(geoms_swapped.len(), 2);
}
