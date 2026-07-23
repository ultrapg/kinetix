use crate::state::Geometry;
use std::collections::HashMap;

pub trait LayoutStrategy {
    fn compute_layout(
        &self,
        windows: &[u64],
        screen_bounds: Geometry,
        gaps: u32,
        inner_gaps: u32,
    ) -> HashMap<u64, Geometry>;
}

pub struct DwindleLayout;
pub struct MasterStackLayout {
    pub master_ratio: f32,
}
pub struct FloatingLayout;

impl LayoutStrategy for DwindleLayout {
    fn compute_layout(
        &self,
        windows: &[u64],
        screen_bounds: Geometry,
        gaps: u32,
        inner_gaps: u32,
    ) -> HashMap<u64, Geometry> {
        let mut result = HashMap::new();
        if windows.is_empty() {
            return result;
        }

        let usable_x = screen_bounds.x + gaps as i32;
        let usable_y = screen_bounds.y + gaps as i32;
        let usable_w = screen_bounds.width.saturating_sub(gaps * 2);
        let usable_h = screen_bounds.height.saturating_sub(gaps * 2);

        let mut current_bounds = Geometry::new(usable_x, usable_y, usable_w, usable_h);
        let count = windows.len();

        for (i, &win_id) in windows.iter().enumerate() {
            if i == count - 1 {
                result.insert(win_id, current_bounds);
                break;
            }

            let is_horizontal_split = i % 2 == 0;
            if is_horizontal_split {
                let half_w = (current_bounds.width / 2).saturating_sub(inner_gaps / 2);
                let left_geom = Geometry::new(
                    current_bounds.x,
                    current_bounds.y,
                    half_w,
                    current_bounds.height,
                );
                result.insert(win_id, left_geom);

                let next_x = current_bounds.x + half_w as i32 + inner_gaps as i32;
                let next_w = current_bounds.width.saturating_sub(half_w + inner_gaps);
                current_bounds = Geometry::new(next_x, current_bounds.y, next_w, current_bounds.height);
            } else {
                let half_h = (current_bounds.height / 2).saturating_sub(inner_gaps / 2);
                let top_geom = Geometry::new(
                    current_bounds.x,
                    current_bounds.y,
                    current_bounds.width,
                    half_h,
                );
                result.insert(win_id, top_geom);

                let next_y = current_bounds.y + half_h as i32 + inner_gaps as i32;
                let next_h = current_bounds.height.saturating_sub(half_h + inner_gaps);
                current_bounds = Geometry::new(current_bounds.x, next_y, current_bounds.width, next_h);
            }
        }

        result
    }
}

impl LayoutStrategy for MasterStackLayout {
    fn compute_layout(
        &self,
        windows: &[u64],
        screen_bounds: Geometry,
        gaps: u32,
        inner_gaps: u32,
    ) -> HashMap<u64, Geometry> {
        let mut result = HashMap::new();
        if windows.is_empty() {
            return result;
        }

        let usable_x = screen_bounds.x + gaps as i32;
        let usable_y = screen_bounds.y + gaps as i32;
        let usable_w = screen_bounds.width.saturating_sub(gaps * 2);
        let usable_h = screen_bounds.height.saturating_sub(gaps * 2);

        if windows.len() == 1 {
            result.insert(windows[0], Geometry::new(usable_x, usable_y, usable_w, usable_h));
            return result;
        }

        let master_w = ((usable_w as f32 * self.master_ratio) as u32).saturating_sub(inner_gaps / 2);
        let stack_x = usable_x + master_w as i32 + inner_gaps as i32;
        let stack_w = usable_w.saturating_sub(master_w + inner_gaps);

        // Master window
        result.insert(windows[0], Geometry::new(usable_x, usable_y, master_w, usable_h));

        // Stack windows
        let stack_count = windows.len() - 1;
        let stack_h = (usable_h.saturating_sub(inner_gaps * (stack_count as u32 - 1))) / stack_count as u32;

        for (idx, &win_id) in windows[1..].iter().enumerate() {
            let win_y = usable_y + (idx as u32 * (stack_h + inner_gaps)) as i32;
            result.insert(win_id, Geometry::new(stack_x, win_y, stack_w, stack_h));
        }

        result
    }
}

impl LayoutStrategy for FloatingLayout {
    fn compute_layout(
        &self,
        _windows: &[u64],
        _screen_bounds: Geometry,
        _gaps: u32,
        _inner_gaps: u32,
    ) -> HashMap<u64, Geometry> {
        HashMap::new()
    }
}
