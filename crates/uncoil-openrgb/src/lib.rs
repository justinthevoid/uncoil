//! A small client for the OpenRGB SDK: enough to drive motherboard, RAM and GPU lighting live from uncoil.
//!
//! Written from the protocol facts in OpenRGB's `Documentation/OpenRGBSDK.md` and `NetworkProtocol.h`
//! (GPL-2.0-or-later; facts only, no code copied). std only, blocking `TcpStream`, and it only ever connects
//! to `127.0.0.1`: the API takes a port, never a host.
//!
//! Protocol in short: every packet is a 16-byte little-endian header (`"ORGB"`, device index, packet id,
//! payload size) and a payload. The client asks for the server's protocol version, and both use the lower of
//! the two. uncoil asks for at most [`MAX_VERSION`] (5): version 6 (OpenRGB 1.0) adds acknowledgements and
//! unique controller ids, which a lighting client does not need, and every OpenRGB server still answers a
//! version 5 client in the version 5 format.
//!
//! The server is untrusted input: while OpenRGB is not running, any local program could listen on its port.
//! Sizes and counts are capped ([`MAX_PACKET`], [`MAX_CONTROLLERS`], [`MAX_LEDS`]) and names lose control
//! characters.

mod proto;

pub use proto::{parse_controller, update_leds_payload, Controller, Matrix, Zone};

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// OpenRGB's default SDK port ("ORGB" on a phone keypad).
pub const DEFAULT_PORT: u16 = 6742;
/// The highest protocol version uncoil speaks.
pub const MAX_VERSION: u32 = 5;
/// OpenRGB's own packet limit (8 MiB).
pub const MAX_PACKET: u32 = 8 << 20;
/// Controllers taken from one server; more are ignored.
pub const MAX_CONTROLLERS: u32 = 64;
/// LEDs per controller; a controller with more is refused.
pub const MAX_LEDS: usize = 4096;

/// Packet ids (`NetworkProtocol.h`).
pub mod id {
    pub const REQUEST_CONTROLLER_COUNT: u32 = 0;
    pub const REQUEST_CONTROLLER_DATA: u32 = 1;
    pub const REQUEST_PROTOCOL_VERSION: u32 = 40;
    pub const SET_CLIENT_NAME: u32 = 50;
    /// Server push: the device list changed; indexes are no longer valid.
    pub const DEVICE_LIST_UPDATED: u32 = 100;
    pub const UPDATE_LEDS: u32 = 1050;
    pub const SET_CUSTOM_MODE: u32 = 1100;
}

/// OpenRGB device types (`RGBControllerInterface.h`), the ones uncoil sorts by.
pub mod device_type {
    pub const MOTHERBOARD: i32 = 0;
    pub const DRAM: i32 = 1;
    pub const GPU: i32 = 2;

    /// The type as the word `uncoil_core::owners` uses: "motherboard", "dram", "gpu" or "other".
    pub fn word(kind: i32) -> &'static str {
        match kind {
            MOTHERBOARD => "motherboard",
            DRAM => "dram",
            GPU => "gpu",
            _ => "other",
        }
    }
}

/// Controller flag (protocol 5): hidden in OpenRGB.
pub const CONTROLLER_FLAG_HIDDEN: u32 = 1 << 3;

const MAGIC: &[u8; 4] = b"ORGB";
/// How long a reply may take (OpenRGB answers from its own thread; detection can keep it busy).
const REPLY_WAIT: Duration = Duration::from_secs(3);
/// A version-0 server never answers the version request.
const VERSION_WAIT: Duration = Duration::from_secs(1);

fn bad(what: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.into())
}

/// One packet: header plus payload.
pub fn packet(dev: u32, id: u32, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(16 + payload.len());
    v.extend_from_slice(MAGIC);
    v.extend_from_slice(&dev.to_le_bytes());
    v.extend_from_slice(&id.to_le_bytes());
    v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    v.extend_from_slice(payload);
    v
}

/// `(device index, packet id, payload size)` from a header.
pub fn header(h: &[u8; 16]) -> io::Result<(u32, u32, u32)> {
    if &h[..4] != MAGIC {
        return Err(bad("not an OpenRGB SDK packet"));
    }
    let word = |i: usize| u32::from_le_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]]);
    let size = word(12);
    if size > MAX_PACKET {
        return Err(bad(format!("OpenRGB sent a {size}-byte packet, more than the protocol allows")));
    }
    Ok((word(4), word(8), size))
}

/// A connection to an OpenRGB SDK server on this PC.
pub struct Client {
    stream: TcpStream,
    version: u32,
    /// The server said the device list changed since the last [`Client::controllers`].
    changed: bool,
}

impl Client {
    /// Connect to `127.0.0.1:port`, agree on a protocol version and introduce ourselves as `name`.
    pub fn connect(port: u16, name: &str, timeout: Duration) -> io::Result<Client> {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let stream = TcpStream::connect_timeout(&addr, timeout)?;
        stream.set_nodelay(true)?;
        stream.set_write_timeout(Some(Duration::from_secs(1)))?;
        let mut c = Client { stream, version: 0, changed: false };
        c.send(0, id::REQUEST_PROTOCOL_VERSION, &MAX_VERSION.to_le_bytes())?;
        c.version = match c.reply_within(id::REQUEST_PROTOCOL_VERSION, VERSION_WAIT) {
            Ok(p) if p.len() >= 4 => u32::from_le_bytes([p[0], p[1], p[2], p[3]]).min(MAX_VERSION),
            Ok(_) => return Err(bad("OpenRGB sent a short protocol version")),
            // no answer at all: a version 0 server
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => 0,
            Err(e) => return Err(e),
        };
        let mut n = name.as_bytes().to_vec();
        n.push(0);
        c.send(0, id::SET_CLIENT_NAME, &n)?;
        Ok(c)
    }

    /// The protocol version both sides use.
    pub fn version(&self) -> u32 {
        self.version
    }

    fn send(&mut self, dev: u32, id: u32, payload: &[u8]) -> io::Result<()> {
        self.stream.write_all(&packet(dev, id, payload))
    }

    /// Read one whole packet (blocking, up to the stream's read timeout).
    fn read_packet(&mut self) -> io::Result<(u32, u32, Vec<u8>)> {
        let mut h = [0u8; 16];
        self.stream.read_exact(&mut h)?;
        let (dev, id, size) = header(&h)?;
        let mut payload = vec![0u8; size as usize];
        self.stream.read_exact(&mut payload)?;
        Ok((dev, id, payload))
    }

    /// Wait for a packet with this id; device-list notices that arrive meanwhile are remembered, anything
    /// else is skipped.
    fn reply_within(&mut self, want: u32, wait: Duration) -> io::Result<Vec<u8>> {
        let deadline = Instant::now() + wait;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "OpenRGB did not answer"));
            }
            self.stream.set_read_timeout(Some(left))?;
            let (_, id, payload) = self.read_packet()?;
            match id {
                i if i == want => return Ok(payload),
                id::DEVICE_LIST_UPDATED => self.changed = true,
                _ => {}
            }
        }
    }

    /// Every controller the server has, in its order (index = position). Clears the "changed" notice.
    pub fn controllers(&mut self) -> io::Result<Vec<Controller>> {
        self.changed = false;
        self.send(0, id::REQUEST_CONTROLLER_COUNT, &[])?;
        let p = self.reply_within(id::REQUEST_CONTROLLER_COUNT, REPLY_WAIT)?;
        let count =
            p.get(..4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| bad("short count"))?;
        let mut out = Vec::new();
        for i in 0..count.min(MAX_CONTROLLERS) {
            let ask = if self.version == 0 { Vec::new() } else { self.version.to_le_bytes().to_vec() };
            self.send(i, id::REQUEST_CONTROLLER_DATA, &ask)?;
            let data = self.reply_within(id::REQUEST_CONTROLLER_DATA, REPLY_WAIT)?;
            out.push(parse_controller(&data, self.version)?);
        }
        Ok(out)
    }

    /// Put a controller in its per-LED ("direct" / custom) mode.
    pub fn set_custom_mode(&mut self, index: u32) -> io::Result<()> {
        self.send(index, id::SET_CUSTOM_MODE, &[])
    }

    /// Set every LED colour of a controller. `colors` must have the controller's colour count.
    pub fn update_leds(&mut self, index: u32, colors: &[[u8; 3]]) -> io::Result<()> {
        self.send(index, id::UPDATE_LEDS, &update_leds_payload(colors))
    }

    /// Read whatever the server pushed, without waiting. `Ok(true)` when the device list changed since the
    /// last [`Client::controllers`] (indexes are stale: list them again); an error when the server went away.
    pub fn poll(&mut self) -> io::Result<bool> {
        loop {
            self.stream.set_nonblocking(true)?;
            let mut h = [0u8; 16];
            let peeked = self.stream.peek(&mut h);
            self.stream.set_nonblocking(false)?;
            match peeked {
                Ok(0) => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "OpenRGB closed the connection")),
                Ok(n) if n < 16 => return Ok(self.changed),
                Ok(_) => {
                    self.stream.set_read_timeout(Some(REPLY_WAIT))?;
                    let (_, id, _) = self.read_packet()?;
                    if id == id::DEVICE_LIST_UPDATED {
                        self.changed = true;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(self.changed),
                Err(e) => return Err(e),
            }
        }
    }
}

/// The detector names (first argument of each `REGISTER_HID_DETECTOR*` line in OpenRGB's
/// `Controllers/RazerController`, master `df3024be`, 2026-10-03), one per line. uncoil drives Razer devices
/// itself, so the OpenRGB it starts gets every one of these turned off.
pub fn razer_detectors() -> impl Iterator<Item = &'static str> {
    include_str!("razer-detectors.txt").lines().map(str::trim).filter(|l| !l.is_empty())
}

#[cfg(test)]
mod tests;
