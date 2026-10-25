// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/child-exec.ts. Spec: spec/child_exec.md.
//!
//! One child process, read to its end: this computer's own shell and the ssh client both need it. The exit code stands
//! for the command's, a negative one for a signal, and a run cut at its deadline answers 124.
use std::path::PathBuf;

use indexmap::IndexMap;

use crate::machine::{ExecResult, OnLine};

/// child-exec.ts:11.
#[derive(Clone, Default)]
pub struct ChildOptions {
    pub cwd: Option<PathBuf>,
    /// The child's whole set of variables where given (nothing inherited); the host's own where `None`.
    pub env: Option<IndexMap<String, String>>,
    /// Past it the child's process group is killed and the result is 124 with the output so far.
    pub timeout_ms: Option<u64>,
    /// Each complete stdout line as it is read; the last partial line follows once, at the end.
    pub on_line: Option<OnLine>,
    /// Bytes written to the child's stdin, which is then closed; `None` leaves stdin inherited.
    pub stdin: Option<Vec<u8>>,
}

/// child-exec.ts:25. A child that could not be started at all: the code a shell answers a missing command with.
pub const CHILD_UNSTARTABLE: i64 = 127;
/// child-exec.ts:26.
pub const CHILD_TIMED_OUT: i64 = 124;
/// child-exec.ts:28. How long the pipes may stay quiet after the command exits before its answer goes without their close.
pub const DRAIN_MS: u64 = 200;
/// child-exec.ts:30. How long after the command exits its answer waits on pipes something it left running keeps writing to.
pub const DRAIN_CAP_MS: u64 = 1_000;

/// child-exec.ts:44. A stream's chunks as lines, the last partial one held until its newline or the flush; past `keep`
/// UTF-16 units only its tail is held. `None` keeps everything (the TS `Infinity`). Lines return to the caller rather
/// than going to a callback.
pub struct LineFeed {
    pending: String,
    keep: Option<usize>,
}

impl LineFeed {
    pub fn new(keep: Option<usize>) -> Self {
        todo!()
    }

    /// Every line the chunk completes, in order, without its `\n` (a `\r` stays). Then the held tail is cut to its last
    /// `keep` UTF-16 units.
    pub fn feed(&mut self, chunk: &str) -> Vec<String> {
        todo!()
    }

    /// The held partial line where it is not empty; the feed holds nothing after.
    pub fn flush(&mut self) -> Option<String> {
        todo!()
    }
}

/// child-exec.ts:63. Spawns `file` with `args` as the leader of its own process group, reads stdout and stderr as UTF-8
/// to the end and answers; never fails. An unstartable child answers 127 with the spawn error's message and a newline
/// appended to stderr. The deadline SIGKILLs the whole group and the answer is 124. Once the child exits, its output is
/// read until the pipes close, go quiet for `DRAIN_MS`, or `DRAIN_CAP_MS` has passed. A signal death with no deadline is
/// -1; no code and no signal is 0.
pub async fn run_child(file: &str, args: &[String], opts: ChildOptions) -> ExecResult {
    todo!()
}
