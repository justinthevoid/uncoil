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
    /// Scroll wheel settings (`02/14`, `02/16`, `02/17` and their gets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll: Option<u8>,
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
            CommandGroup::Scroll => self.scroll,
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

/// `[scroll]`: required with the `scroll` feature. Each flag enables one setting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScrollDef {
    /// Tactile / free spin (`02/14` / `02/94`).
    #[serde(default)]
    pub mode: bool,
    /// Scroll acceleration (`02/16` / `02/96`).
    #[serde(default)]
    pub acceleration: bool,
    /// Smart Reel (`02/17` / `02/97`).
    #[serde(default)]
    pub smart_reel: bool,
}

impl ScrollDef {
    pub fn has(&self, s: crate::features::scroll::Setting) -> bool {
        use crate::features::scroll::Setting;
        match s {
            Setting::Mode => self.mode,
            Setting::Acceleration => self.acceleration,
            Setting::SmartReel => self.smart_reel,
        }
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll: Option<ScrollDef>,
}

fn default_features() -> Vec<Feature> {
    vec![Feature::Lighting]
}

/// Razer's USB vendor id; every device file must use it.
pub const RAZER_VID: u16 = 0x1532;
/// Transaction ids Razer devices use (OpenRazer and the maintainer's devices).
pub const TRANSACTION_IDS: [u8; 4] = [0x1F, 0x3F, 0x9F, 0xFF];
/// Matrix limits: a frame row (5 bytes + 3 per column) must fit one report's 80 argument bytes.
pub const MAX_COLS: usize = 25;
pub const MAX_ROWS: usize = 32;
/// Underglow LEDs per side and points of a `points` layout.
pub const MAX_LEDS: usize = 64;
pub const MAX_REPLY_WAIT_US: u32 = 100_000;
pub const MAX_DPI: u16 = 50_000;
const MAX_NAME: usize = 64;
/// Key, LED, shape and effect names.
const MAX_SHORT: usize = 32;

/// `^[a-z0-9][a-z0-9-]{0,63}$`
fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    (1..=64).contains(&b.len())
        && b[0] != b'-'
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// Text from a device file that reaches the log or the GUI: no control characters, at most `max` chars.
fn text(what: &str, s: &str, max: usize) -> Result<()> {
    if s.chars().count() > max {
        bail!("{what}: {:?}… is longer than {max} characters", s.chars().take(max).collect::<String>());
    }
    if s.chars().any(char::is_control) {
        bail!("{what}: {s:?} contains a control character");
    }
    Ok(())
}

fn validate_layout(l: &LayoutDef) -> Result<()> {
    let num = |what: &str, v: f32| -> Result<()> {
        if !v.is_finite() || v.abs() > 1000.0 {
            bail!("layout: {what} {v} is not a size on a desk");
        }
        Ok(())
    };
    match l {
        LayoutDef::Keyboard { width, depth, rows, underglow } => {
            num("width", *width)?;
            num("depth", *depth)?;
            if rows.len() > 16 {
                bail!("layout: more than 16 key rows");
            }
            for r in rows {
                num("row y", r.y)?;
                if r.keys.len() > 48 {
                    bail!("layout: a key row has more than 48 keys");
                }
                for k in &r.keys {
                    text("layout.rows.keys", k, 48)?;
                }
            }
            if let Some(u) = underglow {
                num("underglow y_start", u.y_start)?;
                num("underglow y_end", u.y_end)?;
                for side in [&u.left, &u.right].into_iter().flatten() {
                    num("underglow x", side.x)?;
                    text("layout.underglow prefix", &side.prefix, MAX_SHORT)?;
                    if side.count > MAX_LEDS {
                        bail!("layout.underglow: count {} is more than {MAX_LEDS}", side.count);
                    }
                }
            }
        }
        LayoutDef::Points { width, depth, points } => {
            num("width", *width)?;
            num("depth", *depth)?;
            if points.len() > MAX_LEDS {
                bail!("layout.points: {} points, more than {MAX_LEDS}", points.len());
            }
            for p in points {
                num("point", p[0])?;
                num("point", p[1])?;
            }
        }
    }
    Ok(())
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

    /// A file from the user's devices folder: like [`DeviceDef::from_toml_named`], but without a `support`
    /// line it is `experimental` (nobody vouched for it), so its writes wait for the read-only checks.
    pub fn from_toml_user(origin: &str, src: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct SupportLine {
            support: Option<Support>,
        }
        let mut d = DeviceDef::from_toml_named(origin, src)?;
        let given = toml::from_str::<SupportLine>(src).map_err(|e| anyhow!("{origin}: {e}"))?.support;
        d.support = given.unwrap_or(Support::Experimental);
        Ok(d)
    }

    /// Device files are untrusted input (a user file runs in the daemon, and every value here ends up in a
    /// HID report, the log or the GUI), so everything with a size or a set of meaningful values is checked.
    fn validate(&self) -> Result<()> {
        if !valid_id(&self.id) {
            bail!(
                "id {:?} must be 1-64 lowercase letters, digits and dashes, starting with a letter or digit",
                self.id
            );
        }
        text("name", &self.name, MAX_NAME)?;
        if self.vendor_id != RAZER_VID {
            bail!("vendor_id 0x{:04X} is not Razer's (0x{RAZER_VID:04X})", self.vendor_id);
        }
        if self.usb.is_empty() || self.usb.len() > 8 {
            bail!("[[usb]]: {} endpoints (1 to 8)", self.usb.len());
        }
        for (i, e) in self.usb.iter().enumerate() {
            if self.usb[..i].iter().any(|o| o.product_id == e.product_id) {
                bail!("[[usb]]: product_id 0x{:04X} listed twice", e.product_id);
            }
            text("[[usb]]: connection", &e.connection, 16)?;
            if e.alt_usages.len() > 8 {
                bail!("[[usb]]: more than 8 alt_usages");
            }
            if e.reply_wait_us.is_some_and(|w| w > MAX_REPLY_WAIT_US) {
                bail!("[[usb]]: reply_wait_us is above {MAX_REPLY_WAIT_US}");
            }
            let t = &e.transaction_ids;
            let groups =
                [t.frame, t.effect, t.keymap, t.profile, t.dpi, t.poll, t.power, t.low_battery, t.scroll, t.device];
            for tid in std::iter::once(Some(e.transaction_id)).chain(groups).flatten() {
                if !TRANSACTION_IDS.contains(&tid) {
                    bail!("[[usb]]: transaction id 0x{tid:02X} is not one Razer devices use ({TRANSACTION_IDS:02X?})");
                }
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
            // a frame row is one report: 5 header bytes + 3 per column must fit the 80 argument bytes
            if !(1..=MAX_COLS).contains(&m.cols) || !(1..=MAX_ROWS).contains(&m.rows) {
                bail!("matrix is {}x{}; rows must be 1-{MAX_ROWS} and cols 1-{MAX_COLS}", m.rows, m.cols);
            }
            if m.names.len() != m.rows {
                bail!("matrix.names has {} rows, expected {}", m.names.len(), m.rows);
            }
            for (r, row) in m.names.iter().enumerate() {
                if row.len() != m.cols {
                    bail!("matrix.names row {r} has {} cols, expected {}", row.len(), m.cols);
                }
                for n in row {
                    text("matrix.names", n, MAX_SHORT)?;
                }
            }
            if let Some(LayoutDef::Points { points, .. }) = &self.layout {
                if points.len() != m.rows * m.cols {
                    bail!("layout.points has {} points for {} matrix slots", points.len(), m.rows * m.cols);
                }
            }
        }
        if let Some(l) = &self.layout {
            validate_layout(l)?;
        }
        if self.has(Feature::Keymap) && self.keymap.is_none() {
            bail!("features has \"keymap\" but there is no [keymap] section");
        }
        if self.has(Feature::HwEffects) && self.hw_effects.is_none() {
            bail!("features has \"hw_effects\" but there is no [hw_effects] section");
        }
        if let Some(h) = &self.hw_effects {
            if h.effects.len() > 16 {
                bail!("[hw_effects]: more than 16 effects");
            }
            for e in &h.effects {
                text("[hw_effects]: effects", e, MAX_SHORT)?;
            }
        }
        if let Some(k) = &self.keymap {
            use crate::features::keymap::{KEYBOARD_GET, MOUSE_GET};
            // only the two known key map getters, each with its own setter (get without the read bit)
            if !matches!(k.get, KEYBOARD_GET | MOUSE_GET) || k.set != k.get & 0x7F {
                bail!(
                    "[keymap]: get 0x{:02X} / set 0x{:02X} must be 0x8D / 0x0D (keyboards) or 0x8C / 0x0C (mice)",
                    k.get,
                    k.set
                );
            }
            let mut ids = std::collections::HashSet::new();
            for key in &k.keys {
                if !ids.insert(key.id) {
                    bail!("keymap.keys: id {} listed twice", key.id);
                }
                text("keymap.keys: name", &key.name, MAX_SHORT)?;
                if let Some(led) = &key.led {
                    text("keymap.keys: led", led, MAX_SHORT)?;
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
                if d.min == 0 || d.min >= d.max || d.max > MAX_DPI {
                    bail!("[dpi]: min {} must be above 0 and below max {}, and max at most {MAX_DPI}", d.min, d.max);
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
                if p.rates.is_empty() || p.rates.len() > 8 {
                    bail!("[poll_rate]: rates lists {} rates (1 to 8)", p.rates.len());
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
        match &self.scroll {
            None if self.has(Feature::Scroll) => bail!("features has \"scroll\" but there is no [scroll] section"),
            Some(s) if !(s.mode || s.acceleration || s.smart_reel) => {
                bail!("[scroll]: enable at least one of mode, acceleration, smart_reel")
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

/// The user's device folder, `%APPDATA%\uncoil\devices`.
pub fn user_dir() -> std::path::PathBuf {
    crate::config::Config::dir().join("devices")
}

/// What the daemon and the app both load: the built-ins plus the user's device folder ([`user_dir`]), and
/// why each user file that was left out failed.
pub fn load_installed() -> (Vec<DeviceDef>, Vec<String>) {
    load_all_with_errors(Some(&user_dir()))
}

/// Built-ins plus any `*.toml` in `dir` (user definitions override built-ins with the same id; without a
/// `support` line they are experimental, see [`DeviceDef::from_toml_user`]), and why each user file that
/// was left out failed.
pub fn load_all_with_errors(dir: Option<&std::path::Path>) -> (Vec<DeviceDef>, Vec<String>) {
    let mut defs = builtin();
    let mut errors = vec![];
    if let Some(dir) = dir {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) != Some("toml") {
                    continue;
                }
                // a device file is a few KB; anything far bigger is not one
                let read = match std::fs::metadata(&p) {
                    Ok(m) if m.len() > 1024 * 1024 => Err(anyhow!("{}: larger than 1 MB", p.display())),
                    _ => std::fs::read_to_string(&p).map_err(anyhow::Error::from),
                };
                match read.and_then(|s| DeviceDef::from_toml_user(&p.display().to_string(), &s)) {
                    Ok(d) => {
                        defs.retain(|x| x.id != d.id);
                        defs.push(d);
                    }
                    Err(e) => errors.push(format!("{e:#}")),
                }
            }
        }
    }
    (defs, errors)
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

    /// build.rs drops comments and deflates the files; what comes back out must parse to exactly what the
    /// files on disk parse to.
    #[test]
    fn embedded_files_match_the_files_on_disk() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for ((path, _), embedded) in builtin_files().iter().zip(builtin_parsed()) {
            let src = std::fs::read_to_string(root.join(path)).unwrap();
            let disk = DeviceDef::from_toml_named(path, &src).unwrap();
            assert_eq!(format!("{:?}", embedded.unwrap()), format!("{disk:?}"), "{path}");
        }
        let on_disk = ["devices", "devices/experimental"]
            .iter()
            .flat_map(|d| std::fs::read_dir(root.join(d)).unwrap().flatten())
            .filter(|e| e.path().extension().is_some_and(|x| x == "toml"))
            .count();
        assert_eq!(builtin_files().len(), on_disk, "every device file is embedded");
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
        // supported device; DPI and poll rate are confirmed on it, power and scroll still wait for a check
        assert_eq!(m.unverified, vec![Feature::Power, Feature::Scroll]);
        assert_eq!(m.scroll, Some(ScrollDef { mode: true, acceleration: true, smart_reel: true }));
        assert!(m.needs_check(Feature::Scroll) && m.needs_check(Feature::Power));
        assert!(!m.needs_check(Feature::Dpi) && !m.needs_check(Feature::PollRate));
        assert!(!m.needs_check(Feature::Keymap));
        // OpenRazer sends only the low-battery pair with 0xFF on this mouse, on both endpoints
        for e in &m.usb {
            assert_eq!(e.tid_for(&crate::features::power::get_low_battery(0)), 0xFF);
            assert_eq!(e.tid_for(&crate::features::power::set_low_battery(0, 0x20)), 0xFF);
            assert_eq!(e.tid_for(&crate::features::power::get_battery(0)), 0x1F);
            assert_eq!(e.tid_for(&crate::features::performance::get_stages(0)), 0x1F);
            assert_eq!(e.tid_for(&crate::features::scroll::get(0, crate::features::scroll::Setting::Mode)), 0x1F);
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
        power = 0x9F

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
        assert_eq!(e.tid_for(&power::get_idle(0)), 0x9F);
        assert_eq!(e.tid_for(&power::get_low_battery(0)), 0x9F, "low_battery falls back to power");
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
        let e = err(&MINIMAL.replace("power = 0x9F", "powr = 0x9F"));
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
        let scroll = |section: &str| {
            MINIMAL.replace("\"power\"]", "\"power\", \"scroll\"]").replace("[power]\n", &format!("{section}[power]\n"))
        };
        assert!(err(&scroll("")).contains("no [scroll] section"));
        assert!(err(&scroll("[scroll]\n")).contains("[scroll]: enable at least one"));
        assert!(err(&scroll("[scroll]\nwobble = true\n")).contains("wobble"));
        let d =
            DeviceDef::from_toml(&scroll("[scroll]\nmode = true\n").replace("power = 0x9F", "scroll = 0xFF")).unwrap();
        assert_eq!(d.scroll, Some(ScrollDef { mode: true, ..Default::default() }));
        use crate::features::scroll::{self, Setting};
        assert_eq!(d.usb[0].tid_for(&scroll::set(0x1F, Setting::SmartReel, 1)), 0xFF, "the scroll group");
    }

    /// One with a key map and lighting, for the untrusted-input checks.
    const LIT: &str = r#"
        id = "razer-test-kb"
        name = "Razer Test Keyboard"
        kind = "keyboard"
        vendor_id = 0x1532
        features = ["lighting", "keymap"]

        [[usb]]
        product_id = 0x0098
        interface = 3
        usage_page = 0x01
        usage = 0x06
        transaction_id = 0x1F

        [matrix]
        rows = 1
        cols = 2
        names = [["A", "B"]]

        [layout]
        type = "points"
        width = 4.0
        depth = 2.0
        points = [[-1.0, 0.0], [1.0, 0.0]]

        [keymap]
        get = 0x8D
        set = 0x0D
        keys = [ { id = 31, name = "A", led = "A", default = "key A" } ]
    "#;

    #[test]
    fn ids_and_names_are_plain() {
        assert!(DeviceDef::from_toml(LIT).is_ok());
        for bad in ["", "-razer", "Razer-Test", "razer test", "razer/../x", "razer_test", &"r".repeat(65)] {
            let e = err(&LIT.replace("id = \"razer-test-kb\"", &format!("id = {bad:?}")));
            assert!(e.contains("id "), "{bad:?}: {e}");
        }
        assert!(DeviceDef::from_toml(&LIT.replace("razer-test-kb", &"r".repeat(64))).is_ok());
        for (from, to) in [
            ("name = \"Razer Test Keyboard\"", "name = \"Razer\\nnext line\""),
            ("name = \"Razer Test Keyboard\"", &format!("name = {:?}", "x".repeat(65))),
            ("name = \"A\", led", "name = \"A\\u001b[31m\", led"),
            ("names = [[\"A\", \"B\"]]", "names = [[\"A\", \"B\\r\"]]"),
            ("led = \"A\"", &format!("led = {:?}", "x".repeat(33))),
        ] {
            assert!(DeviceDef::from_toml(&LIT.replace(from, to)).is_err(), "{to}");
        }
    }

    #[test]
    fn only_razer_devices() {
        let e = err(&LIT.replace("vendor_id = 0x1532", "vendor_id = 0x046D"));
        assert!(e.contains("vendor_id 0x046D"), "{e}");
    }

    #[test]
    fn key_map_commands_are_the_known_pairs() {
        let km = |get: &str, set: &str| {
            DeviceDef::from_toml(
                &LIT.replace("get = 0x8D\n        set = 0x0D", &format!("get = {get}\n        set = {set}")),
            )
        };
        assert!(km("0x8C", "0x0C").is_ok());
        for (get, set) in [("0x8D", "0x0C"), ("0x0D", "0x0D"), ("0x8D", "0x8D"), ("0x85", "0x05"), ("0x8E", "0x0E")] {
            assert!(km(get, set).is_err(), "{get}/{set}");
        }
    }

    #[test]
    fn sizes_are_bounded() {
        let bad = [
            ("rows = 1\n        cols = 2", "rows = 1\n        cols = 26"),
            ("rows = 1\n        cols = 2", "rows = 0\n        cols = 2"),
            ("rows = 1\n        cols = 2", "rows = 33\n        cols = 2"),
            ("transaction_id = 0x1F", "transaction_id = 0x20"),
            ("transaction_id = 0x1F", "transaction_id = 0x1F\n        reply_wait_us = 100001"),
            ("transaction_id = 0x1F", "transaction_id = 0x1F\n        [usb.transaction_ids]\n        dpi = 0x00"),
            ("points = [[-1.0, 0.0], [1.0, 0.0]]", "points = [[-1.0, 0.0], [nan, 0.0]]"),
        ];
        for (from, to) in bad {
            assert_eq!(LIT.matches(from).count(), 1, "{from}");
            assert!(DeviceDef::from_toml(&LIT.replace(from, to)).is_err(), "{to}");
        }
        // a 25-column row is the widest that fits one report, and validation stops there
        let wide = LIT
            .replace("cols = 2", "cols = 25")
            .replace("[[\"A\", \"B\"]]", &format!("[[{}]]", vec!["\"\""; 25].join(", ")))
            .replace("[[-1.0, 0.0], [1.0, 0.0]]", &format!("[{}]", vec!["[0.0, 0.0]"; 25].join(", ")));
        assert!(DeviceDef::from_toml(&wide).is_ok());
        assert!(crate::proto::custom_frame_row(0x1F, 0, 0, &[[0; 3]; MAX_COLS]).is_ok());
        assert!(crate::proto::custom_frame_row(0x1F, 0, 0, &[[0; 3]; MAX_COLS + 1]).is_err());
        // DPI, stages, poll rates
        assert!(err(&MINIMAL.replace("max = 30000", "max = 60000")).contains("[dpi]"));
        assert!(err(&MINIMAL.replace("stages_max = 5", "stages_max = 6")).contains("stages_max"));
        assert!(err(&MINIMAL.replace("rates = [125, 1000, 8000]", "rates = [125, 1000, 3000]")).contains("3000"));
        // a keyboard layout's underglow count
        let kb = builtin_files().iter().find(|(p, _)| *p == "devices/razer-blackwidow-v4-pro-75.toml").unwrap().1;
        assert!(kb.contains("count = 9"));
        let e = format!("{:#}", DeviceDef::from_toml(&kb.replacen("count = 9", "count = 1000000", 1)).unwrap_err());
        assert!(e.contains("underglow"), "{e}");
    }

    #[test]
    fn user_files_are_experimental_unless_they_say_otherwise() {
        let d = DeviceDef::from_toml_user("user.toml", LIT).unwrap();
        assert_eq!(d.support, Support::Experimental);
        assert!(d.needs_check(Feature::Keymap));
        let said = LIT.replace("kind = \"keyboard\"", "kind = \"keyboard\"\n        support = \"supported\"");
        assert_eq!(DeviceDef::from_toml_user("user.toml", &said).unwrap().support, Support::Supported);
        // built-ins keep their rule (the default is supported, see every_device_file_parses)
        assert_eq!(DeviceDef::from_toml(LIT).unwrap().support, Support::Supported);
        let dir = std::env::temp_dir().join(format!("uncoil-test-devices-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("kb.toml"), LIT).unwrap();
        std::fs::write(dir.join("bad.toml"), LIT.replace("0x1532", "0x1234")).unwrap();
        let (defs, errors) = load_all_with_errors(Some(&dir));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(defs.iter().find(|d| d.id == "razer-test-kb").unwrap().support, Support::Experimental);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("bad.toml"), "{errors:?}");
    }
}
