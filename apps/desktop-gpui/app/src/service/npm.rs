// SPDX-License-Identifier: AGPL-3.0-only
//! npm's own `wsp` on PATH beside this app's (`desktop:shim.ts` `replaceOlderOnPath`): a global install's link of an
//! earlier release than the bundle's is pointed at the wsp command, so the wsp a shell finds runs the host this app
//! runs whatever order PATH comes in; one of the same or a later release stays as it is.

use std::cmp::Ordering;
use std::ffi::CStr;
use std::io::Read as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use wsp_host_client::{release_order, Env};

use super::{resolved, shim};

/// Where npm puts a global install's package, from the folder holding its bin link: `<prefix>/bin/wsp` points into
/// `<prefix>/lib/node_modules/@wsp-labs/wsp`.
const NPM_PACKAGE: [&str; 5] = ["..", "lib", "node_modules", "@wsp-labs", "wsp"];

/// This login's own wsp home, off the password file and not HOME (`defaultHomeIn(userInfo().homedir)`).
pub fn own_home() -> PathBuf {
    // SAFETY: getpwuid's record is read at once, on this thread, before any other call could reuse it.
    let dir = unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() || (*entry).pw_dir.is_null() {
            return PathBuf::new();
        }
        CStr::from_ptr((*entry).pw_dir).to_string_lossy().into_owned()
    };
    Path::new(&dir).join(".wsp")
}

/// The PATH launchd gives an app it starts, in no fixed order (`LAUNCHD_PATH`).
const LAUNCHD_PATH: [&str; 4] = ["/usr/bin", "/bin", "/usr/sbin", "/sbin"];
/// How long the login shell is given (`SHELL_LIMIT_MS`).
const SHELL_LIMIT: Duration = Duration::from_secs(15);

/// The PATH a shell of this person's finds `wsp` on (`adoptLoginPath`): the launch's own, except where launchd or a
/// desktop chose it (launchd's four folders, an AppImage launch), where the login shell is asked. A shell that fails,
/// runs past its limit or prints nothing leaves the launch's.
pub fn login_path(env: &Env) -> String {
    let given = env.get("PATH").cloned().unwrap_or_default();
    let dirs: Vec<&str> = given.split(':').filter(|dir| !dir.is_empty()).collect();
    let launchd = dirs.len() == LAUNCHD_PATH.len() && LAUNCHD_PATH.iter().all(|dir| dirs.contains(dir));
    let chosen = launchd || env.get("APPIMAGE").is_some_and(|image| !image.is_empty());
    let Some(shell) = env.get("SHELL").filter(|shell| chosen && !shell.is_empty()) else { return given };
    // Its own process group, so a job an rc file leaves behind, holding the output open, goes with it.
    let child = Command::new(shell)
        .args(["-ilc", "printf '%s' \"$PATH\""])
        .env_clear()
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
    let Ok(mut child) = child else { return given };
    let until = Instant::now() + SHELL_LIMIT;
    let exited = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(50)),
            _ => break false,
        }
    };
    // SAFETY: the group this function's own child leads, which it started above.
    unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
    let _ = child.wait();
    let mut said = String::new();
    let read = child.stdout.take().is_some_and(|mut out| out.read_to_string(&mut said).is_ok());
    let said = said.trim();
    if exited && read && !said.is_empty() {
        said.to_owned()
    } else {
        given
    }
}

/// The release an installed @wsp-labs/wsp package says it is, read off its package.json and never by running it.
fn package_release(dir: &Path) -> Option<String> {
    let pkg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("package.json")).ok()?).ok()?;
    (pkg.get("name")?.as_str()? == "@wsp-labs/wsp").then(|| pkg.get("version")?.as_str().map(str::to_owned))?
}

/// Makes every npm global install's `wsp` link on the PATH `path` reads of an earlier release than `release` a link to
/// the wsp command. A wsp is npm's by where its link points, never by what it prints: a file, a link anywhere else, a later
/// release and a folder PATH names relative to wherever the app started all stay. Only the app on this login's own
/// home `own` touches PATH, or reads it, since a test or a lab on another home would point the real wsp into a folder
/// that goes.
/// A line for each wsp it replaced or left.
pub fn replace_older_on_path(home: &Path, path: impl FnOnce() -> String, release: &str, own: &Path) -> Vec<String> {
    let root = Path::new("/");
    if resolved(home, root) != resolved(own, root) {
        return Vec::new();
    }
    let wsp = shim::path(home);
    let bin = wsp.parent().unwrap_or(root);
    let mut lines = Vec::new();
    let mut seen = Vec::new();
    let path = path();
    for dir in path.split(':').map(Path::new).filter(|dir| dir.is_absolute()) {
        if seen.contains(&dir) {
            continue;
        }
        seen.push(dir);
        let file = dir.join("wsp");
        let Ok(meta) = std::fs::symlink_metadata(&file) else { continue };
        if !meta.file_type().is_symlink() {
            if file != wsp {
                lines.push(format!("{} is a file of its own, not npm's link to wsp, so it stays", file.display()));
            }
            continue;
        }
        let Ok(link) = std::fs::read_link(&file) else { continue };
        let target = resolved(&link, dir);
        if target.starts_with(bin) && target != bin {
            continue;
        }
        let pkg = resolved(&NPM_PACKAGE.iter().collect::<PathBuf>(), dir);
        let theirs = (target.starts_with(&pkg) && target != pkg).then(|| package_release(&pkg)).flatten();
        let Some(theirs) = theirs else {
            lines.push(format!("{} points at {}, not an npm install of wsp, so it stays", file.display(), target.display()));
            continue;
        };
        match release_order(&theirs, release) {
            Ordering::Greater => {
                lines.push(format!("{} is wsp {theirs}, later than this app's {release}, so it stays", file.display()));
                continue;
            }
            Ordering::Equal => continue,
            Ordering::Less => {}
        }
        let part = dir.join(format!(".wsp.{}.part", std::process::id()));
        let _ = std::fs::remove_file(&part);
        match std::os::unix::fs::symlink(&wsp, &part).and_then(|()| std::fs::rename(&part, &file)) {
            Ok(()) => lines.push(format!("{} was npm's wsp {theirs}; it now runs {}, wsp {release}", file.display(), wsp.display())),
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                lines.push(format!(
                    "{} is npm's wsp {theirs} and could not be replaced ({e}); remove it so a shell finds wsp {release}",
                    file.display()
                ));
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An npm prefix under `root` holding @wsp-labs/wsp at `version`, its `bin/wsp` linked into the package as npm
    /// links it; the bin folder.
    fn npm_prefix(root: &Path, version: &str) -> PathBuf {
        let pkg = root.join("lib/node_modules/@wsp-labs/wsp");
        std::fs::create_dir_all(pkg.join("dist")).unwrap();
        std::fs::write(pkg.join("package.json"), format!(r#"{{"name":"@wsp-labs/wsp","version":"{version}"}}"#)).unwrap();
        std::fs::write(pkg.join("dist/bin.js"), "bin").unwrap();
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::os::unix::fs::symlink("../lib/node_modules/@wsp-labs/wsp/dist/bin.js", bin.join("wsp")).unwrap();
        bin
    }

    /// Catches: an npm `wsp` of an earlier release left on PATH, which a shell finds before the app's and whose verbs
    /// the app's newer host refuses, or one of a later release pointed back at the app's older host.
    #[test]
    fn an_older_npm_wsp_on_path_runs_the_wsp_command_and_a_newer_one_stays() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".wsp");
        let older = npm_prefix(&dir.path().join("old"), "0.3.7");
        let newer = npm_prefix(&dir.path().join("new"), "0.4.0");
        let same = npm_prefix(&dir.path().join("same"), "0.3.8");
        let mine = dir.path().join("mine");
        std::fs::create_dir_all(&mine).unwrap();
        std::fs::write(mine.join("wsp"), "#!/bin/sh\n").unwrap();
        let path = [&older, &newer, &same, &mine].map(|dir| dir.to_string_lossy().into_owned()).join(":") + ":relative/bin";
        let lines = replace_older_on_path(&home, || path, "0.3.8", &home);
        let wsp = shim::path(&home);
        assert_eq!(std::fs::read_link(older.join("wsp")).unwrap(), wsp);
        assert_eq!(std::fs::read_link(newer.join("wsp")).unwrap(), Path::new("../lib/node_modules/@wsp-labs/wsp/dist/bin.js"));
        assert_eq!(std::fs::read_link(same.join("wsp")).unwrap(), Path::new("../lib/node_modules/@wsp-labs/wsp/dist/bin.js"));
        assert_eq!(
            lines,
            [
                format!("{} was npm's wsp 0.3.7; it now runs {}, wsp 0.3.8", older.join("wsp").display(), wsp.display()),
                format!("{} is wsp 0.4.0, later than this app's 0.3.8, so it stays", newer.join("wsp").display()),
                format!("{} is a file of its own, not npm's link to wsp, so it stays", mine.join("wsp").display()),
            ]
        );
    }

    /// Catches: npm's links read off launchd's PATH, which an app opened from Finder or the Dock is handed and which
    /// holds no npm prefix, so an older npm `wsp` is never found.
    #[test]
    fn the_path_is_the_login_shells_where_launchd_or_a_desktop_chose_the_launchs() {
        let dir = tempfile::tempdir().unwrap();
        let shell = dir.path().join("login-shell");
        std::fs::write(&shell, "#!/bin/sh\n[ \"$1\" = -ilc ] && printf '%s' /opt/homebrew/bin:/usr/bin\n").unwrap();
        std::fs::set_permissions(&shell, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let env = |path: &str, image: &str| -> Env {
            [("PATH", path), ("SHELL", &shell.to_string_lossy()), ("APPIMAGE", image)].map(|(k, v)| (k.to_owned(), v.to_owned())).into()
        };
        assert_eq!(login_path(&env("/sbin:/usr/bin:/bin:/usr/sbin", "")), "/opt/homebrew/bin:/usr/bin");
        assert_eq!(login_path(&env("/usr/bin:/bin", "/apps/wsp.AppImage")), "/opt/homebrew/bin:/usr/bin");
        assert_eq!(
            login_path(&env("/home/me/.local/bin:/usr/bin:/bin", "")),
            "/home/me/.local/bin:/usr/bin:/bin",
            "a terminal's PATH stands"
        );
        std::fs::write(&shell, "#!/bin/sh\nexit 1\n").unwrap();
        assert_eq!(login_path(&env("/usr/bin:/bin:/usr/sbin:/sbin", "")), "/usr/bin:/bin:/usr/sbin:/sbin", "a failed shell leaves it");
    }

    #[test]
    fn a_launch_on_another_home_touches_no_wsp_on_path() {
        let dir = tempfile::tempdir().unwrap();
        let older = npm_prefix(&dir.path().join("old"), "0.3.7");
        let path = || -> String { panic!("a launch on another home read the login PATH") };
        let lines = replace_older_on_path(&dir.path().join("lab/.wsp"), path, "0.3.8", &dir.path().join(".wsp"));
        assert!(lines.is_empty(), "{lines:?}");
        assert_eq!(std::fs::read_link(older.join("wsp")).unwrap(), Path::new("../lib/node_modules/@wsp-labs/wsp/dist/bin.js"));
    }
}
