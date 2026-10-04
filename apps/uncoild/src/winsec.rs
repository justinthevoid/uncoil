//! Windows security helpers: is the daemon elevated, file writes that refuse redirection, and the checks
//! for the admin-only folder the OpenRGB hand-off uses.
//!
//! uncoild normally runs unelevated (the logon task's default), so a junction under `%LOCALAPPDATA%` can only
//! send its writes where the user could write anyway. The `-Elevated` install is the exception: then the
//! daemon turns on the redirection-trust mitigation (junctions made by non-admins are not followed) and
//! refuses to write its log, status or journal through a reparse point or into a file with other hard links.

use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

static ELEVATED: AtomicBool = AtomicBool::new(false);

/// Remember whether this process is elevated (call once at startup); returns it.
pub fn init() -> bool {
    let e = imp::is_elevated();
    ELEVATED.store(e, Ordering::Relaxed);
    e
}

pub fn elevated() -> bool {
    ELEVATED.load(Ordering::Relaxed)
}

pub use imp::{admin_only_dir, enforce_redirection_trust, session_id, system_dir};

/// Is `path` itself a reparse point (junction, symlink, mount point)? Missing paths are not.
pub fn is_reparse_point(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE: u32 = 0x400; // FILE_ATTRIBUTE_REPARSE_POINT
        std::fs::symlink_metadata(path).is_ok_and(|m| m.file_attributes() & REPARSE != 0)
    }
    #[cfg(not(windows))]
    {
        std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
    }
}

/// Create `dir` (the daemon's folder under `%LOCALAPPDATA%`) if needed; when elevated, refuse it if it is a
/// reparse point.
pub fn user_dir(dir: &Path) -> io::Result<()> {
    if elevated() && is_reparse_point(dir) {
        return Err(refused(dir, "is a junction or link"));
    }
    std::fs::create_dir_all(dir)
}

/// Open one of the daemon's own files under `%LOCALAPPDATA%\uncoil` for writing (`append`, else for reading
/// and writing from the start; the caller truncates). When elevated: the folder must not be a reparse point, the file itself is
/// opened without following one and refused if it is one or has other hard links.
pub fn open_user_file(path: &Path, append: bool) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        user_dir(dir)?;
    }
    let mut o = std::fs::OpenOptions::new();
    o.create(true);
    if append {
        o.append(true);
    } else {
        o.read(true).write(true);
    }
    if !elevated() {
        return o.open(path);
    }
    imp::open_checked(o, path)
}

fn refused(path: &Path, why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, format!("{} {why}; not writing it", path.display()))
}

/// Print a refusal once (the log itself may be what was refused).
pub fn warn_once(e: &io::Error) {
    static SAID: AtomicBool = AtomicBool::new(false);
    if !SAID.swap(true, Ordering::Relaxed) {
        eprintln!("uncoild: {e}");
    }
}

#[cfg(windows)]
mod imp {
    use super::refused;
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        GetAce, GetTokenInformation, IsWellKnownSid, TokenElevation, WinBuiltinAdministratorsSid,
        WinCreatorOwnerRightsSid, WinCreatorOwnerSid, WinLocalSystemSid, ACCESS_ALLOWED_ACE, ACE_HEADER, ACL,
        DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, TOKEN_ELEVATION,
        TOKEN_QUERY,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_FLAG_OPEN_REPARSE_POINT,
    };
    use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, OpenProcessToken, ProcessRedirectionTrustPolicy,
        SetProcessMitigationPolicy,
    };

    pub fn is_elevated() -> bool {
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let mut e = TOKEN_ELEVATION { TokenIsElevated: 0 };
            let mut len = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                (&mut e as *mut TOKEN_ELEVATION).cast(),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut len,
            );
            CloseHandle(token);
            ok != 0 && e.TokenIsElevated != 0
        }
    }

    /// `ProcessRedirectionTrustPolicy` with `EnforceRedirectionTrust`: this process no longer follows
    /// junctions created by non-administrators (Windows 10 21H2 and later).
    pub fn enforce_redirection_trust() -> io::Result<()> {
        let flags: u32 = 1; // EnforceRedirectionTrust
        let ok = unsafe {
            SetProcessMitigationPolicy(
                ProcessRedirectionTrustPolicy,
                (&flags as *const u32).cast(),
                std::mem::size_of::<u32>(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn open_checked(mut o: OpenOptions, path: &Path) -> io::Result<File> {
        let f = o.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT).open(path)?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(f.as_raw_handle() as HANDLE, &mut info) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(refused(path, "is a link"));
        }
        if info.nNumberOfLinks > 1 {
            return Err(refused(path, "has other hard links"));
        }
        Ok(f)
    }

    /// The Windows system folder (`C:\Windows\System32`), from the API rather than the environment.
    pub fn system_dir() -> PathBuf {
        let mut buf = [0u16; 260];
        let n = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if n == 0 || n >= buf.len() {
            return PathBuf::from(r"C:\Windows\System32");
        }
        PathBuf::from(String::from_utf16_lossy(&buf[..n]))
    }

    /// The Windows session this process runs in.
    pub fn session_id() -> u32 {
        let mut s = 0u32;
        unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut s) };
        s
    }

    /// Access bits that change a folder or what is in it.
    const WRITE_BITS: u32 = 0x0002 // FILE_WRITE_DATA / FILE_ADD_FILE
        | 0x0004 // FILE_APPEND_DATA / FILE_ADD_SUBDIRECTORY
        | 0x0010 // FILE_WRITE_EA
        | 0x0040 // FILE_DELETE_CHILD
        | 0x0100 // FILE_WRITE_ATTRIBUTES
        | 0x0001_0000 // DELETE
        | 0x0004_0000 // WRITE_DAC
        | 0x0008_0000 // WRITE_OWNER
        | 0x1000_0000 // GENERIC_ALL
        | 0x4000_0000 // GENERIC_WRITE
        | 0x0200_0000; // MAXIMUM_ALLOWED

    fn is_admin_sid(sid: PSID) -> bool {
        unsafe { IsWellKnownSid(sid, WinBuiltinAdministratorsSid) != 0 || IsWellKnownSid(sid, WinLocalSystemSid) != 0 }
    }

    /// `dir` is a real folder (not a reparse point), owned by Administrators or SYSTEM, and only those (or the
    /// creator-owner placeholders) may change it or what is in it. Used for `%ProgramData%\uncoil\openrgb`,
    /// which the elevated OpenRGB hand-off gives OpenRGB as its configuration folder.
    pub fn admin_only_dir(dir: &Path) -> Result<(), String> {
        if !dir.is_dir() {
            return Err(format!("{} does not exist", dir.display()));
        }
        if super::is_reparse_point(dir) {
            return Err(format!("{} is a junction or link", dir.display()));
        }
        let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let mut owner: PSID = std::ptr::null_mut();
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let err = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut sd,
            )
        };
        if err != 0 {
            return Err(format!("cannot read the permissions of {} ({err})", dir.display()));
        }
        let verdict = unsafe { judge(owner, dacl) };
        unsafe { LocalFree(sd) };
        verdict.map_err(|why| format!("{} {why}", dir.display()))
    }

    unsafe fn judge(owner: PSID, dacl: *mut ACL) -> Result<(), &'static str> {
        if owner.is_null() || !is_admin_sid(owner) {
            return Err("is not owned by Administrators or SYSTEM");
        }
        if dacl.is_null() {
            return Err("has no access list (anyone may write)");
        }
        for i in 0..(*dacl).AceCount as u32 {
            let mut ace: *mut core::ffi::c_void = std::ptr::null_mut();
            if GetAce(dacl, i, &mut ace) == 0 {
                return Err("has an access list uncoil cannot read");
            }
            let header = &*(ace as *const ACE_HEADER);
            match header.AceType as u32 {
                1 => continue, // ACCESS_DENIED_ACE_TYPE only takes rights away
                0 => {}        // ACCESS_ALLOWED_ACE_TYPE
                _ => return Err("has an access entry uncoil does not know"),
            }
            let allowed = &*(ace as *const ACCESS_ALLOWED_ACE);
            let sid = (&allowed.SidStart as *const u32) as PSID;
            let trusted = is_admin_sid(sid)
                || IsWellKnownSid(sid, WinCreatorOwnerSid) != 0
                || IsWellKnownSid(sid, WinCreatorOwnerRightsSid) != 0;
            if !trusted && allowed.Mask & WRITE_BITS != 0 {
                return Err("can be changed by non-administrators");
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn user_folders_are_not_admin_only() {
            let dir = std::env::temp_dir().join(format!("uncoil-test-acl-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let e = admin_only_dir(&dir).unwrap_err();
            let _ = std::fs::remove_dir(&dir);
            assert!(e.contains("not owned by Administrators") || e.contains("non-administrators"), "{e}");
            assert!(admin_only_dir(Path::new(r"C:\no\such\folder")).unwrap_err().contains("does not exist"));
            // ProgramData itself lets users add folders
            if let Some(pd) = std::env::var_os("ProgramData") {
                assert!(admin_only_dir(Path::new(&pd)).is_err());
            }
        }

        #[test]
        fn system_folder_and_session() {
            assert!(system_dir().join("tasklist.exe").exists(), "{}", system_dir().display());
            let _ = session_id();
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::{Path, PathBuf};
    pub fn is_elevated() -> bool {
        false
    }
    pub fn enforce_redirection_trust() -> std::io::Result<()> {
        Ok(())
    }
    pub fn open_checked(mut o: std::fs::OpenOptions, path: &Path) -> std::io::Result<std::fs::File> {
        o.open(path)
    }
    pub fn system_dir() -> PathBuf {
        PathBuf::from("/usr/bin")
    }
    pub fn session_id() -> u32 {
        0
    }
    pub fn admin_only_dir(_dir: &Path) -> Result<(), String> {
        Err("Windows only".into())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// The checks the elevated daemon applies (tested directly; tests do not run elevated).
    #[test]
    fn redirected_files_are_refused() {
        let base = std::env::temp_dir().join(format!("uncoil-test-reparse-{}", std::process::id()));
        let real = base.join("real");
        let link = base.join("link");
        std::fs::create_dir_all(&real).unwrap();
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .output()
            .is_ok_and(|o| o.status.success());
        if made {
            assert!(is_reparse_point(&link));
            assert!(!is_reparse_point(&real));
        }
        // a file with a second hard link
        let file = real.join("uncoild.log");
        std::fs::write(&file, "x").unwrap();
        std::fs::hard_link(&file, real.join("other")).unwrap();
        let mut o = std::fs::OpenOptions::new();
        o.append(true).create(true);
        let e = imp::open_checked(o.clone(), &file).unwrap_err();
        assert!(e.to_string().contains("hard links"), "{e}");
        std::fs::remove_file(real.join("other")).unwrap();
        assert!(imp::open_checked(o, &file).is_ok());
        let _ = std::fs::remove_dir_all(&base);
    }
}
