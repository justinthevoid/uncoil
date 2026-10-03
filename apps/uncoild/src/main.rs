//! uncoild — the uncoil background daemon.
//!
//! Drives every supported device's lighting from one shared effect field, fades with the display,
//! picks up replugged devices, hot-reloads `%APPDATA%\uncoil\config.json` and publishes
//! `%LOCALAPPDATA%\uncoil\status.json` for the GUI. No window, no console.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod log;
mod openrgb;

use anyhow::Result;
use hidapi::HidApi;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uncoil_core::config::{Config, DeviceStatus, Status};
use uncoil_core::device::{self, DeviceDef};
use uncoil_core::layout;
use uncoil_hid::display::{self, DisplayState};
use uncoil_hid::transport::{self, LiveDevice};

const RESCAN: Duration = Duration::from_secs(5);
const STATUS_EVERY: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_millis(33);

/// State shared between the main loop and device threads.
struct Shared {
    config: RwLock<Arc<Config>>,
    /// Bumped whenever the config changes, so threads re-place their LEDs.
    generation: AtomicU32,
    /// Current brightness multiplier from the display fade (f32 bits).
    level: AtomicU32,
    /// Bumped when the display wakes: devices may have reset during sleep, so re-prepare them.
    wake: AtomicU32,
    t0: Instant,
    open_paths: Mutex<HashSet<CString>>,
    stats: Mutex<HashMap<CString, DeviceStatus>>,
}

impl Shared {
    fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
    fn set_level(&self, v: f32) {
        self.level.store(v.to_bits(), Ordering::Relaxed);
    }
    fn config(&self) -> Arc<Config> {
        self.config.read().unwrap().clone()
    }
}

fn main() -> Result<()> {
    if std::env::args().any(|a| a == "--version") {
        println!("uncoild {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if !single_instance() {
        log::line("another uncoild is already running; exiting");
        return Ok(());
    }
    log::line(&format!("start uncoild {}", env!("CARGO_PKG_VERSION")));

    let config = Config::load();
    if config.openrgb_hardware_rainbow {
        thread::spawn(openrgb::hardware_rainbow);
    }
    display::spawn_watcher();

    let defs: Vec<Arc<DeviceDef>> =
        device::load_all(Some(&Config::dir().join("devices"))).into_iter().map(Arc::new).collect();
    let shared = Arc::new(Shared {
        config: RwLock::new(Arc::new(config)),
        generation: AtomicU32::new(0),
        level: AtomicU32::new(1.0f32.to_bits()),
        wake: AtomicU32::new(0),
        t0: Instant::now(),
        open_paths: Mutex::new(HashSet::new()),
        stats: Mutex::new(HashMap::new()),
    });

    let mut api = HidApi::new()?;
    let mut cfg_mtime = mtime(&Config::path());
    let mut last_scan = Instant::now() - RESCAN;
    let mut last_status = Instant::now();
    let mut last_display = DisplayState::On;
    let started = unix_now();
    let mut prev = Instant::now();

    loop {
        let now = Instant::now();
        let dt = now.duration_since(prev).as_secs_f32();
        prev = now;

        // hot-reload config
        let m = mtime(&Config::path());
        if m != cfg_mtime {
            cfg_mtime = m;
            *shared.config.write().unwrap() = Arc::new(Config::load());
            shared.generation.fetch_add(1, Ordering::Relaxed);
            log::line("config reloaded");
        }
        let cfg = shared.config();

        // display fade: off -> 0, dimmed -> dim_level, on -> 1
        let ds = display::current();
        if ds != last_display {
            log::line(&format!("display {}", ds.as_str()));
            if last_display == DisplayState::Off {
                shared.wake.fetch_add(1, Ordering::Relaxed);
            }
            last_display = ds;
        }
        let target = match ds {
            DisplayState::Off if cfg.display.off_when_display_off => 0.0,
            DisplayState::Dimmed => cfg.display.dim_level.clamp(0.0, 1.0),
            _ => 1.0,
        };
        let step = dt / cfg.display.fade_s.max(0.05);
        let lvl = shared.level();
        shared.set_level(if lvl < target { (lvl + step).min(target) } else { (lvl - step).max(target) });

        // pick up new / replugged devices
        if now.duration_since(last_scan) >= RESCAN {
            last_scan = now;
            let skip = shared.open_paths.lock().unwrap().clone();
            for cand in transport::discover(&mut api, &defs, &skip) {
                let name = cand.def.name.clone();
                let pid = cand.endpoint.product_id;
                match LiveDevice::open(&api, cand) {
                    Ok(Some(dev)) => {
                        log::line(&format!("opened {name} ({pid:04X}, {})", dev.endpoint.connection));
                        spawn_renderer(shared.clone(), dev);
                    }
                    Ok(None) => {} // endpoint present, nobody answering (e.g. dongle while mouse is wired)
                    Err(e) => log::line(&format!("open {name} ({pid:04X}) failed: {e:#}")),
                }
            }
        }

        if now.duration_since(last_status) >= STATUS_EVERY {
            last_status = now;
            write_status(&shared, started, ds);
        }

        thread::sleep(if shared.level() > 0.0 { TICK } else { Duration::from_millis(250) });
    }
}

/// One thread per device: render the shared effect at the configured fps.
fn spawn_renderer(shared: Arc<Shared>, mut dev: LiveDevice) {
    let path = dev.path.clone();
    shared.open_paths.lock().unwrap().insert(path.clone());
    thread::Builder::new()
        .name(dev.def.id.clone())
        .spawn(move || {
            let mut gen = u32::MAX;
            let mut wake = shared.wake.load(Ordering::Relaxed);
            let mut positions = Vec::new();
            let mut dark_frames = 0u32;
            let mut frames = 0u32;
            let mut fps_window = Instant::now();
            loop {
                let start = Instant::now();
                let cfg = shared.config();
                let g = shared.generation.load(Ordering::Relaxed);
                if g != gen {
                    gen = g;
                    let at = cfg.desk.get(&dev.def.id).copied().unwrap_or_else(|| layout::default_placement(&dev.def));
                    positions = layout::place(&dev.def, at).positions;
                }
                let w = shared.wake.load(Ordering::Relaxed);
                if w != wake {
                    wake = w;
                    if let Err(e) = dev.prepare() {
                        log::line(&format!("re-prepare {} after wake failed: {e:#}", dev.def.name));
                    }
                }
                let level = shared.level();
                let dark = level <= 0.0;
                // once faded out, send a few black frames and then idle (the device holds the frame)
                if !dark || dark_frames < 3 {
                    let t = shared.t0.elapsed().as_secs_f32();
                    let frame = cfg.effect.at(t, cfg.saturation, cfg.brightness * level);
                    let res = dev.send_frame(|r, c| match positions[r][c] {
                        Some((x, y)) => frame.color_at(x, y).bytes(),
                        None => [0, 0, 0],
                    });
                    if let Err(e) = res {
                        log::line(&format!("lost {} ({e:#})", dev.def.name));
                        break;
                    }
                    frames += 1;
                }
                dark_frames = if dark { dark_frames + 1 } else { 0 };

                let el = fps_window.elapsed();
                if el >= Duration::from_secs(2) {
                    let fps = frames as f32 / el.as_secs_f32();
                    frames = 0;
                    fps_window = Instant::now();
                    shared.stats.lock().unwrap().insert(
                        dev.path.clone(),
                        DeviceStatus {
                            id: dev.def.id.clone(),
                            name: dev.def.name.clone(),
                            product_id: dev.endpoint.product_id,
                            connection: dev.endpoint.connection.clone(),
                            fps,
                            busy_retries: dev.busy_retries,
                            errors: dev.errors,
                        },
                    );
                }

                let target = if dark {
                    Duration::from_millis(250)
                } else {
                    Duration::from_secs_f32(1.0 / cfg.fps.clamp(5, 60) as f32)
                };
                if let Some(rest) = target.checked_sub(start.elapsed()) {
                    thread::sleep(rest);
                }
            }
            shared.stats.lock().unwrap().remove(&path);
            shared.open_paths.lock().unwrap().remove(&path);
        })
        .expect("spawn renderer");
}

fn write_status(shared: &Shared, started: u64, ds: DisplayState) {
    let mut devices: Vec<DeviceStatus> = shared.stats.lock().unwrap().values().cloned().collect();
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    let st = Status {
        pid: std::process::id(),
        version: env!("CARGO_PKG_VERSION").into(),
        started_unix: started,
        updated_unix: unix_now(),
        display: ds.as_str().into(),
        level: shared.level(),
        devices,
    };
    let path = Status::path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_vec_pretty(&st).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(tmp, path);
    }
}

fn mtime(p: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(windows)]
fn single_instance() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name: Vec<u16> = "Local\\uncoild-single-instance\0".encode_utf16().collect();
    unsafe {
        let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        // handle intentionally leaked: held for the life of the process
        !h.is_null() && GetLastError() != ERROR_ALREADY_EXISTS
    }
}

#[cfg(not(windows))]
fn single_instance() -> bool {
    true
}
