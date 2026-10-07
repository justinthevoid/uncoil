//! uncoil desktop GUI. The window is a thin view over `uncoil-core`: it edits the shared config (which
//! the daemon hot-reloads), previews the effect with the exact engine code, and reads the daemon's status.
//! A tray icon switches the effect, and a background thread sends battery notifications (tray.rs, battery.rs);
//! the app's own preferences live in settings.rs.

// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod battery;
mod settings;
mod tray;

use serde::Serialize;
use settings::AppSettings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use uncoil_core::config::{Config, Status};
use uncoil_core::device::{self, DeviceDef, Kind};
use uncoil_core::effect::Press;
use uncoil_core::ipc::{self, Client, Command, OpenRgbDeviceStatus};
use uncoil_core::layout::{self, Desk, PlacedDevice};

/// The daemon rewrites status.json continuously; older than this means it is not running.
const STATUS_STALE_S: u64 = 10;

/// The same devices the daemon loads: built-ins plus the user's device folder (tests: built-ins only).
fn defs() -> &'static [DeviceDef] {
    static DEFS: OnceLock<Vec<DeviceDef>> = OnceLock::new();
    DEFS.get_or_init(|| if cfg!(test) { device::builtin() } else { device::load_installed().0 })
}

/// The desk the daemon renders: supported devices, devices the config places, the `connected` ones (ids
/// from the daemon's `devices`), auto-placed next to their kind when the config does not place them, and the
/// OpenRGB devices the daemon drives (`external`: `status.openrgb.devices`, whose zones give their LEDs).
fn desk(config: &Config, connected: &[String], external: &[OpenRgbDeviceStatus]) -> Desk {
    let external: Vec<DeviceDef> = external.iter().filter(|d| layout::is_external(&d.id)).map(|d| d.def()).collect();
    Desk::new(defs().iter().chain(&external), &config.desk, |id| connected.iter().any(|c| c == id))
}

/// A placed device plus its kind, so the preview can draw a mat differently from a mouse.
#[derive(Serialize)]
struct DeskDevice {
    kind: Kind,
    #[serde(flatten)]
    placed: PlacedDevice,
}

/// config.json as the window last loaded or saved it, so a save never overwrites a change made outside the
/// window (a hand edit, the CLI, the tray). `None` until the window first loads it.
#[derive(Default)]
struct ConfigSeen(Mutex<Option<Option<String>>>);

fn config_text() -> Option<String> {
    std::fs::read_to_string(Config::path()).ok()
}

/// The config, or why it can't be used: a config.json that doesn't parse is reported, not replaced with
/// defaults the next save would write over the user's file.
#[tauri::command]
fn get_config(seen: tauri::State<ConfigSeen>) -> Result<Config, String> {
    let text = config_text();
    let (config, problem) = Config::load_reporting();
    if let Some(p) = problem {
        return Err(format!("{p}. Fix the file (or delete it to start over), then reopen the window."));
    }
    if let Ok(mut s) = seen.0.lock() {
        *s = Some(text);
    }
    Ok(config)
}

#[tauri::command]
fn save_config(app: tauri::AppHandle, seen: tauri::State<ConfigSeen>, config: Config) -> Result<(), String> {
    use tauri::{Emitter, Manager};
    let mut seen = seen.0.lock().map_err(|_| "settings are locked; try again".to_string())?;
    let now = config_text();
    if seen.as_ref().is_some_and(|s| *s != now) {
        // changed outside the window since it loaded: reload instead of overwriting
        let _ = app.emit("config-changed", ());
        return Err("config.json was changed outside the app, so the window reloaded it; make the change again.".into());
    }
    // The tray remembers the effect being left, so its menu can switch back to the same settings.
    let old = Config::load_reporting().0.effect;
    if tray::kind_of(&old) != tray::kind_of(&config.effect) {
        app.state::<tray::TrayState>().remember(&old);
    }
    config.save().map_err(|e| format!("could not save {}: {e}", Config::path().display()))?;
    *seen = Some(config_text());
    drop(seen);
    tray::refresh(&app);
    Ok(())
}

/// App-wide state the tray, the window and the battery thread share.
pub struct AppState {
    pub settings: Mutex<AppSettings>,
}

#[tauri::command]
fn get_app_settings(state: tauri::State<AppState>) -> AppSettings {
    state.settings.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Save the app's preferences, and add or remove the start-with-Windows entry to match.
#[tauri::command]
fn save_app_settings(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let settings = settings.sanitized();
    // the entry first, so app.json never claims a start-with-Windows entry that isn't there
    let autostart = app.autolaunch();
    if autostart.is_enabled().ok() != Some(settings.start_in_tray) {
        let r = if settings.start_in_tray { autostart.enable() } else { autostart.disable() };
        r.map_err(|e| format!("could not change the start-with-Windows entry: {e}"))?;
    }
    settings.save().map_err(|e| format!("could not save {}: {e}", AppSettings::path().display()))?;
    if let Ok(mut s) = state.settings.lock() {
        *s = settings;
    }
    Ok(())
}

/// `connected` (optional): ids of connected devices, so experimental devices with a layout join the desk.
/// `external` (optional): `status.openrgb.devices`, so the PC's OpenRGB devices join it too.
#[tauri::command]
fn get_desk(
    config: Config,
    connected: Option<Vec<String>>,
    external: Option<Vec<OpenRgbDeviceStatus>>,
) -> Vec<DeskDevice> {
    desk(&config, &connected.unwrap_or_default(), &external.unwrap_or_default())
        .devices
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
    external: Option<Vec<OpenRgbDeviceStatus>>,
) -> Vec<Vec<String>> {
    let desk = desk(&config, &connected.unwrap_or_default(), &external.unwrap_or_default());
    let presses = presses.unwrap_or_default();
    let frame =
        config.effect.at_with(t, config.saturation, config.brightness, &desk.inputs(&presses, audio.unwrap_or(0.0)));
    desk.devices
        .iter()
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

/// A failed `daemon` call, as the UI gets it: plain words, the daemon's code if it gave one
/// (`check_failed`, `left_click_guard`, `not_supported`), and whether uncoild could not be reached at all.
#[derive(Debug, Serialize, PartialEq)]
struct DaemonFailure {
    message: String,
    code: Option<&'static str>,
    unreachable: bool,
}

impl DaemonFailure {
    fn new(e: anyhow::Error, unreachable: bool) -> DaemonFailure {
        let code = e.downcast_ref::<ipc::CodedError>().map(|c| c.code);
        DaemonFailure { message: format!("{e:#}"), code, unreachable }
    }
}

/// The control pipe: `UNCOIL_PIPE` points the app at another daemon, e.g. `uncoild --fake` while developing.
pub fn pipe_name() -> String {
    std::env::var("UNCOIL_PIPE").unwrap_or_else(|_| ipc::PIPE_NAME.to_string())
}

/// Forward one typed command to uncoild's control pipe (the same surface the `uncoil` CLI uses).
/// `UNCOIL_PIPE` points the app at another daemon, e.g. `uncoild --fake` while developing.
#[tauri::command]
async fn daemon(
    device: Option<String>,
    cmd: String,
    args: Option<serde_json::Value>,
) -> Result<serde_json::Value, DaemonFailure> {
    tauri::async_runtime::spawn_blocking(move || {
        let args = args.map(|a| a.to_string());
        let command = Command::from_parts(&cmd, args.as_deref()).map_err(|e| DaemonFailure::new(e, false))?;
        let mut client = Client::connect_to(&pipe_name()).map_err(|e| DaemonFailure::new(e, true))?;
        let response = client.call(device.as_deref(), &command).map_err(|e| DaemonFailure::new(e, true))?;
        response.into_result::<serde_json::Value>().map_err(|e| DaemonFailure::new(e, false))
    })
    .await
    .map_err(|e| DaemonFailure::new(e.into(), false))?
}

/// Show the window, sized to its monitor the first time (it may start hidden in the tray).
pub fn show_window<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    static SIZED: AtomicBool = AtomicBool::new(false);
    if SIZED.swap(true, Ordering::Relaxed) {
        let _ = window.show();
    } else {
        size_to_monitor(window);
    }
}

/// Size the window to the monitor it opens on: about 56% x 62.5% of it (1440x900 on a 2560x1440 screen),
/// never below the minimum and never past 1600x1000, then centre and show it. The window starts hidden so
/// it never flashes at the config's fallback size.
fn size_to_monitor<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
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

/// The start-with-Windows entry launches the app with this, to start hidden in the tray.
const TRAY_ARG: &str = "--tray";

fn main() {
    tauri::Builder::default()
        // First, so a second launch (e.g. the start-menu entry while autostart already runs it in the tray)
        // shows the running window instead of starting another copy.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::open_window(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec![TRAY_ARG])))
        .manage(AppState { settings: Mutex::new(AppSettings::load()) })
        .manage(tray::TrayState::default())
        .manage(ConfigSeen::default())
        .setup(|app| {
            use tauri::Manager;
            tray::create(app.handle())?;
            battery::spawn(app.handle().clone());
            if !std::env::args().any(|a| a == TRAY_ARG) {
                if let Some(window) = app.get_webview_window("main") {
                    show_window(&window);
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            use tauri::Manager;
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let hide = window.app_handle().state::<AppState>().settings.lock().is_ok_and(|s| s.hide_on_close());
                if hide {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            get_desk,
            preview_frame,
            get_status,
            daemon,
            get_app_settings,
            save_app_settings
        ])
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
        let real = serde_json::to_string_pretty(&get_desk(Config::default(), None, None)).unwrap() + "\n";
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
        assert!(refused.unwrap_err().message.contains("write=true"), "onboard writes need write=true");
        let guard = run(
            Some("basilisk"),
            "keymap.set",
            Some(serde_json::json!({ "key": "LEFT_CLICK", "function": "button 2", "write": true })),
        );
        assert_eq!(guard.unwrap_err().code, Some(ipc::codes::LEFT_CLICK_GUARD));
        assert!(!run(None, "no.such.command", None).unwrap_err().unreachable);
    }

    #[test]
    fn failures_keep_their_code() {
        let e = DaemonFailure::new(ipc::coded(ipc::codes::LEFT_CLICK_GUARD, "This would leave no button."), false);
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"message":"This would leave no button.","code":"left_click_guard","unreachable":false}"#
        );
        let e = DaemonFailure::new(anyhow::anyhow!("cannot reach uncoild"), true);
        assert_eq!(e, DaemonFailure { message: "cannot reach uncoild".into(), code: None, unreachable: true });
    }

    #[test]
    fn connected_experimental_devices_join_the_desk() {
        // any experimental device with a layout from devices/experimental/
        let extra = defs()
            .iter()
            .find(|d| d.is_experimental() && d.lit().is_some())
            .expect("devices/experimental/ has a device with a layout");
        let base = get_desk(Config::default(), None, None);
        assert!(!base.iter().any(|d| d.placed.id == extra.id));
        let with = get_desk(Config::default(), Some(vec![extra.id.clone()]), None);
        assert_eq!(with.len(), base.len() + 1);
        for (a, b) in base.iter().zip(with.iter().filter(|d| d.placed.id != extra.id)) {
            assert_eq!((a.placed.x, a.placed.y), (b.placed.x, b.placed.y), "{} moved", a.placed.id);
        }
    }

    #[test]
    fn openrgb_devices_from_status_join_the_desk() {
        use uncoil_core::layout::{ExternalZone, ZoneKind};
        let ram = OpenRgbDeviceStatus {
            id: "openrgb:corsair-vengeance-pro-rgb".into(),
            name: "Corsair Vengeance Pro RGB".into(),
            leds: 10,
            zones: vec![ExternalZone { name: "DRAM".into(), kind: ZoneKind::Linear, leds: 10, matrix: None }],
        };
        let not_external = OpenRgbDeviceStatus { id: "razer-blackwidow-v4-pro-75".into(), ..ram.clone() };
        let base = get_desk(Config::default(), None, None);
        let with = get_desk(Config::default(), None, Some(vec![ram.clone(), not_external.clone()]));
        assert_eq!(with.len(), base.len() + 1, "only openrgb: ids are added");
        let dev = with.iter().find(|d| d.placed.id == ram.id).unwrap();
        assert_eq!((dev.kind, dev.placed.shapes.len()), (Kind::Other, 10));
        let config = with_effect(r#"{"kind":"static","color":[255,0,0]}"#);
        let frame = preview_frame(config, 0.0, None, None, None, Some(vec![ram, not_external]));
        assert_eq!(frame.len(), with.len());
        assert_eq!(shape_color(&frame, &with, "openrgb:corsair-vengeance-pro-rgb", "DRAM 1"), "#ff0000");
    }

    #[test]
    fn preview_frame_matches_desk_shape() {
        let config = Config::default();
        let desk = get_desk(config.clone(), None, None);
        let frame = preview_frame(config, 1.5, None, None, None, None);
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
        let desk = get_desk(config.clone(), None, None);
        let frame = preview_frame(config, 0.0, None, None, None, None);
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
        let desk = get_desk(config.clone(), None, None);
        let kb = "razer-blackwidow-v4-pro-75";
        let keyboard = desk.iter().find(|d| d.placed.id == kb).unwrap();
        let w = keyboard.placed.shapes.iter().find(|s| s.name == "W").unwrap();
        let press = vec![Press { x: w.x, y: w.y, t: 2.0 }];
        let lit = preview_frame(config.clone(), 2.0, Some(press), None, None, None);
        assert_eq!(shape_color(&lit, &desk, kb, "W"), "#ffffff");
        assert_eq!(shape_color(&lit, &desk, kb, "Q"), "#000000");
        let idle = preview_frame(config, 2.0, None, None, None, None);
        assert_eq!(shape_color(&idle, &desk, kb, "W"), "#000000");

        let meter = with_effect(r#"{"kind":"audio_meter","sensitivity":1}"#);
        let loud = preview_frame(meter.clone(), 0.0, None, Some(1.0), None, None);
        let quiet = preview_frame(meter, 0.0, None, Some(0.0), None, None);
        assert_ne!(shape_color(&loud, &desk, kb, "Escape"), "#000000");
        assert_eq!(shape_color(&quiet, &desk, kb, "Escape"), "#000000");
    }
}
