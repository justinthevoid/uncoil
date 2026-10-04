//! What a device says about itself: firmware version (every Razer device) and, on keyboards, the layout
//! and colour variant. Read-only; never the serial number (`00/82`).
//!
//! | class/id | name | args → reply | source |
//! |---|---|---|---|
//! | `00/81` | firmware version | size 2 → `[major, minor]` | OpenRazer `razerchromacommon.c` (`get_firmware_version`), OpenRGB `RazerController.cpp` |
//! | `00/86` | keyboard info | size 2 → `[layout, variant]` | OpenRazer `razerkbd_driver.c` (`kbd_layout`), OpenRGB `RazerController.cpp` (`razer_get_keyboard_info`) |
//!
//! Layout codes and variants are OpenRGB's table (`RazerController.h`, most layouts marked "unconfirmed"
//! there) plus OpenRazer's `0x81` (US, Mac). The physical shape (ANSI / ISO / JIS) is OpenRGB's mapping.

use crate::proto::{Reply, Report};
use serde::{Deserialize, Serialize};

pub const CLASS: u8 = 0x00;
pub const GET_FIRMWARE: u8 = 0x81;
pub const GET_KEYBOARD_INFO: u8 = 0x86;

pub fn get_firmware(tid: u8) -> Report {
    Report::sized(tid, 0x02, CLASS, GET_FIRMWARE, &[])
}

/// `"1.04"` from `[1, 4]` (major, then the minor with two digits, as Razer writes its versions; OpenRazer and
/// OpenRGB print the same bytes as `v1.4`). `None` for an all-zero reply (no version).
pub fn parse_firmware(reply: &Reply) -> Option<String> {
    let (major, minor) = (reply.raw[0], reply.raw[1]);
    (major, minor).ne(&(0, 0)).then(|| format!("{major}.{minor:02}"))
}

pub fn get_keyboard_info(tid: u8) -> Report {
    Report::sized(tid, 0x02, CLASS, GET_KEYBOARD_INFO, &[])
}

/// (layout code, variant byte) from `00/86`.
pub fn parse_keyboard_info(reply: &Reply) -> (u8, u8) {
    (reply.raw[0], reply.raw[1])
}

/// A layout code as a name with its physical shape, e.g. `"US (ANSI)"`; `None` for 0 (none) and unknown codes.
pub fn layout_name(code: u8) -> Option<&'static str> {
    Some(match code {
        1 => "US (ANSI)",
        2 => "Greek (ISO)",
        3 => "German (ISO)",
        4 => "French (ISO)",
        5 => "Russian (ANSI)",
        6 => "UK (ISO)",
        7 => "Nordic (ISO)",
        8 => "Traditional Chinese (ANSI)",
        9 => "Korean (ISO)",
        10 => "Turkish (ANSI)",
        11 => "Thai (ANSI)",
        12 => "Japanese (JIS)",
        13 => "Portuguese, Brazil (ISO)",
        14 => "Spanish, Latin America (ISO)",
        15 => "Swiss (ISO)",
        16 => "Spanish, Spain (ISO)",
        17 => "Italian (ISO)",
        18 => "Portuguese, Portugal (ISO)",
        19 => "Hebrew (ISO)",
        20 => "Arabic (ANSI)",
        0x81 => "US, Mac (ANSI)",
        _ => return None,
    })
}

/// The colour variant byte as a name: Black, Quartz (pink) or Mercury (white); `None` otherwise.
pub fn variant_name(b: u8) -> Option<&'static str> {
    Some(match b {
        0x00 => "Black",
        0x80 => "Quartz",
        0x82 => "Mercury",
        _ => return None,
    })
}

/// `info.get` result. `None` where the device does not have it or did not answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceDetails {
    /// `"1.04"`.
    pub firmware: Option<String>,
    /// Keyboards: the layout's name (`"UK (ISO)"`); `None` for a code without one (see `layout_code`).
    pub layout: Option<String>,
    /// Keyboards: the raw layout code from `00/86`.
    pub layout_code: Option<u8>,
    /// Keyboards: `"Black"`, `"Quartz"` or `"Mercury"`.
    pub variant: Option<String>,
}

impl DeviceDetails {
    /// Fill the keyboard fields from a `00/86` reply.
    pub fn apply_keyboard_info(&mut self, reply: &Reply) {
        let (code, variant) = parse_keyboard_info(reply);
        self.layout_code = Some(code);
        self.layout = layout_name(code).map(String::from);
        self.variant = variant_name(variant).map(String::from);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{CommandGroup, Status, MAX_ARGS};

    fn reply(id: u8, args: &[u8]) -> Reply {
        let mut raw = [0u8; MAX_ARGS];
        raw[..args.len()].copy_from_slice(args);
        Reply { status: Status::Ok, transaction_id: 0x1F, size: 2, class: CLASS, id, raw }
    }

    #[test]
    fn reports_are_reads_in_the_device_group() {
        assert_eq!(&get_firmware(0x1F).to_wire()[1..9], &[0, 0x1F, 0, 0, 0, 2, 0x00, 0x81]);
        assert_eq!(&get_keyboard_info(0x1F).to_wire()[1..9], &[0, 0x1F, 0, 0, 0, 2, 0x00, 0x86]);
        for r in [get_firmware(1), get_keyboard_info(1)] {
            assert!(r.is_read_only());
            assert_eq!(CommandGroup::of(&r), CommandGroup::Device);
        }
    }

    #[test]
    fn firmware_layout_and_variant() {
        assert_eq!(parse_firmware(&reply(0x81, &[1, 4])).as_deref(), Some("1.04"));
        assert_eq!(parse_firmware(&reply(0x81, &[2, 15])).as_deref(), Some("2.15"));
        assert_eq!(parse_firmware(&reply(0x81, &[0, 0])), None);
        let mut d = DeviceDetails::default();
        d.apply_keyboard_info(&reply(0x86, &[6, 0x82]));
        assert_eq!(
            d,
            DeviceDetails {
                firmware: None,
                layout: Some("UK (ISO)".into()),
                layout_code: Some(6),
                variant: Some("Mercury".into())
            }
        );
        d.apply_keyboard_info(&reply(0x86, &[42, 0x07]));
        assert_eq!((d.layout, d.layout_code, d.variant), (None, Some(42), None), "unknown code: raw code only");
        assert_eq!(layout_name(1), Some("US (ANSI)"));
        assert_eq!(layout_name(3), Some("German (ISO)"));
        assert_eq!(layout_name(12), Some("Japanese (JIS)"));
        assert_eq!(layout_name(0), None);
        assert_eq!(variant_name(0x80), Some("Quartz"));
        assert_eq!(variant_name(0x00), Some("Black"));
    }
}
