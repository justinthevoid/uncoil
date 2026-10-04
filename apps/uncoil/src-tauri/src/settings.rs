//! The app's own preferences (tray, start with Windows, battery notifications). They live next to the
//! engine's config in `%APPDATA%\uncoil\app.json`, but the engine never reads them.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uncoil_core::config::Config;

/// Lowest and highest battery warning level the app offers; the second warning is fixed at 10 %.
pub const THRESHOLD_RANGE: (u8, u8) = (15, 50);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// Closing the window hides it to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Start hidden in the tray when Windows starts (autostart entry with `--tray`).
    pub start_in_tray: bool,
    /// Notify when a wireless device's battery runs low or finishes charging.
    pub battery_notifications: bool,
    /// Percent for the first low battery notification.
    pub battery_threshold: u8,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings { close_to_tray: false, start_in_tray: false, battery_notifications: true, battery_threshold: 20 }
    }
}

impl AppSettings {
    pub fn path() -> PathBuf {
        Config::dir().join("app.json")
    }

    /// The saved settings, or the defaults when there is no file or it can't be read.
    pub fn load() -> AppSettings {
        Self::load_from(&Self::path())
    }

    fn load_from(path: &Path) -> AppSettings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<AppSettings>(&t).ok())
            .unwrap_or_default()
            .sanitized()
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&Self::path())
    }

    fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).expect("settings serialise"))?;
        std::fs::rename(tmp, path)
    }

    /// The threshold kept inside the offered range.
    pub fn sanitized(mut self) -> AppSettings {
        self.battery_threshold = self.battery_threshold.clamp(THRESHOLD_RANGE.0, THRESHOLD_RANGE.1);
        self
    }

    /// Closing the window hides it: asked for directly, or implied by starting in the tray.
    pub fn hide_on_close(&self) -> bool {
        self.close_to_tray || self.start_in_tray
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_missing_fields() {
        let d = AppSettings::default();
        assert!(!d.close_to_tray && !d.start_in_tray && d.battery_notifications && d.battery_threshold == 20);
        let partial: AppSettings = serde_json::from_str(r#"{"close_to_tray": true}"#).unwrap();
        assert_eq!(partial, AppSettings { close_to_tray: true, ..AppSettings::default() });
        assert!(partial.hide_on_close());
    }

    #[test]
    fn threshold_is_kept_in_range() {
        let low = AppSettings { battery_threshold: 3, ..AppSettings::default() }.sanitized();
        assert_eq!(low.battery_threshold, 15);
        let high = AppSettings { battery_threshold: 90, ..AppSettings::default() }.sanitized();
        assert_eq!(high.battery_threshold, 50);
    }

    #[test]
    fn saves_and_loads_back() {
        let dir = std::env::temp_dir().join(format!("uncoil-gui-settings-{}", std::process::id()));
        let path = dir.join("app.json");
        assert_eq!(AppSettings::load_from(&path), AppSettings::default(), "no file: defaults");
        let s = AppSettings { start_in_tray: true, battery_threshold: 30, ..AppSettings::default() };
        s.save_to(&path).unwrap();
        assert_eq!(AppSettings::load_from(&path), s);
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(AppSettings::load_from(&path), AppSettings::default(), "broken file: defaults");
        let _ = std::fs::remove_dir_all(dir);
    }
}
