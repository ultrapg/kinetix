//! X11/XWayland drag-preview overlay.
//!
//! Creates a borderless, override-redirect X11 window using the SHAPE
//! extension to render a hollow border rectangle above all other windows.
//! Works on KDE Plasma 6 / Wayland because XWayland is always active.
//!
//! ## HiDPI / scale handling
//! KWin reports tile coordinates in **logical pixels** (screen physical ÷ scale).
//! XWayland positions windows in **physical pixels**.  The ShowOverlay command
//! therefore includes the logical screen size so this thread can compute
//!     scale_x = x11_physical_width  / kwin_logical_width
//!     scale_y = x11_physical_height / kwin_logical_height
//! and convert every incoming coordinate before placing the X11 window.

use anyhow::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use x11rb::{
    connection::Connection,
    protocol::{
        shape::{self, ConnectionExt as ShapeExt, SK, SO},
        xproto::*,
    },
    rust_connection::RustConnection,
};

const BORDER_PX: u32 = 5;

// KDE-style accent blue: #3D9AE8
const COLOR_R: u8 = 0x3D;
const COLOR_G: u8 = 0x9A;
const COLOR_B: u8 = 0xE8;

// ---------------------------------------------------------------
// Public API
// ---------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum OverlayCmd {
    /// Show the overlay at the given **KWin logical** rect.
    /// `logical_screen_w/h` are the full KWin logical screen dimensions;
    /// the thread uses them to derive the physical/logical scale factor.
    Show {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        logical_screen_w: u32,
        logical_screen_h: u32,
    },
    Hide,
}

/// Thread-safe handle to the overlay window thread.
#[derive(Clone)]
pub struct OverlayManager {
    tx: mpsc::SyncSender<OverlayCmd>,
}

impl OverlayManager {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::sync_channel(32);
        thread::Builder::new()
            .name("kinetix-overlay".into())
            .spawn(move || {
                if let Err(e) = overlay_thread(rx) {
                    eprintln!("Kinetix overlay thread exited: {e}");
                }
            })
            .expect("failed to spawn overlay thread");
        Self { tx }
    }

    pub fn show(&self, x: i32, y: i32, w: u32, h: u32, logical_screen_w: u32, logical_screen_h: u32) {
        let _ = self.tx.try_send(OverlayCmd::Show { x, y, w, h, logical_screen_w, logical_screen_h });
    }

    pub fn hide(&self) {
        let _ = self.tx.try_send(OverlayCmd::Hide);
    }
}

// ---------------------------------------------------------------
// X11 rendering thread
// ---------------------------------------------------------------

fn overlay_thread(rx: mpsc::Receiver<OverlayCmd>) -> Result<()> {
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
    let (conn, screen_num) = RustConnection::connect(Some(&display))?;

    let screen = conn.setup().roots[screen_num].clone();
    let phys_w = screen.width_in_pixels  as f64;  // X11 physical pixels
    let phys_h = screen.height_in_pixels as f64;

    let pixel = compute_pixel(&screen, COLOR_R, COLOR_G, COLOR_B);

    let win = conn.generate_id()?;
    let gc  = conn.generate_id()?;

    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        win,
        screen.root,
        -4000, -4000, 8, 8,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &CreateWindowAux::new()
            .override_redirect(1u32)
            .background_pixel(pixel)
            .border_pixel(pixel)
            .event_mask(EventMask::EXPOSURE),
    )?;

    // Empty INPUT shape → window never intercepts mouse events.
    conn.shape_rectangles(SO::SET, SK::INPUT, ClipOrdering::UNSORTED, win, 0, 0, &[])?;

    conn.create_gc(gc, win, &CreateGCAux::new()
        .foreground(pixel).background(pixel))?;

    conn.flush()?;

    let mut visible  = false;
    let mut cur_w    = 8u32;
    let mut cur_h    = 8u32;

    loop {
        let mut last: Option<OverlayCmd> = None;
        loop {
            match rx.try_recv() {
                Ok(cmd) => last = Some(cmd),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let _ = conn.unmap_window(win);
                    let _ = conn.flush();
                    return Ok(());
                }
            }
        }

        if let Some(cmd) = last {
            match cmd {
                OverlayCmd::Show { x, y, w, h, logical_screen_w, logical_screen_h } => {
                    // Compute scale: logical → physical pixel conversion.
                    let sx = if logical_screen_w > 0 { phys_w / logical_screen_w as f64 } else { 1.0 };
                    let sy = if logical_screen_h > 0 { phys_h / logical_screen_h as f64 } else { 1.0 };

                    let px = (x as f64 * sx).round() as i32;
                    let py = (y as f64 * sy).round() as i32;
                    let pw = ((w as f64 * sx).round() as u32).max(BORDER_PX * 2 + 2);
                    let ph = ((h as f64 * sy).round() as u32).max(BORDER_PX * 2 + 2);

                    cur_w = pw;
                    cur_h = ph;

                    conn.configure_window(win, &ConfigureWindowAux::new()
                        .x(px).y(py)
                        .width(cur_w).height(cur_h)
                        .stack_mode(StackMode::ABOVE))?;

                    apply_hollow_shape(&conn, win, cur_w, cur_h)?;

                    if !visible {
                        conn.map_window(win)?;
                        visible = true;
                    }
                    redraw(&conn, win, gc, cur_w, cur_h)?;
                }
                OverlayCmd::Hide => {
                    if visible {
                        conn.unmap_window(win)?;
                        visible = false;
                    }
                }
            }
        }

        while let Ok(Some(event)) = conn.poll_for_event() {
            if let x11rb::protocol::Event::Expose(_) = event {
                if visible { let _ = redraw(&conn, win, gc, cur_w, cur_h); }
            }
        }

        conn.flush()?;
        thread::sleep(Duration::from_millis(8));
    }
}

fn redraw(conn: &RustConnection, win: Window, gc: Gcontext, w: u32, h: u32) -> Result<()> {
    conn.poly_fill_rectangle(win, gc, &[Rectangle {
        x: 0, y: 0, width: w as u16, height: h as u16,
    }])?;
    Ok(())
}

fn apply_hollow_shape(conn: &RustConnection, win: Window, w: u32, h: u32) -> Result<()> {
    let outer = Rectangle { x: 0, y: 0, width: w as u16, height: h as u16 };
    conn.shape_rectangles(SO::SET, SK::BOUNDING, ClipOrdering::UNSORTED, win, 0, 0, &[outer])?;

    let b = BORDER_PX as i32;
    let iw = (w as i32 - 2 * b).max(0) as u16;
    let ih = (h as i32 - 2 * b).max(0) as u16;
    if iw > 0 && ih > 0 {
        conn.shape_rectangles(SO::SUBTRACT, SK::BOUNDING, ClipOrdering::UNSORTED, win, 0, 0,
            &[Rectangle { x: b as i16, y: b as i16, width: iw, height: ih }])?;
    }
    Ok(())
}

fn compute_pixel(screen: &Screen, r: u8, g: u8, b: u8) -> u32 {
    for di in &screen.allowed_depths {
        for vi in &di.visuals {
            if vi.visual_id == screen.root_visual {
                let rs = vi.red_mask.trailing_zeros();
                let gs = vi.green_mask.trailing_zeros();
                let bs = vi.blue_mask.trailing_zeros();
                return ((r as u32) << rs) | ((g as u32) << gs) | ((b as u32) << bs);
            }
        }
    }
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}
