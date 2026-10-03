//! Optional: hand non-Razer RGB (motherboard, GPU, RAM) to their own hardware rainbow via a one-shot
//! OpenRGB CLI run. uncoil doesn't drive those devices itself; OpenRGB exits right after.

use crate::log;
use std::process::Command;

const OPENRGB: &str = r"C:\Program Files\OpenRGB\OpenRGB.exe";
/// (OpenRGB device-name substring, hardware mode)
const TARGETS: &[(&str, &str)] = &[("ASUS ROG STRIX", "rainbow"), ("GeForce", "wave"), ("Vengeance", "rainbow wave")];

#[cfg(windows)]
const NO_WINDOW: u32 = 0x0800_0000;

fn command(exe: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(exe);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, NO_WINDOW);
    c
}

fn running(image: &str) -> bool {
    command("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {image}"), "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains(&image.to_lowercase()))
        .unwrap_or(false)
}

pub fn hardware_rainbow() {
    if !std::path::Path::new(OPENRGB).exists() {
        return;
    }
    if running("OpenRGB.exe") {
        let _ = command("taskkill").args(["/F", "/IM", "OpenRGB.exe"]).output();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    // RAM shares the SMBus with iCUE; never write it while iCUE is running.
    let icue = running("iCUE.exe");
    let mut args = vec!["--noautoconnect".to_string()];
    for (name, mode) in TARGETS {
        if *name == "Vengeance" && icue {
            continue;
        }
        args.extend(["-d".into(), name.to_string(), "-m".into(), mode.to_string()]);
    }
    match command(OPENRGB).args(&args).output() {
        Ok(o) => log::line(&format!(
            "openrgb hardware rainbow applied (exit {:?}, ram {})",
            o.status.code(),
            if icue { "skipped: iCUE running" } else { "yes" }
        )),
        Err(e) => log::line(&format!("openrgb failed: {e}")),
    }
}
