// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/config-here.ts. Spec: spec/child_exec.md.
//!
//! The one config write run on this computer, for a caller with no road to a machine: the same line every road runs,
//! through this computer's own sh.
use crate::errors::Result;
use crate::machine::ExecResult;

/// config-here.ts:8. The write's whole budget.
pub const WRITE_MS: u64 = 60_000;

/// config-here.ts:12. Nothing where the write landed. A refusal `config_refusal` reads off the result is an `Other` whose
/// `code` is `Some("usage")` carrying its words; else a non-zero exit is
/// `<shown(file)> stands as it was: the write did not finish (<first>).`, where `first` is the first line of the
/// trimmed stderr (stdout where stderr is empty), or `exit <code>` where that line is empty.
pub fn config_landed(res: &ExecResult, file: &str, shown: Option<&dyn Fn(&str) -> String>) -> Result<()> {
    todo!()
}

/// config-here.ts:21. Runs `/bin/sh -c <config_write_line({file, base, bytes: len, sum})>` with the text's UTF-8 bytes
/// on stdin (spawn_sync_fed), a `WRITE_MS` timeout, the host's own variables with PATH replaced by `/usr/bin:/bin`, and
/// reads the result through `config_landed`: status `None` is exit 1, stderr empty takes the spawn error's message.
pub fn write_config_here(file: &str, base: &str, sum: Option<&str>, text: &str, shown: Option<&dyn Fn(&str) -> String>) -> Result<()> {
    todo!()
}
