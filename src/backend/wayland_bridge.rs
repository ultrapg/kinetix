use super::{Backend, WindowEvent, WindowInfo};
use crate::kwin_bridge::{KWinBridge, KWinBridgeConfig};
use crate::state::Geometry;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::info;
use tokio::sync::mpsc;
use zbus::connection;

/// Minimal D-Bus service - only provides a "ScriptReady" endpoint so the KWin script
/// can optionally signal readiness. All real tiling is done inside the KWin script itself.
pub struct KinetixBridgeServer {
    tx: mpsc::Sender<WindowEvent>,
}

impl KinetixBridgeServer {
    pub fn new(tx: mpsc::Sender<WindowEvent>) -> Self {
        Self { tx }
    }
}

#[zbus::interface(name = "org.kde.kinetix.Bridge")]
impl KinetixBridgeServer {
    async fn script_ready(&self, _payload: String) {
        info!("Wayland: KWin tiling script reported ready.");
        let _ = self.tx.send(WindowEvent::ScriptReady).await;
    }
}

pub struct WaylandBridgeBackend {
    kwin_bridge: std::sync::Arc<tokio::sync::Mutex<KWinBridge>>,
    bridge_cfg: KWinBridgeConfig,
    dbus_conn: std::sync::Arc<tokio::sync::Mutex<Option<zbus::Connection>>>,
}

impl WaylandBridgeBackend {
    pub fn new(max_windows: u32, swap_zone_ratio: f32, gaps: u32, inner_gaps: u32) -> Self {
        Self {
            kwin_bridge: std::sync::Arc::new(tokio::sync::Mutex::new(KWinBridge::new())),
            bridge_cfg: KWinBridgeConfig { max_windows, swap_zone_ratio, gaps, inner_gaps },
            dbus_conn: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}

#[async_trait]
impl Backend for WaylandBridgeBackend {
    async fn init(&mut self) -> Result<()> {
        if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
            return Err(anyhow!(
                "DBUS_SESSION_BUS_ADDRESS environment variable missing"
            ));
        }
        info!("Wayland (KWin D-Bus bridge) backend initialized successfully.");
        Ok(())
    }

    async fn get_windows(&self) -> Result<Vec<WindowInfo>> {
        Ok(vec![])
    }

    async fn set_geometry(&self, _window_id: u64, _x: i32, _y: i32, _w: u32, _h: u32) -> Result<()> {
        Ok(())
    }

    async fn get_geometry(&self, _window_id: u64) -> Result<Geometry> {
        Err(anyhow!("Geometry tracking not used in self-contained KWin script mode"))
    }

    async fn subscribe_events(&self, sender: mpsc::Sender<WindowEvent>) -> Result<()> {
        let server = KinetixBridgeServer::new(sender);

        let conn = connection::Builder::session()?
            .name("org.kde.kinetix.Bridge")?
            .serve_at("/Bridge", server)?
            .build()
            .await?;

        {
            let mut dbus_guard = self.dbus_conn.lock().await;
            *dbus_guard = Some(conn);
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
            let mut dbus_guard = self.dbus_conn.lock().await;
            *dbus_guard = None;
        }
        info!("Wayland bridge backend cleanup complete.");
        Ok(())
    }
}
