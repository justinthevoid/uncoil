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
    /// Hand non-Razer RGB (motherboard, GPU, RAM) to OpenRGB once at logon: OpenRGB puts each device in
    /// `openrgb.devices` on its own hardware mode and exits. Off by default. Needs OpenRGB installed and, for
    /// RAM over SMBus, the elevated `uncoil-openrgb` task (`scripts\install-task.ps1 -OpenRgb`).
    pub openrgb_hardware_rainbow: bool,
    /// What the OpenRGB hand-off sets. Empty by default: there is no built-in device list.
    pub openrgb: OpenRgb,
}

/// `openrgb` in `config.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenRgb {
    pub devices: Vec<OpenRgbDevice>,
}

/// One OpenRGB device and the hardware mode to put it in, e.g.
/// `{"match": "GeForce", "mode": "wave"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenRgbDevice {
    /// Part of the device name as OpenRGB lists it (passed to `OpenRGB.exe -d`).
    #[serde(rename = "match")]
    pub name: String,
    /// OpenRGB mode name (passed to `-m`).
    pub mode: String,
    /// RAM on the SMBus: skipped while Corsair iCUE runs, because both would write the same bus.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ram: bool,
}

impl OpenRgbDevice {
    /// Why this entry may not be passed to OpenRGB, if it may not. The hand-off runs elevated and this
    /// comes from a user-writable file, so both strings are plain names: 1 to 64 letters, digits, spaces
    /// and `-_.()+#&:/`, not starting with `-` (an OpenRGB option) or a space.
    pub fn problem(&self) -> Option<String> {
        for (what, s) in [("match", &self.name), ("mode", &self.mode)] {
            let ok_char = |c: char| c.is_ascii_alphanumeric() || " -_.()+#&:/".contains(c);
            if s.is_empty() || s.len() > 64 || s.starts_with(['-', ' ']) || s.ends_with(' ') || !s.chars().all(ok_char)
            {
                return Some(format!("openrgb.devices: {what} {s:?} is not a plain name"));
            }
        }
        None
    }
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
            openrgb_hardware_rainbow: false,
            openrgb: OpenRgb::default(),
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

    /// Load, falling back to defaults when there is no file. A file that cannot be read or parsed also falls
    /// back to defaults, and the second value says why (the daemon logs it).
    pub fn load_reporting() -> (Config, Option<String>) {
        Self::load_from(&Self::path())
    }

    /// [`Config::load_reporting`], with any problem printed to stderr.
    pub fn load() -> Config {
        let (config, problem) = Self::load_reporting();
        if let Some(p) = problem {
            eprintln!("uncoil: {p}");
        }
        config
    }

    fn load_from(path: &std::path::Path) -> (Config, Option<String>) {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Config::default(), None),
            Err(e) => return (Config::default(), Some(format!("{} not read ({e}); using defaults", path.display()))),
        };
        match serde_json::from_str(&text) {
            Ok(c) => (c, None),
            Err(e) => (Config::default(), Some(format!("{} is not valid ({e}); using defaults", path.display()))),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_config_falls_back_to_defaults_and_says_why() {
        let dir = std::env::temp_dir().join(format!("uncoil-test-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let _ = std::fs::remove_file(&path);
        assert_eq!(Config::load_from(&path), (Config::default(), None), "no file: defaults, nothing to say");
        std::fs::write(&path, r#"{"fps": 45}"#).unwrap();
        assert_eq!(Config::load_from(&path), (Config { fps: 45, ..Config::default() }, None));
        std::fs::write(&path, r#"{"fps": "fast"}"#).unwrap();
        let (c, problem) = Config::load_from(&path);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(c, Config::default());
        let p = problem.unwrap();
        assert!(p.contains("config.json is not valid") && p.ends_with("using defaults"), "{p}");
    }

    #[test]
    fn openrgb_is_off_with_no_built_in_devices() {
        let c = Config::default();
        assert!(!c.openrgb_hardware_rainbow);
        assert!(c.openrgb.devices.is_empty());
        // configs written before the device list existed still load (the old flag alone does nothing)
        let old: Config = serde_json::from_str(r#"{"openrgb_hardware_rainbow": true}"#).unwrap();
        assert!(old.openrgb_hardware_rainbow && old.openrgb.devices.is_empty());
        let new: Config = serde_json::from_str(
            r#"{"openrgb_hardware_rainbow": true, "openrgb": {"devices": [
                {"match": "GeForce", "mode": "wave"}, {"match": "Vengeance", "mode": "rainbow wave", "ram": true}]}}"#,
        )
        .unwrap();
        assert_eq!(
            new.openrgb.devices[1],
            OpenRgbDevice { name: "Vengeance".into(), mode: "rainbow wave".into(), ram: true }
        );
        let back = serde_json::to_string(&new.openrgb.devices[0]).unwrap();
        assert_eq!(back, r#"{"match":"GeForce","mode":"wave"}"#);
    }

    #[test]
    fn openrgb_names_cannot_become_options() {
        let d = |name: &str, mode: &str| OpenRgbDevice { name: name.into(), mode: mode.into(), ram: false };
        assert_eq!(d("ASUS ROG STRIX B550-F GAMING (WI-FI)", "Rainbow Wave").problem(), None);
        assert_eq!(d("GeForce", "wave").problem(), None);
        let long = "x".repeat(65);
        for (n, m) in [
            ("--server", "wave"),
            ("GeForce", "-p"),
            ("", "wave"),
            (" GeForce", "wave"),
            ("GeForce\" --config x", "wave"),
            ("GeForce\n", "wave"),
            ("GeForce", long.as_str()),
        ] {
            assert!(d(n, m).problem().is_some(), "{n:?} / {m:?}");
        }
    }
}
