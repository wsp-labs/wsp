// SPDX-License-Identifier: AGPL-3.0-only
//! The swap's own rules, run as the app runs the script, in temp folders: `up` waits for the new app to open on its
//! host, the old app goes only after `up`, one swap runs at a time, the Linux tools are found, not assumed, and a
//! Mac's are named by their paths.

use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

use super::feed::Feed;
use super::stage::{self, Installed, Kind};
use super::swap::{self, Settled};
use super::tests::{installs, quit_app, read, release, run_swap, run_swap_on, serve, swapping, tmp, until, Swapping};
use super::*;

fn log_of(s: &Swapping) -> String {
    std::fs::read_to_string(&s.swap.log).unwrap_or_default()
}

/// The new app the swap opened, as its log names it.
fn new_pid(s: &Swapping) -> i32 {
    let log = log_of(s);
    let line = log.lines().find(|l| l.contains(" as pid ")).expect("the swap opened the new app");
    line.rsplit(' ').next().unwrap().parse().unwrap()
}

#[test]
fn up_waits_for_the_new_app_to_open_on_its_host() {
    let s = swapping(quit_app(), "exec sleep 30");
    let old = read(&s.installed);
    let mut script = run_swap(&s.swap);
    let root = stage::stage_of(&s.installed);
    until("the swap opened the new app", Duration::from_secs(10), || root.join("opening").exists());
    let waiting =
        after_swap(Some(swap::settle(&s.installed)), || Ok(crate::service::Followed::Restarted), || panic!("nothing to move back"))
            .unwrap();
    assert!(waiting.is_some(), "the swap is handed back to the launch");
    assert!(!root.join("up").exists(), "a host that took the restart is not yet an app open on it");
    // The new app's open is refused: `not_opened` exits it.
    // SAFETY: the pid is the new app the swap started for this test, still running.
    unsafe { libc::kill(new_pid(&s), libc::SIGTERM) };
    assert!(!script.wait().unwrap().success());
    assert_eq!(read(&s.installed), old, "the old app is back in place");
    assert_eq!(std::fs::read_to_string(root.join("back")).unwrap().trim(), "restart", "and moves the host back");
    assert!(log_of(&s).contains("the new app exited before it was up; putting the old app back"), "{}", log_of(&s));

    for install in installs() {
        let (owned, installed_at) = (install.dir.path(), &install.installed.path);
        let served = serve(release("0.3.9", &[(&install.asset, &install.new)], &HashMap::new()));
        let mut road = install.shell(Feed::new(Some(served.base), "wsp/0.3.8".into()));
        road.get("0.3.9").unwrap();
        let cli = owned.join("today/app/main/cli.mjs");
        std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
        std::fs::write(&cli, "cli").unwrap();
        std::fs::write(owned.join("today/app/package.json"), r#"{"version":"0.4.0"}"#).unwrap();
        let wsp = owned.join("wsp-home/bin/wsp");
        std::fs::create_dir_all(wsp.parent().unwrap()).unwrap();
        std::fs::write(&wsp, format!("#!/bin/sh\nELECTRON_RUN_AS_NODE=1 exec '/x/wsp' '{}' \"$@\"\n", cli.display())).unwrap();
        let said = road.open().unwrap_err().error;
        assert_eq!(said, words::newer_command("0.4.0", "0.3.9"));
        assert!(!stage::stage_of(installed_at).join("swapping").exists(), "no swap started");
        assert_eq!(install.now(installed_at), install.was);
    }
}

#[test]
fn the_old_app_goes_only_after_up() {
    let s = swapping(quit_app(), "exec sleep 30");
    let old = read(&s.installed);
    let mut script = run_swap(&s.swap);
    let root = stage::stage_of(&s.installed);
    until("the swap opened the new app", Duration::from_secs(10), || root.join("opening").exists());
    // A logout's SIGKILL ends the swap while it waits on the new app.
    script.kill().unwrap();
    script.wait().unwrap();
    // SAFETY: the pid is the new app the swap started for this test.
    unsafe { libc::kill(new_pid(&s), libc::SIGKILL) };
    until("the new app is gone", Duration::from_secs(5), || unsafe { libc::kill(new_pid(&s), 0) } != 0);
    let previous = root.join(stage::PREVIOUS);
    assert_eq!(swap::settle(&s.installed), Settled::Unfinished { previous: previous.clone() }, "the next launch keeps the app it replaced");
    assert_eq!(read(&previous), old);
    stage::discard(&s.installed);
    assert_eq!(
        swap::settle(&s.installed),
        Settled::Unfinished { previous: previous.clone() },
        "and offers it on every launch until it is back"
    );

    // Put back: the kept app is staged and swapped in, and says whatever version it is.
    let back_app = format!(
        "#!/bin/sh\n: > '{}'\nprintf 0.3.8 > '{}'\nexec sleep 3\n",
        s.installed.with_file_name("old.ran").display(),
        root.join("up").display()
    );
    std::fs::write(&previous, &back_app).unwrap();
    // SAFETY: getuid reads this process's user and cannot fail.
    let staged = stage::restage(&Installed { path: s.installed.clone(), kind: Kind::AppImage }, &|_, _| (1, String::new()), unsafe {
        libc::getuid()
    })
    .unwrap();
    let sum = super::feed::sum_of(&staged.app).unwrap();
    let put_back = swap::Swap { staged, version: String::new(), sum, ..swapping_of(&s) };
    assert!(run_swap(&put_back).wait().unwrap().success(), "{}", log_of(&s));
    assert_eq!(read(&s.installed), back_app.as_bytes(), "the app it replaced is back");
    assert!(!root.exists(), "and once it said up, the stage goes");

    let done = swapping(quit_app(), "exec sleep 30");
    let root = stage::stage_of(&done.installed);
    let mut script = run_swap(&done.swap);
    until("the swap opened the new app", Duration::from_secs(10), || root.join("opening").exists());
    std::fs::write(root.join("up"), "0.3.9").unwrap();
    script.kill().unwrap();
    script.wait().unwrap();
    // SAFETY: the pid is the new app the swap started for this test.
    unsafe { libc::kill(new_pid(&done), libc::SIGKILL) };
    assert_eq!(swap::settle(&done.installed), Settled::Swept, "with up written, the old app goes");
    assert!(!root.exists());
}

/// The arguments of `s` for another swap of the same installed app.
fn swapping_of(s: &Swapping) -> swap::Swap {
    swap::Swap {
        pid: quit_app(),
        installed: s.installed.clone(),
        kind: Kind::AppImage,
        staged: s.swap.staged.clone(),
        version: s.swap.version.clone(),
        sum: s.swap.sum.clone(),
        log: s.swap.log.clone(),
        patience: s.swap.patience,
        grace: s.swap.grace,
    }
}

#[test]
fn one_swap_at_a_time() {
    for install in installs() {
        let installed_at = &install.installed.path;
        let served = serve(release("0.3.9", &[(&install.asset, &install.new)], &HashMap::new()));
        let mut road = install.shell(Feed::new(Some(served.base), "wsp/0.3.8".into()));
        road.get("0.3.9").unwrap();
        // This test's own pid stands for the app, which never quits, so the swap gives up after its patience.
        assert_eq!(road.open(), Ok(Opened::Swapping));
        assert_eq!(road.open().unwrap_err().error, "no verified download to open", "a second Restart starts no second swap");
        let log = install.dir.path().join("update.log");
        until("the swap gave up", Duration::from_secs(10), || std::fs::read_to_string(&log).unwrap_or_default().contains("did not quit"));
        until("the swap let go of the stage", Duration::from_secs(5), || !stage::stage_of(installed_at).join("swapping").exists());
        assert_eq!(install.now(installed_at), install.was);
    }

    let mut app = Command::new("sleep").arg("30").spawn().unwrap();
    let first = swapping(app.id(), "exec sleep 3");
    let mut holder = run_swap(&first.swap);
    let root = stage::stage_of(&first.installed);
    until("the first swap holds the stage", Duration::from_secs(5), || root.join("swapping").exists());
    let second_stage = root.join("stage-second");
    std::fs::create_dir_all(second_stage.join("new")).unwrap();
    std::fs::write(second_stage.join("new/wsp.AppImage"), "second").unwrap();
    let second = swap::Swap {
        staged: stage::Staged {
            app: second_stage.join("new/wsp.AppImage"),
            stage: second_stage,
            exe: String::new(),
            requirement: String::new(),
        },
        sum: super::tests::hex_sum(b"second"),
        ..swapping_of(&first)
    };
    assert!(!run_swap(&second).wait().unwrap().success());
    assert!(log_of(&first).contains("is under way; nothing was changed"), "{}", log_of(&first));
    assert!(first.swap.staged.app.exists(), "the first swap's copy is untouched");
    assert!(!first.installed.with_file_name("new.ran").exists(), "and nothing was swapped while its app runs");
    app.kill().unwrap();
    app.wait().unwrap();
    holder.wait().unwrap();
}

/// A folder of the system's own tools, these alone, as a NixOS system keeps them outside /bin; its sleep marks
/// `sleep.used` beside itself before it sleeps, so a run shows it found that one.
#[cfg(target_os = "linux")]
fn tools_in(names: &[&str]) -> (tempfile::TempDir, String) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tmp();
    for name in names {
        let real = ["/usr/bin", "/bin"].iter().map(|d| std::path::Path::new(d).join(name)).find(|p| p.exists()).unwrap();
        let at = dir.path().join(name);
        if *name == "sleep" {
            std::fs::write(
                &at,
                format!("#!/bin/sh\n: > '{}'\nexec '{}' \"$@\"\n", dir.path().join("sleep.used").display(), real.display()),
            )
            .unwrap();
            std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).unwrap();
        } else {
            std::os::unix::fs::symlink(real, at).unwrap();
        }
    }
    let path = dir.path().to_string_lossy().into_owned();
    (dir, path)
}

#[test]
#[cfg(target_os = "linux")]
fn the_linux_swap_finds_its_tools_or_refuses() {
    let (all, everything) = tools_in(&swap::LINUX_TOOLS);
    let s = swapping(quit_app(), "");
    let up = stage::stage_of(&s.installed).join("up");
    std::fs::write(
        &s.swap.staged.app,
        format!(
            "#!/bin/sh\n: > '{}'\n/bin/sleep 0.5\nprintf 0.3.9 > '{}'\nexec /bin/sleep 3\n",
            s.installed.with_file_name("new.ran").display(),
            up.display()
        ),
    )
    .unwrap();
    let sum = super::feed::sum_of(&s.swap.staged.app).unwrap();
    let found = swap::Swap { sum, ..swapping_of(&s) };
    assert!(run_swap_on(&found, &everything).wait().unwrap().success(), "tools found off a PATH outside /bin: {}", log_of(&s));
    assert!(s.installed.with_file_name("new.ran").exists());
    assert!(all.path().join("sleep.used").exists(), "the swap ran the sleep its PATH holds");

    let (_some, no_sleep) = tools_in(&["mv", "rm", "date", "sha256sum"]);
    let s = swapping(quit_app(), "exec sleep 3");
    let old = read(&s.installed);
    assert!(!run_swap_on(&s.swap, &no_sleep).wait().unwrap().success());
    assert!(log_of(&s).contains("no sleep on "), "{}", log_of(&s));
    assert_eq!(read(&s.installed), old, "nothing was changed");
    assert!(!s.installed.with_file_name("new.ran").exists(), "and the new app never ran from the stage");
    assert_eq!(swap::missing_tools(&no_sleep), ["sleep"]);
    assert!(swap::missing_tools(&everything).is_empty());
}

#[test]
fn the_mac_swap_runs_its_tools_by_their_paths() {
    let named: Vec<&str> = swap::SCRIPT
        .split(|c: char| !(c.is_ascii_alphanumeric() || "/_.-".contains(c)))
        .filter(|word| word.starts_with("/bin/") || word.starts_with("/usr/bin/"))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(
        named,
        ["/bin/date", "/bin/mv", "/bin/rm", "/bin/sleep", "/usr/bin/codesign", "/usr/bin/open", "/usr/bin/plutil"],
        "every tool the Mac's swap runs is named by its path"
    );
    if cfg!(target_os = "macos") {
        use std::os::unix::fs::PermissionsExt;
        for tool in &named {
            let runnable = std::fs::metadata(tool).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0);
            assert!(runnable, "{tool} is a runnable file on this Mac");
        }
    }

    let mut app = Command::new("sleep").arg("30").spawn().unwrap();
    let s = swapping(app.id(), "exec sleep 3");
    let old = read(&s.installed);
    let mac = swap::Swap { pid: app.id(), kind: Kind::Mac, patience: 1, ..swapping_of(&s) };
    let empty = tmp();
    let from = std::time::Instant::now();
    assert!(!run_swap_on(&mac, &empty.path().to_string_lossy()).wait().unwrap().success());
    assert!(from.elapsed() >= Duration::from_millis(900), "its ten waits of 0.1 s ran /bin/sleep on an empty PATH");
    let log = log_of(&s);
    let line = log.lines().find(|l| l.ends_with("did not quit; nothing was changed")).unwrap_or_else(|| panic!("{log}"));
    let stamp = line.split(' ').next().unwrap();
    assert!(stamp.len() == 20 && stamp.ends_with('Z'), "/bin/date dated the line: {line}");
    assert!(!stage::stage_of(&s.installed).join("swapping").exists(), "/bin/rm let go of the stage");
    assert_eq!(read(&s.installed), old, "nothing was changed");
    app.kill().unwrap();
    app.wait().unwrap();
}

#[test]
fn a_host_left_as_it_was_fails_the_swap_and_the_old_app_says_why() {
    let s = swapping(quit_app(), "exec sleep 30");
    let old = read(&s.installed);
    let mut script = run_swap(&s.swap);
    let root = stage::stage_of(&s.installed);
    until("the swap opened the new app", Duration::from_secs(10), || root.join("opening").exists());
    let why = "wsp 0.4.0 serves this home, newer than wsp 0.3.9";
    let left =
        after_swap(Some(swap::settle(&s.installed)), || Ok(crate::service::Followed::Left(why.into())), || panic!("nothing to move back"));
    assert_eq!(left.map(|w| w.is_some()), Err(why.to_owned()), "the new app exits before it is up");
    assert!(!root.join("up").exists());
    // SAFETY: the pid is the new app the swap started for this test, still running.
    unsafe { libc::kill(new_pid(&s), libc::SIGTERM) };
    assert!(!script.wait().unwrap().success());
    assert_eq!(read(&s.installed), old, "the old app is back in place");
    assert_eq!(
        swap::settle(&s.installed),
        Settled::Back { restart: false, said: Some(why.to_owned()) },
        "the host never moved, so it stays, and the app put back says why"
    );
}

#[test]
fn a_swap_put_back_after_a_restart_moves_the_host_back_itself() {
    let moved = std::cell::Cell::new(false);
    let back = after_swap(
        Some(Settled::Back { restart: true, said: None }),
        || panic!("claim's road reads the failed update's host as newer and restarts nothing"),
        || {
            moved.set(true);
            Ok(())
        },
    );
    assert_eq!(back, Ok(None));
    assert!(moved.get(), "the host is restarted back onto the old app's files");
    assert_eq!(
        after_swap(
            Some(Settled::Back { restart: false, said: None }),
            || panic!("the host never moved"),
            || panic!("the host never moved")
        ),
        Ok(None)
    );
}

/// Catches: an app that cannot tell which file it runs from (an AppImage moved, linked or unpacked under its name)
/// offering Restart, the install line's unpacked files taken for an AppImage, and a swap that renames whatever stands at
/// the path.
#[test]
fn an_app_that_cannot_tell_what_it_would_replace_says_so_and_changes_nothing() {
    let dir = tmp();
    let at = |name: &str| dir.path().join(name);
    let image = |name: &str| Installed { path: at(name), kind: Kind::AppImage };
    std::fs::write(at("wsp.AppImage"), b"\x7fELF\x02\x01\x01\x00AI\x02 0.3.8").unwrap();
    assert_eq!(stage::in_place_refusal(&image("wsp.AppImage")), None, "an AppImage file of its own replaces itself");
    std::os::unix::fs::symlink(at("wsp.AppImage"), at("linked.AppImage")).unwrap();
    std::fs::create_dir(at("unpacked.AppImage")).unwrap();
    std::fs::write(at("script.AppImage"), "#!/bin/sh\n").unwrap();
    for name in ["gone.AppImage", "linked.AppImage", "unpacked.AppImage", "script.AppImage"] {
        assert_eq!(stage::in_place_refusal(&image(name)), Some(stage::words::APP_IMAGE_GONE), "{name}");
    }
    if cfg!(target_os = "linux") {
        let files = at(".wsp/app/0.4.0-install/usr/bin/desktop-gpui");
        assert_eq!(Installed::of(&files, None, None), None, "the install line's unpacked files take the Downloads road");
        let leaked = Installed::of(&files, Some(at("wsp.AppImage").as_os_str()), Some(std::ffi::OsStr::new("/tmp/.mount_wspX")));
        assert_eq!(leaked, None, "an APPIMAGE another AppImage's launch left in the environment is not this app's");
    }

    for shape in ["folder", "link"] {
        let s = swapping(quit_app(), "exec sleep 3");
        let real = s.installed.with_file_name("real.AppImage");
        std::fs::rename(&s.installed, &real).unwrap();
        if shape == "folder" {
            std::fs::create_dir(&s.installed).unwrap();
            std::fs::write(s.installed.join("AppRun"), "unpacked").unwrap();
        } else {
            std::os::unix::fs::symlink(&real, &s.installed).unwrap();
        }
        assert!(!run_swap(&s.swap).wait().unwrap().success(), "{shape}");
        assert!(log_of(&s).contains("so the update cannot tell what it would replace; nothing was changed"), "{shape}: {}", log_of(&s));
        assert!(!s.installed.with_file_name("new.ran").exists(), "{shape}: the new app never ran");
        assert!(!s.swap.staged.stage.exists(), "{shape}: the checked copy went");
        if shape == "folder" {
            assert_eq!(std::fs::read_to_string(s.installed.join("AppRun")).unwrap(), "unpacked", "the folder stands as it was");
        } else {
            assert_eq!(std::fs::read_link(&s.installed).unwrap(), real, "the link stands as it was");
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn the_swap_keeps_an_appimage_that_unpacks_itself_doing_so() {
    let kept = swap::env(|name| ["HOME", "APPIMAGE_EXTRACT_AND_RUN", "GH_TOKEN"].contains(&name).then(|| "1".to_owned()));
    let names: Vec<&str> = kept.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["PATH", "HOME", "APPIMAGE_EXTRACT_AND_RUN"], "where FUSE is missing the new app starts as the old one did");
}
