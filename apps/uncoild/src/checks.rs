//! Read-only checks before writes. An experimental device (or a supported device's `unverified` features)
//! may not answer the way its definition expects, so before uncoil changes anything stored in it, it reads
//! the current value with the matching "get" command and checks the reply makes sense. One check per
//! feature, run on first need or by `check.run`, cached until the device disconnects (the cache lives on the
//! device's own thread). A failed or untested check makes that feature's writes fail; reads still work.
//! Lighting checks never block lighting; the firmware-effect check (the same regions read) blocks only
//! saving an effect to the device. Every check reads through `proto::query_read`, so it can send nothing but
//! "get" commands.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use uncoil_core::device::{DeviceDef, Kind};
use uncoil_core::features::info::DeviceDetails;
use uncoil_core::features::keymap::{self, Function, Layer};
use uncoil_core::features::performance as perf;
use uncoil_core::features::scroll::{self, Setting};
use uncoil_core::features::{dial, hw_effect, oled, power, profile, Feature};
use uncoil_core::ipc::{coded, codes, CheckState, FeatureCheck};
use uncoil_core::proto::{query_read as query_ok, Transport};

/// Check results for one connected device, plus what else is read once per connection.
pub struct Checks {
    states: BTreeMap<Feature, (CheckState, Option<String>)>,
    /// Copy of [`Checks::list`] for the control channel (`capabilities` is answered off the device thread).
    mirror: Arc<Mutex<Vec<FeatureCheck>>>,
    /// `info.get`: firmware and keyboard info do not change while the device stays plugged in.
    pub details: Option<DeviceDetails>,
}

/// What a device's checks look like before any has run: `untested` where a check is needed, else
/// `not_needed`. One entry per declared feature.
pub fn initial(def: &DeviceDef) -> Vec<FeatureCheck> {
    def.features
        .iter()
        .map(|&f| FeatureCheck {
            feature: f,
            state: if def.needs_check(f) { CheckState::Untested } else { CheckState::NotNeeded },
            detail: None,
        })
        .collect()
}

impl Checks {
    pub fn new(def: &DeviceDef) -> Checks {
        let list = initial(def);
        let states = list.iter().map(|c| (c.feature, (c.state, None))).collect();
        Checks { states, mirror: Arc::new(Mutex::new(list)), details: None }
    }

    pub fn mirror(&self) -> Arc<Mutex<Vec<FeatureCheck>>> {
        self.mirror.clone()
    }

    pub fn list(&self) -> Vec<FeatureCheck> {
        self.states
            .iter()
            .map(|(&feature, (state, detail))| FeatureCheck { feature, state: *state, detail: detail.clone() })
            .collect()
    }

    fn publish(&self) {
        *self.mirror.lock().unwrap() = self.list();
    }

    /// Run every needed check now (again, if it ran before).
    pub fn run_all(&mut self, t: &mut dyn Transport, def: &DeviceDef, tid: u8) -> Vec<FeatureCheck> {
        let needed: Vec<Feature> = self.states.keys().copied().filter(|f| def.needs_check(*f)).collect();
        for f in needed {
            self.run(t, def, tid, f);
        }
        self.publish();
        self.list()
    }

    fn run(&mut self, t: &mut dyn Transport, def: &DeviceDef, tid: u8, f: Feature) {
        let mut watch = LockWatch { t, busy: false };
        let (state, detail) = match check(&mut watch, def, tid, f) {
            Ok(d) => (CheckState::Passed, Some(d)),
            // another program held the Razer device lock: not a verdict on the device, so try again next time
            Err(d) if watch.busy => (CheckState::Untested, Some(d)),
            Err(d) => (CheckState::Failed, Some(d)),
        };
        self.states.insert(f, (state, detail));
    }

    /// Before a write for `f`: fine when no check is needed or it passed; an untested check runs now; a
    /// failed one refuses with `check_failed`. Lighting is never refused; `HwEffects` is asked for only when
    /// an effect is saved to the device (exec's `gated_features`), never for showing one.
    pub fn require(&mut self, t: &mut dyn Transport, def: &DeviceDef, tid: u8, f: Feature) -> anyhow::Result<()> {
        if !def.needs_check(f) || f == Feature::Lighting {
            return Ok(());
        }
        if matches!(self.states.get(&f), None | Some((CheckState::Untested, _))) {
            self.run(t, def, tid, f);
            self.publish();
        }
        match self.states.get(&f) {
            Some((CheckState::Passed | CheckState::NotNeeded, _)) => Ok(()),
            // the check could not run (the device lock was busy): say that, not that the device failed it
            Some((CheckState::Untested, Some(why))) => Err(anyhow::anyhow!("{why}")),
            Some((_, detail)) => Err(refusal(def, f, detail.as_deref().unwrap_or("not checked"))),
            None => Err(refusal(def, f, "not checked")),
        }
    }
}

/// Notes whether a read gave up waiting for the Razer device lock (`uncoil_hid::guard`).
struct LockWatch<'a> {
    t: &'a mut dyn Transport,
    busy: bool,
}

impl Transport for LockWatch<'_> {
    fn query(&mut self, request: &uncoil_core::proto::Report) -> anyhow::Result<uncoil_core::proto::Reply> {
        let r = self.t.query(request);
        if let Err(e) = &r {
            self.busy |= uncoil_hid::guard::is_busy(e);
        }
        r
    }
}

fn what(f: Feature) -> &'static str {
    match f {
        Feature::Keymap => "key mappings",
        Feature::Profiles => "profiles",
        Feature::Dpi => "DPI settings",
        Feature::PollRate => "poll rate",
        Feature::Power => "power settings",
        Feature::Dial => "dial settings",
        Feature::Oled => "display settings",
        Feature::Lighting => "lighting",
        Feature::HwEffects => "saved lighting effects",
        Feature::Scroll => "scroll wheel settings",
    }
}

fn refusal(def: &DeviceDef, f: Feature, detail: &str) -> anyhow::Error {
    let tail = if def.has(Feature::Lighting) { " Lighting still works." } else { "" };
    coded(
        codes::CHECK_FAILED,
        format!(
            "uncoil couldn't confirm {} answers the way it expects, so it won't change its {} ({detail}).{tail}",
            def.name,
            what(f)
        ),
    )
}

/// One read-only check: `Ok(what was read)` or `Err(why it failed)`.
fn check(t: &mut dyn Transport, def: &DeviceDef, tid: u8, f: Feature) -> Result<String, String> {
    let e = |x: anyhow::Error| format!("{x:#}");
    match f {
        Feature::Keymap => {
            let km = def.keymap.as_ref().ok_or("no [keymap] section")?;
            // (key id, what it must do)
            let expect: Vec<(u8, Vec<Function>)> = match def.kind {
                Kind::Mouse => {
                    vec![(1, vec![Function::MouseButton { button: 1 }]), (2, vec![Function::MouseButton { button: 2 }])]
                }
                _ => [(26u8, 0x13u8), (31, 0x04), (110, 0x29)]
                    .into_iter()
                    .filter_map(|(id, usage)| {
                        let k = km.by_id(id)?;
                        let mut ok = vec![Function::Key { modifiers: 0, usage }];
                        ok.extend(k.default_function());
                        Some((id, ok))
                    })
                    .collect(),
            };
            if expect.is_empty() {
                return Err("the key map lists none of the keys the check reads (P, A, Esc)".into());
            }
            let mut read = vec![];
            for (id, ok) in expect {
                let reply = query_ok(t, &km.get_report(tid, 1, id, Layer::Normal)).map_err(e)?;
                let got = keymap::parse_reply(&reply, 1, id).map_err(e)?;
                let name = km.name_of(id);
                if !ok.contains(&got) {
                    return Err(format!("{name} reads `{got}`, expected `{}`", ok[0]));
                }
                read.push(format!("{name} = {got}"));
            }
            Ok(read.join(", "))
        }
        Feature::Profiles => {
            let max = profile::parse_byte(&query_ok(t, &profile::get_max(tid)).map_err(e)?);
            let count = profile::parse_byte(&query_ok(t, &profile::get_count(tid)).map_err(e)?);
            if max == 0 || max > 16 || count > max {
                return Err(format!("profiles read {count} of {max}"));
            }
            Ok(format!("{count} of {max} profiles"))
        }
        Feature::Dpi => {
            let d = def.dpi.as_ref().ok_or("no [dpi] section")?;
            let dpi = perf::parse_dpi(&query_ok(t, &perf::get_dpi(tid, d.storage)).map_err(e)?).map_err(e)?;
            if !dpi.within(d.min, d.max) {
                return Err(format!("DPI reads {dpi}, outside {}-{}", d.min, d.max));
            }
            let mut out = format!("DPI {dpi}");
            if d.stages_max > 0 {
                let s = perf::parse_stages(&query_ok(t, &perf::get_stages(tid)).map_err(e)?).map_err(e)?;
                if s.list.len() > d.stages_max as usize || s.list.iter().any(|x| !x.within(d.min, d.max)) {
                    return Err(format!(
                        "DPI stages read {:?}",
                        s.list.iter().map(|x| x.to_string()).collect::<Vec<_>>()
                    ));
                }
                out.push_str(&format!(", {} stages, stage {} active", s.list.len(), s.active));
            }
            Ok(out)
        }
        Feature::PollRate => {
            let p = def.poll_rate.as_ref().ok_or("no [poll_rate] section")?;
            let hz = perf::parse_poll(&query_ok(t, &perf::get_poll(tid, p.kind)).map_err(e)?, p.kind).map_err(e)?;
            if !p.rates.contains(&hz) {
                return Err(format!("poll rate reads {hz} Hz, not one of {:?}", p.rates));
            }
            Ok(format!("{hz} Hz"))
        }
        Feature::Power => {
            let p = def.power.as_ref().ok_or("no [power] section")?;
            let mut read = vec![];
            if p.battery {
                let pct = power::parse_battery(&query_ok(t, &power::get_battery(tid)).map_err(e)?);
                let charging = power::parse_charging(&query_ok(t, &power::get_charging(tid)).map_err(e)?).map_err(e)?;
                read.push(format!("battery {pct}%{}", if charging { " (charging)" } else { "" }));
            }
            if p.idle {
                let s = power::parse_idle(&query_ok(t, &power::get_idle(tid)).map_err(e)?).map_err(e)?;
                read.push(format!("sleep after {s} s"));
            }
            if p.low_battery {
                let raw =
                    power::parse_low_battery(&query_ok(t, &power::get_low_battery(tid)).map_err(e)?).map_err(e)?;
                read.push(format!("low battery at {}%", power::raw_to_pct(raw)));
            }
            Ok(read.join(", "))
        }
        Feature::Dial => {
            let s = dial::parse_state(&query_ok(t, &dial::get_active_mode(tid, 1)).map_err(e)?);
            match s.mode {
                Some(m) => Ok(format!("dial mode {}", m.name())),
                None => Err(format!("dial reads unknown mode {}", s.mode_id)),
            }
        }
        Feature::Scroll => {
            let s = def.scroll.as_ref().ok_or("no [scroll] section")?;
            let mut state = scroll::ScrollState::default();
            let mut read = vec![];
            for setting in Setting::ALL.into_iter().filter(|x| s.has(*x)) {
                let v = scroll::parse(&query_ok(t, &scroll::get(tid, setting)).map_err(e)?, setting).map_err(e)?;
                state.apply(setting, v);
                read.push(format!("{} {}", setting.name(), state.words(setting)));
            }
            Ok(read.join(", "))
        }
        Feature::Oled => {
            let b = query_ok(t, &oled::get(tid, oled::BRIGHTNESS)).map_err(e)?.raw[0];
            if b > 100 {
                return Err(format!("display brightness reads {b}%"));
            }
            Ok(format!("display brightness {b}%"))
        }
        Feature::Lighting | Feature::HwEffects => {
            let m = def.matrix.as_ref().ok_or("no [matrix] section")?;
            let regions = hw_effect::parse_regions(&query_ok(t, &hw_effect::get_regions(tid)).map_err(e)?);
            let list: Vec<String> = regions.iter().map(|r| format!("led {} {}x{}", r.led, r.rows, r.cols)).collect();
            let same = regions.iter().any(|r| r.rows as usize == m.rows && r.cols as usize == m.cols);
            let strip = !regions.is_empty()
                && regions.iter().all(|r| r.rows as usize == m.rows)
                && regions.iter().map(|r| r.cols as usize).sum::<usize>() == m.cols;
            if same || strip {
                Ok(format!("regions {}", list.join(", ")))
            } else {
                Err(format!("regions {} do not add up to the file's {}x{} matrix", list.join(", "), m.rows, m.cols))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeDevice;
    use uncoil_core::device::builtin;

    #[test]
    fn supported_devices_need_no_checks_except_unverified_features() {
        let defs = builtin();
        let kb = defs.iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap();
        assert!(initial(kb).iter().all(|c| c.state == CheckState::NotNeeded));
        let mouse = defs.iter().find(|d| d.id == "razer-basilisk-v3-pro").unwrap();
        let states: BTreeMap<Feature, CheckState> = initial(mouse).into_iter().map(|c| (c.feature, c.state)).collect();
        assert_eq!(states[&Feature::Keymap], CheckState::NotNeeded);
        assert_eq!(states[&Feature::Dpi], CheckState::Untested);
        assert_eq!(states[&Feature::Power], CheckState::Untested);
    }

    #[test]
    fn checks_run_read_only_and_cache() {
        let def = builtin().into_iter().find(|d| d.id == "razer-basilisk-v3-pro").unwrap();
        let mut dev = FakeDevice::for_def(&def);
        let mut c = Checks::new(&def);
        let list = c.run_all(&mut dev, &def, 0x1F);
        let get = |f: Feature| list.iter().find(|x| x.feature == f).unwrap().clone();
        assert_eq!(get(Feature::Dpi).state, CheckState::Passed, "{:?}", get(Feature::Dpi));
        assert_eq!(get(Feature::PollRate).detail.as_deref(), Some("1000 Hz"));
        assert_eq!(get(Feature::Power).state, CheckState::Passed, "{:?}", get(Feature::Power));
        assert_eq!(
            get(Feature::Scroll).detail.as_deref(),
            Some("scroll mode tactile, scroll acceleration on, Smart Reel off")
        );
        assert_eq!(get(Feature::Keymap).state, CheckState::NotNeeded);
        assert!(dev.setters().is_empty(), "checks never write");
        assert_eq!(*c.mirror().lock().unwrap(), list);
        // passed: require sends nothing more
        let n = dev.sent().len();
        c.require(&mut dev, &def, 0x1F, Feature::Dpi).unwrap();
        assert_eq!(dev.sent().len(), n);
    }

    #[test]
    fn failed_check_refuses_with_code() {
        let def = builtin().into_iter().find(|d| d.id == "razer-basilisk-v3-pro").unwrap();
        let mut dev = FakeDevice::for_def(&def);
        dev.set_dpi(0, 0);
        let mut c = Checks::new(&def);
        let err = c.require(&mut dev, &def, 0x1F, Feature::Dpi).unwrap_err();
        let coded = err.downcast_ref::<uncoil_core::ipc::CodedError>().unwrap();
        assert_eq!(coded.code, codes::CHECK_FAILED);
        assert!(coded.message.contains("won't change its DPI settings"), "{}", coded.message);
        assert!(coded.message.ends_with("Lighting still works."));
        // lighting is never refused
        c.require(&mut dev, &def, 0x1F, Feature::Lighting).unwrap();
    }

    /// A fake whose reads give up on the Razer device lock while `busy` is set.
    struct Contended {
        dev: FakeDevice,
        busy: bool,
    }

    impl Transport for Contended {
        fn query(&mut self, r: &uncoil_core::proto::Report) -> anyhow::Result<uncoil_core::proto::Reply> {
            if self.busy {
                return Err(anyhow::Error::new(uncoil_hid::guard::Busy("Razer Basilisk V3 Pro".into())));
            }
            self.dev.query(r)
        }
    }

    #[test]
    fn a_busy_device_lock_leaves_the_check_untested_not_failed() {
        let def = builtin().into_iter().find(|d| d.id == "razer-basilisk-v3-pro").unwrap();
        let mut t = Contended { dev: FakeDevice::for_def(&def), busy: true };
        let mut c = Checks::new(&def);
        let e = c.require(&mut t, &def, 0x1F, Feature::Dpi).unwrap_err();
        assert!(e.to_string().starts_with("another program is talking to"), "{e}");
        assert!(e.downcast_ref::<uncoil_core::ipc::CodedError>().is_none(), "not a check_failed refusal");
        let state = |c: &Checks| c.list().into_iter().find(|x| x.feature == Feature::Dpi).unwrap().state;
        assert_eq!(state(&c), CheckState::Untested);
        // once the lock is free, the next write's check runs and passes
        t.busy = false;
        c.require(&mut t, &def, 0x1F, Feature::Dpi).unwrap();
        assert_eq!(state(&c), CheckState::Passed);
    }

    #[test]
    fn lighting_check_compares_regions_with_the_matrix() {
        let defs = builtin();
        let mouse = defs.iter().find(|d| d.id == "razer-basilisk-v3-pro").unwrap();
        let mut dev = FakeDevice::for_def(mouse);
        // the fake answers 1x1 + 1x1 + 1x11 like the real mouse: adds up to the 1x13 matrix
        assert!(check(&mut dev, mouse, 0x1F, Feature::Lighting).is_ok());
        let kb = defs.iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap();
        assert!(check(&mut dev, kb, 0x1F, Feature::Lighting).is_err(), "mouse regions do not fit the keyboard");
    }
}
