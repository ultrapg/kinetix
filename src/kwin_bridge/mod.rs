pub mod script_payload;

use anyhow::{anyhow, Result};
use log::info;
use std::fs;
use std::path::PathBuf;
use zbus::{connection, proxy};

pub struct KWinBridgeConfig {
    pub max_windows: u32,
    pub swap_zone_ratio: f32,
    pub gaps: u32,
    pub inner_gaps: u32,
}

pub struct KWinBridge {
    connection: Option<zbus::Connection>,
    script_id: Option<i32>,
    temp_script_path: Option<PathBuf>,
}

impl KWinBridge {
    pub fn new() -> Self {
        Self {
            connection: None,
            script_id: None,
            temp_script_path: None,
        }
    }

    pub async fn init(&mut self, cfg: &KWinBridgeConfig) -> Result<()> {
        let conn = connection::Connection::session().await?;
        info!("D-Bus session bus connected for KWin bridge script loading.");

        let scripting_proxy = KWinScriptingProxy::new(&conn).await?;

        // Write bridge.js with embedded config values
        let dir = PathBuf::from("./.kinetix");
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        let js_file = dir.join("bridge.js");

        // Inject config into the script as a header block
        let config_header = format!(
            "// === Kinetix Config (injected at load time) ===\nvar KINETIX_MAX_WINDOWS = {};\nvar KINETIX_SWAP_ZONE_RATIO = {};\nvar KINETIX_GAPS = {};\nvar KINETIX_INNER_GAPS = {};\n// ================================================\n",
            cfg.max_windows,
            cfg.swap_zone_ratio,
            cfg.gaps,
            cfg.inner_gaps,
        );
        let full_script = format!("{}\n{}", config_header, script_payload::KWIN_SCRIPT_PAYLOAD);

        fs::write(&js_file, &full_script)?;
        let abs_path = fs::canonicalize(&js_file)?;
        let path_str = abs_path
            .to_str()
            .ok_or_else(|| anyhow!("Invalid path utf-8 string"))?;

        let script_id = scripting_proxy.load_script(path_str).await?;
        info!("KWin loaded transient script from {:?} with ID: {}", abs_path, script_id);

        let script_obj_path = format!("/Scripting/Script{}", script_id);
        let single_script_proxy = KWinSingleScriptProxy::builder(&conn)
            .path(script_obj_path.as_str())?
            .build()
            .await?;

        single_script_proxy.run().await?;
        info!("Started KWin script ID {} via D-Bus path {}", script_id, script_obj_path);

        self.connection = Some(conn);
        self.script_id = Some(script_id);
        self.temp_script_path = Some(js_file);

        Ok(())
    }

    pub async fn cleanup(&mut self) -> Result<()> {
        if let (Some(conn), Some(id)) = (&self.connection, self.script_id) {
            if let Ok(scripting_proxy) = KWinScriptingProxy::new(conn).await {
                if let Some(path) = &self.temp_script_path {
                    if let Ok(abs_path) = fs::canonicalize(path) {
                        if let Some(pstr) = abs_path.to_str() {
                            let _ = scripting_proxy.unload_script(pstr).await;
                        }
                    }
                }
                let _ = scripting_proxy.unload_script(&id.to_string()).await;
                info!("Unloaded KWin transient script ID: {}", id);
            }
        }

        if let Some(path) = &self.temp_script_path {
            if path.exists() {
                let _ = fs::remove_file(path);
            }
        }

        info!("KWinBridge cleanup finished.");
        Ok(())
    }
}

#[proxy(
    interface = "org.kde.kwin.Scripting",
    default_service = "org.kde.KWin",
    default_path = "/Scripting"
)]
trait KWinScripting {
    #[zbus(name = "loadScript")]
    fn load_script(&self, script_path: &str) -> zbus::Result<i32>;

    #[zbus(name = "unloadScript")]
    fn unload_script(&self, script_id_or_path: &str) -> zbus::Result<bool>;
}

#[proxy(
    interface = "org.kde.kwin.Script",
    default_service = "org.kde.KWin"
)]
trait KWinSingleScript {
    #[zbus(name = "run")]
    fn run(&self) -> zbus::Result<()>;

    #[zbus(name = "stop")]
    fn stop(&self) -> zbus::Result<()>;
}
