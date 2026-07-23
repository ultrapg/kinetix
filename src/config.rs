use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LayoutType {
    Dwindle,
    MasterStack,
    Floating,
}

impl std::fmt::Display for LayoutType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutType::Dwindle => write!(f, "dwindle"),
            LayoutType::MasterStack => write!(f, "master-stack"),
            LayoutType::Floating => write!(f, "floating"),
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[command(name = "kinetix", author, version, about = "Zero-installation single-binary tiling engine", long_about = None)]
pub struct Config {
    /// Layout strategy: dwindle, master-stack, or floating
    #[arg(long, value_enum, default_value_t = LayoutType::Dwindle)]
    pub layout: LayoutType,

    /// Outer screen gaps/padding in pixels
    #[arg(long, default_value_t = 0)]
    pub gaps: u32,

    /// Inner gaps between windows in pixels
    #[arg(long, default_value_t = 0)]
    pub inner_gaps: u32,

    /// Movement threshold in pixels to classify as user drag
    #[arg(long, default_value_t = 20)]
    pub drag_threshold: i32,

    /// Restore windows to original geometries on shutdown
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub restore_on_exit: bool,

    /// Enable debug output logging to ./.kinetix/kinetix.log
    #[arg(long, default_value_t = false)]
    pub debug: bool,

    /// Bypass warning and safety exit when running as root/sudo
    #[arg(long, default_value_t = false)]
    pub force_sudo: bool,

    /// Master area width ratio for master-stack layout (0.1 to 0.9)
    #[arg(long, default_value_t = 0.5)]
    pub master_ratio: f32,

    /// Maximum number of tiled windows on screen (0 = unlimited)
    #[arg(long, default_value_t = 0)]
    pub max_windows: u32,

    /// Center swap zone ratio (0.0-1.0): fraction of window area classified as
    /// 'center' for drag-to-swap. Outside this zone = drag-to-side (split).
    /// e.g. 0.4 means the middle 40% width AND height = swap, edges = split.
    #[arg(long, default_value_t = 0.4)]
    pub swap_zone_ratio: f32,
}
