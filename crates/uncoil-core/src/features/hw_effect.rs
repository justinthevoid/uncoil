//! Firmware ("hardware") lighting effects on the extended matrix: `0F/02 Set effect`.
//!
//! Every effect uses one argument layout (OpenRGB's `razer_create_mode_effect_extended_matrix_report`,
//! consistent with the byte captures in OpenRazer's `razerchromacommon.c`):
//!
//! | arg | meaning |
//! |----:|---------|
//! | 0 | storage: `0` = this session only, `1` = save to onboard memory ("VARSTORE") |
//! | 1 | LED / region id: `0` whole device, `5` keyboard backlight, `1` scroll wheel, `4` logo, `10` underglow |
//! | 2 | effect id (below) |
//! | 3 | flags: wave/wheel direction, or number of colours for breathing/starlight |
//! | 4 | rate: wave speed (lower = faster, firmware default `0x28`), reactive/starlight duration 1..4 |
//! | 5 | colour count |
//! | 6.. | colours, 3 bytes each |
//!
//! Data size is `6 + 3 * colours`.
//!
//! Effect ids: 0 off, 1 static, 2 breathing, 3 spectrum, 4 wave, 5 reactive, 6 ripple, 7 starlight,
//! 8 custom frame, 9 fire, 10 wheel (BlackWidow V4). What a device supports can be read from the device:
//! `0F/80` lists lighting regions as 5-byte records `[led, ?, ?, rows, cols]`, and `0F/81 [led]` returns
//! `[led, effect ids…]` (OpenRGB `razer_get_supported_effects`). The list is not exhaustive: OpenRazer runs
//! wave on the Basilisk V3 Pro although it is not listed.

use crate::color::Rgb;
use crate::proto::{Reply, Report};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const CLASS: u8 = 0x0F;
pub const SET_EFFECT: u8 = 0x02;
pub const SET_BRIGHTNESS: u8 = 0x04;
pub const GET_REGIONS: u8 = 0x80;
pub const GET_SUPPORTED_EFFECTS: u8 = 0x81;
pub const GET_BRIGHTNESS: u8 = 0x84;

pub const EFFECT_OFF: u8 = 0x00;
pub const EFFECT_STATIC: u8 = 0x01;
pub const EFFECT_BREATHING: u8 = 0x02;
pub const EFFECT_SPECTRUM: u8 = 0x03;
pub const EFFECT_WAVE: u8 = 0x04;
pub const EFFECT_REACTIVE: u8 = 0x05;
pub const EFFECT_RIPPLE: u8 = 0x06;
pub const EFFECT_STARLIGHT: u8 = 0x07;
pub const EFFECT_CUSTOM_FRAME: u8 = 0x08;
pub const EFFECT_FIRE: u8 = 0x09;
pub const EFFECT_WHEEL: u8 = 0x0A;

/// Firmware default wave speed (OpenRazer hard-codes it; lower is faster).
pub const DEFAULT_WAVE_SPEED: u8 = 0x28;

/// Where the effect is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Storage {
    /// Until the next effect / power cycle. Nothing is written to onboard memory.
    #[default]
    Session = 0x00,
    /// Saved in onboard memory (OpenRazer's VARSTORE): survives unplugging. An onboard write.
    Onboard = 0x01,
}

/// Wave / wheel direction. Firmware values follow OpenRGB's RGBController_Razer (left = 2, right = 1);
/// OpenRazer notes that some older devices use 0/1 instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Firmware value 2.
    Left,
    /// Firmware value 1.
    #[default]
    Right,
}

impl Direction {
    fn byte(self) -> u8 {
        match self {
            Direction::Left => 0x02,
            Direction::Right => 0x01,
        }
    }
}

/// A firmware effect. In JSON (control pipe) and on the command line it is a short spec string:
/// `off`, `static #ff0000`, `breathing`, `breathing #ff0000 #0000ff`, `spectrum`, `wave left speed 40`,
/// `wheel right`, `reactive #ffffff duration 2`, `starlight #ff0000 duration 1`.
#[derive(Debug, Clone, PartialEq)]
pub enum HwEffect {
    Off,
    Static {
        color: Rgb,
    },
    /// No colours = random colours, one or two colours = single / dual breathing.
    Breathing {
        colors: Vec<Rgb>,
    },
    Spectrum,
    Wave {
        direction: Direction,
        speed: u8,
    },
    /// Circular wave around a centre (BlackWidow V4 family).
    Wheel {
        direction: Direction,
        speed: u8,
    },
    /// Keys light up when pressed. `duration` 1 (short) ..= 4 (long).
    Reactive {
        color: Rgb,
        duration: u8,
    },
    /// Random twinkles. No colours = random, else one or two colours. `duration` 1..=3.
    Starlight {
        colors: Vec<Rgb>,
        duration: u8,
    },
}

/// Default reactive / starlight duration.
pub const DEFAULT_DURATION: u8 = 2;

impl HwEffect {
    /// Parse a spec string (see the type docs).
    pub fn parse_spec(spec: &str) -> Result<HwEffect> {
        let mut t = spec.split_whitespace();
        let name = t.next().ok_or_else(|| anyhow::anyhow!("empty effect"))?.to_ascii_lowercase();
        let (mut colors, mut direction, mut speed, mut duration) = (vec![], Direction::default(), None, None);
        while let Some(tok) = t.next() {
            let mut num = |what: &str| -> Result<u8> {
                let v = t.next().ok_or_else(|| anyhow::anyhow!("{what} needs a value"))?;
                crate::features::parse_u8(v).ok_or_else(|| anyhow::anyhow!("bad {what} `{v}`"))
            };
            match tok.to_ascii_lowercase().as_str() {
                "left" => direction = Direction::Left,
                "right" => direction = Direction::Right,
                "speed" => speed = Some(num("speed")?),
                "duration" => duration = Some(num("duration")?),
                c => colors.push(parse_color(c).ok_or_else(|| anyhow::anyhow!("unknown effect option `{tok}`"))?),
            }
        }
        let one = |colors: &[Rgb]| match colors {
            [c] => Ok(*c),
            _ => Err(anyhow::anyhow!("{name} takes exactly one colour")),
        };
        let duration = duration.unwrap_or(DEFAULT_DURATION);
        let speed = speed.unwrap_or(DEFAULT_WAVE_SPEED);
        let e = match name.as_str() {
            "off" | "none" => HwEffect::Off,
            "static" => HwEffect::Static { color: one(&colors)? },
            "breathing" | "breath" => HwEffect::Breathing { colors },
            "spectrum" => HwEffect::Spectrum,
            "wave" => HwEffect::Wave { direction, speed },
            "wheel" => HwEffect::Wheel { direction, speed },
            "reactive" => HwEffect::Reactive { color: one(&colors)?, duration },
            "starlight" => HwEffect::Starlight { colors, duration },
            other => bail!("unknown effect `{other}` (off static breathing spectrum wave wheel reactive starlight)"),
        };
        e.report(0, Storage::Session, 0)?; // validates colour counts
        Ok(e)
    }
}

impl std::fmt::Display for HwEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cs = |c: &[Rgb]| c.iter().map(|c| format!(" {}", c.to_hex())).collect::<String>();
        let dir = |d: &Direction| match d {
            Direction::Left => "left",
            Direction::Right => "right",
        };
        match self {
            HwEffect::Off | HwEffect::Spectrum => write!(f, "{}", self.name()),
            HwEffect::Static { color } => write!(f, "static {}", color.to_hex()),
            HwEffect::Breathing { colors } => write!(f, "breathing{}", cs(colors)),
            HwEffect::Wave { direction, speed } | HwEffect::Wheel { direction, speed } => {
                write!(f, "{} {} speed {speed}", self.name(), dir(direction))
            }
            HwEffect::Reactive { color, duration } => write!(f, "reactive {} duration {duration}", color.to_hex()),
            HwEffect::Starlight { colors, duration } => write!(f, "starlight{} duration {duration}", cs(colors)),
        }
    }
}

impl Serialize for HwEffect {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for HwEffect {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        HwEffect::parse_spec(&s).map_err(serde::de::Error::custom)
    }
}

impl HwEffect {
    pub fn id(&self) -> u8 {
        match self {
            HwEffect::Off => EFFECT_OFF,
            HwEffect::Static { .. } => EFFECT_STATIC,
            HwEffect::Breathing { .. } => EFFECT_BREATHING,
            HwEffect::Spectrum => EFFECT_SPECTRUM,
            HwEffect::Wave { .. } => EFFECT_WAVE,
            HwEffect::Wheel { .. } => EFFECT_WHEEL,
            HwEffect::Reactive { .. } => EFFECT_REACTIVE,
            HwEffect::Starlight { .. } => EFFECT_STARLIGHT,
        }
    }

    /// Name as used in device TOMLs (`[hw_effects] effects = [...]`) and the CLI.
    pub fn name(&self) -> &'static str {
        effect_name(self.id())
    }

    /// Build the `0F/02` report.
    pub fn report(&self, tid: u8, storage: Storage, led: u8) -> Result<Report> {
        let (flags, rate, colors): (u8, u8, &[Rgb]) = match self {
            HwEffect::Off | HwEffect::Spectrum => (0, 0, &[]),
            HwEffect::Static { color } => (0, 0, std::slice::from_ref(color)),
            HwEffect::Breathing { colors } => {
                if colors.len() > 2 {
                    bail!("breathing takes 0 (random), 1 or 2 colours");
                }
                (colors.len() as u8, 0, colors)
            }
            HwEffect::Wave { direction, speed } | HwEffect::Wheel { direction, speed } => {
                (direction.byte(), (*speed).max(1), &[])
            }
            HwEffect::Reactive { color, duration } => (0, (*duration).clamp(1, 4), std::slice::from_ref(color)),
            HwEffect::Starlight { colors, duration } => {
                if colors.len() > 2 {
                    bail!("starlight takes 0 (random), 1 or 2 colours");
                }
                (colors.len() as u8, (*duration).clamp(1, 3), colors)
            }
        };
        let mut args = vec![storage as u8, led, self.id(), flags, rate, colors.len() as u8];
        for c in colors {
            args.extend_from_slice(&c.bytes());
        }
        Ok(Report::new(tid, CLASS, SET_EFFECT, &args))
    }
}

pub fn effect_name(id: u8) -> &'static str {
    match id {
        EFFECT_OFF => "off",
        EFFECT_STATIC => "static",
        EFFECT_BREATHING => "breathing",
        EFFECT_SPECTRUM => "spectrum",
        EFFECT_WAVE => "wave",
        EFFECT_REACTIVE => "reactive",
        EFFECT_RIPPLE => "ripple",
        EFFECT_STARLIGHT => "starlight",
        EFFECT_CUSTOM_FRAME => "custom",
        EFFECT_FIRE => "fire",
        EFFECT_WHEEL => "wheel",
        _ => "unknown",
    }
}

/// `0F/84 [storage, led]` → `[storage, led, brightness]` (0..255). Protocol note; uncoil scales colours
/// itself instead.
#[cfg(test)]
fn get_brightness(tid: u8, storage: Storage, led: u8) -> Report {
    Report::new(tid, CLASS, GET_BRIGHTNESS, &[storage as u8, led, 0])
}

/// `0F/04 [storage, led, brightness]`. Protocol note, like [`get_brightness`].
#[cfg(test)]
fn set_brightness(tid: u8, storage: Storage, led: u8, brightness: u8) -> Report {
    Report::new(tid, CLASS, SET_BRIGHTNESS, &[storage as u8, led, brightness])
}

/// One lighting region as listed by `0F/80`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub led: u8,
    pub rows: u8,
    pub cols: u8,
}

pub fn get_regions(tid: u8) -> Report {
    Report::sized(tid, 0x50, CLASS, GET_REGIONS, &[])
}

/// `0F/80` reply: 5-byte records `[led, ?, ?, rows, cols]`, terminated by led 0.
pub fn parse_regions(reply: &Reply) -> Vec<Region> {
    let a = reply.args();
    (0..a.len() / 5)
        .map(|i| &a[i * 5..i * 5 + 5])
        .take_while(|r| r[0] != 0)
        .map(|r| Region { led: r[0], rows: r[3], cols: r[4] })
        .collect()
}

pub fn get_supported_effects(tid: u8, led: u8) -> Report {
    Report::sized(tid, 0x50, CLASS, GET_SUPPORTED_EFFECTS, &[led])
}

/// `0F/81` reply: `[led, effect ids…]`.
pub fn parse_supported_effects(reply: &Reply) -> Vec<u8> {
    reply.args().iter().skip(1).copied().collect()
}

/// Parse "#rrggbb", "rrggbb" or "r,g,b".
pub fn parse_color(s: &str) -> Option<Rgb> {
    let s = s.trim();
    if let Some((r, rest)) = s.split_once(',') {
        let (g, b) = rest.split_once(',')?;
        return Some(Rgb(r.trim().parse().ok()?, g.trim().parse().ok()?, b.trim().parse().ok()?));
    }
    let h = s.strip_prefix('#').unwrap_or(s);
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Status, MAX_ARGS};

    fn bytes(e: &HwEffect, storage: Storage, led: u8, tid: u8) -> Vec<u8> {
        let w = e.report(tid, storage, led).unwrap().to_wire();
        let size = w[6] as usize;
        w[1..9 + size].to_vec()
    }

    // Expected bytes are OpenRazer's own captures (razerchromacommon.c comments):
    // "Status Trans Packet Proto DataSize Class CMD Args".
    #[test]
    fn static_matches_openrazer_capture() {
        // 00 3f 0000 00 09 0f 02 01 05 01 00 00 01 ff 00 00
        let e = HwEffect::Static { color: Rgb(0xFF, 0, 0) };
        assert_eq!(
            bytes(&e, Storage::Onboard, 0x05, 0x3F),
            [0, 0x3F, 0, 0, 0, 0x09, 0x0F, 0x02, 0x01, 0x05, 0x01, 0, 0, 0x01, 0xFF, 0, 0]
        );
    }

    #[test]
    fn wave_matches_openrazer_capture() {
        // 00 3f 0000 00 06 0f 02 01 05 04 01 28 00
        let e = HwEffect::Wave { direction: Direction::Right, speed: 0x28 };
        assert_eq!(bytes(&e, Storage::Onboard, 0x05, 0x3F), [0, 0x3F, 0, 0, 0, 6, 0x0F, 2, 1, 5, 4, 1, 0x28, 0]);
        // and the existing proto::effect_wave helper (session storage, led 0) agrees
        let old = crate::proto::effect_wave(0x1F, 1, 0x28).to_wire();
        let new =
            HwEffect::Wave { direction: Direction::Right, speed: 0x28 }.report(0x1F, Storage::Session, 0).unwrap();
        assert_eq!(old, new.to_wire());
    }

    #[test]
    fn wheel_matches_openrazer_capture() {
        // 00 1f 0000 00 06 0f 02 01 05 0a 02 28 00 (BlackWidow V4 Pro)
        let e = HwEffect::Wheel { direction: Direction::Left, speed: 0x28 };
        assert_eq!(bytes(&e, Storage::Onboard, 0x05, 0x1F), [0, 0x1F, 0, 0, 0, 6, 0x0F, 2, 1, 5, 0x0A, 2, 0x28, 0]);
    }

    #[test]
    fn spectrum_and_off_match_openrazer_capture() {
        // 00 3f 0000 00 06 0f 02 01 05 03 00 00 00
        assert_eq!(
            bytes(&HwEffect::Spectrum, Storage::Onboard, 5, 0x3F),
            [0, 0x3F, 0, 0, 0, 6, 0x0F, 2, 1, 5, 3, 0, 0, 0]
        );
        // 00 3f 0000 00 06 0f 02 01 05 00 00 00 00
        assert_eq!(bytes(&HwEffect::Off, Storage::Onboard, 5, 0x3F), [0, 0x3F, 0, 0, 0, 6, 0x0F, 2, 1, 5, 0, 0, 0, 0]);
    }

    #[test]
    fn breathing_variants_match_openrazer_capture() {
        // single: 01 05 02 01 00 01 00 ff 00 (size 9)
        let one = HwEffect::Breathing { colors: vec![Rgb(0, 0xFF, 0)] };
        assert_eq!(&bytes(&one, Storage::Onboard, 5, 0x3F)[5..], [9, 0x0F, 2, 1, 5, 2, 1, 0, 1, 0, 0xFF, 0]);
        // dual: 01 05 02 02 00 02 00 ff 00 ff 00 00 (size 12)
        let two = HwEffect::Breathing { colors: vec![Rgb(0, 0xFF, 0), Rgb(0xFF, 0, 0)] };
        assert_eq!(
            &bytes(&two, Storage::Onboard, 5, 0x3F)[5..],
            [12, 0x0F, 2, 1, 5, 2, 2, 0, 2, 0, 0xFF, 0, 0xFF, 0, 0]
        );
        // random: 01 05 02 00 00 00 (size 6)
        let rnd = HwEffect::Breathing { colors: vec![] };
        assert_eq!(&bytes(&rnd, Storage::Onboard, 5, 0x3F)[5..], [6, 0x0F, 2, 1, 5, 2, 0, 0, 0]);
        assert!(HwEffect::Breathing { colors: vec![Rgb::BLACK; 3] }.report(0x1F, Storage::Session, 5).is_err());
    }

    #[test]
    fn reactive_matches_openrazer_capture() {
        // 01 05 05 00 03 01 ff 00 00 (speed 3)
        let e = HwEffect::Reactive { color: Rgb(0xFF, 0, 0), duration: 3 };
        assert_eq!(&bytes(&e, Storage::Onboard, 5, 0x3F)[5..], [9, 0x0F, 2, 1, 5, 5, 0, 3, 1, 0xFF, 0, 0]);
    }

    #[test]
    fn starlight_follows_openrgb_flags() {
        // OpenRGB: flags = colour count; OpenRazer's capture for random: 01 05 07 00 01 00
        let rnd = HwEffect::Starlight { colors: vec![], duration: 1 };
        assert_eq!(&bytes(&rnd, Storage::Onboard, 5, 0x3F)[5..], [6, 0x0F, 2, 1, 5, 7, 0, 1, 0]);
        let two = HwEffect::Starlight { colors: vec![Rgb(0xFF, 0, 0), Rgb(0, 0xFF, 0)], duration: 9 };
        assert_eq!(
            &bytes(&two, Storage::Onboard, 5, 0x3F)[5..],
            [12, 0x0F, 2, 1, 5, 7, 2, 3, 2, 0xFF, 0, 0, 0, 0xFF, 0]
        );
    }

    #[test]
    fn brightness_matches_openrazer_capture() {
        // 00 3f 0000 00 03 0f 04 01 04 b7  (set)  /  03 0f 84 01 04 (get)
        let w = set_brightness(0x3F, Storage::Onboard, 0x04, 0xB7).to_wire();
        assert_eq!(&w[1..12], &[0, 0x3F, 0, 0, 0, 3, 0x0F, 0x04, 1, 4, 0xB7]);
        assert!(get_brightness(0x1F, Storage::Onboard, 5).is_read_only());
    }

    fn reply(args: &[u8]) -> Reply {
        let mut raw = [0u8; MAX_ARGS];
        raw[..args.len()].copy_from_slice(args);
        Reply { status: Status::Ok, transaction_id: 0x1F, size: args.len() as u8, class: 0x0F, id: 0x80, raw }
    }

    #[test]
    fn regions_and_effects_parse_real_replies() {
        // Read on the BlackWidow V4 Pro 75% (2026-10-03): one 6x18 backlight region, effects 0..9
        let kb = reply(&[5, 25, 3, 6, 18]);
        assert_eq!(parse_regions(&kb), vec![Region { led: 5, rows: 6, cols: 18 }]);
        let fx = reply(&[5, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(parse_supported_effects(&fx), (0..=9).collect::<Vec<u8>>());
        // Basilisk V3 Pro: wheel, logo, 11-LED underglow
        let mouse = reply(&[1, 25, 3, 1, 1, 4, 25, 3, 1, 1, 10, 25, 3, 1, 11]);
        let r = parse_regions(&mouse);
        assert_eq!(r.len(), 3);
        assert_eq!(r[2], Region { led: 10, rows: 1, cols: 11 });
    }

    #[test]
    fn spec_strings() {
        let e: HwEffect = serde_json::from_str(r#""wave left""#).unwrap();
        assert_eq!(e, HwEffect::Wave { direction: Direction::Left, speed: DEFAULT_WAVE_SPEED });
        assert_eq!(serde_json::to_string(&e).unwrap(), r#""wave left speed 40""#);
        let s: HwEffect = serde_json::from_str(r#""static #010203""#).unwrap();
        assert_eq!(s, HwEffect::Static { color: Rgb(1, 2, 3) });
        for spec in [
            "off",
            "spectrum",
            "static #ff0000",
            "breathing",
            "breathing #ff0000 #00ff00",
            "wave right speed 16",
            "wheel left speed 40",
            "reactive #ffffff duration 3",
            "starlight duration 1",
            "starlight #ff0000 #0000ff duration 2",
        ] {
            let e = HwEffect::parse_spec(spec).unwrap();
            assert_eq!(e.to_string(), spec);
            assert_eq!(HwEffect::parse_spec(&e.to_string()).unwrap(), e);
        }
        assert!(HwEffect::parse_spec("static").is_err());
        assert!(HwEffect::parse_spec("breathing #000000 #000000 #000000").is_err());
        assert!(HwEffect::parse_spec("disco").is_err());
        assert!(HwEffect::parse_spec("wave sideways").is_err());
    }

    #[test]
    fn colors_parse() {
        assert_eq!(parse_color("#ff8000"), Some(Rgb(255, 128, 0)));
        assert_eq!(parse_color("10, 20,30"), Some(Rgb(10, 20, 30)));
        assert_eq!(parse_color("nope"), None);
    }
}
