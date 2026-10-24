// SPDX-License-Identifier: AGPL-3.0-only
//! The update road on a feed served here, an installed app in a temp folder and a fake signature check: nothing
//! reaches GitHub, no service is touched and no real app is replaced.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;
use sha2::{Digest, Sha256};

use super::feed::{self, Feed};
use super::follow::{follow, older_host, replace_prompt, Follow};
use super::stage::{self, Installed, Kind, Runner, Shape, ZipEntry};
use super::swap::{self, Settled};
use super::*;

pub(super) fn hex_sum(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A release feed on loopback: each path answers its bytes, and every path asked is kept in order.
pub(super) struct Served {
    pub(super) base: String,
    pub(super) asked: Arc<Mutex<Vec<String>>>,
}

pub(super) fn serve(routes: HashMap<String, Vec<u8>>) -> Served {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let kept = asked.clone();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut conn) = conn else { continue };
            let mut reader = BufReader::new(conn.try_clone().unwrap());
            let mut first = String::new();
            if reader.read_line(&mut first).is_err() {
                continue;
            }
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).map_or(true, |n| n == 0) || line == "\r\n" {
                    break;
                }
            }
            let path = first.split_whitespace().nth(1).unwrap_or_default().to_owned();
            kept.lock().unwrap().push(path.clone());
            let (status, body) = match routes.get(&path) {
                Some(body) => ("200 OK", body.clone()),
                None => ("404 Not Found", b"{}".to_vec()),
            };
            let head = format!("HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
            _ = conn.write_all(head.as_bytes()).and_then(|_| conn.write_all(&body));
        }
    });
    Served { base, asked }
}

/// One release's answer on the feed with these assets, each published with `published` sums (the real ones where
/// nothing overrides them), and each asset's bytes at its download path.
pub(super) fn release(version: &str, assets: &[(&str, &[u8])], published: &HashMap<&str, String>) -> HashMap<String, Vec<u8>> {
    let rows: Vec<_> = assets
        .iter()
        .map(|(name, bytes)| {
            let sum = published.get(name).cloned().unwrap_or_else(|| hex_sum(bytes));
            json!({ "name": name, "size": bytes.len(), "digest": format!("sha256:{sum}") })
        })
        .collect();
    let mut routes = HashMap::new();
    let answer = json!({ "tag_name": format!("v{version}"), "published_at": "2026-10-24T00:00:00Z", "assets": rows });
    routes.insert(format!("/repos/wsp-labs/wsp/releases/tags/v{version}"), answer.to_string().into_bytes());
    for (name, bytes) in assets {
        routes.insert(format!("/wsp-labs/wsp/releases/download/v{version}/{name}"), bytes.to_vec());
    }
    routes
}

/// An AppImage's first bytes: an ELF header with the type 2 mark at byte 8, then what tells two apart.
fn app_image(said: &str) -> Vec<u8> {
    let mut bytes = b"\x7fELF\x02\x01\x01\x00AI\x02\x00\x00\x00\x00\x00".to_vec();
    bytes.extend_from_slice(said.as_bytes());
    bytes
}

/// A runtime's ELF header standing in for the type 2 runtime: the AppImage mark at byte 8, and no section headers,
/// so the squashfs starts right after its 64 bytes.
fn runtime_head() -> Vec<u8> {
    let mut head = vec![0u8; 64];
    head[..11].copy_from_slice(b"\x7fELF\x02\x01\x01\x00AI\x02");
    head[0x28..0x30].copy_from_slice(&64u64.to_le_bytes());
    head[0x3a..0x3c].copy_from_slice(&64u16.to_le_bytes());
    head
}

/// An AppImage of these files (`path`, bytes, runnable), packed by mksquashfs as `scripts/package.sh` packs one.
pub(super) fn packed(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
    use std::os::unix::fs::PermissionsExt;
    let dir = tmp();
    let root = dir.path().join("AppDir");
    for (path, bytes, runnable) in files {
        let at = root.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, bytes).unwrap();
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(if *runnable { 0o755 } else { 0o644 })).unwrap();
    }
    let image = dir.path().join("app.squashfs");
    let made = Command::new("mksquashfs")
        .args([root.as_os_str(), image.as_os_str()])
        .args(["-all-root", "-noappend", "-no-progress", "-quiet", "-no-xattrs", "-no-fragments"])
        .status()
        .expect("these tests pack AppImages with mksquashfs: apt-get install squashfs-tools, or brew install squashfs");
    assert!(made.success());
    let mut bytes = runtime_head();
    bytes.extend(std::fs::read(image).unwrap());
    bytes
}

/// The GPUI app's AppImage at `version`: its binary, and the host's Node, command and daemon where the restart reads them.
pub(super) fn gpui_image(version: &str) -> Vec<u8> {
    let daemon = format!("usr/lib/wsp/app/assets/daemon/{}-unknown-linux-musl/wsp-daemon", std::env::consts::ARCH);
    packed(&[
        ("AppRun", b"#!/bin/sh\n", true),
        ("usr/bin/desktop-gpui", version.as_bytes(), true),
        ("usr/bin/node", b"node", true),
        ("usr/lib/wsp/app/main/cli.mjs", b"cli", false),
        (&daemon, b"daemon", true),
    ])
}

/// Today's Electron AppImage as electron-builder lays one out, on the same runtime, so it carries the same marks.
pub(super) fn electron_image() -> Vec<u8> {
    packed(&[
        ("AppRun", b"#!/bin/sh\n", true),
        ("wsp", b"electron", true),
        ("resources/app.asar", b"asar", false),
        ("resources/app.asar.unpacked/assets/daemon/x86_64-unknown-linux-musl/wsp-daemon", b"daemon", true),
        ("usr/lib/libffmpeg.so", b"so", false),
    ])
}

fn no_tools(_: &str, _: &[&str]) -> (i32, String) {
    (1, "no tool in this test".into())
}

/// An app at 0.3.8 installed in a temp folder, and what a release holds for it: the asset that updates it to 0.3.9
/// with its bytes, and today's app under today's own name; with the tools its check runs.
pub(super) struct Install {
    pub(super) dir: tempfile::TempDir,
    pub(super) installed: Installed,
    pub(super) asset: String,
    pub(super) new: Vec<u8>,
    pub(super) today: (&'static str, Vec<u8>),
    /// The installed app's bytes as `now` reads them, and the checked copy's once staged.
    pub(super) was: Vec<u8>,
    pub(super) staged: Vec<u8>,
    run: Arc<Runner>,
}

/// The installed apps a test updates: a Mac's bundle, checked with `mac_tools`, everywhere, and an AppImage packed with
/// mksquashfs on Linux, the one road that has one.
pub(super) fn installs() -> Vec<Install> {
    let mut all = vec![install(Kind::Mac)];
    if cfg!(target_os = "linux") {
        all.push(install(Kind::AppImage));
    }
    all
}

pub(super) fn install(kind: Kind) -> Install {
    let dir = tmp();
    let at = dir.path().join(if kind == Kind::Mac { "wsp.app" } else { "wsp.AppImage" });
    let installed = Installed { path: at.clone(), kind };
    let asset = installed.asset("0.3.9");
    let (new, today, was, staged, run): (_, _, _, _, Arc<Runner>) = match kind {
        Kind::Mac => {
            let today =
                zip_of(&[("wsp.app/", b""), ("wsp.app/Contents/MacOS/wsp", b"electron"), ("wsp.app/Contents/Resources/app.asar", b"asar")]);
            (gpui_zip(), ("wsp-0.3.9-mac.zip", today), b"old".to_vec(), b"new".to_vec(), Arc::new(mac_tools(0)))
        }
        Kind::AppImage => {
            let new = gpui_image("0.3.9");
            (new.clone(), ("wsp-0.3.9.AppImage", electron_image()), app_image("0.3.8"), new, Arc::new(no_tools))
        }
    };
    match kind {
        Kind::Mac => {
            std::fs::create_dir_all(at.join("Contents/MacOS")).unwrap();
            std::fs::write(at.join("Contents/MacOS/desktop-gpui"), &was).unwrap();
        }
        Kind::AppImage => std::fs::write(&at, &was).unwrap(),
    }
    Install { dir, installed, asset, new, today, was, staged, run }
}

impl Install {
    /// The app at `at` as its bytes: an AppImage's own, a bundle's executable's.
    pub(super) fn now(&self, at: &Path) -> Vec<u8> {
        match self.installed.kind {
            Kind::Mac => read(&at.join("Contents/MacOS/desktop-gpui")),
            Kind::AppImage => read(at),
        }
    }

    pub(super) fn shell(&self, feed: Feed) -> Shell {
        let downloads = self.dir.path();
        Shell::new(ShellDeps {
            feed,
            state: downloads.join("wsp-home/state.json"),
            downloads: downloads.to_owned(),
            running: "0.3.8".into(),
            installed: Some(self.installed.clone()),
            road: Some(Road::Linux),
            run: self.run.clone(),
            // SAFETY: getuid reads this process's user and cannot fail.
            uid: unsafe { libc::getuid() },
            pid: std::process::id(),
            log: downloads.join("update.log"),
            patience: 1,
        })
    }
}

pub(super) fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

pub(super) fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_default()
}

pub(super) fn stages(installed: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(stage::stage_of(installed))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("stage-")))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_newer_release_is_found_and_the_asset_for_this_computer_picked() {
    let release_view = json!({ "state": "read", "latest": { "version": "0.3.9", "tag": "v0.3.9", "url": "https://github.com/wsp-labs/wsp/releases/tag/v0.3.9", "publishedAt": "x" } });
    let stage =
        |app, host, here, in_place, download: Option<&Download>| update_stage(Some(&release_view), app, host, here, in_place, download);
    let get = stage("0.3.8", Some("0.3.8"), true, Some(true), None).unwrap();
    assert_eq!((get.version.as_str(), get.act), ("0.3.9", Act::Get), "an app one release behind gets it");
    assert_eq!(stage("0.3.9", Some("0.3.9"), true, Some(true), None), None, "level with the release, nothing");
    assert_eq!(
        stage("0.3.9", Some("0.3.8"), true, Some(true), None).unwrap().act,
        Act::Link,
        "a host that lags alone is updated its own way"
    );
    assert_eq!(stage("0.3.8", Some("0.3.8"), false, Some(true), None).unwrap().act, Act::Link, "a page another computer serves links");
    assert_eq!(stage("0.3.8", Some("0.3.8"), true, None, None).unwrap().act, Act::Link, "no road on this platform links");
    let downloading = Download { version: "0.3.9".into(), phase: Phase::Downloading };
    let kept = Download { version: "0.3.9".into(), phase: Phase::Kept };
    assert_eq!(stage("0.3.8", None, true, Some(true), Some(&downloading)).unwrap().act, Act::Downloading);
    assert_eq!(stage("0.3.8", None, true, Some(true), Some(&kept)).unwrap().act, Act::Restart, "a checked copy restarts in");
    assert_eq!(stage("0.3.8", None, true, Some(false), Some(&kept)).unwrap().act, Act::Open, "a kept disk image opens");
    let installed = json!({ "state": "read", "installed": "0.3.9" });
    assert_eq!(update_stage(Some(&installed), "0.3.8", Some("0.3.8"), true, Some(true), None).unwrap().act, Act::RestartHost);

    let mac = Installed { path: "/Applications/wsp.app".into(), kind: Kind::Mac };
    let linux = Installed { path: "/opt/wsp.AppImage".into(), kind: Kind::AppImage };
    assert_eq!(mac.asset("0.3.9"), "wsp-experimental-0.3.9-mac.zip", "the Mac's universal app comes in the release's zip");
    assert_eq!(linux.asset("0.3.9"), "wsp-experimental-0.3.9-x86_64.AppImage");

    for install in installs() {
        let installed_at = &install.installed.path;
        let assets = ["wsp-experimental-0.3.9.dmg", "wsp-experimental-0.3.9-mac.zip", "wsp-experimental-0.3.9-x86_64.AppImage"];
        let bytes: Vec<(&str, &[u8])> =
            assets.iter().map(|a| (*a, if *a == install.asset { install.new.as_slice() } else { b"another" })).collect();
        let served = serve(release("0.3.9", &bytes, &HashMap::new()));
        let mut road = install.shell(Feed::new(Some(served.base.clone()), "wsp/0.3.8".into()));
        road.get("0.3.9").unwrap();
        assert_eq!(
            served.asked.lock().unwrap().as_slice(),
            ["/repos/wsp-labs/wsp/releases/tags/v0.3.9".to_owned(), format!("/wsp-labs/wsp/releases/download/v0.3.9/{}", install.asset)],
            "the release read, then this computer's asset alone"
        );
        let staged = stages(installed_at);
        assert_eq!(staged.len(), 1);
        let name = installed_at.file_name().unwrap();
        assert_eq!(install.now(&staged[0].join("new").join(name)), install.staged, "the checked copy waits beside the app under its name");
        assert_eq!(install.now(installed_at), install.was, "nothing is replaced before the restart");
        assert_eq!(road.get("0.3.8").unwrap_err().error, "0.3.8 is not a release above this app");
        assert_eq!(road.get("0.4.0-rc.1").unwrap_err().error, "0.4.0-rc.1 is not a release above this app", "never a prerelease");
    }
}

#[test]
fn a_bad_checksum_is_refused_with_nothing_replaced() {
    for install in installs() {
        let (installed_at, asset) = (&install.installed.path, install.asset.as_str());
        let wrong = HashMap::from([(asset, hex_sum(b"something else"))]);
        let served = serve(release("0.3.9", &[(asset, &install.new)], &wrong));
        let mut road = install.shell(Feed::new(Some(served.base), "wsp/0.3.8".into()));
        let said = road.get("0.3.9").unwrap_err();
        assert_eq!(said.error, format!("{asset} did not match the release's sha256 and was deleted"));
        assert_eq!(install.now(installed_at), install.was, "the installed app is untouched");
        let root = stage::stage_of(installed_at);
        let left: Vec<_> = std::fs::read_dir(&root).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
        assert!(left.is_empty(), "no download, part or stage is left: {left:?}");
        assert_eq!(road.open().unwrap_err().error, "no verified download to open", "nothing checked, nothing to restart into");

        let not_an_app = serve(release("0.3.9", &[(asset, b"a page, not an app")], &HashMap::new()));
        let mut road = install.shell(Feed::new(Some(not_an_app.base), "wsp/0.3.8".into()));
        let refused = match install.installed.kind {
            Kind::Mac => format!("{asset} is not a zip that could be read"),
            Kind::AppImage => format!("{asset} is not an AppImage and was deleted"),
        };
        assert_eq!(road.get("0.3.9").unwrap_err().error, refused);
        assert!(stages(installed_at).is_empty());
        assert_eq!(install.now(installed_at), install.was);
    }
}

/// A zip of stored entries, enough for the central directory read; a name ending in / is a folder.
pub(super) fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (name, data) in entries {
        let at = out.len() as u32;
        let crc = 0u32;
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 3, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
        let mode: u32 = if name.ends_with('/') { 0o040755 } else { 0o100644 };
        central.extend_from_slice(&(mode << 16).to_le_bytes());
        central.extend_from_slice(&at.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd_at = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// The Mac's tools as a test plays them: ditto unpacks the one app, codesign answers `signed`, plutil reads the
/// version, executable and bundle id the app was built with, xattr clears.
fn mac_tools(signed: i32) -> impl Fn(&str, &[&str]) -> (i32, String) + Send + Sync {
    mac_app(signed, true)
}

/// The zip's entries for a GPUI app named wsp.app: the binary and the host's Node, command and daemon for this chip.
fn gpui_zip() -> Vec<u8> {
    let exe = Path::new("wsp.app/Contents/MacOS/desktop-gpui");
    let host = crate::service::Bundled::beside(exe, crate::service::Os::Mac, std::env::consts::ARCH);
    let names: Vec<String> =
        [exe.to_path_buf(), host.node, host.command, host.daemon].iter().map(|p| p.to_string_lossy().into_owned()).collect();
    let mut entries: Vec<(&str, &[u8])> = vec![("wsp.app/", b""), ("wsp.app/Contents/", b""), ("wsp.app/Contents/Info.plist", b"<plist/>")];
    entries.extend(names.iter().map(|n| (n.as_str(), &b"x"[..])));
    zip_of(&entries)
}

/// `mac_tools`, the unpacked app carrying the host at the paths the restart reads or, with `host` false, none, as an
/// app of another kind under the same name would.
fn mac_app(signed: i32, host: bool) -> impl Fn(&str, &[&str]) -> (i32, String) + Send + Sync {
    move |tool, args| match tool {
        "/usr/bin/ditto" => {
            let exe = Path::new(args[3]).join("wsp.app/Contents/MacOS/desktop-gpui");
            let mut files = vec![exe.clone()];
            if host {
                let carried = crate::service::Bundled::beside(&exe, crate::service::Os::Mac, std::env::consts::ARCH);
                files.extend([carried.node, carried.command, carried.daemon]);
            }
            for file in files {
                std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                std::fs::write(file, "new").unwrap();
            }
            (0, String::new())
        }
        "/usr/bin/codesign" => (signed, String::new()),
        "/usr/bin/plutil" if args[1] == "CFBundleShortVersionString" => (0, "0.3.9\n".into()),
        "/usr/bin/plutil" if args[1] == "CFBundleIdentifier" => (0, "wsp.desktop.experimental\n".into()),
        "/usr/bin/plutil" => (0, "desktop-gpui\n".into()),
        "/usr/bin/xattr" => (0, String::new()),
        _ => (1, format!("{tool} is not a tool here")),
    }
}

#[test]
fn a_bad_signature_is_refused_with_nothing_replaced() {
    let dir = tmp();
    let app = dir.path().join("wsp.app");
    std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
    std::fs::write(app.join("Contents/MacOS/desktop-gpui"), "old").unwrap();
    let installed = Installed { path: app.clone(), kind: Kind::Mac };
    // SAFETY: getuid reads this process's user and cannot fail.
    let uid = unsafe { libc::getuid() };
    let zip = gpui_zip();
    let download = |name: &str| {
        let at = dir.path().join(name);
        std::fs::write(&at, &zip).unwrap();
        at
    };

    let unsigned = download("wsp-0.3.9-mac.zip");
    let said = stage::stage(&unsigned, "0.3.9", &installed, &mac_tools(1), uid).unwrap_err();
    assert_eq!(said, "wsp-0.3.9-mac.zip failed its signature check and was deleted");
    assert!(!unsigned.exists(), "the download goes");
    assert!(stages(&app).is_empty(), "the unpacked copy goes with it");
    assert_eq!(read(&app.join("Contents/MacOS/desktop-gpui")), b"old", "the installed app is untouched");

    let signed = download("wsp-0.3.9-mac.zip");
    let staged = stage::stage(&signed, "0.3.9", &installed, &mac_tools(0), uid).unwrap();
    assert_eq!((staged.exe.as_str(), staged.app.ends_with("new/wsp.app")), ("desktop-gpui", true), "a signed copy is staged");
    assert_eq!(
        stage::stage(&download("wsp-0.3.9-mac.zip"), "0.4.0", &installed, &mac_tools(0), uid).unwrap_err(),
        "wsp-0.3.9-mac.zip holds wsp 0.3.9, not 0.4.0, and was deleted"
    );

    assert_eq!(
        stage::stage(&download("wsp-0.3.9-mac.zip"), "0.3.9", &installed, &mac_app(0, false), uid).unwrap_err(),
        "wsp-0.3.9-mac.zip holds an app that carries no wsp host and was deleted",
        "a signed app of another kind under the release's name is never swapped in"
    );
    assert!(stages(&app).iter().all(|s| s.join("new/wsp.app").exists()), "only the staged copy that passed is left");

    let entry = |name: &str, link: Option<&str>| ZipEntry { name: name.into(), link: link.map(str::to_owned) };
    assert_eq!(stage::archive_shape(&[entry("wsp.app/", None), entry("../evil", None)]), Shape::Escapes("../evil".into()));
    assert_eq!(stage::archive_shape(&[entry("wsp.app/l", Some("/etc")), entry("wsp.app/", None)]), Shape::Escapes("wsp.app/l".into()));
    assert_eq!(
        stage::archive_shape(&[entry("wsp.app/l", Some("x")), entry("wsp.app/L/f", None)]),
        Shape::Escapes("wsp.app/L/f".into()),
        "through a link, however it is spelled"
    );
    assert_eq!(stage::archive_shape(&[entry("wsp.app/", None), entry("other.app/", None)]), Shape::NotOneApp);
    assert_eq!(
        stage::archive_shape(&[entry("wsp.app/caf\u{e9}", Some("x")), entry("wsp.app/cafe\u{301}/f", None)]),
        Shape::Escapes("wsp.app/cafe\u{301}/f".into()),
        "through a link spelled in the other Unicode form"
    );
    let escaping = dir.path().join("wsp-0.3.9-mac.zip");
    std::fs::write(&escaping, zip_of(&[("wsp.app/", b""), ("wsp.app/../../x", b"x")])).unwrap();
    assert_eq!(
        stage::stage(&escaping, "0.3.9", &installed, &mac_tools(0), uid).unwrap_err(),
        "wsp-0.3.9-mac.zip holds wsp.app/../../x, which reaches outside the app, and was refused"
    );
}

/// An installed AppImage in a temp folder, the checked copy staged beside it, and the swap's arguments for an app
/// whose pid is `pid`; each app is a script that marks `<name>.ran` when it runs, then does `then`.
pub(super) struct Swapping {
    pub(super) _dir: tempfile::TempDir,
    pub(super) installed: PathBuf,
    pub(super) swap: swap::Swap,
}

pub(super) fn swapping(pid: u32, new_then: &str) -> Swapping {
    let dir = tmp();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let script = |name: &str, then: &str| format!("#!/bin/sh\n: > '{}'\n{then}\n", root.join(format!("{name}.ran")).display());
    let installed = root.join("wsp.AppImage");
    std::fs::write(&installed, script("old", "exit 0")).unwrap();
    let stage_root = stage::stage_of(&installed);
    let stage_dir = stage_root.join("stage-test");
    std::fs::create_dir_all(stage_dir.join("new")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&stage_root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let staged_app = stage_dir.join("new/wsp.AppImage");
    let new = script("new", new_then);
    std::fs::write(&staged_app, &new).unwrap();
    for f in [&installed, &staged_app] {
        std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let swap = swap::Swap {
        pid,
        installed: installed.clone(),
        kind: Kind::AppImage,
        staged: stage::Staged { app: staged_app, stage: stage_dir, exe: String::new(), requirement: String::new() },
        version: "0.3.9".into(),
        sum: hex_sum(new.as_bytes()),
        log: root.join("update.log"),
        patience: 4,
        grace: 1,
    };
    Swapping { _dir: dir, installed, swap }
}

/// The script run as the app starts it, in a session of its own, but waited on here.
pub(super) fn run_swap(s: &swap::Swap) -> std::process::Child {
    run_swap_on(s, &linux_tools(s.log.parent().unwrap()))
}

/// The PATH an AppImage's swap finds its tools on: the system's own, and on a Mac, which keeps no sha256sum there, one
/// written into `dir` that runs shasum.
fn linux_tools(dir: &Path) -> String {
    if cfg!(target_os = "linux") {
        return "/usr/bin:/bin".into();
    }
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("bin");
    let sum = bin.join("sha256sum");
    if !sum.exists() {
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(&sum, "#!/bin/sh\nexec /usr/bin/shasum -a 256 \"$@\"\n").unwrap();
        std::fs::set_permissions(&sum, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    format!("{}:/usr/bin:/bin", bin.display())
}

/// `run_swap` with `path` as the PATH the swap finds its tools on.
pub(super) fn run_swap_on(s: &swap::Swap, path: &str) -> std::process::Child {
    Command::new("/bin/sh").arg("-c").arg(swap::SCRIPT).arg("wsp-update").args(swap::args(s)).env_clear().env("PATH", path).spawn().unwrap()
}

pub(super) fn until(what: &str, wait: Duration, done: impl Fn() -> bool) {
    let from = Instant::now();
    while !done() {
        assert!(from.elapsed() < wait, "{what} within {wait:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A process standing in for the running app, gone at once, as an app that quit under the swap.
pub(super) fn quit_app() -> u32 {
    let mut gone = Command::new("true").spawn().unwrap();
    let pid = gone.id();
    gone.wait().unwrap();
    pid
}

#[test]
fn a_swap_whose_new_app_fails_to_start_puts_the_old_one_back() {
    let s = swapping(quit_app(), "exit 3");
    let old = read(&s.installed);
    let status = run_swap(&s.swap).wait().unwrap();
    assert!(!status.success(), "the swap says it failed");
    assert_eq!(read(&s.installed), old, "the old app is back in place");
    let root = stage::stage_of(&s.installed);
    assert!(s.installed.with_file_name("new.ran").exists(), "the new app was started");
    assert_eq!(std::fs::read_to_string(root.join("back")).unwrap().trim(), "keep", "the host never moved, so it stays");
    until("the old app opened again", Duration::from_secs(5), || s.installed.with_file_name("old.ran").exists());
    let log = std::fs::read_to_string(&s.swap.log).unwrap();
    assert!(log.contains("the new app exited before it was up; putting the old app back"), "{log}");
    assert_eq!(swap::settle(&s.installed), Settled::Back { restart: false, said: None }, "the old app's launch sweeps the stage");
    assert!(!root.exists());

    let changed = swapping(quit_app(), "exit 0");
    std::fs::write(&changed.swap.staged.app, "changed after the check").unwrap();
    let old = read(&changed.installed);
    assert!(!run_swap(&changed.swap).wait().unwrap().success());
    assert_eq!(read(&changed.installed), old, "a copy that changed after its check is never moved in");
    assert!(!changed.installed.with_file_name("new.ran").exists());
}

#[test]
fn the_host_follows_onto_the_new_bundle_after_a_good_swap() {
    let s = swapping(quit_app(), "exec sleep 3");
    let new = read(&s.swap.staged.app);
    let mut script = run_swap(&s.swap);
    let root = stage::stage_of(&s.installed);
    until("the swap opened the new app", Duration::from_secs(10), || {
        root.join("opening").exists() && s.installed.with_file_name("new.ran").exists()
    });
    let settled = swap::settle(&s.installed);
    assert!(matches!(settled, Settled::Waiting(_)), "the new app's launch finds the swap waiting on it: {settled:?}");
    let seen = Mutex::new(None);
    let followed = after_swap(
        Some(settled),
        || {
            assert!(root.join("restarted").exists(), "said before the host moves, so a failure moves it back");
            *seen.lock().unwrap() = Some(read(&s.installed));
            Ok(crate::service::Followed::Restarted)
        },
        || panic!("nothing to move back"),
    );
    assert_eq!(seen.into_inner().unwrap().as_deref(), Some(new.as_slice()), "the host restarts onto the new app's files");
    followed.unwrap().expect("the swap is handed back to be answered once the app is open").up("0.3.9").unwrap();
    assert!(script.wait().unwrap().success(), "{}", std::fs::read_to_string(&s.swap.log).unwrap());
    assert_eq!(read(&s.installed), new, "the new app stands in place");
    assert!(!root.exists(), "the old app is deleted once the new one is up");

    let failing = swapping(quit_app(), "exec sleep 3");
    let old = read(&failing.installed);
    let mut script = run_swap(&failing.swap);
    let root = stage::stage_of(&failing.installed);
    until("the swap opened the new app", Duration::from_secs(10), || root.join("opening").exists());
    let said =
        after_swap(Some(swap::settle(&failing.installed)), || Err("the host did not come back".into()), || panic!("nothing to move back"));
    assert_eq!(said, Err("the host did not come back".into()));
    assert!(!root.join("up").exists(), "a host that did not follow never answers the swap");
    assert!(!script.wait().unwrap().success());
    assert_eq!(read(&failing.installed), old, "the old app is put back");
    assert_eq!(std::fs::read_to_string(root.join("back")).unwrap().trim(), "restart", "the old app moves the host back");
}

#[test]
fn no_restart_while_a_turn_runs_unless_asked() {
    assert_eq!(follow(Some(1)), Follow::Ask, "a running turn is asked about first");
    assert_eq!(follow(Some(3)), Follow::Ask);
    assert_eq!(follow(None), Follow::Ask, "a host that would not say is asked about too");
    assert_eq!(follow(Some(0)), Follow::Restart, "nothing running, nothing to ask");
    let prompt = replace_prompt("0.3.8", "0.3.9", Some(2));
    assert_eq!(prompt.message, "wsp 0.3.8 is still serving your threads; restart it as 0.3.9?");
    assert_eq!(
        prompt.detail,
        "2 threads are working on this computer. Running turns carry on while wsp restarts. This app opens only on wsp 0.3.9, so Quit leaves wsp 0.3.8 serving and closes the app."
    );
    assert_eq!((prompt.restart.as_str(), prompt.quit), ("Restart as 0.3.9", "Quit"));
    assert!(replace_prompt("0.3.8", "0.3.9", Some(1)).detail.starts_with("A thread is working on this computer. "));
    assert_eq!(older_host("wsp 0.3.8 keeps serving /h/.wsp/state.json: this app opens only on wsp 0.3.9"), Some("0.3.8"));
    assert_eq!(older_host("wsp stopped as it started"), None);
    assert_eq!(after_swap(Some(Settled::Swept), || panic!("nothing to follow"), || panic!("nothing to move back")), Ok(None));
    assert_eq!(after_swap(None, || panic!("nothing to follow"), || panic!("nothing to move back")), Ok(None));
    assert_eq!(words::keep_running(2), "Your 2 running threads carry on. Open terminals close.");
    assert_eq!(words::keep_running(1), "Your running thread carries on. Open terminals close.");
}

#[test]
fn only_the_gpui_app_is_fetched_and_staged() {
    let names = feed::bundle_names("0.3.9");
    assert_eq!(
        [names.mac.as_str(), names.mac_zip.as_str(), names.app_image.as_str()],
        ["wsp-experimental-0.3.9.dmg", "wsp-experimental-0.3.9-mac.zip", "wsp-experimental-0.3.9-x86_64.AppImage"],
        "the names GPUI_NAMES in bundles.mjs gives the GPUI app's downloads"
    );
    let package = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/package.sh")).unwrap();
    for built in [
        "slug=wsp-experimental-$version",
        r#"dmg="$dist/$slug.dmg""#,
        r#"zip="$dist/$slug-mac.zip""#,
        r#"appimage="$dist/$slug-x86_64.AppImage""#,
    ] {
        assert!(package.contains(built), "package.sh builds what the table names: {built}");
    }

    for install in installs() {
        let (installed_at, asset) = (&install.installed.path, install.asset.as_str());
        let (today_name, today) = (install.today.0, install.today.1.as_slice());
        let today_only = serve(release("0.3.9", &[(today_name, today)], &HashMap::new()));
        let mut road = install.shell(Feed::new(Some(today_only.base.clone()), "wsp/0.3.8".into()));
        assert_eq!(road.get("0.3.9").unwrap_err().error, format!("the release carries no {asset} yet, so nothing was downloaded"));
        assert_eq!(
            today_only.asked.lock().unwrap().as_slice(),
            ["/repos/wsp-labs/wsp/releases/tags/v0.3.9"],
            "today's app is never fetched"
        );
        assert!(!stage::stage_of(installed_at).exists(), "and nothing is made beside the app");

        let renamed = serve(release("0.3.9", &[(asset, today)], &HashMap::new()));
        let mut road = install.shell(Feed::new(Some(renamed.base.clone()), "wsp/0.3.8".into()));
        assert_eq!(
            road.get("0.3.9").unwrap_err().error,
            format!("{asset} holds another app than this one and was deleted"),
            "today's app under the GPUI app's name, an AppImage with the same runtime and marks, is refused off its zip's directory or squashfs"
        );
        assert!(stages(installed_at).is_empty(), "nothing is staged");
        assert_eq!(install.now(installed_at), install.was, "nothing is replaced");
        let mut next_launch = install.shell(Feed::new(Some(renamed.base.clone()), "wsp/0.3.8".into()));
        assert_eq!(
            next_launch.get("0.3.9").unwrap_err().error,
            format!("{asset} was refused before as another app than this one, so it was not downloaded again")
        );
        let downloads = renamed.asked.lock().unwrap().iter().filter(|p| p.contains("/download/")).count();
        assert_eq!(downloads, 1, "the refused asset is downloaded once across launches");
    }
    #[cfg(target_os = "linux")]
    the_appimage_is_read_off_its_squashfs();
}

/// A file read off an AppImage's squashfs with its mode, and a build with no daemon told apart from one with it.
#[cfg(target_os = "linux")]
fn the_appimage_is_read_off_its_squashfs() {
    let dir = tmp();
    let no_daemon = packed(&[
        ("usr/bin/desktop-gpui", b"0.3.9", true),
        ("usr/bin/node", b"node", true),
        ("usr/lib/wsp/app/main/cli.mjs", b"cli", false),
    ]);
    let mut image_at = dir.path().join("no-daemon.AppImage");
    std::fs::write(&image_at, &no_daemon).unwrap();
    let ours = |file: &str| image::Image::open(&image_at).unwrap().entry(Path::new(file)).unwrap();
    assert!(ours("usr/bin/node").is_some_and(|e| e.file && e.mode & 0o111 != 0), "a file is read off the image with its mode");
    assert_eq!(ours("usr/lib/wsp/app/assets/daemon"), None, "a build with no daemon is told apart");
    image_at = dir.path().join("gpui.AppImage");
    std::fs::write(&image_at, gpui_image("0.3.9")).unwrap();
    let daemon = format!("usr/lib/wsp/app/assets/daemon/{}-unknown-linux-musl/wsp-daemon", std::env::consts::ARCH);
    assert!(image::Image::open(&image_at).unwrap().entry(Path::new(&daemon)).unwrap().is_some_and(|e| e.file));
}

/// `mac_app`'s tools with a Developer ID signature on the running app, whose requirement only a copy signed by the
/// same team passes; the new copy is signed `new_team`'s way.
fn signed_by(new_team: &'static str) -> impl Fn(&str, &[&str]) -> (i32, String) + Send + Sync {
    let base = mac_app(0, true);
    move |tool, args| {
        match (tool, args.first().copied()) {
        ("/usr/bin/codesign", Some("-d")) => (0, "Executable=/x\ndesignated => identifier \"wsp.desktop.experimental\" and anchor apple generic and certificate leaf[subject.OU] = TEAM1\n".into()),
        ("/usr/bin/codesign", Some("--verify")) => match args.iter().find_map(|a| a.strip_prefix("-R=")) {
            Some(req) if req.ends_with(new_team) => (0, String::new()),
            Some(_) => (3, "test-requirement: code failed to satisfy specified code requirement(s)".into()),
            None => (0, String::new()),
        },
        _ => base(tool, args),
    }
    }
}

#[test]
fn a_mac_update_must_be_this_app_signed_by_its_team() {
    let dir = tmp();
    let app = dir.path().join("wsp.app");
    std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
    std::fs::write(app.join("Contents/MacOS/desktop-gpui"), "old").unwrap();
    let installed = Installed { path: app.clone(), kind: Kind::Mac };
    // SAFETY: getuid reads this process's user and cannot fail.
    let uid = unsafe { libc::getuid() };
    let download = |bytes: &[u8]| {
        let at = dir.path().join("wsp-experimental-0.3.9-mac.zip");
        std::fs::write(&at, bytes).unwrap();
        at
    };

    assert_eq!(
        stage::stage(&download(&gpui_zip()), "0.3.9", &installed, &signed_by("ADHOC"), uid).unwrap_err(),
        "wsp-experimental-0.3.9-mac.zip failed its signature check and was deleted",
        "a copy any other signature passes --strict with is held to the running app's requirement"
    );
    let staged = stage::stage(&download(&gpui_zip()), "0.3.9", &installed, &signed_by("TEAM1"), uid).unwrap();
    assert!(staged.requirement.ends_with("leaf[subject.OU] = TEAM1"), "the swap checks the copy against it again: {}", staged.requirement);
    assert_eq!(
        swap::args(&swap::Swap {
            pid: 1,
            installed: app.clone(),
            kind: Kind::Mac,
            staged: staged.clone(),
            version: "0.3.9".into(),
            sum: String::new(),
            log: dir.path().join("log"),
            patience: 1,
            grace: 1,
        })[12],
        staged.requirement
    );
    let ad_hoc = stage::stage(&download(&gpui_zip()), "0.3.9", &installed, &mac_tools(0), uid).unwrap();
    assert_eq!(ad_hoc.requirement, "", "a running app signed ad hoc has no team to hold the copy to");

    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let tools = move |tool: &str, args: &[&str]| {
        seen.lock().unwrap().push(tool.to_owned());
        mac_tools(0)(tool, args)
    };
    let today = zip_of(&[("wsp.app/", b""), ("wsp.app/Contents/MacOS/wsp", b"electron"), ("wsp.app/Contents/Resources/app.asar", b"asar")]);
    assert_eq!(
        stage::stage(&download(&today), "0.3.9", &installed, &tools, uid).unwrap_err(),
        "wsp-experimental-0.3.9-mac.zip holds another app than this one and was deleted"
    );
    assert!(calls.lock().unwrap().is_empty(), "refused off the zip's directory, before anything is unpacked");
    let other_id = |tool: &str, args: &[&str]| match tool {
        "/usr/bin/plutil" if args[1] == "CFBundleIdentifier" && args[5].contains("/new/") => (0, "wsp.desktop\n".into()),
        _ => mac_tools(0)(tool, args),
    };
    assert_eq!(
        stage::stage(&download(&gpui_zip()), "0.3.9", &installed, &other_id, uid).unwrap_err(),
        "wsp-experimental-0.3.9-mac.zip holds another app than this one and was deleted",
        "another bundle id is another app"
    );
}
