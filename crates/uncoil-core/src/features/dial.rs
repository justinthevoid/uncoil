//! The BlackWidow V4 Pro 75%'s OLED command dial.
//!
//! `17/00 Set OLED Command dial active mode [profile, mode, display_order, enabled_functions]` is logged by
//! Synapse with names and bytes (`{"profileId":1,"mode":0,"modeEnum":"VOLUME","displayOrder":1,
//! "totalEnabledFunctions":6}` ↔ `[1, 0, 1, 6]`). `display_order` is the 1-based position of the mode among
//! the enabled modes (in mode-id order) and `enabled_functions` how many are enabled; the OLED shows them as
//! "n / total". `17/80 [profile]` reads it back (hardware read 2026-10-03: `[1, 0, 0, 0]`, i.e. VOLUME on
//! profile 1; the firmware did not keep the order/total bytes).
//!
//! Mode names, ids and the action Synapse binds to each dial input come from the `commandDial.modes` objects
//! in Synapse's logs. In driver mode those actions are performed by Synapse on the host; with the keyboard in
//! normal mode (as uncoil leaves it) the dial sends consumer volume codes and the firmware handles the press.
//! Whether the firmware itself performs the other modes in normal mode is **not verified**.

use crate::proto::{Reply, Report};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

pub const CLASS: u8 = 0x17;
pub const SET_ACTIVE_MODE: u8 = 0x00;
pub const GET_ACTIVE_MODE: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DialMode {
    Volume = 0,
    TrackSelector = 1,
    OledBrightness = 2,
    /// Synapse spells it LIGHTNING_BRIGHTNESS; it is the keyboard backlight.
    #[serde(rename = "LIGHTNING_BRIGHTNESS", alias = "LIGHTING_BRIGHTNESS")]
    LightingBrightness = 3,
    SwitchApps = 4,
    Zoom = 5,
    TrackJogging = 6,
    ScrollVertical = 7,
    ScrollHorizontal = 8,
}

impl DialMode {
    pub const ALL: [DialMode; 9] = [
        DialMode::Volume,
        DialMode::TrackSelector,
        DialMode::OledBrightness,
        DialMode::LightingBrightness,
        DialMode::SwitchApps,
        DialMode::Zoom,
        DialMode::TrackJogging,
        DialMode::ScrollVertical,
        DialMode::ScrollHorizontal,
    ];

    /// Synapse's default set of enabled modes (the other three start disabled).
    pub const DEFAULT_ENABLED: [DialMode; 6] = [
        DialMode::Volume,
        DialMode::TrackSelector,
        DialMode::OledBrightness,
        DialMode::LightingBrightness,
        DialMode::SwitchApps,
        DialMode::Zoom,
    ];

    pub fn from_id(id: u8) -> Option<DialMode> {
        DialMode::ALL.get(id as usize).copied()
    }

    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            DialMode::Volume => "VOLUME",
            DialMode::TrackSelector => "TRACK_SELECTOR",
            DialMode::OledBrightness => "OLED_BRIGHTNESS",
            DialMode::LightingBrightness => "LIGHTNING_BRIGHTNESS",
            DialMode::SwitchApps => "SWITCH_APPS",
            DialMode::Zoom => "ZOOM",
            DialMode::TrackJogging => "TRACK_JOGGING",
            DialMode::ScrollVertical => "SCROLL_VERTICAL",
            DialMode::ScrollHorizontal => "SCROLL_HORIZONTAL",
        }
    }

    /// Accepts "volume", "TRACK_SELECTOR", "track-selector", "3", …
    pub fn parse(s: &str) -> Option<DialMode> {
        if let Ok(id) = s.parse::<u8>() {
            return DialMode::from_id(id);
        }
        let n: String = s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase();
        if n == "LIGHTINGBRIGHTNESS" || n == "BACKLIGHT" {
            return Some(DialMode::LightingBrightness);
        }
        DialMode::ALL.into_iter().find(|m| m.name().replace('_', "") == n)
    }

    /// What Synapse binds to (turn clockwise, press, turn counter-clockwise) in this mode.
    pub fn synapse_actions(self) -> [&'static str; 3] {
        match self {
            DialMode::Volume => ["VolumeUp", "MuteVolume", "VolumeDown"],
            DialMode::TrackSelector => ["NextTrack", "Play", "PrevTrack"],
            DialMode::OledBrightness => ["OLED BrightnessUp", "OLED BrightnessToggle", "OLED BrightnessDown"],
            DialMode::LightingBrightness => ["BrightnessUp", "BrightnessToggle", "BrightnessDown"],
            DialMode::SwitchApps => ["SwitchAppRight", "CycleApps", "SwitchAppLeft"],
            DialMode::Zoom => ["OfficeZoomIn", "OfficeZoomReset", "OfficeZoomOut"],
            DialMode::TrackJogging => ["TrackJoggingForward", "Play", "TrackJoggingBackward"],
            DialMode::ScrollVertical => ["ScrollUp", "(disabled)", "ScrollDown"],
            DialMode::ScrollHorizontal => ["ScrollRight", "(disabled)", "ScrollLeft"],
        }
    }
}

/// (display_order, enabled_functions) for `mode` among `enabled`, as Synapse computes them.
pub fn display_order(mode: DialMode, enabled: &[DialMode]) -> Result<(u8, u8)> {
    let mut on: Vec<DialMode> = enabled.to_vec();
    on.sort();
    on.dedup();
    let pos =
        on.iter().position(|&m| m == mode).ok_or_else(|| anyhow!("{} is not among the enabled modes", mode.name()))?;
    Ok((pos as u8 + 1, on.len() as u8))
}

pub fn set_active_mode(tid: u8, profile: u8, mode: DialMode, enabled: &[DialMode]) -> Result<Report> {
    let (order, total) = display_order(mode, enabled)?;
    Ok(Report::new(tid, CLASS, SET_ACTIVE_MODE, &[profile, mode.id(), order, total]))
}

pub fn get_active_mode(tid: u8, profile: u8) -> Report {
    Report::sized(tid, 0x04, CLASS, GET_ACTIVE_MODE, &[profile])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialState {
    pub profile: u8,
    pub mode_id: u8,
    /// None when the firmware reports an id uncoil does not know.
    pub mode: Option<DialMode>,
    pub display_order: u8,
    pub enabled_functions: u8,
}

pub fn parse_state(reply: &Reply) -> DialState {
    let a = &reply.raw;
    DialState {
        profile: a[0],
        mode_id: a[1],
        mode: DialMode::from_id(a[1]),
        display_order: a[2],
        enabled_functions: a[3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Status, MAX_ARGS};

    #[test]
    fn set_matches_synapse_log() {
        // {"profileId":1,"mode":0,"modeEnum":"VOLUME","displayOrder":1,"totalEnabledFunctions":6}
        let r = set_active_mode(6, 1, DialMode::Volume, &DialMode::DEFAULT_ENABLED).unwrap();
        assert_eq!(&r.to_wire()[1..13], &[0, 6, 0, 0, 0, 4, 23, 0, 1, 0, 1, 6]);
        // {"mode":1,"modeEnum":"TRACK_SELECTOR","displayOrder":2,"totalEnabledFunctions":6}
        let r = set_active_mode(0x1F, 1, DialMode::TrackSelector, &DialMode::DEFAULT_ENABLED).unwrap();
        assert_eq!(r.args, vec![1, 1, 2, 6]);
        // {"mode":2,"modeEnum":"OLED_BRIGHTNESS","displayOrder":3,"totalEnabledFunctions":7} (SCROLL_VERTICAL on)
        let mut seven = DialMode::DEFAULT_ENABLED.to_vec();
        seven.push(DialMode::ScrollVertical);
        assert_eq!(set_active_mode(0x1F, 1, DialMode::OledBrightness, &seven).unwrap().args, vec![1, 2, 3, 7]);
        // and the older proto helper agrees
        assert_eq!(
            crate::proto::set_command_dial_mode(6, 1, 0, 1, 6),
            set_active_mode(6, 1, DialMode::Volume, &DialMode::DEFAULT_ENABLED).unwrap()
        );
    }

    #[test]
    fn disabled_mode_is_rejected() {
        assert!(set_active_mode(0x1F, 1, DialMode::Zoom, &[DialMode::Volume]).is_err());
    }

    #[test]
    fn names() {
        assert_eq!(DialMode::parse("track-selector"), Some(DialMode::TrackSelector));
        assert_eq!(DialMode::parse("LIGHTNING_BRIGHTNESS"), Some(DialMode::LightingBrightness));
        assert_eq!(DialMode::parse("lighting brightness"), Some(DialMode::LightingBrightness));
        assert_eq!(DialMode::parse("8"), Some(DialMode::ScrollHorizontal));
        assert_eq!(DialMode::parse("9"), None);
        for m in DialMode::ALL {
            assert_eq!(serde_json::to_string(&m).unwrap(), format!("\"{}\"", m.name()));
            assert_eq!(DialMode::parse(m.name()), Some(m));
        }
    }

    #[test]
    fn parses_hardware_read() {
        let mut raw = [0u8; MAX_ARGS];
        raw[..4].copy_from_slice(&[1, 0, 0, 0]);
        let r = Reply { status: Status::Ok, transaction_id: 0x1F, size: 4, class: CLASS, id: GET_ACTIVE_MODE, raw };
        let s = parse_state(&r);
        assert_eq!(s.mode, Some(DialMode::Volume));
        assert_eq!(s.profile, 1);
        assert!(get_active_mode(0x1F, 1).is_read_only());
    }
}
