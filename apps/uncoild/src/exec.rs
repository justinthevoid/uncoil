//! Runs control-pipe commands against one device. Generic over [`Transport`], so the same code drives the
//! real HID device (from its renderer thread, between frames) and the fake device used by tests.
//!
//! Every command that writes onboard memory is refused unless the request says `write: true`; when it runs,
//! the value is read before and after, the write is logged, and it is appended to the journal
//! (`%LOCALAPPDATA%\uncoil\onboard-writes.jsonl`) so `keymap.reset` can restore what was there before
//! uncoil first touched a key.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use uncoil_core::device::DeviceDef;
use uncoil_core::features::dial::{self, DialMode};
use uncoil_core::features::hw_effect::{self, HwEffect, Storage};
use uncoil_core::features::keymap::{self, Function, KeyDef, KeymapDef, Layer};
use uncoil_core::features::oled::{self, OledState};
use uncoil_core::features::profile::{self, ProfileInfo};
use uncoil_core::ipc::{self, Command, EffectState, KeyMapping, LightingProbe, Raw, WriteResult};
use uncoil_core::proto::{self, query_ok, DeviceMode, Transport};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JournalEntry {
    t: u64,
    device: String,
    cmd: String,
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

    fn record(&self, e: &JournalEntry) {
        let Some(p) = &self.path else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "{}", serde_json::to_string(e).unwrap_or_default());
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

/// Run one device-level command.
pub fn run(t: &mut dyn Transport, def: &DeviceDef, tid: u8, cmd: &Command, journal: &Journal) -> Result<Outcome> {
    if let Some(f) = cmd.feature() {
        if !def.has(f) {
            bail!("{} does not support {}", def.name, f.as_str());
        }
    }
    if cmd.writes_onboard() && !cmd.write_confirmed() {
        bail!("this writes {}'s onboard memory; repeat with write=true (CLI: --write)", def.name);
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
            let max = profile::parse_byte(&query_ok(t, &profile::get_max(tid))?);
            let count = profile::parse_byte(&query_ok(t, &profile::get_count(tid))?);
            let ids = profile::parse_ids(&query_ok(t, &profile::get_ids(tid))?);
            let active = query_ok(t, &profile::get_active(tid)).ok().map(|r| profile::parse_byte(&r));
            Ok(Outcome::value(to_json(&ProfileInfo { max, count, ids, active })))
        }
        Command::DialGet(a) => {
            Ok(Outcome::value(to_json(&dial::parse_state(&query_ok(t, &dial::get_active_mode(tid, a.profile))?))))
        }
        Command::DialSet(a) => {
            let enabled = a.enabled.clone().unwrap_or_else(|| DialMode::DEFAULT_ENABLED.to_vec());
            let report = dial::set_active_mode(tid, a.profile, a.mode, &enabled)?;
            let before = dial::parse_state(&query_ok(t, &dial::get_active_mode(tid, a.profile))?);
            if before.mode == Some(a.mode) {
                return Ok(Outcome::value(to_json(&WriteResult {
                    after: before.clone(),
                    before,
                    verified: true,
                    unchanged: true,
                })));
            }
            query_ok(t, &report)?;
            let after = dial::parse_state(&query_ok(t, &dial::get_active_mode(tid, a.profile))?);
            let verified = after.mode == Some(a.mode);
            let line = format!(
                "ONBOARD WRITE {}: dial mode profile {} {} -> {} (verified {verified})",
                def.id,
                a.profile,
                name_of_mode(before.mode_id),
                a.mode.name()
            );
            journal.record(&JournalEntry {
                t: unix_now(),
                device: def.id.clone(),
                cmd: "dial".into(),
                profile: Some(a.profile),
                key: None,
                layer: None,
                before: name_of_mode(before.mode_id),
                after: a.mode.name().into(),
            });
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
                |t: &mut dyn Transport| -> Result<u8> { Ok(query_ok(t, &oled::get(tid, oled::BRIGHTNESS))?.raw[0]) };
            let before = read(t)?;
            if before == pct {
                return Ok(Outcome::value(to_json(&WriteResult {
                    before,
                    after: before,
                    verified: true,
                    unchanged: true,
                })));
            }
            query_ok(t, &oled::set_brightness(tid, pct))?;
            let after = read(t)?;
            let verified = after == pct;
            journal.record(&JournalEntry {
                t: unix_now(),
                device: def.id.clone(),
                cmd: "oled.brightness".into(),
                profile: None,
                key: None,
                layer: None,
                before: before.to_string(),
                after: after.to_string(),
            });
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
            query_ok(t, &a.effect.report(tid, a.storage, hw.led)?)?;
            let mut log = vec![];
            if a.storage == Storage::Onboard {
                log.push(format!("ONBOARD WRITE {}: firmware effect {} saved to the device", def.id, a.effect.name()));
                journal.record(&JournalEntry {
                    t: unix_now(),
                    device: def.id.clone(),
                    cmd: "effect".into(),
                    profile: None,
                    key: None,
                    layer: None,
                    before: String::new(),
                    after: serde_json::to_string(&a.effect).unwrap_or_default(),
                });
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
        Command::Status | Command::Devices | Command::Capabilities(_) => bail!("{:?} is answered by the daemon", cmd),
    }
}

/// Read `0F/80` regions and `0F/81` effects for each (read-only).
pub fn probe_lighting(t: &mut dyn Transport, tid: u8) -> Result<LightingProbe> {
    let regions = hw_effect::parse_regions(&query_ok(t, &hw_effect::get_regions(tid))?);
    let mut effects = vec![];
    for r in &regions {
        if let Ok(reply) = query_ok(t, &hw_effect::get_supported_effects(tid, r.led)) {
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
    let reply = query_ok(t, &km.get_report(tid, profile, key.id, layer))?;
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
    query_ok(t, &report)?;
    let after = read_key(t, tid, km, key, profile, layer)?;
    let verified = &after.function == f;
    journal.record(&JournalEntry {
        t: unix_now(),
        device: def.id.clone(),
        cmd: "keymap".into(),
        profile: Some(profile),
        key: Some(key.id),
        layer: Some(layer),
        before: before.function.to_string(),
        after: after.function.to_string(),
    });
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

fn read_oled(t: &mut dyn Transport, tid: u8) -> Result<OledState> {
    let mut s = OledState::default();
    let mut any = false;
    for g in oled::STATE_GETTERS {
        if let Ok(reply) = query_ok(t, &oled::get(tid, g)) {
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
        let out = run(&mut dev, &def, 0x1F, &cmd("keymap.get", json!({"key": "P", "layer": "fn"})), &j).unwrap();
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
        let err = run(&mut dev, &def, 0x1F, &c, &j).unwrap_err().to_string();
        assert!(err.contains("write=true"), "{err}");
        assert!(dev.writes().is_empty(), "nothing may reach the device without write=true");
    }

    #[test]
    fn keymap_set_writes_verifies_journals_and_resets() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let (j, path) = journal();
        let c = cmd("keymap.set", json!({"key": "P", "layer": "fn", "function": "key PRINT_SCREEN", "write": true}));
        let out = run(&mut dev, &def, 0x1F, &c, &j).unwrap();
        let r: WriteResult<KeyMapping> = from_raw(out.result).unwrap();
        assert!(r.verified && !r.unchanged);
        assert_eq!(r.before.function.to_string(), "key NONE");
        assert_eq!(r.after.function.to_string(), "key PRINT_SCREEN");
        // exactly the bytes obm_set_fnp.py wrote
        assert_eq!(dev.writes(), vec![(0x02, 0x0D, vec![1, 26, 1, 2, 2, 0, 0x46])]);
        assert!(out.log[0].contains("ONBOARD WRITE"));
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"before\":\"key NONE\""));

        // writing the same thing again sends nothing
        let again = run(&mut dev, &def, 0x1F, &c, &j).unwrap();
        assert!(from_raw::<WriteResult<KeyMapping>>(again.result).unwrap().unchanged);
        assert_eq!(dev.writes().len(), 1);

        // reset restores what was there before uncoil's first write (from the journal)
        let reset = cmd("keymap.reset", json!({"key": "P", "layer": "fn", "write": true}));
        let r: WriteResult<KeyMapping> = from_raw(run(&mut dev, &def, 0x1F, &reset, &j).unwrap().result).unwrap();
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
        let r: WriteResult<KeyMapping> = from_raw(run(&mut dev, &def, 0x1F, &reset, &j).unwrap().result).unwrap();
        assert_eq!(r.after.function, Function::Key { modifiers: 0, usage: 0x13 });
    }

    #[test]
    fn dump_reads_every_key() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let rows: Vec<KeyMapping> = from_raw(
            run(&mut dev, &def, 0x1F, &cmd("keymap.dump", json!({"layer": "hypershift"})), &Journal { path: None })
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
        assert!(run(&mut dev, &def, 0x1F, &cmd("keymap.get", json!({"key": "NOPE"})), &j).is_err());
        let mat = builtin().into_iter().find(|d| d.id == "razer-goliathus-chroma-extended").unwrap();
        let e = run(&mut dev, &mat, 0x3F, &cmd("keymap.get", json!({"key": "P"})), &j).unwrap_err().to_string();
        assert!(e.contains("does not support keymap"), "{e}");
    }

    #[test]
    fn profiles_dial_oled() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let j = Journal { path: None };
        let p: ProfileInfo = from_raw(run(&mut dev, &def, 0x1F, &Command::ProfileList, &j).unwrap().result).unwrap();
        assert_eq!(p, ProfileInfo { max: 5, count: 1, ids: vec![1], active: Some(1) });
        let d: dial::DialState =
            from_raw(run(&mut dev, &def, 0x1F, &cmd("dial.get", json!({})), &j).unwrap().result).unwrap();
        assert_eq!(d.mode, Some(DialMode::Volume));
        let o: OledState = from_raw(run(&mut dev, &def, 0x1F, &Command::OledGet, &j).unwrap().result).unwrap();
        assert_eq!(o.brightness, Some(100));
        assert!(dev.writes().is_empty());

        // writes need the flag, then go through with read-back
        assert!(run(&mut dev, &def, 0x1F, &cmd("dial.set", json!({"mode": "ZOOM"})), &j).is_err());
        let w: WriteResult<dial::DialState> = from_raw(
            run(&mut dev, &def, 0x1F, &cmd("dial.set", json!({"mode": "ZOOM", "write": true})), &j).unwrap().result,
        )
        .unwrap();
        assert!(w.verified);
        assert_eq!(dev.writes(), vec![(0x17, 0x00, vec![1, 5, 6, 6])]);
        let w: WriteResult<u8> = from_raw(
            run(&mut dev, &def, 0x1F, &cmd("oled.set", json!({"brightness": 40, "write": true})), &j).unwrap().result,
        )
        .unwrap();
        assert_eq!((w.before, w.after, w.verified), (100, 40, true));
    }

    #[test]
    fn hardware_effects() {
        let def = kb();
        let mut dev = FakeDevice::keyboard();
        let j = Journal { path: None };
        let out = run(&mut dev, &def, 0x1F, &cmd("effect.hw", json!({"effect": "spectrum"})), &j).unwrap();
        assert_eq!(out.lighting, Lighting::Hardware(HwEffect::Spectrum));
        assert_eq!(dev.sent().last().unwrap(), &(0x0F, 0x02, vec![0, 5, 3, 0, 0, 0]));
        assert!(out.log.is_empty(), "session effects are not onboard writes");
        // saving to the device needs write
        assert!(run(&mut dev, &def, 0x1F, &cmd("effect.hw", json!({"effect": "spectrum", "storage": "onboard"})), &j)
            .is_err());
        // the mat cannot do reactive
        let mat = builtin().into_iter().find(|d| d.id == "razer-goliathus-chroma-extended").unwrap();
        let e = run(&mut dev, &mat, 0x3F, &cmd("effect.hw", json!({"effect": "reactive #ff0000"})), &j);
        assert!(e.unwrap_err().to_string().contains("does not run"));
        let back = run(&mut dev, &def, 0x1F, &Command::EffectSoftware, &j).unwrap();
        assert_eq!(back.lighting, Lighting::Software);
    }

    #[test]
    fn lighting_probe() {
        let mut dev = FakeDevice::keyboard();
        let p = probe_lighting(&mut dev, 0x1F).unwrap();
        assert_eq!(p.regions.len(), 1);
        assert_eq!(p.effects[0].1[4], "wave");
    }
}
