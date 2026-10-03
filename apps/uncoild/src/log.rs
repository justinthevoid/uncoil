//! Tiny append-only log at `%LOCALAPPDATA%\uncoil\uncoild.log`, trimmed when it grows past 256 KB.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("uncoil").join("uncoild.log")
}

pub fn line(msg: &str) {
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "{secs}  {msg}");
    }
    #[cfg(debug_assertions)]
    eprintln!("{msg}");
    if std::fs::metadata(&p).map(|m| m.len() > 256 * 1024).unwrap_or(false) {
        if let Ok(s) = std::fs::read_to_string(&p) {
            let keep: Vec<&str> = s.lines().rev().take(400).collect();
            let _ = std::fs::write(&p, keep.into_iter().rev().collect::<Vec<_>>().join("\n") + "\n");
        }
    }
}
