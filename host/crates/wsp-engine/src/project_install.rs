// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-install.ts. Spec: spec/small.md.
//!
//! The install a project's dependencies are fetched by, once, on the computer that holds the checkout.
use std::sync::LazyLock;

use indexmap::IndexMap;

/// project-install.ts:11.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInstall {
    /// The catalog row's id.
    pub row: String,
    pub command: String,
    /// The manager's version command, where the catalog's row names one.
    pub check: Option<String>,
}

/// project-install.ts:21.
pub const INSTALL_MS: u64 = 30 * 60_000;
/// project-install.ts:24.
pub const CHECK_MS: u64 = 5 * 60_000;

/// project-install.ts:31. `seed_installs_for(root_names)` in its order; the command is the install's `run`, or where
/// it names a store `<run> <flag> <shell_quote(<project_dir less trailing slashes>/<dir>)>`.
pub fn project_installs(root_names: &[String], project_dir: &str) -> Vec<ProjectInstall> {
    todo!()
}

/// project-install.ts:37. Every install's store dir across SEED_ROWS, in row then install order, duplicates kept.
pub static INSTALL_STORES: LazyLock<Vec<String>> = LazyLock::new(|| todo!());

/// project-install.ts:47. `mkdir -p <q log's folder> && cd <q dir> && <command> >> <q log> 2>&1`; the folder is the log
/// with its last `/name` cut (`/[^/]+$`), the whole log where it has no slash.
pub fn install_script(install: &ProjectInstall, dir: &str, log: &str) -> String {
    todo!()
}

/// project-install.ts:54. Two scripts: `export K=<q v>` per variable in order, then `cd <q dir> && <check>`, joined by
/// newlines; and `cd <q dir> && env -i HOME=<q GUEST_HOME> bash -lc <q check>`.
pub fn check_scripts(check: &str, dir: &str, vars: &IndexMap<String, String>) -> [String; 2] {
    todo!()
}
