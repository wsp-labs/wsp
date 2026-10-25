// SPDX-License-Identifier: AGPL-3.0-only
//! One row per ecosystem for the files a folder seeds a project with, and the install each runs. Port of
//! packages/catalog/src/seed.ts at a3b870a6e; the spec is spec/seed.md.
use std::sync::LazyLock;

use indexmap::IndexMap;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SeedKind {
    Config,
    Secret,
    Rebuilt,
    Data,
    Never,
    Junk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeedStore {
    pub flag: String,
    pub dir: String,
}

/// seed.ts:22.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeedInstall {
    pub lockfile: String,
    pub run: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<SeedStore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
}

/// seed.ts:39.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeedRow {
    pub id: String,
    pub name: String,
    pub patterns: IndexMap<SeedKind, Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub except: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opens: Option<Vec<String>>,
    pub examples: IndexMap<SeedKind, Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installs: Option<Vec<SeedInstall>>,
}

/// seed.ts:59, in the order a path is judged by.
pub static SEED_ROWS: LazyLock<Vec<SeedRow>> = LazyLock::new(|| todo!());

/// The row and kind of one ignored path, kinds in never, junk, secret, config, rebuilt, data order and rows in table
/// order within each (seed.ts:203).
pub fn seed_row_for(path: &str, dir: bool) -> Option<(&'static SeedRow, SeedKind)> {
    todo!()
}

/// The installs the root names, one per row, the first listed lockfile there winning (seed.ts:217).
pub fn seed_installs_for(root_names: &[String]) -> Vec<(&'static SeedRow, &'static SeedInstall)> {
    todo!()
}
