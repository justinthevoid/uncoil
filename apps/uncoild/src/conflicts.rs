//! Other programs that drive the same devices: Razer's own software, OpenRGB, SignalRGB. Two programs sending
//! lighting to one device make it flicker between them, and key map or DPI writes can land on top of each
//! other. The daemon looks for them at every device rescan and publishes what it finds as
//! `status.conflicts` (the app shows a notice).
//!
//! Privacy: only process image names are read (a Toolhelp process snapshot), compared against the table
//! below, and only the matching program's name is kept or logged. Nothing else about other processes is
//! looked at.

use uncoil_core::ipc::Conflict;

const SYNAPSE: &str = "Razer Synapse";
const CHROMA: &str = "Razer Chroma";
const OPENRGB: &str = "OpenRGB";
const SIGNALRGB: &str = "SignalRGB";

/// Executable name → program. Names seen in public process listings (Synapse 3 and its services, the
/// Chroma SDK services, Razer Central) or, for Synapse 4, its log folder `RazerAppEngine`.
const KNOWN: &[(&str, &str)] = &[
    ("RazerAppEngine.exe", SYNAPSE),
    ("Razer Synapse 3.exe", SYNAPSE),
    ("Razer Synapse Service.exe", SYNAPSE),
    ("Razer Synapse Service Process.exe", SYNAPSE),
    ("RazerCentralService.exe", SYNAPSE),
    ("RzSDKService.exe", CHROMA),
    ("RzSDKServer.exe", CHROMA),
    ("RzChromaStreamServer.exe", CHROMA),
    ("OpenRGB.exe", OPENRGB),
    ("SignalRgb.exe", SIGNALRGB),
];

fn detail(app: &str) -> &'static str {
    match app {
        SYNAPSE => {
            "Razer Synapse is running. Two programs driving the same devices fight over them; quit Synapse or \
             turn off its start-up entry."
        }
        CHROMA => {
            "Razer's Chroma SDK service is running. It lets games and Synapse change the lighting of Razer \
             devices, so colours can flicker between it and uncoil; stop the Razer Chroma SDK services or \
             uninstall them."
        }
        OPENRGB => {
            "OpenRGB is running. If it also controls your Razer devices, the two fight over them; turn off its \
             Razer devices or quit it."
        }
        _ => {
            "SignalRGB is running. Two programs driving the same devices fight over them; quit SignalRGB or \
             stop it from starting with Windows."
        }
    }
}

/// The conflicts among running process image names, one per program, in table order. OpenRGB is left out
/// when it is uncoil's own live OpenRGB server (`openrgb_ours`): it drives only the PC's other devices then.
pub fn found<'a>(names: impl IntoIterator<Item = &'a str>, openrgb_ours: bool) -> Vec<Conflict> {
    let mut apps: Vec<&str> = vec![];
    for name in names {
        if let Some((_, app)) = KNOWN.iter().find(|(exe, _)| exe.eq_ignore_ascii_case(name)) {
            if !apps.contains(app) && !(*app == OPENRGB && openrgb_ours) {
                apps.push(app);
            }
        }
    }
    let order = |app: &str| KNOWN.iter().position(|(_, a)| *a == app);
    apps.sort_by_key(|a| order(a));
    apps.into_iter().map(|app| Conflict { app: app.into(), detail: detail(app).into() }).collect()
}

/// Look at the running processes now.
pub fn scan(openrgb_ours: bool) -> Vec<Conflict> {
    let names = process_names();
    found(names.iter().map(String::as_str), openrgb_ours)
}

/// Image names of the running processes (empty if the snapshot fails).
#[cfg(windows)]
fn process_names() -> Vec<String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    let mut out = vec![];
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e);
        while ok != 0 {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            out.push(String::from_utf16_lossy(&e.szExeFile[..len]));
            ok = Process32NextW(snap, &mut e);
        }
        CloseHandle(snap);
    }
    out
}

#[cfg(not(windows))]
fn process_names() -> Vec<String> {
    vec![]
}

/// Which conflicts were already logged, so each is logged once when it appears (and again only after it
/// went away and came back).
#[derive(Default)]
pub struct Seen(Vec<String>);

impl Seen {
    /// Log lines for the conflicts in `now` that were not there last time.
    pub fn update(&mut self, now: &[Conflict]) -> Vec<String> {
        let lines = now
            .iter()
            .filter(|c| !self.0.contains(&c.app))
            .map(|c| format!("{} is running and may fight uncoil over the devices", c.app))
            .collect();
        self.0 = now.iter().map(|c| c.app.clone()).collect();
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_names_become_one_conflict_per_program() {
        let names =
            ["explorer.exe", "razer synapse 3.EXE", "RazerCentralService.exe", "SignalRgb.exe", "RzSDKServer.exe"];
        let c = found(names, false);
        let apps: Vec<&str> = c.iter().map(|c| c.app.as_str()).collect();
        assert_eq!(apps, [SYNAPSE, CHROMA, SIGNALRGB]);
        assert!(c[0].detail.starts_with("Razer Synapse is running."));
        assert!(found(["notepad.exe", "Razer Synapse.exe.bak"], false).is_empty());
    }

    #[test]
    fn uncoils_own_openrgb_server_is_not_a_conflict() {
        assert_eq!(found(["OpenRGB.exe"], false)[0].app, OPENRGB);
        assert!(found(["OpenRGB.exe"], true).is_empty());
    }

    #[test]
    fn each_conflict_is_logged_once_when_it_appears() {
        let mut seen = Seen::default();
        let synapse = found(["RazerAppEngine.exe"], false);
        assert_eq!(seen.update(&synapse), ["Razer Synapse is running and may fight uncoil over the devices"]);
        assert!(seen.update(&synapse).is_empty());
        assert!(seen.update(&[]).is_empty());
        assert_eq!(seen.update(&synapse).len(), 1, "back again: logged again");
    }

    #[cfg(windows)]
    #[test]
    fn the_snapshot_sees_this_test() {
        let me = std::env::current_exe().unwrap();
        let name = me.file_name().unwrap().to_string_lossy().into_owned();
        assert!(process_names().iter().any(|n| n.eq_ignore_ascii_case(&name)), "{name}");
    }
}
