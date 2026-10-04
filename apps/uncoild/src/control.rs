//! Routes control-pipe requests. Daemon-level commands (`status`, `devices`, `capabilities`) are answered
//! here; device commands are queued to the device's own thread, which runs them between frames, so feature
//! I/O never interleaves with frame streaming.

use crate::exec::{self, Journal, Lighting};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uncoil_core::config::Status;
use uncoil_core::device::DeviceDef;
use uncoil_core::features::dial::DialMode;
use uncoil_core::features::hw_effect::HwEffect;
use uncoil_core::features::Feature;
use uncoil_core::ipc::{self, Capabilities, Command, DeviceInfo, KeyInfo, Raw, Request, Response};
use uncoil_core::proto::Transport;

/// How long a client waits for a device thread (a full key map dump takes about a second).
const DEVICE_TIMEOUT: Duration = Duration::from_secs(15);

pub enum JobKind {
    Command(Command),
    ProbeLighting,
}

pub struct Job {
    pub kind: JobKind,
    pub reply: Sender<Result<Raw, String>>,
}

/// A connected device, as seen by the control channel.
pub struct DeviceHandle {
    pub def: Arc<DeviceDef>,
    pub product_id: u16,
    pub connection: String,
    pub tx: Sender<Job>,
    /// Firmware effect currently showing instead of frames.
    pub hw: Arc<Mutex<Option<HwEffect>>>,
    token: u64,
}

#[derive(Default)]
pub struct Registry {
    devices: Mutex<HashMap<String, DeviceHandle>>,
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
        if d.get(&self.id).is_some_and(|h| h.token == self.token) {
            d.remove(&self.id);
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
    ) -> (Receiver<Job>, Registration) {
        let (tx, rx) = mpsc::channel();
        let token = self.next.fetch_add(1, Ordering::Relaxed);
        let id = def.id.clone();
        self.devices.lock().unwrap().insert(id.clone(), DeviceHandle { def, product_id, connection, tx, hw, token });
        (rx, Registration { registry: self.clone(), id, token })
    }

    /// Whether a device with this id is connected right now.
    pub fn is_connected(&self, id: &str) -> bool {
        self.devices.lock().unwrap().contains_key(id)
    }

    fn infos(&self) -> Vec<DeviceInfo> {
        let mut v: Vec<DeviceInfo> = self
            .devices
            .lock()
            .unwrap()
            .values()
            .map(|h| DeviceInfo {
                id: h.def.id.clone(),
                name: h.def.name.clone(),
                kind: h.def.kind,
                product_id: h.product_id,
                connection: h.connection.clone(),
                features: h.def.features.clone(),
                hw_effect: h.hw.lock().unwrap().clone(),
            })
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    fn send(&self, id: &str, kind: JobKind) -> Result<Raw, String> {
        let (tx, rx) = mpsc::channel();
        {
            let d = self.devices.lock().unwrap();
            let h = d.get(id).ok_or_else(|| format!("{id} is not connected"))?;
            h.tx.send(Job { kind, reply: tx }).map_err(|_| format!("{id} just disconnected"))?;
        }
        rx.recv_timeout(DEVICE_TIMEOUT).map_err(|_| format!("{id} did not answer in time"))?
    }
}

/// Run one queued job on the device thread. Returns what the renderer must do with the lighting.
pub fn serve_job(job: Job, t: &mut dyn Transport, def: &DeviceDef, tid: u8, journal: &Journal) -> Lighting {
    let (res, lighting) = match &job.kind {
        JobKind::ProbeLighting => {
            (exec::probe_lighting(t, tid).map(|p| ipc::raw(&p)).map_err(|e| format!("{e:#}")), Lighting::Unchanged)
        }
        JobKind::Command(cmd) => match exec::run(t, def, tid, cmd, journal) {
            Ok(out) => {
                for l in &out.log {
                    crate::log::line(l);
                }
                (Ok(out.result), out.lighting)
            }
            Err(e) => (Err(format!("{e:#}")), Lighting::Unchanged),
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
            Err(e) => Response::err(id, e),
        }
    }

    fn dispatch(&self, req: &Request) -> Result<Raw, String> {
        let cmd = req.command().map_err(|e| e.to_string())?;
        match &cmd {
            Command::Status => Ok(to(&*self.status.lock().unwrap())),
            Command::Devices => Ok(to(&self.registry.infos())),
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
                    out.push(capabilities(d, is_connected, probed));
                }
                if req.device.is_some() {
                    Ok(to(&out[0]))
                } else {
                    Ok(to(&out))
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
                        Ok(d) => return Err(format!("{} is not connected", d.name)),
                        Err(_) => return Err(e.to_string()),
                    },
                };
                self.registry.send(&id, JobKind::Command(cmd))
            }
        }
    }

    fn find_def(&self, q: &str) -> Result<&Arc<DeviceDef>, String> {
        let list: Vec<(&str, &str, uncoil_core::device::Kind)> =
            self.defs.iter().map(|d| (d.id.as_str(), d.name.as_str(), d.kind)).collect();
        let id = ipc::resolve_device(q, &list).map_err(|e| e.to_string())?;
        Ok(self.defs.iter().find(|d| d.id == id).expect("resolved id exists"))
    }
}

fn capabilities(d: &DeviceDef, connected: bool, probed: Option<ipc::LightingProbe>) -> Capabilities {
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
    }
}

fn to<T: serde::Serialize>(t: &T) -> Raw {
    ipc::raw(t)
}

/// A device thread without hardware: serves jobs from a fake device until the registry drops it.
#[cfg(any(test, feature = "fake"))]
pub fn spawn_fake(registry: &Arc<Registry>, def: Arc<DeviceDef>, journal: Arc<Journal>) {
    let hw = Arc::new(Mutex::new(None));
    let (rx, reg) = registry.register(def.clone(), def.usb[0].product_id, "fake".into(), hw.clone());
    let tid = def.usb[0].transaction_id;
    std::thread::spawn(move || {
        let _reg = reg;
        let mut dev = crate::fake::FakeDevice::for_def(&def);
        for job in rx {
            match serve_job(job, &mut dev, &def, tid, &journal) {
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
    use uncoil_core::device::builtin;
    use uncoil_core::ipc::{KeyMapping, WriteResult};

    fn control() -> Control {
        let defs: Vec<Arc<DeviceDef>> = builtin().into_iter().map(Arc::new).collect();
        let registry = Arc::new(Registry::default());
        let journal = Arc::new(Journal { path: None });
        for d in defs.iter().filter(|d| d.kind != uncoil_core::device::Kind::Mousemat) {
            spawn_fake(&registry, d.clone(), journal.clone());
        }
        Control { registry, defs, status: Arc::new(Mutex::new(Status { pid: 42, ..Default::default() })) }
    }

    fn req(cmd: &str, device: Option<&str>, args: Value) -> Request {
        serde_json::from_str(&json!({"id": 1, "cmd": cmd, "device": device, "args": args}).to_string()).unwrap()
    }

    #[test]
    fn daemon_level_commands() {
        let c = control();
        let r: Value = c.handle(req("status", None, Value::Null)).into_result().unwrap();
        assert_eq!(r["pid"], 42);
        let devs: Vec<DeviceInfo> = c.handle(req("devices", None, Value::Null)).into_result().unwrap();
        assert_eq!(devs.len(), 2);
        let caps: Capabilities =
            c.handle(req("capabilities", Some("keyboard"), json!({"probe": true}))).into_result().unwrap();
        assert!(caps.connected && caps.features.contains(&Feature::Dial));
        assert_eq!(caps.dial_modes.len(), 9);
        assert_eq!(caps.probed.unwrap().regions[0].cols, 18);
        let all: Vec<Capabilities> = c.handle(req("capabilities", None, Value::Null)).into_result().unwrap();
        assert_eq!(all.len(), 3);
        assert!(!all.iter().find(|c| c.kind == uncoil_core::device::Kind::Mousemat).unwrap().connected);
    }

    #[test]
    fn device_commands_go_through_the_device_thread() {
        let c = control();
        let m: KeyMapping =
            c.handle(req("keymap.get", Some("blackwidow"), json!({"key": "F9", "layer": "fn"}))).into_result().unwrap();
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
        let m: KeyMapping = c.handle(req("keymap.get", Some("mouse"), json!({"key": "CLUTCH"}))).into_result().unwrap();
        assert_eq!(m.function.to_string(), "dpi 5 1 144 1 144");
        // hardware effect shows up in `devices`
        assert!(c.handle(req("effect.hw", Some("mouse"), json!({"effect": "static #00ff00"}))).ok);
        let devs: Vec<DeviceInfo> = c.handle(req("devices", None, Value::Null)).into_result().unwrap();
        assert!(devs.iter().any(|d| d.hw_effect.is_some()));
    }

    #[test]
    fn errors_are_plain() {
        let c = control();
        let e = c.handle(req("keymap.get", Some("mat"), json!({"key": "P"})));
        assert_eq!(e.error.as_deref(), Some("Razer Goliathus Chroma Extended is not connected"));
        let e = c.handle(req("keymap.get", Some("headset"), json!({"key": "P"})));
        assert!(e.error.unwrap().contains("no device matches"));
        let e = c.handle(req("dial.get", Some("mouse"), json!({})));
        assert!(e.error.unwrap().contains("does not support dial"));
        let e = c.handle(req("keymap.get", None, json!({"key": "P"})));
        assert_eq!(e.error.as_deref(), Some("this command needs a device"));
        let e = c.handle(req("frobnicate", None, Value::Null));
        assert!(e.error.unwrap().contains("unknown command"));
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
