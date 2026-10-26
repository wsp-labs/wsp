// SPDX-License-Identifier: AGPL-3.0-only
//! The version this wsp carries and the writer record a state file names. Port of `packages/host/src/version.ts` at
//! 65f6b89c7; the spec is `spec/account.md`.
use wsp_store::store::StateWriter;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::wire::daemon_version::DAEMON_VERSION;

use crate::repo_root::resolve;

/// The release this build carries (`VERSION`, version.ts:11): `packages/host/package.json`'s version.
pub const VERSION: &str = "0.3.8";

/// version.ts:9: the `version` string of the `package.json` at `package_json`, read now. `None` where it cannot be
/// read, is not JSON or holds no string `version` (the TS throws there).
pub fn installed_version(package_json: &str) -> Option<String> {
    let text = std::fs::read_to_string(package_json).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&text).ok()?;
    parsed.get("version")?.as_str().map(str::to_string)
}

/// version.ts:16: `{wsp: VERSION, daemon: DAEMON_VERSION, bin}`, `bin` being `argv[1]` resolved against `cwd` as
/// node's `path.resolve` does, or `exec_path` where there is no `argv[1]`.
pub fn state_writer_here(argv: &[String], exec_path: &str, cwd: &str) -> StateWriter {
    let bin = match argv.get(1) {
        Some(script) => resolve(cwd, script),
        None => exec_path.to_string(),
    };
    StateWriter { wsp: VERSION.to_string(), daemon: JsInt::from(DAEMON_VERSION), bin }
}
