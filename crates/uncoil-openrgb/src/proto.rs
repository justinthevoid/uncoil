//! The controller data block (`NET_PACKET_ID_REQUEST_CONTROLLER_DATA` reply) and the `UpdateLEDs` payload,
//! for protocol versions 0 to 5 (OpenRGBSDK.md: Device Data, Mode Data, Zone Data, Segment Data, Matrix Map
//! Data, LED Data). Only what a lighting client needs is kept; the rest (description, version, serial,
//! location, modes, colours) is skipped without being stored.

use crate::{bad, MAX_LEDS};
use std::io;

/// An OpenRGB controller (one device as OpenRGB lists it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Controller {
    /// OpenRGB device type (`device_type::MOTHERBOARD`, ...).
    pub kind: i32,
    pub name: String,
    /// Empty before protocol 1.
    pub vendor: String,
    pub zones: Vec<Zone>,
    /// LED names, in LED order (zone after zone).
    pub leds: Vec<String>,
    /// How many colours `UpdateLEDs` takes (normally one per LED).
    pub colors: usize,
    /// Controller flags (protocol 5; 0 before).
    pub flags: u32,
}

/// One zone of a controller: its LEDs are the next `leds` LEDs in the controller's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    pub name: String,
    /// 0 single, 1 linear, 2 matrix (later versions add loop and segmented kinds; treat unknown as linear).
    pub kind: i32,
    pub leds: u32,
    pub matrix: Option<Matrix>,
}

/// A matrix zone's map: `map[row * width + col]` is the zone's LED index there, `u32::MAX` for none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matrix {
    pub height: u32,
    pub width: u32,
    pub map: Vec<u32>,
}

/// Little-endian reader over a data block; every read fails cleanly at the end.
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        let end =
            self.at.checked_add(n).filter(|&e| e <= self.b.len()).ok_or_else(|| bad("controller data ends early"))?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u16(&mut self) -> io::Result<u16> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> io::Result<u32> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn i32(&mut self) -> io::Result<i32> {
        Ok(self.u32()? as i32)
    }
    /// A length-prefixed, NUL-terminated string, made safe to show: no control characters, at most 64 chars.
    fn text(&mut self) -> io::Result<String> {
        let n = self.u16()? as usize;
        let raw = self.take(n)?;
        let raw = raw.split(|&c| c == 0).next().unwrap_or_default();
        Ok(String::from_utf8_lossy(raw).chars().filter(|c| !c.is_control()).take(64).collect::<String>().trim().into())
    }
    fn skip_text(&mut self) -> io::Result<()> {
        let n = self.u16()? as usize;
        self.take(n).map(|_| ())
    }
    fn skip(&mut self, n: usize) -> io::Result<()> {
        self.take(n).map(|_| ())
    }
}

fn skip_mode(r: &mut Reader, version: u32) -> io::Result<()> {
    r.skip_text()?; // name
    r.skip(4)?; // value (removed in 6)
    r.skip(4 * 3)?; // flags, speed_min, speed_max
    if version >= 3 {
        r.skip(4 * 2)?; // brightness_min, brightness_max
    }
    r.skip(4 * 3)?; // colors_min, colors_max, speed
    if version >= 3 {
        r.skip(4)?; // brightness
    }
    r.skip(4 * 2)?; // direction, color_mode
    let colors = r.u16()? as usize;
    r.skip(4 * colors)
}

fn zone(r: &mut Reader, version: u32) -> io::Result<Zone> {
    let name = r.text()?;
    let kind = r.i32()?;
    r.skip(4 * 2)?; // leds_min, leds_max
    let leds = r.u32()?;
    let matrix_len = r.u16()? as usize;
    let matrix = if matrix_len >= 8 {
        let height = r.u32()?;
        let width = r.u32()?;
        let cells = (matrix_len - 8) / 4;
        if (height as usize).saturating_mul(width as usize) != cells || cells > MAX_LEDS {
            return Err(bad(format!("zone {name:?}: matrix map does not match its size")));
        }
        let map = (0..cells).map(|_| r.u32()).collect::<io::Result<Vec<u32>>>()?;
        r.skip(matrix_len - 8 - 4 * cells)?;
        Some(Matrix { height, width, map })
    } else {
        r.skip(matrix_len)?;
        None
    };
    if version >= 4 {
        for _ in 0..r.u16()? {
            r.skip_text()?; // segment name
            r.skip(4 * 3)?; // type, start, count
        }
    }
    if version >= 5 {
        r.skip(4)?; // zone flags
    }
    Ok(Zone { name, kind, leds, matrix })
}

/// Parse a controller data block (the reply payload, starting with its `data_size`), as sent for `version`.
pub fn parse_controller(data: &[u8], version: u32) -> io::Result<Controller> {
    let mut r = Reader { b: data, at: 0 };
    r.skip(4)?; // data_size
    let kind = r.i32()?;
    let name = r.text()?;
    let vendor = if version >= 1 { r.text()? } else { String::new() };
    for _ in 0..4 {
        r.skip_text()?; // description, version, serial (never kept), location
    }
    let modes = r.u16()?;
    r.skip(4)?; // active mode
    for _ in 0..modes {
        skip_mode(&mut r, version)?;
    }
    let zones = (0..r.u16()?).map(|_| zone(&mut r, version)).collect::<io::Result<Vec<Zone>>>()?;
    let n = r.u16()? as usize;
    if n > MAX_LEDS {
        return Err(bad(format!("{name:?} reports {n} LEDs, more than uncoil drives")));
    }
    let mut leds = Vec::with_capacity(n);
    for _ in 0..n {
        leds.push(r.text()?);
        r.skip(4)?; // value
    }
    let colors = r.u16()? as usize;
    r.skip(4 * colors)?;
    let mut flags = 0;
    if version >= 5 {
        for _ in 0..r.u16()? {
            r.skip_text()?; // alternative LED names
        }
        flags = r.u32()?;
    }
    Ok(Controller { kind, name, vendor, zones, leds, colors, flags })
}

/// `UpdateLEDs` payload: `data_size` (the whole payload, itself included), colour count, then each colour
/// as R, G, B, 0.
pub fn update_leds_payload(colors: &[[u8; 3]]) -> Vec<u8> {
    let n = colors.len().min(u16::MAX as usize);
    let size = (4 + 2 + 4 * n) as u32;
    let mut v = Vec::with_capacity(size as usize);
    v.extend_from_slice(&size.to_le_bytes());
    v.extend_from_slice(&(n as u16).to_le_bytes());
    for c in &colors[..n] {
        v.extend_from_slice(&[c[0], c[1], c[2], 0]);
    }
    v
}
