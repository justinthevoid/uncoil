//! Embeds every device file: `devices/*.toml` and `devices/experimental/*.toml`, sorted by path, so a new
//! device needs no Rust edit. Comments and blank lines are dropped on the way in (about a third of the
//! text), then everything is deflated as one blob (about a seventh of the stripped text; the daemon carries
//! every file). Generates `$OUT_DIR/builtin_devices.bin` (the blob) and `$OUT_DIR/builtin_devices.rs` with
//! `BUILTIN_BLOB`, `BUILTIN_LEN` (inflated size) and `BUILTIN_INDEX: &[(&str, usize, usize)]`
//! (path, start, end into the inflated text).

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let root = manifest.join("..").join("..");
    let mut files: Vec<(String, PathBuf)> = vec![];
    // Cargo scans a directory recursively, so this also re-runs when devices/experimental/ (or a file in
    // it) appears, changes or goes away. A missing path would make the script run on every build.
    println!("cargo:rerun-if-changed={}", root.join("devices").display());
    for sub in ["devices", "devices/experimental"] {
        let dir = root.join(sub);
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("toml") {
                let name = p.file_name().and_then(|n| n.to_str()).expect("device file names are UTF-8");
                files.push((format!("{sub}/{name}"), p.clone()));
            }
        }
    }
    files.sort();
    let mut text = String::new();
    let mut index = String::from("const BUILTIN_INDEX: &[(&str, usize, usize)] = &[\n");
    for (name, path) in &files {
        let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let start = text.len();
        text.push_str(&strip_comments(&src));
        index.push_str(&format!("    ({name:?}, {start}, {}),\n", text.len()));
    }
    index.push_str("];\n");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let blob = miniz_oxide::deflate::compress_to_vec(text.as_bytes(), 10);
    std::fs::write(out_dir.join("builtin_devices.bin"), blob).expect("write builtin_devices.bin");
    let mut out = String::from("/// Every device file compiled in, without comments, deflated as one blob.\n");
    out.push_str("const BUILTIN_BLOB: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/builtin_devices.bin\"));\n");
    out.push_str(&format!("const BUILTIN_LEN: usize = {};\n", text.len()));
    out.push_str(&index);
    std::fs::write(Path::new(&out_dir).join("builtin_devices.rs"), out).expect("write builtin_devices.rs");
}

/// Drop TOML comments, trailing spaces and blank lines. A `#` inside a quoted string is kept; a line that
/// opens or closes a multi-line string is copied as it is, and so is everything inside one.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_multiline = false;
    for line in src.lines() {
        let toggles = line.matches("\"\"\"").count() + line.matches("'''").count();
        if in_multiline || toggles > 0 {
            out.push_str(line);
            out.push('\n');
            in_multiline ^= toggles % 2 == 1;
            continue;
        }
        let mut cut = line.len();
        let mut quote: Option<char> = None;
        let mut escaped = false;
        for (i, c) in line.char_indices() {
            match quote {
                Some('"') if escaped => escaped = false,
                Some('"') if c == '\\' => escaped = true,
                Some(q) if c == q => quote = None,
                Some(_) => {}
                None if c == '"' || c == '\'' => quote = Some(c),
                None if c == '#' => {
                    cut = i;
                    break;
                }
                None => {}
            }
        }
        let kept = line[..cut].trim_end();
        if !kept.trim().is_empty() {
            out.push_str(kept);
            out.push('\n');
        }
    }
    out
}
