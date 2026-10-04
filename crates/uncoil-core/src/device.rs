//! Device definitions: one TOML file per device in `devices/` (confirmed on hardware) and
//! `devices/experimental/` (built from OpenRazer / OpenRGB data, not yet confirmed). Every file in both
//! folders is compiled into the binary (see `build.rs`); extra ones can be dropped into the user's devices
//! folder.

use crate::features::keymap::KeymapDef;
use crate::features::performance::{DpiStorage, PollKind, MAX_STAGES};
use crate::features::Feature;
use crate::proto::{CommandGroup, Report, WIRE_LEN};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Keyboard,
    Mouse,
    Mousemat,
    Headset,
    Other,
}

/// How far a device definition can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Support {
    /// Confirmed on real hardware.
    #[default]
    Supported,
    /// Built from OpenRazer / OpenRGB data; nobody has confirmed it on the device yet. Writes to device
    /// memory wait for a read-only check (uncoild `check.run`).
    Experimental,
}

impl Support {
    pub fn as_str(self) -> &'static str {
        match self {
            Support::Supported => "supported",
            Support::Experimental => "experimental",
        }
    }
}

/// Per-command-group transaction ids (`[usb.transaction_ids]`); a missing group uses the endpoint's
/// `transaction_id`. See [`CommandGroup`] for which commands are in which group.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionIds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keymap: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<u8>,
    /// `07/01` / `07/81` only; falls back to `power`. OpenRazer sends just this pair with `0xFF` on the
    /// Basilisk V3 Pro, Viper V2 Pro and Cobra Pro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_battery: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<u8>,
}

impl TransactionIds {
    pub fn get(&self, g: CommandGroup) -> Option<u8> {
        match g {
            CommandGroup::Frame => self.frame,
            CommandGroup::Effect => self.effect,
            CommandGroup::Keymap => self.keymap,
            CommandGroup::Profile => self.profile,
            CommandGroup::Dpi => self.dpi,
            CommandGroup::Poll => self.poll,
            CommandGroup::Power => self.power,
            CommandGroup::LowBattery => self.low_battery.or(self.power),
            CommandGroup::Device => self.device,
            CommandGroup::Other => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsbEndpoint {
    pub product_id: u16,
    #[serde(default)]
    pub connection: String,
    pub interface: i32,
    /// First accepted HID collection (usage page, usage) on `interface`.
    pub usage_page: u16,
    pub usage: u16,
    /// More (usage page, usage) pairs accepted on the same interface, e.g. a collection that moved in a
    /// firmware update. Tried in order after the first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alt_usages: Vec<[u16; 2]>,
    /// Default transaction id for every command.
    pub transaction_id: u8,
    /// Wait this long before reading a reply (wireless receivers); the transport's own pauses otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_wait_us: Option<u32>,
    #[serde(default)]
    pub transaction_ids: TransactionIds,
}

impl UsbEndpoint {
    /// The transaction id this endpoint wants for `r`.
    pub fn tid_for(&self, r: &Report) -> u8 {
        self.transaction_ids.get(CommandGroup::of(r)).unwrap_or(self.transaction_id)
    }

    /// `r` on the wire with this endpoint's transaction id for it (the CRC does not cover the id).
    pub fn wire(&self, r: &Report) -> [u8; WIRE_LEN] {
        let mut w = r.to_wire();
        w[2] = self.tid_for(r);
        w
    }

    /// Accepted (usage page, usage) pairs, first one first.
    pub fn usages(&self) -> impl Iterator<Item = (u16, u16)> + '_ {
        std::iter::once((self.usage_page, self.usage)).chain(self.alt_usages.iter().map(|[p, u]| (*p, *u)))
    }

    /// Does a HID collection on `interface` with (`page`, `usage`) belong to this endpoint? Returns its rank
    /// (0 = the first usage) so the best collection can win when several match.
    pub fn accepts(&self, interface: i32, page: u16, usage: u16) -> Option<usize> {
        if interface != self.interface {
            return None;
        }
        self.usages().position(|pu| pu == (page, usage))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct HwEffectsDef {
    /// LED / region id the effect targets (0 whole device, 5 keyboard backlight).
    #[serde(default)]
    pub led: u8,
    /// Effect names this device runs (see `features::hw_effect`).
    pub effects: Vec<String>,
}

/// `[dpi]`: required with the `dpi` feature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DpiDef {
    pub min: u16,
    pub max: u16,
    /// Storage byte of `04/05`: `nostore` (default) or `varstore`.
    #[serde(default)]
    pub storage: DpiStorage,
    /// Most DPI stages the mouse keeps; 0 = no stages.
    #[serde(default)]
    pub stages_max: u8,
}

/// `[poll_rate]`: required with the `poll_rate` feature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PollRateDef {
    #[serde(default)]
    pub kind: PollKind,
    /// Rates offered, in Hz.
    pub rates: Vec<u16>,
    /// HyperPolling only: send the set command twice (argument 0, then 1).
    #[serde(default)]
    pub set_twice: bool,
}

/// `[power]`: required with the `power` feature. Each flag enables one part.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerDef {
    /// Battery level (`07/80`) and charging (`07/84`).
    #[serde(default)]
    pub battery: bool,
    /// Sleep timer (`07/03` / `07/83`).
    #[serde(default)]
    pub idle: bool,
    /// Low-battery threshold (`07/01` / `07/81`).
    #[serde(default)]
    pub low_battery: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceDef {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub vendor_id: u16,
    #[serde(default)]
    pub support: Support,
    /// Where the definition's facts come from (free text, shown nowhere but the file and docs).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<String, String>,
    pub usb: Vec<UsbEndpoint>,
    #[serde(default)]
    pub quirks: Quirks,
    /// What the device supports beyond being identified; `lighting` when omitted.
    #[serde(default = "default_features")]
    pub features: Vec<Feature>,
    /// Features of a supported device that nobody has confirmed on it yet: their writes wait for the same
    /// read-only checks as an experimental device's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unverified: Vec<Feature>,
    /// LED matrix; required with `lighting` or `hw_effects`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<Matrix>,
    /// Physical layout on the desk; required with `lighting` or `hw_effects`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hw_effects: Option<HwEffectsDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keymap: Option<KeymapDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<DpiDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_rate: Option<PollRateDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<PowerDef>,
}

fn default_features() -> Vec<Feature> {
    vec![Feature::Lighting]
}

impl DeviceDef {
    /// Parse and validate a device file. Errors name the file as "device file"; use
    /// [`DeviceDef::from_toml_named`] to name it properly.
    pub fn from_toml(src: &str) -> Result<Self> {
        DeviceDef::from_toml_named("device file", src)
    }

    /// Parse and validate; every error starts with `origin` (a path) and names the field.
    pub fn from_toml_named(origin: &str, src: &str) -> Result<Self> {
        let d: DeviceDef = toml::from_str(src).map_err(|e| anyhow!("{origin}: {}", e.to_string().trim_end()))?;
        d.validate().map_err(|e| anyhow!("{origin}: {e}"))?;
        Ok(d)
    }

    fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            bail!("id is empty");
        }
        if self.usb.is_empty() {
            bail!("no [[usb]] endpoint");
        }
        for (i, e) in self.usb.iter().enumerate() {
            if self.usb[..i].iter().any(|o| o.product_id == e.product_id) {
                bail!("[[usb]]: product_id 0x{:04X} listed twice", e.product_id);
            }
        }
        for (i, f) in self.features.iter().enumerate() {
            if self.features[..i].contains(f) {
                bail!("features: \"{}\" listed twice", f.as_str());
            }
        }
        for f in &self.unverified {
            if !self.has(*f) {
                bail!("unverified: \"{}\" is not in features", f.as_str());
            }
        }
        let lit = self.has(Feature::Lighting) || self.has(Feature::HwEffects);
        match (&self.matrix, &self.layout) {
            (None, _) if lit => bail!("features has lighting or hw_effects but there is no [matrix] section"),
            (_, None) if lit => bail!("features has lighting or hw_effects but there is no [layout] section"),
            (None, Some(_)) => bail!("[layout] needs a [matrix] section"),
            _ => {}
        }
        if let Some(m) = &self.matrix {
            if m.names.len() != m.rows {
                bail!("matrix.names has {} rows, expected {}", m.names.len(), m.rows);
            }
            for (r, row) in m.names.iter().enumerate() {
                if row.len() != m.cols {
                    bail!("matrix.names row {r} has {} cols, expected {}", row.len(), m.cols);
                }
            }
            if let Some(LayoutDef::Points { points, .. }) = &self.layout {
                if points.len() != m.rows * m.cols {
                    bail!("layout.points has {} points for {} matrix slots", points.len(), m.rows * m.cols);
                }
            }
        }
        if self.has(Feature::Keymap) && self.keymap.is_none() {
            bail!("features has \"keymap\" but there is no [keymap] section");
        }
        if self.has(Feature::HwEffects) && self.hw_effects.is_none() {
            bail!("features has \"hw_effects\" but there is no [hw_effects] section");
        }
        if let Some(k) = &self.keymap {
            let mut ids = std::collections::HashSet::new();
            for key in &k.keys {
                if !ids.insert(key.id) {
                    bail!("keymap.keys: id {} listed twice", key.id);
                }
                if let Some(d) = &key.default {
                    crate::features::keymap::Function::parse_spec(d)
                        .map_err(|e| anyhow!("keymap.keys: default of {}: {e}", key.name))?;
                }
            }
        }
        match &self.dpi {
            None if self.has(Feature::Dpi) => bail!("features has \"dpi\" but there is no [dpi] section"),
            Some(d) => {
                if d.min == 0 || d.min >= d.max {
                    bail!("[dpi]: min {} must be above 0 and below max {}", d.min, d.max);
                }
                if d.stages_max as usize > MAX_STAGES {
                    bail!("[dpi]: stages_max {} is more than the {MAX_STAGES} the command carries", d.stages_max);
                }
            }
            None => {}
        }
        match &self.poll_rate {
            None if self.has(Feature::PollRate) => {
                bail!("features has \"poll_rate\" but there is no [poll_rate] section")
            }
            Some(p) => {
                if p.rates.is_empty() {
                    bail!("[poll_rate]: rates is empty");
                }
                if let Some(r) = p.rates.iter().find(|r| p.kind.code(**r).is_none()) {
                    bail!("[poll_rate]: {r} Hz is not a rate the {:?} command can set", p.kind);
                }
                if p.set_twice && p.kind != PollKind::Hyperpolling {
                    bail!("[poll_rate]: set_twice only applies to kind = \"hyperpolling\"");
                }
            }
            None => {}
        }
        match &self.power {
            None if self.has(Feature::Power) => bail!("features has \"power\" but there is no [power] section"),
            Some(p) if !(p.battery || p.idle || p.low_battery) => {
                bail!("[power]: enable at least one of battery, idle, low_battery")
            }
            _ => {}
        }
        Ok(())
    }

    pub fn has(&self, f: Feature) -> bool {
        self.features.contains(&f)
    }

    pub fn is_experimental(&self) -> bool {
        self.support == Support::Experimental
    }

    /// Do writes for `f` wait for a read-only check on this device? Experimental devices: every feature.
    /// Supported devices: only the features listed in `unverified`.
    pub fn needs_check(&self, f: Feature) -> bool {
        self.has(f) && (self.is_experimental() || self.unverified.contains(&f))
    }

    /// The LED matrix and layout, when the device has lighting on the desk.
    pub fn lit(&self) -> Option<(&Matrix, &LayoutDef)> {
        Some((self.matrix.as_ref()?, self.layout.as_ref()?))
    }

    /// Streams lighting frames (per-LED software lighting with a matrix).
    pub fn streams_frames(&self) -> bool {
        self.has(Feature::Lighting) && self.matrix.is_some()
    }

    pub fn endpoint_for(&self, product_id: u16) -> Option<&UsbEndpoint> {
        self.usb.iter().find(|e| e.product_id == product_id)
    }
}

// `BUILTIN_BLOB`, `BUILTIN_LEN` and `BUILTIN_INDEX`, generated by build.rs from devices/*.toml and
// devices/experimental/*.toml
include!(concat!(env!("OUT_DIR"), "/builtin_devices.rs"));

/// Every compiled-in device file: (path relative to the repository, contents), sorted by path. The files are
/// stored deflated and inflated once, on first use.
pub fn builtin_files() -> &'static [(&'static str, &'static str)] {
    static TEXT: OnceLock<String> = OnceLock::new();
    static FILES: OnceLock<Vec<(&'static str, &'static str)>> = OnceLock::new();
    FILES.get_or_init(|| {
        let text = TEXT.get_or_init(|| {
            let bytes = miniz_oxide::inflate::decompress_to_vec_with_limit(BUILTIN_BLOB, BUILTIN_LEN)
                .expect("build.rs wrote a valid deflate blob");
            String::from_utf8(bytes).expect("device files are UTF-8")
        });
        BUILTIN_INDEX.iter().map(|&(path, start, end)| (path, &text[start..end])).collect()
    })
}

/// Every compiled-in file parsed, in path order.
pub fn builtin_parsed() -> Vec<Result<DeviceDef>> {
    builtin_files().iter().map(|(path, src)| DeviceDef::from_toml_named(path, src)).collect()
}

/// All valid device definitions compiled into the binary. A file that fails to parse is left out (the
/// `every_device_file_parses` test keeps that from shipping); [`builtin_errors`] lists such files.
pub fn builtin() -> Vec<DeviceDef> {
    builtin_parsed().into_iter().filter_map(Result::ok).collect()
}

/// Why each compiled-in file that does not parse was left out (empty in a good build).
pub fn builtin_errors() -> Vec<String> {
    builtin_parsed().into_iter().filter_map(|r| r.err().map(|e| e.to_string())).collect()
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
                if let Ok(d) = std::fs::read_to_string(&p)
                    .map_err(anyhow::Error::from)
                    .and_then(|s| DeviceDef::from_toml_named(&p.display().to_string(), &s))
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

    /// The devices agent relies on this: every file in devices/ and devices/experimental/ parses and
    /// validates, and the folder matches the file's `support`.
    #[test]
    fn every_device_file_parses() {
        let mut errors = vec![];
        let mut ids: BTreeMap<String, &str> = BTreeMap::new();
        let mut pids: BTreeMap<(u16, u16), &str> = BTreeMap::new();
        for ((path, _), parsed) in builtin_files().iter().zip(builtin_parsed()) {
            let d = match parsed {
                Ok(d) => d,
                Err(e) => {
                    errors.push(format!("{e:#}"));
                    continue;
                }
            };
            let experimental_dir = path.starts_with("devices/experimental/");
            if experimental_dir != d.is_experimental() {
                errors.push(format!(
                    "{path}: support = \"{}\" but the file is in {}",
                    d.support.as_str(),
                    if experimental_dir { "devices/experimental/" } else { "devices/" }
                ));
            }
            if let Some(other) = ids.insert(d.id.clone(), path) {
                errors.push(format!("{path}: id {} is also used by {other}", d.id));
            }
            for e in &d.usb {
                if let Some(other) = pids.insert((d.vendor_id, e.product_id), path) {
                    errors.push(format!("{path}: product_id 0x{:04X} is also in {other}", e.product_id));
                }
            }
        }
        assert!(errors.is_empty(), "device files with problems:\n{}", errors.join("\n"));
        assert!(builtin_errors().is_empty());
    }

    #[test]
    fn builtins_parse_and_validate() {
        let defs = builtin();
        let supported: Vec<&DeviceDef> = defs.iter().filter(|d| !d.is_experimental()).collect();
        assert_eq!(supported.len(), 3);
        let kb = defs.iter().find(|d| d.kind == Kind::Keyboard && !d.is_experimental()).unwrap();
        let m = kb.matrix.as_ref().unwrap();
        assert_eq!(m.names[0][14], "LU1");
        assert_eq!(m.names[5][5], "RU1");
        assert!(kb.quirks.ack_every_report);
        assert_eq!(kb.endpoint_for(0x02B3).unwrap().interface, 3);
        // embedded sorted by path
        let paths: Vec<&str> = builtin_files().iter().map(|(p, _)| *p).collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
        assert!(paths.contains(&"devices/razer-basilisk-v3-pro.toml"));
    }

    #[test]
    fn keyboard_features_and_keymap() {
        use crate::features::keymap::Function;
        let defs = builtin();
        let kb = defs.iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap();
        for f in
            [Feature::Lighting, Feature::HwEffects, Feature::Keymap, Feature::Profiles, Feature::Dial, Feature::Oled]
        {
            assert!(kb.has(f), "keyboard should declare {f:?}");
        }
        assert!(!kb.has(Feature::Dpi));
        let km = kb.keymap.as_ref().unwrap();
        assert_eq!((km.get, km.set), (0x8D, 0x0D));
        // ids verified on hardware: P = 26, Delete = 76, F9 = 120
        assert_eq!(km.find("P").unwrap().id, 26);
        assert_eq!(km.find("Delete").unwrap().id, 76);
        assert_eq!(km.find("F9").unwrap().id, 120);
        // every key is on the board's matrix
        let m = kb.matrix.as_ref().unwrap();
        for k in &km.keys {
            let led = k.led.as_deref().unwrap_or_else(|| panic!("key {} has no led", k.name));
            assert!(m.names.iter().flatten().any(|n| n == led), "no LED named {led}");
        }
        // normal-layer defaults mined from Synapse match what the device answered
        assert_eq!(km.find("P").unwrap().default_function(), Some(Function::Key { modifiers: 0, usage: 0x13 }));
        assert_eq!(km.find("Left Shift").unwrap().default_function(), Some(Function::Key { modifiers: 2, usage: 0 }));
        assert_eq!(kb.hw_effects.as_ref().unwrap().led, 5);
        assert_eq!(kb.support, Support::Supported);
        assert!(!kb.needs_check(Feature::Keymap));
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

    #[test]
    fn basilisk_performance_and_power() {
        let defs = builtin();
        let m = defs.iter().find(|d| d.id == "razer-basilisk-v3-pro").unwrap();
        assert_eq!(m.dpi, Some(DpiDef { min: 100, max: 30000, storage: DpiStorage::Nostore, stages_max: 5 }));
        let p = m.poll_rate.as_ref().unwrap();
        assert_eq!((p.kind, p.rates.as_slice(), p.set_twice), (PollKind::Classic, &[125, 500, 1000][..], false));
        assert_eq!(m.power, Some(PowerDef { battery: true, idle: true, low_battery: true }));
        // supported device, but its new features wait for a read-only check
        assert_eq!(m.unverified, vec![Feature::Dpi, Feature::PollRate, Feature::Power]);
        assert!(m.needs_check(Feature::Dpi) && m.needs_check(Feature::Power));
        assert!(!m.needs_check(Feature::Keymap));
        // OpenRazer sends only the low-battery pair with 0xFF on this mouse, on both endpoints
        for e in &m.usb {
            assert_eq!(e.tid_for(&crate::features::power::get_low_battery(0)), 0xFF);
            assert_eq!(e.tid_for(&crate::features::power::set_low_battery(0, 0x20)), 0xFF);
            assert_eq!(e.tid_for(&crate::features::power::get_battery(0)), 0x1F);
            assert_eq!(e.tid_for(&crate::features::performance::get_stages(0)), 0x1F);
        }
    }

    const MINIMAL: &str = r#"
        id = "razer-test"
        name = "Razer Test"
        kind = "mouse"
        vendor_id = 0x1532
        support = "experimental"
        features = ["dpi", "poll_rate", "power"]

        [sources]
        openrazer = "mouse.py RazerTest"

        [[usb]]
        product_id = 0x0099
        connection = "wired"
        interface = 0
        usage_page = 0x01
        usage = 0x02
        alt_usages = [[0x0C, 0x01]]
        transaction_id = 0x1F
        reply_wait_us = 31000

        [usb.transaction_ids]
        effect = 0x3F
        dpi = 0xFF
        power = 0xFE

        [dpi]
        min = 100
        max = 30000
        stages_max = 5

        [poll_rate]
        kind = "hyperpolling"
        rates = [125, 1000, 8000]
        set_twice = true

        [power]
        battery = true
        idle = true
        low_battery = true
    "#;

    #[test]
    fn feature_only_device_without_matrix() {
        let d = DeviceDef::from_toml(MINIMAL).unwrap();
        assert!(d.is_experimental() && d.matrix.is_none() && d.lit().is_none() && !d.streams_frames());
        assert!(d.needs_check(Feature::Dpi));
        assert_eq!(d.sources["openrazer"], "mouse.py RazerTest");
        let e = &d.usb[0];
        assert_eq!(e.reply_wait_us, Some(31000));
        assert_eq!(e.accepts(0, 0x01, 0x02), Some(0));
        assert_eq!(e.accepts(0, 0x0C, 0x01), Some(1));
        assert_eq!(e.accepts(1, 0x01, 0x02), None);
        assert_eq!(e.accepts(0, 0x01, 0x06), None);
        use crate::features::{performance as perf, power};
        assert_eq!(e.tid_for(&perf::get_dpi(0, DpiStorage::Nostore)), 0xFF);
        assert_eq!(e.tid_for(&perf::get_poll(0, PollKind::Classic)), 0x1F, "poll group not set");
        assert_eq!(e.tid_for(&power::get_idle(0)), 0xFE);
        assert_eq!(e.tid_for(&power::get_low_battery(0)), 0xFE, "low_battery falls back to power");
        assert_eq!(e.tid_for(&crate::proto::effect_wave(0, 1, 0x28)), 0x3F);
        assert_eq!(e.tid_for(&crate::proto::effect_custom_frame(0)), 0x1F, "custom frame is the frame group");
        let w = e.wire(&perf::get_dpi(0x1F, DpiStorage::Nostore));
        assert_eq!(w[2], 0xFF);
        assert_eq!(w[89], crate::proto::crc(&w[1..]), "the CRC does not cover the transaction id");
    }

    fn err(src: &str) -> String {
        format!("{:#}", DeviceDef::from_toml_named("devices/experimental/razer-test.toml", src).unwrap_err())
    }

    #[test]
    fn errors_name_the_file_and_field() {
        let e = err(&MINIMAL.replace("[dpi]\n", "[dpi_x]\n"));
        assert!(e.starts_with("devices/experimental/razer-test.toml: "), "{e}");
        assert!(e.contains("dpi_x"), "{e}");
        let e = err(&MINIMAL.replace("min = 100", "min = 40000"));
        assert!(e.contains("[dpi]: min 40000"), "{e}");
        let e = err(&MINIMAL.replace("rates = [125, 1000, 8000]", "rates = [125, 333]"));
        assert!(e.contains("[poll_rate]: 333 Hz"), "{e}");
        let e = err(&MINIMAL.replace("kind = \"hyperpolling\"", "kind = \"classic\""));
        assert!(e.contains("[poll_rate]"), "{e}");
        let e = err(&MINIMAL.replace("features = [\"dpi\",", "features = [\"lighting\", \"dpi\","));
        assert!(e.contains("no [matrix] section"), "{e}");
        let e = err(&MINIMAL.replace("power = 0xFE", "powr = 0xFE"));
        assert!(e.contains("powr"), "{e}");
        let e = err(&MINIMAL.replace("support = \"experimental\"", "support = \"maybe\""));
        assert!(e.contains("support") || e.contains("maybe"), "{e}");
        let e = err(&MINIMAL.replace("battery = true\n        idle = true\n        low_battery = true", ""));
        assert!(e.contains("[power]"), "{e}");
        let e = err(&MINIMAL.replace(
            "features = [\"dpi\", \"poll_rate\", \"power\"]",
            "features = [\"dpi\"]\nunverified = [\"power\"]",
        ));
        assert!(e.contains("unverified"), "{e}");
    }
}
