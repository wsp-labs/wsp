// SPDX-License-Identifier: AGPL-3.0-only
//! An installed app replacing itself (`self-update.ts`). The checked download is put in a private folder beside the
//! installed app, so the swap is a rename on one volume, and checked before anything is said ready. A Mac's zip has
//! its entries read before anything is unpacked, is unpacked with ditto, and its app must be a real folder whose
//! signature verifies (against the running app's own requirement where that is a Developer ID's), at the version
//! asked for, naming an executable inside it, with no quarantine flag. Either must be this app: the GPUI binary with
//! the host's Node, command and daemon beside it, read off the zip's directory or the AppImage's squashfs before
//! anything is unpacked or started, and on a Mac the running app's bundle id. An AppImage's version is proven when it
//! starts, since it cannot be read before.

use std::ffi::CString;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

/// `UPDATE_WORDS` in `self-update.ts`.
pub mod words {
    pub const TRANSLOCATED: &str =
        "macOS runs the app from a temporary copy, so it cannot replace itself. Move wsp to Applications and open it again.";
    pub const DISK_IMAGE: &str = "The app runs from a disk image or another drive, so it cannot replace itself. Move wsp to Applications.";
    pub const NOT_WRITABLE: &str = "The folder holding the app is not writable, so it cannot replace itself.";
    pub const APP_IMAGE_GONE: &str = "The AppImage the app runs from is gone or is not a file of its own, so it cannot replace itself.";
    pub fn unreadable(asset: &str) -> String {
        format!("{asset} is not a zip that could be read")
    }
    pub fn escapes(asset: &str, entry: &str) -> String {
        format!("{asset} holds {entry}, which reaches outside the app, and was refused")
    }
    pub fn not_one_app(asset: &str) -> String {
        format!("{asset} does not hold exactly one app and was refused")
    }
    pub fn stage_not_ours(root: &str) -> String {
        format!("{root} is not a folder of this account's own, so nothing was staged there")
    }
    pub fn unpacked(asset: &str, why: &str) -> String {
        format!("{asset} could not be unpacked: {why}")
    }
    pub fn unsigned(asset: &str) -> String {
        format!("{asset} failed its signature check and was deleted")
    }
    pub fn wrong_version(asset: &str, want: &str, got: &str) -> String {
        format!("{asset} holds wsp {}, not {want}, and was deleted", if got.is_empty() { "of no version" } else { got })
    }
    pub fn no_executable(asset: &str) -> String {
        format!("{asset} names no executable inside its app and was deleted")
    }
    pub fn quarantined(asset: &str) -> String {
        format!("{asset} kept its quarantine flag and was deleted")
    }
    pub fn no_host(asset: &str) -> String {
        format!("{asset} holds an app that carries no wsp host and was deleted")
    }
    pub fn not_app_image(asset: &str) -> String {
        format!("{asset} is not an AppImage and was deleted")
    }
    pub fn not_this_app(asset: &str) -> String {
        format!("{asset} holds another app than this one and was deleted")
    }
}

/// The hidden folder beside the installed app holding the download, each checked copy in a private folder of its own,
/// and the old app during a swap; not today's app's `.wsp-update`, since both apps can stand in one Applications.
pub const STAGE_DIR: &str = ".wsp-gpui-update";

/// The app a swap that never heard `up` replaced, kept in the stage until it is put back or an update is up.
pub const PREVIOUS: &str = "previous";

/// The GPUI app's executable inside its bundle or AppImage (`bin` in `scripts/package.sh`).
pub const EXE: &str = "desktop-gpui";

pub fn stage_of(installed: &Path) -> PathBuf {
    installed.parent().unwrap_or(Path::new("/")).join(STAGE_DIR)
}

/// What an installed app is: a Mac's .app folder, which the release's zip replaces, or a Linux AppImage file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Mac,
    AppImage,
}

/// The installed app this process runs from and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub path: PathBuf,
    pub kind: Kind,
}

impl Installed {
    /// The app the running executable sits in: the .app around `exe` on a Mac (`bundleOf`), the AppImage the runtime
    /// names in `appimage` on Linux where `exe` runs from the folder it mounted at `appdir`; nothing for a binary
    /// outside either.
    pub fn of(exe: &Path, appimage: Option<&std::ffi::OsStr>, appdir: Option<&std::ffi::OsStr>) -> Option<Installed> {
        if cfg!(target_os = "macos") {
            return bundle_of(exe).map(|path| Installed { path, kind: Kind::Mac });
        }
        let image = appimage.filter(|a| !a.is_empty()).map(PathBuf::from)?;
        let mounted = appdir.filter(|a| !a.is_empty()).is_some_and(|dir| exe.starts_with(dir));
        (image.is_absolute() && mounted).then_some(Installed { path: image, kind: Kind::AppImage })
    }

    /// The release's asset this app replaces itself from.
    pub fn asset(&self, version: &str) -> String {
        let names = super::feed::bundle_names(version);
        match self.kind {
            Kind::Mac => names.mac_zip,
            Kind::AppImage => names.app_image,
        }
    }
}

/// The .app the executable sits in, or nothing for a binary outside a bundle.
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let named = |p: &Path, n: &str| p.file_name().is_some_and(|f| f == n);
    (named(macos, "MacOS") && named(contents, "Contents") && bundle.extension().is_some_and(|e| e == "app")).then(|| bundle.to_owned())
}

fn writable(path: &Path) -> bool {
    let Ok(c) = CString::new(path.as_os_str().as_encoded_bytes()) else { return false };
    // SAFETY: access reads a NUL-terminated path and changes nothing.
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

/// Why the installed app cannot replace itself, or nothing where it can (`inPlaceRefusal`): a translocated copy, and
/// anything under /Volumes, where a disk image may be mounted writable, are refused, and so is a folder that is not
/// writable, since the swap renames the app inside its own folder. An AppImage must still be an AppImage file of its
/// own, since the swap renames whatever stands at its path.
pub fn in_place_refusal(installed: &Installed) -> Option<&'static str> {
    if installed.kind == Kind::AppImage && !(real_file(&installed.path) && app_image_marked(&installed.path)) {
        return Some(words::APP_IMAGE_GONE);
    }
    let path = installed.path.to_string_lossy();
    if installed.kind == Kind::Mac {
        if path.contains("/AppTranslocation/") {
            return Some(words::TRANSLOCATED);
        }
        if path.starts_with("/Volumes/") {
            return Some(words::DISK_IMAGE);
        }
    }
    let parent = installed.path.parent().unwrap_or(Path::new("/"));
    // Moving a folder to another parent rewrites its own .. entry, so a Mac's bundle has to be writable too.
    if !writable(parent) || (installed.kind == Kind::Mac && !writable(&installed.path)) {
        return Some(words::NOT_WRITABLE);
    }
    None
}

/// One command's answer: its exit code and what it printed, both streams.
pub type Runner = dyn Fn(&str, &[&str]) -> (i32, String) + Send + Sync;

/// The system's tools, named by their paths, on the system's own PATH and nothing of this launch's environment but
/// whose login it is.
pub fn command_runner(file: &str, args: &[&str]) -> (i32, String) {
    let mut cmd = std::process::Command::new(file);
    cmd.args(args).env_clear().env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    for name in ["HOME", "USER", "LOGNAME", "TMPDIR", "LANG"] {
        if let Some(value) = std::env::var_os(name) {
            cmd.env(name, value);
        }
    }
    match cmd.output() {
        Ok(out) => {
            (out.status.code().unwrap_or(1), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
        }
        Err(e) => (1, e.to_string()),
    }
}

/// One entry of a zip as its central directory names it, with the target where it is a symlink.
#[derive(Debug, Clone, PartialEq)]
pub struct ZipEntry {
    pub name: String,
    pub link: Option<String>,
}

/// Past this a symlink's stored target is no target.
const LINK_MAX_BYTES: u64 = 4096;

fn u16_at(b: &[u8], at: usize) -> Result<u16, String> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| "the zip ends early".to_owned())
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| "the zip ends early".to_owned())
}

fn u64_at(b: &[u8], at: usize) -> Result<u64, String> {
    b.get(at..at + 8).map(|s| u64::from_le_bytes(s.try_into().expect("eight bytes"))).ok_or_else(|| "the zip ends early".to_owned())
}

/// Every entry of the zip, read off its central directory and, for a symlink, its stored target; refuses anything
/// that is not a zip it can read.
pub fn zip_entries(zip: &Path) -> Result<Vec<ZipEntry>, String> {
    let mut file = fs::File::open(zip).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let mut read = |at: u64, length: u64| -> Result<Vec<u8>, String> {
        let mut buf = vec![0u8; length as usize];
        file.seek(SeekFrom::Start(at)).map_err(|e| e.to_string())?;
        file.read_exact(&mut buf).map_err(|_| "the zip ends early".to_owned())?;
        Ok(buf)
    };
    let tail_len = size.min(22 + 0xffff);
    let tail = read(size - tail_len, tail_len)?;
    let end = tail.windows(4).rposition(|w| w == [0x50, 0x4b, 0x05, 0x06]).ok_or("no end record")?;
    if end + 22 > tail.len() {
        return Err("no end record".into());
    }
    let mut count = u64::from(u16_at(&tail, end + 10)?);
    let mut cd_size = u64::from(u32_at(&tail, end + 12)?);
    let mut cd_offset = u64::from(u32_at(&tail, end + 16)?);
    if count == 0xffff || cd_size == 0xffff_ffff || cd_offset == 0xffff_ffff {
        let locator = end.checked_sub(20).ok_or("no zip64 locator")?;
        if u32_at(&tail, locator)? != 0x0706_4b50 {
            return Err("no zip64 locator".into());
        }
        let z64 = read(u64_at(&tail, locator + 8)?, 56)?;
        if u32_at(&z64, 0)? != 0x0606_4b50 {
            return Err("no zip64 end record".into());
        }
        count = u64_at(&z64, 32)?;
        cd_size = u64_at(&z64, 40)?;
        cd_offset = u64_at(&z64, 48)?;
    }
    if cd_offset.checked_add(cd_size).is_none_or(|end| end > size) {
        return Err("a broken central directory".into());
    }
    let cd = read(cd_offset, cd_size)?;
    let mut entries = Vec::new();
    let mut at = 0usize;
    for _ in 0..count {
        if u32_at(&cd, at)? != 0x0201_4b50 {
            return Err("a broken central directory".into());
        }
        let method = u16_at(&cd, at + 10)?;
        let mut compressed = u64::from(u32_at(&cd, at + 20)?);
        let uncompressed = u32_at(&cd, at + 24)?;
        let name_len = usize::from(u16_at(&cd, at + 28)?);
        let extra_len = usize::from(u16_at(&cd, at + 30)?);
        let comment_len = usize::from(u16_at(&cd, at + 32)?);
        let mode = u32_at(&cd, at + 38)? >> 16;
        let mut local = u64::from(u32_at(&cd, at + 42)?);
        let name = String::from_utf8_lossy(cd.get(at + 46..at + 46 + name_len).ok_or("the zip ends early")?).into_owned();
        // A field too big for its slot moves into the zip64 extra, in this order, only where its slot is full.
        let mut x = at + 46 + name_len;
        while x + 4 <= at + 46 + name_len + extra_len {
            let id = u16_at(&cd, x)?;
            let length = usize::from(u16_at(&cd, x + 2)?);
            if id == 0x0001 {
                let mut field = x + 4;
                if uncompressed == 0xffff_ffff {
                    field += 8;
                }
                if compressed == 0xffff_ffff {
                    compressed = u64_at(&cd, field)?;
                    field += 8;
                }
                if local == 0xffff_ffff {
                    local = u64_at(&cd, field)?;
                }
            }
            x += 4 + length;
        }
        at += 46 + name_len + extra_len + comment_len;
        // ditto honours the mode bits whatever platform the entry says made it, so they are read the same way here.
        if mode & 0o170000 != 0o120000 {
            entries.push(ZipEntry { name, link: None });
            continue;
        }
        if compressed > LINK_MAX_BYTES {
            return Err("a symlink with no plausible target".into());
        }
        let header = read(local, 30)?;
        if u32_at(&header, 0)? != 0x0403_4b50 {
            return Err("a broken local header".into());
        }
        let data = read(local + 30 + u64::from(u16_at(&header, 26)?) + u64::from(u16_at(&header, 28)?), compressed)?;
        let target = match method {
            0 => data,
            8 => {
                let mut out = Vec::new();
                flate2::read::DeflateDecoder::new(&data[..]).take(LINK_MAX_BYTES).read_to_end(&mut out).map_err(|e| e.to_string())?;
                out
            }
            _ => return Err("a symlink stored in a way it cannot read".into()),
        };
        entries.push(ZipEntry { name, link: Some(String::from_utf8_lossy(&target).into_owned()) });
    }
    Ok(entries)
}

/// What an archive holds (`archiveShape`).
#[derive(Debug, PartialEq)]
pub enum Shape {
    Top(String),
    Escapes(String),
    NotOneApp,
}

/// `..` and `.` taken out of a relative path by its spelling alone (`posix.normalize`), or nothing where it climbs out.
fn normal(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for c in Path::new(path).components() {
        match c {
            Component::Normal(p) => parts.push(p.to_str()?),
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

/// The one app an archive holds, or what is wrong with it: an entry that would land outside it (an absolute path,
/// whose first step is empty, a .. step, a path through a symlink, a symlink whose target leaves the app), or
/// anything but exactly one app at its top.
pub fn archive_shape(entries: &[ZipEntry]) -> Shape {
    let trim = |name: &str| name.trim_end_matches('/').to_owned();
    // A Mac's volume matches names in any case and either Unicode form, so a path reaches a link however it spells it.
    let fold = |name: &str| icu_normalizer::ComposingNormalizerBorrowed::new_nfc().normalize(name).to_lowercase();
    let links: std::collections::HashSet<String> = entries.iter().filter(|e| e.link.is_some()).map(|e| fold(&trim(&e.name))).collect();
    let mut tops = std::collections::BTreeSet::new();
    for entry in entries {
        let name = trim(&entry.name);
        let parts: Vec<&str> = name.split('/').collect();
        if name.contains(['\\', '\0']) || parts.iter().any(|p| p.is_empty() || *p == "." || *p == "..") {
            return Shape::Escapes(entry.name.clone());
        }
        if (1..parts.len()).any(|i| links.contains(&fold(&parts[..i].join("/")))) {
            return Shape::Escapes(entry.name.clone());
        }
        let top = parts[0].to_owned();
        tops.insert(top.clone());
        let Some(link) = &entry.link else { continue };
        if parts.len() == 1 || link.starts_with('/') {
            return Shape::Escapes(entry.name.clone());
        }
        let dir = parts[..parts.len() - 1].join("/");
        let lands = normal(&format!("{dir}/{link}"));
        if !lands.is_some_and(|l| l == top || l.starts_with(&format!("{top}/"))) {
            return Shape::Escapes(entry.name.clone());
        }
    }
    match (tops.len(), tops.into_iter().next()) {
        (1, Some(top)) if top.ends_with(".app") => Shape::Top(top),
        _ => Shape::NotOneApp,
    }
}

/// A checked copy: the new app, the private folder it sits in, its executable's name inside a Mac's bundle, and the
/// signing requirement it passed, empty where the running app is signed ad hoc.
#[derive(Debug, Clone, PartialEq)]
pub struct Staged {
    pub app: PathBuf,
    pub stage: PathBuf,
    pub exe: String,
    pub requirement: String,
}

/// The files that make an app this one, relative to its bundle or AppImage root: the GPUI binary, and the Node,
/// command and daemon the restart reads beside it.
fn ours(os: crate::service::Os) -> Vec<PathBuf> {
    let exe = match os {
        crate::service::Os::Mac => Path::new("Contents/MacOS").join(EXE),
        crate::service::Os::Linux => Path::new("usr/bin").join(EXE),
    };
    let host = crate::service::Bundled::beside(&exe, os, std::env::consts::ARCH);
    vec![exe, host.node, host.command, host.daemon]
}

/// The designated requirement of the running app's signature, or nothing where it has none to hold another copy to:
/// an ad hoc signature names only its own hash.
pub fn requirement_of(app: &Path, run: &Runner) -> Option<String> {
    let (code, said) = run("/usr/bin/codesign", &["-d", "-r-", &app.to_string_lossy()]);
    let line = said.lines().find_map(|l| l.trim().trim_start_matches("# ").strip_prefix("designated => "))?.trim();
    (code == 0 && !line.is_empty() && !line.starts_with("cdhash ")).then(|| line.to_owned())
}

/// `codesign`'s check of a copy, held to `requirement` where there is one.
pub fn verify_args(app: &str, requirement: &str) -> Vec<String> {
    let mut args = vec!["--verify".to_owned(), "--deep".into(), "--strict".into()];
    if !requirement.is_empty() {
        args.push(format!("-R={requirement}"));
    }
    args.push(app.to_owned());
    args
}

fn real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

fn real_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}

fn first_line(text: &str) -> &str {
    text.trim().lines().next().unwrap_or_default()
}

/// A private folder of its own under `root`.
fn private_dir(root: &Path, prefix: &str) -> Result<PathBuf, String> {
    let template = CString::new(root.join(format!("{prefix}XXXXXX")).into_os_string().into_encoded_bytes()).map_err(|e| e.to_string())?;
    let raw = template.into_raw();
    // SAFETY: mkdtemp rewrites the X's of the NUL-terminated buffer it is handed, which into_raw gave up.
    let made = unsafe { libc::mkdtemp(raw) };
    // SAFETY: the buffer came from into_raw above and is taken back once.
    let path = unsafe { CString::from_raw(raw) };
    if made.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(path.as_bytes())))
}

/// The stage beside the installed app, made where it is not and refused where it is not this account's own: another
/// account that can write the app's folder could have made it first and read a stage's name out of it.
fn own_root(installed: &Path, uid: u32) -> Result<PathBuf, String> {
    let root = stage_of(installed);
    match fs::DirBuilder::new().mode(0o700).create(&root) {
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => return Err(e.to_string()),
        _ => {}
    }
    let held = fs::symlink_metadata(&root).map_err(|e| e.to_string())?;
    if held.file_type().is_symlink() || !held.is_dir() || held.uid() != uid {
        return Err(words::stage_not_ours(&root.to_string_lossy()));
    }
    if held.mode() & 0o077 != 0 {
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    Ok(root)
}

/// The download checked and put in a private folder beside the installed app (`stageUpdate`); the download goes
/// either way, and a copy that fails a check goes with it.
pub fn stage(download: &Path, version: &str, installed: &Installed, run: &Runner, uid: u32) -> Result<Staged, String> {
    let asset = download.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let mut made: Option<PathBuf> = None;
    let checked = (|| {
        let root = own_root(&installed.path, uid)?;
        match installed.kind {
            Kind::Mac => stage_mac(download, &asset, version, &installed.path, &root, &mut made, run),
            Kind::AppImage => stage_app_image(download, &asset, &installed.path, &root, &mut made),
        }
    })();
    _ = fs::remove_file(download);
    if checked.is_err() {
        if let Some(stage) = made {
            _ = fs::remove_dir_all(stage);
        }
    }
    checked
}

fn stage_mac(
    zip: &Path,
    asset: &str,
    version: &str,
    running: &Path,
    root: &Path,
    made: &mut Option<PathBuf>,
    run: &Runner,
) -> Result<Staged, String> {
    let Ok(entries) = zip_entries(zip) else { return Err(words::unreadable(asset)) };
    let top = match archive_shape(&entries) {
        Shape::Escapes(entry) => return Err(words::escapes(asset, &entry)),
        Shape::NotOneApp => return Err(words::not_one_app(asset)),
        Shape::Top(top) => top,
    };
    let names: std::collections::HashSet<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    if !ours(crate::service::Os::Mac).iter().all(|f| names.contains(Path::new(&top).join(f).to_string_lossy().as_ref())) {
        return Err(words::not_this_app(asset));
    }
    let stage = private_dir(root, "stage-")?;
    *made = Some(stage.clone());
    let into = stage.join("new");
    fs::DirBuilder::new().mode(0o700).create(&into).map_err(|e| e.to_string())?;
    let path = |p: &Path| p.to_string_lossy().into_owned();
    // By path: a codesign that answers 0 earlier on the login PATH would pass any copy.
    let (code, said) = run("/usr/bin/ditto", &["-x", "-k", &path(zip), &path(&into)]);
    if code != 0 {
        return Err(words::unpacked(asset, first_line(&said)));
    }
    let app = into.join(&top);
    let names: Vec<_> = fs::read_dir(&into).map_err(|e| e.to_string())?.filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    if names.len() != 1 || names[0] != *top || !real_dir(&app) {
        return Err(words::not_one_app(asset));
    }
    let requirement = requirement_of(running, run).unwrap_or_default();
    if requirement.is_empty() {
        crate::desktop::log::info("update: this app is signed ad hoc, so the new app's signature is checked with no requirement");
    }
    let verify = verify_args(&path(&app), &requirement);
    if run("/usr/bin/codesign", &verify.iter().map(String::as_str).collect::<Vec<_>>()).0 != 0 {
        return Err(words::unsigned(asset));
    }
    let key_of = |plist: &Path, name: &str| match run("/usr/bin/plutil", &["-extract", name, "raw", "-o", "-", &path(plist)]) {
        (0, out) => out.trim().to_owned(),
        _ => String::new(),
    };
    let key = |name: &str| key_of(&app.join("Contents/Info.plist"), name);
    let id = key("CFBundleIdentifier");
    if id.is_empty() || id != key_of(&running.join("Contents/Info.plist"), "CFBundleIdentifier") {
        return Err(words::not_this_app(asset));
    }
    let got = key("CFBundleShortVersionString");
    if got != version {
        return Err(words::wrong_version(asset, version, &got));
    }
    let exe = key("CFBundleExecutable");
    if exe.is_empty() || exe.contains(['/', '\0']) || exe == "." || exe == ".." || !real_file(&app.join("Contents/MacOS").join(&exe)) {
        return Err(words::no_executable(asset));
    }
    // Another app under the same name, today's included, would be swapped in for this one and take its host with it.
    let host = crate::service::Bundled::beside(&app.join("Contents/MacOS").join(&exe), crate::service::Os::Mac, std::env::consts::ARCH);
    if ![&host.node, &host.command, &host.daemon].iter().all(|f| real_file(f)) {
        return Err(words::no_host(asset));
    }
    if run("/usr/bin/xattr", &["-dr", "com.apple.quarantine", &path(&app)]).0 != 0 {
        return Err(words::quarantined(asset));
    }
    Ok(Staged { app, stage, exe, requirement })
}

/// An ELF executable carrying the type 2 AppImage mark at byte 8.
fn app_image_marked(file: &Path) -> bool {
    let mut head = [0u8; 11];
    fs::File::open(file).and_then(|mut f| f.read_exact(&mut head)).is_ok() && head[..4] == *b"\x7fELF" && head[8..11] == *b"AI\x02"
}

fn stage_app_image(download: &Path, asset: &str, installed: &Path, root: &Path, made: &mut Option<PathBuf>) -> Result<Staged, String> {
    if !real_file(download) || !app_image_marked(download) {
        return Err(words::not_app_image(asset));
    }
    let mut image = super::image::Image::open(download).map_err(|_| words::not_this_app(asset))?;
    for file in ours(crate::service::Os::Linux) {
        match image.entry(&file) {
            Ok(Some(entry)) if entry.file && (entry.mode & 0o111 != 0 || file.extension().is_some_and(|e| e == "mjs")) => {}
            _ => return Err(words::not_this_app(asset)),
        }
    }
    let stage = private_dir(root, "stage-")?;
    *made = Some(stage.clone());
    let into = stage.join("new");
    fs::DirBuilder::new().mode(0o700).create(&into).map_err(|e| e.to_string())?;
    let app = into.join(installed.file_name().unwrap_or_default());
    fs::rename(download, &app).map_err(|_| words::unpacked(asset, "it could not be moved beside the app"))?;
    fs::set_permissions(&app, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    Ok(Staged { app, stage, exe: String::new(), requirement: String::new() })
}

/// Deletes whatever is staged beside the installed app but the app a swap that never finished replaced: the person
/// dismissed the update, or a newer release replaces it.
pub fn discard(installed: &Path) {
    let root = stage_of(installed);
    let Ok(held) = fs::read_dir(&root) else { return };
    let mut kept = false;
    for entry in held.filter_map(|e| e.ok()) {
        if entry.file_name() == PREVIOUS {
            kept = true;
        } else if entry.file_type().is_ok_and(|t| t.is_dir()) {
            _ = fs::remove_dir_all(entry.path());
        } else {
            _ = fs::remove_file(entry.path());
        }
    }
    if !kept {
        _ = fs::remove_dir(&root);
    }
}

/// The kept app put in a private stage of its own under its installed name, as a checked copy is, for a swap that
/// puts it back.
pub fn restage(installed: &Installed, run: &Runner, uid: u32) -> Result<Staged, String> {
    let root = own_root(&installed.path, uid)?;
    let previous = root.join(PREVIOUS);
    let stage = private_dir(&root, "stage-")?;
    let into = stage.join("new");
    fs::DirBuilder::new().mode(0o700).create(&into).map_err(|e| e.to_string())?;
    let app = into.join(installed.path.file_name().unwrap_or_default());
    fs::rename(&previous, &app).map_err(|e| e.to_string())?;
    let (exe, requirement) = match installed.kind {
        Kind::AppImage => (String::new(), String::new()),
        Kind::Mac => {
            let info = app.join("Contents/Info.plist");
            let exe = match run("/usr/bin/plutil", &["-extract", "CFBundleExecutable", "raw", "-o", "-", &info.to_string_lossy()]) {
                (0, out) => out.trim().to_owned(),
                _ => String::new(),
            };
            (exe, requirement_of(&installed.path, run).unwrap_or_default())
        }
    };
    Ok(Staged { app, stage, exe, requirement })
}
