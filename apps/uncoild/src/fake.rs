//! A fake BlackWidow V4 Pro 75% / Basilisk V3 Pro that answers the feature commands the way the real ones
//! did in read-only probes (2026-10-03). Used by the tests and by `uncoild --fake` (cargo feature `fake`),
//! which serves the control pipe without touching any hardware.

use std::collections::HashMap;
use uncoil_core::device::DeviceDef;
use uncoil_core::features::keymap::{Function, KeymapDef};
use uncoil_core::proto::{Reply, Report, Status, Transport, MAX_ARGS};

pub struct FakeDevice {
    keymap: Option<KeymapDef>,
    /// (profile, key, layer) -> (function id, data)
    keys: HashMap<(u8, u8, u8), (u8, Vec<u8>)>,
    echo_layer: bool,
    dial_mode: u8,
    oled_brightness: u8,
    regions: Vec<u8>,
    effects: Vec<u8>,
    sent: Vec<(u8, u8, Vec<u8>)>,
}

impl FakeDevice {
    pub fn for_def(def: &DeviceDef) -> FakeDevice {
        let mut keys = HashMap::new();
        if let Some(km) = &def.keymap {
            for k in &km.keys {
                let (f, d) = k.default_function().unwrap_or(Function::Off).encode();
                for layer in [0u8, 1] {
                    keys.insert((1, k.id, layer), (f, d.clone()));
                }
            }
        }
        let keyboard = def.id == "razer-blackwidow-v4-pro-75";
        let mut dev = FakeDevice {
            keymap: def.keymap.clone(),
            keys,
            echo_layer: keyboard,
            dial_mode: 0,
            oled_brightness: 100,
            regions: if keyboard {
                vec![5, 25, 3, 6, 18]
            } else {
                vec![1, 25, 3, 1, 1, 4, 25, 3, 1, 1, 10, 25, 3, 1, 11]
            },
            effects: if keyboard { (0..=9).collect() } else { vec![0, 1, 2, 3, 5, 8] },
            sent: vec![],
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
    /// Every report received: (class, id, args).
    pub fn sent(&self) -> Vec<(u8, u8, Vec<u8>)> {
        self.sent.clone()
    }

    #[cfg(test)]
    /// Reports that would change onboard memory (setters outside lighting/device mode).
    pub fn writes(&self) -> Vec<(u8, u8, Vec<u8>)> {
        self.sent.iter().filter(|(c, id, _)| id & 0x80 == 0 && *c != 0x0F && *c != 0x00).cloned().collect()
    }

    fn answer(&mut self, r: &Report) -> (Status, Vec<u8>) {
        let a = |i: usize| r.args.get(i).copied().unwrap_or(0);
        let km = self.keymap.as_ref();
        match (r.class, r.id) {
            (0x00, 0x04) | (0x0F, 0x02) => (Status::Ok, r.args.clone()),
            (0x00, 0x84) => (Status::Ok, vec![0, 0]),
            (0x0F, 0x80) => (Status::Ok, self.regions.clone()),
            (0x0F, 0x81) => {
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
        self.sent.push((r.class, r.id, r.args.clone()));
        let (status, args) = self.answer(r);
        let mut raw = [0u8; MAX_ARGS];
        raw[..args.len()].copy_from_slice(&args);
        Ok(Reply { status, transaction_id: r.transaction_id, size: args.len() as u8, class: r.class, id: r.id, raw })
    }
}
