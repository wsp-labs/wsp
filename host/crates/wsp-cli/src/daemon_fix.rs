// SPDX-License-Identifier: AGPL-3.0-only
//! The line that stages the daemon binary this wsp needs, and the line that moves it onto a newer release, by the
//! road it was installed by. Port of `packages/host/src/daemon-fix.ts` at 65f6b89c7; the spec is `spec/account.md`.

/// daemon-fix.ts:14.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Road {
    Npm,
    App,
    Checkout,
}

/// daemon-fix.ts:18-23: `shim`, else `argv[1]`, else `""`, split on `/`.
pub fn road_of(argv: &[String], shim: Option<&str>) -> Road {
    todo!()
}

/// daemon-fix.ts:34.
pub fn daemon_fix_line(argv: &[String], shim: Option<&str>) -> String {
    todo!()
}

/// daemon-fix.ts:36.
pub fn release_update_line(argv: &[String], shim: Option<&str>, version: &str) -> String {
    todo!()
}

/// daemon-fix.ts:40.
pub fn release_line_of(road: Road, version: &str) -> String {
    todo!()
}
