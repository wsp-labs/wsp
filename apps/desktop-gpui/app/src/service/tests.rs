// SPDX-License-Identifier: AGPL-3.0-only
use std::path::{Path, PathBuf};
use std::process::Command;

use super::appimage::KEPT_DIR;
use super::*;

/// A bundle of `os` with the three files in place, under `root`.
fn bundle_in(root: &Path, os: Os) -> Bundled {
    let exe = match os {
        Os::Mac => root.join("wsp Experimental.app/Contents/MacOS/desktop-gpui"),
        Os::Linux => root.join("usr/bin/desktop-gpui"),
    };
    let bundle = Bundled::beside(&exe, os, "x86_64");
    for file in [&exe, &bundle.node, &bundle.command, &bundle.daemon] {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, file.to_string_lossy().as_bytes()).unwrap();
    }
    bundle
}

/// Every file under `dir` with its bytes, to say nothing in it changed.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    let mut todo = vec![dir.to_owned()];
    while let Some(at) = todo.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                todo.push(path);
            } else {
                files.push((path.clone(), std::fs::read(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}

/// A home today's app has set up: its state, its host's files, its AppImage copy, and a unit it wrote.
fn todays_home(home: &Path) {
    for (rel, text) in [
        ("state.json", "{\"workspaces\":{}}\n"),
        ("host.log", "listening on 127.0.0.1:4400\n"),
        ("host-token", "token\n"),
        ("app/0.3.8-1a2b3c4d/wsp", "electron\n"),
        ("app/0.3.8-1a2b3c4d/resources/cli.mjs", "cli\n"),
    ] {
        let path = home.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

#[test]
fn the_host_sits_where_todays_app_keeps_its_own_under_each_bundles_resources() {
    let mac = Bundled::beside(Path::new("/Applications/wsp Experimental.app/Contents/MacOS/desktop-gpui"), Os::Mac, "aarch64");
    assert_eq!(mac.node, Path::new("/Applications/wsp Experimental.app/Contents/MacOS/node"));
    assert_eq!(mac.command, Path::new("/Applications/wsp Experimental.app/Contents/Resources/app/main/cli.mjs"));
    assert_eq!(
        mac.daemon,
        Path::new("/Applications/wsp Experimental.app/Contents/Resources/app/assets/daemon/aarch64-apple-darwin/wsp-daemon")
    );
    let linux = Bundled::beside(Path::new("/tmp/.mount_wspX/usr/bin/desktop-gpui"), Os::Linux, "x86_64");
    assert_eq!(linux.node, Path::new("/tmp/.mount_wspX/usr/bin/node"));
    assert_eq!(linux.command, Path::new("/tmp/.mount_wspX/usr/lib/wsp/app/main/cli.mjs"));
    assert_eq!(linux.daemon, Path::new("/tmp/.mount_wspX/usr/lib/wsp/app/assets/daemon/x86_64-unknown-linux-musl/wsp-daemon"));
}

/// The lines `package.sh --dry-run` prints for a target, run on nothing of this process's environment.
fn package_dry_run(target: &str) -> String {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/package.sh");
    let home = tempfile::tempdir().unwrap();
    let out = Command::new("/bin/bash")
        .arg(&script)
        .args(["--dry-run", "--target", target])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home.path())
        .output()
        .unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{said}");
    said
}

/// Catches: a bundle that packs the Node, the command or the daemon anywhere but where the app looks for them, which
/// a launch reads as a build carrying no host, so it never writes the wsp command.
#[test]
fn the_package_checks_the_node_the_command_and_the_daemon_at_the_paths_the_app_reads() {
    let mac = package_dry_run("mac");
    for arch in ["aarch64", "x86_64"] {
        let here = Bundled::beside(Path::new("dist/wsp Experimental.app/Contents/MacOS/desktop-gpui"), Os::Mac, arch);
        for (test, file) in [("-x", &here.node), ("-f", &here.command), ("-x", &here.daemon)] {
            let line = format!("+ test {test} '{}'", file.display());
            assert!(mac.lines().any(|said| said == line), "no line {line} in:\n{mac}");
        }
    }
    let linux = package_dry_run("linux");
    let appdir = linux
        .lines()
        .filter_map(|said| said.strip_prefix("+ install -m 755 target/release/desktop-gpui "))
        .find(|to| to.ends_with("/AppDir/usr/bin/desktop-gpui"))
        .unwrap();
    let here = Bundled::beside(Path::new(appdir), Os::Linux, "x86_64");
    for (test, file) in [("-x", &here.node), ("-f", &here.command), ("-x", &here.daemon)] {
        let line = format!("+ test {test} {}", file.display());
        assert!(linux.lines().any(|said| said == line), "no line {line} in:\n{linux}");
    }
}

/// Catches: a shim that runs anything but the bundled command on the bundled Node, or one quoted so a home with a
/// space in its path breaks the line.
#[test]
fn the_wsp_command_runs_the_bundled_command_on_the_bundled_node_behind_the_daemons_forwarder() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Apps and things");
    let bundle = bundle_in(&root, Os::Mac);
    // A daemon that says what it was handed, one word a line, as the forwarder's `forward` reads its words.
    std::fs::write(&bundle.daemon, "#!/bin/sh\nfor word in \"$@\"; do echo \"$word\"; done\n").unwrap();
    std::fs::set_permissions(&bundle.daemon, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let home = dir.path().join("home/.wsp");
    let owned = claim(&home, Ok(bundle.clone()), None, "0.1.0").unwrap().unwrap();
    assert_eq!(owned, Owned { wsp: home.join("bin/wsp"), rewrote: true });
    let ran = Command::new(&owned.wsp).args(["up", "--service"]).env_clear().env("PATH", "/usr/bin:/bin").output().unwrap();
    let words: Vec<String> = String::from_utf8_lossy(&ran.stdout).lines().map(str::to_owned).collect();
    let (node, command) = (bundle.node.to_string_lossy().into_owned(), bundle.command.to_string_lossy().into_owned());
    assert_eq!(words, ["forward", "--wsp-argv", &node, "--wsp-argv", &command, "--", "up", "--service"]);
    assert!(!std::fs::read_to_string(&owned.wsp).unwrap().contains("ELECTRON_RUN_AS_NODE"));
}

/// Catches: a launch that rewrites the command when it already names this bundle (restarting the host for nothing),
/// or that leaves it naming a bundle that moved or was swapped by an update.
#[test]
fn a_launch_keeps_a_command_naming_this_bundle_and_rewrites_one_naming_another() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    let first = bundle_in(&dir.path().join("Downloads"), Os::Mac);
    assert!(claim(&home, Ok(first.clone()), None, "0.1.0").unwrap().unwrap().rewrote);
    assert!(!claim(&home, Ok(first.clone()), None, "0.1.0").unwrap().unwrap().rewrote);
    let moved = bundle_in(&dir.path().join("Applications"), Os::Mac);
    let owned = claim(&home, Ok(moved.clone()), None, "0.1.0").unwrap().unwrap();
    assert!(owned.rewrote);
    assert_eq!(std::fs::read_to_string(&owned.wsp).unwrap(), shim::text(&moved));
}

/// Catches: this app writing over the wsp command today's app wrote, which would move that app's service onto this
/// one (the move over is its own step), or touching anything else of today's install.
#[test]
fn todays_command_naming_an_app_that_stands_is_left_with_everything_else_of_its_install() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    todays_home(&home);
    let electron = dir.path().join("Applications/wsp.app/Contents/MacOS/wsp");
    std::fs::create_dir_all(electron.parent().unwrap()).unwrap();
    std::fs::write(&electron, "electron").unwrap();
    let todays = format!(
        "#!/bin/sh\n# The wsp app writes this on each launch; it runs the wsp the app bundles.\nELECTRON_RUN_AS_NODE=1 exec '{}' --no-memory-pool '/Applications/wsp.app/Contents/Resources/app/main/cli.mjs' \"$@\"\n",
        electron.display()
    );
    std::fs::create_dir_all(home.join("bin")).unwrap();
    std::fs::write(home.join("bin/wsp"), &todays).unwrap();
    let before = snapshot(&home);
    let bundle = bundle_in(&dir.path().join("gpui"), Os::Mac);
    assert_eq!(claim(&home, Ok(bundle.clone()), None, "0.1.0"), Ok(None));
    assert_eq!(snapshot(&home), before);

    // Today's app gone, its command names nothing that runs, and this app's is written in its place.
    std::fs::remove_file(&electron).unwrap();
    let owned = claim(&home, Ok(bundle.clone()), None, "0.1.0").unwrap().unwrap();
    assert_eq!(std::fs::read_to_string(owned.wsp).unwrap(), shim::text(&bundle));
    let after: Vec<_> = snapshot(&home).into_iter().filter(|(path, _)| !path.ends_with("bin/wsp")).collect();
    let kept: Vec<_> = before.into_iter().filter(|(path, _)| !path.ends_with("bin/wsp")).collect();
    assert_eq!(after, kept, "nothing of today's state, logs or copies changed");
}

#[test]
fn a_build_with_no_host_writes_nothing_and_says_which_file_it_lacks() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    let refused = claim(&home, Err("this build carries no host of its own: /x/node is not there".to_owned()), None, "0.1.0");
    assert_eq!(refused, Err("this build carries no host of its own: /x/node is not there".to_owned()));
    assert!(!home.exists());
    let bundle = Bundled::beside(&dir.path().join("target/release/desktop-gpui"), Os::Linux, "x86_64");
    assert_eq!(bundle.missing(), Some(bundle.node.as_path()));
}

/// Catches: a command or unit naming the AppImage's mount, which goes when the launch ends, or copies that pile up,
/// or today's app's own copies under `<home>/app` cleared by this app.
#[test]
fn under_an_appimage_the_command_names_a_copy_in_the_home_and_only_this_apps_older_copies_go() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    todays_home(&home);
    let todays = snapshot(&home.join("app"));
    let mount = dir.path().join("mount");
    let bundle = bundle_in(&mount, Os::Linux);
    std::fs::write(mount.join("usr/lib/wsp/app/main/chunk-A.mjs"), "chunk").unwrap();
    let file = dir.path().join("wsp-experimental-0.1.0-x86_64.AppImage");
    let image = Image { appdir: mount.clone(), file: file.clone() };
    let mut copies = Vec::new();
    for size in 1..=3 {
        std::fs::write(&file, "x".repeat(size)).unwrap();
        let owned = claim(&home, Ok(bundle.clone()), Some(&image), "0.1.0").unwrap().unwrap();
        let text = std::fs::read_to_string(&owned.wsp).unwrap();
        assert!(!text.contains(&*mount.to_string_lossy()), "{text}");
        let copy = std::fs::read_dir(home.join(KEPT_DIR))
            .unwrap()
            .map(|e| e.unwrap().path())
            .max_by_key(|p| p.metadata().unwrap().modified().unwrap())
            .unwrap();
        let kept = Bundled::beside(&copy.join("usr/bin/desktop-gpui"), Os::Linux, "x86_64");
        assert_eq!(text, shim::text(&kept));
        assert_eq!(kept.missing(), None);
        assert!(copy.join("usr/lib/wsp/app/main/chunk-A.mjs").is_file(), "the command's chunks travel with it");
        assert!(!copy.join("usr/bin/desktop-gpui").exists(), "the app's own binary stays in the image");
        copies.push(copy);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!copies[0].exists(), "the oldest copy goes");
    assert!(copies[1].exists() && copies[2].exists(), "the copy before stays for a host still running it");
    assert_eq!(snapshot(&home.join("app")), todays, "today's app's copies are its own");
}

#[test]
fn resolves_the_home_as_path_resolve_does() {
    let cwd = Path::new("/work/here");
    assert_eq!(resolved(Path::new("homes/a"), cwd), PathBuf::from("/work/here/homes/a"));
    assert_eq!(resolved(Path::new("../b/./c"), cwd), PathBuf::from("/work/b/c"));
    assert_eq!(resolved(Path::new("//srv//wsp-home"), cwd), PathBuf::from("/srv/wsp-home"));
    assert_eq!(resolved(Path::new("/srv/wsp-home/"), cwd), PathBuf::from("/srv/wsp-home"));
    assert_eq!(resolved(Path::new("/../../x"), cwd), PathBuf::from("/x"));
}
