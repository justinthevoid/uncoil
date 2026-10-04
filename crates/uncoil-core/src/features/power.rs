//! Wireless mouse power: battery level, charging, the sleep (idle) timer and the low-battery threshold.
//! Byte layouts are OpenRazer's shared commands (`razerchromacommon.c` / `razermouse_driver.c` at
//! a84cd0ae, transcribed as facts). **Not yet read on uncoil's own devices.**
//!
//! | class/id | name | args → reply |
//! |---|---|---|
//! | `07/80` | battery level | size 2 → `[_, 0–255]` |
//! | `07/84` | charging | size 2 → `[_, 0 or 1]` |
//! | `07/03` / `07/83` | idle timer | `[secs_hi, secs_lo]`, 60–900 s, no storage byte |
//! | `07/01` / `07/81` | low-battery threshold | `[raw]`, `0x0C`–`0x3F` of 255 (about 5–25 %) |
//!
//! OpenRazer sends the low-battery pair with transaction id `0xFF` on the Basilisk V3 Pro, Viper V2 Pro
//! and Cobra Pro (`low_battery` in a device file's `[usb.transaction_ids]`).

use crate::proto::{Reply, Report};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const CLASS: u8 = 0x07;
pub const GET_BATTERY: u8 = 0x80;
pub const GET_CHARGING: u8 = 0x84;
pub const SET_IDLE: u8 = 0x03;
pub const GET_IDLE: u8 = 0x83;
pub const SET_LOW_BATTERY: u8 = 0x01;
pub const GET_LOW_BATTERY: u8 = 0x81;
pub const IDLE_MIN: u16 = 60;
pub const IDLE_MAX: u16 = 900;
pub const LOW_BATTERY_MIN: u8 = 0x0C;
pub const LOW_BATTERY_MAX: u8 = 0x3F;

pub fn get_battery(tid: u8) -> Report {
    Report::sized(tid, 0x02, CLASS, GET_BATTERY, &[])
}

/// Battery in percent (the device answers 0–255).
pub fn parse_battery(reply: &Reply) -> u8 {
    raw_to_pct(reply.raw[1])
}

pub fn get_charging(tid: u8) -> Report {
    Report::sized(tid, 0x02, CLASS, GET_CHARGING, &[])
}

pub fn parse_charging(reply: &Reply) -> Result<bool> {
    match reply.raw[1] {
        0 => Ok(false),
        1 => Ok(true),
        x => bail!("charging reply reads {x}"),
    }
}

/// Seconds of inactivity before the mouse sleeps, clamped to 60–900.
pub fn set_idle(tid: u8, secs: u16) -> Report {
    let [hi, lo] = secs.clamp(IDLE_MIN, IDLE_MAX).to_be_bytes();
    Report::new(tid, CLASS, SET_IDLE, &[hi, lo])
}

pub fn get_idle(tid: u8) -> Report {
    Report::sized(tid, 0x02, CLASS, GET_IDLE, &[])
}

pub fn parse_idle(reply: &Reply) -> Result<u16> {
    let s = u16::from_be_bytes([reply.raw[0], reply.raw[1]]);
    if !(IDLE_MIN..=IDLE_MAX).contains(&s) {
        bail!("idle timer reads {s} s, outside {IDLE_MIN}-{IDLE_MAX}");
    }
    Ok(s)
}

/// Raw threshold, clamped to `0x0C..=0x3F`.
pub fn set_low_battery(tid: u8, raw: u8) -> Report {
    Report::new(tid, CLASS, SET_LOW_BATTERY, &[raw.clamp(LOW_BATTERY_MIN, LOW_BATTERY_MAX)])
}

pub fn get_low_battery(tid: u8) -> Report {
    Report::sized(tid, 0x01, CLASS, GET_LOW_BATTERY, &[])
}

/// The raw threshold (0x0C..=0x3F).
pub fn parse_low_battery(reply: &Reply) -> Result<u8> {
    let raw = reply.raw[0];
    if !(LOW_BATTERY_MIN..=LOW_BATTERY_MAX).contains(&raw) {
        bail!("low-battery threshold reads 0x{raw:02X}, outside 0x0C-0x3F");
    }
    Ok(raw)
}

pub fn raw_to_pct(raw: u8) -> u8 {
    ((raw as u32 * 100 + 127) / 255) as u8
}

pub fn pct_to_raw(pct: u8) -> u8 {
    ((pct.min(100) as u32 * 255 + 50) / 100) as u8
}

/// The threshold range in percent: `[5, 25]`.
pub fn low_battery_range_pct() -> (u8, u8) {
    (raw_to_pct(LOW_BATTERY_MIN), raw_to_pct(LOW_BATTERY_MAX))
}

/// `power.get` / `power.set` result. `None` where the device lacks that part or did not answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerState {
    pub battery_pct: Option<u8>,
    pub charging: Option<bool>,
    pub idle_s: Option<u16>,
    pub idle_range: Option<(u16, u16)>,
    pub low_battery_pct: Option<u8>,
    pub low_battery_range: Option<(u8, u8)>,
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
    fn getters() {
        assert_eq!(&get_battery(0x1F).to_wire()[1..9], &[0, 0x1F, 0, 0, 0, 2, 0x07, 0x80]);
        assert_eq!(&get_charging(0x1F).to_wire()[1..9], &[0, 0x1F, 0, 0, 0, 2, 0x07, 0x84]);
        assert_eq!(&get_idle(0x1F).to_wire()[1..9], &[0, 0x1F, 0, 0, 0, 2, 0x07, 0x83]);
        assert_eq!(&get_low_battery(0xFF).to_wire()[1..9], &[0, 0xFF, 0, 0, 0, 1, 0x07, 0x81]);
        for r in [get_battery(1), get_charging(1), get_idle(1), get_low_battery(1)] {
            assert!(r.is_read_only());
        }
    }

    #[test]
    fn setters_clamp() {
        // 300 s = 0x012C
        assert_eq!(&set_idle(0x1F, 300).to_wire()[1..11], &[0, 0x1F, 0, 0, 0, 2, 0x07, 0x03, 0x01, 0x2C]);
        assert_eq!(set_idle(1, 5).args, vec![0, 60]);
        assert_eq!(set_idle(1, 5000).args, vec![0x03, 0x84]);
        assert_eq!(&set_low_battery(0xFF, 0x26).to_wire()[1..10], &[0, 0xFF, 0, 0, 0, 1, 0x07, 0x01, 0x26]);
        assert_eq!(set_low_battery(1, 0).args, vec![0x0C]);
        assert_eq!(set_low_battery(1, 0xFF).args, vec![0x3F]);
    }

    #[test]
    fn parsers() {
        assert_eq!(parse_battery(&reply(0x80, &[0, 199])), 78);
        assert_eq!(parse_battery(&reply(0x80, &[0, 255])), 100);
        assert!(!parse_charging(&reply(0x84, &[0, 0])).unwrap());
        assert!(parse_charging(&reply(0x84, &[0, 1])).unwrap());
        assert!(parse_charging(&reply(0x84, &[0, 7])).is_err());
        assert_eq!(parse_idle(&reply(0x83, &[0x01, 0x2C])).unwrap(), 300);
        assert!(parse_idle(&reply(0x83, &[0, 0])).is_err());
        assert_eq!(parse_low_battery(&reply(0x81, &[0x26])).unwrap(), 0x26);
        assert!(parse_low_battery(&reply(0x81, &[0x40])).is_err());
    }

    #[test]
    fn percent_conversions() {
        assert_eq!(low_battery_range_pct(), (5, 25));
        assert_eq!(pct_to_raw(15), 38);
        assert_eq!(raw_to_pct(38), 15);
        for pct in 5..=25 {
            assert_eq!(raw_to_pct(pct_to_raw(pct)), pct, "{pct}%");
        }
        assert_eq!(pct_to_raw(200), 255);
    }

    #[test]
    fn json_shape() {
        let s = PowerState {
            battery_pct: Some(78),
            charging: Some(false),
            idle_s: Some(300),
            idle_range: Some((60, 900)),
            low_battery_pct: Some(15),
            low_battery_range: Some((5, 25)),
        };
        assert_eq!(
            serde_json::to_string(&s).unwrap(),
            r#"{"battery_pct":78,"charging":false,"idle_s":300,"idle_range":[60,900],"low_battery_pct":15,"low_battery_range":[5,25]}"#
        );
    }
}
