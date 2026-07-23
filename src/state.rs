use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Geometry {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn center(&self) -> (i32, i32) {
        (
            self.x + (self.width as i32 / 2),
            self.y + (self.height as i32 / 2),
        )
    }

    pub fn contains_point(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x + self.width as i32
            && py >= self.y
            && py < self.y + self.height as i32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub id: u64,
    pub class: String,
    pub title: String,
    pub current_geometry: Geometry,
    pub original_geometry: Geometry,
    pub is_tiled: bool,
    pub is_being_dragged: bool,
}

#[derive(Debug, Default)]
pub struct StateManager {
    pub windows: HashMap<u64, WindowState>,
    ignored_classes: Vec<String>,
}

impl StateManager {
    pub fn new() -> Self {
        let ignored_classes = vec![
            "plasmashell".to_string(),
            "yakuake".to_string(),
            "conky".to_string(),
            "krunner".to_string(),
            "kded".to_string(),
            "kded5".to_string(),
            "kded6".to_string(),
            "kinetix".to_string(),
            "desktop".to_string(),
            "dock".to_string(),
        ];

        Self {
            windows: HashMap::new(),
            ignored_classes,
        }
    }

    pub fn is_tileable(&self, class: &str, window_type: Option<&str>) -> bool {
        let class_lower = class.to_lowercase();
        if self
            .ignored_classes
            .iter()
            .any(|ignored| class_lower.contains(ignored))
        {
            return false;
        }

        if let Some(wtype) = window_type {
            let wtype_lower = wtype.to_lowercase();
            if wtype_lower.contains("dialog")
                || wtype_lower.contains("menu")
                || wtype_lower.contains("splash")
                || wtype_lower.contains("dock")
                || wtype_lower.contains("notification")
                || wtype_lower.contains("toolbar")
            {
                return false;
            }
        }

        true
    }

    pub fn register_window(
        &mut self,
        id: u64,
        class: String,
        title: String,
        geometry: Geometry,
        is_tiled: bool,
    ) {
        self.windows
            .entry(id)
            .or_insert_with(|| WindowState {
                id,
                class,
                title,
                current_geometry: geometry,
                original_geometry: geometry,
                is_tiled,
                is_being_dragged: false,
            });
    }

    pub fn unregister_window(&mut self, id: u64) -> Option<WindowState> {
        self.windows.remove(&id)
    }

    pub fn update_geometry(&mut self, id: u64, geometry: Geometry) {
        if let Some(win) = self.windows.get_mut(&id) {
            win.current_geometry = geometry;
        }
    }

    pub fn get_tiled_window_ids(&self) -> Vec<u64> {
        self.windows
            .values()
            .filter(|w| w.is_tiled)
            .map(|w| w.id)
            .collect()
    }
}
