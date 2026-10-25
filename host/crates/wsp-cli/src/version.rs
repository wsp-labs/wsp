// SPDX-License-Identifier: AGPL-3.0-only
//! The version this wsp carries and the writer record a state file names. Port of `packages/host/src/version.ts` at
//! 65f6b89c7; the spec is `spec/account.md`.
use wsp_store::store::StateWriter;

/// The release this build carries (`VERSION`, version.ts:11): `packages/host/package.json`'s version.
pub const VERSION: &str = "0.3.8";

/// version.ts:9: the `version` string of the `package.json` at `package_json`, read now. `None` where it cannot be
/// read, is not JSON or holds no string `version` (the TS throws there).
pub fn installed_version(package_json: &str) -> Option<String> {
    todo!()
}

/// version.ts:16: `{wsp: VERSION, daemon: DAEMON_VERSION, bin}`, `bin` being `argv[1]` resolved against `cwd` as
/// node's `path.resolve` does, or `exec_path` where there is no `argv[1]`.
pub fn state_writer_here(argv: &[String], exec_path: &str, cwd: &str) -> StateWriter {
    todo!()
}
