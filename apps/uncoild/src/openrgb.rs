//! Optional: non-Razer RGB (motherboard, GPU, RAM) through OpenRGB, in one of two modes (`openrgb.mode`):
//!
//! - `hardware`: one OpenRGB CLI run puts each device in `openrgb.devices` on its own hardware mode; OpenRGB
//!   exits right after and uncoil doesn't drive those devices.
//! - `live`: OpenRGB runs as an SDK server on 127.0.0.1 and the daemon sends it the desk effect frame by frame
//!   (`openrgb_live.rs`). This file starts that server and keeps it running.
//!
//! RAM lighting sits on the SMBus, which needs administrator rights, so both run from their own elevated task
//! (`uncoild --openrgb-once`, registered by `install-task.ps1 -OpenRgb`) while the daemon itself runs
//! unelevated. Only an elevated daemon (the `-Elevated` fallback install) does it itself.
//!
//! `config.json` is writable by any program running as the user, so every name is checked to be a plain name
//! (never an OpenRGB option), the port is a number, and OpenRGB gets an administrators-only configuration
//! folder (`%ProgramData%\uncoil\openrgb`) instead of the user's: an elevated OpenRGB must not read settings
//! from, or write them to, the user's profile.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use uncoil_core::config::{Config, OpenRgbDevice, OpenRgbLive, OpenRgbMode};
use uncoil_core::owners::{self, Owner, Part};

const OPENRGB: &str = r"C:\Program Files\OpenRGB\OpenRGB.exe";

/// Exists while uncoil's own OpenRGB server runs: the elevated task holds it open while it waits on OpenRGB,
/// so it goes away with the task (see [`ours`]).
const OURS: &str = r"Global\uncoil-openrgb-server";

#[cfg(windows)]
const NO_WINDOW: u32 = 0x0800_0000;

fn command(exe: &Path) -> Command {
    let mut c = Command::new(exe);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, NO_WINDOW);
    c
}

/// `tasklist` / `taskkill` from the system folder, never from the search path.
fn system_tool(name: &str) -> Command {
    command(&crate::winsec::system_dir().join(name))
}

pub(crate) use crate::conflicts::running;

/// Close an OpenRGB running in this session (it would hold the devices); never another user's, and not one
/// in session 0, such as OpenRGB's own Windows service ([`serve`] refuses to start beside those).
fn close_running_openrgb() {
    if running("OpenRGB.exe") {
        let session = format!("SESSION eq {}", crate::winsec::session_id());
        let _ = system_tool("taskkill.exe").args(["/F", "/IM", "OpenRGB.exe", "/FI", &session]).output();
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// The folder OpenRGB gets as its configuration directory.
pub fn config_dir() -> Option<PathBuf> {
    std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join("uncoil").join("openrgb"))
}

/// OpenRGB itself and its admin-only settings folder, or why not.
fn prerequisites() -> Result<(&'static Path, PathBuf), String> {
    let exe = Path::new(OPENRGB);
    if !exe.exists() {
        return Err(format!("{OPENRGB} is not installed"));
    }
    let dir = config_dir().ok_or("ProgramData is not set")?;
    crate::winsec::admin_only_dir(&dir).map_err(|e| {
        format!("not running OpenRGB elevated without its own admin-only settings folder: {e} (run install-task.ps1 -OpenRgb)")
    })?;
    Ok((exe, dir))
}

/// The OpenRGB arguments for these targets: `--noautoconnect --config DIR` and one `-d NAME -m MODE` pair
/// per target. Entries that are not plain names are left out (and named in the returned problems), and so
/// are entries a running program lights itself (`uncoil_core::owners`; RAM entries count as RAM, which
/// shares the SMBus with every stick), named in the third list with who has them.
fn arguments<S: AsRef<str>>(
    devices: &[OpenRgbDevice],
    config: &Path,
    processes: &[S],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut args = vec!["--noautoconnect".to_string(), "--config".into(), config.display().to_string()];
    let (mut problems, mut held) = (vec![], vec![]);
    for d in devices {
        if let Some(p) = d.problem() {
            problems.push(p);
            continue;
        }
        let part = Part { kind: if d.ram { "dram" } else { "other" }, name: &d.name, vendor: "" };
        if let Some(o) = owners::holder(owners::shipped(), processes, part) {
            held.push(format!("{} ({} has it)", d.name, o.name));
            continue;
        }
        args.extend(["-d".into(), d.name.clone(), "-m".into(), d.mode.clone()]);
    }
    (args, problems, held)
}

/// Run the hardware hand-off once. `Ok` carries a summary for the log; `Err` says why nothing (or not
/// everything) was done.
pub fn hand_off(cfg: &Config) -> Result<String, String> {
    if cfg.openrgb.devices.is_empty() {
        return Err("no devices in openrgb.devices; nothing to hand off".into());
    }
    let (exe, dir) = prerequisites()?;
    let (args, problems, held) = arguments(&cfg.openrgb.devices, &dir, &crate::conflicts::process_names());
    if args.len() == 3 {
        let why: Vec<String> = problems.into_iter().chain(held).collect();
        return Err(format!("nothing to hand off ({})", why.join("; ")));
    }
    close_running_openrgb();
    let o = command(exe).args(&args).output().map_err(|e| format!("OpenRGB did not start: {e}"))?;
    let mut summary = format!("OpenRGB hand-off applied (exit {:?})", o.status.code());
    for h in held {
        summary.push_str(&format!("; left {h}"));
    }
    for p in problems {
        summary.push_str(&format!("; skipped {p}"));
    }
    Ok(summary)
}

/// The OpenRGB arguments for live mode: a headless SDK server bound to 127.0.0.1 only, uncoil's settings
/// folder, and no attempt to join another OpenRGB. An OpenRGB too old to know `--server-host` refuses the
/// whole command line and exits, so it never listens anywhere else.
fn server_arguments(config: &Path, port: u16) -> Vec<String> {
    let port = port.to_string();
    ["--noautoconnect", "--config", &config.display().to_string(), "--server", "--server-host", "127.0.0.1"]
        .into_iter()
        .chain(["--server-port", port.as_str()])
        .map(String::from)
        .collect()
}

/// Where `OpenRGB.json` keeps the names of the detectors uncoil turned off for `openrgb.live.exclude`, so it
/// turns exactly those back on when the exclusion goes (and never one the user turned off in OpenRGB).
const EXCLUDED: &str = "uncoil_excluded_detectors";

/// OpenRGB's `OpenRGB.json` for live mode: whatever is there already, plus every Razer detector off (uncoil
/// drives those devices itself), every detector whose name `openrgb.live.exclude` matches off (so OpenRGB
/// never opens that device and its own software, such as iCUE, keeps it), likewise every detector a running
/// program in `holding` has (`uncoil_core::owners`, with `openrgb.live.pins`), and the server on 127.0.0.1
/// at the port. OpenRGB
/// writes its full detector list into the file on its first run; before that there is nothing to match.
fn settings(existing: Option<&str>, live: &OpenRgbLive, holding: &[&Owner]) -> serde_json::Value {
    use serde_json::{json, Map, Value};
    let mut root = existing.and_then(|t| serde_json::from_str::<Value>(t).ok()).filter(Value::is_object);
    let root = root.get_or_insert_with(|| json!({}));
    let obj = |v: &mut Value, key: &str| -> Map<String, Value> {
        match v.get(key) {
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        }
    };
    let mut detectors = obj(root, "Detectors");
    let mut list = match detectors.get("detectors") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    let razer: Vec<&str> = uncoil_openrgb::razer_detectors().collect();
    // the last run's exclusions on again, then this run's off
    let before = detectors.get(EXCLUDED).and_then(Value::as_array).cloned().unwrap_or_default();
    for name in before.iter().filter_map(Value::as_str) {
        if let Some(on @ Value::Bool(false)) = list.get_mut(name) {
            *on = Value::Bool(true);
        }
    }
    let mut excluded = Vec::new();
    for (name, on) in list.iter_mut() {
        let wanted_off = live.excludes(name) || owners::detector_held(owners::shipped(), holding, &live.pins, name);
        if *on == Value::Bool(true) && wanted_off && !razer.contains(&name.as_str()) {
            *on = Value::Bool(false);
            excluded.push(Value::String(name.clone()));
        }
    }
    for name in razer {
        list.insert(name.into(), Value::Bool(false));
    }
    detectors.insert("detectors".into(), Value::Object(list));
    if excluded.is_empty() {
        detectors.remove(EXCLUDED);
    } else {
        detectors.insert(EXCLUDED.into(), Value::Array(excluded));
    }
    root["Detectors"] = Value::Object(detectors);
    let mut server = obj(root, "Server");
    server.insert("default_host".into(), json!("127.0.0.1"));
    server.insert("default_port".into(), json!(live.port));
    root["Server"] = Value::Object(server);
    root.clone()
}

/// Write `OpenRGB.json` into the admin-only folder (elevated: no links, no hard-linked files). `Ok(true)` when
/// it changed.
fn write_settings(dir: &Path, live: &OpenRgbLive, holding: &[&Owner]) -> Result<bool, String> {
    let path = dir.join("OpenRGB.json");
    let existing = std::fs::read_to_string(&path).ok();
    let new = settings(existing.as_deref(), live, holding);
    if existing.as_deref().and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok()).as_ref() == Some(&new) {
        return Ok(false);
    }
    let text = serde_json::to_string_pretty(&new).unwrap_or_default();
    let tmp = dir.join("OpenRGB.json.uncoil-tmp");
    let written = crate::winsec::open_user_file(&tmp, false).and_then(|mut f| {
        use std::io::Write;
        f.set_len(0)?;
        f.write_all(text.as_bytes())
    });
    written
        .and_then(|()| std::fs::rename(&tmp, &path))
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(true)
}

/// How often the server task looks at `config.json` for new exclusions or a new port, and at the running
/// programs that light PC parts themselves.
const WATCH: Duration = Duration::from_secs(2);

/// The programs in `uncoil_core::owners` running now (one process snapshot), in table order.
fn running_owners() -> Vec<&'static Owner> {
    let processes = crate::conflicts::process_names();
    owners::shipped().iter().filter(|o| o.running(&processes)).collect()
}

/// Why not to start a server while an OpenRGB that [`close_running_openrgb`] could not close still runs.
const ANOTHER_OPENRGB: &str = "another OpenRGB is still running (OpenRGB's own Windows service, or another \
     user's); not starting a second one, since both would open the same devices and flicker. Stop it and set \
     the OpenRGB service to Manual";

/// Live mode: start OpenRGB as uncoil's SDK server and wait while it runs. When `openrgb.live` changes in a
/// way that changes OpenRGB's settings (an exclusion that turns a detector off or on, a new port), or a
/// program that lights PC parts itself starts or quits (seen on two looks in a row, so one starting up
/// restarts OpenRGB once), OpenRGB is restarted with them: a detector only takes effect when OpenRGB starts.
/// When `openrgb.mode` leaves `live`, OpenRGB is closed and this returns `Ok`, so the devices are free for
/// their own software. `Ok` also when it ran and exited cleanly; `Err` says why it did not start or how it
/// ended.
pub fn serve(cfg: &Config) -> Result<(), String> {
    let mut live = cfg.openrgb_live();
    live.valid_port().ok_or("openrgb.live.port must be 1024-65535")?;
    let (exe, dir) = prerequisites()?;
    let mut holding = running_owners();
    // programs seen on the last look when they differed from `holding`, waiting for a second look to agree
    let mut changing: Option<Vec<&Owner>> = None;
    write_settings(&dir, &live, &holding)?;
    close_running_openrgb();
    if running("OpenRGB.exe") {
        return Err(ANOTHER_OPENRGB.into());
    }
    let _ours = imp::Marker::create(OURS);
    loop {
        let port = live.valid_port().ok_or("openrgb.live.port must be 1024-65535")?;
        let mut child = command(exe)
            .args(server_arguments(&dir, port))
            .spawn()
            .map_err(|e| format!("OpenRGB did not start: {e}"))?;
        // OpenRGB ends with this task: stopping the task (or this process dying) closes the job, which ends it
        let _job = imp::Job::kill_on_close(&child);
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| format!("waiting on OpenRGB failed: {e}"))? {
                break Some(status);
            }
            std::thread::sleep(WATCH);
            // a half-saved or mistyped file keeps what OpenRGB runs with
            let Ok(now) = Config::reload() else { continue };
            if now.openrgb_mode() != OpenRgbMode::Live {
                eprintln!("uncoild --openrgb-once: openrgb.mode is no longer live; closing OpenRGB");
                let _ = child.kill();
                let _ = child.wait();
                return Ok(());
            }
            let next = Some(now.openrgb_live()).filter(|n| n.valid_port().is_some()).unwrap_or_else(|| live.clone());
            let seen = running_owners();
            let agreed = seen == holding || changing.as_ref() == Some(&seen);
            changing = (!agreed).then(|| seen.clone());
            let next_holding = if agreed { seen } else { holding.clone() };
            if next == live && next_holding == holding {
                continue;
            }
            live = next;
            holding = next_holding;
            if write_settings(&dir, &live, &holding)? {
                break None;
            }
        };
        match status {
            Some(s) if s.success() => return Ok(()),
            Some(s) => return Err(format!("OpenRGB's SDK server ended (exit {:?})", s.code())),
            None => {
                let names: Vec<&str> = holding.iter().map(|o| o.name.as_str()).collect();
                eprintln!(
                    "uncoild --openrgb-once: OpenRGB's settings changed (left to other programs: {}); restarting it",
                    if names.is_empty() { "none".into() } else { names.join(", ") }
                );
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// Is the OpenRGB that is running uncoil's own server (started by the `uncoil-openrgb` task, or by an
/// `-Elevated` daemon)? Cheap: one look for a named object. For the conflict check, which should not warn
/// about it. (Any program running as the user could fake this name; the worst it does is hide that notice.)
pub fn ours() -> bool {
    imp::Marker::exists(OURS)
}

/// `uncoild --openrgb-once`: the elevated logon task. The outcome is the exit code, which Task Scheduler
/// shows as the task's last result: 0 done (hardware: applied; live: OpenRGB ran and exited cleanly), 1 off or
/// nothing configured, 2 failed. In live mode it runs as long as OpenRGB does. It writes only OpenRGB's
/// settings file, in the admin-only folder.
pub fn once() -> i32 {
    if crate::winsec::init() {
        let _ = crate::winsec::enforce_redirection_trust();
    }
    let cfg = Config::load();
    let result = match cfg.openrgb_mode() {
        OpenRgbMode::Off => return 1,
        OpenRgbMode::Hardware if cfg.openrgb.devices.is_empty() => return 1,
        OpenRgbMode::Hardware => hand_off(&cfg).map(|_| ()),
        OpenRgbMode::Live => serve(&cfg),
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("uncoild --openrgb-once: {e}");
            2
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{CreateEventW, OpenEventW, SYNCHRONIZATION_SYNCHRONIZE};

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// A named event held open for as long as this value lives.
    pub struct Marker(HANDLE);

    impl Marker {
        pub fn create(name: &str) -> Option<Marker> {
            let h = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide(name).as_ptr()) };
            (!h.is_null()).then_some(Marker(h))
        }

        /// Does an object with this name exist? Opening one made by an elevated process is refused, which
        /// still says it exists.
        pub fn exists(name: &str) -> bool {
            let h = unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, wide(name).as_ptr()) };
            if h.is_null() {
                return unsafe { GetLastError() } == ERROR_ACCESS_DENIED;
            }
            unsafe { CloseHandle(h) };
            true
        }
    }

    impl Drop for Marker {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    /// A job that ends its process when the job's last handle closes (this process exiting or being stopped).
    pub struct Job(HANDLE);

    impl Job {
        pub fn kill_on_close(child: &std::process::Child) -> Option<Job> {
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) != 0
                    && AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) != 0;
                if !ok {
                    CloseHandle(job);
                    return None;
                }
                Some(Job(job))
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub struct Marker;
    impl Marker {
        pub fn create(_name: &str) -> Option<Marker> {
            None
        }
        pub fn exists(_name: &str) -> bool {
            false
        }
    }
    pub struct Job;
    impl Job {
        pub fn kill_on_close(_child: &std::process::Child) -> Option<Job> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, mode: &str, ram: bool) -> OpenRgbDevice {
        OpenRgbDevice { name: name.into(), mode: mode.into(), ram }
    }

    #[test]
    fn arguments_carry_only_plain_names() {
        let list = [
            dev("ASUS ROG STRIX", "rainbow", false),
            dev("--server", "x", false),
            dev("Vengeance", "rainbow wave", true),
        ];
        let dir = Path::new(r"C:\ProgramData\uncoil\openrgb");
        let (args, problems, held) = arguments(&list, dir, &["explorer.exe"]);
        assert!(held.is_empty());
        assert_eq!(
            args,
            [
                "--noautoconnect",
                "--config",
                r"C:\ProgramData\uncoil\openrgb",
                "-d",
                "ASUS ROG STRIX",
                "-m",
                "rainbow",
                "-d",
                "Vengeance",
                "-m",
                "rainbow wave"
            ]
        );
        assert_eq!(problems.len(), 1);
        // RAM waits while iCUE runs, and ASUS devices while Armoury Crate's lighting service does
        let (args, _, held) = arguments(&list, dir, &["iCUE.exe"]);
        assert!(!args.contains(&"Vengeance".to_string()));
        assert_eq!(held, ["Vengeance (Corsair iCUE has it)"]);
        let (args, _, held) = arguments(&list, dir, &["lightingservice.exe"]);
        assert_eq!(args.len(), 3, "{args:?}");
        assert_eq!(held, ["ASUS ROG STRIX (ASUS Armoury Crate has it)", "Vengeance (ASUS Armoury Crate has it)"]);
    }

    #[test]
    fn nothing_configured_does_nothing() {
        let e = hand_off(&Config { openrgb_hardware_rainbow: true, ..Config::default() }).unwrap_err();
        assert!(e.contains("no devices"), "{e}");
    }

    #[test]
    fn the_live_server_listens_on_localhost_only() {
        let dir = Path::new(r"C:\ProgramData\uncoil\openrgb");
        assert_eq!(
            server_arguments(dir, 6742),
            [
                "--noautoconnect",
                "--config",
                r"C:\ProgramData\uncoil\openrgb",
                "--server",
                "--server-host",
                "127.0.0.1",
                "--server-port",
                "6742"
            ]
        );
        let bad: Config = serde_json::from_str(r#"{"openrgb": {"mode": "live", "live": {"port": 80}}}"#).unwrap();
        assert!(serve(&bad).unwrap_err().contains("1024-65535"));
    }

    #[test]
    fn live_settings_turn_razer_detectors_off_and_keep_the_rest() {
        let old = r#"{"Detectors": {"detectors": {"Razer Blackwidow 2019": true, "ASUS Aura": true}},
                      "Server": {"default_host": "0.0.0.0", "legacy_workaround": true}, "Theme": {"theme": "dark"}}"#;
        let s = settings(Some(old), &OpenRgbLive { port: 6800, ..OpenRgbLive::default() }, &[]);
        let d = &s["Detectors"]["detectors"];
        assert_eq!(d["Razer Blackwidow 2019"], false);
        assert_eq!(d["ASUS Aura"], true, "other detectors are left as they were");
        assert_eq!(d.as_object().unwrap().len(), uncoil_openrgb::razer_detectors().count() + 1);
        assert_eq!(s["Server"]["default_host"], "127.0.0.1");
        assert_eq!(s["Server"]["default_port"], 6800);
        assert_eq!(s["Server"]["legacy_workaround"], true);
        assert_eq!(s["Theme"]["theme"], "dark");
        // nothing or junk there: a fresh file
        for existing in [None, Some("not json"), Some("[1, 2]")] {
            let s = settings(existing, &OpenRgbLive::default(), &[]);
            assert_eq!(s["Detectors"]["detectors"]["Razer Blackwidow 2019"], false);
            assert_eq!(s["Server"]["default_host"], "127.0.0.1");
        }
    }

    #[test]
    fn exclusions_turn_their_detectors_off_and_back_on() {
        let old = r#"{"Detectors": {"detectors": {"Corsair iCUE Link System Hub": true,
                      "Corsair Vengeance RGB DRAM": true, "ASUS Aura Motherboard": true, "Corsair Lighting Node": false}}}"#;
        let hub = OpenRgbLive { exclude: vec!["corsair icue link system hub".into()], ..OpenRgbLive::default() };
        let s = settings(Some(old), &hub, &[]);
        let d = &s["Detectors"]["detectors"];
        assert_eq!(d["Corsair iCUE Link System Hub"], false);
        assert_eq!(d["Corsair Vengeance RGB DRAM"], true);
        assert_eq!(s["Detectors"][EXCLUDED], serde_json::json!(["Corsair iCUE Link System Hub"]));
        // the same settings again change nothing
        assert_eq!(settings(Some(&s.to_string()), &hub, &[]), s);
        // a wider exclusion: every Corsair detector that was on, but not the Razer ones
        let corsair = OpenRgbLive { exclude: vec!["Corsair".into(), "razer".into()], ..OpenRgbLive::default() };
        let s = settings(Some(&s.to_string()), &corsair, &[]);
        let d = &s["Detectors"]["detectors"];
        assert_eq!(
            (&d["Corsair iCUE Link System Hub"], &d["Corsair Vengeance RGB DRAM"]),
            (&false.into(), &false.into())
        );
        assert_eq!(d["ASUS Aura Motherboard"], true);
        let excluded = s["Detectors"][EXCLUDED].as_array().unwrap();
        assert_eq!(excluded.len(), 2, "{excluded:?}");
        // no exclusions: what uncoil turned off is on again, and the one the user turned off stays off
        let s = settings(Some(&s.to_string()), &OpenRgbLive::default(), &[]);
        let d = &s["Detectors"]["detectors"];
        assert_eq!(
            (&d["Corsair iCUE Link System Hub"], &d["Corsair Vengeance RGB DRAM"]),
            (&true.into(), &true.into())
        );
        assert_eq!(d["Corsair Lighting Node"], false);
        assert_eq!(d["Razer Blackwidow 2019"], false);
        assert!(s["Detectors"].get(EXCLUDED).is_none());
    }

    #[test]
    fn a_running_program_gets_its_detectors_off_until_it_quits() {
        let old = r#"{"Detectors": {"detectors": {"Corsair iCUE Link System Hub": true, "Corsair DRAM": true,
                      "Kingston Fury DDR5 DRAM": true, "ASUS Aura Motherboard": true, "ASUS Aura Addressable": true,
                      "Corsair Lighting Node": false}}}"#;
        let icue: Vec<&Owner> = owners::shipped().iter().filter(|o| o.name == "Corsair iCUE").collect();
        let pump = OpenRgbLive { exclude: vec!["ASUS Aura Addressable".into()], ..OpenRgbLive::default() };
        let s = settings(Some(old), &pump, &icue);
        let d = &s["Detectors"]["detectors"];
        for off in ["Corsair iCUE Link System Hub", "Corsair DRAM", "Kingston Fury DDR5 DRAM", "ASUS Aura Addressable"]
        {
            assert_eq!(d[off], false, "{off}");
        }
        assert_eq!(d["ASUS Aura Motherboard"], true);
        assert_eq!(s["Detectors"][EXCLUDED].as_array().unwrap().len(), 4);
        // iCUE quits: its detectors come back, the exclusion stays, and the one the user turned off stays off
        let s = settings(Some(&s.to_string()), &pump, &[]);
        let d = &s["Detectors"]["detectors"];
        for on in ["Corsair iCUE Link System Hub", "Corsair DRAM", "Kingston Fury DDR5 DRAM", "ASUS Aura Motherboard"] {
            assert_eq!(d[on], true, "{on}");
        }
        assert_eq!((&d["ASUS Aura Addressable"], &d["Corsair Lighting Node"]), (&false.into(), &false.into()));
        assert_eq!(s["Detectors"][EXCLUDED], serde_json::json!(["ASUS Aura Addressable"]));
        // pins: the RAM detector stays on for uncoil, the ASUS board's goes off for iCUE
        let pinned = OpenRgbLive {
            pins: vec![
                uncoil_core::config::OpenRgbPin { name: "Corsair DRAM".into(), to: "uncoil".into() },
                uncoil_core::config::OpenRgbPin { name: "ASUS Aura Motherboard".into(), to: "Corsair iCUE".into() },
            ],
            ..OpenRgbLive::default()
        };
        let s = settings(Some(old), &pinned, &icue);
        let d = &s["Detectors"]["detectors"];
        assert_eq!((&d["Corsair DRAM"], &d["ASUS Aura Motherboard"]), (&true.into(), &false.into()));
        assert_eq!(d["Kingston Fury DDR5 DRAM"], false, "unpinned RAM is still iCUE's");
    }

    #[cfg(windows)]
    #[test]
    fn ours_follows_the_marker() {
        let name = format!(r"Local\uncoil-test-marker-{}", std::process::id());
        assert!(!imp::Marker::exists(&name));
        let m = imp::Marker::create(&name).unwrap();
        assert!(imp::Marker::exists(&name));
        drop(m);
        assert!(!imp::Marker::exists(&name));
    }
}
