// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/host-id.ts at 65f6b89c7: this install's id, kept outside every wsp home. Spec:
//! spec/host_files.md.
use std::path::{Path, PathBuf};

/// The line warned, once per process, when the id cannot be kept. host-id.ts:33.
pub fn host_id_not_kept_line(dir: &Path, why: &str) -> String {
    todo!()
}

/// `$XDG_CONFIG_HOME/wsp` where that variable is set and not empty, on every platform; else `~/Library/Application
/// Support/wsp` on macOS and `~/.config/wsp` elsewhere. host-id.ts:10.
pub fn local_config_dir() -> PathBuf {
    todo!()
}

/// This computer's name as the OS reports it (node's `os.hostname()`).
pub fn hostname() -> String {
    todo!()
}

/// `<hostname>:<id>`, the id eight lowercase hex digits made once and kept in `<dir>/host-id` with a newline; the bare
/// hostname, with one warning per process, where the dir cannot be read or written. `dir` is `local_config_dir()`
/// where `None`. host-id.ts:21.
pub fn host_identity(dir: Option<&Path>) -> String {
    todo!()
}

/// The text after the last `:` (the whole text where there is none), lowercased, with every character but `a-z0-9`
/// dropped. host-id.ts:42.
pub fn template_host(host_id: &str) -> String {
    todo!()
}
