//! uncoil desktop GUI. The window is a thin view over `uncoil-core`: it edits the shared config (which
//! the daemon hot-reloads), previews the effect with the exact engine code, and reads the daemon's status.

// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use uncoil_core::config::{Config, Status};
use uncoil_core::device::{self, DeviceDef, Kind};
use uncoil_core::effect::{Inputs, Press};
use uncoil_core::ipc::{self, Client, Command};
use uncoil_core::layout::{self, PlacedDevice};

/// The daemon rewrites status.json continuously; older than this means it is not running.
const STATUS_STALE_S: u64 = 10;

fn defs() -> &'static [DeviceDef] {
    static DEFS: OnceLock<Vec<DeviceDef>> = OnceLock::new();
    DEFS.get_or_init(|| device::load_all(None))
}

/// The desk: supported devices, devices the config places, and the `connected` ones (ids from the daemon's
/// `devices`), auto-placed next to their kind when the config does not place them (`layout::arrange`).
fn placed(config: &Config, connected: &[String]) -> Vec<(Kind, PlacedDevice)> {
    let shown = layout::desk_devices(defs(), &config.desk, |id| connected.iter().any(|c| c == id));
    layout::arrange(&shown, &config.desk).into_iter().map(|(def, _, p)| (def.kind, p)).collect()
}

/// A placed device plus its kind, so the preview can draw a mat differently from a mouse.
#[derive(Serialize)]
struct DeskDevice {
    kind: Kind,
    #[serde(flatten)]
    placed: PlacedDevice,
}

#[tauri::command]
fn get_config() -> Config {
    Config::load()
}

#[tauri::command]
fn save_config(config: Config) -> Result<(), String> {
    config.save().map_err(|e| format!("could not save {}: {e}", Config::path().display()))
}

/// `connected` (optional): ids of connected devices, so experimental devices with a layout join the desk.
#[tauri::command]
fn get_desk(config: Config, connected: Option<Vec<String>>) -> Vec<DeskDevice> {
    placed(&config, &connected.unwrap_or_default())
        .into_iter()
        .map(|(kind, placed)| DeskDevice { kind, placed })
        .collect()
}

/// Hex colour of every shape of every device (same order as `get_desk`) at time `t`, computed by the
/// same effect code the daemon runs. `presses` simulate key presses (desk position + time on the same clock
/// as `t`) for reactive and ripple; `audio` simulates the audio level 0..1 for the audio meter.
#[tauri::command]
fn preview_frame(
    config: Config,
    t: f32,
    presses: Option<Vec<Press>>,
    audio: Option<f32>,
    connected: Option<Vec<String>>,
) -> Vec<Vec<String>> {
    let desk = placed(&config, &connected.unwrap_or_default());
    let presses = presses.unwrap_or_default();
    let inputs = Inputs {
        presses: &presses,
        audio: audio.unwrap_or(0.0),
        bounds: layout::desk_bounds(desk.iter().map(|(_, d)| d)),
        keyboard_center: desk.iter().find(|(k, _)| *k == Kind::Keyboard).map(|(_, d)| d.center()),
    };
    let frame = config.effect.at_with(t, config.saturation, config.brightness, &inputs);
    desk.iter()
        .map(|(_, dev)| dev.shapes.iter().map(|s| frame.color_led(&dev.id, &s.name, s.x, s.y).to_hex()).collect())
        .collect()
}

#[tauri::command]
fn get_status() -> Option<Status> {
    let text = std::fs::read_to_string(Status::path()).ok()?;
    let status: Status = serde_json::from_str(&text).ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    (now.saturating_sub(status.updated_unix) <= STATUS_STALE_S).then_some(status)
}

/// Forward one typed command to uncoild's control pipe (the same surface the `uncoil` CLI uses).
/// Errors from failing to reach the daemon start with `unreachable:` so the UI can explain them; errors the
/// daemon marks with a code (`check_failed`, `left_click_guard`, `not_supported`) start with that code and
/// `: `, e.g. `check_failed: uncoil couldn't confirm …`.
/// `UNCOIL_PIPE` points the app at another daemon, e.g. `uncoild --fake` while developing.
#[tauri::command]
async fn daemon(
    device: Option<String>,
    cmd: String,
    args: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let args = args.map(|a| a.to_string());
        let command = Command::from_parts(&cmd, args.as_deref()).map_err(|e| e.to_string())?;
        let pipe = std::env::var("UNCOIL_PIPE").unwrap_or_else(|_| ipc::PIPE_NAME.to_string());
        let mut client = Client::connect_to(&pipe).map_err(|e| format!("unreachable: {e}"))?;
        let response = client.call(device.as_deref(), &command).map_err(|e| format!("unreachable: {e}"))?;
        if !response.ok {
            return Err(error_text(response.code.as_deref(), response.error.as_deref()));
        }
        response.into_result::<serde_json::Value>().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A daemon error as the UI reads it: `code: message` when the daemon gave a code, else the message.
fn error_text(code: Option<&str>, error: Option<&str>) -> String {
    let message = error.unwrap_or("daemon reported an error");
    match code {
        Some(c) => format!("{c}: {message}"),
        None => message.to_string(),
    }
}

/// Size the window to the monitor it opens on: about 56% x 65% of it (1440x900 on a 2560x1440 screen),
/// never below the minimum and never past 1600x1000, then centre and show it. The window starts hidden so
/// it never flashes at the config's fallback size.
fn size_to_monitor(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.current_monitor() {
        let scale = monitor.scale_factor();
        let screen = monitor.size().to_logical::<f64>(scale);
        let width = (screen.width * 0.5625).clamp(1024.0, 1600.0).min(screen.width);
        let height = (screen.height * 0.625).clamp(680.0, 1000.0).min(screen.height - 48.0);
        let _ = window.set_size(tauri::LogicalSize::new(width.round(), height.round()));
        let _ = window.center();
    }
    let _ = window.show();
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                size_to_monitor(&window);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_config, save_config, get_desk, preview_frame, get_status, daemon])
        .run(tauri::generate_context!())
        .expect("error while running uncoil");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The browser-only mock (`src/lib/mock/desk.json`, used by `pnpm dev` outside Tauri) must match the
    /// real default desk. Regenerate with `UNCOIL_UPDATE_MOCK=1 cargo test -p uncoil-gui`.
    #[test]
    fn mock_desk_matches_default_desk() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/mock/desk.json");
        let real = serde_json::to_string_pretty(&get_desk(Config::default(), None)).unwrap() + "\n";
        if std::env::var_os("UNCOIL_UPDATE_MOCK").is_some() {
            std::fs::write(&path, &real).unwrap();
        }
        let mock = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
        assert!(mock == real, "{} is stale; rerun with UNCOIL_UPDATE_MOCK=1", path.display());
    }

    /// End to end through the control pipe. Needs `uncoild --fake` (built with `--features fake`) running:
    /// `UNCOIL_PIPE=\\.\pipe\uncoil-fake cargo test -p uncoil-gui -- --ignored`
    #[test]
    #[ignore]
    fn daemon_bridge_against_fake_daemon() {
        let run = |device: Option<&str>, cmd: &str, args: Option<serde_json::Value>| {
            tauri::async_runtime::block_on(daemon(device.map(String::from), cmd.into(), args))
        };
        let devices = run(None, "devices", None).unwrap();
        assert!(devices.as_array().is_some_and(|d| !d.is_empty()), "{devices}");
        let key = run(Some("keyboard"), "keymap.get", Some(serde_json::json!({ "key": "P", "layer": "fn" }))).unwrap();
        assert_eq!(key["name"], "P");
        let refused =
            run(Some("keyboard"), "keymap.set", Some(serde_json::json!({ "key": "P", "function": "key F5" })));
        assert!(refused.unwrap_err().contains("write=true"), "onboard writes need write=true");
        assert!(run(None, "no.such.command", None).is_err());
    }

    #[test]
    fn coded_errors_become_a_prefix() {
        assert_eq!(
            error_text(Some("left_click_guard"), Some("This would leave no button that left-clicks.")),
            "left_click_guard: This would leave no button that left-clicks."
        );
        assert_eq!(error_text(None, Some("boom")), "boom");
    }

    #[test]
    fn connected_experimental_devices_join_the_desk() {
        // any experimental device with a layout from devices/experimental/
        let Some(extra) = defs().iter().find(|d| d.is_experimental() && d.lit().is_some()) else { return };
        let base = get_desk(Config::default(), None);
        assert!(!base.iter().any(|d| d.placed.id == extra.id));
        let with = get_desk(Config::default(), Some(vec![extra.id.clone()]));
        assert_eq!(with.len(), base.len() + 1);
        for (a, b) in base.iter().zip(with.iter().filter(|d| d.placed.id != extra.id)) {
            assert_eq!((a.placed.x, a.placed.y), (b.placed.x, b.placed.y), "{} moved", a.placed.id);
        }
    }

    #[test]
    fn preview_frame_matches_desk_shape() {
        let config = Config::default();
        let desk = get_desk(config.clone(), None);
        let frame = preview_frame(config, 1.5, None, None, None);
        assert_eq!(frame.len(), desk.len());
        for (colors, dev) in frame.iter().zip(&desk) {
            assert_eq!(colors.len(), dev.placed.shapes.len());
            assert!(colors.iter().all(|c| c.len() == 7 && c.starts_with('#')));
        }
    }

    fn with_effect(json: &str) -> Config {
        Config { effect: serde_json::from_str(json).unwrap(), ..Config::default() }
    }

    /// Colour of one named shape of one device in a preview.
    fn shape_color(frame: &[Vec<String>], desk: &[DeskDevice], device: &str, shape: &str) -> String {
        let (i, dev) = desk.iter().enumerate().find(|(_, d)| d.placed.id == device).unwrap();
        let j = dev.placed.shapes.iter().position(|s| s.name == shape).unwrap();
        frame[i][j].clone()
    }

    #[test]
    fn preview_applies_studio_masks_per_device_and_key() {
        let config = with_effect(
            r#"{"kind":"studio","layers":[
                {"name":"base","enabled":true,"opacity":1,"effect":{"kind":"static","color":[0,0,255]},"mask":{"kind":"all"}},
                {"name":"mouse","enabled":true,"opacity":1,"effect":{"kind":"static","color":[255,0,0]},
                 "mask":{"kind":"devices","ids":["razer-basilisk-v3-pro"]}},
                {"name":"wasd","enabled":true,"opacity":1,"effect":{"kind":"static","color":[0,255,0]},
                 "mask":{"kind":"keys","device":"razer-blackwidow-v4-pro-75","shapes":["W","Left Shift"]}}
            ]}"#,
        );
        let desk = get_desk(config.clone(), None);
        let frame = preview_frame(config, 0.0, None, None, None);
        let kb = "razer-blackwidow-v4-pro-75";
        assert_eq!(shape_color(&frame, &desk, kb, "W"), "#00ff00");
        assert_eq!(shape_color(&frame, &desk, kb, "Left Shift"), "#00ff00");
        assert_eq!(shape_color(&frame, &desk, kb, "Q"), "#0000ff");
        assert_eq!(shape_color(&frame, &desk, "razer-basilisk-v3-pro", "Logo"), "#ff0000");
        assert_eq!(shape_color(&frame, &desk, "razer-goliathus-chroma-extended", "Edge"), "#0000ff");
    }

    #[test]
    fn preview_uses_presses_and_audio() {
        let config = with_effect(r#"{"kind":"reactive","color":[255,255,255],"fade_s":1}"#);
        let desk = get_desk(config.clone(), None);
        let kb = "razer-blackwidow-v4-pro-75";
        let keyboard = desk.iter().find(|d| d.placed.id == kb).unwrap();
        let w = keyboard.placed.shapes.iter().find(|s| s.name == "W").unwrap();
        let press = vec![Press { x: w.x, y: w.y, t: 2.0 }];
        let lit = preview_frame(config.clone(), 2.0, Some(press), None, None);
        assert_eq!(shape_color(&lit, &desk, kb, "W"), "#ffffff");
        assert_eq!(shape_color(&lit, &desk, kb, "Q"), "#000000");
        let idle = preview_frame(config, 2.0, None, None, None);
        assert_eq!(shape_color(&idle, &desk, kb, "W"), "#000000");

        let meter = with_effect(r#"{"kind":"audio_meter","sensitivity":1}"#);
        let loud = preview_frame(meter.clone(), 0.0, None, Some(1.0), None);
        let quiet = preview_frame(meter, 0.0, None, Some(0.0), None);
        assert_ne!(shape_color(&loud, &desk, kb, "Escape"), "#000000");
        assert_eq!(shape_color(&quiet, &desk, kb, "Escape"), "#000000");
    }
}
