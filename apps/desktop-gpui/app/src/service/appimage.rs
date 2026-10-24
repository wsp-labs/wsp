// SPDX-License-Identifier: AGPL-3.0-only
//! The host's files moved out of an AppImage's mount, which goes when the launch that made it ends, into a copy under
//! the wsp home that the wsp command and the service run after it (`desktop:shim.ts` `keepAppImage`). The copies live
//! in a folder of this app's own, since today's app clears every copy but its newest two out of `<home>/app`.

use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest as _, Sha256};
use wsp_host_client::Env;

use super::Bundled;

/// Where the copies sit under the wsp home.
pub const KEPT_DIR: &str = "gpui-app";
/// What a copy holds, under the AppImage's own folder: the Node and the host, not the app's own binary.
const KEPT: [&str; 2] = ["usr/bin/node", "usr/lib/wsp"];

/// The AppImage this launch runs from: the folder its runtime mounted for this launch alone, and the image file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub appdir: PathBuf,
    pub file: PathBuf,
}

impl Image {
    /// The image the AppImage runtime names in this launch's environment, where it ran from one.
    pub fn of(env: &Env) -> Option<Image> {
        let named = |name: &str| env.get(name).filter(|path| !path.is_empty()).map(PathBuf::from);
        Some(Image { appdir: named("APPDIR")?, file: named("APPIMAGE")? })
    }
}

/// `bundle` as it stands in a copy of the image's host files under `<home>/gpui-app`: one copy per image file, named
/// by the release and the file's size and time so a rebuilt image of one release is copied again, made whole beside
/// its place and renamed in. The copy made before it stays, since the service's host may still run it until its next
/// start; any older one goes.
pub fn keep(bundle: &Bundled, image: &Image, home: &Path, release: &str) -> io::Result<Bundled> {
    let meta = std::fs::metadata(&image.file)?;
    let made = meta.modified()?.duration_since(UNIX_EPOCH).map(|at| at.as_millis()).unwrap_or(0);
    let digest = Sha256::digest(format!("{}:{made}", meta.len()));
    let name = format!("{release}-{}", digest[..4].iter().map(|byte| format!("{byte:02x}")).collect::<String>());
    let root = home.join(KEPT_DIR);
    let kept = root.join(&name);
    if !kept.exists() {
        let part = root.join(format!("{name}.{}.part", std::process::id()));
        let _ = std::fs::remove_dir_all(&part);
        for rel in KEPT {
            let to = part.join(rel);
            std::fs::create_dir_all(to.parent().unwrap_or(&part))?;
            copy_tree(&image.appdir.join(rel), &to)?;
        }
        std::fs::rename(&part, &kept)?;
    }
    let mut others = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if entry.file_name() != name.as_str() {
            let made = entry.metadata().and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH);
            others.push((made, entry.path()));
        }
    }
    others.sort_by_key(|(made, _)| std::cmp::Reverse(*made));
    let before = others.iter().find(|(_, path)| !path.to_string_lossy().ends_with(".part")).map(|(_, path)| path.clone());
    for (_, path) in others {
        if Some(&path) != before.as_ref() {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
    bundle
        .moved(&image.appdir, &kept)
        .ok_or_else(|| io::Error::other(format!("the host is not inside the AppImage at {}", image.appdir.display())))
}

/// Copies a file, a link or a folder and everything under it, keeping modes and links as they are.
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    let meta = std::fs::symlink_metadata(from)?;
    if meta.file_type().is_symlink() {
        #[cfg(unix)]
        std::os::unix::fs::symlink(std::fs::read_link(from)?, to)?;
        return Ok(());
    }
    if meta.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    std::fs::copy(from, to).map(|_| ())
}
