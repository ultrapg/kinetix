use crate::state::Geometry;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropZone {
    EdgeTop,
    EdgeBottom,
    EdgeLeft,
    EdgeRight,
    CenterSwap { target_id: u64 },
    GapReparent { target_a: u64, target_b: u64 },
    None,
}

pub struct DragDetector {
    edge_threshold: i32,
}

impl DragDetector {
    pub fn new() -> Self {
        Self { edge_threshold: 30 }
    }

    pub fn detect_drop_zone(
        &self,
        pointer_x: i32,
        pointer_y: i32,
        screen_bounds: Geometry,
        tiled_geometries: &HashMap<u64, Geometry>,
        dragged_id: u64,
    ) -> DropZone {
        // 1. Check screen edge drops (<30px from edge)
        if pointer_y <= screen_bounds.y + self.edge_threshold {
            return DropZone::EdgeTop;
        }
        if pointer_y >= screen_bounds.y + screen_bounds.height as i32 - self.edge_threshold {
            return DropZone::EdgeBottom;
        }
        if pointer_x <= screen_bounds.x + self.edge_threshold {
            return DropZone::EdgeLeft;
        }
        if pointer_x >= screen_bounds.x + screen_bounds.width as i32 - self.edge_threshold {
            return DropZone::EdgeRight;
        }

        // 2. Check center swap drops (inside another window's rectangle)
        for (&win_id, &geom) in tiled_geometries.iter() {
            if win_id == dragged_id {
                continue;
            }
            if geom.contains_point(pointer_x, pointer_y) {
                return DropZone::CenterSwap { target_id: win_id };
            }
        }

        DropZone::None
    }
}
