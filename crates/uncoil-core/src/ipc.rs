//! The daemon's control channel: newline-delimited JSON over the named pipe `\\.\pipe\uncoil`.
//!
//! One request per line, one response per line, matched by `id`:
//!
//! ```text
//! → {"id":1,"cmd":"keymap.get","device":"razer-blackwidow-v4-pro-75","args":{"key":"P","layer":"hypershift"}}
//! ← {"id":1,"ok":true,"result":{"profile":1,"key":26,"name":"P","layer":"hypershift","function":{…},…}}
//! ← {"id":2,"ok":false,"error":"no device matches \"headset\""}
//! ```
//!
//! Types here are shared by uncoild (server), the `uncoil` CLI and the GUI. Commands that write a device's
//! onboard memory must carry `"write": true`; the daemon refuses them otherwise and logs every write.
//!
//! A failed request carries a plain-words `error` and, for the cases a client may want to treat specially,
//! a machine-readable `code` (see [`codes`]).

pub use crate::config::UnknownDevice;
use crate::device::{DeviceDef, Kind, Support};
use crate::features::dial::DialMode;
use crate::features::hw_effect::{HwEffect, Region, Storage};
use crate::features::keymap::{Function, Layer};
use crate::features::performance::{Dpi, DpiStages, DpiStorage};
use crate::features::scroll::ScrollMode;
use crate::features::Feature;
use anyhow::{anyhow, bail, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

pub const PIPE_NAME: &str = r"\\.\pipe\uncoil";
/// Longest request line the daemon accepts.
pub const MAX_LINE: usize = 64 * 1024;
/// Longest response line a client accepts (the largest real answer, every known device's capabilities, is
/// a few hundred KB).
pub const MAX_RESPONSE: usize = 1024 * 1024;

/// A JSON value kept as text. The daemon never builds `serde_json::Value` trees: arguments are parsed
/// straight into the typed structs below and results are serialised straight to text (smaller binary).
pub type Raw = Box<RawValue>;

/// Serialise anything to a [`Raw`] JSON value.
pub fn raw<T: Serialize + ?Sized>(t: &T) -> Raw {
    serde_json::value::to_raw_value(t).expect("value serialises")
}

/// A request as it travels on the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Any JSON value; echoed back in the response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Raw>,
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Raw>,
}

impl Request {
    pub fn new(id: u64, device: Option<&str>, command: &Command) -> Request {
        let (cmd, args) = command.to_parts();
        Request { id: Some(raw(&id)), cmd: cmd.into(), device: device.map(String::from), args }
    }

    pub fn command(&self) -> Result<Command> {
        Command::from_parts(&self.cmd, self.args.as_deref().map(RawValue::get))
    }

    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("request serialises");
        s.push('\n');
        s
    }
}

/// Error codes a failed [`Response`] may carry next to its message.
pub mod codes {
    /// A read-only check failed (or could not run), so the device's settings were not changed. The message
    /// carries the check's detail.
    pub const CHECK_FAILED: &str = "check_failed";
    /// The key map change would leave no button that left-clicks.
    pub const LEFT_CLICK_GUARD: &str = "left_click_guard";
    /// The device does not have the feature the command needs.
    pub const NOT_SUPPORTED: &str = "not_supported";

    /// The code as one of the constants above, if it is one.
    pub fn known(code: &str) -> Option<&'static str> {
        [CHECK_FAILED, LEFT_CLICK_GUARD, NOT_SUPPORTED].into_iter().find(|c| *c == code)
    }
}

/// An error with a [`codes`] code; the daemon puts the code in the response. Build one with
/// [`coded`] and return it through `anyhow`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodedError {
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for CodedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CodedError {}

/// An `anyhow` error carrying `code`.
pub fn coded(code: &'static str, message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(CodedError { code, message: message.into() })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Raw>,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Raw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// One of [`codes`], for errors a client may treat specially.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl Response {
    pub fn ok(id: Option<Raw>, result: Raw) -> Response {
        Response { id, ok: true, result: Some(result), error: None, code: None }
    }

    pub fn err(id: Option<Raw>, error: impl Into<String>) -> Response {
        Response { id, ok: false, result: None, error: Some(error.into()), code: None }
    }

    pub fn err_code(id: Option<Raw>, code: Option<&str>, error: impl Into<String>) -> Response {
        Response { id, ok: false, result: None, error: Some(error.into()), code: code.map(String::from) }
    }

    /// The id as JSON text (`"7"`), if any.
    pub fn id_text(&self) -> Option<&str> {
        self.id.as_deref().map(RawValue::get)
    }

    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("response serialises");
        s.push('\n');
        s
    }

    /// The typed result, or the daemon's error: a [`CodedError`] when the response carries a known code.
    pub fn into_result<T: DeserializeOwned>(self) -> Result<T> {
        if !self.ok {
            let message = self.error.unwrap_or_else(|| "daemon reported an error".into());
            return Err(match self.code.as_deref().and_then(codes::known) {
                Some(code) => coded(code, message),
                None => anyhow!(message),
            });
        }
        Ok(serde_json::from_str(self.result.as_deref().map_or("null", RawValue::get))?)
    }
}

fn one() -> u8 {
    1
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CapabilitiesArgs {
    /// Also ask the device which lighting regions and firmware effects it reports (read-only).
    #[serde(default)]
    pub probe: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyArgs {
    /// Key name, LED name or `#id` (see `KeymapDef::find`).
    pub key: String,
    #[serde(default)]
    pub layer: Layer,
    #[serde(default = "one")]
    pub profile: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeySetArgs {
    pub key: String,
    #[serde(default)]
    pub layer: Layer,
    #[serde(default = "one")]
    pub profile: u8,
    pub function: Function,
    /// Must be true: this writes the device's onboard memory.
    #[serde(default)]
    pub write: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyResetArgs {
    pub key: String,
    #[serde(default)]
    pub layer: Layer,
    #[serde(default = "one")]
    pub profile: u8,
    #[serde(default)]
    pub write: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerArgs {
    #[serde(default)]
    pub layer: Layer,
    #[serde(default = "one")]
    pub profile: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileArgs {
    #[serde(default = "one")]
    pub profile: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DialSetArgs {
    pub mode: DialMode,
    #[serde(default = "one")]
    pub profile: u8,
    /// Modes the dial cycles through; Synapse's default six when omitted. Only used for the
    /// "n / total" the OLED shows.
    #[serde(default)]
    pub enabled: Option<Vec<DialMode>>,
    #[serde(default)]
    pub write: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OledSetArgs {
    /// Display brightness in percent (`17/03`, the only OLED setter observed in Synapse's logs).
    pub brightness: Option<u8>,
    #[serde(default)]
    pub write: bool,
}

/// `performance.set`. `dpi` alone is live and not stored (like pressing the DPI button) and needs no
/// `write`; `stages` and `poll_hz` are stored in the device and need `write: true`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PerformanceSetArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<Dpi>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stages: Option<DpiStages>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_hz: Option<u16>,
    #[serde(default)]
    pub write: bool,
}

/// `power.set`: stored in the device, so it needs `write: true`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PowerSetArgs {
    /// Seconds before the mouse sleeps (60–900).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_s: Option<u16>,
    /// Low-battery warning threshold in percent (5–25).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_battery_pct: Option<u8>,
    #[serde(default)]
    pub write: bool,
}

/// `scroll.set`: stored in the mouse, so it needs `write: true`. Settings left out stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScrollSetArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ScrollMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceleration: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smart_reel: Option<bool>,
    #[serde(default)]
    pub write: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectHwArgs {
    /// Spec string, e.g. `"wave left"`, `"static #ff0000"`.
    pub effect: HwEffect,
    /// `session` (default) or `onboard` (saved in the device; requires `write`).
    #[serde(default)]
    pub storage: Storage,
    #[serde(default)]
    pub write: bool,
}

/// Declares [`Command`]: one line per command with its argument struct and wire name, so the variant, the
/// name and the arguments are written down once. Generates the enum, [`Command::NAMES`],
/// [`Command::from_parts`], [`Command::name`] and [`Command::to_parts`]. A command without arguments ignores
/// any `args` it is sent.
macro_rules! commands {
    ($( $(#[$doc:meta])* $variant:ident $( ($args:ty) )? = $name:literal, )*) => {
        /// Every command the daemon understands.
        #[derive(Debug, Clone, PartialEq)]
        pub enum Command {
            $( $(#[$doc])* $variant $( ($args) )?, )*
        }

        impl Command {
            pub const NAMES: &'static [&'static str] = &[$($name),*];

            pub fn from_parts(cmd: &str, a: Option<&str>) -> Result<Command> {
                Ok(match cmd {
                    $( $name => commands!(@parse $variant, cmd, a $(, $args)?), )*
                    other => bail!("unknown command `{other}` (known: {})", Command::NAMES.join(", ")),
                })
            }

            /// The wire name (`"keymap.set"`).
            pub fn name(&self) -> &'static str {
                match self {
                    $( commands!(@pat $variant $(, $args)?) => $name, )*
                }
            }

            pub fn to_parts(&self) -> (&'static str, Option<Raw>) {
                match self {
                    $( commands!(@bind $variant, a $(, $args)?) => ($name, commands!(@raw a $(, $args)?)), )*
                }
            }
        }
    };
    (@parse $v:ident, $cmd:ident, $a:ident, $t:ty) => { Command::$v(args($cmd, $a)?) };
    (@parse $v:ident, $cmd:ident, $a:ident) => { Command::$v };
    (@pat $v:ident, $t:ty) => { Command::$v(_) };
    (@pat $v:ident) => { Command::$v };
    (@bind $v:ident, $a:ident, $t:ty) => { Command::$v($a) };
    (@bind $v:ident, $a:ident) => { Command::$v };
    (@raw $a:ident, $t:ty) => { Some(raw($a)) };
    (@raw $a:ident) => { None };
}

commands! {
    Status = "status",
    Devices = "devices",
    Capabilities(CapabilitiesArgs) = "capabilities",
    KeymapGet(KeyArgs) = "keymap.get",
    KeymapSet(KeySetArgs) = "keymap.set",
    KeymapReset(KeyResetArgs) = "keymap.reset",
    KeymapDump(LayerArgs) = "keymap.dump",
    ProfileList = "profile.list",
    DialGet(ProfileArgs) = "dial.get",
    DialSet(DialSetArgs) = "dial.set",
    OledGet = "oled.get",
    OledSet(OledSetArgs) = "oled.set",
    EffectHw(EffectHwArgs) = "effect.hw",
    /// Drop a firmware effect and go back to the configured software effect.
    EffectSoftware = "effect.software",
    /// Run every read-only check now; returns `Vec<FeatureCheck>`.
    CheckRun = "check.run",
    PerformanceGet = "performance.get",
    PerformanceSet(PerformanceSetArgs) = "performance.set",
    PowerGet = "power.get",
    PowerSet(PowerSetArgs) = "power.set",
    ScrollGet = "scroll.get",
    ScrollSet(ScrollSetArgs) = "scroll.set",
    /// Firmware version and, on keyboards, layout and colour variant (`DeviceDetails`); read once per
    /// connection.
    InfoGet = "info.get",
}

fn args<T: DeserializeOwned>(cmd: &str, a: Option<&str>) -> Result<T> {
    let a = match a {
        None | Some("null") => "{}",
        Some(s) => s,
    };
    serde_json::from_str(a).map_err(|e| anyhow!("{cmd}: bad args: {e}"))
}

/// What a command needs before it may run on a device, decided in one place ([`Command::policy`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Policy {
    /// The device must declare at least one feature of each group (`&[Dpi, PollRate]`: dpi or poll_rate).
    pub required_features: Vec<&'static [Feature]>,
    /// Something the command needs that the device's definition lacks although it declares the features
    /// (refused as `not_supported` with these words).
    pub missing: Option<&'static str>,
    /// Writes the device's onboard memory, so the request must carry `write: true`.
    pub needs_write: bool,
    /// Features whose read-only check must pass first (on devices where the feature needs one).
    pub gated: Vec<Feature>,
}

impl Policy {
    /// The words for a `not_supported` refusal (`"dpi or poll_rate"`), if the device cannot run the command.
    pub fn unsupported(&self, def: &DeviceDef) -> Option<String> {
        if let Some(group) = self.required_features.iter().find(|g| !g.iter().any(|f| def.has(*f))) {
            return Some(group.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(" or "));
        }
        self.missing.map(String::from)
    }
}

impl Command {
    /// What this command needs on `def`: features, `write: true`, read-only checks. Device-dependent where
    /// the device decides (a `varstore` mouse stores its DPI, so even the live DPI change is a write).
    pub fn policy(&self, def: &DeviceDef) -> Policy {
        use Feature::*;
        let p = |required: &'static [Feature], needs_write: bool, gated: &[Feature]| Policy {
            required_features: if required.is_empty() { vec![] } else { vec![required] },
            missing: None,
            needs_write,
            gated: gated.to_vec(),
        };
        match self {
            Command::Status | Command::Devices | Command::Capabilities(_) | Command::CheckRun => p(&[], false, &[]),
            // every Razer device answers these reads; never gated
            Command::InfoGet => p(&[], false, &[]),
            Command::KeymapGet(_) | Command::KeymapDump(_) => p(&[Keymap], false, &[]),
            Command::KeymapSet(_) | Command::KeymapReset(_) => p(&[Keymap], true, &[Keymap]),
            Command::ProfileList => p(&[Profiles], false, &[]),
            Command::DialGet(_) => p(&[Dial], false, &[]),
            Command::DialSet(_) => p(&[Dial], true, &[Dial]),
            Command::OledGet => p(&[Oled], false, &[]),
            // without a brightness nothing is written (refused later as "nothing to set")
            Command::OledSet(a) => p(&[Oled], a.brightness.is_some(), &[Oled]),
            // a session effect is never refused; saving one to the device is an onboard write like any other
            Command::EffectHw(a) if a.storage == Storage::Onboard => p(&[HwEffects], true, &[HwEffects]),
            Command::EffectHw(_) => p(&[HwEffects], false, &[]),
            Command::EffectSoftware => p(&[Lighting], false, &[]),
            Command::PerformanceGet => p(&[Dpi, PollRate], false, &[]),
            Command::PerformanceSet(a) => {
                let mut policy = Policy::default();
                if a.dpi.is_some() || a.stages.is_some() {
                    // the live DPI change also waits for the DPI check, although it is not stored
                    policy.required_features.push(&[Dpi]);
                    policy.gated.push(Dpi);
                }
                if a.poll_hz.is_some() {
                    policy.required_features.push(&[PollRate]);
                    policy.gated.push(PollRate);
                }
                if a.stages.is_some() && def.dpi.as_ref().is_some_and(|d| d.stages_max == 0) {
                    policy.missing = Some("DPI stages");
                }
                let stored_dpi = a.dpi.is_some() && def.dpi.as_ref().is_some_and(|d| d.storage == DpiStorage::Varstore);
                policy.needs_write = a.stages.is_some() || a.poll_hz.is_some() || stored_dpi;
                policy
            }
            Command::PowerGet => p(&[Power], false, &[]),
            Command::PowerSet(_) => p(&[Power], true, &[Power]),
            Command::ScrollGet => p(&[Scroll], false, &[]),
            Command::ScrollSet(_) => p(&[Scroll], true, &[Scroll]),
        }
    }

    /// Was the explicit `write: true` given?
    pub fn write_confirmed(&self) -> bool {
        match self {
            Command::KeymapSet(a) => a.write,
            Command::KeymapReset(a) => a.write,
            Command::DialSet(a) => a.write,
            Command::OledSet(a) => a.write,
            Command::EffectHw(a) => a.write,
            Command::PerformanceSet(a) => a.write,
            Command::PowerSet(a) => a.write,
            Command::ScrollSet(a) => a.write,
            _ => false,
        }
    }
}

// ---- results ------------------------------------------------------------------------------------

/// One connected device (`devices`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub product_id: u16,
    pub connection: String,
    pub features: Vec<Feature>,
    /// A firmware effect is showing instead of the software effect.
    #[serde(default)]
    pub hw_effect: Option<HwEffect>,
    #[serde(default)]
    pub support: Support,
}

/// Result of a read-only check (`check.run`, `Capabilities::checks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Passed,
    Failed,
    /// Not run yet on this connection.
    Untested,
    /// Confirmed on hardware already (supported devices).
    NotNeeded,
}

/// One feature's read-only check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureCheck {
    pub feature: Feature,
    pub state: CheckState,
    /// What was read, or why the check failed.
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyInfo {
    pub id: u8,
    pub name: String,
    #[serde(default)]
    pub led: Option<String>,
}

/// `capabilities`: what a device definition declares (plus, with `probe`, what the device reports).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub connected: bool,
    pub features: Vec<Feature>,
    #[serde(default)]
    pub hw_effects: Vec<String>,
    #[serde(default)]
    pub keymap_layers: Vec<Layer>,
    #[serde(default)]
    pub keys: Vec<KeyInfo>,
    #[serde(default)]
    pub dial_modes: Vec<DialMode>,
    #[serde(default)]
    pub probed: Option<LightingProbe>,
    #[serde(default)]
    pub support: Support,
    /// One entry per declared feature.
    #[serde(default)]
    pub checks: Vec<FeatureCheck>,
    /// Features of a supported device not yet confirmed on it (their writes wait for a check).
    #[serde(default)]
    pub unverified: Vec<Feature>,
}

/// Read from the device: `0F/80` regions and `0F/81` firmware effects per region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightingProbe {
    pub regions: Vec<Region>,
    /// (led, effect names)
    pub effects: Vec<(u8, Vec<String>)>,
}

/// One key's mapping (`keymap.get`, rows of `keymap.dump`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyMapping {
    pub profile: u8,
    pub key: u8,
    pub name: String,
    pub layer: Layer,
    /// Spec string in JSON (`"key PRINT_SCREEN"`), accepted back by `keymap set` and backup files.
    pub function: Function,
    pub description: String,
}

impl KeyMapping {
    pub fn new(profile: u8, key: u8, name: String, layer: Layer, function: Function) -> KeyMapping {
        KeyMapping { profile, key, name, layer, description: function.describe(), function }
    }
}

/// `effect.hw` / `effect.software` result: the firmware effect now showing (None = software effect).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectState {
    pub effect: Option<HwEffect>,
    pub storage: Storage,
}

/// Result of any onboard write: what was there, what is there now (read back), and whether it matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WriteResult<T> {
    pub before: T,
    pub after: T,
    pub verified: bool,
    /// Nothing was sent because the device already held the requested value.
    #[serde(default)]
    pub unchanged: bool,
}

/// A program that drives the same devices as uncoil, seen running (`status.conflicts`). Only known program
/// names are looked for; nothing else about other processes is read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// The program's name ("Razer Synapse").
    pub app: String,
    /// A plain sentence: what it means and what to do.
    pub detail: String,
}

/// The live OpenRGB connection (`status.openrgb.state`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenRgbState {
    /// Live mode is not configured.
    #[default]
    Off,
    /// Configured, no OpenRGB SDK server answering yet.
    Waiting,
    Connected,
    Error,
}

/// `status.openrgb`: the live OpenRGB client's state and the devices it drives.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRgbStatus {
    pub state: OpenRgbState,
    /// Plain words about the state (why it failed, what it waits for).
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub devices: Vec<OpenRgbDeviceStatus>,
    /// Devices OpenRGB lists that uncoil leaves alone because a program that lights them itself is running
    /// (`owners`); they come back when it quits.
    #[serde(default)]
    pub held: Vec<OpenRgbHeld>,
    /// The SDK server was started by uncoil's own `uncoil-openrgb` task.
    #[serde(default)]
    pub ours: bool,
}

/// A device left to another program (`status.openrgb.held`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRgbHeld {
    /// The device's name as OpenRGB lists it.
    pub name: String,
    /// The program that has it ("Corsair iCUE").
    pub by: String,
}

/// One device driven through OpenRGB.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRgbDeviceStatus {
    /// `openrgb:<slug of the name>`, the id the desk uses.
    pub id: String,
    pub name: String,
    pub leds: u32,
    /// Its zones, which place its LEDs on the desk (`layout::external_def`); the app's desk preview builds
    /// the same device from them.
    #[serde(default)]
    pub zones: Vec<crate::layout::ExternalZone>,
}

impl OpenRgbDeviceStatus {
    /// The desk device for it.
    pub fn def(&self) -> crate::device::DeviceDef {
        crate::layout::external_def(&self.id, &self.name, &self.zones)
    }
}

// ---- device selection ---------------------------------------------------------------------------

/// Resolve a user's device query: exact id, kind ("keyboard", "mouse", "mat"), or a unique substring of
/// the id or name ("blackwidow", "basilisk").
pub fn resolve_device<'a>(query: &str, devices: &[(&'a str, &str, Kind)]) -> Result<&'a str> {
    let q = query.to_ascii_lowercase();
    if let Some((id, _, _)) = devices.iter().find(|(id, _, _)| id.eq_ignore_ascii_case(&q)) {
        return Ok(id);
    }
    let kind = match q.as_str() {
        "keyboard" | "kb" => Some(Kind::Keyboard),
        "mouse" => Some(Kind::Mouse),
        "mousemat" | "mat" | "mousepad" | "pad" => Some(Kind::Mousemat),
        "headset" => Some(Kind::Headset),
        _ => None,
    };
    let hits: Vec<&'a str> = devices
        .iter()
        .filter(|(id, name, k)| {
            Some(*k) == kind || id.to_ascii_lowercase().contains(&q) || name.to_ascii_lowercase().contains(&q)
        })
        .map(|(id, _, _)| *id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    match hits.as_slice() {
        [one] => Ok(one),
        [] => bail!("no device matches \"{query}\""),
        many => bail!("\"{query}\" matches several devices: {}", many.join(", ")),
    }
}

// ---- client (for the CLI and the GUI) -----------------------------------------------------------

/// Blocking client for the daemon's pipe. One connection, any number of requests.
pub struct Client {
    pipe: std::fs::File,
    reader: std::io::BufReader<std::fs::File>,
    next_id: u64,
}

impl Client {
    /// Connect to `\\.\pipe\uncoil` (retries briefly while all pipe instances are busy).
    pub fn connect() -> Result<Client> {
        Client::connect_to(PIPE_NAME)
    }

    /// Connect to the pipe at `path`. The connection only lets the server identify the caller (it cannot
    /// act as the caller), and it is refused unless the process serving the pipe runs as the same user.
    pub fn connect_to(path: &str) -> Result<Client> {
        let mut last = None;
        for _ in 0..50 {
            let mut open = std::fs::OpenOptions::new();
            open.read(true).write(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                // also sets SECURITY_SQOS_PRESENT
                open.security_qos_flags(windows_sys::Win32::Storage::FileSystem::SECURITY_IDENTIFICATION);
            }
            match open.open(path) {
                Ok(pipe) => {
                    #[cfg(windows)]
                    server_check::same_user(&pipe).map_err(|e| {
                        anyhow!("something else is serving the uncoil pipe ({path}), not your uncoild: {e:#}")
                    })?;
                    let reader = std::io::BufReader::new(pipe.try_clone()?);
                    return Ok(Client { pipe, reader, next_id: 1 });
                }
                // ERROR_PIPE_BUSY: every instance is taken; the daemon creates a new one right away
                Err(e) if e.raw_os_error() == Some(231) => {
                    last = Some(e);
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(e) => {
                    return Err(anyhow!(
                        "cannot reach uncoild at {path}: {e}. Is the daemon running (scheduled task \"uncoil\")?"
                    ))
                }
            }
        }
        Err(anyhow!("uncoild's pipe stayed busy: {}", last.map(|e| e.to_string()).unwrap_or_default()))
    }

    /// Send one command and wait for its response.
    pub fn call(&mut self, device: Option<&str>, command: &Command) -> Result<Response> {
        use std::io::Write;
        let id = self.next_id;
        self.next_id += 1;
        let sent = self.pipe.write_all(Request::new(id, device, command).to_line().as_bytes());
        if let Err(e) = sent.and_then(|_| self.pipe.flush()) {
            // a full daemon answers the connection and closes it before reading anything: show its answer
            return match self.read_response(id) {
                Ok(r) if !r.ok => Ok(r),
                _ => Err(e.into()),
            };
        }
        self.read_response(id)
    }

    fn read_response(&mut self, id: u64) -> Result<Response> {
        use std::io::BufRead;
        loop {
            use std::io::Read;
            let mut line = String::new();
            let n = (&mut self.reader).take(MAX_RESPONSE as u64 + 1).read_line(&mut line)?;
            if n == 0 {
                bail!("uncoild closed the connection");
            }
            if n > MAX_RESPONSE {
                bail!("uncoild sent a response longer than {} KB", MAX_RESPONSE / 1024);
            }
            let r: Response = serde_json::from_str(line.trim_end())?;
            // an error without an id answers the connection, not a request ("too many clients", "request
            // too long"), so it is this call's answer too
            if r.id_text() == Some(id.to_string().as_str()) || (r.id.is_none() && !r.ok) {
                return Ok(r);
            }
        }
    }
}

/// Who serves the pipe: the client refuses a server process that does not run as the calling user (for
/// example a program that took the pipe name before uncoild started).
#[cfg(windows)]
mod server_check {
    use anyhow::{bail, Result};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
    use windows_sys::Win32::Security::{GetLengthSid, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    /// The token user SID of `process`, as bytes.
    fn user_sid(process: HANDLE) -> Result<Vec<u8>> {
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
                bail!("cannot read the process's token ({})", GetLastError());
            }
            let mut len = 0u32;
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut len);
            let mut buf = vec![0u64; (len as usize).div_ceil(8).max(1)];
            let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), len, &mut len);
            CloseHandle(token);
            if ok == 0 {
                bail!("cannot read the token's user ({})", GetLastError());
            }
            let sid = (*(buf.as_ptr() as *const TOKEN_USER)).User.Sid;
            Ok(std::slice::from_raw_parts(sid as *const u8, GetLengthSid(sid) as usize).to_vec())
        }
    }

    /// `Ok` when process `pid` runs as the same user as this process.
    pub fn pid_is_same_user(pid: u32) -> Result<()> {
        let mine = user_sid(unsafe { GetCurrentProcess() })?;
        let theirs = unsafe {
            let p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if p.is_null() {
                bail!("cannot open the serving process {pid} ({})", GetLastError());
            }
            let sid = user_sid(p);
            CloseHandle(p);
            sid?
        };
        if theirs != mine {
            bail!("the serving process {pid} runs as another user");
        }
        Ok(())
    }

    pub fn same_user(pipe: &std::fs::File) -> Result<()> {
        let mut pid = 0u32;
        if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle() as HANDLE, &mut pid) } == 0 {
            bail!("cannot tell which process serves it ({})", unsafe { GetLastError() });
        }
        pid_is_same_user(pid)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn only_a_server_running_as_you_passes() {
            super::pid_is_same_user(std::process::id()).unwrap();
            // the System process (pid 4) is not you
            assert!(super::pid_is_same_user(4).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb;
    use crate::features::hw_effect::Direction;

    #[test]
    fn request_wire_format() {
        let line =
            r#"{"id":7,"cmd":"keymap.get","device":"razer-blackwidow-v4-pro-75","args":{"key":"P","layer":"fn"}}"#;
        let r: Request = serde_json::from_str(line).unwrap();
        assert_eq!(
            r.command().unwrap(),
            Command::KeymapGet(KeyArgs { key: "P".into(), layer: Layer::Hypershift, profile: 1 })
        );
        let back = Request::new(7, Some("razer-blackwidow-v4-pro-75"), &r.command().unwrap());
        let again: Request = serde_json::from_str(back.to_line().trim_end()).unwrap();
        assert_eq!(again.command().unwrap(), r.command().unwrap());
    }

    #[test]
    fn every_command_roundtrips() {
        let cmds = vec![
            Command::Status,
            Command::Devices,
            Command::Capabilities(CapabilitiesArgs { probe: true }),
            Command::KeymapGet(KeyArgs { key: "P".into(), layer: Layer::Normal, profile: 1 }),
            Command::KeymapSet(KeySetArgs {
                key: "P".into(),
                layer: Layer::Hypershift,
                profile: 1,
                function: Function::Key { modifiers: 0, usage: 0x46 },
                write: true,
            }),
            Command::KeymapReset(KeyResetArgs { key: "P".into(), layer: Layer::Hypershift, profile: 1, write: false }),
            Command::KeymapDump(LayerArgs { layer: Layer::Hypershift, profile: 1 }),
            Command::ProfileList,
            Command::DialGet(ProfileArgs { profile: 1 }),
            Command::DialSet(DialSetArgs { mode: DialMode::Zoom, profile: 1, enabled: None, write: true }),
            Command::OledGet,
            Command::OledSet(OledSetArgs { brightness: Some(40), write: true }),
            Command::EffectHw(EffectHwArgs {
                effect: HwEffect::Wave { direction: Direction::Left, speed: 0x28 },
                storage: Storage::Session,
                write: false,
            }),
            Command::EffectHw(EffectHwArgs {
                effect: HwEffect::Static { color: Rgb(1, 2, 3) },
                storage: Storage::Onboard,
                write: true,
            }),
            Command::EffectSoftware,
            Command::CheckRun,
            Command::PerformanceGet,
            Command::PerformanceSet(PerformanceSetArgs {
                dpi: Some(Dpi { x: 800, y: 800 }),
                stages: Some(DpiStages { active: 1, list: vec![Dpi { x: 400, y: 400 }] }),
                poll_hz: Some(1000),
                write: true,
            }),
            Command::PowerGet,
            Command::PowerSet(PowerSetArgs { idle_s: Some(300), low_battery_pct: Some(15), write: true }),
            Command::ScrollGet,
            Command::ScrollSet(ScrollSetArgs {
                mode: Some(ScrollMode::FreeSpin),
                acceleration: Some(true),
                smart_reel: Some(false),
                write: true,
            }),
            Command::InfoGet,
        ];
        let mut names = std::collections::HashSet::new();
        for c in cmds {
            let req = Request::new(1, None, &c);
            assert_eq!(req.cmd, c.name());
            names.insert(req.cmd.clone());
            let parsed: Request = serde_json::from_str(req.to_line().trim_end()).unwrap();
            assert_eq!(parsed.command().unwrap(), c, "{}", req.cmd);
        }
        assert_eq!(names.len(), Command::NAMES.len());
    }

    fn def(id: &str) -> DeviceDef {
        crate::device::builtin().into_iter().find(|d| d.id == id).unwrap()
    }

    #[test]
    fn write_gating_flags() {
        let kb = def("razer-blackwidow-v4-pro-75");
        let mouse = def("razer-basilisk-v3-pro");
        let set = Command::from_parts("oled.set", Some(r#"{"brightness": 50}"#)).unwrap();
        assert!(set.policy(&kb).needs_write && !set.write_confirmed());
        let hw = Command::from_parts("effect.hw", Some(r#"{"effect": "spectrum"}"#)).unwrap();
        assert!(!hw.policy(&kb).needs_write, "session effects do not touch onboard memory");
        assert!(hw.policy(&kb).gated.is_empty(), "showing an effect is never refused");
        let hw = Command::from_parts("effect.hw", Some(r#"{"effect": "spectrum", "storage": "onboard"}"#)).unwrap();
        assert!(hw.policy(&kb).needs_write);
        assert!(!Command::KeymapDump(LayerArgs { layer: Layer::Normal, profile: 1 }).policy(&kb).needs_write);
        assert!(Command::from_parts("nope", None).is_err());
        assert!(Command::from_parts("keymap.get", None).is_err(), "key is required");
        assert!(Command::from_parts("status", Some("null")).is_ok());
        // dpi alone is live (but checked); stages and poll rate are stored
        let dpi = Command::from_parts("performance.set", Some(r#"{"dpi": {"x": 800, "y": 800}}"#)).unwrap();
        assert_eq!(
            dpi.policy(&mouse),
            Policy {
                required_features: vec![&[Feature::Dpi]],
                missing: None,
                needs_write: false,
                gated: vec![Feature::Dpi]
            }
        );
        let poll = Command::from_parts("performance.set", Some(r#"{"poll_hz": 500}"#)).unwrap();
        assert!(poll.policy(&mouse).needs_write && !poll.write_confirmed());
        let power = Command::from_parts("power.set", Some(r#"{"idle_s": 300, "write": true}"#)).unwrap();
        assert!(power.policy(&mouse).needs_write && power.write_confirmed());
        assert_eq!(power.policy(&mouse).required_features, vec![&[Feature::Power][..]]);
        // what is missing, in the words of the refusal
        assert_eq!(Command::PerformanceGet.policy(&kb).unsupported(&kb).as_deref(), Some("dpi or poll_rate"));
        assert_eq!(Command::PerformanceGet.policy(&mouse).unsupported(&mouse), None);
        assert_eq!(set.policy(&mouse).unsupported(&mouse).as_deref(), Some("oled"));
        // the scroll wheel is stored in the mouse and checked first; device info is a plain read anywhere
        let scroll = Command::from_parts("scroll.set", Some(r#"{"mode": "tactile", "write": true}"#)).unwrap();
        assert_eq!(
            scroll.policy(&mouse),
            Policy {
                required_features: vec![&[Feature::Scroll]],
                missing: None,
                needs_write: true,
                gated: vec![Feature::Scroll]
            }
        );
        assert!(scroll.write_confirmed());
        assert_eq!(scroll.policy(&kb).unsupported(&kb).as_deref(), Some("scroll"));
        assert!(Command::from_parts("scroll.set", Some(r#"{"mode": "wobbly"}"#)).is_err());
        assert_eq!(Command::InfoGet.policy(&kb), Policy::default());
        assert_eq!(Command::InfoGet.policy(&mouse).unsupported(&mouse), None);
    }

    #[test]
    fn a_varstore_mouse_stores_even_the_live_dpi() {
        let mut m = def("razer-basilisk-v3-pro");
        let dpi = Command::from_parts("performance.set", Some(r#"{"dpi": {"x": 800, "y": 800}}"#)).unwrap();
        assert!(!dpi.policy(&m).needs_write);
        m.dpi.as_mut().unwrap().storage = DpiStorage::Varstore;
        assert!(dpi.policy(&m).needs_write);
        let stages = Command::from_parts("performance.set", Some(r#"{"stages": {"active": 1, "list": []}}"#)).unwrap();
        assert_eq!(stages.policy(&m).missing, None);
        m.dpi.as_mut().unwrap().stages_max = 0;
        assert_eq!(stages.policy(&m).unsupported(&m).as_deref(), Some("DPI stages"));
    }

    /// One example of every command, with `write: true` and every optional setting given, so each one's
    /// most demanding policy shows.
    fn every_command() -> Vec<Command> {
        let all = r##"{"key": "#1", "function": "button 1", "mode": "ZOOM", "brightness": 50,
            "effect": "spectrum", "storage": "onboard", "dpi": {"x": 800, "y": 800},
            "stages": {"active": 1, "list": [{"x": 800, "y": 800}]}, "poll_hz": 1000, "idle_s": 300,
            "low_battery_pct": 15, "write": true}"##;
        // `mode` above is a dial mode; the scroll wheel's comes separately
        let mut v: Vec<Command> = Command::NAMES
            .iter()
            .filter(|n| **n != "scroll.set")
            .map(|n| Command::from_parts(n, Some(all)).unwrap())
            .collect();
        let scroll = r#"{"mode": "free_spin", "acceleration": true, "smart_reel": true, "write": true}"#;
        v.push(Command::from_parts("scroll.set", Some(scroll)).unwrap());
        // and the session effect / live DPI variants
        v.push(Command::from_parts("effect.hw", Some(r#"{"effect": "spectrum"}"#)).unwrap());
        v.push(Command::from_parts("performance.set", Some(r#"{"dpi": {"x": 800, "y": 800}}"#)).unwrap());
        v
    }

    /// Commands that write onboard memory without a read-only check in front. None today; a command added
    /// here needs a reason next to it (for example: the device has no getter, so there is nothing to check).
    const WRITES_WITHOUT_A_CHECK: &[&str] = &[];

    /// The invariant behind "experimental devices are checked before anything is stored": every command
    /// that needs `write`, on every known device, is gated by the read-only check of a feature it requires
    /// (one that has a real check: lighting is never refused), unless it is listed as exempt above.
    #[test]
    fn every_write_is_gated_by_a_check() {
        for d in crate::device::builtin() {
            for c in every_command() {
                let p = c.policy(&d);
                if !p.needs_write || WRITES_WITHOUT_A_CHECK.contains(&c.name()) {
                    continue;
                }
                assert!(!p.gated.is_empty(), "{} on {} writes without a check", c.name(), d.id);
                for f in &p.gated {
                    assert_ne!(*f, Feature::Lighting, "{}: the lighting check never refuses", c.name());
                    assert!(
                        p.required_features.iter().any(|g| g.contains(f)),
                        "{} on {}: gated by {f:?}, which it does not require",
                        c.name(),
                        d.id
                    );
                }
            }
        }
    }

    #[test]
    fn coded_errors() {
        let e = coded(codes::LEFT_CLICK_GUARD, "no left click");
        assert_eq!(e.to_string(), "no left click");
        assert_eq!(e.downcast_ref::<CodedError>().unwrap().code, "left_click_guard");
        let r = Response::err_code(Some(raw(&1)), Some(codes::CHECK_FAILED), "nope");
        assert_eq!(r.to_line(), "{\"id\":1,\"ok\":false,\"error\":\"nope\",\"code\":\"check_failed\"}\n");
        assert_eq!(Response::err(None, "x").to_line(), "{\"ok\":false,\"error\":\"x\"}\n");
        let c = FeatureCheck { feature: Feature::PollRate, state: CheckState::NotNeeded, detail: None };
        assert_eq!(serde_json::to_string(&c).unwrap(), r#"{"feature":"poll_rate","state":"not_needed","detail":null}"#);
    }

    #[test]
    fn responses() {
        let r = Response::ok(Some(raw(&3)), raw(&serde_json::json!({"max": 5, "count": 1, "ids": [1]})));
        let line = r.to_line();
        assert_eq!(line, "{\"id\":3,\"ok\":true,\"result\":{\"count\":1,\"ids\":[1],\"max\":5}}\n");
        let r: Response = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(r.id_text(), Some("3"));
        let p: crate::features::profile::ProfileInfo = r.into_result().unwrap();
        assert_eq!(p.max, 5);
        let e = Response::err(Some(raw(&4)), "boom");
        assert_eq!(e.into_result::<u8>().unwrap_err().to_string(), "boom");
        // the code survives the trip back
        let line = Response::err_code(Some(raw(&5)), Some(codes::CHECK_FAILED), "nope").to_line();
        let e = serde_json::from_str::<Response>(line.trim_end()).unwrap().into_result::<u8>().unwrap_err();
        assert_eq!(e.to_string(), "nope");
        assert_eq!(e.downcast_ref::<CodedError>().map(|c| c.code), Some(codes::CHECK_FAILED));
        let odd: Response = serde_json::from_str(r#"{"ok":false,"error":"x","code":"from_the_future"}"#).unwrap();
        assert!(odd.into_result::<u8>().unwrap_err().downcast_ref::<CodedError>().is_none());
    }

    #[test]
    fn connection_errors_without_an_id_answer_any_call() {
        let busy: Response = serde_json::from_str(r#"{"ok":false,"error":"too many clients"}"#).unwrap();
        assert!(busy.id.is_none() && !busy.ok);
        assert_eq!(busy.into_result::<u8>().unwrap_err().to_string(), "too many clients");
    }

    #[test]
    fn device_resolution() {
        let devs = [
            ("razer-blackwidow-v4-pro-75", "Razer BlackWidow V4 Pro 75%", Kind::Keyboard),
            ("razer-basilisk-v3-pro", "Razer Basilisk V3 Pro", Kind::Mouse),
            ("razer-goliathus-chroma-extended", "Razer Goliathus Chroma Extended", Kind::Mousemat),
        ];
        assert_eq!(resolve_device("keyboard", &devs).unwrap(), "razer-blackwidow-v4-pro-75");
        assert_eq!(resolve_device("basilisk", &devs).unwrap(), "razer-basilisk-v3-pro");
        assert_eq!(resolve_device("mat", &devs).unwrap(), "razer-goliathus-chroma-extended");
        assert_eq!(resolve_device("RAZER-BASILISK-V3-PRO", &devs).unwrap(), "razer-basilisk-v3-pro");
        assert!(resolve_device("razer", &devs).is_err());
        assert!(resolve_device("headset", &devs).is_err());
    }
}
