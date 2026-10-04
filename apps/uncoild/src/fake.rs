//! A fake BlackWidow V4 Pro 75% / Basilisk V3 Pro that answers the feature commands the way the real ones
//! did in read-only probes (2026-10-03), plus an experimental DeathAdder V3 Pro. DPI, poll rate, power,
//! scroll wheel, firmware version and keyboard layout answers are made-up but plausible values (nobody has
//! read them on these devices yet). Used by the tests
//! and by `uncoild --fake` (cargo feature `fake`), which serves the control pipe without touching any
//! hardware.

use std::collections::HashMap;
use uncoil_core::device::{DeviceDef, UsbEndpoint};
use uncoil_core::features::keymap::{Function, KeymapDef};
use uncoil_core::features::performance::{Dpi, DpiStages, PollKind};
use uncoil_core::features::Feature;
use uncoil_core::proto::{Reply, Report, Status, Transport, MAX_ARGS};

/// The experimental mouse `uncoild --fake` adds. Used only when `devices/experimental/` has no file for it;
/// the fake normally serves the real file, and its DPI, poll-rate and power answers follow that file's
/// sections (only the current values are made up).
const DEATHADDER: &str = r#"
id = "razer-deathadder-v3-pro"
name = "Razer DeathAdder V3 Pro"
kind = "mouse"
vendor_id = 0x1532
support = "experimental"
features = ["dpi", "poll_rate", "power", "keymap"]

[[usb]]
product_id = 0x00B6
connection = "wired"
interface = 0
usage_page = 0x01
usage = 0x02
transaction_id = 0x1F

[dpi]
min = 100
max = 30000
stages_max = 5

[poll_rate]
kind = "classic"
rates = [125, 500, 1000]

[power]
battery = true
idle = true
low_battery = true

[keymap]
get = 0x8C
set = 0x0C
keys = [
  { id = 1, name = "LEFT_CLICK", default = "button 1" },
  { id = 2, name = "RIGHT_CLICK", default = "button 2" },
  { id = 3, name = "WHEEL_CLICK", default = "button 3" },
  { id = 4, name = "BACK", default = "button 4" },
  { id = 5, name = "FORWARD", default = "button 5" },
  { id = 9, name = "WHEEL_UP", default = "button 9" },
  { id = 10, name = "WHEEL_DOWN", default = "button 10" },
]
"#;

pub const DEATHADDER_ID: &str = "razer-deathadder-v3-pro";

/// The experimental DeathAdder V3 Pro: the compiled-in file when there is one, else the contract's.
pub fn deathadder() -> DeviceDef {
    uncoil_core::device::builtin()
        .into_iter()
        .find(|d| d.id == DEATHADDER_ID)
        .unwrap_or_else(|| DeviceDef::from_toml_named("fake DeathAdder", DEATHADDER).expect("fake definition parses"))
}

/// Every definition `uncoild --fake` knows: the compiled-in ones plus the fake DeathAdder when no file
/// provides it.
pub fn fake_defs() -> Vec<std::sync::Arc<DeviceDef>> {
    let mut defs = uncoil_core::device::builtin();
    if !defs.iter().any(|d| d.id == DEATHADDER_ID) {
        defs.push(deathadder());
    }
    defs.into_iter().map(std::sync::Arc::new).collect()
}

/// The devices `uncoild --fake` shows as connected: the supported keyboard and mouse (devices with a key
/// map) and the experimental DeathAdder V3 Pro.
pub fn connected_fakes(defs: &[std::sync::Arc<DeviceDef>]) -> Vec<std::sync::Arc<DeviceDef>> {
    defs.iter().filter(|d| (!d.is_experimental() && d.has(Feature::Keymap)) || d.id == DEATHADDER_ID).cloned().collect()
}

/// The Razer device without a definition that `uncoild --fake` reports in `status` (a product id no device
/// file claims).
pub fn unknown_devices() -> Vec<uncoil_core::config::UnknownDevice> {
    vec![uncoil_core::config::UnknownDevice { product_id: 0x0FFE, interfaces: vec![0, 1, 2] }]
}

/// What `uncoild --fake` reports in `status.conflicts`: Razer Synapse running, so the app's notice can be
/// tried.
pub fn conflicts() -> Vec<uncoil_core::ipc::Conflict> {
    crate::conflicts::found(["RazerAppEngine.exe"], false)
}

/// Performance and power values the fake answers with.
struct Perf {
    dpi: Dpi,
    stages: DpiStages,
    poll_hz: u16,
    battery_raw: u8,
    charging: bool,
    idle_s: u16,
    low_battery_raw: u8,
}

pub struct FakeDevice {
    keymap: Option<KeymapDef>,
    /// (profile, key, layer) -> (function id, data)
    keys: HashMap<(u8, u8, u8), (u8, Vec<u8>)>,
    echo_layer: bool,
    dial_mode: u8,
    oled_brightness: u8,
    regions: Vec<u8>,
    effects: Vec<u8>,
    dpi: bool,
    poll: Option<PollKind>,
    power: bool,
    perf: Perf,
    /// Scroll wheel: (mode, acceleration, smart reel) when the file has `scroll`.
    scroll: Option<[u8; 3]>,
    /// Firmware version `[major, minor]`.
    firmware: [u8; 2],
    /// Keyboards: `00/86` layout code and variant.
    keyboard_info: Option<[u8; 2]>,
    endpoint: UsbEndpoint,
    /// (class, id, args, transaction id as the real transport would send it)
    sent: Vec<(u8, u8, Vec<u8>, u8)>,
    /// A command that answers `fail` (tests of writes that stop part-way).
    fail: Option<(u8, u8)>,
}

fn stages(list: &[u16], active: u8) -> DpiStages {
    DpiStages { active, list: list.iter().map(|&v| Dpi { x: v, y: v }).collect() }
}

impl FakeDevice {
    pub fn for_def(def: &DeviceDef) -> FakeDevice {
        let mut keys = HashMap::new();
        if let Some(km) = &def.keymap {
            for k in &km.keys {
                let f = k.default_function().unwrap_or(match k.id {
                    1..=5 => Function::MouseButton { button: k.id },
                    _ => Function::Off,
                });
                let (f, d) = f.encode();
                for layer in [0u8, 1] {
                    keys.insert((1, k.id, layer), (f, d.clone()));
                }
            }
        }
        let keyboard = def.id == "razer-blackwidow-v4-pro-75";
        let perf = if def.id == DEATHADDER_ID {
            Perf {
                dpi: Dpi { x: 800, y: 800 },
                stages: stages(&[400, 800, 1600], 2),
                poll_hz: 1000,
                battery_raw: 163, // 64 %
                charging: true,
                idle_s: 600,
                low_battery_raw: 0x26, // 15 %
            }
        } else {
            Perf {
                dpi: Dpi { x: 1600, y: 1600 },
                stages: stages(&[400, 800, 1600, 3200, 6400], 3),
                poll_hz: 1000,
                battery_raw: 199, // 78 %
                charging: false,
                idle_s: 300,
                low_battery_raw: 0x26, // 15 %
            }
        };
        // keep the made-up values inside what the device file allows
        let mut perf = perf;
        if let Some(d) = &def.dpi {
            perf.dpi = perf.dpi.clamp(d.min, d.max);
            perf.stages.list.truncate((d.stages_max as usize).max(1));
            perf.stages = perf.stages.clamp(d.min, d.max);
        }
        if let Some(p) = &def.poll_rate {
            if !p.rates.contains(&perf.poll_hz) {
                perf.poll_hz = p.rates.iter().copied().max().unwrap_or(perf.poll_hz);
            }
        }
        let mut dev = FakeDevice {
            keymap: def.keymap.clone(),
            keys,
            echo_layer: keyboard,
            dial_mode: 0,
            oled_brightness: 100,
            regions: if keyboard {
                vec![5, 25, 3, 6, 18]
            } else if def.has(Feature::Lighting) {
                vec![1, 25, 3, 1, 1, 4, 25, 3, 1, 1, 10, 25, 3, 1, 11]
            } else {
                vec![]
            },
            effects: if keyboard { (0..=9).collect() } else { vec![0, 1, 2, 3, 5, 8] },
            dpi: def.has(Feature::Dpi),
            poll: def.poll_rate.as_ref().filter(|_| def.has(Feature::PollRate)).map(|p| p.kind),
            power: def.has(Feature::Power),
            perf,
            // tactile, acceleration on, Smart Reel off
            scroll: def.has(Feature::Scroll).then_some([0, 1, 0]),
            firmware: if keyboard {
                [1, 3]
            } else if def.id == DEATHADDER_ID {
                [1, 2]
            } else {
                [1, 4]
            },
            // US layout, black
            keyboard_info: keyboard.then_some([1, 0x00]),
            endpoint: def.usb[0].clone(),
            sent: vec![],
            fail: None,
        };
        if keyboard {
            // Fn layer as the real keyboard reported it (Fn+P before uncoil wrote it: an empty key code)
            for (key, f, d) in [
                (26u8, 2u8, vec![0u8, 0]),
                (120, 17, vec![4]),
                (121, 17, vec![3]),
                (122, 17, vec![9]),
                (123, 17, vec![8]),
                (110, 17, vec![11]),
                (76, 17, vec![76]),
            ] {
                dev.keys.insert((1, key, 1), (f, d));
            }
        }
        dev
    }

    #[cfg(test)]
    pub fn keyboard() -> FakeDevice {
        FakeDevice::for_def(
            &uncoil_core::device::builtin().into_iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap(),
        )
    }

    #[cfg(test)]
    pub fn set_key(&mut self, profile: u8, key: u8, layer: u8, f: u8, data: &[u8]) {
        self.keys.insert((profile, key, layer), (f, data.to_vec()));
    }

    #[cfg(test)]
    /// Make the DPI reply read `x`×`y` (e.g. 0×0 to fail the DPI check).
    pub fn set_dpi(&mut self, x: u16, y: u16) {
        self.perf.dpi = Dpi { x, y };
    }

    #[cfg(test)]
    /// Make `class`/`id` answer `fail` from now on.
    pub fn fail_on(&mut self, class: u8, id: u8) {
        self.fail = Some((class, id));
    }

    #[cfg(test)]
    /// Every report received: (class, id, args).
    pub fn sent(&self) -> Vec<(u8, u8, Vec<u8>)> {
        self.sent.iter().map(|(c, i, a, _)| (*c, *i, a.clone())).collect()
    }

    #[cfg(test)]
    /// Transaction id each report would have gone out with: (class, id, tid).
    pub fn sent_tids(&self) -> Vec<(u8, u8, u8)> {
        self.sent.iter().map(|(c, i, _, t)| (*c, *i, *t)).collect()
    }

    #[cfg(test)]
    /// Reports that would change onboard memory (setters outside lighting and device mode).
    pub fn writes(&self) -> Vec<(u8, u8, Vec<u8>)> {
        self.setters().into_iter().filter(|(c, id, _)| !(*c == 0x04 && *id == 0x05)).collect()
    }

    #[cfg(test)]
    /// Every setter except lighting and device mode, including a live (not stored) DPI change.
    pub fn setters(&self) -> Vec<(u8, u8, Vec<u8>)> {
        self.sent().into_iter().filter(|(c, id, _)| id & 0x80 == 0 && *c != 0x0F && (*c, *id) != (0x00, 0x04)).collect()
    }

    fn perf_answer(&mut self, r: &Report) -> Option<(Status, Vec<u8>)> {
        let a = |i: usize| r.args.get(i).copied().unwrap_or(0);
        let be = |v: u16| v.to_be_bytes();
        let p = &mut self.perf;
        Some(match (r.class, r.id) {
            (0x04, 0x85) if self.dpi => {
                let ([xh, xl], [yh, yl]) = (be(p.dpi.x), be(p.dpi.y));
                (Status::Ok, vec![a(0), xh, xl, yh, yl, 0, 0])
            }
            (0x04, 0x05) if self.dpi => {
                p.dpi = Dpi { x: u16::from_be_bytes([a(1), a(2)]), y: u16::from_be_bytes([a(3), a(4)]) };
                (Status::Ok, r.args.clone())
            }
            (0x04, 0x86) if self.dpi => {
                let mut v = vec![1, p.stages.active, p.stages.list.len() as u8];
                for (i, d) in p.stages.list.iter().enumerate() {
                    let ([xh, xl], [yh, yl]) = (be(d.x), be(d.y));
                    v.extend_from_slice(&[i as u8, xh, xl, yh, yl, 0, 0]);
                }
                (Status::Ok, v)
            }
            (0x04, 0x06) if self.dpi => {
                let n = a(2) as usize;
                let list = (0..n)
                    .map(|i| {
                        let o = 3 + i * 7;
                        Dpi { x: u16::from_be_bytes([a(o + 1), a(o + 2)]), y: u16::from_be_bytes([a(o + 3), a(o + 4)]) }
                    })
                    .collect();
                p.stages = DpiStages { active: a(1), list };
                (Status::Ok, r.args.clone())
            }
            (0x00, 0x85) if self.poll == Some(PollKind::Classic) => {
                (Status::Ok, vec![PollKind::Classic.code(p.poll_hz).unwrap_or(0)])
            }
            (0x00, 0x05) if self.poll == Some(PollKind::Classic) => match PollKind::Classic.hz(a(0)) {
                Some(hz) => {
                    p.poll_hz = hz;
                    (Status::Ok, r.args.clone())
                }
                None => (Status::Fail, vec![]),
            },
            (0x00, 0xC0) if self.poll == Some(PollKind::Hyperpolling) => {
                (Status::Ok, vec![0, PollKind::Hyperpolling.code(p.poll_hz).unwrap_or(0)])
            }
            (0x00, 0x40) if self.poll == Some(PollKind::Hyperpolling) => match PollKind::Hyperpolling.hz(a(1)) {
                Some(hz) => {
                    p.poll_hz = hz;
                    (Status::Ok, r.args.clone())
                }
                None => (Status::Fail, vec![]),
            },
            (0x07, 0x80) if self.power => (Status::Ok, vec![0, p.battery_raw]),
            (0x07, 0x84) if self.power => (Status::Ok, vec![0, p.charging as u8]),
            (0x07, 0x83) if self.power => (Status::Ok, be(p.idle_s).to_vec()),
            (0x07, 0x03) if self.power => {
                p.idle_s = u16::from_be_bytes([a(0), a(1)]);
                (Status::Ok, r.args.clone())
            }
            (0x07, 0x81) if self.power => (Status::Ok, vec![p.low_battery_raw]),
            (0x07, 0x01) if self.power => {
                p.low_battery_raw = a(0);
                (Status::Ok, r.args.clone())
            }
            _ => return None,
        })
    }

    fn scroll_answer(&mut self, r: &Report) -> Option<(Status, Vec<u8>)> {
        let s = self.scroll.as_mut()?;
        let i = match r.id & 0x7F {
            0x14 => 0,
            0x16 => 1,
            0x17 => 2,
            _ => return None,
        };
        if r.class != 0x02 {
            return None;
        }
        let a = |i: usize| r.args.get(i).copied().unwrap_or(0);
        if r.id & 0x80 == 0 {
            if a(1) > 1 {
                return Some((Status::Fail, vec![]));
            }
            s[i] = a(1);
        }
        Some((Status::Ok, vec![a(0), s[i]]))
    }

    fn answer(&mut self, r: &Report) -> (Status, Vec<u8>) {
        if let Some(ans) = self.perf_answer(r) {
            return ans;
        }
        if let Some(ans) = self.scroll_answer(r) {
            return ans;
        }
        let a = |i: usize| r.args.get(i).copied().unwrap_or(0);
        let km = self.keymap.as_ref();
        match (r.class, r.id) {
            (0x00, 0x04) => (Status::Ok, r.args.clone()),
            (0x0F, 0x02) if !self.regions.is_empty() => (Status::Ok, r.args.clone()),
            (0x00, 0x84) => (Status::Ok, vec![0, 0]),
            (0x00, 0x81) => (Status::Ok, self.firmware.to_vec()),
            (0x00, 0x86) if self.keyboard_info.is_some() => {
                (Status::Ok, self.keyboard_info.unwrap_or_default().to_vec())
            }
            (0x0F, 0x80) if !self.regions.is_empty() => (Status::Ok, self.regions.clone()),
            (0x0F, 0x81) if !self.regions.is_empty() => {
                let mut v = vec![a(0)];
                v.extend(&self.effects);
                (Status::Ok, v)
            }
            (0x02, id) if km.is_some_and(|k| k.get == id) => {
                let (p, key, layer) = (a(0), a(1), a(2));
                match self.keys.get(&(p, key, layer)) {
                    Some((f, d)) => {
                        let mut v = vec![p, key, if self.echo_layer { layer } else { 0 }, *f, d.len() as u8];
                        v.extend(d);
                        v.resize(10, 0);
                        (Status::Ok, v)
                    }
                    None => (Status::Fail, vec![]),
                }
            }
            (0x02, id) if km.is_some_and(|k| k.set == id) => {
                let (p, key, layer, f, len) = (a(0), a(1), a(2), a(3), a(4) as usize);
                if !self.keys.contains_key(&(p, key, layer)) || len > 5 {
                    return (Status::Fail, vec![]);
                }
                self.keys.insert((p, key, layer), (f, r.args[5..5 + len].to_vec()));
                (Status::Ok, r.args.clone())
            }
            (0x05, 0x8A) => (Status::Ok, vec![5]),
            (0x05, 0x80) => (Status::Ok, vec![1]),
            (0x05, 0x81) => (Status::Ok, vec![1, 1]),
            (0x05, 0x84) => (Status::Ok, vec![1]),
            (0x17, 0x80) if self.echo_layer => (Status::Ok, vec![a(0), self.dial_mode, 0, 0]),
            (0x17, 0x00) if self.echo_layer => {
                self.dial_mode = a(1);
                (Status::Ok, r.args.clone())
            }
            (0x17, 0x03) if self.echo_layer => {
                self.oled_brightness = a(0);
                (Status::Ok, r.args.clone())
            }
            (0x17, id) if self.echo_layer => match id {
                0x82 => (Status::Ok, vec![0, 1, 0, 0, 0, 0, 0]),
                0x83 => (Status::Ok, vec![self.oled_brightness]),
                0x84 | 0x86 => (Status::Ok, vec![1]),
                0x85 | 0x93 => (Status::Ok, vec![0]),
                0x8D => (Status::Ok, vec![0, 0]),
                0x92 => (Status::Ok, vec![20]),
                0x94 => (Status::Ok, vec![1; 6]),
                0x96 => (Status::Ok, vec![0, 0]),
                _ => (Status::Unsupported, vec![]),
            },
            _ => (Status::Unsupported, vec![]),
        }
    }
}

impl Transport for FakeDevice {
    fn query(&mut self, r: &Report) -> anyhow::Result<Reply> {
        let tid = self.endpoint.tid_for(r);
        self.sent.push((r.class, r.id, r.args.clone(), tid));
        let (status, args) = if self.fail == Some((r.class, r.id)) { (Status::Fail, vec![]) } else { self.answer(r) };
        let mut raw = [0u8; MAX_ARGS];
        raw[..args.len()].copy_from_slice(&args);
        Ok(Reply { status, transaction_id: tid, size: args.len() as u8, class: r.class, id: r.id, raw })
    }
}
