//! Routes control-pipe requests. Daemon-level commands (`status`, `devices`, `capabilities`) are answered
//! here; device commands are queued to the device's own thread, which runs them between frames, so feature
//! I/O never interleaves with frame streaming.

use crate::checks::{self, Checks};
use crate::exec::{self, Journal, Lighting};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uncoil_core::config::Status;
use uncoil_core::device::DeviceDef;
use uncoil_core::features::dial::DialMode;
use uncoil_core::features::hw_effect::HwEffect;
use uncoil_core::features::Feature;
use uncoil_core::ipc::{
    self, Capabilities, CodedError, Command, DeviceInfo, FeatureCheck, KeyInfo, Raw, Request, Response,
};
use uncoil_core::proto::Transport;

/// How long a client waits for a device thread (a full key map dump takes about a second).
const DEVICE_TIMEOUT: Duration = Duration::from_secs(15);
/// A job the device thread has not started by then is dropped, not run: its requester is about to be told
/// the device did not answer, and a GUI retry must not apply the same change twice. The gap to
/// [`DEVICE_TIMEOUT`] leaves a started job time to finish.
const START_DEADLINE: Duration = Duration::from_secs(10);

pub enum JobKind {
    Command(Command),
    ProbeLighting,
}

/// Why a request failed: plain words plus, for some cases, one of `ipc::codes`.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub code: Option<&'static str>,
    pub message: String,
}

impl From<String> for Failure {
    fn from(message: String) -> Failure {
        Failure { code: None, message }
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Failure {
        Failure { code: None, message: message.into() }
    }
}

impl From<anyhow::Error> for Failure {
    fn from(e: anyhow::Error) -> Failure {
        Failure { code: e.downcast_ref::<CodedError>().map(|c| c.code), message: format!("{e:#}") }
    }
}

pub struct Job {
    pub kind: JobKind,
    pub reply: Sender<Result<Raw, Failure>>,
    /// Not started by then: dropped (see [`START_DEADLINE`]).
    pub deadline: Instant,
}

impl Job {
    pub fn new(kind: JobKind, reply: Sender<Result<Raw, Failure>>) -> Job {
        Job { kind, reply, deadline: Instant::now() + START_DEADLINE }
    }
}

/// A connected device, as seen by the control channel.
pub struct DeviceHandle {
    pub def: Arc<DeviceDef>,
    pub product_id: u16,
    pub connection: String,
    pub tx: Sender<Job>,
    /// Firmware effect currently showing instead of frames.
    pub hw: Arc<Mutex<Option<HwEffect>>>,
    /// This connection's read-only check results (kept by the device thread).
    pub checks: Arc<Mutex<Vec<FeatureCheck>>>,
    token: u64,
}

/// Connected devices by id. A device seen on two endpoints at once (cable and dongle) has two handles; the
/// first one registered serves commands, and the other takes over when it goes away, so neither endpoint's
/// thread is left without a job queue.
#[derive(Default)]
pub struct Registry {
    devices: Mutex<HashMap<String, Vec<DeviceHandle>>>,
    next: AtomicU64,
}

/// Returned by [`Registry::register`]; unregisters on drop (the device thread holds it).
pub struct Registration {
    registry: Arc<Registry>,
    id: String,
    token: u64,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let mut d = self.registry.devices.lock().unwrap();
        if let Some(list) = d.get_mut(&self.id) {
            list.retain(|h| h.token != self.token);
            if list.is_empty() {
                d.remove(&self.id);
            }
        }
    }
}

impl Registry {
    /// Register a device thread; returns its job queue and the registration guard.
    pub fn register(
        self: &Arc<Self>,
        def: Arc<DeviceDef>,
        product_id: u16,
        connection: String,
        hw: Arc<Mutex<Option<HwEffect>>>,
        checks: Arc<Mutex<Vec<FeatureCheck>>>,
    ) -> (Receiver<Job>, Registration) {
        let (tx, rx) = mpsc::channel();
        let token = self.next.fetch_add(1, Ordering::Relaxed);
        let id = def.id.clone();
        self.devices.lock().unwrap().entry(id.clone()).or_default().push(DeviceHandle {
            def,
            product_id,
            connection,
            tx,
            hw,
            checks,
            token,
        });
        (rx, Registration { registry: self.clone(), id, token })
    }

    /// Whether a device with this id is connected right now.
    pub fn is_connected(&self, id: &str) -> bool {
        self.devices.lock().unwrap().contains_key(id)
    }

    /// Ids of every connected device, sorted.
    pub fn connected_ids(&self) -> Vec<String> {
        let mut v: Vec<String> = self.devices.lock().unwrap().keys().cloned().collect();
        v.sort();
        v
    }

    fn checks_of(&self, id: &str) -> Option<Vec<FeatureCheck>> {
        self.devices.lock().unwrap().get(id).and_then(|l| l.first()).map(|h| h.checks.lock().unwrap().clone())
    }

    fn infos(&self) -> Vec<DeviceInfo> {
        let mut v: Vec<DeviceInfo> = self
            .devices
            .lock()
            .unwrap()
            .values()
            .filter_map(|l| l.first())
            .map(|h| DeviceInfo {
                id: h.def.id.clone(),
                name: h.def.name.clone(),
                kind: h.def.kind,
                product_id: h.product_id,
                connection: h.connection.clone(),
                features: h.def.features.clone(),
                hw_effect: h.hw.lock().unwrap().clone(),
                support: h.def.support,
            })
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    /// Queue a probe without waiting for it (tests).
    #[cfg(test)]
    fn send_nowait(&self, id: &str) -> Result<(), String> {
        let (tx, _rx) = mpsc::channel();
        let d = self.devices.lock().unwrap();
        let h = d.get(id).and_then(|l| l.first()).ok_or("not connected")?;
        h.tx.send(Job::new(JobKind::ProbeLighting, tx)).map_err(|_| "gone".to_string())
    }

    fn send(&self, id: &str, kind: JobKind) -> Result<Raw, Failure> {
        let (tx, rx) = mpsc::channel();
        {
            let d = self.devices.lock().unwrap();
            let h = d.get(id).and_then(|l| l.first()).ok_or_else(|| format!("{id} is not connected"))?;
            h.tx.send(Job::new(kind, tx)).map_err(|_| format!("{id} just disconnected"))?;
        }
        rx.recv_timeout(DEVICE_TIMEOUT).map_err(|_| format!("{id} did not answer in time"))?
    }
}

/// Run one queued job on the device thread. Returns what the renderer must do with the lighting.
pub fn serve_job(
    job: Job,
    t: &mut dyn Transport,
    def: &DeviceDef,
    tid: u8,
    journal: &Journal,
    checks: &mut Checks,
) -> Lighting {
    if Instant::now() > job.deadline {
        let what = match &job.kind {
            JobKind::Command(c) => c.name(),
            JobKind::ProbeLighting => "lighting probe",
        };
        crate::log::line(&format!("{}: dropped a {what} that waited too long to start", def.name));
        let _ = job.reply.send(Err(format!("{} was busy for too long; nothing was done", def.name).into()));
        return Lighting::Unchanged;
    }
    let (res, lighting) = match &job.kind {
        JobKind::ProbeLighting => {
            (exec::probe_lighting(t, tid).map(|p| ipc::raw(&p)).map_err(Failure::from), Lighting::Unchanged)
        }
        JobKind::Command(cmd) => match exec::run(t, def, tid, cmd, journal, checks) {
            Ok(out) => {
                for l in &out.log {
                    crate::log::line(l);
                }
                (Ok(out.result), out.lighting)
            }
            Err(e) => {
                if cmd.policy(def).needs_write && cmd.write_confirmed() {
                    crate::log::line(&format!("ONBOARD WRITE NOT DONE {} ({}): {e:#}", def.id, cmd.name()));
                }
                (Err(Failure::from(e)), Lighting::Unchanged)
            }
        },
    };
    let _ = job.reply.send(res);
    lighting
}

/// The request router behind the pipe.
pub struct Control {
    pub registry: Arc<Registry>,
    pub defs: Vec<Arc<DeviceDef>>,
    /// Latest status snapshot from the main loop.
    pub status: Arc<Mutex<Status>>,
}

impl Control {
    pub fn handle(&self, req: Request) -> Response {
        let id = req.id.clone();
        match self.dispatch(&req) {
            Ok(v) => Response::ok(id, v),
            Err(e) => Response::err_code(id, e.code, e.message),
        }
    }

    fn dispatch(&self, req: &Request) -> Result<Raw, Failure> {
        let cmd = req.command().map_err(|e| e.to_string())?;
        match &cmd {
            Command::Status => Ok(ipc::raw(&*self.status.lock().unwrap())),
            Command::Devices => Ok(ipc::raw(&self.registry.infos())),
            Command::Capabilities(a) => {
                let connected: Vec<String> = self.registry.infos().into_iter().map(|d| d.id).collect();
                let defs: Vec<&Arc<DeviceDef>> = match &req.device {
                    Some(q) => vec![self.find_def(q)?],
                    None => self.defs.iter().collect(),
                };
                let mut out = vec![];
                for d in defs {
                    let is_connected = connected.contains(&d.id);
                    let probed = if a.probe && is_connected && d.has(Feature::Lighting) {
                        self.registry
                            .send(&d.id, JobKind::ProbeLighting)
                            .ok()
                            .and_then(|v| serde_json::from_str(v.get()).ok())
                    } else {
                        None
                    };
                    let checks = self.registry.checks_of(&d.id).unwrap_or_else(|| checks::initial(d));
                    out.push(capabilities(d, is_connected, probed, checks));
                }
                if req.device.is_some() {
                    Ok(ipc::raw(&out[0]))
                } else {
                    Ok(ipc::raw(&out))
                }
            }
            _ => {
                let q = req.device.as_deref().ok_or("this command needs a device")?;
                let infos = self.registry.infos();
                let list: Vec<(&str, &str, uncoil_core::device::Kind)> =
                    infos.iter().map(|d| (d.id.as_str(), d.name.as_str(), d.kind)).collect();
                let id = match ipc::resolve_device(q, &list) {
                    Ok(id) => id.to_string(),
                    // known device that is not plugged in: say so plainly
                    Err(e) => match self.find_def(q) {
                        Ok(d) => return Err(format!("{} is not connected", d.name).into()),
                        Err(_) => return Err(e.to_string().into()),
                    },
                };
                self.registry.send(&id, JobKind::Command(cmd))
            }
        }
    }

    /// A definition by id, kind or part of the name: among the connected devices first (so "keyboard"
    /// means the plugged-in keyboard, not one of the many known ones), then among every known device.
    fn find_def(&self, q: &str) -> Result<&Arc<DeviceDef>, String> {
        let connected = self.registry.connected_ids();
        let list = |only_connected: bool| -> Vec<(&str, &str, uncoil_core::device::Kind)> {
            self.defs
                .iter()
                .filter(|d| !only_connected || connected.contains(&d.id))
                .map(|d| (d.id.as_str(), d.name.as_str(), d.kind))
                .collect()
        };
        let id = match ipc::resolve_device(q, &list(true)) {
            Ok(id) => id,
            Err(_) => ipc::resolve_device(q, &list(false)).map_err(|e| e.to_string())?,
        };
        Ok(self.defs.iter().find(|d| d.id == id).expect("resolved id exists"))
    }
}

fn capabilities(
    d: &DeviceDef,
    connected: bool,
    probed: Option<ipc::LightingProbe>,
    checks: Vec<FeatureCheck>,
) -> Capabilities {
    let km = d.keymap.as_ref();
    Capabilities {
        id: d.id.clone(),
        name: d.name.clone(),
        kind: d.kind,
        connected,
        features: d.features.clone(),
        hw_effects: d.hw_effects.as_ref().map(|h| h.effects.clone()).unwrap_or_default(),
        keymap_layers: km.map(|k| k.layers.clone()).unwrap_or_default(),
        keys: km
            .map(|k| k.keys.iter().map(|k| KeyInfo { id: k.id, name: k.name.clone(), led: k.led.clone() }).collect())
            .unwrap_or_default(),
        dial_modes: if d.has(Feature::Dial) { DialMode::ALL.to_vec() } else { vec![] },
        probed,
        support: d.support,
        checks,
        unverified: d.unverified.clone(),
    }
}

/// A device thread without hardware: serves jobs from a fake device until the registry drops it.
#[cfg(any(test, feature = "fake"))]
pub fn spawn_fake(registry: &Arc<Registry>, def: Arc<DeviceDef>, journal: Arc<Journal>) {
    let hw = Arc::new(Mutex::new(None));
    let mut checks = Checks::new(&def);
    let (rx, reg) = registry.register(def.clone(), def.usb[0].product_id, "fake".into(), hw.clone(), checks.mirror());
    let tid = def.usb[0].transaction_id;
    std::thread::spawn(move || {
        let _reg = reg;
        let mut dev = crate::fake::FakeDevice::for_def(&def);
        for job in rx {
            match serve_job(job, &mut dev, &def, tid, &journal, &mut checks) {
                Lighting::Hardware(e) => *hw.lock().unwrap() = Some(e),
                Lighting::Software => *hw.lock().unwrap() = None,
                Lighting::Unchanged => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use uncoil_core::ipc::{KeyMapping, WriteResult};

    /// The same devices `uncoild --fake` serves: keyboard, Basilisk and the experimental DeathAdder.
    fn control() -> Control {
        let defs = crate::fake::fake_defs();
        let registry = Arc::new(Registry::default());
        let journal = Arc::new(Journal { path: None });
        for d in crate::fake::connected_fakes(&defs) {
            spawn_fake(&registry, d, journal.clone());
        }
        let status = Status { pid: 42, unknown_devices: crate::fake::unknown_devices(), ..Default::default() };
        Control { registry, defs, status: Arc::new(Mutex::new(status)) }
    }

    fn req(cmd: &str, device: Option<&str>, args: Value) -> Request {
        serde_json::from_str(&json!({"id": 1, "cmd": cmd, "device": device, "args": args}).to_string()).unwrap()
    }

    #[test]
    fn daemon_level_commands() {
        let c = control();
        let r: Value = c.handle(req("status", None, Value::Null)).into_result().unwrap();
        assert_eq!(r["pid"], 42);
        assert_eq!(r["unknown_devices"], json!([{"product_id": 0x0FFE, "interfaces": [0, 1, 2]}]));
        assert!(c.defs.iter().all(|d| d.endpoint_for(0x0FFE).is_none()), "the fake unknown device must be unknown");
        let devs: Vec<DeviceInfo> = c.handle(req("devices", None, Value::Null)).into_result().unwrap();
        assert_eq!(devs.len(), 3);
        let da = devs.iter().find(|d| d.id == "razer-deathadder-v3-pro").unwrap();
        assert_eq!(da.support, uncoil_core::device::Support::Experimental);
        assert!(!da.features.contains(&Feature::Lighting));
        let caps: Capabilities =
            c.handle(req("capabilities", Some("keyboard"), json!({"probe": true}))).into_result().unwrap();
        assert!(caps.connected && caps.features.contains(&Feature::Dial));
        assert_eq!(caps.dial_modes.len(), 9);
        assert_eq!(caps.probed.unwrap().regions[0].cols, 18);
        assert!(caps.checks.iter().all(|c| c.state == ipc::CheckState::NotNeeded));
        let all: Vec<Capabilities> = c.handle(req("capabilities", None, Value::Null)).into_result().unwrap();
        assert_eq!(all.len(), c.defs.len());
        assert!(!all.iter().find(|c| c.id == "razer-goliathus-chroma-extended").unwrap().connected);
        // the Basilisk's unverified features and the experimental mouse start untested
        let b: Capabilities = c.handle(req("capabilities", Some("basilisk"), Value::Null)).into_result().unwrap();
        assert_eq!(b.unverified, vec![Feature::Dpi, Feature::PollRate, Feature::Power]);
        let state = |caps: &Capabilities, f: Feature| caps.checks.iter().find(|c| c.feature == f).unwrap().state;
        assert_eq!(state(&b, Feature::Dpi), ipc::CheckState::Untested);
        assert_eq!(state(&b, Feature::Keymap), ipc::CheckState::NotNeeded);
        let da: Capabilities =
            c.handle(req("capabilities", Some("deathadder v3 pro"), Value::Null)).into_result().unwrap();
        assert!(da.checks.iter().all(|c| c.state == ipc::CheckState::Untested));
        // check.run passes them, and capabilities shows it
        let ran: Vec<FeatureCheck> =
            c.handle(req("check.run", Some("deathadder v3 pro"), Value::Null)).into_result().unwrap();
        assert!(ran.iter().all(|c| c.state == ipc::CheckState::Passed), "{ran:?}");
        let da: Capabilities =
            c.handle(req("capabilities", Some("deathadder v3 pro"), Value::Null)).into_result().unwrap();
        assert_eq!(da.checks, ran);
    }

    #[test]
    fn codes_travel_in_the_response() {
        let c = control();
        let e = c.handle(req("dial.get", Some("basilisk"), json!({})));
        assert_eq!(e.code.as_deref(), Some(ipc::codes::NOT_SUPPORTED));
        let e = c.handle(req(
            "keymap.set",
            Some("basilisk"),
            json!({"key": "LEFT_CLICK", "function": "button 2", "write": true}),
        ));
        assert_eq!(e.code.as_deref(), Some(ipc::codes::LEFT_CLICK_GUARD));
        assert!(e.error.unwrap().starts_with("This would leave no button that left-clicks."));
        let ok = c.handle(req("performance.get", Some("basilisk"), Value::Null));
        assert!(ok.ok && ok.code.is_none());
    }

    #[test]
    fn device_commands_go_through_the_device_thread() {
        let c = control();
        let m: KeyMapping = c
            .handle(req("keymap.get", Some("blackwidow v4 pro 75"), json!({"key": "F9", "layer": "fn"})))
            .into_result()
            .unwrap();
        assert_eq!(m.function.to_string(), "razer 4");
        let w: WriteResult<KeyMapping> = c
            .handle(req(
                "keymap.set",
                Some("keyboard"),
                json!({"key": "P", "layer": "fn", "function": "key PRINT_SCREEN", "write": true}),
            ))
            .into_result()
            .unwrap();
        assert!(w.verified);
        // mouse: the clutch button
        let m: KeyMapping =
            c.handle(req("keymap.get", Some("basilisk"), json!({"key": "CLUTCH"}))).into_result().unwrap();
        assert_eq!(m.function.to_string(), "dpi 5 1 144 1 144");
        // hardware effect shows up in `devices`
        assert!(c.handle(req("effect.hw", Some("basilisk"), json!({"effect": "static #00ff00"}))).ok);
        let devs: Vec<DeviceInfo> = c.handle(req("devices", None, Value::Null)).into_result().unwrap();
        assert!(devs.iter().any(|d| d.hw_effect.is_some()));
    }

    #[test]
    fn errors_are_plain() {
        let c = control();
        let e = c.handle(req("keymap.get", Some("goliathus"), json!({"key": "P"})));
        assert_eq!(e.error.as_deref(), Some("Razer Goliathus Chroma Extended is not connected"));
        let e = c.handle(req("keymap.get", Some("headset"), json!({"key": "P"})));
        assert!(e.error.unwrap().contains("no device matches"));
        let e = c.handle(req("dial.get", Some("basilisk"), json!({})));
        assert!(e.error.unwrap().contains("does not support dial"));
        let e = c.handle(req("keymap.get", None, json!({"key": "P"})));
        assert_eq!(e.error.as_deref(), Some("this command needs a device"));
        let e = c.handle(req("frobnicate", None, Value::Null));
        assert!(e.error.unwrap().contains("unknown command"));
    }

    /// The browser mock (`apps/uncoil/src/lib/mock/daemon/*.json`) is these fake-daemon answers, the same
    /// JSON `uncoil --json --pipe \\.\pipe\uncoil-fake …` prints, and nothing else: every file in that folder
    /// must be listed here and match. Regenerate with `UNCOIL_UPDATE_MOCK=1 cargo test -p uncoild gui_mock`.
    #[test]
    fn gui_mock_from_fake_answers() {
        let c = control();
        let kb = "razer-blackwidow-v4-pro-75";
        let mouse = "razer-basilisk-v3-pro";
        let da = crate::fake::DEATHADDER_ID;
        let files: [(&str, Option<&str>, &str, Value); 19] = [
            ("devices", None, "devices", Value::Null),
            ("caps-keyboard", Some(kb), "capabilities", Value::Null),
            ("caps-mouse", Some(mouse), "capabilities", Value::Null),
            ("caps-deathadder", Some(da), "capabilities", Value::Null),
            ("keymap-keyboard-normal", Some(kb), "keymap.dump", json!({"layer": "normal"})),
            ("keymap-keyboard-hypershift", Some(kb), "keymap.dump", json!({"layer": "hypershift"})),
            ("keymap-mouse-normal", Some(mouse), "keymap.dump", json!({"layer": "normal"})),
            ("keymap-mouse-hypershift", Some(mouse), "keymap.dump", json!({"layer": "hypershift"})),
            ("keymap-deathadder-normal", Some(da), "keymap.dump", json!({"layer": "normal"})),
            ("keymap-deathadder-hypershift", Some(da), "keymap.dump", json!({"layer": "hypershift"})),
            ("profiles-keyboard", Some(kb), "profile.list", Value::Null),
            ("profiles-mouse", Some(mouse), "profile.list", Value::Null),
            ("profiles-deathadder", Some(da), "profile.list", Value::Null),
            ("dial", Some(kb), "dial.get", json!({})),
            ("oled", Some(kb), "oled.get", Value::Null),
            ("performance-mouse", Some(mouse), "performance.get", Value::Null),
            ("performance-deathadder", Some(da), "performance.get", Value::Null),
            ("power-mouse", Some(mouse), "power.get", Value::Null),
            ("power-deathadder", Some(da), "power.get", Value::Null),
        ];
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../uncoil/src/lib/mock/daemon");
        let write = std::env::var_os("UNCOIL_UPDATE_MOCK").is_some();
        let mut stale = vec![];
        for (name, device, cmd, args) in &files {
            let v: Value =
                c.handle(req(cmd, *device, args.clone())).into_result().unwrap_or_else(|e| panic!("{name}: {e}"));
            let path = dir.join(format!("{name}.json"));
            let real = serde_json::to_string_pretty(&v).unwrap() + "\n";
            if write {
                std::fs::write(&path, &real).unwrap();
            }
            if std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n") != real {
                stale.push(path.display().to_string());
            }
        }
        // nothing hand-written next to them
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !files.iter().any(|(f, ..)| format!("{f}.json") == name) {
                stale.push(format!("{name} (not generated by this test)"));
            }
        }
        assert!(stale.is_empty(), "stale browser mock files; rerun with UNCOIL_UPDATE_MOCK=1:\n{}", stale.join("\n"));
    }

    #[test]
    fn jobs_that_waited_too_long_are_dropped_not_run() {
        let def = crate::fake::deathadder();
        let mut dev = crate::fake::FakeDevice::for_def(&def);
        let journal = Journal { path: None };
        let mut checks = Checks::new(&def);
        let (tx, rx) = mpsc::channel();
        let cmd = Command::from_parts("power.set", Some(r#"{"idle_s": 300, "write": true}"#)).unwrap();
        let mut job = Job::new(JobKind::Command(cmd.clone()), tx.clone());
        job.deadline = Instant::now() - Duration::from_millis(1);
        serve_job(job, &mut dev, &def, 0x1F, &journal, &mut checks);
        assert!(rx.recv().unwrap().is_err());
        assert!(dev.setters().is_empty(), "an expired write never reaches the device");
        assert!(dev.sent().is_empty(), "nor does its check");
        // on time, the same job runs
        serve_job(Job::new(JobKind::Command(cmd), tx), &mut dev, &def, 0x1F, &journal, &mut checks);
        assert!(rx.recv().unwrap().is_ok());
        assert!(!dev.setters().is_empty());
    }

    #[test]
    fn a_second_endpoint_does_not_orphan_the_first() {
        let registry = Arc::new(Registry::default());
        let def = Arc::new(crate::fake::deathadder());
        let reg = |r: &Arc<Registry>| {
            r.register(def.clone(), 1, "x".into(), Arc::new(Mutex::new(None)), Arc::new(Mutex::new(vec![])))
        };
        let (rx1, reg1) = reg(&registry);
        let (rx2, reg2) = reg(&registry);
        // commands go to the first endpoint, which keeps its queue
        let _ = registry.send_nowait(&def.id);
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_err());
        // the second going away changes nothing for the first
        drop(reg2);
        assert!(registry.is_connected(&def.id));
        let _ = registry.send_nowait(&def.id);
        assert!(rx1.try_recv().is_ok());
        // the first going away hands the device to whoever is left; then it is gone
        let (rx3, reg3) = reg(&registry);
        drop(reg1);
        let _ = registry.send_nowait(&def.id);
        assert!(rx3.try_recv().is_ok());
        drop(reg3);
        assert!(!registry.is_connected(&def.id));
    }

    #[cfg(windows)]
    #[test]
    fn full_stack_over_a_real_pipe() {
        use uncoil_core::features::keymap::Layer;
        use uncoil_core::ipc::{Client, KeyArgs};
        let name = format!(r"\\.\pipe\uncoil-test-{}-fullstack", std::process::id());
        let c = Arc::new(control());
        crate::pipe::serve(&name, Arc::new(move |r| c.handle(r))).unwrap();
        let mut client = Client::connect_to(&name).unwrap();
        let r = client
            .call(
                Some("keyboard"),
                &Command::KeymapGet(KeyArgs { key: "Delete".into(), layer: Layer::Hypershift, profile: 1 }),
            )
            .unwrap();
        let m: KeyMapping = r.into_result().unwrap();
        assert_eq!(m.function.to_string(), "razer 76");
    }
}
