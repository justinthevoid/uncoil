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
//! Razer devices that OpenRGB reports are always left alone (uncoil drives them itself), as are hidden ones
//! and names matched by `openrgb.live.exclude`. Devices a running program lights itself (iCUE, Armoury
//! Crate, ...: `uncoil_core::owners`) are left to it and listed as `held`; the running programs are looked
//! at every few seconds, so a device is let go soon after its program starts and taken back after it quits.
//! An OpenRGB that is not uncoil's own server is never driven.

use crate::{log, Shared};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use uncoil_core::config::{OpenRgbLive, OpenRgbMode};
use uncoil_core::device::DeviceDef;
use uncoil_core::ipc::{OpenRgbDeviceStatus, OpenRgbHeld, OpenRgbState, OpenRgbStatus};
use uncoil_core::layout::{self, ExternalZone, ZoneKind, ZoneMatrix};
use uncoil_core::owners::{self, Part};
use uncoil_openrgb::{device_type, Client, Controller};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(10);
/// Resend an unchanged frame this often (a device may have been reset meanwhile).
const REFRESH: Duration = Duration::from_secs(1);
/// OpenRGB devices are slower than USB ones; no point sending more often than this.
const MAX_FPS: u32 = 30;
/// How often to look whether a program that lights PC parts itself started or quit (one process snapshot).
const OWNER_CHECK: Duration = Duration::from_secs(3);

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

    /// New state (logged when it changes); `devices` replaces the driven and the held devices when given.
    fn set(
        &self,
        state: OpenRgbState,
        detail: Option<String>,
        devices: Option<(Vec<OpenRgbDeviceStatus>, Vec<OpenRgbHeld>)>,
    ) {
        let mut s = self.state.lock().unwrap();
        if (s.status.state, &s.status.detail) != (state, &detail) {
            let what = detail.clone().unwrap_or_else(|| format!("{state:?}").to_lowercase());
            log::line(&format!("OpenRGB live: {what}"));
        }
        s.status.state = state;
        s.status.detail = detail;
        if let Some((devices, held)) = devices {
            s.status.held = held;
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

/// A controller as `owners` sees it: its OpenRGB type, name and vendor.
type PartOf = (i32, String, String);

fn part_of(c: &Controller) -> PartOf {
    (c.kind, c.name.clone(), c.vendor.clone())
}

/// For each part, the running program that lights it itself, if any (`owners`).
fn holders<S: AsRef<str>>(parts: &[PartOf], processes: &[S]) -> Vec<Option<String>> {
    parts
        .iter()
        .map(|(kind, name, vendor)| {
            let part = Part { kind: device_type::word(*kind), name, vendor };
            owners::holder(owners::shipped(), processes, part).map(|o| o.name.clone())
        })
        .collect()
}

/// What [`plan`] decides: the devices to drive and the ones left to another program, each with its index on
/// the server.
type Plan = (Vec<(u32, OpenRgbDeviceStatus)>, Vec<(u32, OpenRgbHeld)>);

/// Which OpenRGB devices to drive, in desk order (motherboards, RAM, GPUs, the rest), with their status
/// entries (ids `openrgb:<slug>`, made unique with `-2`, `-3`, ...), and the ones left to the program in
/// `holders` (one entry per controller), with their indexes.
fn plan(controllers: &[Controller], live: &OpenRgbLive, holders: &[Option<String>]) -> Plan {
    let mut held = Vec::new();
    let mut picked: Vec<(u32, &Controller)> = Vec::new();
    for (i, c) in controllers.iter().enumerate() {
        let razer = [&c.name, &c.vendor].iter().any(|s| s.to_lowercase().contains("razer"));
        if razer
            || live.excludes(&c.name)
            || c.flags & uncoil_openrgb::CONTROLLER_FLAG_HIDDEN != 0
            || !c.zones.iter().any(|z| z.leds > 0)
        {
            continue;
        }
        match holders.get(i).cloned().flatten() {
            Some(by) => held.push((i as u32, OpenRgbHeld { name: c.name.clone(), by })),
            None => picked.push((i as u32, c)),
        }
    }
    let rank = |k: i32| match k {
        device_type::MOTHERBOARD => 0,
        device_type::DRAM => 1,
        device_type::GPU => 2,
        _ => 3,
    };
    picked.sort_by_key(|(_, c)| rank(c.kind));
    let mut ids: Vec<String> = Vec::new();
    let planned = picked
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
        .collect();
    (planned, held)
}

/// The status line for a connection: how many devices are driven and which program has the rest.
fn connected_detail(driving: usize, held: &[OpenRgbHeld]) -> String {
    let mut by: Vec<(&str, usize)> = Vec::new();
    for h in held {
        match by.iter_mut().find(|(name, _)| *name == h.by) {
            Some((_, n)) => *n += 1,
            None => by.push((&h.by, 1)),
        }
    }
    let left: Vec<String> = by.iter().map(|(name, n)| format!("; {n} left to {name}")).collect();
    format!("Connected; driving {driving} device(s){}.", left.concat())
}

/// A live connection: the client, the devices it drives, the settings it was made for, every controller as
/// a part (to look again at who holds what without asking OpenRGB) and who held each one then.
struct Connection {
    client: Client,
    driven: Vec<Driven>,
    made_for: OpenRgbLive,
    parts: Vec<PartOf>,
    holders: Vec<Option<String>>,
}

/// A held device with the part it is, so it can be recognised when OpenRGB lists it again.
type HeldPart = (PartOf, OpenRgbHeld);

/// The held devices after a new connection: the ones held now (`held_now`), plus the ones held before that
/// OpenRGB no longer lists while their program still runs (uncoil's OpenRGB turns off the detectors a
/// running program claims, so they drop out of its list), with who holds them as of `processes`.
fn carry_over<S: AsRef<str>>(
    before: Vec<HeldPart>,
    listed: &[PartOf],
    held_now: Vec<HeldPart>,
    processes: &[S],
) -> Vec<HeldPart> {
    let mut held = held_now;
    for (part, h) in before {
        if listed.contains(&part) {
            continue;
        }
        if let Some(by) = holders(std::slice::from_ref(&part), processes).pop().flatten() {
            held.push((part, OpenRgbHeld { by, ..h }));
        }
    }
    held
}

/// Connect, list the devices and put the ones to drive in their per-LED mode. Only uncoil's own server is
/// driven: another OpenRGB (its own Windows service, say) has the Razer detectors and the exclusions on.
/// Also returns the status entries of the driven and the held devices.
fn connect(live: &OpenRgbLive) -> std::io::Result<(Connection, Vec<OpenRgbDeviceStatus>, Vec<HeldPart>)> {
    let port = live.valid_port().ok_or_else(|| std::io::Error::other("openrgb.live.port must be 1024-65535"))?;
    let mut client = Client::connect(port, "uncoil", CONNECT_TIMEOUT)?;
    if !crate::openrgb::ours() {
        return Err(std::io::Error::other(NOT_OURS));
    }
    let controllers = client.controllers()?;
    let parts: Vec<PartOf> = controllers.iter().map(part_of).collect();
    let holders = holders(&parts, &crate::conflicts::process_names());
    let (planned, held) = plan(&controllers, live, &holders);
    let mut driven = Vec::new();
    for (index, status) in &planned {
        client.set_custom_mode(*index)?;
        let colors = controllers[*index as usize].colors;
        let names = layout::external_led_names(&status.zones);
        driven.push(Driven { index: *index, id: status.id.clone(), names, colors, last: vec![], sent: None });
    }
    let held = held.into_iter().map(|(i, h)| (parts[i as usize].clone(), h)).collect();
    let conn = Connection { client, driven, made_for: live.clone(), parts, holders };
    Ok((conn, planned.into_iter().map(|(_, s)| s).collect(), held))
}

/// Why a server that answers is not driven.
const NOT_OURS: &str = "this OpenRGB is not uncoil's own server (OpenRGB's Windows service or one you started?), so \
     uncoil won't drive it. Close it; the uncoil-openrgb task starts uncoil's at sign-in.";

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
    // the connection, the devices it drives, the settings it was made for and iCUE then (see `Connected`)
    let mut conn: Option<Connection> = None;
    let mut backoff = Duration::from_secs(1);
    let mut next_try = Instant::now();
    let mut dark_frames = 0u32;
    let mut owners_checked = Instant::now();
    // who held what at the last look, when it differed from the connection's: acted on only when the next
    // look agrees, so a program that is starting up (or flickers in and out) doesn't make devices bounce
    let mut changing: Option<Vec<Option<String>>> = None;
    // the devices held at the last connection, kept across reconnections (see `carry_over`)
    let mut held: Vec<HeldPart> = Vec::new();
    loop {
        let start = Instant::now();
        let cfg = shared.config();
        if cfg.openrgb_mode() != OpenRgbMode::Live {
            conn = None;
            held.clear();
            if live.state.lock().unwrap().status.state != OpenRgbState::Off {
                live.set(OpenRgbState::Off, None, Some(Default::default()));
            }
            thread::sleep(Duration::from_secs(1));
            continue;
        }
        let settings = cfg.openrgb_live();
        // new port or exclusions: list the devices again
        if conn.as_ref().is_some_and(|c| c.made_for != settings) {
            conn = None;
            next_try = start;
        }
        // a program that lights PC parts started or quit: plan again, so its devices are let go (or taken
        // back) within seconds
        if start.duration_since(owners_checked) >= OWNER_CHECK {
            owners_checked = start;
            if let Some(c) = &conn {
                let now = holders(&c.parts, &crate::conflicts::process_names());
                if now == c.holders {
                    changing = None;
                } else if changing.as_ref() == Some(&now) {
                    changing = None;
                    conn = None;
                    next_try = start;
                } else {
                    changing = Some(now);
                }
            }
        }
        if conn.is_none() {
            if start < next_try {
                thread::sleep((next_try - start).min(Duration::from_millis(250)));
                continue;
            }
            match connect(&settings) {
                Ok((c, devices, held_now)) => {
                    let processes = crate::conflicts::process_names();
                    held = carry_over(std::mem::take(&mut held), &c.parts, held_now, &processes);
                    let held: Vec<OpenRgbHeld> = held.iter().map(|(_, h)| h.clone()).collect();
                    let detail = connected_detail(devices.len(), &held);
                    live.set(OpenRgbState::Connected, Some(detail), Some((devices, held)));
                    backoff = Duration::from_secs(1);
                    changing = None;
                    conn = Some(c);
                }
                Err(e) => {
                    let (state, detail) = failure(&e, settings.port);
                    live.set(state, Some(detail), Some(Default::default()));
                    next_try = start + backoff;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                    continue;
                }
            }
        }
        let Some(Connection { client, driven, .. }) = conn.as_mut() else { continue };
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
                live.set(state, Some(detail), Some(Default::default()));
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

    fn nobody(controllers: &[Controller]) -> Vec<Option<String>> {
        vec![None; controllers.len()]
    }

    #[test]
    fn plan_skips_razer_and_sorts_the_pc() {
        let pc = pc();
        let (planned, held) = plan(&pc, &OpenRgbLive::default(), &nobody(&pc));
        assert!(held.is_empty());
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
    fn plan_respects_exclusions_and_leaves_held_devices_to_their_program() {
        let pc = pc();
        let live = OpenRgbLive { exclude: vec!["geforce".into()], ..OpenRgbLive::default() };
        let parts: Vec<PartOf> = pc.iter().map(part_of).collect();
        // iCUE running: both Corsair sticks are its; the Razer keyboard is never anyone's business but uncoil's
        let icue = holders(&parts, &["iCUE.exe"]);
        let (planned, held) = plan(&pc, &live, &icue);
        let ids: Vec<String> = planned.into_iter().map(|(_, s)| s.id).collect();
        assert_eq!(ids, ["openrgb:asus-rog-strix-b550-f-gaming-wi-fi", "openrgb:case-panel"]);
        let stick = OpenRgbHeld { name: "Corsair Vengeance Pro RGB".into(), by: "Corsair iCUE".into() };
        assert_eq!(held, [(2, stick.clone()), (4, stick)]);
        let held: Vec<OpenRgbHeld> = held.into_iter().map(|(_, h)| h).collect();
        assert_eq!(connected_detail(2, &held), "Connected; driving 2 device(s); 2 left to Corsair iCUE.");
        // nothing running: nothing held
        assert_eq!(holders(&parts, &["explorer.exe"]), nobody(&pc));
        assert_eq!(connected_detail(5, &[]), "Connected; driving 5 device(s).");
    }

    #[test]
    fn held_devices_stay_listed_while_their_program_runs() {
        let hub: PartOf = (4, "Corsair iCUE Link System Hub".into(), "Corsair".into());
        let board: PartOf = (0, "ASUS ROG STRIX B650E-F GAMING WIFI".into(), "ASUS".into());
        let held = |p: &PartOf| (p.clone(), OpenRgbHeld { name: p.1.clone(), by: "Corsair iCUE".into() });
        // iCUE took the hub; OpenRGB restarted without its detector, so only the board is listed now
        let after = carry_over(vec![held(&hub)], std::slice::from_ref(&board), vec![], &["iCUE.exe"]);
        assert_eq!(after, [held(&hub)]);
        // iCUE quit: the hub is back in OpenRGB's list (and driven), or gone for good; either way not held
        assert!(carry_over(after.clone(), &[board.clone(), hub.clone()], vec![], &["iCUE.exe"]).is_empty());
        assert!(carry_over(after, std::slice::from_ref(&board), vec![], &["explorer.exe"]).is_empty());
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
        live.set(OpenRgbState::Waiting, Some("waiting".into()), Some(Default::default()));
        assert_eq!(live.generation(), g, "no devices before, none now");
        let pc = pc();
        let planned: Vec<OpenRgbDeviceStatus> =
            plan(&pc, &OpenRgbLive::default(), &nobody(&pc)).0.into_iter().map(|(_, s)| s).collect();
        live.set(OpenRgbState::Connected, None, Some((planned.clone(), vec![])));
        assert_eq!(live.generation(), g + 1);
        assert_eq!(live.defs().len(), 5);
        // only who holds what changed: the status says so, the desk stays
        let held = vec![OpenRgbHeld { name: "Corsair iCUE Link System Hub".into(), by: "Corsair iCUE".into() }];
        live.set(OpenRgbState::Connected, None, Some((planned, held.clone())));
        assert_eq!(live.generation(), g + 1);
        let s = live.status();
        assert_eq!((s.state, s.devices.len(), s.held), (OpenRgbState::Connected, 5, held));
    }
}
