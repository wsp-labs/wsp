// SPDX-License-Identifier: AGPL-3.0-only
//! vault.ts:289-539 at a4033dc34: a tar onto a machine. Spec: spec/vault_import.md.
use std::sync::Arc;

use super::{UploadProgress, VaultOptions};
use crate::errors::{EngineError, Result};
use crate::fetch::Fetch;
use crate::machine::Machine;

/// vault.ts:291. The signed upload URL takes one PUT of at most 32 MiB.
pub const UPLOAD_PART_BYTES: u64 = 32 * 1024 * 1024;

/// vault.ts:304 `uploadFailure`: the sentence for a refused part. `where` is empty for a one-part archive, else ` on
/// part <part> of <parts>`; the reason is the body's JSON `error` where it is a string, else the trimmed body's first
/// 300 UTF-16 units; ` after <attempts> attempts` only past one attempt; `; the upload takes at most <limit> bytes per
/// PUT` only where the body's `limit` is a number.
pub fn upload_failure(status: u16, body: &str, part: u32, parts: u32, attempts: u32) -> String {
    todo!()
}

/// vault.ts:315 `putPart`: one part PUT until it lands, at most three attempts, sleeping
/// `crate::errors::backoff_ms(attempt, random)` between them.
pub async fn put_part(fetch: &dyn Fetch, url: &str, bytes: &[u8], part: u32, parts: u32) -> Result<()> {
    todo!()
}

/// vault.ts:335-351 `TarEntry`: a file, a directory of its own, or a symbolic link carried as one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarEntry {
    File { path: String, mode: u32, content: Vec<u8> },
    Dir { path: String, mode: u32 },
    Link { path: String, target: String },
}

/// vault.ts:355. The ustar size field is eleven octal digits.
pub const TAR_MAX_FILE_BYTES: u64 = 0o77777777777;
/// vault.ts:356.
pub const TAR_LINK_MAX: usize = 100;

/// vault.ts:363 `splitName`: the path whole when it is at most 100 bytes; else the last slash (right to left, never
/// at index 0) that leaves a prefix of at most 155 bytes and a name of at most 100. `None` when no cut fits.
pub fn split_name(path: &str) -> Option<(String, String)> {
    todo!()
}

/// vault.ts:380 `fitsTar`: leading slashes cut, the path splits, and the target (where given) is at most 100 bytes.
pub fn fits_tar(path: &str, target: Option<&str>) -> bool {
    todo!()
}

/// vault.ts:386 `tarOf` with its clock handed in: the ustar blocks of the entries, each header's mtime `mtime_secs`,
/// two zero blocks after, then gzipped. The gzip framing is flate2's; the tar inside is byte for byte the TS's.
pub fn tar_of_at(entries: &[TarEntry], mtime_secs: u64) -> Result<Vec<u8>> {
    todo!()
}

/// vault.ts:386 `tarOf`: [`tar_of_at`] at the current second (`floor(now_ms / 1000)`).
pub fn tar_of(entries: &[TarEntry]) -> Result<Vec<u8>> {
    todo!()
}

/// vault.ts:422. A hash mismatch on the guest exits 65.
pub const HASH_MISMATCH_EXIT: i64 = 65;

/// vault.ts:424 `importInto`'s answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Imported {
    pub parts: u32,
}

/// vault.ts:424 `importInto`.
pub async fn import_into(machine: &dyn Machine, tar: &[u8], dest_dir: &str, opts: &VaultOptions) -> Result<Imported> {
    todo!()
}

/// vault.ts:478 `keptAside`'s three texts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptAside {
    pub r#where: String,
    pub aside: String,
    /// The `back` function's definition with this `rm -f` of the parts inside it.
    pub back: String,
}

/// vault.ts:478 `keptAside(destDir, leave, keep)` with `back` already given `remove_parts`. `None` when nothing is
/// left.
pub fn kept_aside(dest_dir: &str, leave: &[String], keep: &str, remove_parts: &str) -> Option<KeptAside> {
    todo!()
}

/// vault.ts:489.
#[derive(Clone, Default)]
pub struct LandOptions {
    pub fetch: Option<Arc<dyn Fetch>>,
    pub timeout_ms: Option<u64>,
    pub tmp_dir: Option<String>,
    /// Remove what is at the destination first; without it an existing path is refused with `Exists`.
    pub replace: bool,
    pub on_part: Option<Arc<dyn Fn(UploadProgress) + Send + Sync>>,
    /// Called once the archive is extracted, right before it is moved into place.
    pub on_landing: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// vault.ts:503. The landing refuses an existing path with exit 66.
pub const EXISTS_EXIT: i64 = 66;

/// vault.ts:505 `exists`: `EngineError::Exists` whose message is `<path> already exists on the machine; import with
/// replace to overwrite it`.
pub fn exists_on_machine(path: &str) -> EngineError {
    todo!()
}

/// vault.ts:510 `landBundle`.
pub async fn land_bundle(machine: &dyn Machine, tar: &[u8], dest: &str, opts: &LandOptions) -> Result<Imported> {
    todo!()
}
