// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/run-env.ts. Spec: spec/child_exec.md.
//!
//! Values from this host's vault on a computer somebody owns, for one run and no longer: the run's own input carries
//! every NAME=value, never its command line or a file.
use std::sync::Arc;

use futures::future::BoxFuture;
use indexmap::IndexMap;

use crate::errors::Result;
use crate::machine::{ExecResult, Machine};

/// protocol's shell-quote.ts ENV_FROM_INPUT, re-exported as run-env.ts:11 does.
pub use wsp_wire::types::shell_quote::ENV_FROM_INPUT;

/// run-env.ts:14. `ENV_FROM_INPUT`, a newline, then the script.
pub fn with_env_from_input(script: &str) -> String {
    todo!()
}

/// run-env.ts:17. One `NAME=value\0` per entry in the map's order, values unescaped (a newline stays a newline); an
/// empty map is no bytes.
pub fn env_input(vars: &IndexMap<String, String>) -> Vec<u8> {
    todo!()
}

/// run-env.ts:28. Where `machine.takes_stdin()`, `exec(command, { timeout_ms, stdin: env_input(vars) })`. Else the pairs
/// ride a pipe: an empty map runs `{\n<command>\n} </dev/null`, any other runs
/// `printf '%s\0' <shell_quote("NAME=value")...> | {\n<command>\n}` (pairs space-separated, map order). Never sends
/// `stdin` on the pipe road.
pub async fn env_exec(machine: &dyn Machine, command: &str, vars: &IndexMap<String, String>, timeout_ms: Option<u64>) -> Result<ExecResult> {
    todo!()
}

/// protocol's `HarnessExec`: a command and its variables in, the command's stdout out.
pub type HarnessExec = Arc<dyn Fn(String, Option<IndexMap<String, String>>) -> BoxFuture<'static, Result<String>> + Send + Sync>;

/// run-env.ts:36. With no variables the command runs as it is under `timeout_ms`; with some it goes through `env_exec`.
/// Either way the answer is the result's stdout alone, whatever the exit code; an exec that fails is the failure.
pub fn harness_exec(machine: Arc<dyn Machine>, timeout_ms: u64) -> HarnessExec {
    todo!()
}

/// run-env.ts:44. The first line of a script sudo runs as root.
pub const DROP_SUDO_MARKS: &str = "unset SUDO_USER SUDO_UID SUDO_GID SUDO_COMMAND";

/// run-env.ts:47. The variable gh reads its token from.
pub const GITHUB_TOKEN_ENV: &str = "GH_TOKEN";
