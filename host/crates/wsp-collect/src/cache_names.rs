// SPDX-License-Identifier: AGPL-3.0-only
//! Names that mean a cache, an install or Finder metadata: what a machine recreates and a copy never carries. Port of
//! packages/collect/src/cache-names.ts at a3b870a6e.

pub use wsp_wire::types::daemon_contract::CACHE_DIRS;

/// Finder's per-directory metadata, a file by exact name; it means nothing on a Linux machine.
pub const FINDER_METADATA: &str = ".DS_Store";
