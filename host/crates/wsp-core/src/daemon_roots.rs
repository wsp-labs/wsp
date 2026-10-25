// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/daemon-roots.ts at 65f6b89c7: the one file of folders the daemon browses beside its
//! home, written whole and in one rename. Spec: spec/host_files.md.

/// The shell script that writes `roots` one a line to `roots_path` (DAEMON_ROOTS_PATH where `None`) through a temp
/// file named for the shell's pid. daemon-roots.ts:10.
pub fn write_daemon_roots_script(roots: &[String], roots_path: Option<&str>) -> String {
    todo!()
}
