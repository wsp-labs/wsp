// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/host-id.ts at 65f6b89c7: this install's id, kept outside every wsp home. Spec:
//! spec/host_files.md.
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::console;
use wsp_store::own_file::write_own;

static WARNED: AtomicBool = AtomicBool::new(false);

/// The line warned, once per process, when the id cannot be kept. host-id.ts:33.
pub fn host_id_not_kept_line(dir: &Path, why: &str) -> String {
    format!("host id not kept in {} ({why}); holds carry the hostname alone", dir.display())
}

/// `$XDG_CONFIG_HOME/wsp` where that variable is set and not empty, on every platform; else `~/Library/Application
/// Support/wsp` on macOS and `~/.config/wsp` elsewhere. host-id.ts:10.
pub fn local_config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg).join("wsp");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support").join("wsp")
    } else {
        home.join(".config").join("wsp")
    }
}

/// This computer's name as the OS reports it (node's `os.hostname()`).
pub fn hostname() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: the buffer is 256 bytes and gethostname writes at most that many, a NUL-terminated name inside it.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) };
    if rc != 0 {
        return String::new();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

fn kept_id(dir: &Path) -> io::Result<String> {
    let path = dir.join("host-id");
    let mut id = if path.exists() { std::fs::read_to_string(&path)?.trim().to_string() } else { String::new() };
    if id.is_empty() {
        id = hex::encode(rand::random::<[u8; 4]>());
        write_own(dir, "host-id", format!("{id}\n"))?;
    }
    Ok(id)
}

/// `<hostname>:<id>`, the id eight lowercase hex digits made once and kept in `<dir>/host-id` with a newline; the bare
/// hostname, with one warning per process, where the dir cannot be read or written. `dir` is `local_config_dir()`
/// where `None`. host-id.ts:21.
pub fn host_identity(dir: Option<&Path>) -> String {
    let dir = dir.map_or_else(local_config_dir, Path::to_path_buf);
    let name = hostname();
    match kept_id(&dir) {
        Ok(id) => format!("{name}:{id}"),
        Err(e) => {
            if !WARNED.swap(true, Ordering::Relaxed) {
                console::warn(&host_id_not_kept_line(&dir, &e.to_string()));
            }
            name
        }
    }
}

/// The text after the last `:` (the whole text where there is none), lowercased, with every character but `a-z0-9`
/// dropped. host-id.ts:42.
pub fn template_host(host_id: &str) -> String {
    let after = host_id.rfind(':').map_or(host_id, |i| &host_id[i + 1..]);
    after.to_lowercase().chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect()
}
