//! uncoild — the uncoil background daemon.
//!
//! Drives every supported device's lighting from one shared effect field, fades with the display,
//! picks up replugged devices, hot-reloads `%APPDATA%\uncoil\config.json` and publishes
//! `%LOCALAPPDATA%\uncoil\status.json` for the GUI. No window, no console.
//!
//! It is also the single owner of device I/O for everything else (key maps, profiles, the OLED dial, firmware
//! effects): the GUI and the `uncoil` CLI send commands over the control pipe `\\.\pipe\uncoil`
//! (`pipe.rs`, `control.rs`), and each device's renderer thread runs them between frames (`exec.rs`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod control;
mod exec;
#[cfg(any(test, feature = "fake"))]
mod fake;
mod inputs;
mod log;
mod openrgb;
mod pipe;
mod selfstat;

use anyhow::Result;
use hidapi::HidApi;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uncoil_core::config::{Config, DeviceStatus, Status};
use uncoil_core::device::{self, DeviceDef};
use uncoil_core::effect::Inputs;
use uncoil_core::features::hw_effect::{HwEffect, Storage};
use uncoil_core::ipc;
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
    /// Connected devices, for the control pipe.
    registry: Arc<control::Registry>,
    journal: Arc<exec::Journal>,
    /// Latest status snapshot (also served over the pipe).
    status: Arc<Mutex<Status>>,
    /// Key presses (positions only), audio level and desk geometry for the effects.
    inputs: Arc<inputs::Inputs>,
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

/// Control pipe name; `UNCOIL_PIPE` overrides it (tests, `--fake`).
fn pipe_name(default: &str) -> String {
    std::env::var("UNCOIL_PIPE").ok().filter(|s| s.starts_with(r"\\.\pipe\")).unwrap_or_else(|| default.into())
}

fn main() -> Result<()> {
    if std::env::args().any(|a| a == "--version") {
        println!("uncoild {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if std::env::args().any(|a| a == "--fake") {
        return fake_mode();
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
        registry: Arc::new(control::Registry::default()),
        journal: Arc::new(exec::Journal { path: Some(exec::Journal::default_path()) }),
        status: Arc::new(Mutex::new(Status::default())),
        inputs: Arc::new(inputs::Inputs::new()),
    });
    let mut listeners = inputs::Listeners::default();
    {
        let cfg = shared.config();
        shared.inputs.set_desk(inputs::Desk::new(&defs, &cfg));
        listeners.sync(&cfg, &shared.inputs, &shared.registry, shared.t0);
    }

    // control pipe: commands for the GUI / CLI, executed by the device threads
    let ctl = Arc::new(control::Control {
        registry: shared.registry.clone(),
        defs: defs.clone(),
        status: shared.status.clone(),
    });
    let name = pipe_name(ipc::PIPE_NAME);
    match pipe::serve(&name, Arc::new(move |r| ctl.handle(r))) {
        Ok(()) => log::line(&format!("control pipe {name}")),
        Err(e) => log::line(&format!("control pipe {name} unavailable: {e:#}")),
    }

    let mut api = HidApi::new()?;
    let mut cfg_mtime = mtime(&Config::path());
    let mut last_scan = Instant::now() - RESCAN;
    let mut last_status = Instant::now();
    let mut last_display = DisplayState::On;
    let started = unix_now();
    let mut prev = Instant::now();
    let mut selfstat = selfstat::SelfStat::new();

    loop {
        let now = Instant::now();
        let dt = now.duration_since(prev).as_secs_f32();
        prev = now;

        // hot-reload config
        let m = mtime(&Config::path());
        if m != cfg_mtime {
            cfg_mtime = m;
            let cfg = Arc::new(Config::load());
            *shared.config.write().unwrap() = cfg.clone();
            shared.inputs.set_desk(inputs::Desk::new(&defs, &cfg));
            listeners.sync(&cfg, &shared.inputs, &shared.registry, shared.t0);
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
            write_status(&shared, started, ds, &mut selfstat);
        }

        thread::sleep(if shared.level() > 0.0 { TICK } else { Duration::from_millis(250) });
    }
}

/// One thread per device: render the shared effect at the configured fps, and run control-pipe commands
/// for this device between frames.
fn spawn_renderer(shared: Arc<Shared>, mut dev: LiveDevice) {
    let path = dev.path.clone();
    shared.open_paths.lock().unwrap().insert(path.clone());
    let hw_state = Arc::new(Mutex::new(None::<HwEffect>));
    let (jobs, registration) = shared.registry.register(
        dev.def.clone(),
        dev.endpoint.product_id,
        dev.endpoint.connection.clone(),
        hw_state.clone(),
    );
    thread::Builder::new()
        .name(dev.def.id.clone())
        .spawn(move || {
            let _registration = registration;
            // a firmware effect replaces streamed frames until the config changes or `effect.software`
            let mut hw: Option<HwEffect> = None;
            let mut hw_dark = false;
            let mut last_ping = Instant::now();
            let mut jobs_open = true;
            let mut gen = u32::MAX;
            let mut wake = shared.wake.load(Ordering::Relaxed);
            let mut positions = Vec::new();
            let def = dev.def.clone();
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
                    if hw.take().is_some() {
                        // editing the config means "use the software effect again"
                        *hw_state.lock().unwrap() = None;
                        hw_dark = false;
                        if let Err(e) = dev.prepare() {
                            log::line(&format!("re-prepare {} failed: {e:#}", dev.def.name));
                        }
                    }
                }
                let w = shared.wake.load(Ordering::Relaxed);
                if w != wake {
                    wake = w;
                    let res = match &hw {
                        Some(e) => apply_hw(&mut dev, e),
                        None => dev.prepare().map(|_| ()),
                    };
                    if let Err(e) = res {
                        log::line(&format!("re-prepare {} after wake failed: {e:#}", dev.def.name));
                    }
                }
                let level = shared.level();
                let dark = level <= 0.0;
                if let Some(e) = &hw {
                    // no frames go out, so check now and then that the device is still there
                    if last_ping.elapsed() >= Duration::from_secs(2) {
                        last_ping = Instant::now();
                        if let Err(e) = dev.query(&uncoil_core::proto::get_device_mode(dev.tid())) {
                            log::line(&format!("lost {} ({e:#})", dev.def.name));
                            break;
                        }
                    }
                    // firmware effect: just follow the display (off while it is off)
                    if dark != hw_dark {
                        hw_dark = dark;
                        let res = if dark { apply_hw(&mut dev, &HwEffect::Off) } else { apply_hw(&mut dev, e) };
                        if let Err(e) = res {
                            log::line(&format!("firmware effect on {} failed: {e:#}", dev.def.name));
                        }
                    }
                } else if !dark || dark_frames < 3 {
                    // once faded out, send a few black frames and then idle (the device holds the frame)
                    let t = shared.t0.elapsed().as_secs_f32();
                    let presses = if cfg.effect.uses_keys() { shared.inputs.presses(t) } else { Vec::new() };
                    let desk = shared.inputs.desk();
                    let inputs = Inputs {
                        presses: &presses,
                        audio: shared.inputs.audio(),
                        bounds: desk.bounds,
                        keyboard_center: desk.keyboard_center,
                    };
                    let frame = cfg.effect.at_with(t, cfg.saturation, cfg.brightness * level, &inputs);
                    let names = &def.matrix.names;
                    let res = dev.send_frame(|r, c| match positions[r][c] {
                        Some((x, y)) => frame.color_led(&def.id, &names[r][c], x, y).bytes(),
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

                let target = if dark || hw.is_some() {
                    Duration::from_millis(250)
                } else {
                    Duration::from_secs_f32(1.0 / cfg.fps.clamp(5, 60) as f32)
                };
                // wait for the next frame, running any control commands for this device meanwhile
                let deadline = start + target;
                while let Some(rest) = deadline.checked_duration_since(Instant::now()) {
                    if !jobs_open {
                        thread::sleep(rest);
                        break;
                    }
                    match jobs.recv_timeout(rest) {
                        Ok(job) => {
                            let (tid, def) = (dev.tid(), dev.def.clone());
                            match control::serve_job(job, &mut dev, &def, tid, &shared.journal) {
                                exec::Lighting::Hardware(e) => {
                                    log::line(&format!("{}: firmware effect {}", def.name, e.name()));
                                    *hw_state.lock().unwrap() = Some(e.clone());
                                    hw = Some(e);
                                    hw_dark = false;
                                }
                                exec::Lighting::Software => {
                                    *hw_state.lock().unwrap() = None;
                                    hw = None;
                                }
                                exec::Lighting::Unchanged => {}
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => break,
                        // another endpoint of the same device took over the control channel
                        Err(RecvTimeoutError::Disconnected) => jobs_open = false,
                    }
                }
            }
            shared.stats.lock().unwrap().remove(&path);
            shared.open_paths.lock().unwrap().remove(&path);
        })
        .expect("spawn renderer");
}

/// Show a firmware effect for this session (not saved to the device).
fn apply_hw(dev: &mut LiveDevice, e: &HwEffect) -> Result<()> {
    let led = dev.def.hw_effects.as_ref().map_or(0, |h| h.led);
    let r = e.report(dev.tid(), Storage::Session, led)?;
    uncoil_core::proto::query_ok(dev, &r)?;
    Ok(())
}

/// `uncoild --fake`: serve the control pipe for fake devices (no hardware, no lighting). For trying the CLI
/// and GUI; build with `--features fake`. Pipe: `UNCOIL_PIPE` or `\\.\pipe\uncoil-fake`.
#[cfg(feature = "fake")]
fn fake_mode() -> Result<()> {
    use uncoil_core::features::Feature;
    log::to_stderr();
    let defs: Vec<Arc<DeviceDef>> = device::builtin().into_iter().map(Arc::new).collect();
    let registry = Arc::new(control::Registry::default());
    let journal = Arc::new(exec::Journal { path: None });
    for d in defs.iter().filter(|d| d.has(Feature::Keymap)) {
        control::spawn_fake(&registry, d.clone(), journal.clone());
    }
    let ctl = Arc::new(control::Control { registry, defs, status: Arc::new(Mutex::new(Status::default())) });
    let name = pipe_name(r"\\.\pipe\uncoil-fake");
    pipe::serve(&name, Arc::new(move |r| ctl.handle(r)))?;
    println!("uncoild --fake: serving fake devices on {name}");
    loop {
        thread::park();
    }
}

#[cfg(not(feature = "fake"))]
fn fake_mode() -> Result<()> {
    anyhow::bail!("this uncoild was built without the `fake` feature")
}

fn write_status(shared: &Shared, started: u64, ds: DisplayState, me: &mut selfstat::SelfStat) {
    let (memory_bytes, cpu_percent) = me.sample();
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
        memory_bytes,
        cpu_percent,
        exe_bytes: me.exe_bytes,
    };
    *shared.status.lock().unwrap() = st.clone();
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
