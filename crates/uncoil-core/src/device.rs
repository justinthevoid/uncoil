//! Device definitions: one TOML file per device in `devices/`. Built-in definitions are compiled into
//! the binary; extra ones can be dropped into the user's devices folder.

use crate::features::keymap::KeymapDef;
use crate::features::Feature;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Keyboard,
    Mouse,
    Mousemat,
    Headset,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbEndpoint {
    pub product_id: u16,
    #[serde(default)]
    pub connection: String,
    pub interface: i32,
    pub usage_page: u16,
    pub usage: u16,
    pub transaction_id: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Quirks {
    /// Read the reply after every report (BlackWidow freezes otherwise).
    #[serde(default)]
    pub ack_every_report: bool,
    /// Send the custom-frame effect once, not per frame.
    #[serde(default = "yes")]
    pub custom_mode_once: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    /// LED name at each (row, col); "" where the slot has no LED.
    pub names: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRow {
    pub y: f32,
    /// "name:width" entries, left to right; name "gap" = empty space.
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnderglowSide {
    pub prefix: String,
    pub x: f32,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Underglow {
    pub left: Option<UnderglowSide>,
    pub right: Option<UnderglowSide>,
    pub y_start: f32,
    pub y_end: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LayoutDef {
    /// Keys described as physical rows; LEDs are matched to keys by name.
    Keyboard { width: f32, depth: f32, rows: Vec<KeyRow>, underglow: Option<Underglow> },
    /// Explicit LED points relative to the device centre, in matrix order (row-major).
    Points { width: f32, depth: f32, points: Vec<[f32; 2]> },
}

/// Firmware lighting effects (`[hw_effects]` in the device TOML).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwEffectsDef {
    /// LED / region id the effect targets (0 whole device, 5 keyboard backlight).
    #[serde(default)]
    pub led: u8,
    /// Effect names this device runs (see `features::hw_effect`).
    pub effects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDef {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub vendor_id: u16,
    pub usb: Vec<UsbEndpoint>,
    #[serde(default)]
    pub quirks: Quirks,
    /// What the device supports beyond being identified; `lighting` when omitted.
    #[serde(default = "default_features")]
    pub features: Vec<Feature>,
    pub matrix: Matrix,
    pub layout: LayoutDef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hw_effects: Option<HwEffectsDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keymap: Option<KeymapDef>,
}

fn default_features() -> Vec<Feature> {
    vec![Feature::Lighting]
}

impl DeviceDef {
    pub fn from_toml(src: &str) -> Result<Self> {
        let d: DeviceDef = toml::from_str(src)?;
        d.validate()?;
        Ok(d)
    }

    fn validate(&self) -> Result<()> {
        if self.matrix.names.len() != self.matrix.rows {
            bail!("{}: matrix.names has {} rows, expected {}", self.id, self.matrix.names.len(), self.matrix.rows);
        }
        for (r, row) in self.matrix.names.iter().enumerate() {
            if row.len() != self.matrix.cols {
                bail!("{}: matrix row {r} has {} cols, expected {}", self.id, row.len(), self.matrix.cols);
            }
        }
        if self.has(Feature::Keymap) && self.keymap.is_none() {
            bail!("{}: declares the keymap feature but has no [keymap] section", self.id);
        }
        if self.has(Feature::HwEffects) && self.hw_effects.is_none() {
            bail!("{}: declares the hw_effects feature but has no [hw_effects] section", self.id);
        }
        if let Some(k) = &self.keymap {
            let mut ids = std::collections::HashSet::new();
            for key in &k.keys {
                if !ids.insert(key.id) {
                    bail!("{}: keymap key id {} listed twice", self.id, key.id);
                }
                if let Some(d) = &key.default {
                    crate::features::keymap::Function::parse_spec(d)
                        .with_context(|| format!("{}: default of key {}", self.id, key.name))?;
                }
            }
        }
        if let LayoutDef::Points { points, .. } = &self.layout {
            if points.len() != self.matrix.rows * self.matrix.cols {
                bail!(
                    "{}: {} layout points for {} matrix slots",
                    self.id,
                    points.len(),
                    self.matrix.rows * self.matrix.cols
                );
            }
        }
        Ok(())
    }

    pub fn has(&self, f: Feature) -> bool {
        self.features.contains(&f)
    }

    pub fn endpoint_for(&self, product_id: u16) -> Option<&UsbEndpoint> {
        self.usb.iter().find(|e| e.product_id == product_id)
    }
}

const BUILTIN: &[(&str, &str)] = &[
    ("razer-blackwidow-v4-pro-75", include_str!("../../../devices/razer-blackwidow-v4-pro-75.toml")),
    ("razer-basilisk-v3-pro", include_str!("../../../devices/razer-basilisk-v3-pro.toml")),
    ("razer-goliathus-chroma-extended", include_str!("../../../devices/razer-goliathus-chroma-extended.toml")),
];

/// All device definitions compiled into the binary.
pub fn builtin() -> Vec<DeviceDef> {
    BUILTIN
        .iter()
        .map(|(id, src)| DeviceDef::from_toml(src).with_context(|| format!("builtin device {id}")).unwrap())
        .collect()
}

/// Built-ins plus any `*.toml` in `dir` (user definitions override built-ins with the same id).
pub fn load_all(dir: Option<&std::path::Path>) -> Vec<DeviceDef> {
    let mut defs = builtin();
    if let Some(dir) = dir {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) != Some("toml") {
                    continue;
                }
                if let Ok(d) =
                    std::fs::read_to_string(&p).map_err(anyhow::Error::from).and_then(|s| DeviceDef::from_toml(&s))
                {
                    defs.retain(|x| x.id != d.id);
                    defs.push(d);
                }
            }
        }
    }
    defs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_parse_and_validate() {
        let defs = builtin();
        assert_eq!(defs.len(), 3);
        let kb = defs.iter().find(|d| d.kind == Kind::Keyboard).unwrap();
        assert_eq!(kb.matrix.names[0][14], "LU1");
        assert_eq!(kb.matrix.names[5][5], "RU1");
        assert!(kb.quirks.ack_every_report);
        assert_eq!(kb.endpoint_for(0x02B3).unwrap().interface, 3);
    }

    #[test]
    fn keyboard_features_and_keymap() {
        use crate::features::keymap::Function;
        let defs = builtin();
        let kb = defs.iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap();
        for f in Feature::ALL {
            assert!(kb.has(f), "keyboard should declare {f:?}");
        }
        let km = kb.keymap.as_ref().unwrap();
        assert_eq!((km.get, km.set), (0x8D, 0x0D));
        // ids verified on hardware: P = 26, Delete = 76, F9 = 120
        assert_eq!(km.find("P").unwrap().id, 26);
        assert_eq!(km.find("Delete").unwrap().id, 76);
        assert_eq!(km.find("F9").unwrap().id, 120);
        // every key is on the board's matrix
        for k in &km.keys {
            let led = k.led.as_deref().unwrap_or_else(|| panic!("key {} has no led", k.name));
            assert!(kb.matrix.names.iter().flatten().any(|n| n == led), "no LED named {led}");
        }
        // normal-layer defaults mined from Synapse match what the device answered
        assert_eq!(km.find("P").unwrap().default_function(), Some(Function::Key { modifiers: 0, usage: 0x13 }));
        assert_eq!(km.find("Left Shift").unwrap().default_function(), Some(Function::Key { modifiers: 2, usage: 0 }));
        assert_eq!(kb.hw_effects.as_ref().unwrap().led, 5);
    }

    #[test]
    fn mouse_keymap() {
        let defs = builtin();
        let m = defs.iter().find(|d| d.id == "razer-basilisk-v3-pro").unwrap();
        let km = m.keymap.as_ref().unwrap();
        assert_eq!((km.get, km.set), (0x8C, 0x0C));
        // the 13 button ids the mouse lists (02/84), read 2026-10-03
        let mut ids: Vec<u8> = km.keys.iter().map(|k| k.id).collect();
        ids.sort();
        assert_eq!(ids, vec![1, 2, 3, 4, 5, 9, 10, 14, 15, 52, 53, 96, 106]);
        assert!(!m.has(Feature::Dial));
    }
}
