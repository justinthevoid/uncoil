//! User configuration, shared by the daemon (reads, hot-reloads) and the GUI (reads, writes).
//! Stored as JSON at `%APPDATA%\uncoil\config.json`.

use crate::effect::Effect;
use crate::layout::Placement;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub effect: Effect,
    /// 0..1
    pub brightness: f32,
    /// 0..1
    pub saturation: f32,
    pub fps: u32,
    pub display: DisplayPolicy,
    /// Desk position per device id; missing devices use `layout::default_placement`.
    pub desk: HashMap<String, Placement>,
    /// Run OpenRGB once at start to put non-Razer RGB (motherboard, GPU, RAM) on a hardware rainbow.
    pub openrgb_hardware_rainbow: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayPolicy {
    /// Fade lighting out when Windows turns the display off.
    pub off_when_display_off: bool,
    /// Brightness multiplier while Windows has dimmed the display.
    pub dim_level: f32,
    pub fade_s: f32,
}

impl Default for DisplayPolicy {
    fn default() -> Self {
        DisplayPolicy { off_when_display_off: true, dim_level: 0.35, fade_s: 1.2 }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            effect: Effect::default(),
            brightness: 1.0,
            saturation: 1.0,
            fps: 30,
            display: DisplayPolicy::default(),
            desk: HashMap::new(),
            openrgb_hardware_rainbow: true,
        }
    }
}

impl Config {
    pub fn dir() -> PathBuf {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join("uncoil")
    }

    pub fn path() -> PathBuf {
        Self::dir().join("config.json")
    }

    /// Load, falling back to defaults on missing/invalid file.
    pub fn load() -> Config {
        std::fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(Self::dir())?;
        let tmp = Self::path().with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).expect("config serialises"))?;
        std::fs::rename(tmp, Self::path())
    }
}

/// Daemon status, written by uncoild to `%LOCALAPPDATA%\uncoil\status.json` for the GUI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Status {
    pub pid: u32,
    pub version: String,
    pub started_unix: u64,
    pub updated_unix: u64,
    pub display: String,
    pub level: f32,
    pub devices: Vec<DeviceStatus>,
    /// The daemon's own footprint, measured by itself: private memory, CPU as % of one core
    /// (averaged over the last status interval), and the size of its executable.
    #[serde(default)]
    pub memory_bytes: u64,
    #[serde(default)]
    pub cpu_percent: f32,
    #[serde(default)]
    pub exe_bytes: u64,
    /// Razer devices (vendor 0x1532) on the bus that no device definition knows.
    #[serde(default)]
    pub unknown_devices: Vec<UnknownDevice>,
}

/// A Razer device uncoil has no definition for: its product id and the HID interface numbers it shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownDevice {
    pub product_id: u16,
    pub interfaces: Vec<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub id: String,
    pub name: String,
    pub product_id: u16,
    pub connection: String,
    pub fps: f32,
    pub busy_retries: u64,
    pub errors: u64,
}

impl Status {
    pub fn path() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join("uncoil").join("status.json")
    }
}
