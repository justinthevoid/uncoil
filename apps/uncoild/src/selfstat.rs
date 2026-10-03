//! The daemon measures its own footprint so the GUI can show it as data (lightness is the pitch).

use std::time::Instant;

pub struct SelfStat {
    last_cpu_s: f64,
    last_at: Instant,
    pub exe_bytes: u64,
}

impl SelfStat {
    pub fn new() -> Self {
        let exe_bytes = std::env::current_exe().and_then(std::fs::metadata).map(|m| m.len()).unwrap_or(0);
        SelfStat { last_cpu_s: cpu_seconds(), last_at: Instant::now(), exe_bytes }
    }

    /// (private memory in bytes, CPU % of one core since the previous call)
    pub fn sample(&mut self) -> (u64, f32) {
        let cpu = cpu_seconds();
        let wall = self.last_at.elapsed().as_secs_f64().max(1e-3);
        let pct = ((cpu - self.last_cpu_s) / wall * 100.0) as f32;
        self.last_cpu_s = cpu;
        self.last_at = Instant::now();
        (private_bytes(), pct.max(0.0))
    }
}

#[cfg(windows)]
fn private_bytes() -> u64 {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut c as *mut _ as *mut _, c.cb) != 0 {
            c.PrivateUsage as u64
        } else {
            0
        }
    }
}

#[cfg(windows)]
fn cpu_seconds() -> f64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    unsafe {
        let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut c, mut e, mut k, mut u) = (z, z, z, z);
        if GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) == 0 {
            return 0.0;
        }
        let t = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64 / 1e7;
        t(k) + t(u)
    }
}

#[cfg(not(windows))]
fn private_bytes() -> u64 {
    0
}

#[cfg(not(windows))]
fn cpu_seconds() -> f64 {
    0.0
}
