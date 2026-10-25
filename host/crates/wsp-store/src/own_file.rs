// SPDX-License-Identifier: AGPL-3.0-only
//! One writer for everything under the owner's state folder: a 0600 temporary file renamed into place under folders
//! that stand at 0700. Port of packages/own-file/src/index.ts at a3b870a6e; the spec is spec/own_file.md.
use std::ffi::OsStr;
use std::fs::{self, DirBuilder, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rand_core::{OsRng, RngCore};

/// What a folder of the owner's stands at (index.ts:13).
pub const OWN_DIR_MODE: u32 = 0o700;
/// What a file of the owner's is written with (index.ts:14).
pub const OWN_FILE_MODE: u32 = 0o600;

/// A folder of the owner's: made with its parents when missing, and chmodded to 0700 when it stands at anything else
/// (index.ts:18). Only `dir` itself is repaired; a parent this call made keeps the mode mkdir gave it.
pub fn own_folder(dir: &Path) -> io::Result<()> {
    if let Err(made) = DirBuilder::new().recursive(true).mode(OWN_DIR_MODE).create(dir) {
        if made.kind() != io::ErrorKind::AlreadyExists {
            return Err(made);
        }
        // node stats a name that exists before it refuses it, so a dangling link fails with that stat's error
        if !fs::metadata(dir)?.is_dir() {
            return Err(made);
        }
    }
    if fs::metadata(dir)?.permissions().mode() & 0o777 != OWN_DIR_MODE {
        fs::set_permissions(dir, Permissions::from_mode(OWN_DIR_MODE))?;
    }
    Ok(())
}

/// One file of the owner's, written under `root` at `rel`, joined as node's `path.join` joins (index.ts:27).
pub fn write_own(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    let target = join(root, rel.as_bytes());
    own_folder(root)?;
    let folder = target.parent().unwrap_or(&target);
    let (_, root_parts) = normalize(root.as_os_str().as_bytes());
    let (_, folder_parts) = normalize(folder.as_os_str().as_bytes());
    let common = root_parts.iter().zip(&folder_parts).take_while(|(a, b)| a == b).count();
    let climb = (0..root_parts.len() - common).map(|_| &b".."[..]);
    let mut at = root.to_path_buf();
    for part in climb.chain(folder_parts[common..].iter().copied()) {
        at = join(&at, part);
        own_folder(&at)?;
    }

    let mut tmp = target.as_os_str().to_owned();
    tmp.push(format!(".{}-{}-{}.tmp", std::process::id(), millis(), random_suffix()));
    let mut file = OpenOptions::new().write(true).create_new(true).mode(OWN_FILE_MODE).open(&tmp)?;
    file.write_all(bytes.as_ref())?;
    drop(file);
    fs::set_permissions(&tmp, Permissions::from_mode(OWN_FILE_MODE))?;
    fs::rename(&tmp, &target)
}

/// node's `path.join(base, rel)` on posix: the joined text is normalised as a whole, so a leading `/` in `rel` stays
/// under `base` where `Path::join` would replace `base`.
fn join(base: &Path, rel: &[u8]) -> PathBuf {
    let mut text = base.as_os_str().as_bytes().to_vec();
    if !rel.is_empty() {
        text.push(b'/');
        text.extend_from_slice(rel);
    }
    let (absolute, parts) = normalize(&text);
    let path = to_path(absolute, &parts);
    if text.ends_with(b"/") && !parts.is_empty() {
        let mut kept = path.into_os_string();
        kept.push("/");
        return PathBuf::from(kept);
    }
    path
}

/// node's `path.normalize` on posix: empty and `.` parts drop, `..` takes back the part before it, and an absolute
/// path has nothing above its root to climb to.
fn normalize(text: &[u8]) -> (bool, Vec<&[u8]>) {
    let absolute = text.first() == Some(&b'/');
    let mut parts: Vec<&[u8]> = Vec::new();
    for part in text.split(|&b| b == b'/') {
        match part {
            b"" | b"." => {}
            b".." if parts.last().is_some_and(|last| *last != b"..") => {
                parts.pop();
            }
            b".." if absolute => {}
            _ => parts.push(part),
        }
    }
    (absolute, parts)
}

fn to_path(absolute: bool, parts: &[&[u8]]) -> PathBuf {
    let mut path = if absolute { PathBuf::from("/") } else { PathBuf::new() };
    for part in parts {
        path.push(OsStr::from_bytes(part));
    }
    if path.as_os_str().is_empty() {
        path.push(".");
    }
    path
}

fn millis() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis())
}

fn random_suffix() -> String {
    const BASE36: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    (0..6).map(|_| BASE36[OsRng.next_u32() as usize % 36] as char).collect()
}
