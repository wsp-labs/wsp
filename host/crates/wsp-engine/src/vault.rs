// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/vault.ts at a4033dc34. Specs: spec/vault_export.md (vault.ts:1-287) and
//! spec/vault_import.md (vault.ts:289-539).
//!
//! Folders off a machine as a gzipped tar, and a tar onto one: export reads the archive back over the provider's
//! signed URL (or base64 through exec, capped), import sends it up in parts of at most 32 MiB and checks its sha256 on
//! the guest before extracting. A plain TS `Error` is `EngineError::Other { message, code: None }`; the TS's `kind:
//! "vaultTooLarge"` is `EngineError::VaultTooLarge` and `kind: "exists"` is `EngineError::Exists`.
use std::sync::Arc;

use crate::fetch::Fetch;

mod export;
mod import;

pub use export::*;
pub use import::*;

/// vault.ts:18. Every field optional in the TS; `Default` is the TS's `{}`.
#[derive(Clone, Default)]
pub struct VaultOptions {
    /// `opts.fetch ?? globalThis.fetch`: `None` is `crate::fetch::platform_fetch()`.
    pub fetch: Option<Arc<dyn Fetch>>,
    pub timeout_ms: Option<u64>,
    /// Export only: an archive over this many bytes is removed and refused with `VaultTooLarge`.
    pub max_bytes: Option<u64>,
    /// export_paths only: what the archive leaves behind under each path.
    pub exclude: Option<CacheRule>,
    /// Import only: merge into what the destination holds instead of replacing its directories.
    pub overlay: bool,
    /// export_paths and import_into: absolute guest paths left out of the archive and out of what lands. The TS's
    /// `undefined` and `[]` read the same everywhere, so this is a plain list.
    pub leave: Vec<String>,
    /// The folder on the machine the trip's own parts are written in; `GUEST_TMP` when `None`.
    pub tmp_dir: Option<String>,
    /// Import only: called as each part lands.
    pub on_part: Option<Arc<dyn Fn(UploadProgress) + Send + Sync>>,
    /// Export only: called as the archive comes down.
    pub on_progress: Option<Arc<dyn Fn(DownloadProgress) + Send + Sync>>,
    /// Export only: how the archive comes off the machine.
    pub read_road: ReadRoad,
}

/// vault.ts:42 `readRoad`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReadRoad {
    #[default]
    SignedUrl,
    Exec,
}

/// vault.ts:49.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub bytes: u64,
    pub total: u64,
}

/// vault.ts:54.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadProgress {
    pub part: u32,
    pub parts: u32,
    pub bytes: u64,
    pub total: u64,
    /// The pieces the part was cut into where the road it took cuts them; `None` on a road that takes it whole.
    pub pieces: Option<u32>,
}

/// vault.ts:134. Names, never substrings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CacheRule {
    pub dirs: Vec<String>,
    pub files: Vec<String>,
    pub markers: Vec<String>,
}

/// vault.ts:182. Paths to archive from one root on the guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveGroup {
    pub root: String,
    pub paths: Vec<String>,
}
