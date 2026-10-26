// SPDX-License-Identifier: AGPL-3.0-only
//! Which home a line on this computer works on: the one WSP_HOME names, or this computer's own. Port of
//! `packages/host/src/serving-home.ts` at 65f6b89c7; the spec is `spec/account.md` (the files-and-readings unit).
use std::fs;
use std::path::Path;

use wsp_wire::types::env::HOME_ENV;

use crate::host_lock::serving_host;
use crate::hosts::Env;
use crate::repo_root::{basename, dirname, join};

/// serving-home.ts:15: `join(user, ".wsp")`.
pub fn default_home_in(user: &str) -> String {
    join(&[user, ".wsp"])
}

/// serving-home.ts:19: unset and empty are one answer; `" "` is a home.
pub fn home_named(value: Option<&str>) -> Option<String> {
    value.filter(|v| !v.is_empty()).map(str::to_string)
}

/// serving-home.ts:22-24.
pub fn serving_home(env: &Env, user: &str) -> String {
    home_named(env.get(HOME_ENV).map(String::as_str)).unwrap_or_else(|| default_home_in(user))
}

/// serving-home.ts:28-32.
pub fn real_state(path: &str) -> String {
    let dir = dirname(path);
    if Path::new(path).exists() {
        return real_path(path);
    }
    if Path::new(&dir).exists() {
        return join(&[real_path(&dir).as_str(), basename(path).as_str()]);
    }
    path.to_string()
}

fn real_path(path: &str) -> String {
    fs::canonicalize(path).map_or_else(|_| path.to_string(), |p| p.to_string_lossy().into_owned())
}

/// serving-home.ts:38-41.
pub fn serving_elsewhere(state_path: &str, env: &Env, user: &str) -> Option<String> {
    let served = join(&[serving_home(env, user).as_str(), "state.json"]);
    (real_state(&served) != real_state(state_path) && serving_host(&served).is_some()).then_some(served)
}

/// serving-home.ts:48-51, byte for byte: two lines joined by one newline, no trailing newline.
pub const SERVING_HOME_SH: &str = concat!(
    r#"wsp_serving() { [ -f "$1/host.lock" ] || return 1; pid=$(sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$1/host.lock"); [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; }"#,
    "\n",
    r#"home="${WSP_HOME:-$HOME/.wsp}""#
);
