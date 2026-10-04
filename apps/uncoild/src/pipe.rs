//! The control pipe server: `\\.\pipe\uncoil`, newline-delimited JSON (see `uncoil_core::ipc`).
//!
//! * One acceptor thread blocks in `ConnectNamedPipe`; each client gets a thread that blocks in `ReadFile`.
//!   Nothing polls, so an idle daemon spends no CPU here.
//! * Access: the DACL grants the daemon's own user account (the logged-in user; the scheduled task runs as
//!   that user) and nobody else; the medium mandatory label (no write up, no read up) lets that user's
//!   non-elevated GUI and CLI connect when the daemon runs elevated. Remote clients are rejected, and
//!   `FILE_FLAG_FIRST_PIPE_INSTANCE` on the first instance makes [`serve`] fail rather than share a name
//!   another process already took; the daemon then exits before opening any device (its single-instance
//!   lock).
//! * At most [`imp::MAX_CLIENTS`] clients; one that sends no request for [`IDLE`] is disconnected, so idle
//!   connections cannot lock the GUI out.

#[cfg(not(windows))]
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use uncoil_core::ipc::{Request, Response};

pub type Handler = Arc<dyn Fn(Request) -> Response + Send + Sync>;

/// A connection that sends no request for this long is closed.
pub const IDLE: Duration = Duration::from_secs(5 * 60);

/// Handle one request line (also used by tests without a pipe).
pub fn handle_line(handler: &Handler, line: &str) -> Response {
    match serde_json::from_str::<Request>(line) {
        Ok(req) => handler(req),
        Err(e) => {
            #[derive(serde::Deserialize)]
            struct IdOnly {
                id: Option<uncoil_core::ipc::Raw>,
            }
            let id = serde_json::from_str::<IdOnly>(line).ok().and_then(|v| v.id);
            Response::err(id, format!("bad request: {e}"))
        }
    }
}

#[cfg(all(windows, test))]
use imp::serve_with;

/// Start serving `name` with the default idle timeout.
#[cfg(windows)]
pub fn serve(name: &str, handler: Handler) -> anyhow::Result<()> {
    imp::serve_with(name, handler, IDLE)
}

#[cfg(not(windows))]
pub fn serve(_name: &str, _handler: Handler) -> Result<()> {
    anyhow::bail!("the control pipe is Windows-only")
}

#[cfg(windows)]
mod imp {
    use super::{handle_line, Handler};
    use anyhow::{bail, Result};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::windows::io::FromRawHandle;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};
    use uncoil_core::ipc::{Response, MAX_LINE};
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
        PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentThreadId, OpenProcessToken, OpenThread, THREAD_TERMINATE,
    };
    use windows_sys::Win32::System::IO::CancelSynchronousIo;

    /// At most this many clients at once (the GUI plus a few CLI calls).
    pub const MAX_CLIENTS: usize = 8;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// String SID of the user this process runs as.
    fn current_user_sid() -> Result<String> {
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                bail!("OpenProcessToken failed ({})", GetLastError());
            }
            let mut len = 0u32;
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut len);
            let mut buf = vec![0u64; (len as usize).div_ceil(8).max(1)];
            let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), len, &mut len);
            CloseHandle(token);
            if ok == 0 {
                bail!("GetTokenInformation failed ({})", GetLastError());
            }
            let user = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut s: *mut u16 = std::ptr::null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut s) == 0 {
                bail!("ConvertSidToStringSidW failed ({})", GetLastError());
            }
            let n = (0..).take_while(|&i| *s.add(i) != 0).count();
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(s, n));
            LocalFree(s.cast());
            Ok(sid)
        }
    }

    /// Security descriptor: full access for the current user only; medium integrity label with no write up
    /// and no read up.
    pub fn sddl() -> Result<String> {
        Ok(format!("D:P(A;;GA;;;{})S:(ML;;NWNR;;;ME)", current_user_sid()?))
    }

    struct Security {
        attrs: SECURITY_ATTRIBUTES,
    }
    // the descriptor is created once and never freed or mutated
    unsafe impl Send for Security {}

    fn security() -> Result<Security> {
        let sddl = wide(&sddl()?);
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        unsafe {
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                std::ptr::null_mut(),
            ) == 0
            {
                bail!("bad pipe security descriptor ({})", GetLastError());
            }
        }
        Ok(Security {
            attrs: SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd,
                bInheritHandle: 0,
            },
        })
    }

    fn create_instance(name: &[u16], sec: &Security, first: bool) -> Result<HANDLE> {
        let mode = PIPE_ACCESS_DUPLEX | if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
        let h = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64 * 1024,
                64 * 1024,
                0,
                &sec.attrs,
            )
        };
        if h == INVALID_HANDLE_VALUE {
            bail!("CreateNamedPipe failed ({})", unsafe { GetLastError() });
        }
        Ok(h)
    }

    /// Connected clients and when each last sent a request. A watchdog thread sleeps until the oldest one
    /// is due and then cancels its pending read (`CancelSynchronousIo`), which ends that client's thread.
    struct Watch {
        base: Instant,
        clients: Mutex<Vec<Watched>>,
        wake: Condvar,
        next_id: AtomicU64,
    }

    struct Watched {
        id: u64,
        /// The client thread, opened with THREAD_TERMINATE (what CancelSynchronousIo needs), as an integer.
        thread: usize,
        /// Milliseconds after `base` of the last request.
        last: Arc<AtomicU64>,
    }

    impl Watch {
        fn now_ms(&self) -> u64 {
            self.base.elapsed().as_millis() as u64
        }

        /// Register the calling client thread; `None` if its handle cannot be opened (then it is unwatched).
        fn enter(&self) -> Option<(u64, Arc<AtomicU64>)> {
            let thread = unsafe { OpenThread(THREAD_TERMINATE, 0, GetCurrentThreadId()) };
            if thread.is_null() {
                return None;
            }
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let last = Arc::new(AtomicU64::new(self.now_ms()));
            self.clients.lock().unwrap().push(Watched { id, thread: thread as usize, last: last.clone() });
            self.wake.notify_one();
            Some((id, last))
        }

        fn leave(&self, id: u64) {
            let mut c = self.clients.lock().unwrap();
            if let Some(i) = c.iter().position(|w| w.id == id) {
                let w = c.swap_remove(i);
                unsafe { CloseHandle(w.thread as HANDLE) };
            }
        }

        fn run(&self, idle: Duration) {
            let idle = idle.as_millis() as u64;
            let mut clients = self.clients.lock().unwrap();
            loop {
                let now = self.now_ms();
                let mut next: Option<u64> = None;
                for w in clients.iter() {
                    let due = w.last.load(Ordering::Relaxed) + idle;
                    if due <= now {
                        // the thread may be between reads; look again shortly until it has gone
                        unsafe { CancelSynchronousIo(w.thread as HANDLE) };
                        next = Some(next.map_or(1000, |n| n.min(1000)));
                    } else {
                        next = Some(next.map_or(due - now, |n| n.min(due - now)));
                    }
                }
                clients = match next {
                    None => self.wake.wait(clients).unwrap(),
                    Some(ms) => self.wake.wait_timeout(clients, Duration::from_millis(ms)).unwrap().0,
                };
            }
        }
    }

    /// Start serving `name` (e.g. `\\.\pipe\uncoil`); clients silent for `idle` are disconnected. Returns
    /// once the first instance exists, so clients can connect as soon as this returns.
    pub fn serve_with(name: &str, handler: Handler, idle: Duration) -> Result<()> {
        let sec = security()?;
        let wname = wide(name);
        let first = create_instance(&wname, &sec, true)?;
        let first = first as usize; // HANDLE is a raw pointer; move it across the thread boundary as an integer
        let clients = Arc::new(AtomicUsize::new(0));
        let watch = Arc::new(Watch {
            base: Instant::now(),
            clients: Mutex::new(Vec::new()),
            wake: Condvar::new(),
            next_id: AtomicU64::new(0),
        });
        let w = watch.clone();
        std::thread::Builder::new().name("pipe-idle".into()).spawn(move || w.run(idle))?;
        std::thread::Builder::new().name("pipe".into()).spawn(move || {
            let mut next = Some(first as HANDLE);
            loop {
                let h = match next.take() {
                    Some(h) => h,
                    None => match create_instance(&wname, &sec, false) {
                        Ok(h) => h,
                        Err(e) => {
                            crate::log::line(&format!("control pipe: {e:#}"));
                            std::thread::sleep(std::time::Duration::from_secs(1));
                            continue;
                        }
                    },
                };
                // blocks until a client connects
                let ok = unsafe { ConnectNamedPipe(h, std::ptr::null_mut()) } != 0
                    || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
                if !ok {
                    unsafe { CloseHandle(h) };
                    continue;
                }
                let file = unsafe { std::fs::File::from_raw_handle(h as _) };
                if clients.fetch_add(1, Ordering::SeqCst) >= MAX_CLIENTS {
                    clients.fetch_sub(1, Ordering::SeqCst);
                    // a well-formed error line without an id: clients take it as the answer to their call
                    let mut f = file;
                    let busy =
                        format!("too many clients: uncoild serves at most {MAX_CLIENTS} at once; close one and retry");
                    let _ = f.write_all(Response::err(None, busy).to_line().as_bytes());
                    continue;
                }
                let (handler, clients, watch) = (handler.clone(), clients.clone(), watch.clone());
                let spawned = std::thread::Builder::new().name("pipe-client".into()).spawn(move || {
                    let watched = watch.enter();
                    serve_client(file, &handler, watched.as_ref().map(|(_, last)| (&*watch, &**last)));
                    if let Some((id, _)) = watched {
                        watch.leave(id);
                    }
                    clients.fetch_sub(1, Ordering::SeqCst);
                });
                if spawned.is_err() {
                    crate::log::line("control pipe: cannot spawn client thread");
                }
            }
        })?;
        Ok(())
    }

    fn serve_client(file: std::fs::File, handler: &Handler, watched: Option<(&Watch, &AtomicU64)>) {
        let Ok(mut out) = file.try_clone() else { return };
        let mut reader = BufReader::new(file.take(u64::MAX));
        loop {
            let mut line = String::new();
            // cap each line: a client cannot make the daemon buffer unbounded input
            reader.get_mut().set_limit(MAX_LINE as u64 + 1);
            // an idle client's read is cancelled by the watchdog: Err ends this connection
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => return,
                Ok(n) if n > MAX_LINE => {
                    let _ = out.write_all(Response::err(None, "request too long").to_line().as_bytes());
                    return;
                }
                Ok(_) => {}
            }
            if let Some((watch, last)) = watched {
                last.store(watch.now_ms(), Ordering::Relaxed);
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let resp = handle_line(handler, line);
            if out.write_all(resp.to_line().as_bytes()).is_err() {
                return;
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use uncoil_core::ipc::{Client, Command};

    #[test]
    fn pipe_roundtrip_in_process() {
        let name = format!(r"\\.\pipe\uncoil-test-{}-roundtrip", std::process::id());
        let handler: Handler = Arc::new(|req: Request| match req.command() {
            Ok(Command::Status) => Response::ok(req.id, uncoil_core::ipc::raw(&serde_json::json!({"pong": true}))),
            Ok(_) => Response::err(req.id, "unexpected"),
            Err(e) => Response::err(req.id, e.to_string()),
        });
        serve(&name, handler).unwrap();
        // a second server on the same name must fail (pipe squatting protection)
        assert!(serve(&name, Arc::new(|r: Request| Response::ok(r.id, uncoil_core::ipc::raw(&0)))).is_err());

        let mut c = Client::connect_to(&name).unwrap();
        let r = c.call(None, &Command::Status).unwrap();
        assert!(r.ok, "{r:?}");
        let v: serde_json::Value = r.into_result().unwrap();
        assert_eq!(v["pong"], true);
        let r = c.call(None, &Command::Devices).unwrap();
        assert_eq!(r.error.as_deref(), Some("unexpected"));
        // several clients at once
        let mut c2 = Client::connect_to(&name).unwrap();
        assert!(c2.call(None, &Command::Status).unwrap().ok);
        assert!(c.call(None, &Command::Status).unwrap().ok);
    }

    #[test]
    fn malformed_lines_get_an_error_not_a_hangup() {
        use std::io::{BufRead, BufReader, Write};
        let name = format!(r"\\.\pipe\uncoil-test-{}-malformed", std::process::id());
        serve(&name, Arc::new(|r: Request| Response::ok(r.id, uncoil_core::ipc::raw(&0)))).unwrap();
        let mut f = std::fs::OpenOptions::new().read(true).write(true).open(&name).unwrap();
        f.write_all(b"{not json\n{\"id\":5,\"cmd\":\"status\"}\n").unwrap();
        let mut r = BufReader::new(f.try_clone().unwrap());
        let mut a = String::new();
        r.read_line(&mut a).unwrap();
        assert!(a.contains("bad request"), "{a}");
        let mut b = String::new();
        r.read_line(&mut b).unwrap();
        assert!(b.contains("\"id\":5") && b.contains("\"ok\":true"), "{b}");
    }

    #[test]
    fn descriptor_names_only_the_current_user() {
        let s = imp_sddl();
        assert!(s.starts_with("D:P(A;;GA;;;S-1-5-"), "{s}");
        assert_eq!(s.matches("(A;").count(), 1, "exactly one allow entry: {s}");
        assert!(s.ends_with("S:(ML;;NWNR;;;ME)"));
    }

    fn imp_sddl() -> String {
        super::imp::sddl().unwrap()
    }

    fn echo() -> Handler {
        Arc::new(|r: Request| Response::ok(r.id, uncoil_core::ipc::raw(&0)))
    }

    #[test]
    fn idle_clients_are_disconnected() {
        use std::io::Read;
        let name = format!(r"\\.\pipe\uncoil-test-{}-idle", std::process::id());
        serve_with(&name, echo(), Duration::from_millis(300)).unwrap();
        // a client that keeps talking stays
        let mut busy = Client::connect_to(&name).unwrap();
        // one that never sends anything is dropped
        let mut idle = loop {
            match std::fs::OpenOptions::new().read(true).write(true).open(&name) {
                Ok(f) => break f,
                // the next instance is created right after the previous client connected
                Err(e) if e.raw_os_error() == Some(231) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => panic!("{e}"),
            }
        };
        for _ in 0..4 {
            std::thread::sleep(Duration::from_millis(150));
            assert!(busy.call(None, &Command::Status).unwrap().ok);
        }
        std::thread::sleep(Duration::from_millis(1500));
        let mut buf = [0u8; 16];
        assert!(matches!(idle.read(&mut buf), Ok(0) | Err(_)), "the idle connection was closed");
    }

    #[test]
    fn a_full_server_says_so_plainly() {
        let name = format!(r"\\.\pipe\uncoil-test-{}-full", std::process::id());
        serve(&name, echo()).unwrap();
        let mut held: Vec<Client> = (0..imp::MAX_CLIENTS).map(|_| Client::connect_to(&name).unwrap()).collect();
        for c in held.iter_mut() {
            assert!(c.call(None, &Command::Status).unwrap().ok);
        }
        let mut extra = Client::connect_to(&name).unwrap();
        let r = extra.call(None, &Command::Status).unwrap();
        assert!(!r.ok);
        assert!(r.error.as_deref().unwrap_or("").starts_with("too many clients"), "{r:?}");
        drop(held);
    }
}
