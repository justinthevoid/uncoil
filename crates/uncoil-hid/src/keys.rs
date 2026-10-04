//! Key press notifications from Windows Raw Input, for the reactive and ripple effects.
//!
//! PRIVACY: this module hands each key-down / key-up to a callback as a scan code and immediately forgets
//! it. It logs nothing, stores nothing and keeps no history. The daemon's callback turns the code into a
//! desk position on the spot (see `apps/uncoild/src/inputs.rs`). The listener only exists while the
//! active effect needs key presses; dropping it unregisters the device and ends its thread.

/// A running key listener. Dropping it unregisters Raw Input and stops its thread.
pub struct KeyListener {
    #[cfg(windows)]
    thread_id: u32,
    #[cfg(windows)]
    handle: Option<std::thread::JoinHandle<()>>,
}

/// Called for every key event: `make` = scan code, `e0` = extended-key prefix, `down` = pressed (false =
/// released). Runs on the listener thread.
pub type KeyCallback = Box<dyn FnMut(u16, bool, bool) + Send>;

impl KeyListener {
    /// Register for keyboard Raw Input (`RIDEV_INPUTSINK`, so presses arrive whichever window has focus) on
    /// a message-only window. `None` if Windows refused.
    #[cfg(windows)]
    pub fn start(callback: KeyCallback) -> Option<KeyListener> {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle =
            std::thread::Builder::new().name("key-listener".into()).spawn(move || win::run(callback, tx)).ok()?;
        match rx.recv() {
            Ok(Some(thread_id)) => Some(KeyListener { thread_id, handle: Some(handle) }),
            _ => {
                let _ = handle.join();
                None
            }
        }
    }

    #[cfg(not(windows))]
    pub fn start(_callback: KeyCallback) -> Option<KeyListener> {
        None
    }
}

impl Drop for KeyListener {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
            unsafe {
                PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
            }
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
        }
    }
}

#[cfg(windows)]
mod win {
    use super::KeyCallback;
    use std::cell::RefCell;
    use std::sync::mpsc::Sender;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::{
        GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_INPUTSINK,
        RIDEV_REMOVE, RID_INPUT, RIM_TYPEKEYBOARD,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, RegisterClassW, HWND_MESSAGE,
        MSG, RI_KEY_BREAK, RI_KEY_E0, RI_KEY_E1, WM_INPUT, WNDCLASSW,
    };

    thread_local! {
        static CALLBACK: RefCell<Option<KeyCallback>> = const { RefCell::new(None) };
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_INPUT {
            let mut raw: RAWINPUT = std::mem::zeroed();
            let mut size = std::mem::size_of::<RAWINPUT>() as u32;
            let got = GetRawInputData(
                lparam as HRAWINPUT,
                RID_INPUT,
                &mut raw as *mut RAWINPUT as *mut _,
                &mut size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            );
            if got != u32::MAX
                && got as usize >= std::mem::size_of::<RAWINPUTHEADER>()
                && raw.header.dwType == RIM_TYPEKEYBOARD
            {
                let kb = raw.data.keyboard;
                let flags = kb.Flags as u32;
                // 0xFF = keyboard overrun; E1 = Pause's odd sequence (no lighting needs it)
                if kb.MakeCode != 0xFF && flags & RI_KEY_E1 == 0 {
                    CALLBACK.with(|c| {
                        if let Some(f) = c.borrow_mut().as_mut() {
                            f(kb.MakeCode, flags & RI_KEY_E0 != 0, flags & RI_KEY_BREAK == 0);
                        }
                    });
                }
            }
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    fn device(flags: u32, hwnd: HWND) -> RAWINPUTDEVICE {
        // generic desktop page, keyboard usage
        RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x06, dwFlags: flags, hwndTarget: hwnd }
    }

    pub fn run(callback: KeyCallback, ready: Sender<Option<u32>>) {
        CALLBACK.with(|c| *c.borrow_mut() = Some(callback));
        unsafe {
            let class = wide("UncoilKeyListener");
            let hinst = GetModuleHandleW(std::ptr::null());
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(wndproc);
            wc.hInstance = hinst;
            wc.lpszClassName = class.as_ptr();
            // fails harmlessly when a previous listener already registered the class
            RegisterClassW(&wc);
            let title = wide("uncoil keys");
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
                let _ = ready.send(None);
                return;
            }
            let dev = device(RIDEV_INPUTSINK, hwnd);
            if RegisterRawInputDevices(&dev, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) == 0 {
                DestroyWindow(hwnd);
                let _ = ready.send(None);
                return;
            }
            let _ = ready.send(Some(GetCurrentThreadId()));
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
            let gone = device(RIDEV_REMOVE, std::ptr::null_mut());
            RegisterRawInputDevices(&gone, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
            DestroyWindow(hwnd);
        }
        CALLBACK.with(|c| *c.borrow_mut() = None);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Registers for keyboard Raw Input and unregisters again, twice (the window class is reused). The
    /// callback discards everything; no key is looked at.
    #[test]
    fn registers_and_stops_cleanly() {
        for _ in 0..2 {
            let l = KeyListener::start(Box::new(|_, _, _| {})).expect("raw input registration");
            drop(l);
        }
    }
}
