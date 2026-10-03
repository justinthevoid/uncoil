//! uncoil desktop GUI. The window is a thin view over `uncoil-core`: it edits the shared config (which
//! the daemon hot-reloads), previews the effect with the exact engine code, and reads the daemon's status.

// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use uncoil_core::config::{Config, Status};
use uncoil_core::device::{self, DeviceDef, Kind};
use uncoil_core::ipc::{self, Client, Command};
use uncoil_core::layout::{self, PlacedDevice};

/// The daemon rewrites status.json continuously; older than this means it is not running.
const STATUS_STALE_S: u64 = 10;

fn defs() -> &'static [DeviceDef] {
    static DEFS: OnceLock<Vec<DeviceDef>> = OnceLock::new();
    DEFS.get_or_init(|| device::load_all(None))
}

fn placed(config: &Config) -> Vec<(Kind, PlacedDevice)> {
    defs()
        .iter()
        .map(|def| {
            let at = config.desk.get(&def.id).copied().unwrap_or_else(|| layout::default_placement(def));
            (def.kind, layout::place(def, at))
        })
        .collect()
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

#[tauri::command]
fn get_desk(config: Config) -> Vec<DeskDevice> {
    placed(&config).into_iter().map(|(kind, placed)| DeskDevice { kind, placed }).collect()
}

/// Hex colour of every shape of every device (same order as `get_desk`) at time `t`, computed by the
/// same effect code the daemon runs.
#[tauri::command]
fn preview_frame(config: Config, t: f32) -> Vec<Vec<String>> {
    let frame = config.effect.at(t, config.saturation, config.brightness);
    placed(&config)
        .iter()
        .map(|(_, dev)| dev.shapes.iter().map(|s| frame.color_at(s.x, s.y).to_hex()).collect())
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
/// Errors from failing to reach the daemon start with `unreachable:` so the UI can explain them.
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
        response.into_result::<serde_json::Value>().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
        let real = serde_json::to_string_pretty(&get_desk(Config::default())).unwrap() + "\n";
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
    fn preview_frame_matches_desk_shape() {
        let config = Config::default();
        let desk = get_desk(config.clone());
        let frame = preview_frame(config, 1.5);
        assert_eq!(frame.len(), desk.len());
        for (colors, dev) in frame.iter().zip(&desk) {
            assert_eq!(colors.len(), dev.placed.shapes.len());
            assert!(colors.iter().all(|c| c.len() == 7 && c.starts_with('#')));
        }
    }
}
