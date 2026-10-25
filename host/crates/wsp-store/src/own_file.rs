// SPDX-License-Identifier: AGPL-3.0-only
//! One writer for everything under the owner's state folder: a 0600 temporary file renamed into place under folders
//! that stand at 0700. Port of packages/own-file/src/index.ts at a3b870a6e; the spec is spec/own_file.md.
use std::io;
use std::path::Path;

/// What a folder of the owner's stands at (index.ts:13).
pub const OWN_DIR_MODE: u32 = 0o700;
/// What a file of the owner's is written with (index.ts:14).
pub const OWN_FILE_MODE: u32 = 0o600;

/// A folder of the owner's: made with its parents when missing, and chmodded to 0700 when it stands at anything else
/// (index.ts:18). Only `dir` itself is repaired; a parent this call made keeps the mode mkdir gave it.
pub fn own_folder(dir: &Path) -> io::Result<()> {
    todo!()
}

/// One file of the owner's, written under `root` at `rel`, joined as node's `path.join` joins (index.ts:27).
pub fn write_own(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    todo!()
}
