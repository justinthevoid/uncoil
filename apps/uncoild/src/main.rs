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

mod checks;
mod conflicts;
mod control;
mod exec;
#[cfg(any(test, feature = "fake"))]
mod fake;
mod inputs;
mod log;
mod openrgb;
mod openrgb_live;
mod pipe;
mod selfstat;
mod winsec;

use anyhow::Result;
use hidapi::HidApi;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uncoil_core::config::{Config, DeviceStatus, Status, UnknownDevice};
use uncoil_core::device::{self, DeviceDef};
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
    /// Bumped whenever the config changes, so threads re-place their LEDs and drop firmware effects.
    generation: AtomicU32,
    /// Bumped when the desk changes without a config change (a device connected or went away), so threads
    /// re-place their LEDs.
    desk_generation: AtomicU32,
    /// Razer devices on the bus with no definition (for status).
    unknown: Mutex<Vec<UnknownDevice>>,
    /// Other programs running that drive the same devices (for status).
    conflicts: Mutex<Vec<ipc::Conflict>>,
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
    /// Live OpenRGB: its state and the external devices it puts on the desk.
    openrgb: openrgb_live::Live,
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

    /// Rebuild the desk from the config, the connected devices and the OpenRGB devices being driven. Returns
    /// whether any device moved.
    fn rearrange(&self, defs: &[Arc<DeviceDef>]) -> bool {
        let all: Vec<Arc<DeviceDef>> = defs.iter().cloned().chain(self.openrgb.defs()).collect();
        let desk = inputs::desk(&all, &self.config(), &self.registry.connected_ids());
        let moved = !desk.same_places(&self.inputs.desk());
        self.inputs.set_desk(desk);
        moved
    }
}

/// Control pipe name. Builds with the `fake` feature let `UNCOIL_PIPE` override it (`--fake`, development);
/// release builds always use the default, so nothing in the environment can move the daemon's pipe.
fn pipe_name(default: &str) -> String {
    #[cfg(feature = "fake")]
    if let Some(name) = std::env::var("UNCOIL_PIPE").ok().filter(|s| s.starts_with(r"\\.\pipe\")) {
        return name;
    }
    default.into()
}

fn main() -> Result<()> {
    if std::env::args().any(|a| a == "--version") {
        println!("uncoild {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if std::env::args().any(|a| a == "--fake") {
        return fake_mode();
    }
    if std::env::args().any(|a| a == "--openrgb-once") {
        std::process::exit(openrgb::once());
    }
    // Normally unelevated; the `-Elevated` install is the fallback for PCs where HID needs it.
    let elevated = winsec::init();
    let hardened = if elevated { Some(winsec::enforce_redirection_trust()) } else { None };

    let (config, config_problem) = Config::load_reporting();
    let (defs, user_errors) = device::load_installed();
    let defs: Vec<Arc<DeviceDef>> = defs.into_iter().map(Arc::new).collect();
    let registry = Arc::new(control::Registry::default());
    let status = Arc::new(Mutex::new(Status::default()));

    // The control pipe is also the single-instance lock: it is created with FILE_FLAG_FIRST_PIPE_INSTANCE,
    // so a second uncoild (or anything else that took the name first) cannot create it, and this process
    // exits before it opens a single device.
    let ctl = Arc::new(control::Control { registry: registry.clone(), defs: defs.clone(), status: status.clone() });
    let name = pipe_name(ipc::PIPE_NAME);
    if let Err(e) = pipe::serve(&name, Arc::new(move |r| ctl.handle(r))) {
        log::line(&format!(
            "control pipe {name} unavailable ({e:#}): another uncoild, or another program, holds it; exiting"
        ));
        return Ok(());
    }
    log::line(&format!(
        "start uncoild {} ({}), control pipe {name}",
        env!("CARGO_PKG_VERSION"),
        if elevated { "elevated" } else { "not elevated" }
    ));
    match hardened {
        Some(Err(e)) => log::line(&format!("redirection-trust mitigation unavailable: {e}")),
        Some(Ok(())) => log::line("elevated: junctions made by non-administrators are not followed"),
        None => {}
    }
    match config.openrgb_mode() {
        uncoil_core::config::OpenRgbMode::Off => {}
        _ if !elevated => log::line(
            "OpenRGB: the hand-off or server is left to the elevated uncoil-openrgb task (install-task.ps1 -OpenRgb)",
        ),
        uncoil_core::config::OpenRgbMode::Hardware => {
            let cfg = config.clone();
            thread::spawn(move || match openrgb::hand_off(&cfg) {
                Ok(s) => log::line(&s),
                Err(e) => log::line(&format!("OpenRGB hand-off: {e}")),
            });
        }
        uncoil_core::config::OpenRgbMode::Live => {
            let cfg = config.clone();
            thread::spawn(move || match openrgb::serve(&cfg) {
                Ok(()) => log::line("OpenRGB's SDK server ended"),
                Err(e) => log::line(&format!("OpenRGB server: {e}")),
            });
        }
    }
    display::spawn_watcher();
    if let uncoil_hid::guard::Opened::Without(why) = uncoil_hid::guard::razer().opened() {
        log::line(&format!("Razer device lock unavailable ({why}); running without it"));
    }
    if let Some(p) = config_problem {
        log::line(&p);
    }

    for e in device::builtin_errors() {
        log::line(&format!("built-in device file left out: {e}"));
    }
    for e in user_errors {
        log::line(&format!("device file left out: {e}"));
    }
    let shared = Arc::new(Shared {
        config: RwLock::new(Arc::new(config)),
        generation: AtomicU32::new(0),
        desk_generation: AtomicU32::new(0),
        unknown: Mutex::new(Vec::new()),
        conflicts: Mutex::new(Vec::new()),
        level: AtomicU32::new(1.0f32.to_bits()),
        wake: AtomicU32::new(0),
        t0: Instant::now(),
        open_paths: Mutex::new(HashSet::new()),
        stats: Mutex::new(HashMap::new()),
        registry,
        journal: Arc::new(exec::Journal { path: Some(exec::Journal::default_path()) }),
        status,
        inputs: Arc::new(inputs::Inputs::new()),
        openrgb: openrgb_live::Live::default(),
    });
    openrgb_live::spawn(shared.clone());
    let mut listeners = inputs::Listeners::default();
    {
        let cfg = shared.config();
        shared.rearrange(&defs);
        listeners.sync(&cfg, &shared.inputs, &shared.registry, shared.t0);
    }
    let mut connected = shared.registry.connected_ids();
    let mut openrgb_gen = shared.openrgb.generation();
    let mut logged_unknown: HashSet<u16> = HashSet::new();
    let mut seen_conflicts = conflicts::Seen::default();

    let mut api = HidApi::new()?;
    let mut cfg_mtime = mtime(&Config::path());
    // None: scan right away
    let mut last_scan: Option<Instant> = None;
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
            let (cfg, problem) = Config::load_reporting();
            if let Some(p) = problem {
                log::line(&p);
            }
            let cfg = Arc::new(cfg);
            *shared.config.write().unwrap() = cfg.clone();
            shared.rearrange(&defs);
            listeners.sync(&cfg, &shared.inputs, &shared.registry, shared.t0);
            shared.generation.fetch_add(1, Ordering::Relaxed);
            log::line("config reloaded");
        }
        let cfg = shared.config();

        // OpenRGB devices came or went: they join or leave the desk
        let g = shared.openrgb.generation();
        if g != openrgb_gen {
            openrgb_gen = g;
            if shared.rearrange(&defs) {
                shared.desk_generation.fetch_add(1, Ordering::Relaxed);
            }
        }

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
        if last_scan.is_none_or(|t| now.duration_since(t) >= RESCAN) {
            last_scan = Some(now);
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
            let unknown = transport::unknown_devices(&api, &defs);
            for u in &unknown {
                if logged_unknown.insert(u.product_id) {
                    log::line(&format!(
                        "Razer device {:04X} (interfaces {:?}) has no device definition",
                        u.product_id, u.interfaces
                    ));
                }
            }
            *shared.unknown.lock().unwrap() = unknown;
            // other programs driving the same devices (uncoil's own OpenRGB server does not count)
            let ours = shared.status.lock().unwrap().openrgb.ours;
            let found = conflicts::scan(ours);
            for l in seen_conflicts.update(&found) {
                log::line(&l);
            }
            *shared.conflicts.lock().unwrap() = found;
            // connected devices with a layout join the desk; ones that left free their spot
            let now_connected = shared.registry.connected_ids();
            if now_connected != connected {
                connected = now_connected;
                if shared.rearrange(&defs) {
                    shared.desk_generation.fetch_add(1, Ordering::Relaxed);
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
    // read-only check results for this connection; dropped (and forgotten) when the device goes away
    let mut checks = checks::Checks::new(&dev.def);
    let (jobs, registration) = shared.registry.register(
        dev.def.clone(),
        dev.endpoint.product_id,
        dev.endpoint.connection.clone(),
        hw_state.clone(),
        checks.mirror(),
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
            let mut desk_gen = u32::MAX;
            let mut wake = shared.wake.load(Ordering::Relaxed);
            let mut positions: Vec<Vec<Option<(f32, f32)>>> = Vec::new();
            let def = dev.def.clone();
            // feature-only devices (no lighting) are only served commands, never frames
            let streams = def.streams_frames();
            let names: Vec<Vec<String>> = def.matrix.as_ref().map(|m| m.names.clone()).unwrap_or_default();
            let mut dark_frames = 0u32;
            let mut frames = 0u32;
            let mut fps_window = Instant::now();
            loop {
                let start = Instant::now();
                let cfg = shared.config();
                let g = shared.generation.load(Ordering::Relaxed);
                let dg = shared.desk_generation.load(Ordering::Relaxed);
                if g != gen || dg != desk_gen {
                    let at = shared.inputs.desk().placement(&dev.def);
                    positions = layout::place(&dev.def, at).map(|p| p.positions).unwrap_or_default();
                    desk_gen = dg;
                }
                if g != gen {
                    gen = g;
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
                if hw.is_some() || !streams {
                    // no frames go out, so check now and then that the device is still there
                    if last_ping.elapsed() >= Duration::from_secs(2) {
                        last_ping = Instant::now();
                        match dev.query(&uncoil_core::proto::get_device_mode(dev.tid())) {
                            // another program held the Razer device lock: the device is still there
                            Err(e) if !uncoil_hid::guard::is_busy(&e) => {
                                log::line(&format!("lost {} ({e:#})", dev.def.name));
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(e) = &hw {
                    // firmware effect: just follow the display (off while it is off)
                    if dark != hw_dark {
                        hw_dark = dark;
                        let res = if dark { apply_hw(&mut dev, &HwEffect::Off) } else { apply_hw(&mut dev, e) };
                        if let Err(e) = res {
                            log::line(&format!("firmware effect on {} failed: {e:#}", dev.def.name));
                        }
                    }
                } else if streams && (!dark || dark_frames < 3) {
                    // once faded out, send a few black frames and then idle (the device holds the frame)
                    let t = shared.t0.elapsed().as_secs_f32();
                    let presses = if cfg.effect.uses_keys() { shared.inputs.presses(t) } else { Vec::new() };
                    let inputs = shared.inputs.desk().inputs(&presses, shared.inputs.audio());
                    let frame = cfg.effect.at_with(t, cfg.saturation, cfg.brightness * level, &inputs);
                    let res =
                        dev.send_frame(|r, c| match positions.get(r).and_then(|row| row.get(c)).copied().flatten() {
                            Some((x, y)) => frame.color_led(&def.id, &names[r][c], x, y).bytes(),
                            None => [0, 0, 0],
                        });
                    match res {
                        Ok(sent) => frames += sent as u32,
                        Err(e) => {
                            log::line(&format!("lost {} ({e:#})", dev.def.name));
                            break;
                        }
                    }
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

                let target = if dark || hw.is_some() || !streams {
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
                            match control::serve_job(job, &mut dev, &def, tid, &shared.journal, &mut checks) {
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
                        // the registry dropped this endpoint's queue (does not happen while it is registered)
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
    log::to_stderr();
    let registry = Arc::new(control::Registry::default());
    let journal = Arc::new(exec::Journal { path: None });
    let defs = fake::fake_defs();
    for d in fake::connected_fakes(&defs) {
        control::spawn_fake(&registry, d, journal.clone());
    }
    let status = Status {
        version: env!("CARGO_PKG_VERSION").into(),
        unknown_devices: fake::unknown_devices(),
        conflicts: fake::conflicts(),
        ..Default::default()
    };
    let ctl = Arc::new(control::Control { registry, defs, status: Arc::new(Mutex::new(status)) });
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
        unknown_devices: shared.unknown.lock().unwrap().clone(),
        conflicts: shared.conflicts.lock().unwrap().clone(),
        openrgb: shared.openrgb.status(),
    };
    *shared.status.lock().unwrap() = st.clone();
    let path = Status::path();
    let tmp = path.with_extension("json.tmp");
    let written = winsec::open_user_file(&tmp, false).and_then(|mut f| {
        use std::io::Write;
        f.set_len(0)?;
        f.write_all(&serde_json::to_vec_pretty(&st).unwrap_or_default())
    });
    match written {
        // renaming replaces status.json itself, never a file it might link to
        Ok(()) => {
            let _ = std::fs::rename(tmp, path);
        }
        Err(e) => winsec::warn_once(&e),
    }
}

fn mtime(p: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
