// SPDX-License-Identifier: AGPL-3.0-only
//! The release every catalog row on the release road installs. Port of packages/catalog/src/release-pins.ts at
//! a3b870a6e; the spec is spec/roads.md.
use std::sync::LazyLock;

use indexmap::IndexMap;

use crate::pending::HostError;
use crate::roads::{InstallRoad, ReleaseAssets};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasePin {
    pub tag: String,
    pub assets: ReleaseAssets,
}

/// By repository, in the table's order (release-pins.ts:342).
pub static RELEASE_PINS: LazyLock<IndexMap<String, ReleasePin>> = LazyLock::new(|| todo!());

/// The release road at the pinned tag and assets, `go` set where given; errors "the release pins table names no
/// <repo>" (release-pins.ts:373).
pub fn pinned_release(repo: &str, go: Option<&str>) -> Result<InstallRoad, HostError> {
    todo!()
}
