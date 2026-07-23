pub mod drag_detect;
pub mod layouts;
pub mod tree;

use crate::backend::Backend;
use crate::config::{Config, LayoutType};
use crate::state::{Geometry, StateManager};
use anyhow::Result;
use drag_detect::{DragDetector, DropZone};
use layouts::{DwindleLayout, FloatingLayout, LayoutStrategy, MasterStackLayout};
use std::collections::HashMap;

pub struct Engine {
    config: Config,
    state_manager: StateManager,
    drag_detector: DragDetector,
    screen_bounds: Geometry,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state_manager: StateManager::new(),
            drag_detector: DragDetector::new(),
            screen_bounds: Geometry::new(0, 0, 1920, 1080),
        }
    }

    pub fn set_screen_bounds(&mut self, bounds: Geometry) {
        self.screen_bounds = bounds;
    }

    pub fn state_manager_mut(&mut self) -> &mut StateManager {
        &mut self.state_manager
    }

    pub fn state_manager(&self) -> &StateManager {
        &self.state_manager
    }

    pub async fn apply_layout(&mut self, backend: &dyn Backend) -> Result<()> {
        if self.config.layout == LayoutType::Floating {
            return Ok(());
        }

        let tiled_ids = self.state_manager.get_tiled_window_ids();
        if tiled_ids.is_empty() {
            return Ok(());
        }

        let layout_strategy: Box<dyn LayoutStrategy> = match self.config.layout {
            LayoutType::Dwindle => Box::new(DwindleLayout),
            LayoutType::MasterStack => Box::new(MasterStackLayout {
                master_ratio: self.config.master_ratio,
            }),
            LayoutType::Floating => Box::new(FloatingLayout),
        };

        let calculated_geometries = layout_strategy.compute_layout(
            &tiled_ids,
            self.screen_bounds,
            self.config.gaps,
            self.config.inner_gaps,
        );

        for (&win_id, &geom) in calculated_geometries.iter() {
            self.state_manager.update_geometry(win_id, geom);
            backend
                .set_geometry(win_id, geom.x, geom.y, geom.width, geom.height)
                .await?;
        }

        Ok(())
    }

    pub async fn handle_drag_drop(
        &mut self,
        backend: &dyn Backend,
        dragged_id: u64,
        pointer_x: i32,
        pointer_y: i32,
    ) -> Result<()> {
        let tiled_geometries: HashMap<u64, Geometry> = self
            .state_manager
            .get_tiled_window_ids()
            .into_iter()
            .filter_map(|id| {
                self.state_manager
                    .windows
                    .get(&id)
                    .map(|w| (id, w.current_geometry))
            })
            .collect();

        let drop_zone = self.drag_detector.detect_drop_zone(
            pointer_x,
            pointer_y,
            self.screen_bounds,
            &tiled_geometries,
            dragged_id,
        );

        match drop_zone {
            DropZone::CenterSwap { target_id } => {
                let ids = self.state_manager.get_tiled_window_ids();
                if let (Some(pos_a), Some(pos_b)) = (
                    ids.iter().position(|&x| x == dragged_id),
                    ids.iter().position(|&x| x == target_id),
                ) {
                    let mut mut_ids = ids;
                    mut_ids.swap(pos_a, pos_b);
                }
            }
            _ => {}
        }

        self.apply_layout(backend).await?;

        Ok(())
    }

    pub async fn restore_all_windows(&self, backend: &dyn Backend) -> Result<()> {
        for window in self.state_manager.windows.values() {
            if window.is_tiled {
                let orig = window.original_geometry;
                let _ = backend
                    .set_geometry(window.id, orig.x, orig.y, orig.width, orig.height)
                    .await;
            }
        }
        Ok(())
    }
}
