use super::{Backend, WindowEvent, WindowInfo};
use crate::state::Geometry;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::{debug, error, info};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _,
    EventMask, GetGeometryReply, Window,
};
use x11rb::rust_connection::RustConnection;

pub struct X11Backend {
    conn: Option<Arc<RustConnection>>,
    screen_num: usize,
    drag_threshold: i32,
    last_programmatic_moves: Arc<Mutex<HashMap<u64, Instant>>>,
}

impl X11Backend {
    pub fn new(drag_threshold: i32) -> Self {
        Self {
            conn: None,
            screen_num: 0,
            drag_threshold,
            last_programmatic_moves: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn get_conn(&self) -> Result<Arc<RustConnection>> {
        self.conn
            .clone()
            .ok_or_else(|| anyhow!("X11 connection not initialized"))
    }

    fn get_window_class(&self, win: Window) -> String {
        if let Ok(conn) = self.get_conn() {
            if let Ok(cookie) = conn.get_property(false, win, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024) {
                if let Ok(reply) = cookie.reply() {
                    let value = String::from_utf8_lossy(&reply.value).to_string();
                    let parts: Vec<&str> = value.split('\0').filter(|s| !s.is_empty()).collect();
                    if !parts.is_empty() {
                        return parts.last().unwrap_or(&"x11-window").to_string();
                    }
                }
            }
        }
        "x11-window".to_string()
    }
}

#[async_trait]
impl Backend for X11Backend {
    async fn init(&mut self) -> Result<()> {
        let (conn, screen_num) = x11rb::connect(None)
            .map_err(|e| anyhow!("Failed to connect to X11 display: {}", e))?;

        let conn = Arc::new(conn);
        self.screen_num = screen_num;

        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;

        let aux = ChangeWindowAttributesAux::new().event_mask(
            EventMask::SUBSTRUCTURE_NOTIFY
                | EventMask::SUBSTRUCTURE_REDIRECT
                | EventMask::STRUCTURE_NOTIFY
                | EventMask::PROPERTY_CHANGE,
        );

        let width = screen.width_in_pixels;
        let height = screen.height_in_pixels;
        let _ = conn.change_window_attributes(root, &aux);

        conn.flush()?;
        self.conn = Some(conn);
        info!("X11 backend initialized successfully. Screen size: {}x{}", width, height);
        Ok(())
    }

    async fn get_windows(&self) -> Result<Vec<WindowInfo>> {
        let conn = self.get_conn()?;
        let screen = &conn.setup().roots[self.screen_num];
        let root = screen.root;

        // Try _NET_CLIENT_LIST first for EWMH window managers
        let mut window_ids: Vec<Window> = Vec::new();
        if let Ok(reply_atom) = conn.intern_atom(false, b"_NET_CLIENT_LIST") {
            if let Ok(atom_reply) = reply_atom.reply() {
                if let Ok(prop_cookie) = conn.get_property(false, root, atom_reply.atom, AtomEnum::WINDOW, 0, 4096) {
                    if let Ok(prop) = prop_cookie.reply() {
                        if let Some(val32) = prop.value32() {
                            window_ids = val32.collect();
                        }
                    }
                }
            }
        }

        // Fallback to query_tree if _NET_CLIENT_LIST was empty
        if window_ids.is_empty() {
            if let Ok(tree) = conn.query_tree(root)?.reply() {
                window_ids = tree.children;
            }
        }

        let mut result = Vec::new();

        for &win in window_ids.iter() {
            let attrs = match conn.get_window_attributes(win) {
                Ok(cookie) => match cookie.reply() {
                    Ok(a) => a,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            if attrs.override_redirect || attrs.map_state != x11rb::protocol::xproto::MapState::VIEWABLE {
                continue;
            }

            let geom: GetGeometryReply = match conn.get_geometry(win) {
                Ok(cookie) => match cookie.reply() {
                    Ok(g) => g,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            let class = self.get_window_class(win);
            let title = format!("Window {}", win);

            info!("Discovered X11 window ID: {}, class: {}, geometry: ({}, {}, {}, {})",
                win, class, geom.x, geom.y, geom.width, geom.height);

            result.push(WindowInfo {
                id: win as u64,
                class,
                title,
                geometry: Geometry::new(
                    geom.x as i32,
                    geom.y as i32,
                    geom.width as u32,
                    geom.height as u32,
                ),
                is_tileable: true,
            });
        }

        info!("Total tileable X11 windows discovered: {}", result.len());
        Ok(result)
    }

    async fn set_geometry(&self, window_id: u64, x: i32, y: i32, w: u32, h: u32) -> Result<()> {
        let conn = self.get_conn()?;
        let win = window_id as Window;
        let screen = &conn.setup().roots[self.screen_num];
        let root = screen.root;

        {
            let mut moves = self.last_programmatic_moves.lock().unwrap();
            moves.insert(window_id, Instant::now());
        }

        // Direct ConfigureWindow call
        let aux = ConfigureWindowAux::new()
            .x(x)
            .y(y)
            .width(w)
            .height(h);
        let _ = conn.configure_window(win, &aux);

        // EWMH _NET_MOVERESIZE_WINDOW client message for window managers
        if let Ok(reply) = conn.intern_atom(false, b"_NET_MOVERESIZE_WINDOW") {
            if let Ok(atom_reply) = reply.reply() {
                let event = ClientMessageEvent::new(
                    32,
                    win,
                    atom_reply.atom,
                    [
                        0x0F00, // StaticGravity, X, Y, Width, Height
                        x as u32,
                        y as u32,
                        w,
                        h,
                    ],
                );
                let _ = conn.send_event(
                    false,
                    root,
                    EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT,
                    event,
                );
            }
        }

        conn.flush()?;
        info!("X11 set_geometry for window {}: ({}, {}, {}, {})", window_id, x, y, w, h);
        Ok(())
    }

    async fn get_geometry(&self, window_id: u64) -> Result<Geometry> {
        let conn = self.get_conn()?;
        let win = window_id as Window;

        let geom = conn.get_geometry(win)?.reply()?;
        Ok(Geometry::new(
            geom.x as i32,
            geom.y as i32,
            geom.width as u32,
            geom.height as u32,
        ))
    }

    async fn subscribe_events(&self, sender: mpsc::Sender<WindowEvent>) -> Result<()> {
        let conn = self.get_conn()?;
        let last_moves = self.last_programmatic_moves.clone();

        tokio::task::spawn_blocking(move || loop {
            match conn.wait_for_event() {
                Ok(event) => match event {
                    x11rb::protocol::Event::MapNotify(e) => {
                        let win_id = e.window as u64;
                        let _ = sender.blocking_send(WindowEvent::WindowCreated {
                            info: WindowInfo {
                                id: win_id,
                                class: "x11-window".to_string(),
                                title: format!("Window {}", win_id),
                                geometry: Geometry::new(0, 0, 400, 300),
                                is_tileable: true,
                            },
                            screen_bounds: None,
                        });
                    }
                    x11rb::protocol::Event::UnmapNotify(e) => {
                        let _ = sender.blocking_send(WindowEvent::WindowDestroyed(e.window as u64));
                    }
                    x11rb::protocol::Event::DestroyNotify(e) => {
                        let _ = sender.blocking_send(WindowEvent::WindowDestroyed(e.window as u64));
                    }
                    x11rb::protocol::Event::ConfigureNotify(e) => {
                        let win_id = e.window as u64;
                        let is_recent_programmatic = {
                            let moves = last_moves.lock().unwrap();
                            if let Some(&time) = moves.get(&win_id) {
                                time.elapsed() < Duration::from_millis(100)
                            } else {
                                false
                            }
                        };

                        let geometry = Geometry::new(e.x as i32, e.y as i32, e.width as u32, e.height as u32);
                        let is_user_drag = !is_recent_programmatic;

                        let _ = sender.blocking_send(WindowEvent::WindowMovedResized {
                            id: win_id,
                            geometry,
                            is_user_drag,
                        });
                    }
                    _ => {}
                },
                Err(e) => {
                    error!("X11 event loop error: {}", e);
                    break;
                }
            }
        });

        Ok(())
    }

    fn supports_drag_detection(&self) -> bool {
        true
    }

    async fn cleanup(&mut self) -> Result<()> {
        info!("X11 backend cleanup completed.");
        Ok(())
    }
}
