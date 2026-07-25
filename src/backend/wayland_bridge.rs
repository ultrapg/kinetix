use super::{Backend, WindowEvent, WindowInfo};
use crate::kwin_bridge::{KWinBridge, KWinBridgeConfig};
use crate::overlay::OverlayManager;
use crate::state::Geometry;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::info;
use tokio::sync::mpsc;
use zbus::connection;

/// D-Bus service that the KWin JS script calls back into.
/// - `ScriptReady` — script init signal
/// - `ShowOverlay` — show drag-preview border at given rect (JSON payload)
/// - `HideOverlay` — hide the preview border
pub struct KinetixBridgeServer {
    tx:      mpsc::Sender<WindowEvent>,
    overlay: OverlayManager,
}

impl KinetixBridgeServer {
    pub fn new(tx: mpsc::Sender<WindowEvent>, overlay: OverlayManager) -> Self {
        Self { tx, overlay }
    }
}

#[zbus::interface(name = "org.kde.kinetix.Bridge")]
impl KinetixBridgeServer {
    async fn script_ready(&self, _payload: String) {
        info!("Wayland: KWin tiling script reported ready.");
        let _ = self.tx.send(WindowEvent::ScriptReady).await;
    }

    /// Called from the KWin JS script when a drag enters a new zone.
    /// `data` is JSON: `{"x":…,"y":…,"w":…,"h":…,"screenW":…,"screenH":…}`
    /// screenW/H are the KWin logical screen dimensions used to compute the
    /// physical/logical scale factor inside the X11 overlay thread.
    async fn show_overlay(&self, data: String) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
            let x  = v["x"].as_i64().unwrap_or(0) as i32;
            let y  = v["y"].as_i64().unwrap_or(0) as i32;
            let w  = v["w"].as_i64().unwrap_or(100).max(1) as u32;
            let h  = v["h"].as_i64().unwrap_or(100).max(1) as u32;
            let sw = v["screenW"].as_u64().unwrap_or(0) as u32;
            let sh = v["screenH"].as_u64().unwrap_or(0) as u32;
            let blocked = v["blocked"].as_bool().unwrap_or(false);
            self.overlay.show(x, y, w, h, sw, sh, blocked);
        }
    }

    /// Called from the KWin JS script when the drag ends or leaves all zones.
    async fn hide_overlay(&self, _payload: String) {
        self.overlay.hide();
    }
}

pub struct WaylandBridgeBackend {
    kwin_bridge: std::sync::Arc<tokio::sync::Mutex<KWinBridge>>,
    bridge_cfg:  KWinBridgeConfig,
    dbus_conn:   std::sync::Arc<tokio::sync::Mutex<Option<zbus::Connection>>>,
}

impl WaylandBridgeBackend {
    pub fn new(max_windows: u32, swap_zone_ratio: f32, gaps: u32, inner_gaps: u32) -> Self {
        Self {
            kwin_bridge: std::sync::Arc::new(tokio::sync::Mutex::new(KWinBridge::new())),
            bridge_cfg:  KWinBridgeConfig { max_windows, swap_zone_ratio, gaps, inner_gaps },
            dbus_conn:   std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}

#[async_trait]
impl Backend for WaylandBridgeBackend {
    async fn init(&mut self) -> Result<()> {
        if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
            return Err(anyhow!("DBUS_SESSION_BUS_ADDRESS environment variable missing"));
        }
        info!("Wayland (KWin D-Bus bridge) backend initialized successfully.");
        Ok(())
    }

    async fn get_windows(&self) -> Result<Vec<WindowInfo>> {
        Ok(vec![])
    }

    async fn set_geometry(&self, _id: u64, _x: i32, _y: i32, _w: u32, _h: u32) -> Result<()> {
        Ok(())
    }

    async fn get_geometry(&self, _id: u64) -> Result<Geometry> {
        Err(anyhow!("Geometry tracking not used in self-contained KWin script mode"))
    }

    async fn subscribe_events(&self, sender: mpsc::Sender<WindowEvent>) -> Result<()> {
        // Spawn the X11 overlay window thread
        let overlay = OverlayManager::spawn();
        let server  = KinetixBridgeServer::new(sender, overlay);

        let conn = connection::Builder::session()?
            .name("org.kde.kinetix.Bridge")?
            .serve_at("/Bridge", server)?
            .build()
            .await?;

        {
            let mut g = self.dbus_conn.lock().await;
            *g = Some(conn);
        }

        info!("Registered org.kde.kinetix.Bridge D-Bus server at /Bridge");

        let mut bridge = self.kwin_bridge.lock().await;
        bridge.init(&self.bridge_cfg).await?;
        Ok(())
    }

    fn supports_drag_detection(&self) -> bool {
        false
    }

    async fn cleanup(&mut self) -> Result<()> {
        let mut bridge = self.kwin_bridge.lock().await;
        bridge.cleanup().await?;
        {
            let mut g = self.dbus_conn.lock().await;
            *g = None;
        }
        info!("Wayland bridge backend cleanup complete.");
        Ok(())
    }
}
