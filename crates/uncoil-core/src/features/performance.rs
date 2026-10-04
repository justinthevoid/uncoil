//! Mouse performance: sensitivity (DPI), the DPI stages the DPI button cycles through, and the USB poll
//! rate. Byte layouts are OpenRazer's shared mouse commands (`razerchromacommon.c` and
//! `razermouse_driver.c` at a84cd0ae, transcribed as facts). **None of these has been read on uncoil's own
//! devices yet**; the read-only checks in uncoild gate every write until one has.
//!
//! | class/id | name | args → reply |
//! |---|---|---|
//! | `04/05` | set DPI | `[storage, x_hi, x_lo, y_hi, y_lo, 0, 0]` (size 7) |
//! | `04/85` | get DPI | `[storage]` (size 7) → `[storage, x_hi, x_lo, y_hi, y_lo, …]` |
//! | `04/06` | set DPI stages | `[1, active, count, count × [index, x_hi, x_lo, y_hi, y_lo, 0, 0]]` (size `0x26`) |
//! | `04/86` | get DPI stages | `[1]` (size `0x26`) → the same layout |
//! | `00/05` / `00/85` | poll rate, classic | `[code]`: `1` = 1000, `2` = 500, `8` = 125 Hz |
//! | `00/40` / `00/C0` | poll rate, HyperPolling | `[arg, code]`; the reply carries the code in argument 1 |
//!
//! Storage byte: `0` (NOSTORE) for almost every modern mouse, so a DPI change lasts until the next DPI
//! button press or power cycle, like pressing the DPI button; `1` (VARSTORE) on the few mice OpenRazer lists.
//! Stages are always sent with VARSTORE, so they are an onboard write. The active stage is 1-based; each
//! stage record carries its 0-based index.

use crate::proto::{Reply, Report};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const DPI_CLASS: u8 = 0x04;
pub const SET_DPI: u8 = 0x05;
pub const GET_DPI: u8 = 0x85;
pub const SET_STAGES: u8 = 0x06;
pub const GET_STAGES: u8 = 0x86;
pub const POLL_CLASS: u8 = 0x00;
pub const SET_POLL: u8 = 0x05;
pub const GET_POLL: u8 = 0x85;
pub const SET_POLL_HYPER: u8 = 0x40;
pub const GET_POLL_HYPER: u8 = 0xC0;
pub const NOSTORE: u8 = 0x00;
pub const VARSTORE: u8 = 0x01;
/// OpenRazer caps the stage list at five.
pub const MAX_STAGES: usize = 5;
const DPI_SIZE: u8 = 0x07;
const STAGES_SIZE: u8 = 0x26;
const STAGE_RECORD: usize = 7;

/// Byte 0 of `04/05` (`storage` in the device file's `[dpi]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DpiStorage {
    /// Not stored: lasts until the DPI button or a power cycle changes it.
    #[default]
    Nostore,
    /// Stored in the device (an onboard write).
    Varstore,
}

impl DpiStorage {
    pub fn byte(self) -> u8 {
        match self {
            DpiStorage::Nostore => NOSTORE,
            DpiStorage::Varstore => VARSTORE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dpi {
    pub x: u16,
    pub y: u16,
}

impl Dpi {
    pub fn clamp(self, min: u16, max: u16) -> Dpi {
        Dpi { x: self.x.clamp(min, max), y: self.y.clamp(min, max) }
    }

    pub fn within(self, min: u16, max: u16) -> bool {
        (min..=max).contains(&self.x) && (min..=max).contains(&self.y)
    }
}

impl std::fmt::Display for Dpi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.x == self.y {
            write!(f, "{}", self.x)
        } else {
            write!(f, "{}x{}", self.x, self.y)
        }
    }
}

/// The stage list and which one is active (1-based).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DpiStages {
    pub active: u8,
    pub list: Vec<Dpi>,
}

impl DpiStages {
    /// Every stage clamped to `min..=max`; the active stage kept inside the list.
    pub fn clamp(&self, min: u16, max: u16) -> DpiStages {
        let list: Vec<Dpi> = self.list.iter().map(|d| d.clamp(min, max)).collect();
        let active = self.active.clamp(1, list.len().max(1) as u8);
        DpiStages { active, list }
    }
}

fn be(hi: u8, lo: u8) -> u16 {
    u16::from_be_bytes([hi, lo])
}

pub fn set_dpi(tid: u8, storage: DpiStorage, dpi: Dpi) -> Report {
    let [xh, xl] = dpi.x.to_be_bytes();
    let [yh, yl] = dpi.y.to_be_bytes();
    Report::new(tid, DPI_CLASS, SET_DPI, &[storage.byte(), xh, xl, yh, yl, 0, 0])
}

pub fn get_dpi(tid: u8, storage: DpiStorage) -> Report {
    Report::sized(tid, DPI_SIZE, DPI_CLASS, GET_DPI, &[storage.byte()])
}

/// `[storage, x_hi, x_lo, y_hi, y_lo]`. A zero axis is not a DPI any mouse runs at, so it is an error.
pub fn parse_dpi(reply: &Reply) -> Result<Dpi> {
    let a = &reply.raw;
    let d = Dpi { x: be(a[1], a[2]), y: be(a[3], a[4]) };
    if d.x == 0 || d.y == 0 {
        bail!("DPI reply reads {}x{}", d.x, d.y);
    }
    Ok(d)
}

pub fn set_stages(tid: u8, stages: &DpiStages) -> Result<Report> {
    let n = stages.list.len();
    if n == 0 || n > MAX_STAGES {
        bail!("a mouse takes 1 to {MAX_STAGES} DPI stages, not {n}");
    }
    if stages.active == 0 || stages.active as usize > n {
        bail!("active stage {} is not one of the {n} stages", stages.active);
    }
    let mut args = vec![VARSTORE, stages.active, n as u8];
    for (i, d) in stages.list.iter().enumerate() {
        let [xh, xl] = d.x.to_be_bytes();
        let [yh, yl] = d.y.to_be_bytes();
        args.extend_from_slice(&[i as u8, xh, xl, yh, yl, 0, 0]);
    }
    Ok(Report::sized(tid, STAGES_SIZE, DPI_CLASS, SET_STAGES, &args))
}

pub fn get_stages(tid: u8) -> Report {
    Report::sized(tid, STAGES_SIZE, DPI_CLASS, GET_STAGES, &[VARSTORE])
}

/// `[storage, active, count, count × 7-byte records]`.
pub fn parse_stages(reply: &Reply) -> Result<DpiStages> {
    let a = &reply.raw;
    let (active, count) = (a[1], a[2] as usize);
    if count == 0 || count > MAX_STAGES {
        bail!("DPI stage reply lists {count} stages");
    }
    if active == 0 || active as usize > count {
        bail!("DPI stage reply marks stage {active} of {count} as active");
    }
    let list = (0..count)
        .map(|i| {
            let r = &a[3 + i * STAGE_RECORD..3 + (i + 1) * STAGE_RECORD];
            Dpi { x: be(r[1], r[2]), y: be(r[3], r[4]) }
        })
        .collect();
    Ok(DpiStages { active, list })
}

/// Which poll-rate command a mouse answers (`kind` in the device file's `[poll_rate]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PollKind {
    /// `00/05` / `00/85`.
    #[default]
    Classic,
    /// `00/40` / `00/C0` (8000 Hz capable mice and dongles).
    Hyperpolling,
}

const CLASSIC: [(u16, u8); 3] = [(1000, 0x01), (500, 0x02), (125, 0x08)];
const HYPER: [(u16, u8); 7] =
    [(8000, 0x01), (4000, 0x02), (2000, 0x04), (1000, 0x08), (500, 0x10), (250, 0x20), (125, 0x40)];

impl PollKind {
    fn table(self) -> &'static [(u16, u8)] {
        match self {
            PollKind::Classic => &CLASSIC,
            PollKind::Hyperpolling => &HYPER,
        }
    }

    pub fn code(self, hz: u16) -> Option<u8> {
        self.table().iter().find(|(h, _)| *h == hz).map(|(_, c)| *c)
    }

    pub fn hz(self, code: u8) -> Option<u16> {
        self.table().iter().find(|(_, c)| *c == code).map(|(h, _)| *h)
    }
}

/// The report(s) that set `hz`. HyperPolling mice with `set_twice` get the command twice, argument 0 then
/// 1, as OpenRazer sends it to the DeathAdder V3 and Viper V3 Pro.
pub fn set_poll(tid: u8, kind: PollKind, hz: u16, set_twice: bool) -> Result<Vec<Report>> {
    let Some(code) = kind.code(hz) else {
        let rates: Vec<String> = kind.table().iter().rev().map(|(h, _)| h.to_string()).collect();
        bail!("{hz} Hz is not a poll rate this command knows ({} Hz)", rates.join(", "))
    };
    Ok(match kind {
        PollKind::Classic => vec![Report::new(tid, POLL_CLASS, SET_POLL, &[code])],
        PollKind::Hyperpolling if set_twice => vec![
            Report::new(tid, POLL_CLASS, SET_POLL_HYPER, &[0x00, code]),
            Report::new(tid, POLL_CLASS, SET_POLL_HYPER, &[0x01, code]),
        ],
        PollKind::Hyperpolling => vec![Report::new(tid, POLL_CLASS, SET_POLL_HYPER, &[0x00, code])],
    })
}

pub fn get_poll(tid: u8, kind: PollKind) -> Report {
    match kind {
        PollKind::Classic => Report::sized(tid, 0x01, POLL_CLASS, GET_POLL, &[]),
        PollKind::Hyperpolling => Report::sized(tid, 0x01, POLL_CLASS, GET_POLL_HYPER, &[]),
    }
}

pub fn parse_poll(reply: &Reply, kind: PollKind) -> Result<u16> {
    let code = match kind {
        PollKind::Classic => reply.raw[0],
        PollKind::Hyperpolling => reply.raw[1],
    };
    match kind.hz(code) {
        Some(hz) => Ok(hz),
        None => bail!("poll rate reply has unknown code 0x{code:02X}"),
    }
}

/// `performance.get` / `performance.set` result. Fields are `None` when the device lacks the feature or did
/// not answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceState {
    pub dpi: Option<Dpi>,
    pub dpi_min: Option<u16>,
    pub dpi_max: Option<u16>,
    pub stages: Option<DpiStages>,
    /// 0 when the mouse has no stages.
    pub stages_max: u8,
    pub poll_hz: Option<u16>,
    /// Empty when the mouse has no poll-rate setting.
    pub poll_rates: Vec<u16>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Status, MAX_ARGS};

    fn reply(class: u8, id: u8, args: &[u8]) -> Reply {
        let mut raw = [0u8; MAX_ARGS];
        raw[..args.len()].copy_from_slice(args);
        Reply { status: Status::Ok, transaction_id: 0x1F, size: args.len() as u8, class, id, raw }
    }

    #[test]
    fn dpi_reports() {
        // 1600 = 0x0640, 800 = 0x0320
        let w = set_dpi(0x1F, DpiStorage::Nostore, Dpi { x: 1600, y: 800 }).to_wire();
        assert_eq!(&w[1..16], &[0, 0x1F, 0, 0, 0, 7, 0x04, 0x05, 0, 0x06, 0x40, 0x03, 0x20, 0, 0]);
        let w = get_dpi(0x1F, DpiStorage::Nostore).to_wire();
        assert_eq!(&w[1..10], &[0, 0x1F, 0, 0, 0, 7, 0x04, 0x85, 0]);
        assert!(get_dpi(1, DpiStorage::Nostore).is_read_only());
        assert_eq!(set_dpi(1, DpiStorage::Varstore, Dpi { x: 400, y: 400 }).args[0], 1);
        let r = reply(4, 0x85, &[0, 0x06, 0x40, 0x06, 0x40]);
        assert_eq!(parse_dpi(&r).unwrap(), Dpi { x: 1600, y: 1600 });
        assert!(parse_dpi(&reply(4, 0x85, &[0, 0, 0, 0, 0])).is_err());
    }

    #[test]
    fn dpi_clamps() {
        assert_eq!(Dpi { x: 50, y: 40000 }.clamp(100, 30000), Dpi { x: 100, y: 30000 });
        assert!(Dpi { x: 100, y: 30000 }.within(100, 30000));
        assert!(!Dpi { x: 99, y: 800 }.within(100, 30000));
        let s = DpiStages { active: 9, list: vec![Dpi { x: 10, y: 10 }, Dpi { x: 800, y: 800 }] }.clamp(100, 30000);
        assert_eq!(s, DpiStages { active: 2, list: vec![Dpi { x: 100, y: 100 }, Dpi { x: 800, y: 800 }] });
        assert_eq!(Dpi { x: 800, y: 800 }.to_string(), "800");
        assert_eq!(Dpi { x: 800, y: 600 }.to_string(), "800x600");
    }

    #[test]
    fn stage_reports() {
        let s = DpiStages { active: 2, list: vec![Dpi { x: 400, y: 400 }, Dpi { x: 1600, y: 800 }] };
        let w = set_stages(0x1F, &s).unwrap().to_wire();
        assert_eq!(&w[1..9], &[0, 0x1F, 0, 0, 0, 0x26, 0x04, 0x06]);
        #[rustfmt::skip]
        assert_eq!(&w[9..26], &[
            1, 2, 2,
            0, 0x01, 0x90, 0x01, 0x90, 0, 0,
            1, 0x06, 0x40, 0x03, 0x20, 0, 0,
        ]);
        assert!(w[26..89].iter().all(|&b| b == 0));
        assert_eq!(&get_stages(0x1F).to_wire()[1..10], &[0, 0x1F, 0, 0, 0, 0x26, 0x04, 0x86, 1]);
        // the reply has the same layout as the request
        let back = parse_stages(&reply(4, 0x86, &w[9..9 + 17])).unwrap();
        assert_eq!(back, s);
        assert!(set_stages(1, &DpiStages { active: 1, list: vec![] }).is_err());
        assert!(set_stages(1, &DpiStages { active: 3, list: vec![Dpi { x: 400, y: 400 }; 2] }).is_err());
        assert!(set_stages(1, &DpiStages { active: 1, list: vec![Dpi { x: 400, y: 400 }; 6] }).is_err());
        assert!(parse_stages(&reply(4, 0x86, &[1, 0, 0])).is_err());
        assert!(parse_stages(&reply(4, 0x86, &[1, 4, 3])).is_err());
    }

    #[test]
    fn poll_reports() {
        let r = set_poll(0x1F, PollKind::Classic, 500, false).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(&r[0].to_wire()[1..10], &[0, 0x1F, 0, 0, 0, 1, 0x00, 0x05, 0x02]);
        let r = set_poll(0x1F, PollKind::Hyperpolling, 8000, true).unwrap();
        assert_eq!(r.iter().map(|r| r.args.clone()).collect::<Vec<_>>(), vec![vec![0, 1], vec![1, 1]]);
        assert_eq!((r[0].class, r[0].id, r[0].size), (0x00, 0x40, 2));
        let r = set_poll(0x1F, PollKind::Hyperpolling, 125, false).unwrap();
        assert_eq!(r.iter().map(|r| r.args.clone()).collect::<Vec<_>>(), vec![vec![0, 0x40]]);
        assert!(set_poll(1, PollKind::Classic, 8000, false).is_err());
        assert!(set_poll(1, PollKind::Classic, 250, false).is_err());
        assert_eq!(&get_poll(0x1F, PollKind::Classic).to_wire()[1..9], &[0, 0x1F, 0, 0, 0, 1, 0x00, 0x85]);
        assert_eq!(&get_poll(0x1F, PollKind::Hyperpolling).to_wire()[1..9], &[0, 0x1F, 0, 0, 0, 1, 0x00, 0xC0]);
        assert_eq!(parse_poll(&reply(0, 0x85, &[0x01]), PollKind::Classic).unwrap(), 1000);
        assert_eq!(parse_poll(&reply(0, 0xC0, &[0x00, 0x04]), PollKind::Hyperpolling).unwrap(), 2000);
        assert!(parse_poll(&reply(0, 0x85, &[0x03]), PollKind::Classic).is_err());
        for (kind, rates) in [(PollKind::Classic, &CLASSIC[..]), (PollKind::Hyperpolling, &HYPER[..])] {
            for (hz, code) in rates {
                assert_eq!(kind.code(*hz), Some(*code));
                assert_eq!(kind.hz(*code), Some(*hz));
            }
        }
    }

    #[test]
    fn json_shape() {
        let s = PerformanceState {
            dpi: Some(Dpi { x: 1600, y: 1600 }),
            dpi_min: Some(100),
            dpi_max: Some(30000),
            stages: Some(DpiStages { active: 3, list: vec![Dpi { x: 400, y: 400 }] }),
            stages_max: 5,
            poll_hz: None,
            poll_rates: vec![],
        };
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(
            j,
            r#"{"dpi":{"x":1600,"y":1600},"dpi_min":100,"dpi_max":30000,"stages":{"active":3,"list":[{"x":400,"y":400}]},"stages_max":5,"poll_hz":null,"poll_rates":[]}"#
        );
    }
}
