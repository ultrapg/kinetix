use crate::state::Geometry;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum Node {
    Window {
        id: u64,
        original_geometry: Geometry,
    },
    Split {
        direction: Direction,
        ratio: f32,
        left: Box<Node>,
        right: Box<Node>,
    },
}

impl Node {
    pub fn new_window(id: u64, original_geometry: Geometry) -> Self {
        Node::Window {
            id,
            original_geometry,
        }
    }

    pub fn insert(&mut self, new_id: u64, original_geom: Geometry, target_id: Option<u64>) -> bool {
        match self {
            Node::Window { id, original_geometry } => {
                if target_id.is_none() || target_id == Some(*id) {
                    let old_node = Node::Window {
                        id: *id,
                        original_geometry: *original_geometry,
                    };
                    let new_node = Node::Window {
                        id: new_id,
                        original_geometry: original_geom,
                    };
                    *self = Node::Split {
                        direction: Direction::Horizontal,
                        ratio: 0.5,
                        left: Box::new(old_node),
                        right: Box::new(new_node),
                    };
                    return true;
                }
                false
            }
            Node::Split {
                direction,
                ratio: _,
                left,
                right,
            } => {
                if left.insert(new_id, original_geom, target_id) {
                    return true;
                }
                if right.insert(new_id, original_geom, target_id) {
                    return true;
                }
                false
            }
        }
    }

    pub fn remove(&mut self, target_id: u64) -> (bool, Option<Node>) {
        match self {
            Node::Window { id, .. } => {
                if *id == target_id {
                    (true, None)
                } else {
                    (false, None)
                }
            }
            Node::Split { left, right, .. } => {
                if let Node::Window { id, .. } = **left {
                    if id == target_id {
                        let replacement = (**right).clone();
                        return (true, Some(replacement));
                    }
                }
                if let Node::Window { id, .. } = **right {
                    if id == target_id {
                        let replacement = (**left).clone();
                        return (true, Some(replacement));
                    }
                }

                let (removed_left, replacement_left) = left.remove(target_id);
                if removed_left {
                    if let Some(repl) = replacement_left {
                        *left = Box::new(repl);
                    }
                    return (true, None);
                }

                let (removed_right, replacement_right) = right.remove(target_id);
                if removed_right {
                    if let Some(repl) = replacement_right {
                        *right = Box::new(repl);
                    }
                    return (true, None);
                }

                (false, None)
            }
        }
    }

    pub fn swap(&mut self, id_a: u64, id_b: u64) {
        let mut ids = Vec::new();
        self.collect_ids(&mut ids);
        if ids.contains(&id_a) && ids.contains(&id_b) {
            self.swap_ids(id_a, id_b);
        }
    }

    fn collect_ids(&self, out: &mut Vec<u64>) {
        match self {
            Node::Window { id, .. } => out.push(*id),
            Node::Split { left, right, .. } => {
                left.collect_ids(out);
                right.collect_ids(out);
            }
        }
    }

    fn swap_ids(&mut self, id_a: u64, id_b: u64) {
        match self {
            Node::Window { id, .. } => {
                if *id == id_a {
                    *id = id_b;
                } else if *id == id_b {
                    *id = id_a;
                }
            }
            Node::Split { left, right, .. } => {
                left.swap_ids(id_a, id_b);
                right.swap_ids(id_a, id_b);
            }
        }
    }

    pub fn compute_geometries(
        &self,
        bounds: Geometry,
        inner_gaps: u32,
        out: &mut HashMap<u64, Geometry>,
    ) {
        match self {
            Node::Window { id, .. } => {
                out.insert(*id, bounds);
            }
            Node::Split {
                direction,
                ratio,
                left,
                right,
            } => {
                let half_gap = (inner_gaps / 2) as i32;
                match direction {
                    Direction::Horizontal => {
                        let total_w = bounds.width as f32;
                        let left_w = ((total_w * ratio) as u32).saturating_sub(inner_gaps / 2);
                        let right_w = bounds.width.saturating_sub(left_w).saturating_sub(inner_gaps / 2);

                        let left_bounds = Geometry::new(
                            bounds.x,
                            bounds.y,
                            left_w.max(1),
                            bounds.height,
                        );
                        let right_bounds = Geometry::new(
                            bounds.x + left_w as i32 + half_gap,
                            bounds.y,
                            right_w.max(1),
                            bounds.height,
                        );

                        left.compute_geometries(left_bounds, inner_gaps, out);
                        right.compute_geometries(right_bounds, inner_gaps, out);
                    }
                    Direction::Vertical => {
                        let total_h = bounds.height as f32;
                        let left_h = ((total_h * ratio) as u32).saturating_sub(inner_gaps / 2);
                        let right_h = bounds.height.saturating_sub(left_h).saturating_sub(inner_gaps / 2);

                        let left_bounds = Geometry::new(
                            bounds.x,
                            bounds.y,
                            bounds.width,
                            left_h.max(1),
                        );
                        let right_bounds = Geometry::new(
                            bounds.x,
                            bounds.y + left_h as i32 + half_gap,
                            bounds.width,
                            right_h.max(1),
                        );

                        left.compute_geometries(left_bounds, inner_gaps, out);
                        right.compute_geometries(right_bounds, inner_gaps, out);
                    }
                }
            }
        }
    }
}
