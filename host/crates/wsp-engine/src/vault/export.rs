// SPDX-License-Identifier: AGPL-3.0-only
//! vault.ts:45-287 at a4033dc34: the archive off a machine. Spec: spec/vault_export.md.
use std::path::Path;

use tokio::io::AsyncWrite;

use super::{ArchiveGroup, CacheRule, VaultOptions};
use crate::errors::{EngineError, Result};
use crate::machine::Machine;

/// vault.ts:47. The most an archive read back through exec may weigh: 16 MiB.
pub const EXEC_READ_CAP: u64 = 16 * 1024 * 1024;

/// vault.ts:70 `download`: the archive at `path` on the guest into `sink` as it arrives. `ReadRoad::Exec` goes to
/// [`exec_read`]. `total` is the size read on the guest when the caller has it.
pub async fn download(
    machine: &dyn Machine,
    path: &str,
    sink: &mut (dyn AsyncWrite + Unpin),
    opts: &VaultOptions,
    total: Option<u64>,
) -> Result<()> {
    todo!()
}

/// vault.ts:92 `execRead`: the archive as base64 on exec's stdout, refused over [`EXEC_READ_CAP`] before any of it is
/// read.
pub async fn exec_read(
    machine: &dyn Machine,
    path: &str,
    sink: &mut (dyn AsyncWrite + Unpin),
    opts: &VaultOptions,
    total: Option<u64>,
) -> Result<()> {
    todo!()
}

/// vault.ts:107 `downloadBuffer`: a download held in memory.
pub async fn download_buffer(machine: &dyn Machine, path: &str, opts: &VaultOptions, total: Option<u64>) -> Result<Vec<u8>> {
    todo!()
}

/// vault.ts:115 `downloadFile`: a download streamed into a file on this computer; a download that fails leaves no
/// file behind. Answers the bytes the file holds (its size on disk once closed).
pub async fn download_file(machine: &dyn Machine, path: &str, into: &Path, opts: &VaultOptions, total: Option<u64>) -> Result<u64> {
    todo!()
}

/// vault.ts:145 `excludingArchiveScript`: the script that archives `targets`, relative to `cwd`, into `out` with
/// the rule's caches left behind and named on stdout. Byte for byte the TS's text (tests/fixtures/oracle).
pub fn excluding_archive_script(cwd: &str, targets: &[String], rule: &CacheRule, out: &str, leave: &[String]) -> String {
    todo!()
}

/// vault.ts:169.
pub fn folder_export_script(dir: &str, rule: &CacheRule, out: &str) -> String {
    todo!()
}

/// vault.ts:177 `guestTmpPath(what, dir = GUEST_TMP)`: `<dir with trailing slashes cut>/<what>-<ms since epoch>-<six
/// base-36 chars>`.
pub fn guest_tmp_path(what: &str, dir: Option<&str>) -> String {
    todo!()
}

/// vault.ts:189 `tarPath`: a path as tar names it from its group's root. Root `/` cuts one leading slash and keeps
/// a relative path as it is; any other root is `path.relative(root, path)` (posix, lexical, no filesystem read).
pub fn tar_path(root: &str, path: &str) -> String {
    todo!()
}

/// vault.ts:193 `leaveFlags`: ` --exclude=<quoted tar_path(root, p)>` per left path, each with its leading space.
pub fn leave_flags(root: &str, leave: &[String]) -> String {
    todo!()
}

/// vault.ts:196 `groupArchiveScript`.
pub fn group_archive_script(groups: &[ArchiveGroup], out: &str, leave: &[String]) -> String {
    todo!()
}

/// vault.ts:200 `guestFileSize`: `wc -c < <path>` inline, read as JS `Number(stdout.trim())`: an empty stdout is 0,
/// `0x10` is 16. A non-zero exit or a non-finite number fails.
pub async fn guest_file_size(machine: &dyn Machine, path: &str) -> Result<u64> {
    todo!()
}

/// vault.ts:235 `keptPaths`: the paths whose last name (trailing slashes cut) is in neither `rule.dirs` nor
/// `rule.files`, in order; refuses when none is kept.
pub fn kept_paths(paths: &[String], rule: &CacheRule) -> Result<Vec<String>> {
    todo!()
}

/// vault.ts:248 `emptyArchiveScript`.
pub fn empty_archive_script(out: &str) -> String {
    todo!()
}

/// vault.ts:251 `exportPaths`: the paths archived from `/` and read back into memory.
pub async fn export_paths(machine: &dyn Machine, paths: &[String], opts: &VaultOptions) -> Result<Vec<u8>> {
    todo!()
}

/// vault.ts:265 `exportPathsInto`: the groups into a file on this computer. Answers the bytes written.
pub async fn export_paths_into(machine: &dyn Machine, groups: &[ArchiveGroup], into: &Path, opts: &VaultOptions) -> Result<u64> {
    todo!()
}

/// vault.ts:270 `destExists`: `EngineError::Exists` whose message is `<dest> already exists on this computer with
/// <plural(files, "file")>; export with replace to overwrite it`.
pub fn dest_exists(dest: &str, files: u64) -> EngineError {
    todo!()
}

/// vault.ts:275 `exportFolder`'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderExport {
    pub bytes: u64,
    /// The cache roots left behind, as find spelled them less a leading `./`, sorted by UTF-16 code unit.
    pub excluded: Vec<String>,
}

/// vault.ts:275 `exportFolder`.
pub async fn export_folder(machine: &dyn Machine, dir: &str, rule: &CacheRule, into: &Path, opts: &VaultOptions) -> Result<FolderExport> {
    todo!()
}
