//! The Razer device lock: a named mutex, `Global\RazerLinkReadWriteGuardMutex`, that OpenRGB takes around
//! every report it sends to or reads from a Razer device (RazerDeviceGuard.cpp), and that Razer's own
//! software appears to share. uncoil takes it around each request and its reply, so two programs never
//! interleave reports on one device.
//!
//! Waits are short ([`WAIT`]): a frame whose lock is busy is skipped ([`RazerLock::for_frame`]); a command
//! tries twice and then fails with plain words ([`RazerLock::for_command`]). The lock is never held across
//! frames. Windows mutexes belong to the thread that took them and may be taken again by that thread, so
//! nested calls on one device thread are fine, and the device threads of one uncoild take turns too.

use anyhow::Result;
use std::sync::OnceLock;
use std::time::Duration;

/// The name OpenRGB uses.
pub const NAME: &str = r"Global\RazerLinkReadWriteGuardMutex";
/// Longest wait for the lock before a frame is skipped or a command tries again.
pub const WAIT: Duration = Duration::from_millis(25);

/// What happened when the lock was opened at start, for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// Created it, or opened the one another program created.
    Shared,
    /// Neither possible (or not Windows): uncoil runs without it.
    Without(String),
}

/// The process's handle on the lock (or none).
pub struct RazerLock {
    #[cfg(windows)]
    handle: Option<win::Handle>,
    opened: Opened,
}

/// Holding the lock; released when dropped, on the thread that took it (hence not `Send`).
pub struct Held<'a> {
    #[cfg(windows)]
    lock: Option<&'a win::Handle>,
    #[cfg(not(windows))]
    _lock: std::marker::PhantomData<&'a ()>,
    _not_send: std::marker::PhantomData<*const ()>,
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(h) = self.lock {
            h.release();
        }
    }
}

/// The lock shared by every device thread of this process, opened on first use.
pub fn razer() -> &'static RazerLock {
    static LOCK: OnceLock<RazerLock> = OnceLock::new();
    LOCK.get_or_init(|| RazerLock::open(NAME))
}

impl RazerLock {
    /// Create the named mutex, or open an existing one when it cannot be created (another program made it
    /// with rights that do not let this user create it); without either, run unlocked.
    pub fn open(name: &str) -> RazerLock {
        #[cfg(windows)]
        {
            let (handle, opened) = win::open(name);
            RazerLock { handle, opened }
        }
        #[cfg(not(windows))]
        {
            let _ = name;
            RazerLock { opened: Opened::Without("not Windows".into()) }
        }
    }

    pub fn opened(&self) -> &Opened {
        &self.opened
    }

    /// Wait up to `wait` for the lock. `None` on timeout. With no lock at all this always succeeds.
    pub fn acquire(&self, wait: Duration) -> Option<Held<'_>> {
        #[cfg(windows)]
        {
            let Some(h) = &self.handle else { return Some(self.unlocked()) };
            match h.wait(wait) {
                win::Wait::Held => Some(Held { lock: Some(h), _not_send: std::marker::PhantomData }),
                win::Wait::TimedOut => None,
                // the handle stopped working: carry on without the lock rather than stop the device
                win::Wait::Failed => Some(self.unlocked()),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = wait;
            Some(self.unlocked())
        }
    }

    fn unlocked(&self) -> Held<'_> {
        Held {
            #[cfg(windows)]
            lock: None,
            #[cfg(not(windows))]
            _lock: std::marker::PhantomData,
            _not_send: std::marker::PhantomData,
        }
    }

    /// Run one frame upload under the lock. `Ok(None)`: another program held it for [`WAIT`], so the frame
    /// was skipped (the next one comes soon).
    pub fn for_frame<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<Option<T>> {
        match self.acquire(WAIT) {
            Some(_held) => f().map(Some),
            None => Ok(None),
        }
    }

    /// Run one command's request and reply under the lock: wait [`WAIT`], try once more, then fail with
    /// plain words naming `device`.
    pub fn for_command<T>(&self, device: &str, f: impl FnOnce() -> Result<T>) -> Result<T> {
        for _ in 0..2 {
            if let Some(_held) = self.acquire(WAIT) {
                return f();
            }
        }
        Err(anyhow::Error::new(Busy(device.to_string())))
    }
}

/// A command gave up waiting for the lock: another program held it. Not a sign the device is gone.
#[derive(Debug)]
pub struct Busy(pub String);

impl std::fmt::Display for Busy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "another program is talking to {} right now (Razer's software and OpenRGB use the same device lock); \
             try again in a moment",
            self.0
        )
    }
}

impl std::error::Error for Busy {}

/// Did `e` come from waiting for the lock ([`Busy`]) rather than from the device?
pub fn is_busy(e: &anyhow::Error) -> bool {
    e.downcast_ref::<Busy>().is_some()
}

#[cfg(windows)]
mod win {
    use super::Opened;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ACCESS_DENIED, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::System::Threading::{CreateMutexW, OpenMutexW, ReleaseMutex, WaitForSingleObject};

    const SYNCHRONIZE: u32 = 0x0010_0000;
    const MUTEX_MODIFY_STATE: u32 = 0x0001;

    pub struct Handle(HANDLE);
    // a mutex handle may be used from any thread; ownership is per thread and handled by `Held`
    unsafe impl Send for Handle {}
    unsafe impl Sync for Handle {}

    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    pub enum Wait {
        Held,
        TimedOut,
        Failed,
    }

    impl Handle {
        pub fn wait(&self, wait: Duration) -> Wait {
            let ms = wait.as_millis().min(u32::MAX as u128 - 1) as u32;
            match unsafe { WaitForSingleObject(self.0, ms) } {
                // abandoned: its last owner exited without releasing it; the lock is ours now
                WAIT_OBJECT_0 | WAIT_ABANDONED => Wait::Held,
                WAIT_TIMEOUT => Wait::TimedOut,
                _ => Wait::Failed,
            }
        }

        pub fn release(&self) {
            unsafe { ReleaseMutex(self.0) };
        }
    }

    pub fn open(name: &str) -> (Option<Handle>, Opened) {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        unsafe {
            // default security: this user, SYSTEM and administrators can open it
            let h = CreateMutexW(std::ptr::null(), 0, wide.as_ptr());
            if !h.is_null() {
                return (Some(Handle(h)), Opened::Shared);
            }
            let created = GetLastError();
            let h = OpenMutexW(SYNCHRONIZE | MUTEX_MODIFY_STATE, 0, wide.as_ptr());
            if !h.is_null() {
                return (Some(Handle(h)), Opened::Shared);
            }
            let opened = GetLastError();
            let why = if created == ERROR_ACCESS_DENIED {
                format!("not allowed to create or open it (error {opened})")
            } else {
                format!("cannot create it (error {created}) or open it (error {opened})")
            };
            (None, Opened::Without(why))
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn test_lock(tag: &str) -> String {
        format!(r"Local\uncoil-test-guard-{tag}-{}", std::process::id())
    }

    #[test]
    fn a_held_lock_skips_frames_and_fails_commands_then_frees_up() {
        let name = test_lock("held");
        let lock = RazerLock::open(&name);
        assert_eq!(lock.opened(), &Opened::Shared);
        let (taken, take) = mpsc::channel();
        let (done, finish) = mpsc::channel::<()>();
        let holder = {
            let name = name.clone();
            std::thread::spawn(move || {
                // another program: its own handle on the same named mutex
                let other = RazerLock::open(&name);
                let held = other.acquire(WAIT).unwrap();
                taken.send(()).unwrap();
                finish.recv().unwrap();
                drop(held);
            })
        };
        take.recv().unwrap();
        let t = std::time::Instant::now();
        let mut ran = false;
        let frame = lock.for_frame(|| {
            ran = true;
            Ok(())
        });
        assert_eq!(frame.unwrap(), None, "the frame is skipped");
        assert!(!ran);
        let err = lock.for_command("Razer Basilisk V3 Pro", || Ok(())).unwrap_err().to_string();
        assert!(err.contains("another program is talking to Razer Basilisk V3 Pro"), "{err}");
        let busy = lock.for_command("x", || Ok(())).unwrap_err().context("reading DPI");
        assert!(is_busy(&busy), "still recognisable under context");
        assert!(!is_busy(&anyhow::anyhow!("no reply")));
        // one frame wait plus two command waits, with Windows' timer granularity: well under a second
        assert!(t.elapsed() < Duration::from_millis(500), "{:?}", t.elapsed());
        done.send(()).unwrap();
        holder.join().unwrap();
        assert_eq!(lock.for_frame(|| Ok(7)).unwrap(), Some(7));
        assert_eq!(lock.for_command("x", || Ok(8)).unwrap(), 8);
    }

    #[test]
    fn the_owner_may_take_it_again_and_an_abandoned_lock_is_taken_over() {
        let name = test_lock("again");
        let lock = RazerLock::open(&name);
        let outer = lock.acquire(WAIT).unwrap();
        assert!(lock.acquire(Duration::ZERO).is_some(), "the same thread nests");
        drop(outer);
        let name2 = name.clone();
        // a thread that exits while holding it abandons it
        std::thread::spawn(move || std::mem::forget(RazerLock::open(&name2).acquire(WAIT).unwrap())).join().unwrap();
        assert!(lock.acquire(WAIT).is_some());
    }
}
