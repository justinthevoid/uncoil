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
use uncoil_core::config::{Config, OpenRgbDevice, OpenRgbMode};

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

/// Is a process with this image name running (any session)?
pub(crate) fn running(image: &str) -> bool {
    system_tool("tasklist.exe")
        .args(["/FI", &format!("IMAGENAME eq {image}"), "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains(&image.to_lowercase()))
        .unwrap_or(false)
}

/// Close an OpenRGB running in this session (it would hold the devices); never another user's.
fn close_running_openrgb() {
    if running("OpenRGB.exe") {
        let session = format!("SESSION eq {}", crate::winsec::session_id());
        let _ = system_tool("taskkill.exe").args(["/F", "/IM", "OpenRGB.exe", "/FI", &session]).output();
        std::thread::sleep(std::time::Duration::from_secs(2));
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
/// per target. Entries that are not plain names are left out (and named in the returned problems); RAM
/// entries are left out while iCUE runs.
fn arguments(devices: &[OpenRgbDevice], config: &Path, icue: bool) -> (Vec<String>, Vec<String>) {
    let mut args = vec!["--noautoconnect".to_string(), "--config".into(), config.display().to_string()];
    let mut problems = vec![];
    for d in devices {
        if let Some(p) = d.problem() {
            problems.push(p);
            continue;
        }
        if d.ram && icue {
            continue;
        }
        args.extend(["-d".into(), d.name.clone(), "-m".into(), d.mode.clone()]);
    }
    (args, problems)
}

/// Run the hardware hand-off once. `Ok` carries a summary for the log; `Err` says why nothing (or not
/// everything) was done.
pub fn hand_off(cfg: &Config) -> Result<String, String> {
    if cfg.openrgb.devices.is_empty() {
        return Err("no devices in openrgb.devices; nothing to hand off".into());
    }
    let (exe, dir) = prerequisites()?;
    // RAM shares the SMBus with iCUE; never write it while iCUE is running.
    let icue = running("iCUE.exe");
    let (args, problems) = arguments(&cfg.openrgb.devices, &dir, icue);
    if args.len() == 3 {
        return Err(format!("nothing to hand off ({})", problems.join("; ")));
    }
    close_running_openrgb();
    let o = command(exe).args(&args).output().map_err(|e| format!("OpenRGB did not start: {e}"))?;
    let mut summary = format!(
        "OpenRGB hand-off applied (exit {:?}, RAM {})",
        o.status.code(),
        if icue { "skipped: iCUE running" } else { "included" }
    );
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

/// OpenRGB's `OpenRGB.json` for live mode: whatever is there already, plus every Razer detector off (uncoil
/// drives those devices itself) and the server on 127.0.0.1 at `port`.
fn settings(existing: Option<&str>, port: u16) -> serde_json::Value {
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
    for name in uncoil_openrgb::razer_detectors() {
        list.insert(name.into(), Value::Bool(false));
    }
    detectors.insert("detectors".into(), Value::Object(list));
    root["Detectors"] = Value::Object(detectors);
    let mut server = obj(root, "Server");
    server.insert("default_host".into(), json!("127.0.0.1"));
    server.insert("default_port".into(), json!(port));
    root["Server"] = Value::Object(server);
    root.clone()
}

/// Write `OpenRGB.json` into the admin-only folder (elevated: no links, no hard-linked files).
fn write_settings(dir: &Path, port: u16) -> Result<(), String> {
    let path = dir.join("OpenRGB.json");
    let existing = std::fs::read_to_string(&path).ok();
    let text = serde_json::to_string_pretty(&settings(existing.as_deref(), port)).unwrap_or_default();
    let tmp = dir.join("OpenRGB.json.uncoil-tmp");
    let written = crate::winsec::open_user_file(&tmp, false).and_then(|mut f| {
        use std::io::Write;
        f.set_len(0)?;
        f.write_all(text.as_bytes())
    });
    written.and_then(|()| std::fs::rename(&tmp, &path)).map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// Live mode: start OpenRGB as uncoil's SDK server and wait while it runs. `Ok` when it ran and exited
/// cleanly; `Err` says why it did not start or how it ended.
pub fn serve(cfg: &Config) -> Result<(), String> {
    let port = cfg.openrgb_live().valid_port().ok_or("openrgb.live.port must be 1024-65535")?;
    let (exe, dir) = prerequisites()?;
    write_settings(&dir, port)?;
    close_running_openrgb();
    let _ours = imp::Marker::create(OURS);
    let mut child =
        command(exe).args(server_arguments(&dir, port)).spawn().map_err(|e| format!("OpenRGB did not start: {e}"))?;
    // OpenRGB ends with this task: stopping the task (or this process dying) closes the job, which ends it
    let _job = imp::Job::kill_on_close(&child);
    let status = child.wait().map_err(|e| format!("waiting on OpenRGB failed: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("OpenRGB's SDK server ended (exit {:?})", status.code()))
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
        let (args, problems) = arguments(&list, dir, false);
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
        // RAM waits while iCUE runs
        let (args, _) = arguments(&list, dir, true);
        assert!(!args.contains(&"Vengeance".to_string()));
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
        let s = settings(Some(old), 6800);
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
            let s = settings(existing, 6742);
            assert_eq!(s["Detectors"]["detectors"]["Razer Blackwidow 2019"], false);
            assert_eq!(s["Server"]["default_host"], "127.0.0.1");
        }
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
