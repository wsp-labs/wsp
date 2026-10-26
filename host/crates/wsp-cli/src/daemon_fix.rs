// SPDX-License-Identifier: AGPL-3.0-only
//! The line that stages the daemon binary this wsp needs, and the line that moves it onto a newer release, by the road
//! it was installed by. Port of `packages/host/src/daemon-fix.ts` at 65f6b89c7; the spec is `spec/account.md`.

/// The published package, as an install line names it (daemon-fix.ts:12).
const NPM_PACKAGE: &str = "@wsp-labs/wsp";

const APP_LINE: &str = "updating the wsp app";
const CHECKOUT_RELEASE_LINE: &str = "a pull of the checkout and a build";

/// daemon-fix.ts:14.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Road {
    Npm,
    App,
    Checkout,
}

/// daemon-fix.ts:18-23: `shim`, else `argv[1]`, else `""`, split on `/`.
pub fn road_of(argv: &[String], shim: Option<&str>) -> Road {
    let path = shim.or_else(|| argv.get(1).map(String::as_str)).unwrap_or("");
    let parts: Vec<&str> = path.split('/').collect();
    if parts.contains(&"node_modules") {
        Road::Npm
    } else if parts.iter().any(|part| part.ends_with(".app")) {
        Road::App
    } else {
        Road::Checkout
    }
}

/// daemon-fix.ts:34.
pub fn daemon_fix_line(argv: &[String], shim: Option<&str>) -> String {
    daemon_line_of(road_of(argv, shim))
}

/// daemon-fix.ts:36.
pub fn release_update_line(argv: &[String], shim: Option<&str>, version: &str) -> String {
    release_line_of(road_of(argv, shim), version)
}

/// daemon-fix.ts:40.
pub fn release_line_of(road: Road, version: &str) -> String {
    match road {
        Road::Npm => format!("npm i -g {NPM_PACKAGE}@{version}"),
        Road::App => APP_LINE.to_string(),
        Road::Checkout => CHECKOUT_RELEASE_LINE.to_string(),
    }
}

/// daemon-fix.ts:28-32, the daemon column of each road.
fn daemon_line_of(road: Road) -> String {
    match road {
        Road::Npm => format!("npm i -g {NPM_PACKAGE}"),
        Road::App => APP_LINE.to_string(),
        Road::Checkout => "a cargo build of the daemon and node packages/wspx/scripts/daemon-binary.mjs --from its binary".to_string(),
    }
}
