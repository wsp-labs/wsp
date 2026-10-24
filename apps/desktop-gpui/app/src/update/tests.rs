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
use super::stage::{self, Installed, Kind, Shape, ZipEntry};
use super::swap::{self, Settled};
use super::*;

fn hex_sum(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A release feed on loopback: each path answers its bytes, and every path asked is kept in order.
struct Served {
    base: String,
    asked: Arc<Mutex<Vec<String>>>,
}

fn serve(routes: HashMap<String, Vec<u8>>) -> Served {
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
fn release(version: &str, assets: &[(&str, &[u8])], published: &HashMap<&str, String>) -> HashMap<String, Vec<u8>> {
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

fn no_tools(_: &str, _: &[&str]) -> (i32, String) {
    (1, "no tool in this test".into())
}

fn shell(feed: Feed, installed: Option<Installed>, downloads: &Path) -> Shell {
    Shell::new(ShellDeps {
        feed,
        downloads: downloads.to_owned(),
        running: "0.3.8".into(),
        installed,
        road: Some(Road::Linux),
        run: Arc::new(no_tools),
        // SAFETY: getuid reads this process's user and cannot fail.
        uid: unsafe { libc::getuid() },
        pid: std::process::id(),
        log: downloads.join("update.log"),
    })
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_default()
}

fn stages(installed: &Path) -> Vec<PathBuf> {
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
    assert_eq!(mac.asset("0.3.9"), "wsp-0.3.9-mac.zip", "the Mac's universal app comes in the release's zip");
    assert_eq!(linux.asset("0.3.9"), "wsp-0.3.9.AppImage");
    let names = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/protocol/src/bundles.mjs")).unwrap();
    let ours = feed::bundle_names("${version}");
    for name in [ours.mac, ours.mac_zip, ours.app_image] {
        assert!(names.contains(&format!("`{name}`")), "bundles.mjs names {name}");
    }

    let dir = tmp();
    let installed_at = dir.path().join("wsp.AppImage");
    std::fs::write(&installed_at, app_image("0.3.8")).unwrap();
    let new = app_image("0.3.9");
    let served = serve(release(
        "0.3.9",
        &[("wsp-0.3.9-mac.dmg", b"dmg"), ("wsp-0.3.9-mac.zip", b"zip"), ("wsp-0.3.9.AppImage", &new)],
        &HashMap::new(),
    ));
    let mut road = shell(
        Feed::new(Some(served.base.clone()), "wsp/0.3.8".into()),
        Some(Installed { path: installed_at.clone(), kind: Kind::AppImage }),
        dir.path(),
    );
    road.get("0.3.9").unwrap();
    assert_eq!(
        served.asked.lock().unwrap().as_slice(),
        ["/repos/wsp-labs/wsp/releases/tags/v0.3.9", "/wsp-labs/wsp/releases/download/v0.3.9/wsp-0.3.9.AppImage"],
        "the release read, then this computer's asset alone"
    );
    let staged = stages(&installed_at);
    assert_eq!(staged.len(), 1);
    assert_eq!(read(&staged[0].join("new/wsp.AppImage")), new, "the checked copy waits beside the app under its name");
    assert_eq!(read(&installed_at), app_image("0.3.8"), "nothing is replaced before the restart");
    assert_eq!(road.get("0.3.8").unwrap_err().error, "0.3.8 is not a release above this app");
    assert_eq!(road.get("0.4.0-rc.1").unwrap_err().error, "0.4.0-rc.1 is not a release above this app", "never a prerelease");
}

#[test]
fn a_bad_checksum_is_refused_with_nothing_replaced() {
    let dir = tmp();
    let installed_at = dir.path().join("wsp.AppImage");
    std::fs::write(&installed_at, app_image("0.3.8")).unwrap();
    let new = app_image("0.3.9");
    let wrong = HashMap::from([("wsp-0.3.9.AppImage", hex_sum(b"something else"))]);
    let served = serve(release("0.3.9", &[("wsp-0.3.9.AppImage", &new)], &wrong));
    let mut road = shell(
        Feed::new(Some(served.base), "wsp/0.3.8".into()),
        Some(Installed { path: installed_at.clone(), kind: Kind::AppImage }),
        dir.path(),
    );
    let said = road.get("0.3.9").unwrap_err();
    assert_eq!(said.error, "wsp-0.3.9.AppImage did not match the release's sha256 and was deleted");
    assert_eq!(read(&installed_at), app_image("0.3.8"), "the installed app is untouched");
    let root = stage::stage_of(&installed_at);
    let left: Vec<_> = std::fs::read_dir(&root).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    assert!(left.is_empty(), "no download, part or stage is left: {left:?}");
    assert_eq!(road.open().unwrap_err().error, "no verified download to open", "nothing checked, nothing to restart into");

    let not_an_app = serve(release("0.3.9", &[("wsp-0.3.9.AppImage", b"a page, not an app")], &HashMap::new()));
    let mut road = shell(
        Feed::new(Some(not_an_app.base), "wsp/0.3.8".into()),
        Some(Installed { path: installed_at.clone(), kind: Kind::AppImage }),
        dir.path(),
    );
    assert_eq!(road.get("0.3.9").unwrap_err().error, "wsp-0.3.9.AppImage is not an AppImage and was deleted");
    assert!(stages(&installed_at).is_empty());
    assert_eq!(read(&installed_at), app_image("0.3.8"));
}

/// A zip of stored entries, enough for the central directory read; a name ending in / is a folder.
fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
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
/// version and executable the app was built with, xattr clears.
fn mac_tools(signed: i32) -> impl Fn(&str, &[&str]) -> (i32, String) + Send + Sync {
    mac_app(signed, true)
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
    let zip = zip_of(&[("wsp.app/", b""), ("wsp.app/Contents/", b""), ("wsp.app/Contents/Info.plist", b"<plist/>")]);
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
struct Swapping {
    _dir: tempfile::TempDir,
    installed: PathBuf,
    swap: swap::Swap,
}

fn swapping(pid: u32, new_then: &str) -> Swapping {
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
        staged: stage::Staged { app: staged_app, stage: stage_dir, exe: String::new() },
        version: "0.3.9".into(),
        sum: hex_sum(new.as_bytes()),
        log: root.join("update.log"),
        patience: 4,
        grace: 1,
    };
    Swapping { _dir: dir, installed, swap }
}

/// The script run as the app starts it, in a session of its own, but waited on here.
fn run_swap(s: &swap::Swap) -> std::process::Child {
    Command::new("/bin/sh")
        .arg("-c")
        .arg(swap::SCRIPT)
        .arg("wsp-update")
        .args(swap::args(s))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .spawn()
        .unwrap()
}

fn until(what: &str, wait: Duration, done: impl Fn() -> bool) {
    let from = Instant::now();
    while !done() {
        assert!(from.elapsed() < wait, "{what} within {wait:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A process standing in for the running app, gone at once, as an app that quit under the swap.
fn quit_app() -> u32 {
    let mut gone = Command::new("/bin/true").spawn().unwrap();
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
    assert_eq!(swap::settle(&s.installed), Settled::Back { restart: false }, "the old app's launch sweeps the stage");
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
    let followed = after_swap(Some(settled), "0.3.9", || {
        assert!(root.join("restarted").exists(), "said before the host moves, so a failure moves it back");
        *seen.lock().unwrap() = Some(read(&s.installed));
        Ok(())
    });
    assert_eq!(followed, Ok(()));
    assert_eq!(seen.into_inner().unwrap().as_deref(), Some(new.as_slice()), "the host restarts onto the new app's files");
    assert!(script.wait().unwrap().success(), "{}", std::fs::read_to_string(&s.swap.log).unwrap());
    assert_eq!(read(&s.installed), new, "the new app stands in place");
    assert!(!root.exists(), "the old app is deleted once the new one is up");

    let failing = swapping(quit_app(), "exec sleep 3");
    let old = read(&failing.installed);
    let mut script = run_swap(&failing.swap);
    let root = stage::stage_of(&failing.installed);
    until("the swap opened the new app", Duration::from_secs(10), || root.join("opening").exists());
    let said = after_swap(Some(swap::settle(&failing.installed)), "0.3.9", || Err("the host did not come back".into()));
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
    assert_eq!(after_swap(Some(Settled::Swept), "0.3.9", || panic!("nothing to follow")), Ok(()));
    assert_eq!(after_swap(None, "0.3.9", || panic!("nothing to follow")), Ok(()));
    assert_eq!(words::keep_running(2), "Your 2 running threads carry on. Open terminals close.");
    assert_eq!(words::keep_running(1), "Your running thread carries on. Open terminals close.");
}
