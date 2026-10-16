// SPDX-License-Identifier: AGPL-3.0-only
//! A new daemon binary from the host, landed part by part under the put folder and moved over the running one.

use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use wsp_frames::words;

use super::{place_daemon_paths, sha256_hex};
use crate::beneath;
use crate::under_home::open_under_home;

/// Where the parts of an update are appended while they arrive: under the place's own put folder, named after the
/// upload, so nothing half landed ever sits where the binary the unit starts does.
pub(crate) fn update_part(home: &Path, upload_id: &str) -> PathBuf {
    place_daemon_paths(home).put_dir.join(upload_id)
}

/// Where the seq this upload expects next is written, beside its bytes. Two files rather than one: the bytes are
/// appended to and their length says nothing about how many parts made them, since a part is whatever the host cut.
fn update_next(part: &Path) -> PathBuf {
    part.with_extension("next")
}

/// The most of a seq read, and of a landed binary: a daemon is under 10 MB.
const UPDATE_NEXT_MAX_BYTES: u64 = 32;
const UPDATE_MAX_BYTES: u64 = 256 << 20;

/// Every plain file in the put folder except the upload named's, removed. A link that drops mid-upload leaves its
/// parts there and the host picks a fresh id for its next try, so without this the box keeps every abandoned attempt.
/// Run when the agent starts and again when an upload begins, which are the two moments nothing else is arriving. The
/// folder is a home's local threads write: it is opened with no link followed on the way or at its name, and each name
/// in it looked at and unlinked on its descriptor, so a link there or in it is never followed and stays where it is.
pub(crate) fn sweep_updates(home: &Path, keep: Option<&str>) -> usize {
    let put = place_daemon_paths(home).put_dir;
    let held = put.strip_prefix(home).ok().and_then(|rel| open_under_home(home, rel, false).ok());
    let Some(mut folder) = held.and_then(|fd| nix::dir::Dir::from_fd(fd).ok()) else { return 0 };
    let names: Vec<std::ffi::CString> = folder.iter().filter_map(Result::ok).map(|entry| entry.file_name().to_owned()).collect();
    let mut swept = 0;
    for name in names {
        let name = std::ffi::OsStr::from_bytes(name.as_bytes());
        if keep.is_some() && Path::new(name).file_stem().and_then(std::ffi::OsStr::to_str) == keep {
            continue;
        }
        if unlink_plain(&folder, name) {
            swept += 1;
        }
    }
    swept
}

/// A name in a held folder unlinked on its descriptor where a plain file stands there, after a look that follows no
/// link: a link, a fifo or a folder there stays. Answers whether it went.
fn unlink_plain(folder: &impl std::os::fd::AsFd, name: &std::ffi::OsStr) -> bool {
    let plain = nix::sys::stat::fstatat(folder, name, nix::fcntl::AtFlags::AT_SYMLINK_NOFOLLOW)
        .is_ok_and(|meta| wsp_runtime::file_type::is_regular(meta.st_mode));
    plain && nix::unistd::unlinkat(folder, name, nix::unistd::UnlinkatFlags::NoRemoveDir).is_ok()
}

/// The put folder, reached from the home with no link followed on the way or at its own name and made where it is not
/// there yet. An upload's part and seq are read, written and taken off on this descriptor alone, so a link at ~/.wsp,
/// at the put folder or in it never points the update at a folder a home's threads chose.
fn put_folder(home: &Path) -> Result<OwnedFd, String> {
    let put = place_daemon_paths(home).put_dir;
    let rel = put.strip_prefix(home).map_err(|_| format!("{} is not under {}", put.display(), home.display()))?;
    open_under_home(home, rel, true).map_err(|e| format!("{} is not there as a folder: {e}", put.display()))
}

/// The folder the running binary sits in, held: under the home, which a home's threads write, through the home's walk
/// with no link followed on the way or at its own name; anywhere else as it stands.
fn binary_folder(home: &Path, exe: &Path) -> Result<OwnedFd, String> {
    let folder = exe.parent().ok_or_else(|| format!("{} sits in no folder", exe.display()))?;
    match folder.strip_prefix(home) {
        Ok(rel) if !rel.as_os_str().is_empty() => {
            open_under_home(home, rel, false).map_err(|e| format!("{} is not there as a folder: {e}", folder.display()))
        }
        _ => nix::fcntl::open(
            folder,
            nix::fcntl::OFlag::O_RDONLY | nix::fcntl::OFlag::O_DIRECTORY | nix::fcntl::OFlag::O_CLOEXEC,
            nix::sys::stat::Mode::empty(),
        )
        .map_err(|e| format!("{}: {e}", folder.display())),
    }
}

/// A name in a held folder opened to write with no link followed and no wait on a fifo, created where it is not there.
fn open_in(folder: &OwnedFd, at: &Path, more: nix::fcntl::OFlag) -> Result<std::fs::File, String> {
    use nix::fcntl::OFlag;
    let name = at.file_name().ok_or_else(|| format!("{} names no file", at.display()))?;
    let flags = OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC | more;
    let file = match nix::fcntl::openat(folder, name, flags, nix::sys::stat::Mode::from_bits_truncate(0o644)) {
        Ok(fd) => std::fs::File::from(fd),
        Err(nix::errno::Errno::ELOOP) => return Err(format!("a link stands at {}", at.display())),
        Err(e) => return Err(format!("{}: {e}", at.display())),
    };
    file.metadata().is_ok_and(|m| m.is_file()).then_some(file).ok_or_else(|| format!("{} is not a file", at.display()))
}

/// An upload's part and the seq beside it taken off the held put folder, plain files only.
fn drop_upload(put: &OwnedFd, part: &Path) {
    for at in [part.to_path_buf(), update_next(part)] {
        if let Some(name) = at.file_name() {
            unlink_plain(put, name);
        }
    }
}

/// One part appended under the seq this upload is waiting for: 0 for an upload nothing has been landed for, and the
/// one after the last part taken from then on. A part that is not it, a repeat or a skip alike, drops the upload
/// rather than letting it become a file with a hole in it or the same bytes twice.
pub(crate) fn take_update_part(home: &Path, upload_id: &str, seq: u64, bytes: &[u8]) -> Result<(), String> {
    let put = put_folder(home)?;
    let part = &update_part(home, upload_id);
    let next_name = update_next(part).file_name().and_then(std::ffi::OsStr::to_str).map(str::to_owned).unwrap_or_default();
    let dropped = |part: &Path| drop_upload(&put, part);
    let next = beneath::read_capped_in(&put, &next_name, UPDATE_NEXT_MAX_BYTES)
        .map_err(|e| format!("{}: {e}", place_daemon_paths(home).put_dir.display()))
        .inspect_err(|_| dropped(part))?;
    let next = next.and_then(|bytes| String::from_utf8(bytes).ok()).and_then(|text| text.trim().parse::<u64>().ok());
    let landed = nix::sys::stat::fstatat(&put, upload_id, nix::fcntl::AtFlags::AT_SYMLINK_NOFOLLOW).is_ok();
    let wanted = match (next, landed) {
        // Nothing landed and nothing recorded: this upload starts here.
        (None, false) => 0,
        (Some(wanted), true) => wanted,
        // Bytes with no record beside them, or a record with no bytes: neither is an upload this can go on with.
        _ => {
            dropped(part);
            return Err(words::update_out_of_order(seq, 0, upload_id));
        }
    };
    if seq != wanted {
        dropped(part);
        return Err(words::update_out_of_order(seq, wanted, upload_id));
    }
    let mut file = open_in(&put, part, nix::fcntl::OFlag::O_APPEND).inspect_err(|_| dropped(part))?;
    std::io::Write::write_all(&mut file, bytes).map_err(|e| format!("{}: {e}", part.display()))?;
    let mut next = open_in(&put, &update_next(part), nix::fcntl::OFlag::O_TRUNC)?;
    std::io::Write::write_all(&mut next, format!("{}", seq + 1).as_bytes()).map_err(|e| format!("{}: {e}", update_next(part).display()))
}

/// The file this daemon was execed from, which is the file its unit starts. Linux answers /proc/self/exe with the
/// path and " (deleted)" after it once the inode the process runs has been unlinked, which is what a rename over
/// that path leaves behind: an update that landed and whose daemon has not restarted yet reads its own path that
/// way, and writing to it would make a second file nothing starts.
pub(crate) fn running_daemon(said: std::io::Result<PathBuf>) -> Result<PathBuf, String> {
    let exe = said.map_err(|e| format!("this daemon cannot say which file it runs from: {e}"))?;
    let text = exe.to_string_lossy();
    Ok(match text.strip_suffix(" (deleted)") {
        Some(path) => PathBuf::from(path),
        None => exe,
    })
}

/// The landed binary moved over the one this process runs from, and where the one it replaced was kept. The bytes
/// are checked against the host's sha256 first and the whole of the move is a rename beside the binary, which is
/// the one write a running executable takes: writing into it is refused while it is mapped, and a copy that failed
/// halfway would leave the unit starting a truncated file forever under Restart=always.
pub(crate) fn install_daemon(exe: &Path, home: &Path, upload_id: &str, sha256: &str) -> Result<(String, String), String> {
    let put = put_folder(home)?;
    let part = &update_part(home, upload_id);
    let landed = beneath::read_capped_in(&put, upload_id, UPDATE_MAX_BYTES)
        .map_err(|e| format!("{}: {e}", place_daemon_paths(home).put_dir.display()))?
        .ok_or_else(|| format!("{}: nothing landed", part.display()))?;
    let held = sha256_hex(&landed);
    if held != sha256 {
        drop_upload(&put, part);
        return Err(words::update_bytes_differ(upload_id, sha256, &held));
    }
    let folder = binary_folder(home, exe)?;
    let (fresh, kept) = (exe.with_extension("new"), exe.with_extension("old"));
    let name = |at: &Path| {
        at.file_name().and_then(std::ffi::OsStr::to_str).map(str::to_owned).ok_or_else(|| format!("{} names no file", at.display()))
    };
    let mut running = beneath::file(&folder, &name(exe)?)
        .map_err(|e| format!("{}: {e}", exe.parent().unwrap_or(exe).display()))?
        .ok_or_else(|| format!("{}: nothing there", exe.display()))?;
    let mut new = open_in(&folder, &fresh, nix::fcntl::OFlag::O_TRUNC)?;
    std::io::Write::write_all(&mut new, &landed).map_err(|e| format!("{}: {e}", fresh.display()))?;
    new.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o755)).map_err(|e| format!("{}: {e}", fresh.display()))?;
    // The old binary kept beside the new one, so a box whose daemon will not come up still holds the one it ran.
    let mut old = open_in(&folder, &kept, nix::fcntl::OFlag::O_TRUNC)?;
    std::io::copy(&mut running, &mut old).map_err(|e| format!("{}: {e}", kept.display()))?;
    old.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o755)).map_err(|e| format!("{}: {e}", kept.display()))?;
    nix::fcntl::renameat(&folder, name(&fresh)?.as_str(), &folder, name(exe)?.as_str()).map_err(|e| format!("{}: {e}", exe.display()))?;
    drop_upload(&put, part);
    Ok((exe.to_string_lossy().into_owned(), kept.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests;
