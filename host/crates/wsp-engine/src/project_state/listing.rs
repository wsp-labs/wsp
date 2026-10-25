// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/listing.ts. Spec: spec/project_state_scripts.md.
//!
//! Which paths under an agent's home on the machine hold one project's state: a python3 script the machine runs, so a
//! trip pulls only the paths it prints, and a store kept for every project travels as a filtered copy.
use serde::{Deserialize, Serialize};

use crate::errors::Result;
use crate::project_state::resolver::SharedStore;

/// listing.ts:16. The step that names a module's whole roots.
pub const ROOTS_STEP: &str = "for r in ROOTS:\n    say(r)";

/// listing.ts:22. The mark a listing prints an unreadable store on, ahead of its base64 JSON.
pub const UNREAD_MARK: &str = "!unread ";

/// listing.ts:26. Keys in this order on the wire: agent, store, why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnreadStore {
    pub agent: String,
    pub store: String,
    pub why: String,
}

/// listing.ts:33 answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateListing {
    pub paths: Vec<String>,
    pub unread: Vec<UnreadStore>,
}

/// listing.ts:33. Each non-empty line of stdout split on `\n`: a line opening with `UNREAD_MARK` is the base64 JSON of
/// an UnreadStore, any other a path, both in order. A marked line that does not decode or parse is a failure (the TS
/// `JSON.parse` throws).
pub fn parse_state_listing(stdout: &str) -> Result<StateListing> {
    todo!()
}

/// listing.ts:50. The whole listing script for one agent's home, byte for byte as the TS builds it (pinned by the
/// oracle): preamble, AGENT, HOME, PATH normalised, ROOTS, OUT, UNREAD, say(), unread(), the steps indented four spaces
/// inside `try:`, the roots fallback, the prints.
pub fn list_script(agent: &str, home: &str, path: &str, roots: &[String], steps: &[String]) -> String {
    todo!()
}

/// listing.ts:88. The keyed-directories step, `key` a python expression for the module's key rule.
pub fn keyed_step(parent: &str, key: &str) -> String {
    todo!()
}

/// listing.ts:127. `join(scratch, home, store)`, node's posix join: one `/` between parts, `.` and `..` resolved,
/// repeated slashes collapsed, a trailing slash kept.
pub fn store_copy(scratch: &str, home: &str, store: &str) -> String {
    todo!()
}

/// listing.ts:137. The step that writes a store's rows for the project into its copy, names it and runs the module's
/// steps, each indented eight spaces inside its `try:`.
pub fn filter_step(scratch: &str, home: &str, shared: &SharedStore, steps: &[String]) -> String {
    todo!()
}
