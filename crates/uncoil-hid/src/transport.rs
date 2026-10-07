use anyhow::{Context, Result};
use hidapi::{HidApi, HidDevice};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ffi::{CStr, CString};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use uncoil_core::config::UnknownDevice;
use uncoil_core::device::{DeviceDef, UsbEndpoint};
use uncoil_core::proto::{self, DeviceMode, Reply, Report, Status, Transport, WIRE_LEN};

use crate::guard;

/// Razer's USB vendor id.
pub use uncoil_core::device::RAZER_VID;

/// An opened device. Devices with lighting receive frames; feature-only devices (no `lighting`, e.g. a
/// mouse without RGB) are opened for control commands only and never get a frame.
pub struct LiveDevice {
    pub def: Arc<DeviceDef>,
    pub endpoint: UsbEndpoint,
    pub path: CString,
    dev: HidDevice,
    /// Busy replies we retried, and reports that ended in an error status.
    pub busy_retries: u64,
    pub errors: u64,
}

/// A matching endpoint found on the bus.
pub struct Candidate {
    pub def: Arc<DeviceDef>,
    pub endpoint: UsbEndpoint,
    pub path: CString,
}

/// The USB interface a HID collection path belongs to. Windows gives each top-level collection of one
/// interface its own path: `\\?\hid#vid_1532&pid_00aa&mi_00&col01#8&1e3b5c3a&0&0000#{guid}` and
/// `…&col02#8&1e3b5c3a&0&0001#{guid}`. Dropping `&colNN` and the last part of the instance id leaves
/// what they share. Paths in another shape are their own key.
fn interface_key(path: &CStr) -> String {
    let p = path.to_string_lossy().to_ascii_lowercase();
    let parts: Vec<&str> = p.split('#').collect();
    if parts.len() < 3 {
        return p;
    }
    let ids = match parts[1].find("&col") {
        Some(i) => &parts[1][..i],
        None => parts[1],
    };
    let instance = parts[2].rsplit_once('&').map_or(parts[2], |(head, _)| head);
    format!("{ids}#{instance}")
}

/// Find every known device endpoint currently attached, skipping paths already open. A device matches on
/// its interface plus any accepted (usage page, usage); when several collections of one interface match,
/// only the best-ranked one is used, and none when one of them is already open.
pub fn discover(api: &mut HidApi, defs: &[Arc<DeviceDef>], skip: &HashSet<CString>) -> Vec<Candidate> {
    let _ = api.refresh_devices();
    // (device id, interface key) -> (rank, candidate)
    let mut best: BTreeMap<String, (usize, Candidate)> = BTreeMap::new();
    let mut busy: HashSet<String> = HashSet::new();
    for info in api.device_list() {
        for def in defs {
            if info.vendor_id() != def.vendor_id {
                continue;
            }
            let Some(ep) = def.endpoint_for(info.product_id()) else { continue };
            let Some(rank) = ep.accepts(info.interface_number(), info.usage_page(), info.usage()) else { continue };
            let key = format!("{}|{}", def.id, interface_key(info.path()));
            if skip.contains(info.path()) {
                busy.insert(key);
                continue;
            }
            if best.get(&key).is_none_or(|(r, _)| rank < *r) {
                let c = Candidate { def: def.clone(), endpoint: ep.clone(), path: info.path().to_owned() };
                best.insert(key, (rank, c));
            }
        }
    }
    best.into_iter().filter(|(k, _)| !busy.contains(k)).map(|(_, (_, c))| c).collect()
}

/// Razer devices (vendor 0x1532) on the bus that no definition knows, with their interface numbers. Call
/// after [`discover`] (which refreshes the device list).
pub fn unknown_devices(api: &HidApi, defs: &[Arc<DeviceDef>]) -> Vec<UnknownDevice> {
    let mut found: BTreeMap<u16, BTreeSet<u8>> = BTreeMap::new();
    for info in api.device_list() {
        if info.vendor_id() != RAZER_VID {
            continue;
        }
        let pid = info.product_id();
        if defs.iter().any(|d| d.vendor_id == RAZER_VID && d.endpoint_for(pid).is_some()) {
            continue;
        }
        let set = found.entry(pid).or_default();
        if let Ok(i) = u8::try_from(info.interface_number()) {
            set.insert(i);
        }
    }
    found.into_iter().map(|(product_id, i)| UnknownDevice { product_id, interfaces: i.into_iter().collect() }).collect()
}

impl LiveDevice {
    /// Open and prepare (see [`LiveDevice::prepare`]).
    /// Returns Ok(None) when the endpoint exists but nothing answers (e.g. a dongle whose mouse is
    /// on its cable).
    pub fn open(api: &HidApi, c: Candidate) -> Result<Option<LiveDevice>> {
        let dev = api.open_path(&c.path).with_context(|| format!("open {}", c.def.name))?;
        let mut d = LiveDevice { def: c.def, endpoint: c.endpoint, path: c.path, dev, busy_retries: 0, errors: 0 };
        Ok(match d.prepare()? {
            Status::Ok => Some(d),
            _ => None,
        })
    }

    /// Devices that stream frames: normal (firmware) mode, then the custom-frame effect once. Safe to
    /// repeat, e.g. after the PC wakes and the device may have reset to its onboard lighting.
    ///
    /// Feature-only devices: read the device mode (`00/84`) and only set normal mode when it is not normal
    /// already, so an untried device gets no write it does not need.
    pub fn prepare(&mut self) -> Result<Status> {
        let tid = self.tid();
        if !self.def.streams_frames() {
            let reply = self.query(&proto::get_device_mode(tid))?;
            if reply.status == Status::Ok && reply.raw[0] != DeviceMode::Normal as u8 {
                self.ask(&proto::set_device_mode(tid, DeviceMode::Normal))?;
            }
            return Ok(reply.status);
        }
        let st = self.ask(&proto::set_device_mode(tid, DeviceMode::Normal))?;
        if st == Status::Ok {
            self.ask(&proto::effect_custom_frame(tid))?;
        }
        Ok(st)
    }

    /// The endpoint's default transaction id. Every report goes out with the id its command group needs
    /// (`[usb.transaction_ids]`), whatever id it was built with.
    pub fn tid(&self) -> u8 {
        self.endpoint.transaction_id
    }

    /// Pause before reading a reply: the endpoint's `reply_wait_us`, else `default`.
    fn reply_wait(&self, default: Duration) -> Duration {
        self.endpoint.reply_wait_us.map_or(default, |us| Duration::from_micros(us as u64))
    }

    /// Send a report and read the device's reply status (under the Razer device lock).
    pub fn ask(&mut self, r: &Report) -> Result<Status> {
        let def = self.def.clone();
        guard::razer().for_command(&def.name, || self.ask_unlocked(r))
    }

    fn ask_unlocked(&mut self, r: &Report) -> Result<Status> {
        self.dev.send_feature_report(&self.endpoint.wire(r))?;
        sleep(self.reply_wait(Duration::from_millis(2)));
        let mut buf = [0u8; WIRE_LEN];
        self.dev.get_feature_report(&mut buf)?;
        Ok(proto::reply_status(&buf))
    }

    /// Send a report the way this device needs it: fire-and-forget, or acknowledged with busy-retry. The
    /// caller holds the Razer device lock.
    fn send(&mut self, r: &Report) -> Result<()> {
        let wire = self.endpoint.wire(r);
        if !self.def.quirks.ack_every_report {
            self.dev.send_feature_report(&wire)?;
            return Ok(());
        }
        let mut buf = [0u8; WIRE_LEN];
        for _ in 0..4 {
            self.dev.send_feature_report(&wire)?;
            sleep(self.reply_wait(Duration::from_millis(1)));
            self.dev.get_feature_report(&mut buf)?;
            match proto::reply_status(&buf) {
                Status::Busy => {
                    self.busy_retries += 1;
                    sleep(Duration::from_millis(2));
                }
                Status::Ok => return Ok(()),
                _ => {
                    self.errors += 1;
                    return Ok(());
                }
            }
        }
        self.errors += 1;
        Ok(())
    }

    /// Upload one full frame, joining the process's turn on the Razer device lock for each report (see
    /// [`guard`]): uncoil's device threads share a turn and send in parallel, and between 20 ms turns the
    /// lock thread lets go of the mutex so OpenRGB gets in, even partway through a keyboard frame.
    /// `color(row, col)` returns the colour for that matrix slot. `Ok(false)`: the frame was not sent in
    /// full, because the device does not stream frames or no turn came in time (the rest of the frame is
    /// skipped; the next one comes soon).
    pub fn send_frame(&mut self, mut color: impl FnMut(usize, usize) -> [u8; 3]) -> Result<bool> {
        let (rows, cols) = match &self.def.matrix {
            Some(m) if self.def.streams_frames() => (m.rows, m.cols),
            _ => return Ok(false),
        };
        let tid = self.tid();
        let mut row_buf = Vec::with_capacity(cols);
        for r in 0..rows {
            row_buf.clear();
            row_buf.extend((0..cols).map(|c| color(r, c)));
            let rep = proto::custom_frame_row(tid, r as u8, 0, &row_buf)?;
            if guard::razer().for_frame(|| self.send(&rep))?.is_none() {
                return Ok(false);
            }
        }
        if !self.def.quirks.custom_mode_once
            && guard::razer().for_frame(|| self.send(&proto::effect_custom_frame(tid)))?.is_none()
        {
            return Ok(false);
        }
        Ok(true)
    }

    /// Send a command and return the device's matching reply (feature commands: key maps, OLED, …).
    ///
    /// Busy / not-yet-processed replies and replies to a different command are re-read with growing pauses;
    /// after six reads the request is sent again (all commands uncoil sends are idempotent). Within uncoild
    /// only the device's own thread calls this, between frames. The whole exchange runs under the Razer
    /// device lock, so another program's reports cannot land between the request and its reply.
    pub fn query(&mut self, r: &Report) -> Result<Reply> {
        let def = self.def.clone();
        guard::razer().for_command(&def.name, || self.query_unlocked(r))
    }

    fn query_unlocked(&mut self, r: &Report) -> Result<Reply> {
        let wire = self.endpoint.wire(r);
        let mut buf = [0u8; WIRE_LEN];
        let mut last = None;
        let first = self.reply_wait(Duration::from_millis(2));
        for attempt in 0..4u64 {
            self.dev.send_feature_report(&wire)?;
            for read in 0..6u64 {
                sleep(first + Duration::from_millis(2 * read + 4 * attempt));
                self.dev.get_feature_report(&mut buf)?;
                let reply = Reply::parse(&buf).context("short reply")?;
                if !reply.answers(r) {
                    // still the previous command's reply (or another program's): read again, then resend
                    last = Some(reply);
                    continue;
                }
                // busy, or not processed yet (status still "new"): read again
                if matches!(reply.status, Status::Busy | Status::New) {
                    self.busy_retries += 1;
                    last = Some(reply);
                    continue;
                }
                return Ok(reply);
            }
        }
        match last {
            Some(reply) if reply.answers(r) => Ok(reply),
            _ => anyhow::bail!("no reply to {:02X}/{:02X} from {}", r.class, r.id, self.def.name),
        }
    }
}

impl Transport for LiveDevice {
    fn query(&mut self, request: &Report) -> anyhow::Result<Reply> {
        LiveDevice::query(self, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collections_of_one_interface_share_a_key() {
        let col1 = CString::new(r"\\?\HID#VID_1532&PID_00AA&MI_00&Col01#8&1e3b5c3a&0&0000#{4d1e55b2-f16f}").unwrap();
        let col2 = CString::new(r"\\?\hid#vid_1532&pid_00aa&mi_00&col02#8&1e3b5c3a&0&0001#{4d1e55b2-f16f}").unwrap();
        let other = CString::new(r"\\?\hid#vid_1532&pid_00aa&mi_01#8&22222222&0&0000#{4d1e55b2-f16f}").unwrap();
        assert_eq!(interface_key(&col1), interface_key(&col2));
        assert_eq!(interface_key(&col1), "vid_1532&pid_00aa&mi_00#8&1e3b5c3a&0");
        assert_ne!(interface_key(&col1), interface_key(&other));
        let odd = CString::new("some-other-path").unwrap();
        assert_eq!(interface_key(&odd), "some-other-path");
    }
}
