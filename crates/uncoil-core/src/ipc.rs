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

use crate::device::Kind;
use crate::features::dial::DialMode;
use crate::features::hw_effect::{HwEffect, Region, Storage};
use crate::features::keymap::{Function, Layer};
use crate::features::Feature;
use anyhow::{anyhow, bail, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

pub const PIPE_NAME: &str = r"\\.\pipe\uncoil";
/// Longest request line the daemon accepts.
pub const MAX_LINE: usize = 64 * 1024;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Raw>,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Raw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(id: Option<Raw>, result: Raw) -> Response {
        Response { id, ok: true, result: Some(result), error: None }
    }

    pub fn err(id: Option<Raw>, error: impl Into<String>) -> Response {
        Response { id, ok: false, result: None, error: Some(error.into()) }
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

    /// The typed result, or the daemon's error.
    pub fn into_result<T: DeserializeOwned>(self) -> Result<T> {
        if !self.ok {
            bail!("{}", self.error.unwrap_or_else(|| "daemon reported an error".into()));
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

/// Every command the daemon understands.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Status,
    Devices,
    Capabilities(CapabilitiesArgs),
    KeymapGet(KeyArgs),
    KeymapSet(KeySetArgs),
    KeymapReset(KeyResetArgs),
    KeymapDump(LayerArgs),
    ProfileList,
    DialGet(ProfileArgs),
    DialSet(DialSetArgs),
    OledGet,
    OledSet(OledSetArgs),
    EffectHw(EffectHwArgs),
    /// Drop a firmware effect and go back to the configured software effect.
    EffectSoftware,
}

fn args<T: DeserializeOwned>(cmd: &str, a: Option<&str>) -> Result<T> {
    let a = match a {
        None | Some("null") => "{}",
        Some(s) => s,
    };
    serde_json::from_str(a).map_err(|e| anyhow!("{cmd}: bad args: {e}"))
}

fn to_value<T: Serialize>(t: &T) -> Option<Raw> {
    Some(raw(t))
}

impl Command {
    pub const NAMES: [&'static str; 14] = [
        "status",
        "devices",
        "capabilities",
        "keymap.get",
        "keymap.set",
        "keymap.reset",
        "keymap.dump",
        "profile.list",
        "dial.get",
        "dial.set",
        "oled.get",
        "oled.set",
        "effect.hw",
        "effect.software",
    ];

    pub fn from_parts(cmd: &str, a: Option<&str>) -> Result<Command> {
        Ok(match cmd {
            "status" => Command::Status,
            "devices" => Command::Devices,
            "capabilities" => Command::Capabilities(args(cmd, a)?),
            "keymap.get" => Command::KeymapGet(args(cmd, a)?),
            "keymap.set" => Command::KeymapSet(args(cmd, a)?),
            "keymap.reset" => Command::KeymapReset(args(cmd, a)?),
            "keymap.dump" => Command::KeymapDump(args(cmd, a)?),
            "profile.list" => Command::ProfileList,
            "dial.get" => Command::DialGet(args(cmd, a)?),
            "dial.set" => Command::DialSet(args(cmd, a)?),
            "oled.get" => Command::OledGet,
            "oled.set" => Command::OledSet(args(cmd, a)?),
            "effect.hw" => Command::EffectHw(args(cmd, a)?),
            "effect.software" => Command::EffectSoftware,
            other => bail!("unknown command `{other}` (known: {})", Command::NAMES.join(", ")),
        })
    }

    pub fn to_parts(&self) -> (&'static str, Option<Raw>) {
        match self {
            Command::Status => ("status", None),
            Command::Devices => ("devices", None),
            Command::Capabilities(a) => ("capabilities", to_value(a)),
            Command::KeymapGet(a) => ("keymap.get", to_value(a)),
            Command::KeymapSet(a) => ("keymap.set", to_value(a)),
            Command::KeymapReset(a) => ("keymap.reset", to_value(a)),
            Command::KeymapDump(a) => ("keymap.dump", to_value(a)),
            Command::ProfileList => ("profile.list", None),
            Command::DialGet(a) => ("dial.get", to_value(a)),
            Command::DialSet(a) => ("dial.set", to_value(a)),
            Command::OledGet => ("oled.get", None),
            Command::OledSet(a) => ("oled.set", to_value(a)),
            Command::EffectHw(a) => ("effect.hw", to_value(a)),
            Command::EffectSoftware => ("effect.software", None),
        }
    }

    /// Commands answered by the daemon itself rather than a device thread.
    pub fn is_daemon_level(&self) -> bool {
        matches!(self, Command::Status | Command::Devices | Command::Capabilities(_))
    }

    /// Does this command (as given) write the device's onboard memory?
    pub fn writes_onboard(&self) -> bool {
        match self {
            Command::KeymapSet(_) | Command::KeymapReset(_) | Command::DialSet(_) => true,
            Command::OledSet(a) => a.brightness.is_some(),
            Command::EffectHw(a) => a.storage == Storage::Onboard,
            _ => false,
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
            _ => false,
        }
    }

    /// Feature a device must declare for this command.
    pub fn feature(&self) -> Option<Feature> {
        Some(match self {
            Command::KeymapGet(_) | Command::KeymapSet(_) | Command::KeymapReset(_) | Command::KeymapDump(_) => {
                Feature::Keymap
            }
            Command::ProfileList => Feature::Profiles,
            Command::DialGet(_) | Command::DialSet(_) => Feature::Dial,
            Command::OledGet | Command::OledSet(_) => Feature::Oled,
            Command::EffectHw(_) => Feature::HwEffects,
            Command::EffectSoftware => Feature::Lighting,
            _ => return None,
        })
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

    pub fn connect_to(path: &str) -> Result<Client> {
        let mut last = None;
        for _ in 0..50 {
            match std::fs::OpenOptions::new().read(true).write(true).open(path) {
                Ok(pipe) => {
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
        use std::io::{BufRead, Write};
        let id = self.next_id;
        self.next_id += 1;
        self.pipe.write_all(Request::new(id, device, command).to_line().as_bytes())?;
        self.pipe.flush()?;
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line)? == 0 {
                bail!("uncoild closed the connection");
            }
            let r: Response = serde_json::from_str(line.trim_end())?;
            if r.id_text() == Some(id.to_string().as_str()) {
                return Ok(r);
            }
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
        ];
        let mut names = std::collections::HashSet::new();
        for c in cmds {
            let req = Request::new(1, None, &c);
            names.insert(req.cmd.clone());
            let parsed: Request = serde_json::from_str(req.to_line().trim_end()).unwrap();
            assert_eq!(parsed.command().unwrap(), c, "{}", req.cmd);
        }
        assert_eq!(names.len(), Command::NAMES.len());
    }

    #[test]
    fn write_gating_flags() {
        let set = Command::from_parts("oled.set", Some(r#"{"brightness": 50}"#)).unwrap();
        assert!(set.writes_onboard() && !set.write_confirmed());
        let hw = Command::from_parts("effect.hw", Some(r#"{"effect": "spectrum"}"#)).unwrap();
        assert!(!hw.writes_onboard(), "session effects do not touch onboard memory");
        let hw = Command::from_parts("effect.hw", Some(r#"{"effect": "spectrum", "storage": "onboard"}"#)).unwrap();
        assert!(hw.writes_onboard());
        assert!(!Command::KeymapDump(LayerArgs { layer: Layer::Normal, profile: 1 }).writes_onboard());
        assert!(Command::from_parts("nope", None).is_err());
        assert!(Command::from_parts("keymap.get", None).is_err(), "key is required");
        assert!(Command::from_parts("status", Some("null")).is_ok());
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
