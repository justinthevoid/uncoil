//! Display power state from Windows (GUID_CONSOLE_DISPLAY_STATE), like Synapse's
//! "turn off lighting when the display turns off".

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayState {
    Off,
    On,
    Dimmed,
}

impl DisplayState {
    pub fn as_str(self) -> &'static str {
        match self {
            DisplayState::Off => "off",
            DisplayState::On => "on",
            DisplayState::Dimmed => "dimmed",
        }
    }
}

static STATE: AtomicU8 = AtomicU8::new(1);

pub fn current() -> DisplayState {
    match STATE.load(Ordering::Relaxed) {
        0 => DisplayState::Off,
        2 => DisplayState::Dimmed,
        _ => DisplayState::On,
    }
}

/// Start watching on a background thread. Windows posts the current state right after registering.
pub fn spawn_watcher() {
    #[cfg(windows)]
    std::thread::Builder::new().name("display-watcher".into()).spawn(win::run).expect("spawn display watcher");
}

#[cfg(windows)]
mod win {
    use super::STATE;
    use std::sync::atomic::Ordering;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Power::{RegisterPowerSettingNotification, POWERBROADCAST_SETTING};
    use windows_sys::Win32::System::SystemServices::GUID_CONSOLE_DISPLAY_STATE;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, TranslateMessage,
        DEVICE_NOTIFY_WINDOW_HANDLE, HWND_MESSAGE, MSG, PBT_POWERSETTINGCHANGE, WM_POWERBROADCAST, WNDCLASSW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_POWERBROADCAST && wparam as u32 == PBT_POWERSETTINGCHANGE && lparam != 0 {
            let pbs = &*(lparam as *const POWERBROADCAST_SETTING);
            let g = &pbs.PowerSetting;
            let want = &GUID_CONSOLE_DISPLAY_STATE;
            if g.data1 == want.data1 && g.data2 == want.data2 && g.data3 == want.data3 && g.data4 == want.data4 {
                STATE.store(pbs.Data[0], Ordering::Relaxed);
            }
            return 1;
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    pub fn run() {
        unsafe {
            let class = wide("UncoilDisplayWatcher");
            let hinst = GetModuleHandleW(std::ptr::null());
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(wndproc);
            wc.hInstance = hinst;
            wc.lpszClassName = class.as_ptr();
            if RegisterClassW(&wc) == 0 {
                return;
            }
            let title = wide("uncoil");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                hinst,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return;
            }
            if RegisterPowerSettingNotification(hwnd, &GUID_CONSOLE_DISPLAY_STATE, DEVICE_NOTIFY_WINDOW_HANDLE) == 0 {
                return;
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}
