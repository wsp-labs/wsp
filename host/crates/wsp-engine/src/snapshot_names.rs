// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/snapshot-names.ts. Spec: spec/sweeps.md.
//!
//! The mark wsp writes into the name of every snapshot and template it makes, and the one parser that reads it back.
use crate::errors::Result;

/// snapshot-names.ts:9. Every name below opens with this word.
pub const MARK: &str = "wsp";

/// snapshot-names.ts:23. `wsp-<host>-<golden>-v<version>`. Refuses a host that is not `^[a-z0-9]+$` (empty too) with
/// `a snapshot name needs a host of lowercase letters and digits, got <JSON.stringify(host)>` as an `Other`.
pub fn golden_name(host: &str, golden: &str, version: u64) -> Result<String> {
    todo!()
}

/// snapshot-names.ts:26. `wsp-<host>-project-<project>-<stamp>`, the same host refusal.
pub fn project_snapshot_name(host: &str, project: &str, stamp: &str) -> Result<String> {
    todo!()
}

/// snapshot-names.ts:29. The host that made this snapshot or template: the `[a-z0-9]+` after `wsp-` of a name matching
/// `^wsp-([a-z0-9]+)-(.+)$`; `None` for no name or a name without the mark.
pub fn name_owner(name: Option<&str>) -> Option<String> {
    todo!()
}
