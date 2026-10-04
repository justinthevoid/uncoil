//! Live OpenRGB (`openrgb.mode = "live"`): the PC's other lighting (motherboard, RAM, GPU) follows the desk
//! effect. OpenRGB runs as an SDK server on 127.0.0.1 (started by the elevated `uncoil-openrgb` task, see
//! `openrgb.rs`); this thread connects to it as a client, puts each of its devices on the desk as
//! `openrgb:<slug>` and sends every one a frame per tick.
//!
//! It has its own thread, so a slow or stuck OpenRGB only ever delays OpenRGB's frames, never the Razer
//! devices': each frame is computed when it is sent (nothing queues up to go stale), writes time out after
//! a second and then the connection is dropped and retried with a growing pause (1 s doubling to 10 s).
//! OpenRGB coalesces updates per device itself, so slow SMBus devices skip frames rather than lag.
//!
//! Razer devices that OpenRGB reports are always left alone (uncoil drives them itself), as are hidden ones,
//! names matched by `openrgb.live.exclude`, and RAM while Corsair iCUE runs (both would write the SMBus).

use crate::{log, Shared};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use uncoil_core::config::{OpenRgbLive, OpenRgbMode};
use uncoil_core::device::DeviceDef;
use uncoil_core::ipc::{OpenRgbDeviceStatus, OpenRgbState, OpenRgbStatus};
use uncoil_core::layout::{self, ExternalZone, ZoneKind, ZoneMatrix};
use uncoil_openrgb::{device_type, Client, Controller};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(10);
/// Resend an unchanged frame this often (a device may have been reset meanwhile).
const REFRESH: Duration = Duration::from_secs(1);
/// OpenRGB devices are slower than USB ones; no point sending more often than this.
const MAX_FPS: u32 = 30;

/// What the main loop, the desk and the status see of the live client.
#[derive(Default)]
pub struct Live {
    state: Mutex<State>,
    /// Bumped when the device list changes (the desk is rebuilt).
    generation: AtomicU32,
}

#[derive(Default)]
struct State {
    status: OpenRgbStatus,
    defs: Vec<Arc<DeviceDef>>,
}

impl Live {
    /// `status.openrgb`, with whether the running OpenRGB is uncoil's own.
    pub fn status(&self) -> OpenRgbStatus {
        let mut s = self.state.lock().unwrap().status.clone();
        s.ours = crate::openrgb::ours();
        s
    }

    /// Desk devices for the OpenRGB devices being driven.
    pub fn defs(&self) -> Vec<Arc<DeviceDef>> {
        self.state.lock().unwrap().defs.clone()
    }

    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Relaxed)
    }

    /// New state (logged when it changes); `devices` replaces the device list when given.
    fn set(&self, state: OpenRgbState, detail: Option<String>, devices: Option<Vec<OpenRgbDeviceStatus>>) {
        let mut s = self.state.lock().unwrap();
        if (s.status.state, &s.status.detail) != (state, &detail) {
            let what = detail.clone().unwrap_or_else(|| format!("{state:?}").to_lowercase());
            log::line(&format!("OpenRGB live: {what}"));
        }
        s.status.state = state;
        s.status.detail = detail;
        if let Some(devices) = devices {
            if devices != s.status.devices {
                s.defs = devices.iter().map(|d| Arc::new(d.def())).collect();
                s.status.devices = devices;
                self.generation.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// One OpenRGB device being driven.
struct Driven {
    /// Its index on the server (protocol 5 and below address devices by index).
    index: u32,
    id: String,
    names: Vec<String>,
    /// How many colours its `UpdateLEDs` takes.
    colors: usize,
    last: Vec<[u8; 3]>,
    sent: Option<Instant>,
}

/// Which OpenRGB devices to drive, in desk order (motherboards, RAM, GPUs, the rest), with their status
/// entries (ids `openrgb:<slug>`, made unique with `-2`, `-3`, ...).
fn plan(controllers: &[Controller], live: &OpenRgbLive, icue: bool) -> Vec<(u32, OpenRgbDeviceStatus)> {
    let mut picked: Vec<(u32, &Controller)> = controllers
        .iter()
        .enumerate()
        .map(|(i, c)| (i as u32, c))
        .filter(|(_, c)| {
            let razer = [&c.name, &c.vendor].iter().any(|s| s.to_lowercase().contains("razer"));
            !razer
                && !live.excludes(&c.name)
                && c.flags & uncoil_openrgb::CONTROLLER_FLAG_HIDDEN == 0
                && c.zones.iter().any(|z| z.leds > 0)
                && !(icue && c.kind == device_type::DRAM)
        })
        .collect();
    let rank = |k: i32| match k {
        device_type::MOTHERBOARD => 0,
        device_type::DRAM => 1,
        device_type::GPU => 2,
        _ => 3,
    };
    picked.sort_by_key(|(_, c)| rank(c.kind));
    let mut ids: Vec<String> = Vec::new();
    picked
        .into_iter()
        .map(|(index, c)| {
            let base = format!("{}{}", layout::EXTERNAL_PREFIX, layout::slug(&c.name));
            let mut id = base.clone();
            let mut n = 2;
            while ids.contains(&id) {
                id = format!("{base}-{n}");
                n += 1;
            }
            ids.push(id.clone());
            let zones: Vec<ExternalZone> = c
                .zones
                .iter()
                .map(|z| ExternalZone {
                    name: z.name.clone(),
                    kind: match (z.kind, &z.matrix) {
                        (0, _) => ZoneKind::Single,
                        (2, Some(_)) => ZoneKind::Matrix,
                        _ => ZoneKind::Linear,
                    },
                    leds: z.leds,
                    matrix: z.matrix.as_ref().map(|m| ZoneMatrix {
                        width: m.width,
                        height: m.height,
                        map: m.map.iter().map(|&l| (l != u32::MAX).then_some(l)).collect(),
                    }),
                })
                .collect();
            let leds = zones.iter().map(|z| z.leds).sum();
            (index, OpenRgbDeviceStatus { id, name: c.name.clone(), leds, zones })
        })
        .collect()
}

/// Connect, list the devices and put the ones to drive in their per-LED mode.
fn connect(live: &OpenRgbLive) -> std::io::Result<(Client, Vec<Driven>, Vec<OpenRgbDeviceStatus>)> {
    let port = live.valid_port().ok_or_else(|| std::io::Error::other("openrgb.live.port must be 1024-65535"))?;
    let mut client = Client::connect(port, "uncoil", CONNECT_TIMEOUT)?;
    let controllers = client.controllers()?;
    let icue = controllers.iter().any(|c| c.kind == device_type::DRAM) && crate::openrgb::running("iCUE.exe");
    let planned = plan(&controllers, live, icue);
    let mut driven = Vec::new();
    for (index, status) in &planned {
        client.set_custom_mode(*index)?;
        let colors = controllers[*index as usize].colors;
        let names = layout::external_led_names(&status.zones);
        driven.push(Driven { index: *index, id: status.id.clone(), names, colors, last: vec![], sent: None });
    }
    Ok((client, driven, planned.into_iter().map(|(_, s)| s).collect()))
}

/// Why connecting failed, in plain words: still waiting for a server, or something wrong.
fn failure(e: &std::io::Error, port: u16) -> (OpenRgbState, String) {
    use std::io::ErrorKind::*;
    match e.kind() {
        ConnectionRefused | TimedOut | WouldBlock | ConnectionReset | ConnectionAborted => (
            OpenRgbState::Waiting,
            format!(
                "Waiting for OpenRGB's SDK server on 127.0.0.1:{port}. uncoil's uncoil-openrgb task starts it at sign-in."
            ),
        ),
        _ => (OpenRgbState::Error, format!("OpenRGB on 127.0.0.1:{port}: {e}")),
    }
}

/// Compute this tick's colours for every driven device and send the ones that changed.
fn send_frame(shared: &Shared, client: &mut Client, driven: &mut [Driven], level: f32) -> std::io::Result<()> {
    let cfg = shared.config();
    let t = shared.t0.elapsed().as_secs_f32();
    let desk = shared.inputs.desk();
    let presses = if cfg.effect.uses_keys() { shared.inputs.presses(t) } else { Vec::new() };
    let inputs = desk.inputs(&presses, shared.inputs.audio());
    let frame = cfg.effect.at_with(t, cfg.saturation, cfg.brightness * level, &inputs);
    for d in driven {
        // not on the desk yet (the main loop rebuilds it within a tick)
        let Some((_, placed)) = desk.devices.iter().find(|(_, p)| p.id == d.id) else { continue };
        let row = placed.positions.first();
        let colors: Vec<[u8; 3]> = (0..d.colors)
            .map(|i| match (row.and_then(|r| r.get(i)).copied().flatten(), d.names.get(i)) {
                (Some((x, y)), Some(name)) => frame.color_led(&d.id, name, x, y).bytes(),
                _ => [0, 0, 0],
            })
            .collect();
        if colors == d.last && d.sent.is_some_and(|s| s.elapsed() < REFRESH) {
            continue;
        }
        client.update_leds(d.index, &colors)?;
        d.last = colors;
        d.sent = Some(Instant::now());
    }
    Ok(())
}

/// Start the live client's thread. It idles (one config look a second) unless live mode is on.
pub fn spawn(shared: Arc<Shared>) {
    thread::Builder::new().name("openrgb".into()).spawn(move || run(&shared)).expect("spawn the OpenRGB thread");
}

fn run(shared: &Shared) {
    let live = &shared.openrgb;
    // the connection, the devices it drives and the settings it was made for
    let mut conn: Option<(Client, Vec<Driven>, OpenRgbLive)> = None;
    let mut backoff = Duration::from_secs(1);
    let mut next_try = Instant::now();
    let mut dark_frames = 0u32;
    loop {
        let start = Instant::now();
        let cfg = shared.config();
        if cfg.openrgb_mode() != OpenRgbMode::Live {
            conn = None;
            if live.state.lock().unwrap().status.state != OpenRgbState::Off {
                live.set(OpenRgbState::Off, None, Some(vec![]));
            }
            thread::sleep(Duration::from_secs(1));
            continue;
        }
        let settings = cfg.openrgb_live();
        // new port or exclusions: list the devices again
        if conn.as_ref().is_some_and(|(_, _, made_for)| *made_for != settings) {
            conn = None;
            next_try = start;
        }
        if conn.is_none() {
            if start < next_try {
                thread::sleep((next_try - start).min(Duration::from_millis(250)));
                continue;
            }
            match connect(&settings) {
                Ok((client, driven, devices)) => {
                    let detail = format!("Connected; driving {} device(s).", devices.len());
                    live.set(OpenRgbState::Connected, Some(detail), Some(devices));
                    backoff = Duration::from_secs(1);
                    conn = Some((client, driven, settings.clone()));
                }
                Err(e) => {
                    let (state, detail) = failure(&e, settings.port);
                    live.set(state, Some(detail), Some(vec![]));
                    next_try = start + backoff;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                    continue;
                }
            }
        }
        let Some((client, driven, _)) = conn.as_mut() else { continue };
        let level = shared.level();
        let dark = level <= 0.0;
        // once faded out, send a few black frames and then idle (the devices hold the frame)
        let sent = if !dark || dark_frames < 3 { send_frame(shared, client, driven, level) } else { Ok(()) };
        dark_frames = if dark { dark_frames + 1 } else { 0 };
        match sent.and_then(|()| client.poll()) {
            Ok(false) => {}
            // OpenRGB's device list changed: its indexes are stale, so start over
            Ok(true) => {
                conn = None;
                next_try = Instant::now();
            }
            Err(e) => {
                let (state, detail) = failure(&e, settings.port);
                live.set(state, Some(detail), Some(vec![]));
                conn = None;
                next_try = Instant::now() + backoff;
            }
        }
        let target = if dark {
            Duration::from_millis(250)
        } else {
            Duration::from_secs_f32(1.0 / cfg.fps.clamp(5, MAX_FPS) as f32)
        };
        if let Some(rest) = (start + target).checked_duration_since(Instant::now()) {
            thread::sleep(rest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uncoil_openrgb::{Matrix, Zone};

    fn controller(kind: i32, name: &str, vendor: &str, zones: &[(i32, u32)]) -> Controller {
        let zones: Vec<Zone> = zones
            .iter()
            .enumerate()
            .map(|(i, &(kind, leds))| Zone { name: format!("Zone {i}"), kind, leds, matrix: None })
            .collect();
        let n: u32 = zones.iter().map(|z| z.leds).sum();
        Controller {
            kind,
            name: name.into(),
            vendor: vendor.into(),
            zones,
            leds: (0..n).map(|i| format!("LED {i}")).collect(),
            colors: n as usize,
            flags: 0,
        }
    }

    /// The maintainer's PC as OpenRGB might list it, plus things uncoil must leave alone.
    fn pc() -> Vec<Controller> {
        let mut hidden = controller(4, "Hidden strip", "", &[(1, 30)]);
        hidden.flags = uncoil_openrgb::CONTROLLER_FLAG_HIDDEN;
        let mut panel = controller(15, "Case panel", "", &[(2, 4)]);
        panel.zones[0].matrix = Some(Matrix { height: 2, width: 3, map: vec![0, 1, u32::MAX, 2, 3, u32::MAX] });
        vec![
            controller(2, "NVIDIA GeForce RTX 4070", "NVIDIA", &[(0, 1)]),
            controller(5, "Razer BlackWidow V4 Pro 75%", "Razer", &[(2, 99)]),
            controller(1, "Corsair Vengeance Pro RGB", "Corsair", &[(1, 10)]),
            controller(0, "ASUS ROG STRIX B550-F GAMING (WI-FI)", "ASUS", &[(1, 3), (1, 0), (0, 1)]),
            controller(1, "Corsair Vengeance Pro RGB", "Corsair", &[(1, 10)]),
            hidden,
            controller(4, "Empty header", "", &[(1, 0)]),
            panel,
        ]
    }

    #[test]
    fn plan_skips_razer_and_sorts_the_pc() {
        let planned = plan(&pc(), &OpenRgbLive::default(), false);
        let got: Vec<(u32, &str)> = planned.iter().map(|(i, s)| (*i, s.id.as_str())).collect();
        assert_eq!(
            got,
            [
                (3, "openrgb:asus-rog-strix-b550-f-gaming-wi-fi"),
                (2, "openrgb:corsair-vengeance-pro-rgb"),
                (4, "openrgb:corsair-vengeance-pro-rgb-2"),
                (0, "openrgb:nvidia-geforce-rtx-4070"),
                (7, "openrgb:case-panel"),
            ]
        );
        let board = &planned[0].1;
        assert_eq!(board.leds, 4);
        assert_eq!(
            board.zones.iter().map(|z| z.kind).collect::<Vec<_>>(),
            [ZoneKind::Linear, ZoneKind::Linear, ZoneKind::Single]
        );
        let panel = &planned[4].1.zones[0];
        assert_eq!(panel.kind, ZoneKind::Matrix);
        assert_eq!(panel.matrix.as_ref().unwrap().map, [Some(0), Some(1), None, Some(2), Some(3), None]);
        // every planned device builds a desk device with one LED per colour it takes
        for (_, s) in &planned {
            assert_eq!(s.def().matrix.unwrap().cols, s.leds as usize, "{}", s.id);
        }
    }

    #[test]
    fn plan_respects_exclusions_and_icue() {
        let live = OpenRgbLive { exclude: vec!["geforce".into()], ..OpenRgbLive::default() };
        let ids: Vec<String> = plan(&pc(), &live, true).into_iter().map(|(_, s)| s.id).collect();
        assert_eq!(ids, ["openrgb:asus-rog-strix-b550-f-gaming-wi-fi", "openrgb:case-panel"]);
    }

    #[test]
    fn failures_read_plainly() {
        let refused = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        let (state, detail) = failure(&refused, 6742);
        assert_eq!(state, OpenRgbState::Waiting);
        assert!(detail.contains("127.0.0.1:6742") && detail.contains("uncoil-openrgb"), "{detail}");
        let junk = std::io::Error::new(std::io::ErrorKind::InvalidData, "not an OpenRGB SDK packet");
        assert_eq!(failure(&junk, 6742).0, OpenRgbState::Error);
    }

    #[test]
    fn status_changes_bump_the_desk_only_when_devices_change() {
        let live = Live::default();
        let g = live.generation();
        live.set(OpenRgbState::Waiting, Some("waiting".into()), Some(vec![]));
        assert_eq!(live.generation(), g, "no devices before, none now");
        let planned: Vec<OpenRgbDeviceStatus> =
            plan(&pc(), &OpenRgbLive::default(), false).into_iter().map(|(_, s)| s).collect();
        live.set(OpenRgbState::Connected, None, Some(planned.clone()));
        assert_eq!(live.generation(), g + 1);
        assert_eq!(live.defs().len(), 5);
        live.set(OpenRgbState::Connected, None, Some(planned));
        assert_eq!(live.generation(), g + 1);
        let s = live.status();
        assert_eq!((s.state, s.devices.len()), (OpenRgbState::Connected, 5));
    }
}
