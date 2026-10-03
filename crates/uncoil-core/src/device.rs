//! Device definitions: one TOML file per device in `devices/`. Built-in definitions are compiled into
//! the binary; extra ones can be dropped into the user's devices folder.

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDef {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub vendor_id: u16,
    pub usb: Vec<UsbEndpoint>,
    #[serde(default)]
    pub quirks: Quirks,
    pub matrix: Matrix,
    pub layout: LayoutDef,
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
}
