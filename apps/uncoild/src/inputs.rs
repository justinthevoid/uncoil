//! Live inputs for the effects: recent key presses and the system audio level, plus the desk geometry the
//! effects need (bounds, keyboard centre, where each key sits).
//!
//! PRIVACY (hard rule): a key press becomes a desk position and a time the moment it arrives, inside the
//! listener callback. Which key it was is never logged, stored, sent anywhere or kept as a sequence; the
//! buffer holds at most [`MAX_PRESSES`] `(x, y, t)` entries from the last [`KEEP`], in memory only. The key
//! listener exists only while the active effect needs presses (reactive, ripple), the audio meter only while
//! the audio meter effect is in use; both stop on the config reload that drops them.

use crate::control::Registry;
use crate::log;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use uncoil_core::config::Config;
use uncoil_core::device::{DeviceDef, Kind};
use uncoil_core::effect::{Bounds, Press};
use uncoil_core::layout::{self, PlacedDevice, Placement};
use uncoil_core::scancode;
use uncoil_hid::audio::AudioMeter;
use uncoil_hid::keys::KeyListener;

/// Presses older than this are dropped (longer than any sensible fade).
pub const KEEP: f32 = 5.0;
pub const MAX_PRESSES: usize = 64;
/// A key-down at a position already held within this long is auto-repeat, not a new press.
const REPEAT_WINDOW: Duration = Duration::from_millis(1000);

/// Desk geometry derived from the config and the connected devices, rebuilt when either changes.
#[derive(Default)]
pub struct Desk {
    pub bounds: Option<Bounds>,
    pub keyboard_center: Option<(f32, f32)>,
    keyboards: Vec<PlacedDevice>,
    /// Where each desk device sits (configured, default or auto-placed).
    placements: HashMap<String, Placement>,
}

impl Desk {
    /// The desk the GUI preview shows: supported devices, configured devices and the connected devices in
    /// `connected`, at their configured place or auto-placed next to their kind (`layout::arrange`).
    pub fn new(defs: &[Arc<DeviceDef>], cfg: &Config, connected: &[String]) -> Desk {
        let shown = layout::desk_devices(defs.iter().map(|d| &**d), &cfg.desk, |id| connected.iter().any(|c| c == id));
        let arranged = layout::arrange(&shown, &cfg.desk);
        let placements = arranged.iter().map(|(d, at, _)| (d.id.clone(), *at)).collect();
        let bounds = layout::desk_bounds(arranged.iter().map(|(_, _, p)| p));
        let keyboards: Vec<PlacedDevice> =
            arranged.into_iter().filter(|(d, _, _)| d.kind == Kind::Keyboard).map(|(_, _, p)| p).collect();
        Desk { bounds, keyboard_center: keyboards.first().map(PlacedDevice::center), keyboards, placements }
    }

    /// Where a device sits on the desk; its kind's default spot when the desk does not show it.
    pub fn placement(&self, def: &DeviceDef) -> Placement {
        self.placements.get(&def.id).copied().unwrap_or_else(|| layout::default_placement(def))
    }

    pub fn same_places(&self, other: &Desk) -> bool {
        self.placements == other.placements
    }

    /// Where the key with this shape name sits: on a connected keyboard if one has it, else the first.
    fn key_position(&self, name: &str, connected: impl Fn(&str) -> bool) -> Option<(f32, f32)> {
        let on = |kb: &PlacedDevice| kb.shape_position(name);
        self.keyboards.iter().filter(|kb| connected(&kb.id)).find_map(on).or_else(|| self.keyboards.iter().find_map(on))
    }
}

/// Shared between the main loop, the renderers and the listener threads.
pub struct Inputs {
    presses: Mutex<VecDeque<Press>>,
    /// Audio peak level, f32 bits.
    audio: Arc<AtomicU32>,
    desk: RwLock<Arc<Desk>>,
}

impl Inputs {
    pub fn new() -> Inputs {
        Inputs { presses: Mutex::new(VecDeque::new()), audio: Arc::new(AtomicU32::new(0)), desk: Default::default() }
    }

    pub fn desk(&self) -> Arc<Desk> {
        self.desk.read().unwrap().clone()
    }

    pub fn set_desk(&self, desk: Desk) {
        *self.desk.write().unwrap() = Arc::new(desk);
    }

    pub fn audio(&self) -> f32 {
        f32::from_bits(self.audio.load(Ordering::Relaxed))
    }

    fn push(&self, p: Press) {
        let mut q = self.presses.lock().unwrap();
        while q.front().is_some_and(|o| p.t - o.t > KEEP) || q.len() >= MAX_PRESSES {
            q.pop_front();
        }
        q.push_back(p);
    }

    /// Presses from the last [`KEEP`] seconds before `now`.
    pub fn presses(&self, now: f32) -> Vec<Press> {
        self.presses.lock().unwrap().iter().filter(|p| now - p.t <= KEEP).copied().collect()
    }

    /// Forget every press, overwriting the whole buffer (every slot it ever used) with zeros first.
    fn clear_presses(&self) {
        let mut q = self.presses.lock().unwrap();
        q.clear();
        let zero = Press { x: 0.0, y: 0.0, t: 0.0 };
        let cap = q.capacity();
        q.resize(cap, zero);
        q.iter_mut().for_each(|p| *p = zero);
        q.clear();
    }
}

/// Starts and stops the key listener and the audio meter to match the active effect.
#[derive(Default)]
pub struct Listeners {
    keys: Option<KeyListener>,
    /// The meter and the poll rate it was started with.
    audio: Option<(AudioMeter, u32)>,
}

impl Listeners {
    pub fn sync(&mut self, cfg: &Config, inputs: &Arc<Inputs>, registry: &Arc<Registry>, t0: Instant) {
        let want_keys = cfg.effect.uses_keys();
        if want_keys && self.keys.is_none() {
            self.keys = KeyListener::start(key_callback(inputs.clone(), registry.clone(), t0));
            log::line(if self.keys.is_some() { "key listener on" } else { "key listener unavailable" });
        } else if !want_keys && self.keys.is_some() {
            self.keys = None;
            inputs.clear_presses();
            log::line("key listener off");
        }

        let fps = cfg.fps.clamp(5, 60);
        let want_audio = cfg.effect.uses_audio();
        if self.audio.as_ref().is_some_and(|(_, f)| !want_audio || *f != fps) {
            self.audio = None;
            if !want_audio {
                log::line("audio meter off");
            }
        }
        if want_audio && self.audio.is_none() {
            let interval = Duration::from_secs_f32(1.0 / fps as f32);
            self.audio = Some((AudioMeter::start(interval, inputs.audio.clone()), fps));
            log::line("audio meter on");
        }
    }
}

/// Turns each key event into a desk position on the spot; the scan code and key name go no further.
fn key_callback(inputs: Arc<Inputs>, registry: Arc<Registry>, t0: Instant) -> uncoil_hid::keys::KeyCallback {
    // positions currently held down and when they last reported, to ignore auto-repeat
    let mut held: Vec<((f32, f32), Instant)> = Vec::new();
    Box::new(move |make, e0, down| {
        let Some(pos) = scancode::shape_name(make, e0)
            .and_then(|name| inputs.desk().key_position(name, |id| registry.is_connected(id)))
        else {
            return;
        };
        let now = Instant::now();
        held.retain(|(_, seen)| now.duration_since(*seen) < REPEAT_WINDOW);
        let slot = held.iter().position(|(p, _)| *p == pos);
        if !down {
            if let Some(i) = slot {
                held.swap_remove(i);
            }
            return;
        }
        match slot {
            Some(i) => held[i].1 = now, // auto-repeat
            None => {
                if held.len() < 32 {
                    held.push((pos, now));
                }
                inputs.push(Press { x: pos.0, y: pos.1, t: t0.elapsed().as_secs_f32() });
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uncoil_core::device;

    fn defs() -> Vec<Arc<DeviceDef>> {
        device::builtin().into_iter().map(Arc::new).collect()
    }

    #[test]
    fn desk_matches_the_effect_defaults() {
        let desk = Desk::new(&defs(), &Config::default(), &[]);
        assert_eq!(desk.bounds, Some(Bounds::DEFAULT));
        let (cx, cy) = desk.keyboard_center.unwrap();
        assert!((cx - 8.125).abs() < 1e-4 && (cy - 3.125).abs() < 1e-4);
        assert_eq!(desk.key_position("Escape", |_| false), Some((0.5, 0.5)));
        assert_eq!(desk.key_position("No Such Key", |_| true), None);
    }

    #[test]
    fn press_buffer_keeps_recent_positions_only() {
        let inputs = Inputs::new();
        for i in 0..100 {
            inputs.push(Press { x: 1.0, y: 2.0, t: i as f32 * 0.01 });
        }
        assert_eq!(inputs.presses(1.0).len(), MAX_PRESSES);
        inputs.push(Press { x: 3.0, y: 4.0, t: 10.0 });
        assert_eq!(inputs.presses(10.0), vec![Press { x: 3.0, y: 4.0, t: 10.0 }], "older than 5 s are dropped");
        assert!(inputs.presses(16.0).is_empty());
    }

    #[test]
    fn clearing_zeroes_the_buffer() {
        let inputs = Inputs::new();
        for i in 0..10 {
            inputs.push(Press { x: 3.0, y: 4.0, t: i as f32 });
        }
        let cap = inputs.presses.lock().unwrap().capacity();
        inputs.clear_presses();
        let q = inputs.presses.lock().unwrap();
        assert!(q.is_empty());
        assert_eq!(q.capacity(), cap, "the same allocation, overwritten in place");
    }

    #[test]
    fn key_events_become_positions_and_ignore_repeats() {
        let inputs = Arc::new(Inputs::new());
        inputs.set_desk(Desk::new(&defs(), &Config::default(), &[]));
        let mut cb = key_callback(inputs.clone(), Arc::new(Registry::default()), Instant::now());
        cb(0x11, false, true); // W down
        cb(0x11, false, true); // auto-repeat
        cb(0x11, false, false); // W up
        cb(0x11, false, true); // pressed again
        cb(0x63, false, true); // a code no layout names: ignored
        let p = inputs.presses(KEEP);
        assert_eq!(p.len(), 2);
        let w = Desk::new(&defs(), &Config::default(), &[]).key_position("W", |_| true).unwrap();
        assert!(p.iter().all(|p| (p.x, p.y) == w));
    }
}
