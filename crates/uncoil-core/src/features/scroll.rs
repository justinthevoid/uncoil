//! Mouse scroll wheel: tactile or free-spin, scroll acceleration and Smart Reel (switches to free spin when
//! the wheel is flicked fast). Byte layouts are OpenRazer's (`razerchromacommon.c` and `razermouse_driver.c`
//! at a84cd0ae, transcribed as facts; meanings from its `mouse_scroll_wheel.py`). **Not yet read on uncoil's
//! own devices**; the `scroll` read-only check gates every write until one has.
//!
//! | class/id | name | args → reply |
//! |---|---|---|
//! | `02/14` / `02/94` | scroll mode | `[VARSTORE, 0 tactile / 1 free spin]`; the get sends `[VARSTORE]` |
//! | `02/16` / `02/96` | scroll acceleration | `[VARSTORE, 0 off / 1 on]` |
//! | `02/17` / `02/97` | Smart Reel | `[VARSTORE, 0 off / 1 on]` |
//!
//! Every command has size 2 and the reply carries the value in argument 1. VARSTORE: the value is stored in
//! the mouse, so every set is an onboard write.

use crate::features::performance::VARSTORE;
use crate::proto::{Reply, Report};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const CLASS: u8 = 0x02;
const SIZE: u8 = 0x02;

/// One of the three wheel settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Mode,
    Acceleration,
    SmartReel,
}

impl Setting {
    pub const ALL: [Setting; 3] = [Setting::Mode, Setting::Acceleration, Setting::SmartReel];

    fn set_id(self) -> u8 {
        match self {
            Setting::Mode => 0x14,
            Setting::Acceleration => 0x16,
            Setting::SmartReel => 0x17,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Setting::Mode => "scroll mode",
            Setting::Acceleration => "scroll acceleration",
            Setting::SmartReel => "Smart Reel",
        }
    }
}

/// Tactile (notched) or free spin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollMode {
    Tactile,
    FreeSpin,
}

impl ScrollMode {
    pub fn byte(self) -> u8 {
        match self {
            ScrollMode::Tactile => 0,
            ScrollMode::FreeSpin => 1,
        }
    }

    pub fn from_byte(b: u8) -> Option<ScrollMode> {
        match b {
            0 => Some(ScrollMode::Tactile),
            1 => Some(ScrollMode::FreeSpin),
            _ => None,
        }
    }

    /// `tactile`, `free-spin` (also `free_spin`, `freespin`).
    pub fn parse(s: &str) -> Option<ScrollMode> {
        match s.to_ascii_lowercase().as_str() {
            "tactile" | "notched" => Some(ScrollMode::Tactile),
            "free-spin" | "free_spin" | "freespin" | "free" => Some(ScrollMode::FreeSpin),
            _ => None,
        }
    }

    /// Plain words: `tactile`, `free spin`.
    pub fn words(self) -> &'static str {
        match self {
            ScrollMode::Tactile => "tactile",
            ScrollMode::FreeSpin => "free spin",
        }
    }
}

/// The get report for `s`.
pub fn get(tid: u8, s: Setting) -> Report {
    Report::sized(tid, SIZE, CLASS, s.set_id() | 0x80, &[VARSTORE])
}

/// The set report for `s` (stored in the mouse).
pub fn set(tid: u8, s: Setting, value: u8) -> Report {
    Report::new(tid, CLASS, s.set_id(), &[VARSTORE, value])
}

/// The value in argument 1: 0 or 1.
pub fn parse(reply: &Reply, s: Setting) -> Result<u8> {
    match reply.raw[1] {
        v @ (0 | 1) => Ok(v),
        v => bail!("{} reads {v}, expected 0 or 1", s.name()),
    }
}

/// `scroll.get` / `scroll.set` result. `None` where the device lacks the setting or did not answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrollState {
    pub mode: Option<ScrollMode>,
    pub acceleration: Option<bool>,
    pub smart_reel: Option<bool>,
}

impl ScrollState {
    /// The value of `s` as the byte the device uses.
    pub fn byte(&self, s: Setting) -> Option<u8> {
        match s {
            Setting::Mode => self.mode.map(ScrollMode::byte),
            Setting::Acceleration => self.acceleration.map(u8::from),
            Setting::SmartReel => self.smart_reel.map(u8::from),
        }
    }

    /// Record `value` (as the device sends it) for `s`.
    pub fn apply(&mut self, s: Setting, value: u8) {
        match s {
            Setting::Mode => self.mode = ScrollMode::from_byte(value),
            Setting::Acceleration => self.acceleration = Some(value == 1),
            Setting::SmartReel => self.smart_reel = Some(value == 1),
        }
    }

    /// The value of `s` in plain words (`free spin`, `on`, `?`).
    pub fn words(&self, s: Setting) -> &'static str {
        let on_off = |v: Option<bool>| match v {
            Some(true) => "on",
            Some(false) => "off",
            None => "?",
        };
        match s {
            Setting::Mode => self.mode.map_or("?", ScrollMode::words),
            Setting::Acceleration => on_off(self.acceleration),
            Setting::SmartReel => on_off(self.smart_reel),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Status, MAX_ARGS};

    fn reply(id: u8, args: &[u8]) -> Reply {
        let mut raw = [0u8; MAX_ARGS];
        raw[..args.len()].copy_from_slice(args);
        Reply { status: Status::Ok, transaction_id: 0x1F, size: 2, class: CLASS, id, raw }
    }

    #[test]
    fn reports_match_openrazer() {
        // razer_chroma_misc_set_scroll_mode / get_scroll_mode: class 02, size 2, [VARSTORE, mode]
        assert_eq!(&set(0x1F, Setting::Mode, 1).to_wire()[1..11], &[0, 0x1F, 0, 0, 0, 2, 0x02, 0x14, 1, 1]);
        assert_eq!(&get(0x1F, Setting::Mode).to_wire()[1..11], &[0, 0x1F, 0, 0, 0, 2, 0x02, 0x94, 1, 0]);
        assert_eq!(&set(0x1F, Setting::Acceleration, 0).to_wire()[7..11], &[0x02, 0x16, 1, 0]);
        assert_eq!(&get(0x1F, Setting::Acceleration).to_wire()[7..9], &[0x02, 0x96]);
        assert_eq!(&set(0x1F, Setting::SmartReel, 1).to_wire()[7..11], &[0x02, 0x17, 1, 1]);
        assert_eq!(&get(0x1F, Setting::SmartReel).to_wire()[7..9], &[0x02, 0x97]);
        for s in Setting::ALL {
            assert!(get(1, s).is_read_only());
            assert!(!set(1, s, 0).is_read_only());
        }
    }

    #[test]
    fn parsing_and_state() {
        assert_eq!(parse(&reply(0x94, &[1, 1]), Setting::Mode).unwrap(), 1);
        assert_eq!(parse(&reply(0x96, &[1, 0]), Setting::Acceleration).unwrap(), 0);
        let e = parse(&reply(0x97, &[1, 7]), Setting::SmartReel).unwrap_err().to_string();
        assert_eq!(e, "Smart Reel reads 7, expected 0 or 1");
        let mut s = ScrollState::default();
        s.apply(Setting::Mode, 1);
        s.apply(Setting::Acceleration, 1);
        assert_eq!(s, ScrollState { mode: Some(ScrollMode::FreeSpin), acceleration: Some(true), smart_reel: None });
        assert_eq!((s.byte(Setting::Mode), s.byte(Setting::SmartReel)), (Some(1), None));
        assert_eq!(
            (s.words(Setting::Mode), s.words(Setting::Acceleration), s.words(Setting::SmartReel)),
            ("free spin", "on", "?")
        );
        assert_eq!(serde_json::to_string(&s).unwrap(), r#"{"mode":"free_spin","acceleration":true,"smart_reel":null}"#);
        assert_eq!(ScrollMode::parse("Free-Spin"), Some(ScrollMode::FreeSpin));
        assert_eq!(ScrollMode::parse("tactile"), Some(ScrollMode::Tactile));
        assert_eq!(ScrollMode::parse("wobbly"), None);
    }
}
