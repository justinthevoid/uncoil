//! Razer HID feature-report protocol.
//!
//! Every command is one 90-byte feature report (sent with report id 0, so 91 bytes on the wire):
//!
//! | offset | field            |
//! |-------:|------------------|
//! | 0      | status           |
//! | 1      | transaction id   |
//! | 2..4   | remaining packets|
//! | 4      | protocol type    |
//! | 5      | data size        |
//! | 6      | command class    |
//! | 7      | command id       |
//! | 8..88  | arguments (80)   |
//! | 88     | CRC: XOR of bytes 2..88 |
//! | 89     | reserved         |
//!
//! The same layout is used by OpenRazer and OpenRGB; command names below are the ones Razer's
//! own software logs (see docs/PROTOCOL.md).

pub const REPORT_LEN: usize = 90;
/// Report as written to hidapi: leading report-id byte (0) + the 90-byte report.
pub const WIRE_LEN: usize = REPORT_LEN + 1;
pub const MAX_ARGS: usize = 80;

/// Reply status byte (offset 0 of the device's answer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    New,
    Busy,
    Ok,
    Fail,
    Timeout,
    Unsupported,
    Other(u8),
}

impl From<u8> for Status {
    fn from(b: u8) -> Self {
        match b {
            0x00 => Status::New,
            0x01 => Status::Busy,
            0x02 => Status::Ok,
            0x03 => Status::Fail,
            0x04 => Status::Timeout,
            0x05 => Status::Unsupported,
            x => Status::Other(x),
        }
    }
}

/// Device mode (command 0x00/0x04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DeviceMode {
    /// Firmware handles Fn layer, media keys, dial, DPI buttons.
    Normal = 0x00,
    /// Host ("driver") handles them; what Synapse uses.
    Driver = 0x03,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub transaction_id: u8,
    /// Value of the "data size" byte. Usually `args.len()`, but several getters announce a larger
    /// size than the arguments they send (e.g. `0x50` for list replies), exactly as Synapse does.
    pub size: u8,
    pub class: u8,
    pub id: u8,
    pub args: Vec<u8>,
}

impl Report {
    pub fn new(transaction_id: u8, class: u8, id: u8, args: &[u8]) -> Self {
        assert!(args.len() <= MAX_ARGS, "razer report args exceed 80 bytes");
        Report { transaction_id, size: args.len() as u8, class, id, args: args.to_vec() }
    }

    /// Like [`Report::new`] but with an explicit data-size byte (`size >= args.len()`).
    pub fn sized(transaction_id: u8, size: u8, class: u8, id: u8, args: &[u8]) -> Self {
        assert!(args.len() <= size as usize && size as usize <= MAX_ARGS, "bad razer report size");
        Report { transaction_id, size, class, id, args: args.to_vec() }
    }

    /// Commands with the high bit set in their id only read state ("get"); everything else may write.
    pub fn is_read_only(&self) -> bool {
        self.id & 0x80 != 0
    }

    /// Serialise to the 91-byte buffer hidapi expects (report id 0 first).
    pub fn to_wire(&self) -> [u8; WIRE_LEN] {
        let mut w = [0u8; WIRE_LEN];
        let r = &mut w[1..];
        r[1] = self.transaction_id;
        r[5] = self.size;
        r[6] = self.class;
        r[7] = self.id;
        r[8..8 + self.args.len()].copy_from_slice(&self.args);
        r[88] = crc(r);
        w
    }
}

/// XOR of bytes 2..88 of the 90-byte report (i.e. excluding status and transaction id).
pub fn crc(report: &[u8]) -> u8 {
    report[2..88].iter().fold(0, |a, b| a ^ b)
}

/// Status byte from a 91-byte reply buffer as returned by `get_feature_report` (report id first).
pub fn reply_status(wire: &[u8]) -> Status {
    Status::from(wire.get(1).copied().unwrap_or(0))
}

/// A parsed device reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: Status,
    pub transaction_id: u8,
    /// Data-size byte as reported by the device.
    pub size: u8,
    pub class: u8,
    pub id: u8,
    /// All 80 argument bytes (devices do not always zero what lies beyond `size`).
    pub raw: [u8; MAX_ARGS],
}

impl Reply {
    /// Parse a 91-byte buffer (report id first) or a bare 90-byte report.
    pub fn parse(buf: &[u8]) -> Option<Reply> {
        let r = match buf.len() {
            WIRE_LEN => &buf[1..],
            REPORT_LEN => buf,
            _ => return None,
        };
        let mut raw = [0u8; MAX_ARGS];
        raw.copy_from_slice(&r[8..88]);
        Some(Reply { status: Status::from(r[0]), transaction_id: r[1], size: r[5], class: r[6], id: r[7], raw })
    }

    /// The argument bytes the device declared (`size`, capped at 80).
    pub fn args(&self) -> &[u8] {
        &self.raw[..(self.size as usize).min(MAX_ARGS)]
    }

    /// Does this reply answer `request` (same command class and id)?
    pub fn answers(&self, request: &Report) -> bool {
        self.class == request.class && self.id == request.id
    }

    /// Build a reply buffer as a device would send it (used by fake devices in tests).
    pub fn to_wire(&self) -> [u8; WIRE_LEN] {
        let mut w = [0u8; WIRE_LEN];
        let r = &mut w[1..];
        r[0] = status_byte(self.status);
        r[1] = self.transaction_id;
        r[5] = self.size;
        r[6] = self.class;
        r[7] = self.id;
        r[8..88].copy_from_slice(&self.raw);
        r[88] = crc(r);
        w
    }
}

fn status_byte(s: Status) -> u8 {
    match s {
        Status::New => 0x00,
        Status::Busy => 0x01,
        Status::Ok => 0x02,
        Status::Fail => 0x03,
        Status::Timeout => 0x04,
        Status::Unsupported => 0x05,
        Status::Other(x) => x,
    }
}

/// Something that can answer a report: the real HID device (uncoil-hid) or a fake in tests.
/// Implementations retry `busy` replies and skip replies that answer a different command.
pub trait Transport {
    fn query(&mut self, request: &Report) -> anyhow::Result<Reply>;
}

/// Query and require an `ok` status.
pub fn query_ok(t: &mut dyn Transport, request: &Report) -> anyhow::Result<Reply> {
    let reply = t.query(request)?;
    match reply.status {
        Status::Ok => Ok(reply),
        s => anyhow::bail!("device answered {:?} to {:02X}/{:02X}", s, request.class, request.id),
    }
}

/// Query on a read path (key map reads, checks, `*.get`, probes): refuses anything but a "get" command
/// (high bit of the command id set), so no read can turn into a write whatever the device file says.
pub fn query_read(t: &mut dyn Transport, request: &Report) -> anyhow::Result<Reply> {
    if !request.is_read_only() {
        anyhow::bail!("{:02X}/{:02X} is not a read command; refusing it on a read path", request.class, request.id);
    }
    query_ok(t, request)
}

/// Commands that some firmwares want with their own transaction id (`[usb.transaction_ids]` in a device
/// file). OpenRazer sends, for example, a Viper Mini's lighting with `0x3F` and its DPI with `0xFF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandGroup {
    /// Custom frame rows `0F/03` and the custom-frame effect (`0F/02` with effect 8).
    Frame,
    /// Every other class-`0F` command: firmware effects, brightness, regions.
    Effect,
    /// `02/0D`, `02/8D`, `02/0C`, `02/8C`, `02/84`.
    Keymap,
    /// Class `05`.
    Profile,
    /// `04/05`, `04/85`, `04/06`, `04/86`.
    Dpi,
    /// `00/05`, `00/85`, `00/40`, `00/C0`.
    Poll,
    /// `07/80`, `07/84`, `07/03`, `07/83` (and the low-battery pair when `low_battery` is not set).
    Power,
    /// `07/01`, `07/81`.
    LowBattery,
    /// `00/04`, `00/84`, `00/81`, `00/82`.
    Device,
    /// Anything else: always the endpoint's `transaction_id`.
    Other,
}

impl CommandGroup {
    pub fn of(r: &Report) -> CommandGroup {
        match (r.class, r.id) {
            (0x0F, 0x03) => CommandGroup::Frame,
            (0x0F, 0x02) if r.args.get(2) == Some(&0x08) => CommandGroup::Frame,
            (0x0F, _) => CommandGroup::Effect,
            (0x02, 0x0D | 0x8D | 0x0C | 0x8C | 0x84) => CommandGroup::Keymap,
            (0x05, _) => CommandGroup::Profile,
            (0x04, 0x05 | 0x85 | 0x06 | 0x86) => CommandGroup::Dpi,
            (0x00, 0x05 | 0x85 | 0x40 | 0xC0) => CommandGroup::Poll,
            (0x07, 0x01 | 0x81) => CommandGroup::LowBattery,
            (0x07, 0x80 | 0x84 | 0x03 | 0x83) => CommandGroup::Power,
            (0x00, 0x04 | 0x84 | 0x81 | 0x82) => CommandGroup::Device,
            _ => CommandGroup::Other,
        }
    }
}

// ---- known commands ---------------------------------------------------------------------------

pub fn set_device_mode(tid: u8, mode: DeviceMode) -> Report {
    Report::new(tid, 0x00, 0x04, &[mode as u8, 0x00])
}

pub fn get_device_mode(tid: u8) -> Report {
    Report::new(tid, 0x00, 0x84, &[0x00, 0x00])
}

/// Extended-matrix "custom frame" effect: show whatever frame rows are uploaded.
/// Send ONCE; re-sending every frame freezes some keyboards on the first frame.
pub fn effect_custom_frame(tid: u8) -> Report {
    let mut args = [0u8; 12];
    args[2] = 0x08;
    Report::new(tid, 0x0F, 0x02, &args)
}

/// Extended-matrix hardware wave. `speed`: higher = slower (firmware default 0x28).
pub fn effect_wave(tid: u8, direction: u8, speed: u8) -> Report {
    Report::new(tid, 0x0F, 0x02, &[0x00, 0x00, 0x04, direction, speed, 0x00])
}

/// One row of a custom frame (extended matrix): colours for columns `start..=stop`. At most 25 columns fit
/// one report; device files cannot ask for more (`device::MAX_COLS`), and this refuses rather than panics.
pub fn custom_frame_row(tid: u8, row: u8, start: u8, colors: &[[u8; 3]]) -> anyhow::Result<Report> {
    if colors.is_empty() || colors.len() * 3 + 5 > MAX_ARGS || start as usize + colors.len() > 256 {
        anyhow::bail!("a frame row of {} colours does not fit one report", colors.len());
    }
    let stop = start + (colors.len() - 1) as u8;
    let mut args = Vec::with_capacity(5 + colors.len() * 3);
    args.extend_from_slice(&[0x00, 0x00, row, start, stop]);
    for c in colors {
        args.extend_from_slice(c);
    }
    Ok(Report::new(tid, 0x0F, 0x03, &args))
}

/// OLED command dial (BlackWidow V4 Pro 75%): which function the dial drives.
pub fn set_command_dial_mode(tid: u8, profile: u8, mode: u8, display_order: u8, enabled_functions: u8) -> Report {
    Report::new(tid, 0x17, 0x00, &[profile, mode, display_order, enabled_functions])
}

pub fn set_oled_brightness(tid: u8, percent: u8) -> Report {
    Report::new(tid, 0x17, 0x03, &[percent.min(100)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_mode_report_matches_synapse_bytes() {
        // From Synapse's log: dataSend [0,5,0,0,0,2,0,4,3,0,...] for "Set Device Mode" (driver), tid 5
        let w = Report::new(0x05, 0x00, 0x04, &[3, 0]).to_wire();
        assert_eq!(&w[1..11], &[0, 5, 0, 0, 0, 2, 0, 4, 3, 0]);
    }

    #[test]
    fn command_dial_report_matches_synapse_bytes() {
        // "Set OLED Command dial active mode": dataSend [0,6,0,0,0,4,23,0,1,0,1,6,...]
        let w = set_command_dial_mode(6, 1, 0, 1, 6).to_wire();
        assert_eq!(&w[1..13], &[0, 6, 0, 0, 0, 4, 23, 0, 1, 0, 1, 6]);
    }

    #[test]
    fn crc_is_xor_of_body() {
        let w = custom_frame_row(0x1F, 2, 0, &[[1, 2, 3], [4, 5, 6]]).unwrap().to_wire();
        let r = &w[1..];
        let expect = r[2..88].iter().fold(0u8, |a, b| a ^ b);
        assert_eq!(r[88], expect);
        assert_eq!(r[5], 11); // 5 header args + 2*3 colour bytes
        assert_eq!(&r[8..13], &[0, 0, 2, 0, 1]);
    }

    #[test]
    fn full_keyboard_row_fits() {
        let row = [[0u8; 3]; 18];
        let r = custom_frame_row(0x1F, 0, 0, &row).unwrap();
        assert_eq!(r.args.len(), 5 + 54);
        assert!(custom_frame_row(0x1F, 0, 0, &[]).is_err());
        assert!(custom_frame_row(0x1F, 0, 250, &[[0; 3]; 10]).is_err(), "stop column would wrap");
    }

    #[test]
    fn sized_report_keeps_declared_size() {
        // obm_probe.py: profile id list is requested with data size 0x50 and no arguments
        let w = Report::sized(0x1F, 0x50, 0x05, 0x81, &[]).to_wire();
        assert_eq!(&w[1..9], &[0, 0x1F, 0, 0, 0, 0x50, 0x05, 0x81]);
        assert!(Report::sized(0x1F, 0x50, 0x05, 0x81, &[]).is_read_only());
        assert!(!Report::new(0x1F, 0x02, 0x0D, &[1]).is_read_only());
    }

    #[test]
    fn reply_roundtrip() {
        let mut raw = [0u8; MAX_ARGS];
        raw[..3].copy_from_slice(&[1, 26, 1]);
        let r = Reply { status: Status::Ok, transaction_id: 0x1F, size: 7, class: 2, id: 0x8D, raw };
        let back = Reply::parse(&r.to_wire()).unwrap();
        assert_eq!(back, r);
        assert_eq!(back.args().len(), 7);
        assert!(back.answers(&Report::new(0x1F, 2, 0x8D, &[1, 26, 1])));
        assert!(!back.answers(&Report::new(0x1F, 2, 0x0D, &[1, 26, 1])));
    }

    #[test]
    fn command_groups() {
        use CommandGroup as G;
        assert_eq!(G::of(&custom_frame_row(1, 0, 0, &[[0, 0, 0]]).unwrap()), G::Frame);
        assert_eq!(G::of(&effect_custom_frame(1)), G::Frame);
        assert_eq!(G::of(&effect_wave(1, 1, 0x28)), G::Effect);
        assert_eq!(G::of(&Report::new(1, 0x0F, 0x80, &[])), G::Effect);
        assert_eq!(G::of(&Report::new(1, 0x02, 0x8D, &[1, 26, 0])), G::Keymap);
        assert_eq!(G::of(&Report::new(1, 0x02, 0x14, &[1, 0])), G::Other, "scroll mode is not a key map command");
        assert_eq!(G::of(&Report::new(1, 0x05, 0x81, &[])), G::Profile);
        assert_eq!(G::of(&Report::new(1, 0x04, 0x86, &[1])), G::Dpi);
        assert_eq!(G::of(&Report::new(1, 0x00, 0xC0, &[])), G::Poll);
        assert_eq!(G::of(&Report::new(1, 0x07, 0x81, &[])), G::LowBattery);
        assert_eq!(G::of(&Report::new(1, 0x07, 0x83, &[])), G::Power);
        assert_eq!(G::of(&set_device_mode(1, DeviceMode::Normal)), G::Device);
        assert_eq!(G::of(&set_oled_brightness(1, 50)), G::Other);
    }

    struct Echo(Vec<(u8, u8)>);
    impl Transport for Echo {
        fn query(&mut self, r: &Report) -> anyhow::Result<Reply> {
            self.0.push((r.class, r.id));
            Ok(Reply { status: Status::Ok, transaction_id: 0, size: 0, class: r.class, id: r.id, raw: [0; MAX_ARGS] })
        }
    }

    #[test]
    fn read_paths_send_only_get_commands() {
        let mut t = Echo(vec![]);
        assert!(query_read(&mut t, &get_device_mode(0x1F)).is_ok());
        let e = query_read(&mut t, &set_device_mode(0x1F, DeviceMode::Normal)).unwrap_err().to_string();
        assert!(e.contains("not a read command"), "{e}");
        assert!(query_read(&mut t, &Report::new(0x1F, 0x02, 0x0D, &[1, 26, 1])).is_err());
        assert_eq!(t.0, vec![(0x00, 0x84)], "the refused setters never reached the device");
    }

    #[test]
    fn status_parsing() {
        let mut w = [0u8; WIRE_LEN];
        w[1] = 0x02;
        assert_eq!(reply_status(&w), Status::Ok);
        w[1] = 0x01;
        assert_eq!(reply_status(&w), Status::Busy);
    }
}
