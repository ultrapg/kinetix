mod backend;
mod config;
mod engine;
mod kwin_bridge;
mod overlay;
mod state;

use anyhow::{anyhow, Result};
use backend::wayland_bridge::WaylandBridgeBackend;
use backend::x11::X11Backend;
use backend::{Backend, WindowEvent};
use clap::Parser;
use config::Config;
use engine::Engine;
use log::{error, info, warn, LevelFilter};
use simplelog::{CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger};
use std::fs;
use std::path::PathBuf;
use tokio::sync::mpsc;

fn check_sudo_warning(force_sudo: bool) -> Result<()> {
    let uid = unsafe { libc::geteuid() };
    if uid == 0 {
        eprintln!("===========================================================");
        eprintln!(" WARNING: Kinetix is running with root/sudo privileges!");
        eprintln!(" Running with sudo breaks session D-Bus and XDG_RUNTIME_DIR");
        eprintln!(" environment connections required for Wayland and X11.");
        eprintln!(" Sudo is completely unnecessary and harmful for Kinetix.");
        eprintln!("===========================================================");

        if !force_sudo {
            eprintln!("Refusing to start. Use --force-sudo to override this safety check.");
            std::process::exit(1);
        } else {
            eprintln!("--force-sudo provided. Proceeding despite root execution...");
        }
    }
    Ok(())
}

fn setup_runtime_dir_and_logger(debug_enabled: bool) -> Result<()> {
    let runtime_dir = PathBuf::from("./.kinetix");
    if !runtime_dir.exists() {
        fs::create_dir_all(&runtime_dir)?;
    }

    let mut loggers: Vec<Box<dyn simplelog::SharedLogger>> = Vec::new();
    let log_level = if debug_enabled {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    let log_config = ConfigBuilder::new().build();

    loggers.push(TermLogger::new(
        log_level,
        log_config.clone(),
        TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    ));

    if debug_enabled {
        let log_file_path = runtime_dir.join("kinetix.log");
        if let Ok(file) = fs::File::create(log_file_path) {
            loggers.push(WriteLogger::new(log_level, log_config, file));
        }
    }

    CombinedLogger::init(loggers)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();

    check_sudo_warning(config.force_sudo)?;
    setup_runtime_dir_and_logger(config.debug)?;

    info!("Starting Kinetix Tiling Engine v{}", env!("CARGO_PKG_VERSION"));
    info!("Configuration: layout={}, gaps={}, inner_gaps={}, drag_threshold={}, max_windows={}, swap_zone_ratio={}",
        config.layout, config.gaps, config.inner_gaps, config.drag_threshold,
        config.max_windows, config.swap_zone_ratio);

    // Auto-detect Display Server
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok().filter(|s| !s.is_empty());
    let dbus_bus = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
    let x11_display = std::env::var("DISPLAY").ok();

    let mut backend: Box<dyn Backend> = if wayland_display.is_some() && dbus_bus.is_some() {
        info!("Detected Wayland environment (WAYLAND_DISPLAY={:?}). Initializing KWin D-Bus bridge...", wayland_display.unwrap());
        Box::new(WaylandBridgeBackend::new(config.max_windows, config.swap_zone_ratio, config.gaps, config.inner_gaps))
    } else if let Some(disp) = x11_display {
        info!("Detected X11 environment (DISPLAY={}). Initializing x11rb backend...", disp);
        Box::new(X11Backend::new(config.drag_threshold))
    } else {
        return Err(anyhow!("No supported display server detected! Set DISPLAY or WAYLAND_DISPLAY + DBUS_SESSION_BUS_ADDRESS."));
    };

    // Initialize backend
    backend.init().await?;

    // Channel for events
    let (tx, mut rx) = mpsc::channel::<WindowEvent>(100);
    backend.subscribe_events(tx).await?;

    let mut engine = Engine::new(config.clone());

    // Register initial windows (for X11)
    match backend.get_windows().await {
        Ok(windows) => {
            for win in windows {
                let is_tileable = engine.state_manager().is_tileable(&win.class, None);
                engine.state_manager_mut().register_window(
                    win.id,
                    win.class,
                    win.title,
                    win.geometry,
                    is_tileable,
                );
            }
        }
        Err(e) => {
            warn!("Failed to query initial window list: {}", e);
        }
    }

    // Apply initial layout
    if let Err(e) = engine.apply_layout(backend.as_ref()).await {
        error!("Error applying initial layout: {}", e);
    }

    info!("Kinetix running. Press Ctrl+C to exit.");

    // Signal handlers for graceful shutdown
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    loop {
        tokio::select! {
            _ = sigint.recv() => {
                info!("SIGINT received. Initiating graceful shutdown...");
                break;
            }
            _ = sigterm.recv() => {
                info!("SIGTERM received. Initiating graceful shutdown...");
                break;
            }
            Some(event) = rx.recv() => {
                match event {
                    WindowEvent::WindowCreated { info: win, screen_bounds } => {
                        if let Some(bounds) = screen_bounds {
                            info!("Updating engine screen bounds to: {:?}", bounds);
                            engine.set_screen_bounds(bounds);
                        }
                        let is_tileable = engine.state_manager().is_tileable(&win.class, None);
                        engine.state_manager_mut().register_window(
                            win.id,
                            win.class,
                            win.title,
                            win.geometry,
                            is_tileable,
                        );
                        let _ = engine.apply_layout(backend.as_ref()).await;
                    }
                    WindowEvent::WindowDestroyed(win_id) => {
                        engine.state_manager_mut().unregister_window(win_id);
                        let _ = engine.apply_layout(backend.as_ref()).await;
                    }
                    WindowEvent::WindowMovedResized { id, geometry, is_user_drag } => {
                        if is_user_drag {
                            let (px, py) = geometry.center();
                            info!("User drag detected for window {}. Rebuilding layout...", id);
                            let _ = engine.handle_drag_drop(backend.as_ref(), id, px, py).await;
                        } else {
                            engine.state_manager_mut().update_geometry(id, geometry);
                        }
                    }
                    WindowEvent::WindowFocused(_win_id) => {
                        // Focus changed
                    }
                    WindowEvent::ScriptReady => {
                        info!("KWin tiling script is ready and active.");
                    }
                }
            }
        }
    }

    // Shutdown Sequence
    if config.restore_on_exit {
        info!("Restoring original window geometries...");
        if let Err(e) = engine.restore_all_windows(backend.as_ref()).await {
            error!("Error restoring windows: {}", e);
        }
    }

    backend.cleanup().await?;

    let bridge_js = PathBuf::from("./.kinetix/bridge.js");
    if bridge_js.exists() {
        let _ = fs::remove_file(bridge_js);
    }

    println!("Kinetix shutdown complete.");
    Ok(())
}
