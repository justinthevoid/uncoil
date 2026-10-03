//! Onboard key / button mapping ("OBM"): what each key does, per profile and per layer, stored in the
//! device itself. On the BlackWidow V4 Pro 75% the Hypershift layer *is* the Fn layer.
//!
//! | class/id | name | args → reply |
//! |---|---|---|
//! | `02/8D` | get key mapping (keyboard) | `[profile, key, layer]` → `[profile, key, layer, fn, len, data…]` |
//! | `02/0D` | set key mapping (keyboard) | `[profile, key, layer, fn, len, data…]` |
//! | `02/8C` / `02/0C` | the same for mice (OpenSynapse `ViperObmProtocol`) | |
//! | `02/84` | list button ids (mice) | → `[count, ids…]` |
//!
//! `data` is at most 5 bytes. Mice echo layer 0 even for Hypershift requests, so replies are matched by
//! request context, not by the echo (OpenSynapse notes the same).
//!
//! A mapping is written here as a short "spec" string, used by the CLI, by device TOML defaults and in
//! keymap backup files: `off`, `key PRINT_SCREEN`, `key A +lctrl`, `button 1`, `razer 4`, `power 0x82`,
//! `profile 4`, `dpi 5 1 144 1 144`, `turbo-button 104 20`, `raw 10 0 233`, …

use crate::features::parse_u8;
use crate::proto::{Reply, Report};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const CLASS: u8 = 0x02;
pub const KEYBOARD_GET: u8 = 0x8D;
pub const KEYBOARD_SET: u8 = 0x0D;
pub const MOUSE_GET: u8 = 0x8C;
pub const MOUSE_SET: u8 = 0x0C;
pub const MOUSE_BUTTON_IDS: u8 = 0x84;
/// Data size byte Synapse / OpenSynapse use for mapping requests.
const REQUEST_SIZE: u8 = 0x50;
pub const MAX_DATA: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    #[default]
    Normal = 0,
    /// Hypershift; on the BlackWidow V4 Pro 75% this is the Fn layer.
    #[serde(alias = "fn")]
    Hypershift = 1,
}

impl Layer {
    pub fn parse(s: &str) -> Option<Layer> {
        match s.to_ascii_lowercase().as_str() {
            "normal" | "0" | "base" => Some(Layer::Normal),
            "hypershift" | "fn" | "1" | "hyper" => Some(Layer::Hypershift),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Normal => "normal",
            Layer::Hypershift => "hypershift",
        }
    }
}

/// What a key does. Function ids are shared by Razer keyboards and mice (OpenSynapse `ViperObmFunctionId`,
/// names as Synapse logs them in `fnIdEnum`). In JSON and TOML a mapping is its spec string
/// (`"key PRINT_SCREEN"`), see [`Function::parse_spec`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Function {
    /// 0 — does nothing.
    Off,
    /// 1 — a mouse button: 1 left, 2 right, 3 middle, 4 back, 5 forward, 9/10 wheel up/down,
    /// 104/105 wheel tilt left/right.
    MouseButton { button: u8 },
    /// 2 — a keyboard key: HID usage (page 7) plus modifier bits (1 LCtrl, 2 LShift, 4 LAlt, 8 LWin,
    /// 16 RCtrl, 32 RShift, 64 RAlt, 128 RWin). Modifier keys themselves are usage 0 + their bit.
    Key { modifiers: u8, usage: u8 },
    /// 3, 4, 5, 15 — macro types I–IV (data layout not mapped; data is the macro reference).
    Macro { function: u8, data: Vec<u8> },
    /// 6 — DPI: `[1]`/`[2]` stage up/down (inferred), `[6]` cycle up, `[7]` cycle down (inferred),
    /// `[5, x_hi, x_lo, y_hi, y_lo]` clutch.
    Dpi { data: Vec<u8> },
    /// 7 — profile switching; `4` = cycle up (only value seen in Synapse's logs).
    Profile { action: u8 },
    /// 8 — lighting (layout not mapped).
    Lighting { data: Vec<u8> },
    /// 9 — system power key (HID generic desktop usage, e.g. `0x82` sleep).
    PowerKey { code: u8 },
    /// 10 — consumer / media key (2 data bytes; layout not mapped on the keyboard).
    MediaKey { data: Vec<u8> },
    /// 11 — double click (`[1]`).
    DoubleClick { button: u8 },
    /// 12 — Hypershift / mode key (`[1]`).
    Hypershift { data: Vec<u8> },
    /// 13 — turbo key (4 data bytes, not mapped).
    TurboKey { data: Vec<u8> },
    /// 14 — turbo mouse button: repeat `button` every `interval_ms` (Synapse: 50/s → 20 ms).
    TurboButton { button: u8, interval_ms: u16 },
    /// 16 — controller (not mapped).
    Controller { data: Vec<u8> },
    /// 17 — "Razer key": sent to the host in input report 4 (Fn functions Synapse handles in software).
    RazerKey { code: u8 },
    /// 18 — Windows shortcut on keyboards; on the Basilisk V3 Pro Synapse names it ScrollWheelMode.
    Shortcut { data: Vec<u8> },
    /// Anything else, kept verbatim.
    Raw { function: u8, data: Vec<u8> },
}

impl Function {
    /// (function id, data bytes) as stored in the device.
    pub fn encode(&self) -> (u8, Vec<u8>) {
        match self {
            Function::Off => (0, vec![]),
            Function::MouseButton { button } => (1, vec![*button]),
            Function::Key { modifiers, usage } => (2, vec![*modifiers, *usage]),
            Function::Macro { function, data } => (*function, data.clone()),
            Function::Dpi { data } => (6, data.clone()),
            Function::Profile { action } => (7, vec![*action]),
            Function::Lighting { data } => (8, data.clone()),
            Function::PowerKey { code } => (9, vec![*code]),
            Function::MediaKey { data } => (10, data.clone()),
            Function::DoubleClick { button } => (11, vec![*button]),
            Function::Hypershift { data } => (12, data.clone()),
            Function::TurboKey { data } => (13, data.clone()),
            Function::TurboButton { button, interval_ms } => {
                (14, vec![*button, (interval_ms >> 8) as u8, *interval_ms as u8])
            }
            Function::Controller { data } => (16, data.clone()),
            Function::RazerKey { code } => (17, vec![*code]),
            Function::Shortcut { data } => (18, data.clone()),
            Function::Raw { function, data } => (*function, data.clone()),
        }
    }

    /// Decode a stored (function id, data) pair. Unknown or malformed combinations become [`Function::Raw`]
    /// so nothing is lost on a read → write round trip.
    pub fn decode(function: u8, data: &[u8]) -> Function {
        let d = data.to_vec();
        let one =
            |f: fn(u8) -> Function| if d.len() == 1 { f(d[0]) } else { Function::Raw { function, data: d.clone() } };
        match function {
            0 if d.is_empty() => Function::Off,
            1 => one(|button| Function::MouseButton { button }),
            2 if d.len() == 2 => Function::Key { modifiers: d[0], usage: d[1] },
            3 | 4 | 5 | 15 => Function::Macro { function, data: d },
            6 => Function::Dpi { data: d },
            7 => one(|action| Function::Profile { action }),
            8 => Function::Lighting { data: d },
            9 => one(|code| Function::PowerKey { code }),
            10 => Function::MediaKey { data: d },
            11 => one(|button| Function::DoubleClick { button }),
            12 => Function::Hypershift { data: d },
            13 => Function::TurboKey { data: d },
            14 if d.len() == 3 => Function::TurboButton { button: d[0], interval_ms: u16::from_be_bytes([d[1], d[2]]) },
            16 => Function::Controller { data: d },
            17 => one(|code| Function::RazerKey { code }),
            18 => Function::Shortcut { data: d },
            _ => Function::Raw { function, data: d },
        }
    }

    pub fn function_id(&self) -> u8 {
        self.encode().0
    }

    /// Synapse's name for the function id (`fnIdEnum`).
    pub fn function_name(id: u8) -> &'static str {
        match id {
            0 => "Off",
            1 => "ButtonCode",
            2 => "KeyCode",
            3 => "MacroTypeI",
            4 => "MacroTypeII",
            5 => "MacroTypeIII",
            6 => "DPI",
            7 => "Profile",
            8 => "Lighting",
            9 => "PowerKeys",
            10 => "MediaKeys",
            11 => "DoubleClick",
            12 => "ModeButtonKey",
            13 => "TurboModeKey",
            14 => "TurboModeButton",
            15 => "MacroTypeIV",
            16 => "Controller",
            17 => "RazerKey",
            18 => "WindowsShortcutsKey",
            _ => "Unknown",
        }
    }

    /// Parse a spec string such as `key PRINT_SCREEN`, `key A +lctrl`, `razer 4`, `off`.
    pub fn parse_spec(spec: &str) -> Result<Function> {
        let t: Vec<&str> = spec.split_whitespace().collect();
        let Some((&head, rest)) = t.split_first() else { bail!("empty mapping") };
        let bytes = |r: &[&str]| -> Result<Vec<u8>> {
            r.iter().map(|s| parse_u8(s).ok_or_else(|| anyhow!("not a byte: {s}"))).collect()
        };
        let byte = |r: &[&str]| -> Result<u8> {
            match r {
                [b] => parse_u8(b).ok_or_else(|| anyhow!("not a byte: {b}")),
                _ => bail!("`{head}` takes exactly one value"),
            }
        };
        let f = match head.to_ascii_lowercase().as_str() {
            "off" | "none" | "disable" | "disabled" => Function::Off,
            "button" | "mouse" => Function::MouseButton { button: byte(rest)? },
            "key" => {
                let mut modifiers = 0u8;
                let mut usage = 0u8;
                for tok in rest {
                    if let Some(m) = tok.strip_prefix('+') {
                        modifiers |= modifier_bit(m).ok_or_else(|| anyhow!("unknown modifier `{m}`"))?;
                    } else if let Some(bit) = modifier_bit(tok) {
                        modifiers |= bit;
                    } else {
                        usage = usage_from_name(tok).ok_or_else(|| anyhow!("unknown key `{tok}`"))?;
                    }
                }
                Function::Key { modifiers, usage }
            }
            "macro" => match bytes(rest)?.split_first() {
                Some((&function, data)) if matches!(function, 3 | 4 | 5 | 15) => {
                    Function::Macro { function, data: data.to_vec() }
                }
                _ => bail!("`macro <3|4|5|15> <data…>`"),
            },
            "dpi" => Function::Dpi { data: bytes(rest)? },
            "profile" => Function::Profile { action: byte(rest)? },
            "lighting" => Function::Lighting { data: bytes(rest)? },
            "power" => Function::PowerKey { code: byte(rest)? },
            "media" => Function::MediaKey { data: bytes(rest)? },
            "double-click" | "doubleclick" => Function::DoubleClick { button: byte(rest)? },
            "hypershift" => Function::Hypershift { data: bytes(rest)? },
            "turbo-key" => Function::TurboKey { data: bytes(rest)? },
            "turbo-button" => match rest {
                [b, ms] => Function::TurboButton {
                    button: parse_u8(b).ok_or_else(|| anyhow!("not a byte: {b}"))?,
                    interval_ms: ms.parse().map_err(|_| anyhow!("not a number: {ms}"))?,
                },
                _ => bail!("`turbo-button <button> <interval_ms>`"),
            },
            "controller" => Function::Controller { data: bytes(rest)? },
            "razer" => Function::RazerKey { code: byte(rest)? },
            "shortcut" => Function::Shortcut { data: bytes(rest)? },
            "raw" => match bytes(rest)?.split_first() {
                Some((&function, data)) => Function::decode(function, data),
                None => bail!("`raw <function id> <data…>`"),
            },
            other => {
                bail!("unknown mapping type `{other}` (off, key, button, razer, power, media, profile, dpi, raw, …)")
            }
        };
        let (_, data) = f.encode();
        if data.len() > MAX_DATA {
            bail!("mapping data is {} bytes; the device stores at most {MAX_DATA}", data.len());
        }
        Ok(f)
    }

    /// A human description, e.g. "Print Screen" or "Razer key 4 (macro record)".
    pub fn describe(&self) -> String {
        match self {
            Function::Off => "nothing".into(),
            Function::Key { modifiers: 0, usage: 0 } => "empty key code (does nothing)".into(),
            Function::Key { modifiers, usage } => {
                let mut parts: Vec<String> = MODIFIERS
                    .iter()
                    .filter(|(bit, _)| modifiers & bit != 0)
                    .map(|(_, n)| {
                        let (side, key) = n.split_at(1);
                        format!("{}{}", side.to_uppercase(), title(key))
                    })
                    .collect();
                if *usage != 0 {
                    parts.push(usage_name(*usage).map(|n| title(&n)).unwrap_or_else(|| format!("usage 0x{usage:02X}")));
                }
                parts.join("+")
            }
            Function::RazerKey { code } => match razer_key_name(*code) {
                Some(n) => format!("Razer key {code} ({n}; handled by host software)"),
                None => format!("Razer key {code} (handled by host software)"),
            },
            Function::PowerKey { code: 0x82 } => "system sleep".into(),
            Function::Profile { action: 4 } => "next profile".into(),
            Function::MouseButton { button } => format!("mouse button {button}"),
            other => other.to_string(),
        }
    }
}

impl fmt::Display for Function {
    /// The spec form, accepted back by [`Function::parse_spec`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = |d: &[u8]| d.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(" ");
        match self {
            Function::Off => write!(f, "off"),
            Function::MouseButton { button } => write!(f, "button {button}"),
            Function::Key { modifiers, usage } => {
                write!(f, "key")?;
                if *usage != 0 || *modifiers == 0 {
                    match usage_name(*usage) {
                        Some(n) => write!(f, " {n}")?,
                        None => write!(f, " 0x{usage:02X}")?,
                    }
                }
                for (bit, name) in MODIFIERS {
                    if modifiers & bit != 0 {
                        write!(f, " +{name}")?;
                    }
                }
                Ok(())
            }
            Function::Macro { function, data } => write!(f, "macro {function} {}", list(data)),
            Function::Dpi { data } => write!(f, "dpi {}", list(data)),
            Function::Profile { action } => write!(f, "profile {action}"),
            Function::Lighting { data } => write!(f, "lighting {}", list(data)),
            Function::PowerKey { code } => write!(f, "power 0x{code:02X}"),
            Function::MediaKey { data } => write!(f, "media {}", list(data)),
            Function::DoubleClick { button } => write!(f, "double-click {button}"),
            Function::Hypershift { data } => write!(f, "hypershift {}", list(data)),
            Function::TurboKey { data } => write!(f, "turbo-key {}", list(data)),
            Function::TurboButton { button, interval_ms } => write!(f, "turbo-button {button} {interval_ms}"),
            Function::Controller { data } => write!(f, "controller {}", list(data)),
            Function::RazerKey { code } => write!(f, "razer {code}"),
            Function::Shortcut { data } => write!(f, "shortcut {}", list(data)),
            Function::Raw { function, data } => write!(f, "raw {function} {}", list(data)),
        }
    }
}

/// "PRINT_SCREEN" -> "Print Screen", "ctrl" -> "Ctrl".
fn title(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c.flat_map(|x| x.to_lowercase())).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

impl Serialize for Function {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Function {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Function::parse_spec(&s).map_err(serde::de::Error::custom)
    }
}

/// Names for the Razer key codes Synapse's logs show (`razerKeyAssignment`) and the dial codes seen in
/// input report 4.
pub fn razer_key_name(code: u8) -> Option<&'static str> {
    Some(match code {
        1 => "Fn held",
        3 => "game mode",
        4 => "macro record",
        8 => "backlight up",
        9 => "backlight down",
        11 => "low power mode",
        76 => "system sleep",
        82 => "dial click",
        83 => "next",
        84 => "previous",
        85 => "play/pause",
        96 => "dial",
        _ => return None,
    })
}

const MODIFIERS: [(u8, &str); 8] = [
    (0x01, "lctrl"),
    (0x02, "lshift"),
    (0x04, "lalt"),
    (0x08, "lwin"),
    (0x10, "rctrl"),
    (0x20, "rshift"),
    (0x40, "ralt"),
    (0x80, "rwin"),
];

fn norm(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, '_' | ' ' | '-')).flat_map(|c| c.to_uppercase()).collect()
}

pub fn modifier_bit(name: &str) -> Option<u8> {
    Some(match norm(name).as_str() {
        "LCTRL" | "LEFTCTRL" | "LEFTCONTROL" | "CTRL" | "CONTROL" => 0x01,
        "LSHIFT" | "LEFTSHIFT" | "SHIFT" => 0x02,
        "LALT" | "LEFTALT" | "ALT" => 0x04,
        "LWIN" | "LEFTWIN" | "LEFTWINDOWS" | "WIN" | "GUI" | "META" | "SUPER" => 0x08,
        "RCTRL" | "RIGHTCTRL" | "RIGHTCONTROL" => 0x10,
        "RSHIFT" | "RIGHTSHIFT" => 0x20,
        "RALT" | "RIGHTALT" | "ALTGR" => 0x40,
        "RWIN" | "RIGHTWIN" | "RIGHTWINDOWS" => 0x80,
        _ => return None,
    })
}

/// HID keyboard-page usage names (canonical name first). Covers everything a 75% / full-size board sends.
const USAGES: &[(u8, &str)] = &[
    (0x28, "ENTER"),
    (0x29, "ESCAPE"),
    (0x2A, "BACKSPACE"),
    (0x2B, "TAB"),
    (0x2C, "SPACE"),
    (0x2D, "MINUS"),
    (0x2E, "EQUAL"),
    (0x2F, "LEFT_BRACKET"),
    (0x30, "RIGHT_BRACKET"),
    (0x31, "BACKSLASH"),
    (0x32, "NON_US_HASH"),
    (0x33, "SEMICOLON"),
    (0x34, "APOSTROPHE"),
    (0x35, "GRAVE"),
    (0x36, "COMMA"),
    (0x37, "PERIOD"),
    (0x38, "SLASH"),
    (0x39, "CAPS_LOCK"),
    (0x46, "PRINT_SCREEN"),
    (0x47, "SCROLL_LOCK"),
    (0x48, "PAUSE"),
    (0x49, "INSERT"),
    (0x4A, "HOME"),
    (0x4B, "PAGE_UP"),
    (0x4C, "DELETE"),
    (0x4D, "END"),
    (0x4E, "PAGE_DOWN"),
    (0x4F, "RIGHT"),
    (0x50, "LEFT"),
    (0x51, "DOWN"),
    (0x52, "UP"),
    (0x53, "NUM_LOCK"),
    (0x54, "KP_SLASH"),
    (0x55, "KP_ASTERISK"),
    (0x56, "KP_MINUS"),
    (0x57, "KP_PLUS"),
    (0x58, "KP_ENTER"),
    (0x62, "KP_0"),
    (0x63, "KP_PERIOD"),
    (0x64, "NON_US_BACKSLASH"),
    (0x65, "APPLICATION"),
    (0x66, "POWER"),
    (0x67, "KP_EQUAL"),
    (0x7F, "MUTE"),
    (0x80, "VOLUME_UP"),
    (0x81, "VOLUME_DOWN"),
    (0x87, "INTL_RO"),
    (0x88, "KATAKANA_HIRAGANA"),
    (0x89, "YEN"),
    (0x8A, "HENKAN"),
    (0x8B, "MUHENKAN"),
    (0x90, "HANGEUL"),
    (0x91, "HANJA"),
];

const ALIASES: &[(&str, u8)] = &[
    ("ESC", 0x29),
    ("RETURN", 0x28),
    ("PRTSC", 0x46),
    ("PRINTSCR", 0x46),
    ("PRINTSCREEN", 0x46),
    ("DEL", 0x4C),
    ("INS", 0x49),
    ("PGUP", 0x4B),
    ("PGDN", 0x4E),
    ("PAGEDOWN", 0x4E),
    ("HYPHEN", 0x2D),
    ("DASH", 0x2D),
    ("TILDE", 0x35),
    ("BACKTICK", 0x35),
    ("MENU", 0x65),
    ("APP", 0x65),
    ("SPACEBAR", 0x2C),
    ("LEFTARROW", 0x50),
    ("RIGHTARROW", 0x4F),
    ("UPARROW", 0x52),
    ("DOWNARROW", 0x51),
];

/// Canonical name for a HID keyboard usage.
pub fn usage_name(usage: u8) -> Option<String> {
    match usage {
        0x00 => Some("NONE".into()),
        0x04..=0x1D => Some(((b'A' + usage - 0x04) as char).to_string()),
        0x1E..=0x26 => Some(((b'1' + usage - 0x1E) as char).to_string()),
        0x27 => Some("0".into()),
        0x3A..=0x45 => Some(format!("F{}", usage - 0x3A + 1)),
        0x59..=0x61 => Some(format!("KP_{}", usage - 0x59 + 1)),
        0x68..=0x73 => Some(format!("F{}", usage - 0x68 + 13)),
        _ => USAGES.iter().find(|(u, _)| *u == usage).map(|(_, n)| n.to_string()),
    }
}

/// HID usage for a key name ("PRINT_SCREEN", "prtsc", "F5", "a", "0x46").
pub fn usage_from_name(name: &str) -> Option<u8> {
    if name.starts_with("0x") || name.starts_with("0X") {
        return parse_u8(name);
    }
    let n = norm(name);
    let n = n.strip_prefix("KEY").filter(|r| !r.is_empty()).unwrap_or(&n).to_string();
    if n == "NONE" {
        return Some(0);
    }
    if n.len() == 1 {
        let c = n.as_bytes()[0];
        return match c {
            b'A'..=b'Z' => Some(c - b'A' + 0x04),
            b'1'..=b'9' => Some(c - b'1' + 0x1E),
            b'0' => Some(0x27),
            _ => None,
        };
    }
    if let Some(num) = n.strip_prefix('F').and_then(|r| r.parse::<u8>().ok()) {
        return match num {
            1..=12 => Some(0x3A + num - 1),
            13..=24 => Some(0x68 + num - 13),
            _ => None,
        };
    }
    if let Some(num) = n.strip_prefix("KP").and_then(|r| r.parse::<u8>().ok()) {
        return match num {
            0 => Some(0x62),
            1..=9 => Some(0x59 + num - 1),
            _ => None,
        };
    }
    USAGES
        .iter()
        .find(|(_, u)| norm(u) == n)
        .map(|(c, _)| *c)
        .or_else(|| ALIASES.iter().find(|(a, _)| *a == n).map(|(_, c)| *c))
}

/// Key / button table and command ids for one device, from its TOML `[keymap]` section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeymapDef {
    /// Get command id in class 0x02 (`0x8D` keyboards, `0x8C` mice).
    pub get: u8,
    /// Set command id (`0x0D` keyboards, `0x0C` mice).
    pub set: u8,
    /// Layers the device has.
    #[serde(default = "both_layers")]
    pub layers: Vec<Layer>,
    pub keys: Vec<KeyDef>,
}

fn both_layers() -> Vec<Layer> {
    vec![Layer::Normal, Layer::Hypershift]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDef {
    /// Razer key / button id.
    pub id: u8,
    /// Synapse's name (`inputID`) without the `KEY_` prefix.
    pub name: String,
    /// Matrix LED name of this key (links the key map to the lighting layout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub led: Option<String>,
    /// Factory mapping on the normal layer, as a spec string (see [`Function::parse_spec`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

impl KeyDef {
    pub fn default_function(&self) -> Option<Function> {
        self.default.as_deref().and_then(|s| Function::parse_spec(s).ok())
    }
}

impl KeymapDef {
    /// Find a key by Synapse name ("P", "KEY_P", "page_up"), LED name ("Page Up"), or id ("#26", or a bare
    /// number when no key has that name: "1" is the 1 key, "26" is P).
    pub fn find(&self, s: &str) -> Option<&KeyDef> {
        if let Some(id) = s.strip_prefix('#').and_then(parse_u8) {
            return self.by_id(id);
        }
        let n = norm(s);
        let n = n.strip_prefix("KEY").filter(|r| !r.is_empty()).unwrap_or(&n).to_string();
        self.keys
            .iter()
            .find(|k| norm(&k.name) == n || k.led.as_deref().map(norm).as_deref() == Some(n.as_str()))
            .or_else(|| parse_u8(s).and_then(|id| self.by_id(id)))
    }

    pub fn by_id(&self, id: u8) -> Option<&KeyDef> {
        self.keys.iter().find(|k| k.id == id)
    }

    pub fn name_of(&self, id: u8) -> String {
        self.by_id(id).map(|k| k.name.clone()).unwrap_or_else(|| format!("#{id}"))
    }

    pub fn get_report(&self, tid: u8, profile: u8, key: u8, layer: Layer) -> Report {
        Report::sized(tid, REQUEST_SIZE, CLASS, self.get, &[profile, key, layer as u8])
    }

    /// The set report. Layout verified on hardware: `02/0D [1, 26, 1, 2, 2, 0, 0x46]` made Fn+P Print Screen.
    pub fn set_report(&self, tid: u8, profile: u8, key: u8, layer: Layer, f: &Function) -> Result<Report> {
        let (function, data) = f.encode();
        if data.len() > MAX_DATA {
            bail!("mapping data is {} bytes; at most {MAX_DATA}", data.len());
        }
        let mut args = vec![profile, key, layer as u8, function, data.len() as u8];
        args.extend_from_slice(&data);
        Ok(Report::sized(tid, REQUEST_SIZE, CLASS, self.set, &args))
    }
}

/// Parse a get reply `[profile, key, layer, fn, len, data(5)]`. The layer echo is ignored (mice always
/// echo 0). Data beyond `len` is kept when non-zero: the Basilisk stores its DPI-clutch mapping as
/// `len 1` followed by all five bytes.
pub fn parse_reply(reply: &Reply, profile: u8, key: u8) -> Result<Function> {
    let a = &reply.raw;
    if a[0] != profile || a[1] != key {
        bail!("mapping reply is for profile {} key {}, asked for profile {profile} key {key}", a[0], a[1]);
    }
    let len = (a[4] as usize).min(MAX_DATA);
    let data = &a[5..5 + MAX_DATA];
    let used = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1).max(len);
    Ok(Function::decode(a[3], &data[..used]))
}

/// `02/84`: list of button ids (mice).
pub fn get_button_ids(tid: u8) -> Report {
    Report::sized(tid, REQUEST_SIZE, CLASS, MOUSE_BUTTON_IDS, &[])
}

/// `[count, ids…]`
pub fn parse_id_list(reply: &Reply) -> Vec<u8> {
    let a = reply.args();
    let n = a.first().copied().unwrap_or(0) as usize;
    a.iter().skip(1).take(n).copied().collect()
}

/// A key map backup file (`uncoil keymap export`): one spec string per key name and layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KeymapFile {
    pub device: String,
    pub profile: u8,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub normal: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hypershift: BTreeMap<String, String>,
}

impl KeymapFile {
    pub fn layer(&self, layer: Layer) -> &BTreeMap<String, String> {
        match layer {
            Layer::Normal => &self.normal,
            Layer::Hypershift => &self.hypershift,
        }
    }

    pub fn layer_mut(&mut self, layer: Layer) -> &mut BTreeMap<String, String> {
        match layer {
            Layer::Normal => &mut self.normal,
            Layer::Hypershift => &mut self.hypershift,
        }
    }

    pub fn to_toml(&self) -> Result<String> {
        let header = "# uncoil key map backup. Each entry: key name = mapping spec (see docs/PROTOCOL.md).\n";
        Ok(format!("{header}{}", toml::to_string(self)?))
    }

    pub fn from_toml(s: &str) -> Result<KeymapFile> {
        Ok(toml::from_str(s)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Status, MAX_ARGS};

    fn kb() -> KeymapDef {
        KeymapDef {
            get: KEYBOARD_GET,
            set: KEYBOARD_SET,
            layers: both_layers(),
            keys: vec![
                KeyDef { id: 26, name: "P".into(), led: Some("P".into()), default: Some("key P".into()) },
                KeyDef { id: 85, name: "PAGE_UP".into(), led: Some("Page Up".into()), default: None },
                KeyDef { id: 2, name: "1".into(), led: Some("1".into()), default: None },
            ],
        }
    }

    fn reply(args: &[u8]) -> Reply {
        let mut raw = [0u8; MAX_ARGS];
        raw[..args.len()].copy_from_slice(args);
        Reply { status: Status::Ok, transaction_id: 0x1F, size: args.len() as u8, class: 2, id: 0x8D, raw }
    }

    #[test]
    fn set_matches_verified_fn_p_write() {
        // tools/reference/obm_set_fnp.py wrote exactly these bytes and Fn+P became Print Screen.
        let r = kb().set_report(0x1F, 1, 26, Layer::Hypershift, &Function::parse_spec("key PRINT_SCREEN").unwrap());
        let w = r.unwrap().to_wire();
        assert_eq!(&w[1..16], &[0, 0x1F, 0, 0, 0, 0x50, 0x02, 0x0D, 1, 26, 1, 2, 2, 0, 0x46]);
        assert!(w[16..89].iter().all(|&b| b == 0));
    }

    #[test]
    fn get_matches_probe() {
        let w = kb().get_report(0x1F, 1, 26, Layer::Hypershift).to_wire();
        assert_eq!(&w[1..12], &[0, 0x1F, 0, 0, 0, 0x50, 0x02, 0x8D, 1, 26, 1]);
    }

    #[test]
    fn parses_real_keyboard_replies() {
        // Read from the BlackWidow V4 Pro 75% on 2026-10-03 (tools/reference/readonly_probe.py)
        assert_eq!(
            parse_reply(&reply(&[1, 26, 1, 2, 2, 0, 70]), 1, 26).unwrap(),
            Function::Key { modifiers: 0, usage: 0x46 }
        );
        assert_eq!(parse_reply(&reply(&[1, 76, 1, 17, 1, 76]), 1, 76).unwrap(), Function::RazerKey { code: 76 });
        assert_eq!(parse_reply(&reply(&[1, 120, 1, 17, 1, 4]), 1, 120).unwrap(), Function::RazerKey { code: 4 });
        assert!(parse_reply(&reply(&[1, 120, 1, 17, 1, 4]), 1, 26).is_err());
    }

    #[test]
    fn parses_real_mouse_replies() {
        // Basilisk V3 Pro: DPI clutch declares len 1 but stores all five bytes; profile button; scroll mode
        let clutch = parse_reply(&reply(&[1, 15, 0, 6, 1, 5, 1, 144, 1, 144]), 1, 15).unwrap();
        assert_eq!(clutch, Function::Dpi { data: vec![5, 1, 144, 1, 144] });
        assert_eq!(parse_reply(&reply(&[1, 14, 0, 7, 1, 4]), 1, 14).unwrap(), Function::Profile { action: 4 });
        assert_eq!(parse_reply(&reply(&[1, 106, 0, 18, 1, 1]), 1, 106).unwrap(), Function::Shortcut { data: vec![1] });
        assert_eq!(parse_reply(&reply(&[1, 4, 0, 1, 1, 4]), 1, 4).unwrap(), Function::MouseButton { button: 4 });
    }

    #[test]
    fn turbo_matches_synapse_bytes() {
        // Synapse: TurboModeButton fnDataByte [104, 0, 20] = wheel tilt left at 50 presses/s
        let f = Function::decode(14, &[104, 0, 20]);
        assert_eq!(f, Function::TurboButton { button: 104, interval_ms: 20 });
        assert_eq!(f.encode(), (14, vec![104, 0, 20]));
    }

    #[test]
    fn every_function_id_roundtrips() {
        for id in 0..=20u8 {
            for data in [vec![], vec![1], vec![0, 0x46], vec![104, 0, 20], vec![5, 1, 144, 1, 144]] {
                let f = Function::decode(id, &data);
                assert_eq!(f.encode(), (id, data.clone()), "fn {id} data {data:?}");
                // and through the spec string
                let back = Function::parse_spec(&f.to_string()).unwrap();
                assert_eq!(back.encode(), (id, data.clone()), "spec `{f}`");
            }
        }
    }

    #[test]
    fn specs() {
        assert_eq!(Function::parse_spec("key A +lctrl").unwrap(), Function::Key { modifiers: 1, usage: 4 });
        assert_eq!(Function::parse_spec("key lshift").unwrap(), Function::Key { modifiers: 2, usage: 0 });
        assert_eq!(Function::parse_spec("key prtsc").unwrap(), Function::Key { modifiers: 0, usage: 0x46 });
        assert_eq!(Function::parse_spec("key F12").unwrap(), Function::Key { modifiers: 0, usage: 0x45 });
        assert_eq!(Function::parse_spec("key 0x46").unwrap().to_string(), "key PRINT_SCREEN");
        assert_eq!(Function::Key { modifiers: 0x22, usage: 0 }.to_string(), "key +lshift +rshift");
        assert_eq!(Function::Key { modifiers: 0, usage: 0 }.to_string(), "key NONE");
        assert_eq!(Function::parse_spec("razer 4").unwrap(), Function::RazerKey { code: 4 });
        assert!(Function::parse_spec("key NOPE").is_err());
        assert!(Function::parse_spec("raw 2 1 2 3 4 5 6").is_err());
        assert!(Function::parse_spec("").is_err());
    }

    #[test]
    fn descriptions() {
        assert_eq!(Function::Key { modifiers: 0, usage: 0x46 }.describe(), "Print Screen");
        assert_eq!(Function::Key { modifiers: 0x05, usage: 0x04 }.describe(), "LCtrl+LAlt+A");
        assert_eq!(Function::Key { modifiers: 0x02, usage: 0 }.describe(), "LShift");
        assert_eq!(Function::RazerKey { code: 4 }.describe(), "Razer key 4 (macro record; handled by host software)");
    }

    #[test]
    fn usage_names_roundtrip() {
        for u in 0u8..=0x91 {
            if let Some(n) = usage_name(u) {
                assert_eq!(usage_from_name(&n), Some(u), "{n}");
            }
        }
    }

    #[test]
    fn key_lookup() {
        let k = kb();
        assert_eq!(k.find("P").unwrap().id, 26);
        assert_eq!(k.find("key_p").unwrap().id, 26);
        assert_eq!(k.find("26").unwrap().id, 26);
        assert_eq!(k.find("#2").unwrap().name, "1");
        assert_eq!(k.find("page up").unwrap().id, 85);
        assert_eq!(k.find("PAGE_UP").unwrap().id, 85);
        assert_eq!(k.find("1").unwrap().id, 2, "a key named 1 wins over id 1 when id 1 is absent");
        assert!(k.find("nope").is_none());
        assert_eq!(k.find("P").unwrap().default_function(), Some(Function::Key { modifiers: 0, usage: 0x13 }));
    }

    #[test]
    fn backup_file_roundtrip() {
        let mut f = KeymapFile { device: "razer-blackwidow-v4-pro-75".into(), profile: 1, ..Default::default() };
        f.hypershift.insert("P".into(), "key PRINT_SCREEN".into());
        f.hypershift.insert("F9".into(), "razer 4".into());
        let s = f.to_toml().unwrap();
        assert!(s.contains("[hypershift]"));
        assert_eq!(KeymapFile::from_toml(&s).unwrap(), f);
    }

    #[test]
    fn json_shape_for_gui() {
        let j = serde_json::to_string(&Function::Key { modifiers: 0, usage: 0x46 }).unwrap();
        assert_eq!(j, r#""key PRINT_SCREEN""#);
        let f: Function = serde_json::from_str(r#""key a +lctrl""#).unwrap();
        assert_eq!(f, Function::Key { modifiers: 1, usage: 4 });
        assert!(serde_json::from_str::<Function>(r#""key nope""#).is_err());
        let l: Layer = serde_json::from_str("\"fn\"").unwrap();
        assert_eq!(l, Layer::Hypershift);
    }
}
