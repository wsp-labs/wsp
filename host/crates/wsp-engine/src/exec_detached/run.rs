// SPDX-License-Identifier: AGPL-3.0-only
//! exec-detached.ts:229-399 at a4033dc34. Spec: spec/exec_run.md.
//!
//! The launch, the polls, the kill and the cleanup of one detached run, and the loop that drives them.
use crate::errors::Result;
use crate::exec_detached::put::UploadResult;
use crate::machine::{ExecResult, Machine, RunOptions};

/// exec-detached.ts:231. Puts `{base}.sh` on the guest and starts it in its own session, behind the `.d` claim.
pub async fn launch(machine: &dyn Machine, base: &str, script: &str) -> Result<UploadResult> {
    todo!()
}

/// exec-detached.ts:248. One read of the run's exit file and both streams from these offsets.
pub fn poll_command(base: &str, out_offset: u64, err_offset: u64) -> String {
    todo!()
}

/// exec-detached.ts:260. TERM, two seconds, KILL, to the recorded group and pid.
pub fn kill_command(base: &str) -> String {
    todo!()
}

/// exec-detached.ts:266. Every file of one run.
pub fn clean_command(base: &str) -> String {
    todo!()
}

/// exec-detached.ts:270.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poll {
    pub exit: String,
    pub out: Vec<u8>,
    pub err: Vec<u8>,
    pub up: bool,
}

/// exec-detached.ts:277. The first whole `WSP_POLL` .. `WSP_POLL_END` block, or `None`.
pub fn parse_poll(stdout: &str) -> Option<Poll> {
    todo!()
}

/// exec-detached.ts:319. Runs the script on the machine as run() promises, under [`crate::exec_detached::RUN_DIR`].
pub async fn exec_detached(machine: &dyn Machine, script: &str, opts: RunOptions) -> Result<ExecResult> {
    todo!()
}

/// exec-detached.ts:319 with its `runDir`: a machine somebody owns names its own folder, a test a local one.
pub async fn exec_detached_in(machine: &dyn Machine, script: &str, opts: RunOptions, run_dir: &str) -> Result<ExecResult> {
    todo!()
}
