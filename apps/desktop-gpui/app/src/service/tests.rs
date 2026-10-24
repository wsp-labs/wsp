// SPDX-License-Identifier: AGPL-3.0-only
use std::path::{Path, PathBuf};
use std::process::Command;

use wsp_host_client::start::Ran;

use super::appimage::KEPT_DIR;
use super::*;

/// A bundle of `os` of release `release` with the three files in place, under `root`.
fn bundle_in(root: &Path, os: Os, release: &str) -> Bundled {
    let exe = match os {
        Os::Mac => root.join("wsp Experimental.app/Contents/MacOS/desktop-gpui"),
        Os::Linux => root.join("usr/bin/desktop-gpui"),
    };
    let bundle = Bundled::beside(&exe, os, "x86_64");
    for file in [&exe, &bundle.node, &bundle.command, &bundle.daemon] {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, file.to_string_lossy().as_bytes()).unwrap();
    }
    set_release(&bundle, release);
    bundle
}

/// The release the bundle's host says it is, rewritten as an update in place swaps the files under one path.
fn set_release(bundle: &Bundled, release: &str) {
    let pkg = bundle.command.parent().unwrap().parent().unwrap().join("package.json");
    std::fs::write(pkg, format!(r#"{{"name":"@wsp/desktop","version":"{release}"}}"#)).unwrap();
}

/// The file the wsp command is, which a rewrite replaces and a kept command leaves.
fn inode(path: &Path) -> u64 {
    std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(path).unwrap())
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
    let bundle = bundle_in(&root, Os::Mac, "0.3.8");
    // A daemon that says what it was handed, one word a line, as the forwarder's `forward` reads its words.
    std::fs::write(&bundle.daemon, "#!/bin/sh\nfor word in \"$@\"; do echo \"$word\"; done\n").unwrap();
    std::fs::set_permissions(&bundle.daemon, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let home = dir.path().join("home/.wsp");
    let owned = claim(&home, Ok(bundle.clone()), None, None).unwrap().unwrap();
    assert_eq!(owned, Owned { wsp: home.join("bin/wsp"), restart: None });
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
    let first = bundle_in(&dir.path().join("Downloads"), Os::Mac, "0.3.8");
    let owned = claim(&home, Ok(first.clone()), None, None).unwrap().unwrap();
    let written = inode(&owned.wsp);
    assert_eq!(claim(&home, Ok(first.clone()), None, Some("0.3.8")), Ok(Some(Owned { wsp: owned.wsp.clone(), restart: None })));
    assert_eq!(inode(&owned.wsp), written, "a command naming this bundle is kept");
    let moved = bundle_in(&dir.path().join("Applications"), Os::Mac, "0.3.8");
    let owned = claim(&home, Ok(moved.clone()), None, None).unwrap().unwrap();
    assert_eq!(std::fs::read_to_string(&owned.wsp).unwrap(), shim::text(&moved));
}

/// Catches: a second copy of one release (the move's copy into Applications, which keeps the Downloads one) writing
/// the command with no restart, so the host runs off the first copy until it is trashed and nothing notices.
#[test]
fn a_host_of_this_release_is_restarted_whenever_the_command_it_ran_named_other_files() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    let downloads = bundle_in(&dir.path().join("Downloads"), Os::Mac, "0.3.8");
    let applications = bundle_in(&dir.path().join("Applications"), Os::Mac, "0.3.8");
    assert_eq!(claim(&home, Ok(downloads.clone()), None, None).unwrap().unwrap().restart, None, "nothing serves yet");
    for bundle in [&applications, &downloads] {
        let owned = claim(&home, Ok(bundle.clone()), None, Some("0.3.8")).unwrap().unwrap();
        assert_eq!(owned.restart, Some(Restart::Rewritten), "the host ran the other copy, though it still stands");
        assert_eq!(std::fs::read_to_string(&owned.wsp).unwrap(), shim::text(bundle));
    }
    let kept = claim(&home, Ok(downloads.clone()), None, Some("0.3.8")).unwrap().unwrap();
    assert_eq!(kept.restart, None, "a kept command restarts nothing");
}

/// Catches: an older copy of the app (an old `.app` in Downloads, an older DMG or AppImage) rewriting the command and
/// restarting a newer host onto its older files, which refuse the database the newer wsp migrated; or the two apps
/// taking turns restarting the host on every switch.
#[test]
fn a_newer_host_or_command_is_left_as_it_is_and_two_apps_taking_turns_restart_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    let newer = bundle_in(&dir.path().join("Applications"), Os::Mac, "0.4.0");
    let older = bundle_in(&dir.path().join("Downloads"), Os::Mac, "0.3.8");
    let wsp = claim(&home, Ok(newer.clone()), None, None).unwrap().unwrap().wsp;
    let before = snapshot(&home);
    for _ in 0..2 {
        assert_eq!(claim(&home, Ok(older.clone()), None, Some("0.4.0")), Ok(None), "a newer host serves");
        assert_eq!(claim(&home, Ok(older.clone()), None, None), Ok(None), "the command runs newer files that stand");
        assert_eq!(snapshot(&home), before);
        assert_eq!(claim(&home, Ok(newer.clone()), None, Some("0.4.0")), Ok(Some(Owned { wsp: wsp.clone(), restart: None })));
        assert_eq!(snapshot(&home), before);
    }
    // The newer bundle trashed, moved or on a volume that went, while its host still serves.
    std::fs::remove_dir_all(dir.path().join("Applications")).unwrap();
    assert_eq!(claim(&home, Ok(older.clone()), None, Some("0.4.0")), Ok(None));
    assert_eq!(snapshot(&home), before);
}

/// Catches: an update that replaced the app in place, under the same path, keeping the command's text and so
/// restarting nothing, which leaves the service on the replaced bundle and the newer app refusing its host.
#[test]
fn an_update_in_place_restarts_the_older_host_though_the_command_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    let bundle = bundle_in(&dir.path().join("Applications"), Os::Mac, "0.3.8");
    let wsp = claim(&home, Ok(bundle.clone()), None, None).unwrap().unwrap().wsp;
    let written = inode(&wsp);
    set_release(&bundle, "0.3.9");
    assert_eq!(claim(&home, Ok(bundle.clone()), None, Some("0.3.8")), Ok(Some(Owned { wsp: wsp.clone(), restart: Some(Restart::Older) })));
    assert_eq!(inode(&wsp), written, "the command already names these files");
    assert_eq!(claim(&home, Ok(bundle.clone()), None, Some("0.3.9")), Ok(Some(Owned { wsp, restart: None })), "once level");
}

/// Catches: a command or a restart naming a relative path, which a launch from a relative path would write and the
/// unit would run from wherever its manager starts it.
#[test]
fn a_command_naming_a_relative_path_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    let relative = Bundled { node: PathBuf::from("dist/usr/bin/node"), ..bundle_in(dir.path(), Os::Linux, "0.3.8") };
    let refused = claim(&home, Ok(relative), None, None).unwrap_err();
    assert_eq!(
        refused,
        format!("the wsp command was not written at {}: dist/usr/bin/node is not an absolute path", home.join("bin/wsp").display())
    );
    assert!(!home.join("bin/wsp").exists());
}

/// Catches: today's app writing its command while this app copies its files out of the AppImage, then this app
/// renaming its own over it.
#[test]
fn a_command_another_app_wrote_meanwhile_is_left_as_that_app_wrote_it() {
    let dir = tempfile::tempdir().unwrap();
    let wsp = dir.path().join(".wsp/bin/wsp");
    std::fs::create_dir_all(wsp.parent().unwrap()).unwrap();
    std::fs::write(
        &wsp,
        "today's app wrote this meanwhile
",
    )
    .unwrap();
    let refused = shim::install(
        &wsp,
        "#!/bin/sh
this app's
",
        Some(
            "what the launch read
",
        ),
    )
    .unwrap_err();
    assert!(refused.to_string().contains("another app wrote it"), "{refused}");
    assert_eq!(
        std::fs::read_to_string(&wsp).unwrap(),
        "today's app wrote this meanwhile
"
    );
    assert_eq!(std::fs::read_dir(wsp.parent().unwrap()).unwrap().count(), 1, "no part file left");
    assert_eq!(
        shim::install(
            &wsp,
            "#!/bin/sh
this app's
",
            Some(
                "today's app wrote this meanwhile
"
            )
        )
        .unwrap(),
        shim::Done::Written
    );
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
    let bundle = bundle_in(&dir.path().join("gpui"), Os::Mac, "0.3.8");
    assert_eq!(claim(&home, Ok(bundle.clone()), None, None), Ok(None));
    assert_eq!(snapshot(&home), before);

    // Today's app gone, its command names nothing that runs, and this app's is written in its place.
    std::fs::remove_file(&electron).unwrap();
    let owned = claim(&home, Ok(bundle.clone()), None, None).unwrap().unwrap();
    assert_eq!(std::fs::read_to_string(owned.wsp).unwrap(), shim::text(&bundle));
    let after: Vec<_> = snapshot(&home).into_iter().filter(|(path, _)| !path.ends_with("bin/wsp")).collect();
    let kept: Vec<_> = before.into_iter().filter(|(path, _)| !path.ends_with("bin/wsp")).collect();
    assert_eq!(after, kept, "nothing of today's state, logs or copies changed");
}

/// Catches: today's older command taken over and its host restarted onto this app while Go Back stays offered: today's
/// app writes its own command again on its next launch and keeps the newer host, so the next login runs its older host
/// on the newer database. Its AppImage copy under `<home>/app` stands like an app.
#[test]
fn todays_command_is_left_whatever_its_release_while_go_back_is_offered() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    todays_home(&home);
    let copy = home.join("app/0.3.8-1a2b3c4d");
    std::fs::create_dir_all(copy.join("resources/app/main")).unwrap();
    std::fs::write(copy.join("resources/app/main/cli.mjs"), "cli\n").unwrap();
    std::fs::write(copy.join("resources/app/package.json"), r#"{"name":"@wsp/desktop","version":"0.3.8"}"#).unwrap();
    let todays = format!(
        "#!/bin/sh\n# The wsp app writes this on each launch; it runs the wsp the app bundles.\nELECTRON_RUN_AS_NODE=1 exec '{daemon}' forward --wsp-argv '{exe}' --wsp-argv --no-memory-pool --wsp-argv '{cli}' -- \"$@\"\n",
        daemon = copy.join("resources/app/assets/daemon/x86_64-unknown-linux-musl/wsp-daemon").display(),
        exe = copy.join("wsp").display(),
        cli = copy.join("resources/app/main/cli.mjs").display(),
    );
    std::fs::create_dir_all(home.join("bin")).unwrap();
    std::fs::write(home.join("bin/wsp"), &todays).unwrap();
    let before = snapshot(&home);
    for release in ["0.3.8", "0.3.9"] {
        let bundle = bundle_in(&dir.path().join(release), Os::Linux, release);
        assert_eq!(claim(&home, Ok(bundle.clone()), None, Some("0.3.8")), Ok(None), "today's host serving");
        assert_eq!(claim(&home, Ok(bundle), None, None), Ok(None), "nothing serving");
    }
    assert_eq!(snapshot(&home), before, "today's command, copies and state are as they were");
    assert!(matches!(left_as_is(&home, Some("0.3.8"), "0.3.9"), Some(Left::Today(_))), "the update reads the same rule");
}

/// Catches: today's app of this release losing its command to this app because an older host (a terminal's
/// `wsp up`, today's own service before its restart) serves, which moves today's service onto this app's files.
#[test]
fn todays_app_of_this_release_keeps_its_command_whatever_release_serves() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    let electron = dir.path().join("Applications/wsp.app/Contents/MacOS/wsp");
    let cli = dir.path().join("Applications/wsp.app/Contents/Resources/app/main/cli.mjs");
    std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
    std::fs::create_dir_all(electron.parent().unwrap()).unwrap();
    std::fs::write(&electron, "electron").unwrap();
    std::fs::write(&cli, "cli").unwrap();
    std::fs::write(cli.parent().unwrap().parent().unwrap().join("package.json"), r#"{"version":"0.3.9"}"#).unwrap();
    let todays = format!("#!/bin/sh\nELECTRON_RUN_AS_NODE=1 exec '{}' --no-memory-pool '{}' \"$@\"\n", electron.display(), cli.display());
    std::fs::create_dir_all(home.join("bin")).unwrap();
    std::fs::write(home.join("bin/wsp"), &todays).unwrap();
    let bundle = bundle_in(&dir.path().join("gpui"), Os::Mac, "0.3.9");
    assert_eq!(claim(&home, Ok(bundle), None, Some("0.3.8")), Ok(None));
    assert_eq!(std::fs::read_to_string(home.join("bin/wsp")).unwrap(), todays);
}

/// Catches: a release read off anything but the files the command runs, today's app's or this app's.
#[test]
fn the_command_a_wsp_command_runs_is_read_off_either_apps_text() {
    let bundle = Bundled::beside(Path::new("/Apps/wsp Experimental.app/Contents/MacOS/desktop-gpui"), Os::Mac, "aarch64");
    assert_eq!(shim::command_of(&shim::text(&bundle)), Some(bundle.command.clone()));
    let plain = "#!/bin/sh\nELECTRON_RUN_AS_NODE=1 exec /Applications/wsp.app/Contents/MacOS/wsp --no-memory-pool '/Applications/wsp.app/Contents/Resources/app/main/cli.mjs' \"$@\"\n";
    assert_eq!(shim::command_of(plain), Some(PathBuf::from("/Applications/wsp.app/Contents/Resources/app/main/cli.mjs")));
    assert_eq!(shim::command_of("#!/bin/sh\nexec node dist/cli.mjs \"$@\"\n"), None, "a relative command names no files");
    assert_eq!(shim::command_of("#!/bin/sh\necho hi\n"), None);
}

#[test]
fn a_build_with_no_host_writes_nothing_and_says_which_file_it_lacks() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    let refused = claim(&home, Err("this build carries no host of its own: /x/node is not there".to_owned()), None, None);
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
    let bundle = bundle_in(&mount, Os::Linux, "0.3.8");
    std::fs::write(mount.join("usr/lib/wsp/app/main/chunk-A.mjs"), "chunk").unwrap();
    let file = dir.path().join("wsp-experimental-0.1.0-x86_64.AppImage");
    let image = Image { appdir: mount.clone(), file: file.clone() };
    let mut copies = Vec::new();
    for size in 1..=3 {
        std::fs::write(&file, "x".repeat(size)).unwrap();
        let owned = claim(&home, Ok(bundle.clone()), Some(&image), None).unwrap().unwrap();
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

/// Catches: a swap that put the old app back after the new app had moved the host onto its own files, where claim reads
/// the failed update's command as newer and leaves the host on files the stage then deletes.
#[test]
fn a_swap_put_back_takes_the_command_back_from_the_failed_newer_files() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    let failed = bundle_in(&dir.path().join("stage/failed"), Os::Mac, "0.3.9");
    let old = bundle_in(&dir.path().join("Applications"), Os::Mac, "0.3.8");
    let wsp = claim(&home, Ok(failed.clone()), None, Some("0.3.8")).unwrap().unwrap().wsp;
    assert_eq!(claim(&home, Ok(old.clone()), None, Some("0.3.9")), Ok(None), "claim leaves a newer host and command");
    assert_eq!(take_back(&home, Ok(old.clone()), None), Ok(wsp.clone()));
    assert_eq!(std::fs::read_to_string(&wsp).unwrap(), shim::text(&old), "the command runs the old app's files again");
}

/// A manager no test may reach: every road `restart_host` takes here says why before it would run one.
struct NoManager;

impl Runner for NoManager {
    async fn run(&self, argv: &[String]) -> Ran {
        panic!("a manager's command ran: {argv:?}")
    }
}

fn follow_onto_now(state: &Path, here: Bundled) -> Result<Followed, String> {
    follow_onto_by(state, here, true, &NoManager)
}

fn follow_onto_by<R: Runner>(state: &Path, here: Bundled, back: bool, runner: &R) -> Result<Followed, String> {
    let vars: Env = [("HOME".to_owned(), state.parent().unwrap().to_string_lossy().into_owned())].into();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(follow_onto(state, &vars, Ok(here), runner, back))
}

/// A manager that keeps every command it was handed and holds no unit.
#[derive(Default)]
struct Kept(std::sync::Mutex<Vec<String>>);

impl Runner for Kept {
    async fn run(&self, argv: &[String]) -> Ran {
        self.0.lock().unwrap().push(argv.join(" "));
        let held = argv.get(1).is_some_and(|word| word == "print");
        Ran { code: if held { 113 } else { 0 }, output: "ActiveState=inactive\nUnitFileState=disabled\nMainPID=0".to_owned() }
    }
}

/// Catches: `NotServing` answered while the unit runs npm's older wsp, which the next start loads as it stands, so
/// the host an update's swap waits on comes back on npm's files.
#[test]
fn nothing_serving_on_a_unit_running_npms_wsp_has_the_unit_run_the_command() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    std::fs::create_dir_all(&home).unwrap();
    let state = home.join("state.json");
    let manager = start::Manager::here().unwrap();
    let unit = manager.unit(&home, &state);
    std::fs::create_dir_all(unit.parent().unwrap()).unwrap();
    let npm = "/usr/lib/node_modules/@wsp-labs/wsp/dist/bin.js";
    let text = match manager {
        start::Manager::Systemd => format!("ExecStart='/usr/bin/node' '{npm}' 'up' '--state' '{}'\n", state.display()),
        start::Manager::Launchd => format!(
            "<key>ProgramArguments</key>\n  <array>\n    <string>/usr/bin/node</string>\n    <string>{npm}</string>\n    <string>up</string>\n    <string>--state</string>\n    <string>{}</string>\n  </array>\n",
            state.display()
        ),
    };
    std::fs::write(&unit, text).unwrap();
    let newer = bundle_in(&dir.path().join("Applications"), Os::here(), "0.3.9");
    let kept = Kept::default();
    assert_eq!(follow_onto_by(&state, newer, true, &kept), Ok(Followed::NotServing));
    let up = format!("{} up --service --state {}", shim::path(&home).display(), state.display());
    assert!(kept.0.lock().unwrap().contains(&up), "{:?}", kept.0.lock().unwrap());
}

/// Catches: the failure box saying Go Back returns to today's app while the switch is off and it offers no Go Back.
#[test]
fn a_host_left_for_todays_app_names_go_back_only_where_it_is_offered() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    std::fs::create_dir_all(home.join("bin")).unwrap();
    let state = home.join("state.json");
    let electron = dir.path().join("wsp.app/Contents/MacOS/wsp");
    std::fs::create_dir_all(electron.parent().unwrap()).unwrap();
    std::fs::write(&electron, "electron").unwrap();
    std::fs::write(shim::path(&home), format!("#!/bin/sh\nELECTRON_RUN_AS_NODE=1 exec '{}' /x/cli.mjs \"$@\"\n", electron.display()))
        .unwrap();
    let newer = bundle_in(&dir.path().join("Applications"), Os::Mac, "0.3.9");
    let said = |back: bool| match follow_onto_by(&state, newer.clone(), back, &NoManager) {
        Ok(Followed::Left(why)) => why,
        other => panic!("the host was not said to be left: {other:?}"),
    };
    assert!(said(true).contains(switch::GO_BACK), "{}", said(true));
    let off = said(false);
    assert!(!off.contains("Go Back") && off.ends_with("whose host this app leaves; open that app instead"), "{off}");
}

/// Catches: `restart_host` answering Ok where it restarted nothing, which the update's swap reads as the host
/// following it (it deletes the old app the host still runs off) and the ask reads as done, opening on the bare
/// refusal with no reason.
#[test]
fn restart_host_says_what_it_did_and_why_it_left_a_host() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".wsp");
    std::fs::create_dir_all(&home).unwrap();
    let state = home.join("state.json");
    let older = bundle_in(&dir.path().join("Downloads"), Os::Mac, "0.3.8");
    let newer = bundle_in(&dir.path().join("Applications"), Os::Mac, "0.3.9");
    assert_eq!(follow_onto_now(&state, newer.clone()), Ok(Followed::NotServing));

    let said = |followed: Result<Followed, String>| match followed {
        Ok(Followed::Left(why)) => why,
        other => panic!("the host was not said to be left: {other:?}"),
    };
    assert!(said(follow_onto_now(&state, older.clone())).contains("newer than wsp 0.3.8"), "the command runs newer files");

    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let lock = format!(r#"{{"pid":{},"port":{port},"startedAt":"2026-10-24T00:00:00.000Z","startedBy":"service"}}"#, std::process::id());
    std::fs::write(home.join("host.lock"), lock).unwrap();
    assert!(said(follow_onto_now(&state, newer.clone())).contains("did not say its release within 2 s"), "a page that does not answer");
    std::fs::remove_file(home.join("host.lock")).unwrap();

    let electron = dir.path().join("wsp.app/Contents/MacOS/wsp");
    std::fs::create_dir_all(electron.parent().unwrap()).unwrap();
    std::fs::write(&electron, "electron").unwrap();
    std::fs::write(shim::path(&home), format!("#!/bin/sh\nELECTRON_RUN_AS_NODE=1 exec '{}' /x/cli.mjs \"$@\"\n", electron.display()))
        .unwrap();
    assert!(said(follow_onto_now(&state, newer)).contains(switch::GO_BACK), "today's app owns the command");
}

/// Catches: npm's links read again, and the login shell run again, on every open after the launch's: a later open, the
/// ask's reopen, the update's move.
#[test]
fn npms_links_are_read_once_in_a_run() {
    let dir = tempfile::tempdir().unwrap();
    let env: Env = [("PATH".to_owned(), "/usr/bin:/bin".to_owned())].into();
    assert!(npm_pass(&dir.path().join(".wsp"), &env, "0.3.8".to_owned()));
    assert!(!npm_pass(&dir.path().join(".wsp"), &env, "0.3.8".to_owned()));
}
