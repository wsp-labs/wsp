// SPDX-License-Identifier: AGPL-3.0-only
//! The switch between the two apps, GPUI's side: whether the person asked for this app instead of today's (a file
//! under `<home>/desktop/`, read before the first window), the path today's app finds this one by
//! (`<home>/desktop/desktop-gpui.path`), and the way back, which takes the switch away, opens today's app through the
//! executable the wsp command names (`desktop:shim.ts`) and quits once that open went.

use std::ffi::OsString;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use gpui_kit::{App, AppContext as _, AsyncApp, Global, PromptLevel};

/// Present while the person runs this app instead of today's.
pub const SWITCH_FILE: &str = "experimental";
/// This app's own executable, for today's app to open it by.
pub const PATH_FILE: &str = "desktop-gpui.path";

/// The menu row that takes the person back to today's app.
pub const GO_BACK: &str = "Go Back to the Current wsp App";
/// The box a way back that did not open says.
pub const NOT_OPENED: &str = "The current wsp app did not open";

/// Whether the switch was on as this launch read it, before its first window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Switch {
    pub on: bool,
}

impl Global for Switch {}

/// The folder today's app keeps its own lock and Start at login's unit in, where both switch files live.
pub fn desktop_dir(home: &Path) -> PathBuf {
    home.join("desktop")
}

pub fn switch_file(home: &Path) -> PathBuf {
    desktop_dir(home).join(SWITCH_FILE)
}

pub fn path_file(home: &Path) -> PathBuf {
    desktop_dir(home).join(PATH_FILE)
}

/// What today's app opens this one by: under an AppImage the AppImage's own file, since the executable sits in a mount
/// that goes when the app exits; else the executable. `appimage` is the launch's APPIMAGE variable.
pub fn own_path(appimage: Option<OsString>) -> io::Result<PathBuf> {
    match appimage.filter(|path| !path.is_empty()) {
        Some(path) => Ok(PathBuf::from(path)),
        None => std::env::current_exe(),
    }
}

/// Reads the switch: its file's presence means on.
pub fn read(home: &Path) -> Switch {
    Switch { on: switch_file(home).exists() }
}

/// Whether this run is the one the switch asked for.
pub fn on(cx: &App) -> bool {
    cx.try_global::<Switch>().is_some_and(|s| s.on)
}

/// Writes `exe`'s absolute path to `<home>/desktop/desktop-gpui.path`, whole or not at all, readable by today's app;
/// a missing `desktop` folder is made private to this login, as the home is.
pub fn write_path(home: &Path, exe: &Path) -> io::Result<()> {
    let exe = std::path::absolute(exe)?;
    let text = exe.to_str().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "the executable's path is not UTF-8"))?;
    let dir = desktop_dir(home);
    make_dir(&dir)?;
    let path = path_file(home);
    let part = dir.join(format!("{PATH_FILE}.{}.part", std::process::id()));
    let mut file = part_file(&part)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&part, &path).inspect_err(|_| _ = std::fs::remove_file(&part))
}

#[cfg(unix)]
fn make_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
}

#[cfg(not(unix))]
fn make_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)
}

#[cfg(unix)]
fn part_file(part: &Path) -> io::Result<std::fs::File> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o644).open(part)?;
    // The mode on open is cut by the umask; the file is 0644 whatever it is.
    file.set_permissions(std::fs::Permissions::from_mode(0o644))?;
    Ok(file)
}

#[cfg(not(unix))]
fn part_file(part: &Path) -> io::Result<std::fs::File> {
    std::fs::File::create(part)
}

/// The app executable the wsp command runs as node, read off its text as `shimText` writes it: the
/// `ELECTRON_RUN_AS_NODE=1 exec <app> <cli>` line, or the daemon's `forward --wsp-argv <app> --wsp-argv <cli> --`.
pub fn shim_app(text: &str) -> Option<PathBuf> {
    let line = text.lines().find_map(|line| line.trim().strip_prefix("ELECTRON_RUN_AS_NODE=1 exec "))?;
    let words = shell_words(line)?;
    let app = match words.iter().position(|w| w == "--wsp-argv") {
        Some(at) => words.get(at + 1)?,
        None => words.first()?,
    };
    Path::new(app).is_absolute().then(|| PathBuf::from(app))
}

/// A line's words as `/bin/sh` reads `shellQuote`'s output: single quoted runs, `\'` and bare words; `"$@"` is a word.
fn shell_words(line: &str) -> Option<Vec<String>> {
    let (mut words, mut word, mut open) = (Vec::new(), String::new(), false);
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '\'' {
                        closed = true;
                        break;
                    }
                    word.push(c);
                }
                if !closed {
                    return None;
                }
                open = true;
            }
            '\\' => {
                word.push(chars.next()?);
                open = true;
            }
            c if c.is_whitespace() => {
                if open {
                    words.push(std::mem::take(&mut word));
                    open = false;
                }
            }
            c => {
                word.push(c);
                open = true;
            }
        }
    }
    if open {
        words.push(word);
    }
    Some(words)
}

/// Today's app's executable as `<home>/bin/wsp` names it, where it names one that stands.
pub fn named_app(home: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(home.join("bin").join("wsp")).ok()?;
    shim_app(&text).filter(|app| app.exists())
}

/// The `.app` bundle holding a Mac executable, `/Applications/wsp.app` for `/Applications/wsp.app/Contents/MacOS/wsp`.
pub fn bundle_of(exe: &Path) -> Option<&Path> {
    exe.ancestors().find(|dir| dir.extension().is_some_and(|e| e == "app"))
}

/// What the opened app runs with (`swapEnv`): the system's own PATH, and of this launch's environment only whose login
/// and which wsp home it is, with what a Linux app needs to reach the person's display and session bus.
pub fn back_env(vars: impl IntoIterator<Item = (OsString, OsString)>) -> Vec<(OsString, OsString)> {
    const KEPT: [&str; 10] = [
        "HOME",
        "USER",
        "LOGNAME",
        "TMPDIR",
        "LANG",
        "WSP_HOME",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
    ];
    let mut env = vec![(OsString::from("PATH"), OsString::from("/usr/bin:/bin:/usr/sbin:/sbin"))];
    env.extend(vars.into_iter().filter(|(name, _)| KEPT.iter().any(|kept| name == kept)));
    env
}

/// Opens today's app.
pub trait Opener {
    fn open(&self, app: &Path) -> io::Result<()>;
}

/// The system's way: `/usr/bin/open` on the bundle on a Mac, as the swap opens it; elsewhere the executable started
/// in its own session with its input closed, so this app quitting takes nothing of it.
pub struct SystemOpener;

impl Opener for SystemOpener {
    fn open(&self, app: &Path) -> io::Result<()> {
        let env = back_env(std::env::vars_os());
        if cfg!(target_os = "macos") {
            let bundle = bundle_of(app).ok_or_else(|| io::Error::other(format!("{} is in no app bundle", app.display())))?;
            let status = open_command(bundle, env).status()?;
            return if status.success() { Ok(()) } else { Err(io::Error::other(format!("open exited with {status}"))) };
        }
        detach(app, env)
    }
}

/// `/usr/bin/open -a` on a Mac bundle, as `gpuiLaunch` opens this app: the opener hands the app none of its own
/// environment, so a named wsp home rides its `--env`.
pub fn open_command(bundle: &Path, env: Vec<(OsString, OsString)>) -> std::process::Command {
    let mut open = std::process::Command::new("/usr/bin/open");
    if let Some((_, home)) = env.iter().find(|(name, home)| name == "WSP_HOME" && !home.is_empty()) {
        let mut pair = OsString::from("WSP_HOME=");
        pair.push(home);
        open.arg("--env").arg(pair);
    }
    open.env_clear().envs(env).arg("-a").arg(bundle);
    open
}

#[cfg(unix)]
fn detach(app: &Path, env: Vec<(OsString, OsString)>) -> io::Result<()> {
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command, Stdio};
    let mut command = Command::new(app);
    command.env_clear().envs(env).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // SAFETY: setsid is async-signal-safe and touches nothing of the parent's.
    unsafe {
        command.pre_exec(|| if libc::setsid() == -1 { Err(io::Error::last_os_error()) } else { Ok(()) });
    }
    command.spawn().map(drop)
}

#[cfg(not(unix))]
fn detach(app: &Path, env: Vec<(OsString, OsString)>) -> io::Result<()> {
    std::process::Command::new(app).env_clear().envs(env).spawn().map(drop)
}

/// The way back: the switch file goes first, so the app it opens stays, then today's app opens. One that did not open
/// puts the switch back, as this app stays (`turnExperimental`).
pub fn go_back(home: &Path, app: &Path, opener: &dyn Opener) -> io::Result<()> {
    let stood = match std::fs::remove_file(switch_file(home)) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(e),
    };
    opener.open(app).inspect_err(|_| {
        if stood {
            use std::os::unix::fs::OpenOptionsExt;
            _ = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(switch_file(home));
        }
    })
}

/// The home this run serves and today's app it would go back to, for the menu row.
#[derive(Clone, Debug)]
pub struct Back {
    pub home: PathBuf,
    pub app: PathBuf,
}

impl Global for Back {}

/// Keeps the way back where the wsp command names today's app; the menu shows its row only then.
pub fn init(home: &Path, cx: &mut App) {
    if let Some(app) = named_app(home) {
        cx.set_global(Back { home: home.to_owned(), app });
    }
}

/// The way back from the menu, run off the main thread since a Mac's `open` waits on Launch Services: quits once
/// today's app opened, else says why in a box and stays.
pub fn back(cx: &mut App) {
    let Some(Back { home, app }) = cx.try_global::<Back>().cloned() else { return };
    let opened = cx.background_spawn(async move { go_back(&home, &app, &SystemOpener) });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let went = opened.await;
        cx.update(|cx| match went {
            Ok(()) => cx.quit(),
            Err(e) => {
                eprintln!("desktop-gpui: {NOT_OPENED}: {e}");
                let Some(window) = cx.active_window().or_else(|| cx.windows().first().copied()) else { return };
                let said = e.to_string();
                _ = window.update(cx, |_, window, cx| {
                    let answer = window.prompt(PromptLevel::Critical, NOT_OPENED, Some(&said), &["OK"], cx);
                    cx.background_spawn(answer).detach();
                });
            }
        })
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shim_names_the_app_in_either_form() {
        let plain = "#!/bin/sh\n# The wsp app writes this on each launch; it runs the wsp the app bundles.\nELECTRON_RUN_AS_NODE=1 exec '/Applications/wsp.app/Contents/MacOS/wsp' '/Applications/wsp.app/Contents/Resources/app.asar.unpacked/cli.mjs' \"$@\"\n";
        assert_eq!(shim_app(plain), Some(PathBuf::from("/Applications/wsp.app/Contents/MacOS/wsp")));
        let forward = "#!/bin/sh\nELECTRON_RUN_AS_NODE=1 exec '/home/a/.wsp/app/0.3.8-1a2b3c4d/resources/wsp-daemon' forward --wsp-argv '/home/a/.wsp/app/0.3.8-1a2b3c4d/wsp' --wsp-argv '/home/a/.wsp/app/0.3.8-1a2b3c4d/resources/cli.mjs' -- \"$@\"\n";
        assert_eq!(shim_app(forward), Some(PathBuf::from("/home/a/.wsp/app/0.3.8-1a2b3c4d/wsp")));
    }

    #[test]
    fn quoted_paths_read_as_the_shell_reads_them() {
        let spaced = "ELECTRON_RUN_AS_NODE=1 exec '/Users/a b/Apps/wsp.app/Contents/MacOS/wsp' '/x/cli.mjs' \"$@\"";
        assert_eq!(shim_app(spaced), Some(PathBuf::from("/Users/a b/Apps/wsp.app/Contents/MacOS/wsp")));
        let quote = "ELECTRON_RUN_AS_NODE=1 exec '/opt/it'\\''s/wsp' '/x/cli.mjs' \"$@\"";
        assert_eq!(shim_app(quote), Some(PathBuf::from("/opt/it's/wsp")));
        let forward = "ELECTRON_RUN_AS_NODE=1 exec '/d/wsp daemon' forward --wsp-argv '/o p/wsp' --wsp-argv '/c.mjs' -- \"$@\"";
        assert_eq!(shim_app(forward), Some(PathBuf::from("/o p/wsp")));
    }

    #[test]
    fn a_shim_naming_nothing_names_no_app() {
        assert_eq!(shim_app(""), None);
        assert_eq!(shim_app("#!/bin/sh\nexec node /x/cli.mjs \"$@\"\n"), None);
        assert_eq!(shim_app("ELECTRON_RUN_AS_NODE=1 exec \"$@\""), None, "\"$@\" is no path");
        assert_eq!(shim_app("ELECTRON_RUN_AS_NODE=1 exec 'wsp' cli.mjs"), None, "a bare name is no app");
        assert_eq!(shim_app("ELECTRON_RUN_AS_NODE=1 exec '/d/wsp-daemon' forward --wsp-argv"), None);
        assert_eq!(shim_app("ELECTRON_RUN_AS_NODE=1 exec '/a/wsp"), None, "an unclosed quote is no line sh runs");
    }

    #[test]
    fn an_appimage_is_opened_by_its_own_file() {
        assert_eq!(own_path(Some(OsString::from("/home/a/wsp.AppImage"))).unwrap(), PathBuf::from("/home/a/wsp.AppImage"));
        assert_eq!(own_path(None).unwrap(), std::env::current_exe().unwrap());
        assert_eq!(own_path(Some(OsString::new())).unwrap(), std::env::current_exe().unwrap(), "an empty APPIMAGE names none");
    }

    #[test]
    fn a_mac_executable_opens_by_its_bundle() {
        assert_eq!(bundle_of(Path::new("/Applications/wsp.app/Contents/MacOS/wsp")), Some(Path::new("/Applications/wsp.app")));
        assert_eq!(bundle_of(Path::new("/opt/wsp/wsp")), None);
    }

    #[test]
    fn a_mac_open_carries_the_named_home_to_the_app() {
        let args = |vars: Vec<(OsString, OsString)>| -> Vec<String> {
            let open = open_command(Path::new("/Applications/wsp.app"), vars);
            assert_eq!(open.get_program(), "/usr/bin/open");
            open.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
        };
        let named = vec![(OsString::from("HOME"), OsString::from("/u")), (OsString::from("WSP_HOME"), OsString::from("/w h"))];
        assert_eq!(args(named), ["--env", "WSP_HOME=/w h", "-a", "/Applications/wsp.app"]);
        let empty = vec![(OsString::from("WSP_HOME"), OsString::new())];
        assert_eq!(args(empty), ["-a", "/Applications/wsp.app"], "an empty WSP_HOME names none");
        assert_eq!(args(Vec::new()), ["-a", "/Applications/wsp.app"]);
    }

    #[test]
    fn the_opened_app_keeps_only_whose_login_and_home_it_is() {
        let vars = ["PATH", "HOME", "WSP_HOME", "DISPLAY", "ELECTRON_RUN_AS_NODE", "SECRET_TOKEN", "XDG_RUNTIME_DIR", "LD_PRELOAD"]
            .map(|name| (OsString::from(name), OsString::from("x")));
        let names: Vec<_> = back_env(vars).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["PATH", "HOME", "WSP_HOME", "DISPLAY", "XDG_RUNTIME_DIR"]);
        assert_eq!(back_env([]), [(OsString::from("PATH"), OsString::from("/usr/bin:/bin:/usr/sbin:/sbin"))], "the system's own PATH");
    }
}
