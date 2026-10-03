use anyhow::{Context, Result};
use hidapi::{HidApi, HidDevice};
use std::collections::HashSet;
use std::ffi::CString;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use uncoil_core::device::{DeviceDef, UsbEndpoint};
use uncoil_core::proto::{self, DeviceMode, Report, Status, WIRE_LEN};

/// An opened device, ready to receive frames.
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

/// Find every known device endpoint currently attached, skipping paths already open.
pub fn discover(api: &mut HidApi, defs: &[Arc<DeviceDef>], skip: &HashSet<CString>) -> Vec<Candidate> {
    let _ = api.refresh_devices();
    let mut out = Vec::new();
    for info in api.device_list() {
        for def in defs {
            if info.vendor_id() != def.vendor_id {
                continue;
            }
            let Some(ep) = def.endpoint_for(info.product_id()) else { continue };
            if info.interface_number() == ep.interface
                && info.usage_page() == ep.usage_page
                && info.usage() == ep.usage
                && !skip.contains(info.path())
            {
                out.push(Candidate { def: def.clone(), endpoint: ep.clone(), path: info.path().to_owned() });
            }
        }
    }
    out
}

impl LiveDevice {
    /// Open and prepare: normal (firmware) mode, then the custom-frame effect once.
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

    /// Normal (firmware) mode, then the custom-frame effect once. Safe to repeat, e.g. after the PC
    /// wakes and the device may have reset to its onboard lighting.
    pub fn prepare(&mut self) -> Result<Status> {
        let tid = self.tid();
        let st = self.ask(&proto::set_device_mode(tid, DeviceMode::Normal))?;
        if st == Status::Ok {
            self.ask(&proto::effect_custom_frame(tid))?;
        }
        Ok(st)
    }

    pub fn tid(&self) -> u8 {
        self.endpoint.transaction_id
    }

    /// Send a report and read the device's reply status.
    pub fn ask(&mut self, r: &Report) -> Result<Status> {
        self.dev.send_feature_report(&r.to_wire())?;
        sleep(Duration::from_millis(2));
        let mut buf = [0u8; WIRE_LEN];
        self.dev.get_feature_report(&mut buf)?;
        Ok(proto::reply_status(&buf))
    }

    /// Send a report the way this device needs it: fire-and-forget, or acknowledged with busy-retry.
    pub fn send(&mut self, r: &Report) -> Result<()> {
        let wire = r.to_wire();
        if !self.def.quirks.ack_every_report {
            self.dev.send_feature_report(&wire)?;
            return Ok(());
        }
        let mut buf = [0u8; WIRE_LEN];
        for _ in 0..4 {
            self.dev.send_feature_report(&wire)?;
            sleep(Duration::from_millis(1));
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

    /// Upload one full frame. `color(row, col)` returns the colour for that matrix slot.
    pub fn send_frame(&mut self, mut color: impl FnMut(usize, usize) -> [u8; 3]) -> Result<()> {
        let (rows, cols) = (self.def.matrix.rows, self.def.matrix.cols);
        let tid = self.tid();
        let mut row_buf = Vec::with_capacity(cols);
        for r in 0..rows {
            row_buf.clear();
            row_buf.extend((0..cols).map(|c| color(r, c)));
            let rep = proto::custom_frame_row(tid, r as u8, 0, &row_buf);
            self.send(&rep)?;
        }
        if !self.def.quirks.custom_mode_once {
            self.send(&proto::effect_custom_frame(tid))?;
        }
        Ok(())
    }

    /// Put the device back in firmware mode (best effort; used on shutdown).
    pub fn release(&mut self) {
        let _ = self.ask(&proto::set_device_mode(self.tid(), DeviceMode::Normal));
    }
}
