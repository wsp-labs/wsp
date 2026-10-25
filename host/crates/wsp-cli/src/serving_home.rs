// SPDX-License-Identifier: AGPL-3.0-only
//! Which home a line on this computer works on: the one WSP_HOME names, or this computer's own. Port of
//! `packages/host/src/serving-home.ts` at 65f6b89c7; the spec is `spec/account.md` (the files-and-readings unit).
use crate::hosts::Env;

/// serving-home.ts:15: `join(user, ".wsp")`.
pub fn default_home_in(user: &str) -> String {
    todo!()
}

/// serving-home.ts:19: unset and empty are one answer; `" "` is a home.
pub fn home_named(value: Option<&str>) -> Option<String> {
    todo!()
}

/// serving-home.ts:22-24.
pub fn serving_home(env: &Env, user: &str) -> String {
    todo!()
}

/// serving-home.ts:28-32.
pub fn real_state(path: &str) -> String {
    todo!()
}

/// serving-home.ts:38-41.
pub fn serving_elsewhere(state_path: &str, env: &Env, user: &str) -> Option<String> {
    todo!()
}

/// serving-home.ts:48-51, byte for byte: two lines joined by one newline, no trailing newline.
pub const SERVING_HOME_SH: &str = concat!(
    r#"wsp_serving() { [ -f "$1/host.lock" ] || return 1; pid=$(sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$1/host.lock"); [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; }"#,
    "\n",
    r#"home="${WSP_HOME:-$HOME/.wsp}""#
);
