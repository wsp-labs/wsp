// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/spawn-fed.ts. Spec: spec/child_exec.md.
use std::path::PathBuf;
use std::time::Duration;

use indexmap::IndexMap;

/// The spawnSync options spawn-fed.ts passes through (`input` and `stdio` are its own).
#[derive(Debug, Clone, Default)]
pub struct FedOptions {
    pub cwd: Option<PathBuf>,
    /// The child's whole set of variables where given.
    pub env: Option<IndexMap<String, String>>,
    /// spawnSync's `timeout`: past it the child is killed with SIGTERM and `error` says it timed out.
    pub timeout: Option<Duration>,
}

/// spawnSync's answer: `status` is `None` where the child died of a signal or never started; `error` is the spawn or
/// timeout failure's message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FedOutput {
    pub status: Option<i32>,
    pub signal: Option<String>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub error: Option<String>,
}

/// spawn-fed.ts:14. Runs the command to its end with stdin read from an unlinked file: written 0600 into a fresh
/// `<tmp>/wsp-stdin-XXXXXX` folder, opened for reading, and the folder removed before the child starts, so nothing of
/// the bytes stays on disk under any name. stdout and stderr are piped.
pub fn spawn_sync_fed(command: &str, args: &[String], stdin: &[u8], opts: &FedOptions) -> FedOutput {
    todo!()
}
