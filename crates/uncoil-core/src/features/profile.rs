//! Onboard profiles (class `0x05`). Command layout from OpenSynapse's `ViperObmProtocol` (MIT), confirmed
//! read-only on the BlackWidow V4 Pro 75% and the Basilisk V3 Pro (2026-10-03).
//!
//! | class/id | name | reply |
//! |---|---|---|
//! | `05/8A` | max profiles | `[5]` on both devices |
//! | `05/80` | profile count | `[1]` |
//! | `05/81` | profile ids | `[count, ids…]` (keyboard `[1, 1]`; the mouse answers `[5, 1]` with data size 2) |
//! | `05/84` | **probably** the active profile | `[1]` on both devices (only one profile exists, so unconfirmed) |
//!
//! Switching the active profile: not observed in Synapse's logs or OpenSynapse. By the get/set convention it
//! would be `05/04 [id]`, but uncoil does not send it until that is confirmed.

use crate::proto::{Reply, Report};
use serde::{Deserialize, Serialize};

pub const CLASS: u8 = 0x05;
pub const GET_COUNT: u8 = 0x80;
pub const GET_IDS: u8 = 0x81;
pub const GET_ACTIVE: u8 = 0x84;
pub const GET_MAX: u8 = 0x8A;

pub fn get_max(tid: u8) -> Report {
    Report::sized(tid, 0x01, CLASS, GET_MAX, &[])
}

pub fn get_count(tid: u8) -> Report {
    Report::sized(tid, 0x01, CLASS, GET_COUNT, &[])
}

pub fn get_ids(tid: u8) -> Report {
    Report::sized(tid, 0x50, CLASS, GET_IDS, &[])
}

pub fn get_active(tid: u8) -> Report {
    Report::sized(tid, 0x50, CLASS, GET_ACTIVE, &[])
}

/// First argument byte of a scalar reply.
pub fn parse_byte(reply: &Reply) -> u8 {
    reply.raw[0]
}

/// `[count, ids…]`, clipped to the declared data size; zero ids dropped.
pub fn parse_ids(reply: &Reply) -> Vec<u8> {
    let a = reply.args();
    let n = a.first().copied().unwrap_or(0) as usize;
    a.iter().skip(1).take(n).copied().filter(|&id| id != 0).collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub max: u8,
    pub count: u8,
    pub ids: Vec<u8>,
    /// From `05/84`; see the module docs for how sure that is.
    #[serde(default)]
    pub active: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Status, MAX_ARGS};

    fn reply(size: u8, args: &[u8]) -> Reply {
        let mut raw = [0u8; MAX_ARGS];
        raw[..args.len()].copy_from_slice(args);
        Reply { status: Status::Ok, transaction_id: 0x1F, size, class: CLASS, id: GET_IDS, raw }
    }

    #[test]
    fn requests_match_opensynapse() {
        assert_eq!(&get_max(0x1F).to_wire()[1..9], &[0, 0x1F, 0, 0, 0, 0x01, 0x05, 0x8A]);
        assert_eq!(&get_count(0x1F).to_wire()[1..9], &[0, 0x1F, 0, 0, 0, 0x01, 0x05, 0x80]);
        assert_eq!(&get_ids(0x1F).to_wire()[1..9], &[0, 0x1F, 0, 0, 0, 0x50, 0x05, 0x81]);
        for r in [get_max(1), get_count(1), get_ids(1), get_active(1)] {
            assert!(r.is_read_only());
        }
    }

    #[test]
    fn parses_real_replies() {
        // keyboard: data size 80, [1, 1]
        assert_eq!(parse_ids(&reply(80, &[1, 1])), vec![1]);
        // mouse: data size 2, [5, 1]
        assert_eq!(parse_ids(&reply(2, &[5, 1])), vec![1]);
        assert_eq!(parse_byte(&reply(1, &[5])), 5);
    }
}
