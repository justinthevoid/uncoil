//! The Razer device lock: a named mutex, `Global\RazerLinkReadWriteGuardMutex`, that OpenRGB takes around
//! every report it sends to or reads from a Razer device (RazerDeviceGuard.cpp), and that Razer's own
//! software appears to share. uncoil takes it around each request and its reply, so two programs never
//! interleave reports on one device.
//!
//! The lock keeps other programs out; it should not make uncoil's own devices wait for each other (they are
//! separate USB devices). So one lock thread holds the mutex for the whole process, a turn ([`TURN`]) at a
//! time: during a turn every device thread sends in parallel, and between turns the lock thread lets go, so
//! a program waiting for the lock gets it. Taking the mutex per report on each device thread instead had the
//! BlackWidow's acknowledged rows keep it busy and cut the mouse and mat from 30 to about 22 fps.
//!
//! Waits are short ([`WAIT`]): a frame report that can't get a turn skips the rest of that frame
//! ([`RazerLock::for_frame`]); a command tries twice and then fails with plain words
//! ([`RazerLock::for_command`]). A thread that already holds a turn may take it again (nested calls).

use anyhow::Result;
use std::marker::PhantomData;
use std::sync::OnceLock;
use std::time::Duration;

/// The name OpenRGB uses.
pub const NAME: &str = r"Global\RazerLinkReadWriteGuardMutex";
/// Longest wait for the lock before a frame is skipped or a command tries again.
pub const WAIT: Duration = Duration::from_millis(25);
/// How long the process keeps the lock before letting another program have it (plus the report in flight).
pub const TURN: Duration = Duration::from_millis(20);

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
    turns: Option<std::sync::Arc<turns::Turns>>,
    opened: Opened,
}

/// Holding the lock (a share of the process's turn); let go when dropped, on the thread that took it (hence
/// not `Send`).
pub struct Held<'a> {
    #[cfg(windows)]
    turns: Option<&'a turns::Turns>,
    #[cfg(not(windows))]
    _lock: PhantomData<&'a ()>,
    _not_send: PhantomData<*const ()>,
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(t) = self.turns {
            t.leave();
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
            RazerLock { turns: handle.map(turns::Turns::start), opened }
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
            let Some(t) = &self.turns else { return Some(self.unlocked()) };
            t.enter(wait).then_some(Held { turns: Some(t.as_ref()), _not_send: PhantomData })
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
            turns: None,
            #[cfg(not(windows))]
            _lock: PhantomData,
            _not_send: PhantomData,
        }
    }

    /// Run one frame report under the lock. `Ok(None)`: another program held it for [`WAIT`], so the report
    /// was not sent (the caller skips the rest of the frame; the next one comes soon).
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

#[cfg(windows)]
impl Drop for RazerLock {
    fn drop(&mut self) {
        if let Some(t) = &self.turns {
            t.close();
        }
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

/// The lock thread and the turns it hands out. A Windows mutex belongs to the thread that took it, so one
/// thread takes and releases it; device threads only count themselves in and out of its turn.
#[cfg(windows)]
mod turns {
    use super::{win, TURN, WAIT};
    use std::cell::RefCell;
    use std::sync::{Arc, Condvar, Mutex, MutexGuard};
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct State {
        /// The lock thread holds the mutex (or runs without it after the handle failed).
        held: bool,
        /// The turn is over: no new entries until the threads inside have left and the mutex is let go.
        draining: bool,
        /// Device threads inside the turn.
        active: usize,
        /// Device threads waiting for a turn.
        wanted: usize,
        closed: bool,
    }

    pub struct Turns {
        state: Mutex<State>,
        cv: Condvar,
    }

    thread_local! {
        /// How deep this thread is in each lock's turn (nested calls), keyed by the `Turns` address.
        static DEPTH: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) };
    }

    impl Turns {
        pub fn start(handle: win::Handle) -> Arc<Turns> {
            let t = Arc::new(Turns { state: Mutex::new(State::default()), cv: Condvar::new() });
            let run = t.clone();
            std::thread::Builder::new()
                .name("razer-lock".into())
                .spawn(move || run.run(handle))
                .expect("spawn the Razer lock thread");
            t
        }

        fn lock(&self) -> MutexGuard<'_, State> {
            self.state.lock().unwrap_or_else(|e| e.into_inner())
        }

        fn key(&self) -> usize {
            self as *const Turns as usize
        }

        /// Join the current turn, waiting up to `wait` for one. `false` on timeout.
        pub fn enter(&self, wait: Duration) -> bool {
            let key = self.key();
            let nested = DEPTH.with(|d| match d.borrow_mut().iter_mut().find(|e| e.0 == key) {
                Some(e) => {
                    e.1 += 1;
                    true
                }
                None => false,
            });
            if nested {
                return true;
            }
            let deadline = Instant::now() + wait;
            let mut st = self.lock();
            st.wanted += 1;
            self.cv.notify_all();
            loop {
                if st.held && !st.draining {
                    st.wanted -= 1;
                    st.active += 1;
                    break;
                }
                let now = Instant::now();
                if now >= deadline || st.closed {
                    st.wanted -= 1;
                    self.cv.notify_all();
                    return false;
                }
                st = self.cv.wait_timeout(st, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
            }
            drop(st);
            DEPTH.with(|d| d.borrow_mut().push((key, 1)));
            true
        }

        pub fn leave(&self) {
            let key = self.key();
            let last = DEPTH.with(|d| {
                let mut d = d.borrow_mut();
                let Some(i) = d.iter().position(|e| e.0 == key) else { return false };
                d[i].1 -= 1;
                if d[i].1 == 0 {
                    d.swap_remove(i);
                    true
                } else {
                    false
                }
            });
            if last {
                self.lock().active -= 1;
                self.cv.notify_all();
            }
        }

        pub fn close(&self) {
            self.lock().closed = true;
            self.cv.notify_all();
        }

        fn run(&self, handle: win::Handle) {
            loop {
                let mut st = self.lock();
                while st.wanted == 0 && !st.closed {
                    st = self.cv.wait(st).unwrap_or_else(|e| e.into_inner());
                }
                if st.closed {
                    return;
                }
                drop(st);
                let owned = match handle.wait(WAIT) {
                    win::Wait::Held => true,
                    // another program has it; the waiting device threads time out on their own
                    win::Wait::TimedOut => continue,
                    // the handle stopped working: carry on without the lock rather than stop the devices
                    win::Wait::Failed => false,
                };
                let start = Instant::now();
                let mut st = self.lock();
                st.held = true;
                self.cv.notify_all();
                while !st.closed {
                    let Some(rest) = TURN.checked_sub(start.elapsed()) else { break };
                    st = self.cv.wait_timeout(st, rest).unwrap_or_else(|e| e.into_inner()).0;
                }
                // end of the turn: let the reports in flight finish, then let go
                st.draining = true;
                while st.active > 0 {
                    st = self.cv.wait(st).unwrap_or_else(|e| e.into_inner());
                }
                st.held = false;
                st.draining = false;
                self.cv.notify_all();
                drop(st);
                if owned {
                    // a program waiting on the mutex is given it here
                    handle.release();
                    std::thread::yield_now();
                } else {
                    std::thread::sleep(WAIT);
                }
            }
        }
    }
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
    // a mutex handle may be used from any thread; ownership is per thread (the lock thread)
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
    use std::sync::{mpsc, Arc};
    use std::time::Instant;

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
        let t = Instant::now();
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
        assert_eq!(lock.acquire(Duration::from_secs(1)).map(|_| 7), Some(7), "free again once the other lets go");
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
        // another program's thread that exits while holding the mutex abandons it
        std::thread::spawn(move || {
            let (h, _) = win::open(&name2);
            assert!(matches!(h.unwrap().wait(Duration::from_secs(1)), win::Wait::Held));
        })
        .join()
        .unwrap();
        assert!(lock.acquire(Duration::from_secs(1)).is_some());
    }

    #[test]
    fn device_threads_of_one_process_share_a_turn() {
        let lock = Arc::new(RazerLock::open(&test_lock("share")));
        let held = lock.acquire(WAIT).unwrap();
        let other = lock.clone();
        // a second device thread gets in while the first is still inside, without waiting for it
        let t = Instant::now();
        assert!(std::thread::spawn(move || other.acquire(Duration::ZERO).is_some()).join().unwrap());
        assert!(t.elapsed() < Duration::from_millis(100));
        drop(held);
    }

    #[test]
    fn a_busy_process_still_lets_another_program_in_between_turns() {
        let name = test_lock("turns");
        let lock = Arc::new(RazerLock::open(&name));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        // a device thread sending reports back to back, like the BlackWidow's acknowledged rows
        let busy = {
            let (lock, stop) = (lock.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Some(_held) = lock.acquire(WAIT) {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            })
        };
        std::thread::sleep(Duration::from_millis(30));
        let (h, _) = win::open(&name);
        let h = h.unwrap();
        let t = Instant::now();
        assert!(matches!(h.wait(Duration::from_millis(500)), win::Wait::Held), "OpenRGB gets its turn");
        assert!(t.elapsed() < Duration::from_millis(200), "within about one turn: {:?}", t.elapsed());
        h.release();
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        busy.join().unwrap();
    }
}
