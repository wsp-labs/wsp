// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/config-here.ts. Spec: spec/child_exec.md.
//!
//! The one config write run on this computer, for a caller with no road to a machine: the same line every road runs,
//! through this computer's own sh.
use indexmap::IndexMap;
use wsp_catalog::config_write::{config_refusal, config_write_line, ConfigWrite};
use wsp_wire::types::js_number::JsInt;
use wsp_words::js::js_trim;

use crate::errors::{EngineError, Result};
use crate::machine::ExecResult;
use crate::spawn_fed::{spawn_sync_fed_checked, FedOptions};

/// config-here.ts:8. The write's whole budget.
pub const WRITE_MS: u64 = 60_000;

/// config-here.ts:12. Nothing where the write landed. A refusal `config_refusal` reads off the result is an `Other` whose
/// `code` is `Some("usage")` carrying its words; else a non-zero exit is
/// `<shown(file)> stands as it was: the write did not finish (<first>).`, where `first` is the first line of the
/// trimmed stderr (stdout where stderr is empty), or `exit <code>` where that line is empty.
pub fn config_landed(res: &ExecResult, file: &str, shown: Option<&dyn Fn(&str) -> String>) -> Result<()> {
    let shown_path = |p: &str| shown.map_or_else(|| p.to_string(), |f| f(p));
    let code = res.exit_code.get();
    if let Some(said) = config_refusal(i32::try_from(code).unwrap_or(i32::MIN), &res.stdout, file, &shown_path) {
        return Err(EngineError::Other { message: said, code: Some("usage".into()) });
    }
    if code == 0 {
        return Ok(());
    }
    let text = if res.stderr.is_empty() { &res.stdout } else { &res.stderr };
    let first = js_trim(text).split('\n').next().unwrap_or("");
    let first = if first.is_empty() { format!("exit {code}") } else { first.to_string() };
    Err(EngineError::Other { message: format!("{} stands as it was: the write did not finish ({first}).", shown_path(file)), code: None })
}

/// config-here.ts:21. Runs `/bin/sh -c <config_write_line({file, base, bytes: len, sum})>` with the text's UTF-8 bytes
/// on stdin (spawn_sync_fed), a `WRITE_MS` timeout, the host's own variables with PATH replaced by `/usr/bin:/bin`, and
/// reads the result through `config_landed`: status `None` is exit 1, stderr empty takes the spawn error's message.
pub fn write_config_here(file: &str, base: &str, sum: Option<&str>, text: &str, shown: Option<&dyn Fn(&str) -> String>) -> Result<()> {
    let line = config_write_line(&ConfigWrite {
        file: file.to_string(),
        base: base.to_string(),
        sum: sum.map(str::to_string),
        bytes: text.len() as u64,
        from: None,
    });
    let mut vars: IndexMap<String, String> = crate::child_exec::process_env();
    vars.insert("PATH".into(), "/usr/bin:/bin".into());
    let opts = FedOptions { timeout: Some(std::time::Duration::from_millis(WRITE_MS)), env: Some(vars), ..Default::default() };
    let ran = spawn_sync_fed_checked("/bin/sh", &["-c".to_string(), line], text.as_bytes(), &opts)
        .map_err(|thrown| EngineError::Other { message: thrown.message, code: Some(thrown.code) })?;
    let stderr = if ran.stderr.is_empty() { ran.error.unwrap_or_default() } else { String::from_utf8_lossy(&ran.stderr).into_owned() };
    let res = ExecResult {
        exit_code: JsInt::from(i64::from(ran.status.unwrap_or(1))),
        stdout: String::from_utf8_lossy(&ran.stdout).into_owned(),
        stderr,
    };
    config_landed(&res, file, shown)
}
