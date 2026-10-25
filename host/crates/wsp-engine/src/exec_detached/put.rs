// SPDX-License-Identifier: AGPL-3.0-only
//! exec-detached.ts:1-227 at a4033dc34. Spec: spec/exec_put.md.
//!
//! The constants every guest road shares, many short reads in as few execs as fit, and the one way bytes go onto a
//! guest through exec.
use indexmap::IndexMap;

use crate::errors::Result;
use crate::machine::{ExecResult, Framing, Machine};

/// exec-detached.ts:18. The longest one plain exec may take.
pub const INLINE_EXEC_MS: u64 = 20_000;
/// exec-detached.ts:21. What a run past its deadline exits with.
pub const DEADLINE_EXIT: i64 = wsp_wire::wire::daemon::EXEC_DEADLINE_EXIT;
/// exec-detached.ts:23. What a run answers when its caller cancelled it.
pub const CANCELLED_EXIT: i64 = 130;
/// exec-detached.ts:28.
pub const GUEST_TMP: &str = "/tmp";
/// exec-detached.ts:32.
pub const RUN_DIR: &str = "/tmp/wsp-run";

/// exec-detached.ts:37. What a guest prints back to say a step landed.
pub struct Handshake {
    pub piece: &'static str,
    pub launched: &'static str,
    pub written: &'static str,
    pub gone: &'static str,
    pub run: &'static str,
}

/// exec-detached.ts:37.
pub const HANDSHAKE: Handshake =
    Handshake { piece: "WSP_PIECE", launched: "WSP_LAUNCHED", written: "WSP_OK", gone: "WSP_GONE", run: "WSP_RUN" };

/// exec-detached.ts:42. What the machine said, for a sentence that would otherwise end in a colon.
pub fn machine_answer(res: &ExecResult) -> String {
    todo!()
}

/// exec-detached.ts:50.
pub const DAEMON_ENV_FILE: &str = "/etc/wsp/daemon.env";

/// exec-detached.ts:53. Room left under the cap for what a backend wraps around the command on the wire.
pub const EXEC_ENVELOPE_BYTES: usize = 512;

/// exec-detached.ts:61. Whether one command fits an exec body, the backend's wrapper counted.
pub fn exec_fits(command: &str) -> bool {
    todo!()
}

/// exec-detached.ts:68.
pub const READS_PER_EXEC: usize = 8;

/// exec-detached.ts:71. One page of reads and what the exec that carried it answered.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadPage<T> {
    pub rows: Vec<T>,
    pub res: ExecResult,
}

/// exec-detached.ts:82. Many short reads in as few execs as they fit in; `line` gets the row's place in its page.
pub async fn paged_reads<T: Clone + Send + Sync>(
    machine: &dyn Machine,
    rows: &[T],
    line: impl Fn(&T, usize) -> String + Send + Sync,
    head: &str,
) -> Result<Vec<ReadPage<T>>> {
    todo!()
}

/// exec-detached.ts:105. What one page printed, by the place in that page the marker names, in first-seen order.
pub fn markers_of(stdout: &str, marker: &str) -> IndexMap<String, String> {
    todo!()
}

/// exec-detached.ts:132.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestWrite {
    /// Where on the guest; its directory is made if missing.
    pub path: String,
    pub text: String,
    /// Add to the file's end instead of replacing it.
    pub append: bool,
}

/// exec-detached.ts:140.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PutFilesOptions {
    /// Lines the last exec runs before any file lands.
    pub before: Vec<String>,
    /// Lines the last exec runs once every file is on disk.
    pub after: Vec<String>,
    pub timeout_ms: Option<u64>,
    /// Bytes for the last exec's own input.
    pub stdin: Option<Vec<u8>>,
}

/// exec-detached.ts:154.
pub fn append_marker(path: &str) -> String {
    todo!()
}

/// exec-detached.ts:159.
pub const OLD_APPEND_MARKS: &str = ".a[0-9a-f]*";

/// exec-detached.ts:169. The execs that put the files on the guest under upload id `upload`, every body under the cap
/// as `framing` wraps it: the pieces, then the last exec. Refuses before anything is sent.
pub fn upload_sequence(
    files: &[GuestWrite],
    before: &[String],
    after: &[String],
    upload: &str,
    framing: Option<&dyn Framing>,
) -> Result<Vec<String>> {
    todo!()
}

/// exec-detached.ts:204. What one upload took: the last exec's answer and the pieces cut to fit an exec body.
#[derive(Debug, Clone, PartialEq)]
pub struct UploadResult {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
    pub pieces: u32,
}

/// exec-detached.ts:215. Sends the upload's execs in order, each piece confirmed before the next goes.
pub async fn put_files(machine: &dyn Machine, files: &[GuestWrite], opts: PutFilesOptions) -> Result<UploadResult> {
    todo!()
}
