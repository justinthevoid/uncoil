//! System audio level for the audio meter effect: the default playback device's peak meter
//! (WASAPI `IAudioMeterInformation::GetPeakValue`). No audio samples are captured or read; Windows
//! reports one number, the recent peak, 0..1.
//!
//! The COM calls go through hand-written vtables (three methods) instead of the `windows` crate, which
//! would add several hundred KB to the daemon for the same three calls.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// A running peak-meter poller. Dropping it stops the thread and resets the level to 0.
pub struct AudioMeter {
    stop: Arc<AtomicBool>,
    level: Arc<AtomicU32>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl AudioMeter {
    /// Poll every `interval`, writing the level (f32 bits, 0..1) to `level`. Falls back to 0 while there is
    /// no playback device. The level decays gently between peaks so the meter doesn't flicker.
    pub fn start(interval: Duration, level: Arc<AtomicU32>) -> AudioMeter {
        let stop = Arc::new(AtomicBool::new(false));
        let (s, l) = (stop.clone(), level.clone());
        let handle = std::thread::Builder::new().name("audio-meter".into()).spawn(move || poll(interval, s, l)).ok();
        AudioMeter { stop, level, handle }
    }
}

impl Drop for AudioMeter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.level.store(0f32.to_bits(), Ordering::Relaxed);
    }
}

/// How fast the level falls after a peak, per second.
const DECAY_PER_S: f32 = 1.8;
/// Re-open the default device this often, so switching speakers / headphones is followed.
const REOPEN: Duration = Duration::from_secs(5);

fn poll(interval: Duration, stop: Arc<AtomicBool>, level: Arc<AtomicU32>) {
    let interval = interval.max(Duration::from_millis(10));
    let fall = DECAY_PER_S * interval.as_secs_f32();
    let mut meter = platform::Meter::open();
    let mut opened = std::time::Instant::now();
    let mut shown = 0.0f32;
    while !stop.load(Ordering::Relaxed) {
        if opened.elapsed() >= REOPEN {
            meter = platform::Meter::open();
            opened = std::time::Instant::now();
        }
        let peak = match meter.as_ref().map(|m| m.peak()) {
            Some(Some(p)) => p,
            Some(None) => {
                meter = None; // device went away; try again at the next reopen
                0.0
            }
            None => 0.0,
        };
        shown = peak.clamp(0.0, 1.0).max(shown - fall);
        level.store(shown.to_bits(), Ordering::Relaxed);
        std::thread::sleep(interval);
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use windows_sys::core::{GUID, HRESULT};
    use windows_sys::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

    const CLSID_MM_DEVICE_ENUMERATOR: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
    const IID_IMM_DEVICE_ENUMERATOR: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
    const IID_IAUDIO_METER_INFORMATION: GUID = GUID::from_u128(0xc02216f6_8c67_4b5b_9d00_d008e73e0064);
    const E_RENDER: i32 = 0;
    const E_CONSOLE: i32 = 0;

    #[repr(C)]
    struct IUnknownVtbl {
        query_interface: usize,
        add_ref: usize,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
    }
    #[repr(C)]
    struct EnumeratorVtbl {
        base: IUnknownVtbl,
        enum_audio_endpoints: usize,
        get_default_audio_endpoint: unsafe extern "system" fn(*mut c_void, i32, i32, *mut *mut c_void) -> HRESULT,
    }
    #[repr(C)]
    struct DeviceVtbl {
        base: IUnknownVtbl,
        activate: unsafe extern "system" fn(*mut c_void, *const GUID, u32, *const c_void, *mut *mut c_void) -> HRESULT,
    }
    #[repr(C)]
    struct MeterVtbl {
        base: IUnknownVtbl,
        get_peak_value: unsafe extern "system" fn(*mut c_void, *mut f32) -> HRESULT,
    }

    /// An owned COM interface pointer; released on drop.
    struct Com(*mut c_void);

    impl Com {
        unsafe fn vtbl<T>(&self) -> &T {
            &**(self.0 as *mut *const T)
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            unsafe { (self.vtbl::<IUnknownVtbl>().release)(self.0) };
        }
    }

    pub struct Meter(Com);

    impl Meter {
        /// The default render endpoint's meter, or `None` (no playback device, COM unavailable).
        pub fn open() -> Option<Meter> {
            unsafe {
                // S_FALSE (already initialised on this thread) is fine; the thread lives as long as the poller
                CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32);
                let mut p = std::ptr::null_mut();
                if CoCreateInstance(
                    &CLSID_MM_DEVICE_ENUMERATOR,
                    std::ptr::null_mut(),
                    CLSCTX_ALL,
                    &IID_IMM_DEVICE_ENUMERATOR,
                    &mut p,
                ) < 0
                    || p.is_null()
                {
                    return None;
                }
                let enumerator = Com(p);
                let mut p = std::ptr::null_mut();
                let hr = (enumerator.vtbl::<EnumeratorVtbl>().get_default_audio_endpoint)(
                    enumerator.0,
                    E_RENDER,
                    E_CONSOLE,
                    &mut p,
                );
                if hr < 0 || p.is_null() {
                    return None;
                }
                let device = Com(p);
                let mut p = std::ptr::null_mut();
                let hr = (device.vtbl::<DeviceVtbl>().activate)(
                    device.0,
                    &IID_IAUDIO_METER_INFORMATION,
                    CLSCTX_ALL,
                    std::ptr::null(),
                    &mut p,
                );
                if hr < 0 || p.is_null() {
                    return None;
                }
                Some(Meter(Com(p)))
            }
        }

        /// Recent peak, 0..1; `None` if the device stopped answering.
        pub fn peak(&self) -> Option<f32> {
            let mut v = 0.0f32;
            let hr = unsafe { (self.0.vtbl::<MeterVtbl>().get_peak_value)(self.0 .0, &mut v) };
            (hr >= 0).then_some(v)
        }
    }
}

#[cfg(not(windows))]
mod platform {
    pub struct Meter;
    impl Meter {
        pub fn open() -> Option<Meter> {
            None
        }
        pub fn peak(&self) -> Option<f32> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opens the real default playback device's meter and reads it; on a machine with no audio device the
    /// meter simply reads 0. No samples are captured either way.
    #[test]
    fn meter_starts_reads_and_resets_on_drop() {
        let level = Arc::new(AtomicU32::new(0));
        let m = AudioMeter::start(Duration::from_millis(10), level.clone());
        std::thread::sleep(Duration::from_millis(60));
        let v = f32::from_bits(level.load(Ordering::Relaxed));
        assert!((0.0..=1.0).contains(&v), "{v}");
        drop(m);
        assert_eq!(f32::from_bits(level.load(Ordering::Relaxed)), 0.0);
    }

    /// Needs a playback device (not guaranteed on CI): `cargo test -p uncoil-hid -- --ignored`.
    #[test]
    #[ignore]
    fn default_playback_device_meter_opens_and_reads() {
        let m = platform::Meter::open().expect("no default playback device");
        let peak = m.peak().expect("meter did not answer");
        assert!((0.0..=1.0).contains(&peak), "{peak}");
    }
}
