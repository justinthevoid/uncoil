//! Device features beyond streaming lighting frames. Each module is pure: it builds [`Report`]s and parses
//! [`Reply`]s, nothing more. The daemon (uncoild) is the only thing that sends them, from the device's own
//! renderer thread, so feature commands never interleave with frame streaming.
//!
//! Byte layouts come from Razer's own Synapse logs where Synapse logs them (asserted in the tests), from
//! OpenRazer / OpenRGB / OpenSynapse otherwise, and from read-only probes of real hardware. See
//! `docs/PROTOCOL.md` for sources and what is still unknown.
//!
//! [`Report`]: crate::proto::Report
//! [`Reply`]: crate::proto::Reply

pub mod dial;
pub mod hw_effect;
pub mod keymap;
pub mod oled;
pub mod performance;
pub mod power;
pub mod profile;

use serde::{Deserialize, Serialize};

/// What a device supports, declared per device in `devices/*.toml` (`features = [...]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    /// Per-LED software lighting (custom frames streamed by the daemon).
    Lighting,
    /// Firmware lighting effects (`0F/02`).
    HwEffects,
    /// Onboard key / button mapping, including the Hypershift (Fn) layer.
    Keymap,
    /// Onboard profiles (list / count).
    Profiles,
    /// OLED command dial (BlackWidow V4 Pro 75%).
    Dial,
    /// OLED display settings.
    Oled,
    /// Mouse sensitivity: current DPI and the DPI stages (`[dpi]` in the device file).
    Dpi,
    /// USB poll rate (`[poll_rate]`).
    PollRate,
    /// Battery, charging, sleep timer and low-battery threshold (`[power]`).
    Power,
}

impl Feature {
    pub const ALL: [Feature; 9] = [
        Feature::Lighting,
        Feature::HwEffects,
        Feature::Keymap,
        Feature::Profiles,
        Feature::Dial,
        Feature::Oled,
        Feature::Dpi,
        Feature::PollRate,
        Feature::Power,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Feature::Lighting => "lighting",
            Feature::HwEffects => "hw_effects",
            Feature::Keymap => "keymap",
            Feature::Profiles => "profiles",
            Feature::Dial => "dial",
            Feature::Oled => "oled",
            Feature::Dpi => "dpi",
            Feature::PollRate => "poll_rate",
            Feature::Power => "power",
        }
    }
}

/// Parse a lowercase hex or decimal byte ("0x46", "70").
pub fn parse_u8(s: &str) -> Option<u8> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => u8::from_str_radix(h, 16).ok(),
        None => s.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_names_roundtrip() {
        for f in Feature::ALL {
            let j = serde_json::to_string(&f).unwrap();
            assert_eq!(j, format!("\"{}\"", f.as_str()));
            assert_eq!(serde_json::from_str::<Feature>(&j).unwrap(), f);
        }
    }

    #[test]
    fn parse_bytes() {
        assert_eq!(parse_u8("0x46"), Some(0x46));
        assert_eq!(parse_u8("70"), Some(70));
        assert_eq!(parse_u8("256"), None);
    }
}
