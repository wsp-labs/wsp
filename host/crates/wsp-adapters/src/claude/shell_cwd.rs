// SPDX-License-Identifier: AGPL-3.0-only
//! Where the agent's tool shell is, read from its tool calls. TS: packages/adapter-claude/src/shell-cwd.ts at
//! a3b870a6e. Paths are posix, normalized as node's `path.posix.normalize`, a trailing slash dropped.

use serde_json::Value;

/// The shell's folder after this tool call when the call moves it, else `None`. shell-cwd.ts:27.
pub fn shell_cwd_after(tool_name: Option<&str>, input: Option<&Value>, current: &str, harness_cwd: &str) -> Option<String> {
    todo!()
}
