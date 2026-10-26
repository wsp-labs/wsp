// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/daemon-roots.ts at 65f6b89c7: the one file of folders the daemon browses beside its
//! home, written whole and in one rename. Spec: spec/host_files.md.
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::wire::daemon_version::DAEMON_ROOTS_PATH;

/// The shell script that writes `roots` one a line to `roots_path` (DAEMON_ROOTS_PATH where `None`) through a temp
/// file named for the shell's pid. daemon-roots.ts:10.
pub fn write_daemon_roots_script(roots: &[String], roots_path: Option<&str>) -> String {
    let path = roots_path.unwrap_or(DAEMON_ROOTS_PATH);
    let quoted: Vec<String> = roots.iter().map(|r| shell_quote(r)).collect();
    [
        format!("next={}$$", shell_quote(&format!("{path}.next."))),
        format!("mkdir -p {}", shell_quote(&posix_dirname(path))),
        format!("printf '%s\\n' {} > \"$next\"", quoted.join(" ")),
        format!("mv -f \"$next\" {}", shell_quote(path)),
    ]
    .join("\n")
}

/// Node's `path.posix.dirname`: the text before the last slash past a leading one, `/` for a root child, `.` for none.
pub(crate) fn posix_dirname(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut matched = true;
    let mut end = None;
    for i in (1..bytes.len()).rev() {
        if bytes[i] == b'/' {
            if !matched {
                end = Some(i);
                break;
            }
        } else {
            matched = false;
        }
    }
    let has_root = path.starts_with('/');
    match end {
        None if has_root => "/".to_string(),
        None => ".".to_string(),
        Some(1) if has_root => "//".to_string(),
        Some(e) => path[..e].to_string(),
    }
}
