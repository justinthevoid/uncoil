//! Optional: hand non-Razer RGB (motherboard, GPU, RAM) to their own hardware modes with one OpenRGB CLI
//! run. uncoil doesn't drive those devices itself; OpenRGB exits right after.
//!
//! RAM lighting sits on the SMBus, which needs administrator rights, so the hand-off runs from its own
//! elevated one-shot task (`uncoild --openrgb-once`, registered by `install-task.ps1 -OpenRgb`) while the
//! daemon itself runs unelevated. Only an elevated daemon (the `-Elevated` fallback install) runs it itself.
//!
//! The device list comes from `config.json` (`openrgb.devices`), which any program running as the user can
//! write, so every name is checked to be a plain name (never an OpenRGB option), and OpenRGB gets an
//! administrators-only configuration folder (`%ProgramData%\uncoil\openrgb`) instead of the user's: an
//! elevated OpenRGB must not read settings from, or write them to, the user's profile.

use std::path::{Path, PathBuf};
use std::process::Command;
use uncoil_core::config::{Config, OpenRgbDevice};

const OPENRGB: &str = r"C:\Program Files\OpenRGB\OpenRGB.exe";

#[cfg(windows)]
const NO_WINDOW: u32 = 0x0800_0000;

fn command(exe: &Path) -> Command {
    #[allow(unused_mut)]
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
fn running(image: &str) -> bool {
    system_tool("tasklist.exe")
        .args(["/FI", &format!("IMAGENAME eq {image}"), "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains(&image.to_lowercase()))
        .unwrap_or(false)
}

/// The folder OpenRGB gets as its configuration directory.
pub fn config_dir() -> Option<PathBuf> {
    std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join("uncoil").join("openrgb"))
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

/// Run the hand-off once. `Ok` carries a summary for the log; `Err` says why nothing (or not everything)
/// was done.
pub fn hand_off(cfg: &Config) -> Result<String, String> {
    if cfg.openrgb.devices.is_empty() {
        return Err("no devices in openrgb.devices; nothing to hand off".into());
    }
    let exe = Path::new(OPENRGB);
    if !exe.exists() {
        return Err(format!("{OPENRGB} is not installed"));
    }
    let dir = config_dir().ok_or("ProgramData is not set")?;
    crate::winsec::admin_only_dir(&dir).map_err(|e| {
        format!("not running OpenRGB elevated without its own admin-only settings folder: {e} (run install-task.ps1 -OpenRgb)")
    })?;
    // RAM shares the SMBus with iCUE; never write it while iCUE is running.
    let icue = running("iCUE.exe");
    let (args, problems) = arguments(&cfg.openrgb.devices, &dir, icue);
    if args.len() == 3 {
        return Err(format!("nothing to hand off ({})", problems.join("; ")));
    }
    // a running OpenRGB would hold the devices: close the one in this session only, never another user's
    if running("OpenRGB.exe") {
        let session = format!("SESSION eq {}", crate::winsec::session_id());
        let _ = system_tool("taskkill.exe").args(["/F", "/IM", "OpenRGB.exe", "/FI", &session]).output();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
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

/// `uncoild --openrgb-once`: the elevated logon task. Writes nothing: the outcome is the exit code, which
/// Task Scheduler shows as the task's last result (0 done, 1 nothing configured or turned off, 2 failed).
pub fn once() -> i32 {
    if crate::winsec::init() {
        let _ = crate::winsec::enforce_redirection_trust();
    }
    let cfg = Config::load();
    if !cfg.openrgb_hardware_rainbow {
        return 1;
    }
    match hand_off(&cfg) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("uncoild --openrgb-once: {e}");
            if cfg.openrgb.devices.is_empty() {
                1
            } else {
                2
            }
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
}
