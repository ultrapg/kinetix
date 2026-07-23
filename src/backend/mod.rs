pub mod wayland_bridge;
pub mod x11;

use crate::state::Geometry;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u64,
    pub class: String,
    pub title: String,
    pub geometry: Geometry,
    pub is_tileable: bool,
}

#[derive(Debug, Clone)]
pub enum WindowEvent {
    WindowCreated {
        info: WindowInfo,
        screen_bounds: Option<Geometry>,
    },
    WindowDestroyed(u64),
    WindowMovedResized {
        id: u64,
        geometry: Geometry,
        is_user_drag: bool,
    },
    WindowFocused(u64),
    ScriptReady,
}

#[async_trait]
pub trait Backend: Send + Sync {
    async fn init(&mut self) -> Result<()>;
    async fn get_windows(&self) -> Result<Vec<WindowInfo>>;
    async fn set_geometry(&self, window_id: u64, x: i32, y: i32, w: u32, h: u32) -> Result<()>;
    async fn get_geometry(&self, window_id: u64) -> Result<Geometry>;
    async fn subscribe_events(&self, sender: mpsc::Sender<WindowEvent>) -> Result<()>;
    fn supports_drag_detection(&self) -> bool;
    async fn cleanup(&mut self) -> Result<()>;
}
