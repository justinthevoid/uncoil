//! OLED display settings of the BlackWidow V4 Pro 75% (class `0x17`). Names, ids and reply meanings come
//! from Synapse's logs (`rzDevice25.getOLED…`, current firmware, 2026); values read back from the user's
//! keyboard on 2026-10-03 matched (brightness 100, time to dim 1 min, home screen animation 1).
//!
//! An older protocol stack in the logs (`rzDevice30`, 2025 / May 2026) used different ids for the same
//! getters (time to dim `17/85`, language `17/89`, …); uncoil uses the current table below.
//!
//! Only one setter has been observed: `17/03 Set OLED Display brightness [percent]`. The other setters very
//! likely mirror the getters without the high bit, but uncoil does not send unobserved writes.
//! Image upload (`17/8F` lists 20-byte image GUIDs per slot) is not understood yet.

use crate::proto::{Reply, Report};
use serde::{Deserialize, Serialize};

pub const CLASS: u8 = 0x17;
pub const SET_BRIGHTNESS: u8 = 0x03;

/// (command id, data size) of every getter uncoil reads.
pub const HOME_SCREEN: (u8, u8) = (0x82, 7);
pub const BRIGHTNESS: (u8, u8) = (0x83, 1);
pub const TIME_TO_HOME: (u8, u8) = (0x84, 1);
pub const LANGUAGE: (u8, u8) = (0x85, 1);
pub const TIME_TO_DIM: (u8, u8) = (0x86, 1);
pub const ACTIVE_ITEM: (u8, u8) = (0x8D, 2);
pub const IMAGE_GUID: (u8, u8) = (0x8F, 22);
pub const LOW_BATTERY_PERCENT: (u8, u8) = (0x92, 1);
pub const LOW_POWER_MODE: (u8, u8) = (0x93, 1);
pub const ANIMATION_STATE: (u8, u8) = (0x94, 6);
pub const IMAGE_STATE: (u8, u8) = (0x95, 10);
pub const SCREENSAVER: (u8, u8) = (0x96, 2);

pub fn get(tid: u8, which: (u8, u8)) -> Report {
    Report::sized(tid, which.1, CLASS, which.0, &[])
}

/// The getters [`OledState`] is built from, in order.
pub const STATE_GETTERS: [(u8, u8); 10] = [
    HOME_SCREEN,
    BRIGHTNESS,
    TIME_TO_HOME,
    LANGUAGE,
    TIME_TO_DIM,
    ACTIVE_ITEM,
    LOW_BATTERY_PERCENT,
    LOW_POWER_MODE,
    ANIMATION_STATE,
    SCREENSAVER,
];

/// `17/03 [percent]` — logged by Synapse for every step of its brightness slider.
pub fn set_brightness(tid: u8, percent: u8) -> Report {
    Report::new(tid, CLASS, SET_BRIGHTNESS, &[percent.min(100)])
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OledState {
    /// Display brightness, percent.
    pub brightness: Option<u8>,
    /// "ANIMATION" (built-in animation `index`) or "IMAGE".
    pub home_screen: Option<String>,
    pub home_screen_index: Option<u8>,
    /// Raw `17/84` value; 1 = 5 seconds (the only value seen).
    pub time_to_home: Option<u8>,
    /// Minutes before the display dims.
    pub time_to_dim_minutes: Option<u8>,
    /// 0 = English.
    pub language: Option<u8>,
    /// What the home screen shows: ANIMATION, KEYBOARD_INFO, OFF, or the raw id.
    pub active_item: Option<String>,
    pub low_battery_warning_percent: Option<u8>,
    pub low_power_mode: Option<bool>,
    /// Which of the 6 built-in animations are enabled.
    pub animations_enabled: Option<Vec<bool>>,
    /// Screensaver `[mode, item]`.
    pub screensaver: Option<[u8; 2]>,
}

pub fn active_item_name(id: u8) -> String {
    match id {
        0 => "ANIMATION".into(),
        3 => "KEYBOARD_INFO".into(),
        7 => "OFF".into(),
        x => format!("ITEM_{x}"),
    }
}

impl OledState {
    /// Fold one getter reply into the state.
    pub fn apply(&mut self, reply: &Reply) {
        let a = &reply.raw;
        match reply.id {
            0x82 => {
                self.home_screen = Some(match a[0] {
                    0 => "ANIMATION".into(),
                    1 => "IMAGE".into(),
                    x => format!("TYPE_{x}"),
                });
                self.home_screen_index = Some(a[1]);
            }
            0x83 => self.brightness = Some(a[0]),
            0x84 => self.time_to_home = Some(a[0]),
            0x85 => self.language = Some(a[0]),
            0x86 => self.time_to_dim_minutes = Some(a[0]),
            0x8D => self.active_item = Some(active_item_name(a[0])),
            0x92 => self.low_battery_warning_percent = Some(a[0]),
            0x93 => self.low_power_mode = Some(a[0] != 0),
            0x94 => self.animations_enabled = Some(a[..6].iter().map(|&b| b != 0).collect()),
            0x96 => self.screensaver = Some([a[0], a[1]]),
            _ => {}
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
        Reply { status: Status::Ok, transaction_id: 0x1F, size: args.len() as u8, class: CLASS, id, raw }
    }

    #[test]
    fn brightness_matches_synapse_log() {
        // "Set OLED Display brightness" {"brightness":46}: dataSend [0,x,0,0,0,1,23,3,46]
        let w = set_brightness(0x1F, 46).to_wire();
        assert_eq!(&w[1..10], &[0, 0x1F, 0, 0, 0, 1, 23, 3, 46]);
        assert_eq!(set_brightness(0x1F, 250).args, vec![100]);
    }

    #[test]
    fn getters_are_read_only_and_sized_like_synapse() {
        for g in STATE_GETTERS {
            let r = get(0x1F, g);
            assert!(r.is_read_only());
            assert_eq!(r.class, 0x17);
        }
        // "Get OLED Display home screen": dataSend [..,7,23,130,0,0,0,0,0,0,0]
        assert_eq!(&get(5, HOME_SCREEN).to_wire()[6..9], &[7, 23, 130]);
        assert_eq!(&get(5, IMAGE_GUID).to_wire()[6..9], &[22, 23, 143]);
    }

    #[test]
    fn state_from_logged_and_hardware_replies() {
        let mut s = OledState::default();
        s.apply(&reply(0x82, &[0, 1, 0, 0, 0, 0, 0])); // {"type":0,"typeEnum":"ANIMATION","data":{"gifIndex":1}}
        s.apply(&reply(0x83, &[100]));
        s.apply(&reply(0x84, &[1])); // TIME_5S
        s.apply(&reply(0x86, &[1])); // "1 minute"
        s.apply(&reply(0x8D, &[3, 3])); // KEYBOARD_INFO
        s.apply(&reply(0x92, &[20]));
        s.apply(&reply(0x93, &[0])); // INACTIVE
        s.apply(&reply(0x94, &[1, 1, 1, 1, 1, 1]));
        s.apply(&reply(0x96, &[0, 2])); // {"mode":0,"item":2}
        assert_eq!(s.home_screen.as_deref(), Some("ANIMATION"));
        assert_eq!(s.home_screen_index, Some(1));
        assert_eq!(s.brightness, Some(100));
        assert_eq!(s.time_to_dim_minutes, Some(1));
        assert_eq!(s.active_item.as_deref(), Some("KEYBOARD_INFO"));
        assert_eq!(s.low_power_mode, Some(false));
        assert_eq!(s.animations_enabled.as_ref().map(|v| v.len()), Some(6));
        assert_eq!(s.screensaver, Some([0, 2]));
    }
}
