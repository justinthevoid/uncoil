//! Runs control-pipe commands against one device. Generic over [`Transport`], so the same code drives the
//! real HID device (from its renderer thread, between frames) and the fake device used by tests.
//!
//! Every command that writes onboard memory is refused unless the request says `write: true`; when it runs,
//! the value is read before and after, the write is logged, and it is journalled
//! (`%LOCALAPPDATA%\uncoil\onboard-writes.jsonl`): a `pending` entry with the value from before goes in
//! before anything is sent, a `done` entry after the read-back, and a `failed` entry (with what was already
//! applied) when a write of several reports stops part-way. `keymap.reset` restores what was there before
//! uncoil first touched a key.
//!
//! Every report sent while reading (gets, dumps, checks, probes) goes through `proto::query_read`, which
//! refuses anything but a "get" command.
//!
//! On experimental devices (and a supported device's `unverified` features) every write first passes the
//! feature's read-only check (`checks.rs`). On every mouse, a key map change that would leave no button
//! producing left click is refused (`left_click_guard`).

use crate::checks::Checks;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use uncoil_core::device::{DeviceDef, Kind};
use uncoil_core::features::dial::{self, DialMode};
use uncoil_core::features::hw_effect::{self, HwEffect, Storage};
use uncoil_core::features::keymap::{self, Function, KeyDef, KeymapDef, Layer};
use uncoil_core::features::oled::{self, OledState};
use uncoil_core::features::performance::{self as perf, DpiStorage, PerformanceState};
use uncoil_core::features::power::{self, PowerState};
use uncoil_core::features::profile::{self, ProfileInfo};
use uncoil_core::features::Feature;
use uncoil_core::ipc::{
    self, coded, codes, Command, EffectState, KeyMapping, LightingProbe, PerformanceSetArgs, PowerSetArgs, Raw,
    WriteResult,
};
use uncoil_core::proto::{self, query_ok, query_read, DeviceMode, Report, Transport};

/// Profiles a request may name (onboard profiles are numbered from 1; Razer devices keep at most 5).
pub const MAX_PROFILES: u8 = 5;

/// What the renderer must do with the lighting after a command.
#[derive(Debug, Clone, PartialEq)]
pub enum Lighting {
    Unchanged,
    /// A firmware effect is now showing: stop streaming frames.
    Hardware(HwEffect),
    /// Back to the software effect: frames resume (the custom-frame effect was re-sent).
    Software,
}

#[derive(Debug)]
pub struct Outcome {
    pub result: Raw,
    pub lighting: Lighting,
    /// Lines for uncoild.log (every onboard write is logged here).
    pub log: Vec<String>,
}

impl Outcome {
    fn value(result: Raw) -> Outcome {
        Outcome { result, lighting: Lighting::Unchanged, log: vec![] }
    }
}

/// Append-only record of onboard writes.
pub struct Journal {
    pub path: Option<PathBuf>,
}

/// Where an onboard write got to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum State {
    /// About to send; `after` is what was asked for.
    Pending,
    /// Sent and read back; `after` is what the device now holds. (Entries from before states existed.)
    #[default]
    Done,
    /// Stopped part-way; `after` says what was applied.
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JournalEntry {
    t: u64,
    device: String,
    cmd: String,
    #[serde(default)]
    state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profile: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layer: Option<Layer>,
    before: String,
    after: String,
}

impl Journal {
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join("uncoil").join("onboard-writes.jsonl")
    }

    /// Append `e` with `state`. A journal that cannot be written is logged, not fatal.
    fn record(&self, e: &JournalEntry, state: State) {
        let Some(p) = &self.path else { return };
        let line = serde_json::to_string(&JournalEntry { state, ..e.clone() }).unwrap_or_default();
        let res = crate::winsec::open_user_file(p, true).and_then(|mut f| writeln!(f, "{line}"));
        if let Err(err) = res {
            crate::log::line(&format!("journal {} not written ({err}): {line}", p.display()));
        }
    }

    /// The mapping a key had before uncoil's first write to it, if uncoil ever wrote it.
    fn original(&self, device: &str, profile: u8, key: u8, layer: Layer) -> Option<String> {
        let f = std::fs::File::open(self.path.as_ref()?).ok()?;
        std::io::BufReader::new(f).lines().map_while(|l| l.ok()).find_map(|l| {
            let e: JournalEntry = serde_json::from_str(&l).ok()?;
            (e.device == device
                && e.cmd == "keymap"
                && e.profile == Some(profile)
                && e.key == Some(key)
                && e.layer == Some(layer))
            .then_some(e.before)
        })
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn to_json<T: Serialize>(t: &T) -> Raw {
    ipc::raw(t)
}

fn not_supported(def: &DeviceDef, what: &str) -> anyhow::Error {
    coded(codes::NOT_SUPPORTED, format!("{} does not support {what}", def.name))
}

/// The onboard profile a command names, if it names one.
fn profile_of(cmd: &Command) -> Option<u8> {
    Some(match cmd {
        Command::KeymapGet(a) => a.profile,
        Command::KeymapSet(a) => a.profile,
        Command::KeymapReset(a) => a.profile,
        Command::KeymapDump(a) => a.profile,
        Command::DialGet(a) => a.profile,
        Command::DialSet(a) => a.profile,
        _ => return None,
    })
}

/// A journal entry for `def` (fields the command does not use stay empty).
fn entry(def: &DeviceDef, cmd: &str, before: String, after: String) -> JournalEntry {
    JournalEntry {
        t: unix_now(),
        device: def.id.clone(),
        cmd: cmd.into(),
        state: State::Pending,
        profile: None,
        key: None,
        layer: None,
        before,
        after,
    }
}

/// Features whose read-only check must pass before this command may write.
fn gated_features(cmd: &Command) -> Vec<Feature> {
    match cmd {
        Command::KeymapSet(_) | Command::KeymapReset(_) => vec![Feature::Keymap],
        Command::DialSet(_) => vec![Feature::Dial],
        Command::OledSet(_) => vec![Feature::Oled],
        Command::PerformanceSet(a) => {
            let mut v = vec![];
            if a.dpi.is_some() || a.stages.is_some() {
                v.push(Feature::Dpi);
            }
            if a.poll_hz.is_some() {
                v.push(Feature::PollRate);
            }
            v
        }
        Command::PowerSet(_) => vec![Feature::Power],
        // a session effect is never refused; saving one to the device is an onboard write like any other
        Command::EffectHw(a) if a.storage == Storage::Onboard => vec![Feature::HwEffects],
        _ => vec![],
    }
}

/// Run one device-level command. `checks` is the device's check cache for this connection.
pub fn run(
    t: &mut dyn Transport,
    def: &DeviceDef,
    tid: u8,
    cmd: &Command,
    journal: &Journal,
    checks: &mut Checks,
) -> Result<Outcome> {
    if let Some(f) = cmd.feature() {
        if !def.has(f) {
            return Err(not_supported(def, f.as_str()));
        }
    }
    match cmd {
        Command::PerformanceGet if !def.has(Feature::Dpi) && !def.has(Feature::PollRate) => {
            return Err(not_supported(def, "dpi or poll_rate"));
        }
        Command::PerformanceSet(a) => {
            if (a.dpi.is_some() || a.stages.is_some()) && !def.has(Feature::Dpi) {
                return Err(not_supported(def, "dpi"));
            }
            if a.poll_hz.is_some() && !def.has(Feature::PollRate) {
                return Err(not_supported(def, "poll_rate"));
            }
            if a.stages.is_some() && def.dpi.as_ref().is_some_and(|d| d.stages_max == 0) {
                return Err(not_supported(def, "DPI stages"));
            }
        }
        _ => {}
    }
    let stored_dpi = matches!(cmd, Command::PerformanceSet(a)
        if a.dpi.is_some() && def.dpi.as_ref().is_some_and(|d| d.storage == DpiStorage::Varstore));
    if (cmd.writes_onboard() || stored_dpi) && !cmd.write_confirmed() {
        bail!("this writes {}'s onboard memory; repeat with write=true (CLI: --write)", def.name);
    }
    if let Some(p) = profile_of(cmd) {
        if !(1..=MAX_PROFILES).contains(&p) {
            bail!("profile {p} does not exist (profiles are 1 to {MAX_PROFILES})");
        }
    }
    for f in gated_features(cmd) {
        checks.require(t, def, tid, f)?;
    }
    match cmd {
        Command::KeymapGet(a) => {
            let (km, key) = key_of(def, &a.key, a.layer)?;
            Ok(Outcome::value(to_json(&read_key(t, tid, km, key, a.profile, a.layer)?)))
        }
        Command::KeymapDump(a) => {
            let km = keymap_of(def)?;
            check_layer(km, a.layer)?;
            let rows: Vec<KeyMapping> =
                km.keys.iter().map(|k| read_key(t, tid, km, k, a.profile, a.layer)).collect::<Result<_>>()?;
            Ok(Outcome::value(to_json(&rows)))
        }
        Command::KeymapSet(a) => {
            let (km, key) = key_of(def, &a.key, a.layer)?;
            write_key(t, tid, def, km, key, a.profile, a.layer, &a.function, journal, "keymap.set")
        }
        Command::KeymapReset(a) => {
            let (km, key) = key_of(def, &a.key, a.layer)?;
            let target = match journal.original(&def.id, a.profile, key.id, a.layer) {
                Some(spec) => Function::parse_spec(&spec).with_context(|| format!("journal entry `{spec}`"))?,
                None => key.default_function().ok_or_else(|| {
                    anyhow!("no recorded original and no default for {} in the device definition", key.name)
                })?,
            };
            write_key(t, tid, def, km, key, a.profile, a.layer, &target, journal, "keymap.reset")
        }
        Command::ProfileList => {
            let max = profile::parse_byte(&query_read(t, &profile::get_max(tid))?);
            let count = profile::parse_byte(&query_read(t, &profile::get_count(tid))?);
            let ids = profile::parse_ids(&query_read(t, &profile::get_ids(tid))?);
            let active = query_read(t, &profile::get_active(tid)).ok().map(|r| profile::parse_byte(&r));
            Ok(Outcome::value(to_json(&ProfileInfo { max, count, ids, active })))
        }
        Command::DialGet(a) => {
            Ok(Outcome::value(to_json(&dial::parse_state(&query_read(t, &dial::get_active_mode(tid, a.profile))?))))
        }
        Command::DialSet(a) => {
            let enabled = a.enabled.clone().unwrap_or_else(|| DialMode::DEFAULT_ENABLED.to_vec());
            let report = dial::set_active_mode(tid, a.profile, a.mode, &enabled)?;
            let before = dial::parse_state(&query_read(t, &dial::get_active_mode(tid, a.profile))?);
            if before.mode == Some(a.mode) {
                return Ok(Outcome::value(to_json(&WriteResult {
                    after: before.clone(),
                    before,
                    verified: true,
                    unchanged: true,
                })));
            }
            let mut e = entry(def, "dial", name_of_mode(before.mode_id), a.mode.name().into());
            e.profile = Some(a.profile);
            journal.record(&e, State::Pending);
            query_ok(t, &report)?;
            let after = dial::parse_state(&query_read(t, &dial::get_active_mode(tid, a.profile))?);
            let verified = after.mode == Some(a.mode);
            let line = format!(
                "ONBOARD WRITE {}: dial mode profile {} {} -> {} (verified {verified})",
                def.id,
                a.profile,
                name_of_mode(before.mode_id),
                a.mode.name()
            );
            journal.record(&JournalEntry { after: name_of_mode(after.mode_id), ..e }, State::Done);
            Ok(Outcome {
                result: to_json(&WriteResult { before, after, verified, unchanged: false }),
                lighting: Lighting::Unchanged,
                log: vec![line],
            })
        }
        Command::OledGet => Ok(Outcome::value(to_json(&read_oled(t, tid)?))),
        Command::OledSet(a) => {
            let pct = a.brightness.ok_or_else(|| anyhow!("nothing to set (supported: brightness)"))?.min(100);
            let read =
                |t: &mut dyn Transport| -> Result<u8> { Ok(query_read(t, &oled::get(tid, oled::BRIGHTNESS))?.raw[0]) };
            let before = read(t)?;
            if before == pct {
                return Ok(Outcome::value(to_json(&WriteResult {
                    before,
                    after: before,
                    verified: true,
                    unchanged: true,
                })));
            }
            let e = entry(def, "oled.brightness", before.to_string(), pct.to_string());
            journal.record(&e, State::Pending);
            query_ok(t, &oled::set_brightness(tid, pct))?;
            let after = read(t)?;
            let verified = after == pct;
            journal.record(&JournalEntry { after: after.to_string(), ..e }, State::Done);
            let line = format!("ONBOARD WRITE {}: OLED brightness {before}% -> {after}% (verified {verified})", def.id);
            Ok(Outcome {
                result: to_json(&WriteResult { before, after, verified, unchanged: false }),
                lighting: Lighting::Unchanged,
                log: vec![line],
            })
        }
        Command::EffectHw(a) => {
            let hw = def.hw_effects.as_ref().ok_or_else(|| anyhow!("{} has no firmware effects", def.name))?;
            if !hw.effects.iter().any(|e| e == a.effect.name()) {
                bail!("{} does not run the {} effect (it runs: {})", def.name, a.effect.name(), hw.effects.join(", "));
            }
            let report = a.effect.report(tid, a.storage, hw.led)?;
            let mut log = vec![];
            if a.storage == Storage::Onboard {
                // the device has no getter for its saved effect, so there is no before-state to record
                let e = entry(def, "effect", String::new(), serde_json::to_string(&a.effect).unwrap_or_default());
                journal.record(&e, State::Pending);
                query_ok(t, &report)?;
                journal.record(&e, State::Done);
                log.push(format!("ONBOARD WRITE {}: firmware effect {} saved to the device", def.id, a.effect.name()));
            } else {
                query_ok(t, &report)?;
            }
            Ok(Outcome {
                result: to_json(&EffectState { effect: Some(a.effect.clone()), storage: a.storage }),
                lighting: Lighting::Hardware(a.effect.clone()),
                log,
            })
        }
        Command::EffectSoftware => {
            query_ok(t, &proto::set_device_mode(tid, DeviceMode::Normal))?;
            query_ok(t, &proto::effect_custom_frame(tid))?;
            Ok(Outcome {
                result: to_json(&EffectState { effect: None, storage: Storage::Session }),
                lighting: Lighting::Software,
                log: vec![],
            })
        }
        Command::CheckRun => Ok(Outcome::value(to_json(&checks.run_all(t, def, tid)))),
        Command::PerformanceGet => Ok(Outcome::value(to_json(&read_performance(t, def, tid)))),
        Command::PerformanceSet(a) => set_performance(t, def, tid, a, journal),
        Command::PowerGet => Ok(Outcome::value(to_json(&read_power(t, def, tid)))),
        Command::PowerSet(a) => set_power(t, def, tid, a, journal),
        Command::Status | Command::Devices | Command::Capabilities(_) => bail!("{:?} is answered by the daemon", cmd),
    }
}

/// Everything readable about DPI and poll rate; `None` where the device lacks it or did not answer.
fn read_performance(t: &mut dyn Transport, def: &DeviceDef, tid: u8) -> PerformanceState {
    let mut s = PerformanceState::default();
    if let (true, Some(d)) = (def.has(Feature::Dpi), &def.dpi) {
        s.dpi = query_read(t, &perf::get_dpi(tid, d.storage)).and_then(|r| perf::parse_dpi(&r)).ok();
        s.dpi_min = Some(d.min);
        s.dpi_max = Some(d.max);
        s.stages_max = d.stages_max;
        if d.stages_max > 0 {
            s.stages = query_read(t, &perf::get_stages(tid)).and_then(|r| perf::parse_stages(&r)).ok();
        }
    }
    if let (true, Some(p)) = (def.has(Feature::PollRate), &def.poll_rate) {
        s.poll_hz = query_read(t, &perf::get_poll(tid, p.kind)).and_then(|r| perf::parse_poll(&r, p.kind)).ok();
        s.poll_rates = p.rates.clone();
    }
    s
}

fn set_performance(
    t: &mut dyn Transport,
    def: &DeviceDef,
    tid: u8,
    a: &PerformanceSetArgs,
    journal: &Journal,
) -> Result<Outcome> {
    if a.dpi.is_none() && a.stages.is_none() && a.poll_hz.is_none() {
        bail!("nothing to set (dpi, stages, poll_hz)");
    }
    // targets, clamped to what the device file allows
    let dpi_def = def.dpi.as_ref();
    let dpi = match (a.dpi, dpi_def) {
        (Some(d), Some(dd)) => Some(d.clamp(dd.min, dd.max)),
        _ => None,
    };
    let stages = match (&a.stages, dpi_def) {
        (Some(s), Some(dd)) => {
            if s.list.len() > dd.stages_max as usize {
                bail!("{} keeps at most {} DPI stages, not {}", def.name, dd.stages_max, s.list.len());
            }
            let s = s.clamp(dd.min, dd.max);
            perf::set_stages(tid, &s)?; // validates count and active stage
            Some(s)
        }
        _ => None,
    };
    let poll = match (a.poll_hz, def.poll_rate.as_ref()) {
        (Some(hz), Some(p)) => {
            if !p.rates.contains(&hz) {
                let rates: Vec<String> = p.rates.iter().map(u16::to_string).collect();
                bail!("{} runs at {} Hz, not {hz} Hz", def.name, rates.join(", "));
            }
            Some((hz, p))
        }
        _ => None,
    };
    let dpi_report = |d| perf::set_dpi(tid, dpi_def.map_or(DpiStorage::Nostore, |x| x.storage), d);

    // DPI alone: live, like pressing the DPI button (unless the device stores it)
    let stored_dpi = dpi_def.is_some_and(|d| d.storage == DpiStorage::Varstore);
    if stages.is_none() && poll.is_none() && !stored_dpi {
        if let Some(d) = dpi {
            query_ok(t, &dpi_report(d))?;
        }
        return Ok(Outcome::value(to_json(&read_performance(t, def, tid))));
    }

    let before = read_performance(t, def, tid);
    let wants = |s: &PerformanceState| {
        dpi.is_none_or(|d| s.dpi == Some(d))
            && stages.as_ref().is_none_or(|x| s.stages.as_ref() == Some(x))
            && poll.is_none_or(|(hz, _)| s.poll_hz == Some(hz))
    };
    if wants(&before) {
        let r = WriteResult { after: before.clone(), before, verified: true, unchanged: true };
        return Ok(Outcome::value(to_json(&r)));
    }
    let mut steps: Vec<(String, Vec<Report>)> = vec![];
    if let Some(d) = dpi {
        steps.push((format!("DPI {d}"), vec![dpi_report(d)]));
    }
    if let Some(s) = &stages {
        let l: Vec<String> = s.list.iter().map(|d| d.to_string()).collect();
        steps.push((format!("DPI stages [{}] active {}", l.join(", "), s.active), vec![perf::set_stages(tid, s)?]));
    }
    if let Some((hz, p)) = poll {
        let from = before.poll_hz.map_or("?".into(), |h| h.to_string());
        steps.push((format!("poll rate {from} -> {hz} Hz"), perf::set_poll(tid, p.kind, hz, p.set_twice)?));
    }
    let show = |s: &PerformanceState| serde_json::to_string(s).unwrap_or_default();
    let parts = apply_steps(t, journal, entry(def, "performance", show(&before), String::new()), steps)?;
    let after = read_performance(t, def, tid);
    let verified = wants(&after);
    journal.record(&entry(def, "performance", show(&before), show(&after)), State::Done);
    let line = format!("ONBOARD WRITE {}: {} (verified {verified})", def.id, parts.join(", "));
    Ok(Outcome {
        result: to_json(&WriteResult { before, after, verified, unchanged: false }),
        lighting: Lighting::Unchanged,
        log: vec![line],
    })
}

fn read_power(t: &mut dyn Transport, def: &DeviceDef, tid: u8) -> PowerState {
    let mut s = PowerState::default();
    let Some(p) = &def.power else { return s };
    if p.battery {
        s.battery_pct = query_read(t, &power::get_battery(tid)).ok().map(|r| power::parse_battery(&r));
        s.charging = query_read(t, &power::get_charging(tid)).and_then(|r| power::parse_charging(&r)).ok();
    }
    if p.idle {
        s.idle_s = query_read(t, &power::get_idle(tid)).and_then(|r| power::parse_idle(&r)).ok();
        s.idle_range = Some((power::IDLE_MIN, power::IDLE_MAX));
    }
    if p.low_battery {
        s.low_battery_pct = query_read(t, &power::get_low_battery(tid))
            .and_then(|r| power::parse_low_battery(&r))
            .ok()
            .map(power::raw_to_pct);
        s.low_battery_range = Some(power::low_battery_range_pct());
    }
    s
}

fn set_power(t: &mut dyn Transport, def: &DeviceDef, tid: u8, a: &PowerSetArgs, journal: &Journal) -> Result<Outcome> {
    let p = def.power.clone().unwrap_or_default();
    if a.idle_s.is_none() && a.low_battery_pct.is_none() {
        bail!("nothing to set (idle_s, low_battery_pct)");
    }
    if a.idle_s.is_some() && !p.idle {
        return Err(not_supported(def, "a sleep timer"));
    }
    if a.low_battery_pct.is_some() && !p.low_battery {
        return Err(not_supported(def, "a low-battery threshold"));
    }
    let idle = a.idle_s.map(|s| s.clamp(power::IDLE_MIN, power::IDLE_MAX));
    let low_raw =
        a.low_battery_pct.map(|pct| power::pct_to_raw(pct).clamp(power::LOW_BATTERY_MIN, power::LOW_BATTERY_MAX));
    let wants = |s: &PowerState| {
        idle.is_none_or(|v| s.idle_s == Some(v))
            && low_raw.is_none_or(|r| s.low_battery_pct == Some(power::raw_to_pct(r)))
    };
    let before = read_power(t, def, tid);
    if wants(&before) {
        let r = WriteResult { after: before.clone(), before, verified: true, unchanged: true };
        return Ok(Outcome::value(to_json(&r)));
    }
    let opt = |v: Option<u16>| v.map_or("?".to_string(), |x| x.to_string());
    let mut steps: Vec<(String, Vec<Report>)> = vec![];
    if let Some(v) = idle {
        steps.push((format!("sleep after {} -> {v} s", opt(before.idle_s)), vec![power::set_idle(tid, v)]));
    }
    if let Some(r) = low_raw {
        let pct = power::raw_to_pct(r);
        let label = format!("low battery {}% -> {pct}%", opt(before.low_battery_pct.map(u16::from)));
        steps.push((label, vec![power::set_low_battery(tid, r)]));
    }
    let show = |s: &PowerState| serde_json::to_string(s).unwrap_or_default();
    let parts = apply_steps(t, journal, entry(def, "power", show(&before), String::new()), steps)?;
    let after = read_power(t, def, tid);
    let verified = wants(&after);
    journal.record(&entry(def, "power", show(&before), show(&after)), State::Done);
    let line = format!("ONBOARD WRITE {}: {} (verified {verified})", def.id, parts.join(", "));
    Ok(Outcome {
        result: to_json(&WriteResult { before, after, verified, unchanged: false }),
        lighting: Lighting::Unchanged,
        log: vec![line],
    })
}

/// Send a write made of several labelled steps, journalling it `pending` first. If a step fails, a
/// `failed` entry records which steps were applied, and the error says so too. Returns the labels.
fn apply_steps(
    t: &mut dyn Transport,
    journal: &Journal,
    pending: JournalEntry,
    steps: Vec<(String, Vec<Report>)>,
) -> Result<Vec<String>> {
    let labels: Vec<String> = steps.iter().map(|(l, _)| l.clone()).collect();
    journal.record(
        &JournalEntry { after: format!("requested: {}", labels.join(", ")), ..pending.clone() },
        State::Pending,
    );
    let mut applied: Vec<&str> = vec![];
    for (label, reports) in &steps {
        for r in reports {
            if let Err(e) = query_ok(t, r) {
                let done = if applied.is_empty() { "nothing".to_string() } else { applied.join(", ") };
                let after = format!("applied: {done}; failed at {label}: {e:#}");
                journal.record(&JournalEntry { after, ..pending }, State::Failed);
                return Err(e.context(format!("{label} failed (already applied: {done})")));
            }
        }
        applied.push(label);
    }
    Ok(labels)
}

/// Read `0F/80` regions and `0F/81` effects for each (read-only).
pub fn probe_lighting(t: &mut dyn Transport, tid: u8) -> Result<LightingProbe> {
    let regions = hw_effect::parse_regions(&query_read(t, &hw_effect::get_regions(tid))?);
    let mut effects = vec![];
    for r in &regions {
        if let Ok(reply) = query_read(t, &hw_effect::get_supported_effects(tid, r.led)) {
            let names = hw_effect::parse_supported_effects(&reply)
                .into_iter()
                .map(|e| hw_effect::effect_name(e).to_string())
                .collect();
            effects.push((r.led, names));
        }
    }
    Ok(LightingProbe { regions, effects })
}

fn name_of_mode(id: u8) -> String {
    DialMode::from_id(id).map(|m| m.name().to_string()).unwrap_or_else(|| format!("MODE_{id}"))
}

fn keymap_of(def: &DeviceDef) -> Result<&KeymapDef> {
    def.keymap.as_ref().ok_or_else(|| anyhow!("{} has no key map definition", def.name))
}

fn check_layer(km: &KeymapDef, layer: Layer) -> Result<()> {
    if km.layers.contains(&layer) {
        Ok(())
    } else {
        bail!("this device has no {} layer", layer.as_str())
    }
}

fn key_of<'a>(def: &'a DeviceDef, key: &str, layer: Layer) -> Result<(&'a KeymapDef, &'a KeyDef)> {
    let km = keymap_of(def)?;
    check_layer(km, layer)?;
    let k = km.find(key).ok_or_else(|| {
        let names: Vec<&str> = km.keys.iter().map(|k| k.name.as_str()).collect();
        anyhow!("{} has no key \"{key}\" (keys: {})", def.name, names.join(" "))
    })?;
    Ok((km, k))
}

fn read_key(
    t: &mut dyn Transport,
    tid: u8,
    km: &KeymapDef,
    key: &KeyDef,
    profile: u8,
    layer: Layer,
) -> Result<KeyMapping> {
    let reply = query_read(t, &km.get_report(tid, profile, key.id, layer))?;
    let f = keymap::parse_reply(&reply, profile, key.id)?;
    Ok(KeyMapping::new(profile, key.id, key.name.clone(), layer, f))
}

#[allow(clippy::too_many_arguments)]
fn write_key(
    t: &mut dyn Transport,
    tid: u8,
    def: &DeviceDef,
    km: &KeymapDef,
    key: &KeyDef,
    profile: u8,
    layer: Layer,
    f: &Function,
    journal: &Journal,
    cmd: &str,
) -> Result<Outcome> {
    let report = km.set_report(tid, profile, key.id, layer, f)?;
    let before = read_key(t, tid, km, key, profile, layer)?;
    if &before.function == f {
        let r = WriteResult { after: before.clone(), before, verified: true, unchanged: true };
        return Ok(Outcome::value(to_json(&r)));
    }
    left_click_guard(t, tid, def, km, key, profile, layer, &before.function, f)?;
    let mut e = entry(def, "keymap", before.function.to_string(), f.to_string());
    (e.profile, e.key, e.layer) = (Some(profile), Some(key.id), Some(layer));
    journal.record(&e, State::Pending);
    query_ok(t, &report)?;
    let after = read_key(t, tid, km, key, profile, layer)?;
    let verified = &after.function == f;
    journal.record(&JournalEntry { after: after.function.to_string(), ..e }, State::Done);
    let line = format!(
        "ONBOARD WRITE {} ({cmd}): profile {profile} {} {} `{}` -> `{}` (verified {verified})",
        def.id,
        layer.as_str(),
        key.name,
        before.function,
        after.function
    );
    Ok(Outcome {
        result: to_json(&WriteResult { before, after, verified, unchanged: false }),
        lighting: Lighting::Unchanged,
        log: vec![line],
    })
}

/// On a mouse's normal layer, refuse to take left click away from the last button that has it.
#[allow(clippy::too_many_arguments)]
fn left_click_guard(
    t: &mut dyn Transport,
    tid: u8,
    def: &DeviceDef,
    km: &KeymapDef,
    key: &KeyDef,
    profile: u8,
    layer: Layer,
    current: &Function,
    new: &Function,
) -> Result<()> {
    let left = Function::MouseButton { button: 1 };
    if def.kind != Kind::Mouse || layer != Layer::Normal || current != &left || new == &left {
        return Ok(());
    }
    for other in km.keys.iter().filter(|k| k.id != key.id) {
        if read_key(t, tid, km, other, profile, layer)?.function == left {
            return Ok(());
        }
    }
    Err(coded(
        codes::LEFT_CLICK_GUARD,
        "This would leave no button that left-clicks. Map another button to left click first.",
    ))
}

fn read_oled(t: &mut dyn Transport, tid: u8) -> Result<OledState> {
    let mut s = OledState::default();
    let mut any = false;
    for g in oled::STATE_GETTERS {
        if let Ok(reply) = query_read(t, &oled::get(tid, g)) {
            s.apply(&reply);
            any = true;
        }
    }
    if !any {
        bail!("the device answered none of the OLED getters");
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeDevice;
    use serde_json::json;
    use uncoil_core::device::builtin;
    use uncoil_core::features::keymap::Function;

    fn kb() -> DeviceDef {
        builtin().into_iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap()
    }

    fn journal() -> (Journal, PathBuf) {
        let p =
            std::env::temp_dir().join(format!("uncoil-test-journal-{}-{}.jsonl", std::process::id(), rand_suffix()));
        let _ = std::fs::remove_file(&p);
        (Journal { path: Some(p.clone()) }, p)
    }

    fn rand_suffix() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    /// `run` with a fresh check cache (supported devices need none).
    fn go(dev: &mut FakeDevice, def: &DeviceDef, tid: u8, cmd: &Command, j: &Journal) -> Result<Outcome> {
        run(dev, def, tid, cmd, j, &mut Checks::new(def))
    }

    fn cmd(name: &str, args: serde_json::Value) -> Command {
        Command::from_parts(name, Some(&args.to_string())).unwrap()
    }

    #[allow(clippy::boxed_local)]
    fn from_raw<T: serde::de::DeserializeOwned>(r: Raw) -> serde_json::Result<T> {
        serde_json::from_str(r.get())
    }

    #[test]
    fn keymap_get_reads_without_writing() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let (j, _) = journal();
        let out = go(&mut dev, &def, 0x1F, &cmd("keymap.get", json!({"key": "P", "layer": "fn"})), &j).unwrap();
        let m: KeyMapping = from_raw(out.result).unwrap();
        assert_eq!(m.function, Function::Key { modifiers: 0, usage: 0 }, "the fake starts as Synapse left Fn+P");
        assert_eq!(m.key, 26);
        assert!(dev.writes().is_empty());
    }

    #[test]
    fn keymap_set_requires_write_flag() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let (j, _) = journal();
        let c = cmd("keymap.set", json!({"key": "P", "layer": "fn", "function": "key PRINT_SCREEN"}));
        let err = go(&mut dev, &def, 0x1F, &c, &j).unwrap_err().to_string();
        assert!(err.contains("write=true"), "{err}");
        assert!(dev.writes().is_empty(), "nothing may reach the device without write=true");
    }

    #[test]
    fn keymap_set_writes_verifies_journals_and_resets() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let (j, path) = journal();
        let c = cmd("keymap.set", json!({"key": "P", "layer": "fn", "function": "key PRINT_SCREEN", "write": true}));
        let out = go(&mut dev, &def, 0x1F, &c, &j).unwrap();
        let r: WriteResult<KeyMapping> = from_raw(out.result).unwrap();
        assert!(r.verified && !r.unchanged);
        assert_eq!(r.before.function.to_string(), "key NONE");
        assert_eq!(r.after.function.to_string(), "key PRINT_SCREEN");
        // exactly the bytes obm_set_fnp.py wrote
        assert_eq!(dev.writes(), vec![(0x02, 0x0D, vec![1, 26, 1, 2, 2, 0, 0x46])]);
        assert!(out.log[0].contains("ONBOARD WRITE"));
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"before\":\"key NONE\""));

        // writing the same thing again sends nothing
        let again = go(&mut dev, &def, 0x1F, &c, &j).unwrap();
        assert!(from_raw::<WriteResult<KeyMapping>>(again.result).unwrap().unchanged);
        assert_eq!(dev.writes().len(), 1);

        // reset restores what was there before uncoil's first write (from the journal)
        let reset = cmd("keymap.reset", json!({"key": "P", "layer": "fn", "write": true}));
        let r: WriteResult<KeyMapping> = from_raw(go(&mut dev, &def, 0x1F, &reset, &j).unwrap().result).unwrap();
        assert_eq!(r.after.function.to_string(), "key NONE");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reset_without_journal_uses_the_definition_default() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let j = Journal { path: None };
        dev.set_key(1, 26, 0, 17, &[4]);
        let reset = cmd("keymap.reset", json!({"key": "P", "write": true}));
        let r: WriteResult<KeyMapping> = from_raw(go(&mut dev, &def, 0x1F, &reset, &j).unwrap().result).unwrap();
        assert_eq!(r.after.function, Function::Key { modifiers: 0, usage: 0x13 });
    }

    #[test]
    fn dump_reads_every_key() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let rows: Vec<KeyMapping> = from_raw(
            go(&mut dev, &def, 0x1F, &cmd("keymap.dump", json!({"layer": "hypershift"})), &Journal { path: None })
                .unwrap()
                .result,
        )
        .unwrap();
        assert_eq!(rows.len(), def.keymap.as_ref().unwrap().keys.len());
        assert!(rows.iter().any(|r| r.name == "F9" && r.function == Function::RazerKey { code: 4 }));
        assert!(dev.writes().is_empty());
    }

    #[test]
    fn unknown_key_and_unsupported_feature() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let j = Journal { path: None };
        assert!(go(&mut dev, &def, 0x1F, &cmd("keymap.get", json!({"key": "NOPE"})), &j).is_err());
        let mat = builtin().into_iter().find(|d| d.id == "razer-goliathus-chroma-extended").unwrap();
        let e = go(&mut dev, &mat, 0x3F, &cmd("keymap.get", json!({"key": "P"})), &j).unwrap_err().to_string();
        assert!(e.contains("does not support keymap"), "{e}");
    }

    #[test]
    fn profiles_dial_oled() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let j = Journal { path: None };
        let p: ProfileInfo = from_raw(go(&mut dev, &def, 0x1F, &Command::ProfileList, &j).unwrap().result).unwrap();
        assert_eq!(p, ProfileInfo { max: 5, count: 1, ids: vec![1], active: Some(1) });
        let d: dial::DialState =
            from_raw(go(&mut dev, &def, 0x1F, &cmd("dial.get", json!({})), &j).unwrap().result).unwrap();
        assert_eq!(d.mode, Some(DialMode::Volume));
        let o: OledState = from_raw(go(&mut dev, &def, 0x1F, &Command::OledGet, &j).unwrap().result).unwrap();
        assert_eq!(o.brightness, Some(100));
        assert!(dev.writes().is_empty());

        // writes need the flag, then go through with read-back
        assert!(go(&mut dev, &def, 0x1F, &cmd("dial.set", json!({"mode": "ZOOM"})), &j).is_err());
        let w: WriteResult<dial::DialState> = from_raw(
            go(&mut dev, &def, 0x1F, &cmd("dial.set", json!({"mode": "ZOOM", "write": true})), &j).unwrap().result,
        )
        .unwrap();
        assert!(w.verified);
        assert_eq!(dev.writes(), vec![(0x17, 0x00, vec![1, 5, 6, 6])]);
        let w: WriteResult<u8> = from_raw(
            go(&mut dev, &def, 0x1F, &cmd("oled.set", json!({"brightness": 40, "write": true})), &j).unwrap().result,
        )
        .unwrap();
        assert_eq!((w.before, w.after, w.verified), (100, 40, true));
    }

    #[test]
    fn hardware_effects() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let j = Journal { path: None };
        let out = go(&mut dev, &def, 0x1F, &cmd("effect.hw", json!({"effect": "spectrum"})), &j).unwrap();
        assert_eq!(out.lighting, Lighting::Hardware(HwEffect::Spectrum));
        assert_eq!(dev.sent().last().unwrap(), &(0x0F, 0x02, vec![0, 5, 3, 0, 0, 0]));
        assert!(out.log.is_empty(), "session effects are not onboard writes");
        // saving to the device needs write
        assert!(go(&mut dev, &def, 0x1F, &cmd("effect.hw", json!({"effect": "spectrum", "storage": "onboard"})), &j)
            .is_err());
        // the mat cannot do reactive
        let mat = builtin().into_iter().find(|d| d.id == "razer-goliathus-chroma-extended").unwrap();
        let e = go(&mut dev, &mat, 0x3F, &cmd("effect.hw", json!({"effect": "reactive #ff0000"})), &j);
        assert!(e.unwrap_err().to_string().contains("does not run"));
        let back = go(&mut dev, &def, 0x1F, &Command::EffectSoftware, &j).unwrap();
        assert_eq!(back.lighting, Lighting::Software);
    }

    #[test]
    fn lighting_probe() {
        let mut dev = FakeDevice::keyboard();
        let p = probe_lighting(&mut dev, 0x1F).unwrap();
        assert_eq!(p.regions.len(), 1);
        assert_eq!(p.effects[0].1[4], "wave");
    }

    fn mouse() -> DeviceDef {
        builtin().into_iter().find(|d| d.id == "razer-basilisk-v3-pro").unwrap()
    }

    fn code(e: &anyhow::Error) -> Option<&'static str> {
        e.downcast_ref::<ipc::CodedError>().map(|c| c.code)
    }

    #[test]
    fn performance_get_reads_dpi_stages_and_poll() {
        let def = mouse();
        let mut dev = FakeDevice::for_def(&def);
        let j = Journal { path: None };
        let s: PerformanceState =
            from_raw(go(&mut dev, &def, 0x1F, &Command::PerformanceGet, &j).unwrap().result).unwrap();
        assert_eq!(s.dpi, Some(perf::Dpi { x: 1600, y: 1600 }));
        assert_eq!((s.dpi_min, s.dpi_max, s.stages_max), (Some(100), Some(30000), 5));
        let st = s.stages.unwrap();
        assert_eq!(st.active, 3);
        assert_eq!(st.list.iter().map(|d| d.x).collect::<Vec<_>>(), vec![400, 800, 1600, 3200, 6400]);
        assert_eq!((s.poll_hz, s.poll_rates), (Some(1000), vec![125, 500, 1000]));
        assert!(dev.setters().is_empty());
        // the keyboard has neither
        let kb = kb();
        let e = go(&mut FakeDevice::keyboard(), &kb, 0x1F, &Command::PerformanceGet, &j).unwrap_err();
        assert_eq!(code(&e), Some(codes::NOT_SUPPORTED));
    }

    #[test]
    fn live_dpi_needs_no_write_flag_but_passes_the_check() {
        let def = mouse();
        let mut dev = FakeDevice::for_def(&def);
        let j = Journal { path: None };
        let mut checks = Checks::new(&def);
        let c = cmd("performance.set", json!({"dpi": {"x": 50, "y": 800}}));
        let s: PerformanceState = from_raw(run(&mut dev, &def, 0x1F, &c, &j, &mut checks).unwrap().result).unwrap();
        assert_eq!(s.dpi, Some(perf::Dpi { x: 100, y: 800 }), "clamped to the file's minimum");
        // the DPI check ran first (read-only), then one live set
        assert_eq!(dev.setters(), vec![(0x04, 0x05, vec![0, 0, 100, 3, 0x20, 0, 0])]);
        assert!(dev.writes().is_empty(), "a live DPI change is not an onboard write");
        assert!(checks.list().iter().any(|c| c.feature == Feature::Dpi && c.state == ipc::CheckState::Passed));
    }

    #[test]
    fn stages_and_poll_need_write_and_are_read_back() {
        let def = mouse();
        let mut dev = FakeDevice::for_def(&def);
        let (j, path) = journal();
        let c = cmd("performance.set", json!({"poll_hz": 500}));
        assert!(go(&mut dev, &def, 0x1F, &c, &j).unwrap_err().to_string().contains("write=true"));
        assert!(dev.setters().is_empty());
        let c = cmd(
            "performance.set",
            json!({"poll_hz": 500, "stages": {"active": 2, "list": [{"x": 400, "y": 400}, {"x": 900, "y": 900}]}, "write": true}),
        );
        let out = go(&mut dev, &def, 0x1F, &c, &j).unwrap();
        let w: WriteResult<PerformanceState> = from_raw(out.result).unwrap();
        assert!(w.verified && !w.unchanged);
        assert_eq!((w.before.poll_hz, w.after.poll_hz), (Some(1000), Some(500)));
        assert_eq!(w.after.stages.unwrap().list.len(), 2);
        assert!(out.log[0].starts_with("ONBOARD WRITE razer-basilisk-v3-pro"), "{}", out.log[0]);
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"cmd\":\"performance\""));
        assert!(go(&mut dev, &def, 0x1F, &cmd("performance.set", json!({"poll_hz": 8000, "write": true})), &j).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn power_get_and_set() {
        let def = mouse();
        let mut dev = FakeDevice::for_def(&def);
        let j = Journal { path: None };
        let s: PowerState = from_raw(go(&mut dev, &def, 0x1F, &Command::PowerGet, &j).unwrap().result).unwrap();
        assert_eq!(
            s,
            PowerState {
                battery_pct: Some(78),
                charging: Some(false),
                idle_s: Some(300),
                idle_range: Some((60, 900)),
                low_battery_pct: Some(15),
                low_battery_range: Some((5, 25)),
            }
        );
        assert!(go(&mut dev, &def, 0x1F, &cmd("power.set", json!({"idle_s": 600})), &j).is_err());
        let w: WriteResult<PowerState> = from_raw(
            go(
                &mut dev,
                &def,
                0x1F,
                &cmd("power.set", json!({"idle_s": 9000, "low_battery_pct": 20, "write": true})),
                &j,
            )
            .unwrap()
            .result,
        )
        .unwrap();
        assert!(w.verified);
        assert_eq!((w.after.idle_s, w.after.low_battery_pct), (Some(900), Some(20)));
        // low battery went out with transaction id 0xFF (OpenRazer), everything else with 0x1F
        let tids = dev.sent_tids();
        assert!(tids.iter().filter(|(c, i, _)| *c == 0x07 && (*i == 0x01 || *i == 0x81)).all(|(_, _, t)| *t == 0xFF));
        assert!(tids.iter().filter(|(c, i, _)| *c == 0x07 && *i == 0x03).all(|(_, _, t)| *t == 0x1F));
        assert!(tids.iter().any(|(c, i, _)| (*c, *i) == (0x07, 0x01)));
    }

    #[test]
    fn experimental_writes_wait_for_their_check() {
        let def = crate::fake::deathadder();
        let mut dev = FakeDevice::for_def(&def);
        let j = Journal { path: None };
        let mut checks = Checks::new(&def);
        assert!(checks.list().iter().all(|c| c.state == ipc::CheckState::Untested));
        // a failing DPI check blocks DPI changes with check_failed; reads still work
        dev.set_dpi(0, 0);
        let c = cmd("performance.set", json!({"dpi": {"x": 800, "y": 800}}));
        let e = run(&mut dev, &def, 0x1F, &c, &j, &mut checks).unwrap_err();
        assert_eq!(code(&e), Some(codes::CHECK_FAILED), "{e}");
        assert!(!e.to_string().contains("Lighting still works"), "this mouse has no lighting");
        assert!(dev.setters().is_empty());
        assert!(run(&mut dev, &def, 0x1F, &Command::PerformanceGet, &j, &mut checks).is_ok());
        // check.run after the device answers properly: everything passes, then writes go through
        dev.set_dpi(800, 800);
        let list: Vec<ipc::FeatureCheck> =
            from_raw(run(&mut dev, &def, 0x1F, &Command::CheckRun, &j, &mut checks).unwrap().result).unwrap();
        assert!(list.iter().all(|c| c.state == ipc::CheckState::Passed), "{list:?}");
        assert!(run(&mut dev, &def, 0x1F, &c, &j, &mut checks).is_ok());
    }

    /// Every read command, on every fake device, sends only "get" reports.
    #[test]
    fn read_paths_send_only_get_reports() {
        let j = Journal { path: None };
        for def in [kb(), mouse(), crate::fake::deathadder()] {
            let mut dev = FakeDevice::for_def(&def);
            let mut checks = Checks::new(&def);
            for c in [
                cmd("keymap.get", json!({"key": "#1"})),
                cmd("keymap.dump", json!({})),
                cmd("profile.list", json!({})),
                cmd("dial.get", json!({})),
                cmd("oled.get", json!({})),
                cmd("check.run", json!({})),
                cmd("performance.get", json!({})),
                cmd("power.get", json!({})),
            ] {
                let _ = run(&mut dev, &def, 0x1F, &c, &j, &mut checks);
            }
            let _ = probe_lighting(&mut dev, 0x1F);
            assert!(!dev.sent().is_empty());
            assert!(dev.sent().iter().all(|(_, id, _)| id & 0x80 != 0), "{}: {:?}", def.id, dev.sent());
        }
    }

    #[test]
    fn profiles_out_of_range_are_refused_before_any_io() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let j = Journal { path: None };
        for p in [0, 6, 255] {
            let e = go(&mut dev, &def, 0x1F, &cmd("keymap.get", json!({"key": "P", "profile": p})), &j).unwrap_err();
            assert!(e.to_string().contains("profiles are 1 to 5"), "{e}");
            let set = cmd("dial.set", json!({"mode": "ZOOM", "profile": p, "write": true}));
            assert!(go(&mut dev, &def, 0x1F, &set, &j).is_err());
        }
        assert!(dev.sent().is_empty());
    }

    #[test]
    fn saving_an_effect_on_an_unconfirmed_device_waits_for_its_check() {
        // an experimental keyboard whose regions the fake answers do not add up to its matrix
        let def = builtin()
            .into_iter()
            .find(|d| {
                d.is_experimental() && d.hw_effects.as_ref().is_some_and(|h| h.effects.iter().any(|e| e == "spectrum"))
            })
            .unwrap();
        let mut dev = FakeDevice::for_def(&def);
        let j = Journal { path: None };
        let mut checks = Checks::new(&def);
        let effect = "spectrum";
        let save = cmd("effect.hw", json!({"effect": effect, "storage": "onboard", "write": true}));
        let e = run(&mut dev, &def, 0x1F, &save, &j, &mut checks).unwrap_err();
        assert_eq!(code(&e), Some(codes::CHECK_FAILED), "{e}");
        assert!(!dev.sent().iter().any(|(c, id, _)| (*c, *id) == (0x0F, 0x02)), "nothing saved");
        // showing it for the session is never refused
        let show = cmd("effect.hw", json!({"effect": effect}));
        assert!(run(&mut dev, &def, 0x1F, &show, &j, &mut checks).is_ok());
    }

    #[test]
    fn journal_has_pending_before_the_write_and_failed_when_it_stops_part_way() {
        let def = mouse();
        let mut dev = FakeDevice::for_def(&def);
        let (j, path) = journal();
        // a key map write: pending (with the before-state) then done
        let c = cmd("keymap.set", json!({"key": "BACK", "function": "button 5", "write": true}));
        go(&mut dev, &def, 0x1F, &c, &j).unwrap();
        let lines: Vec<serde_json::Value> =
            std::fs::read_to_string(&path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!((lines[0]["state"].as_str(), lines[0]["after"].as_str()), (Some("pending"), Some("button 5")));
        assert_eq!(lines[0]["before"], "button 4");
        assert_eq!(lines[1]["state"], "done");
        // a power write whose second report fails: the first is recorded as applied
        let _ = std::fs::remove_file(&path);
        dev.fail_on(0x07, 0x01);
        let c = cmd("power.set", json!({"idle_s": 120, "low_battery_pct": 20, "write": true}));
        let e = format!("{:#}", go(&mut dev, &def, 0x1F, &c, &j).unwrap_err());
        assert!(e.contains("already applied: sleep after 300 -> 120 s"), "{e}");
        let text = std::fs::read_to_string(&path).unwrap();
        let last: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(last["state"], "failed");
        assert!(last["after"]
            .as_str()
            .unwrap()
            .starts_with("applied: sleep after 300 -> 120 s; failed at low battery"));
        assert!(text.lines().next().unwrap().contains("\"pending\""));
        // reset still finds the original mapping (the pending entry carries it)
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn left_click_guard_keeps_one_left_click() {
        let def = mouse();
        let mut dev = FakeDevice::for_def(&def);
        let j = Journal { path: None };
        let take = cmd("keymap.set", json!({"key": "LEFT_CLICK", "function": "button 3", "write": true}));
        let e = go(&mut dev, &def, 0x1F, &take, &j).unwrap_err();
        assert_eq!(code(&e), Some(codes::LEFT_CLICK_GUARD));
        assert_eq!(
            e.to_string(),
            "This would leave no button that left-clicks. Map another button to left click first."
        );
        assert!(dev.writes().is_empty());
        // give left click to the back button first, then the main button may change
        go(&mut dev, &def, 0x1F, &cmd("keymap.set", json!({"key": "BACK", "function": "button 1", "write": true})), &j)
            .unwrap();
        assert!(go(&mut dev, &def, 0x1F, &take, &j).is_ok());
        // the Hypershift layer is not guarded
        let fnl = cmd("keymap.set", json!({"key": "BACK", "layer": "fn", "function": "button 3", "write": true}));
        assert!(go(&mut dev, &def, 0x1F, &fnl, &j).is_ok());
    }
}
