// SPDX-License-Identifier: AGPL-3.0-only
//! A slate's image, read for the host by its whole path: answered with bytes only where it is a regular file of at most
//! the cap whose own bytes name one of the image types the protocol allows, so no other file leaves the computer
//! through it. Opened without blocking and judged on the handle it was opened as, so a fifo or a device in its place
//! can neither hold the read nor feed it. A path on this computer may pass links, since any path here is what is asked
//! for; a path in a workspace is read under its root with no link followed, since an agent arranges every name there.
//! A frame's page takes the same road, answered only where the file is named .html or .htm and its bytes are text.

use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use base64::Engine;
use nix::fcntl::{open, AtFlags};
use nix::sys::stat::{fstatat, Mode};
use wsp_frames::{numbers, DaemonErrorCode, FsImageReply};

use crate::beneath;
use crate::fs::blocking;
use crate::paths::OpError;
use crate::Ctx;

/// The four types by the bytes they start with; WebP's word sits eight bytes in, after the RIFF length.
pub(crate) fn media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

fn not_a_file(shown: &str) -> OpError {
    OpError::coded(DaemonErrorCode::NotAFile, format!("{shown} is not a regular file"))
}

fn not_found(shown: &str) -> OpError {
    OpError::coded(DaemonErrorCode::NotFound, format!("{shown} does not exist"))
}

/// Whether the head of something that is not an image reads as an SVG document, so the host can say so by name.
pub(crate) fn looks_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_lowercase();
    head.match_indices("<svg").any(|(at, _)| head[at + 4..].starts_with(|c: char| c == '>' || c.is_whitespace()))
}

/// What a slate reads off a file: an image its bytes name, or a page that is text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Image,
    Page,
}

impl Kind {
    /// The most a read of this kind carries; past it only the size is answered, which the host refuses.
    fn cap(self) -> u64 {
        match self {
            Kind::Image => numbers::FS_IMAGE_CAP_BYTES,
            Kind::Page => numbers::FS_PAGE_CAP_BYTES,
        }
    }

    fn type_of(self, bytes: &[u8]) -> Option<&'static str> {
        match self {
            Kind::Image => media_type(bytes),
            Kind::Page => std::str::from_utf8(bytes).is_ok().then_some("text/html"),
        }
    }
}

/// The answer for an opened file, with its modified time: its size alone where it is over the cap or not of its
/// kind, whether an SVG stands where an image was asked for, else its bytes.
fn judged(file: File, shown: &str, cap: u64, kind: Kind) -> Result<FsImageReply, OpError> {
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(not_a_file(shown));
    }
    let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64);
    let (inode, changed) = (Some(meta.ino()), u64::try_from(meta.ctime()).ok().map(|s| s * 1_000_000_000 + meta.ctime_nsec() as u64));
    let only =
        |size: u64, svg: bool| FsImageReply { size, modified, inode, changed, media_type: None, content: None, svg: svg.then_some(true) };
    if meta.len() > cap {
        return Ok(only(meta.len(), false));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(cap + 1).read_to_end(&mut bytes)?;
    let size = bytes.len() as u64;
    if size > cap {
        return Ok(only(size, false));
    }
    let Some(found) = kind.type_of(&bytes) else {
        return Ok(only(size, kind == Kind::Image && looks_svg(&bytes)));
    };
    Ok(FsImageReply {
        size,
        modified,
        inode,
        changed,
        media_type: Some(found.to_owned()),
        content: Some(base64::engine::general_purpose::STANDARD.encode(&bytes)),
        svg: None,
    })
}

/// A whole path on this computer.
pub(crate) async fn read_here(path: String, cap: u64, kind: Kind) -> Result<FsImageReply, OpError> {
    blocking(move || {
        let opened = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC).open(&path);
        match opened {
            Ok(file) => judged(file, &path, cap, kind),
            Err(e) if matches!(e.raw_os_error(), Some(libc::ENOENT) | Some(libc::ENOTDIR)) => Err(not_found(&path)),
            // A socket refuses an open outright; anything standing there that is not a file is said as such.
            Err(e) => match std::fs::metadata(&path) {
                Ok(meta) if !meta.is_file() => Err(not_a_file(&path)),
                _ => Err(e.into()),
            },
        }
    })
    .await
}

/// A whole path inside a workspace whose root is `root` on this computer, each name opened with no link followed.
/// Workspaces run on Linux alone; its test runs everywhere.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) async fn read_beneath(root: PathBuf, path: String, cap: u64, kind: Kind) -> Result<FsImageReply, OpError> {
    blocking(move || {
        let held: OwnedFd = open(&root, beneath::dir_flags(), Mode::empty()).map_err(|e| OpError::plain(e.to_string()))?;
        let rel = path.trim_start_matches('/');
        if rel.is_empty() {
            return Err(not_a_file(&path));
        }
        match beneath::file(&held, rel) {
            Ok(Some(file)) => judged(file, &path, cap, kind),
            Ok(None) => match beneath::parent_of(&held, rel) {
                Ok(Some((parent, leaf))) if fstatat(&parent, leaf, AtFlags::AT_SYMLINK_NOFOLLOW).is_ok() => Err(not_a_file(&path)),
                _ => Err(not_found(&path)),
            },
            Err(why) => Err(OpError::coded(DaemonErrorCode::NotAFile, format!("{path}: {why}"))),
        }
    })
    .await
}

/// The image a frame names: a whole path on this computer, or inside the workspace it names.
pub(crate) async fn image_of(ctx: &Ctx, machine: Option<&str>, path: String) -> Result<FsImageReply, OpError> {
    file_of(ctx, machine, path, Kind::Image).await
}

/// A frame's page, by the same road as an image, refused unless it is named as an HTML file.
pub(crate) async fn page_of(ctx: &Ctx, machine: Option<&str>, path: String) -> Result<FsImageReply, OpError> {
    if !is_page_name(&path) {
        return Err(OpError::coded(DaemonErrorCode::BadRequest, format!("{path} is not an .html file")));
    }
    file_of(ctx, machine, path, Kind::Page).await
}

fn is_page_name(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm")
}

async fn file_of(ctx: &Ctx, machine: Option<&str>, path: String, kind: Kind) -> Result<FsImageReply, OpError> {
    if !Path::new(&path).is_absolute() {
        return Err(OpError::coded(DaemonErrorCode::BadRequest, format!("{path} is not a whole path")));
    }
    match machine {
        None => read_here(path, kind.cap(), kind).await,
        Some(machine) => inside(ctx, machine, path, kind).await,
    }
}

#[cfg(target_os = "linux")]
async fn inside(ctx: &Ctx, machine: &str, path: String, kind: Kind) -> Result<FsImageReply, OpError> {
    let root = crate::workspace::workspaces_of(ctx, machine)?.rootfs_of_running(machine).map_err(crate::ops::from_runtime)?;
    read_beneath(root, path, kind.cap(), kind).await
}

#[cfg(not(target_os = "linux"))]
async fn inside(_ctx: &Ctx, machine: &str, _path: String, _kind: Kind) -> Result<FsImageReply, OpError> {
    Err(crate::workspace::no_such_workspace(machine))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0];

    #[tokio::test]
    async fn a_workspace_image_is_read_under_its_root_with_no_link_followed() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("root/shots")).unwrap();
        std::fs::write(root.path().join("root/shots/home.png"), PNG).unwrap();
        std::fs::write(outside.path().join("key.png"), PNG).unwrap();
        std::os::unix::fs::symlink(outside.path().join("key.png"), root.path().join("root/shots/link.png")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("root/away")).unwrap();
        let read = |path: &str| read_beneath(root.path().to_path_buf(), path.to_owned(), 1024, Kind::Image);

        let shown = read("/root/shots/home.png").await.unwrap();
        assert_eq!((shown.media_type.as_deref(), shown.size), (Some("image/png"), PNG.len() as u64));
        assert_eq!(read("/root/shots/link.png").await.unwrap_err().code, Some(DaemonErrorCode::NotAFile));
        // A link standing for a folder on the way: Linux says ELOOP and macOS ENOTDIR, and neither reads past it.
        assert!(read("/root/away/key.png").await.is_err());
        assert_eq!(read("/root/shots").await.unwrap_err().code, Some(DaemonErrorCode::NotAFile));
        assert_eq!(read("/root/shots/gone.png").await.unwrap_err().code, Some(DaemonErrorCode::NotFound));
        assert_eq!(read("/root/../etc/passwd").await.unwrap_err().code, Some(DaemonErrorCode::NotAFile));
    }

    #[tokio::test]
    async fn a_page_is_answered_only_as_text_under_an_html_name() {
        let dir = tempfile::tempdir().unwrap();
        let at = |name: &str| dir.path().join(name).to_string_lossy().into_owned();
        std::fs::write(at("dial.html"), "<!doctype html><p>dial</p>").unwrap();
        std::fs::write(at("bytes.html"), [0xff, 0xfe, 0x00]).unwrap();
        let page = read_here(at("dial.html"), 1024, Kind::Page).await.unwrap();
        assert_eq!(page.media_type.as_deref(), Some("text/html"));
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(page.content.unwrap()).unwrap(), b"<!doctype html><p>dial</p>");
        let bytes = read_here(at("bytes.html"), 1024, Kind::Page).await.unwrap();
        assert_eq!((bytes.media_type, bytes.content, bytes.size), (None, None, 3));
        let over = read_here(at("dial.html"), 8, Kind::Page).await.unwrap();
        assert_eq!((over.content, over.size), (None, 26));
        assert!(is_page_name("/tmp/a.HTML") && is_page_name("/tmp/b.htm"));
        assert!(!is_page_name("/root/.ssh/id_ed25519") && !is_page_name("/tmp/a.html.txt"));
    }

    #[test]
    fn the_four_types_are_read_off_their_bytes() {
        assert_eq!(media_type(PNG), Some("image/png"));
        assert_eq!(media_type(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(media_type(b"GIF89a"), Some("image/gif"));
        assert_eq!(media_type(b"RIFF\x1a\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(media_type(b"RIFF\x1a\0\0\0WAVEfmt "), None);
        assert_eq!(media_type(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"), None);
        assert!(looks_svg(b"<?xml version=\"1.0\"?>\n<SVG xmlns=\"http://www.w3.org/2000/svg\">"));
        assert!(!looks_svg(b"<svgish>not one</svgish>"));
    }
}
