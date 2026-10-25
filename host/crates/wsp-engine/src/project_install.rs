// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-install.ts. Spec: spec/small.md.
//!
//! The install a project's dependencies are fetched by, once, on the computer that holds the checkout.
use std::sync::LazyLock;

use indexmap::IndexMap;
use wsp_catalog::roads::GUEST_HOME;
use wsp_catalog::seed::{seed_installs_for, SeedInstall, SEED_ROWS};
use wsp_wire::types::shell_quote::shell_quote;

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
    seed_installs_for(root_names)
        .into_iter()
        .map(|(row, install)| ProjectInstall {
            row: row.id.clone(),
            command: command_of(install, project_dir),
            check: install.check.clone(),
        })
        .collect()
}

fn command_of(install: &SeedInstall, project_dir: &str) -> String {
    match &install.store {
        None => install.run.clone(),
        Some(store) => {
            let dir = format!("{}/{}", project_dir.trim_end_matches('/'), store.dir);
            format!("{} {} {}", install.run, store.flag, shell_quote(&dir))
        }
    }
}

/// project-install.ts:37. Every install's store dir across SEED_ROWS, in row then install order, duplicates kept.
pub static INSTALL_STORES: LazyLock<Vec<String>> = LazyLock::new(|| {
    SEED_ROWS
        .iter()
        .flat_map(|row| row.installs.iter().flatten())
        .filter_map(|install| install.store.as_ref().map(|store| store.dir.clone()))
        .collect()
});

/// project-install.ts:47. `mkdir -p <q log's folder> && cd <q dir> && <command> >> <q log> 2>&1`; the folder is the log
/// with its last `/name` cut (`/[^/]+$`), the whole log where it has no slash.
pub fn install_script(install: &ProjectInstall, dir: &str, log: &str) -> String {
    let folder = match log.rfind('/') {
        Some(at) if at + 1 < log.len() => &log[..at],
        _ => log,
    };
    format!(
        "mkdir -p {} && cd {} && {} >> {} 2>&1",
        shell_quote(folder),
        shell_quote(dir),
        install.command,
        shell_quote(log)
    )
}

/// project-install.ts:54. Two scripts: `export K=<q v>` per variable in order, then `cd <q dir> && <check>`, joined by
/// newlines; and `cd <q dir> && env -i HOME=<q GUEST_HOME> bash -lc <q check>`.
pub fn check_scripts(check: &str, dir: &str, vars: &IndexMap<String, String>) -> [String; 2] {
    let cd = format!("cd {}", shell_quote(dir));
    let exports = vars.iter().map(|(name, value)| format!("export {name}={}", shell_quote(value)));
    let first = exports.chain(std::iter::once(format!("{cd} && {check}"))).collect::<Vec<_>>().join("\n");
    let second = format!("{cd} && env -i HOME={} bash -lc {}", shell_quote(GUEST_HOME), shell_quote(check));
    [first, second]
}
