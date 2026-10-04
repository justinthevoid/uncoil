//! Battery notifications: every 10 minutes while the app runs (window or tray), read the battery of each
//! device with the `power` feature over the control pipe and notify once per threshold per discharge cycle.

use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;
use uncoil_core::features::power::PowerState;
use uncoil_core::features::Feature;
use uncoil_core::ipc::{Client, Command, DeviceInfo};

use crate::settings::AppSettings;
use crate::AppState;

/// How often the battery is read.
pub const INTERVAL: Duration = Duration::from_secs(10 * 60);
/// The first read, a little after the app starts (the engine may still be finding devices).
const FIRST_CHECK: Duration = Duration::from_secs(60);
/// The second, fixed warning.
pub const CRITICAL_PCT: u8 = 10;
/// A level this far above the threshold starts a new cycle even if charging was never seen.
const REARM_MARGIN: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alert {
    Low(u8),
    Critical(u8),
    Charged,
}

/// What has been said about one device in the current cycle.
#[derive(Debug, Default)]
pub struct DeviceWatch {
    low: bool,
    critical: bool,
    full: bool,
}

impl DeviceWatch {
    /// The notification for this reading, if any. `charging` is None when the device doesn't say.
    pub fn update(&mut self, pct: u8, charging: Option<bool>, threshold: u8) -> Option<Alert> {
        if charging == Some(true) || pct > threshold.saturating_add(REARM_MARGIN) {
            // Charging (or charged without us seeing it): the next discharge is a new cycle.
            self.low = false;
            self.critical = false;
        }
        if charging == Some(true) {
            if pct >= 100 && !self.full {
                self.full = true;
                return Some(Alert::Charged);
            }
            return None;
        }
        if charging == Some(false) {
            self.full = false;
        }
        if pct <= CRITICAL_PCT && !self.critical {
            self.critical = true;
            self.low = true;
            return Some(Alert::Critical(pct));
        }
        if pct <= threshold && !self.low {
            self.low = true;
            return Some(Alert::Low(pct));
        }
        None
    }
}

/// Every device's watch, by device id.
#[derive(Debug, Default)]
pub struct BatteryWatch {
    devices: HashMap<String, DeviceWatch>,
}

impl BatteryWatch {
    /// `readings`: (id, name, power state) for each device with the `power` feature.
    pub fn check(&mut self, readings: &[(String, String, PowerState)], threshold: u8) -> Vec<(String, Alert)> {
        let mut out = Vec::new();
        for (id, name, p) in readings {
            let Some(pct) = p.battery_pct else { continue };
            if let Some(a) = self.devices.entry(id.clone()).or_default().update(pct, p.charging, threshold) {
                out.push((name.clone(), a));
            }
        }
        out
    }
}

/// "Razer Basilisk V3 Pro" → "Basilisk V3 Pro".
fn short_name(name: &str) -> &str {
    name.strip_prefix("Razer ").unwrap_or(name)
}

/// Title and body of a notification.
pub fn words(name: &str, alert: Alert) -> (String, String) {
    let name = short_name(name);
    match alert {
        Alert::Low(p) => (format!("{name} battery is low"), format!("{p}% left.")),
        Alert::Critical(p) => (format!("{name} battery is very low"), format!("{p}% left. Charge it soon.")),
        Alert::Charged => (format!("{name} is charged"), "The battery is full.".into()),
    }
}

fn call<T: serde::de::DeserializeOwned>(device: Option<&str>, cmd: &str) -> anyhow::Result<T> {
    let command = Command::from_parts(cmd, None)?;
    let mut client = Client::connect_to(&crate::pipe_name())?;
    client.call(device, &command)?.into_result()
}

/// The battery of every connected device that has one. Errors (engine not running) give an empty list.
fn readings() -> Vec<(String, String, PowerState)> {
    let Ok(devices) = call::<Vec<DeviceInfo>>(None, "devices") else { return Vec::new() };
    devices
        .into_iter()
        .filter(|d| d.features.contains(&Feature::Power))
        .filter_map(|d| call::<PowerState>(Some(&d.id), "power.get").ok().map(|p| (d.id, d.name, p)))
        .collect()
}

/// Runs for the life of the app on its own thread.
pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    std::thread::Builder::new()
        .name("battery".into())
        .spawn(move || {
            let mut watch = BatteryWatch::default();
            std::thread::sleep(FIRST_CHECK);
            loop {
                let settings: AppSettings =
                    app.state::<AppState>().settings.lock().map(|s| s.clone()).unwrap_or_default();
                if settings.battery_notifications {
                    for (name, alert) in watch.check(&readings(), settings.battery_threshold) {
                        let (title, body) = words(&name, alert);
                        let _ = app.notification().builder().title(title).body(body).show();
                    }
                }
                std::thread::sleep(INTERVAL);
            }
        })
        .expect("battery thread starts");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(watch: &mut DeviceWatch, steps: &[(u8, Option<bool>)]) -> Vec<Option<Alert>> {
        steps.iter().map(|&(p, c)| watch.update(p, c, 20)).collect()
    }

    #[test]
    fn warns_once_per_threshold_per_discharge() {
        let mut w = DeviceWatch::default();
        let got = run(
            &mut w,
            &[(60, Some(false)), (20, Some(false)), (18, Some(false)), (10, Some(false)), (7, Some(false))],
        );
        assert_eq!(got, vec![None, Some(Alert::Low(20)), None, Some(Alert::Critical(10)), None]);
        // Charging starts a new cycle and says "charged" once at 100 %.
        let got = run(
            &mut w,
            &[(40, Some(true)), (100, Some(true)), (100, Some(true)), (99, Some(false)), (19, Some(false))],
        );
        assert_eq!(got, vec![None, Some(Alert::Charged), None, None, Some(Alert::Low(19))]);
    }

    #[test]
    fn first_reading_already_critical_gives_one_notification() {
        let mut w = DeviceWatch::default();
        assert_eq!(run(&mut w, &[(8, Some(false)), (8, Some(false))]), vec![Some(Alert::Critical(8)), None]);
    }

    #[test]
    fn unknown_charging_rearms_when_the_level_rises() {
        let mut w = DeviceWatch::default();
        let got = run(&mut w, &[(19, None), (22, None), (26, None), (20, None)]);
        assert_eq!(got, vec![Some(Alert::Low(19)), None, None, Some(Alert::Low(20))]);
        assert_eq!(w.update(100, None, 20), None, "no charged notice without knowing it charged");
    }

    #[test]
    fn devices_are_watched_separately_and_missing_levels_are_skipped() {
        let p = |pct: Option<u8>| PowerState { battery_pct: pct, charging: Some(false), ..PowerState::default() };
        let mut watch = BatteryWatch::default();
        let readings = vec![
            ("a".to_string(), "Razer Basilisk V3 Pro".to_string(), p(Some(15))),
            ("b".to_string(), "Razer DeathAdder V3 Pro".to_string(), p(None)),
        ];
        assert_eq!(watch.check(&readings, 20), vec![("Razer Basilisk V3 Pro".to_string(), Alert::Low(15))]);
        assert!(watch.check(&readings, 20).is_empty());
    }

    #[test]
    fn notification_words_are_plain() {
        assert_eq!(
            words("Razer Basilisk V3 Pro", Alert::Low(18)),
            ("Basilisk V3 Pro battery is low".into(), "18% left.".into())
        );
        assert_eq!(words("Razer Basilisk V3 Pro", Alert::Charged).0, "Basilisk V3 Pro is charged");
    }
}
