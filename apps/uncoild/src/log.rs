//! Tiny append-only log at `%LOCALAPPDATA%\uncoil\uncoild.log`, trimmed when it grows past 256 KB.
//!
//! Lines may carry text from device files, the config or pipe requests, so control characters are escaped:
//! nothing logged can start a new line that looks like the daemon's own.

use std::borrow::Cow;
use std::io::{Read, Seek, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Off in tests and `--fake` mode, so neither writes the real daemon log.
static TO_FILE: AtomicBool = AtomicBool::new(cfg!(not(test)));

/// Print log lines to stderr instead of the log file.
#[cfg_attr(not(feature = "fake"), allow(dead_code))]
pub fn to_stderr() {
    TO_FILE.store(false, Ordering::Relaxed);
}

fn path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("uncoil").join("uncoild.log")
}

/// `msg` with control characters written as escapes (`\n`, `\r`, `\t`, `\u{1b}`).
pub fn escape(msg: &str) -> Cow<'_, str> {
    if !msg.chars().any(char::is_control) {
        return Cow::Borrowed(msg);
    }
    let mut out = String::with_capacity(msg.len() + 8);
    for c in msg.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

pub fn line(msg: &str) {
    let msg = escape(msg);
    if !TO_FILE.load(Ordering::Relaxed) {
        eprintln!("{msg}");
        return;
    }
    #[cfg(debug_assertions)]
    eprintln!("{msg}");
    let p = path();
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut f = match crate::winsec::open_user_file(&p, true) {
        Ok(f) => f,
        Err(e) => return crate::winsec::warn_once(&e),
    };
    let _ = writeln!(f, "{secs}  {msg}");
    if f.metadata().map(|m| m.len() > 256 * 1024).unwrap_or(false) {
        drop(f);
        trim(&p);
    }
}

/// Keep the last 400 lines, rewriting the file through the same checked open.
fn trim(p: &std::path::Path) {
    let Ok(mut f) = crate::winsec::open_user_file(p, false) else { return };
    let mut s = String::new();
    if f.read_to_string(&mut s).is_err() {
        return;
    }
    let keep: Vec<&str> = s.lines().rev().take(400).collect();
    let text = keep.into_iter().rev().collect::<Vec<_>>().join("\n") + "\n";
    if f.set_len(0).is_ok() && f.rewind().is_ok() {
        let _ = f.write_all(text.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_data_cannot_forge_log_lines() {
        assert!(matches!(escape("opened Razer Test (00AA, wired)"), Cow::Borrowed(_)));
        assert_eq!(
            escape("Razer\n1700000000  ONBOARD WRITE fake\r\t\u{1b}[31m"),
            "Razer\\n1700000000  ONBOARD WRITE fake\\r\\t\\u{1b}[31m"
        );
        assert_eq!(escape("dégradé ✓"), "dégradé ✓", "non-control text is kept");
        assert!(!escape("a\u{85}b\u{7f}").chars().any(char::is_control));
    }
}
