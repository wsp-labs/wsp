// SPDX-License-Identifier: AGPL-3.0-only
//! provision-files.ts:649-796 at a4033dc34: what a setup writes outside the home, recorded around each install step
//! and taken off by a leave. Spec: spec/files_outside.md.

/// provision-files.ts:650.
pub const OUTSIDE_MARK: &str = "wsp-outside";
/// provision-files.ts:654. The last line of a listing that ran to its end.
pub const LISTED: &str = "wsp-listed";

/// provision-files.ts:660 `outsideHead(root)`.
pub fn outside_head(root: &str) -> Vec<String> {
    todo!()
}

/// provision-files.ts:667 `OUTSIDE_STATES`, the `states` shell function's lines.
pub fn outside_states() -> Vec<String> {
    todo!()
}

/// provision-files.ts:684 `OUTSIDE_STILL`, the `still` shell function's lines.
pub fn outside_still() -> Vec<String> {
    todo!()
}

/// provision-files.ts:705 `outsideListing(walked, out, tests = "")`.
pub fn outside_listing(walked: &[String], out: &str, tests: &str) -> Vec<String> {
    todo!()
}

/// provision-files.ts:718 `outsideBeforeScript(step, walked, root = "")`: `exit 0` when nothing is walked. Byte for
/// byte the TS's text.
pub fn outside_before_script(step: &str, walked: &[String], root: &str) -> String {
    todo!()
}

/// provision-files.ts:742 `outsideAfterScript(step, walked, root = "")`: `exit 0` when nothing is walked. Byte for
/// byte the TS's text.
pub fn outside_after_script(step: &str, walked: &[String], root: &str) -> String {
    todo!()
}

/// provision-files.ts:767 `outsideSweepScript(root = "")`. Byte for byte the TS's text; `outside_sweep_script("{root}")`
/// plus a newline is `daemon/fixtures/contract/landed-outside.sh`.
pub fn outside_sweep_script(root: &str) -> String {
    todo!()
}

/// provision-files.ts:790 `outsideMarks`: in the order printed; a line counts with the mark first and a path (every
/// field from the second on, joined by tabs) that starts with `/`.
pub fn outside_marks(stdout: &str) -> Vec<String> {
    todo!()
}
