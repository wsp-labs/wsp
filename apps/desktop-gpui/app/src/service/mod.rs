// SPDX-License-Identifier: AGPL-3.0-only
//! The host this app carries and runs as this computer's service, as today's app runs it on Electron's own node
//! (`desktop:main.ts` `installCommand`, `desktop:host-lifecycle.ts` `ensureService`): the bundle holds a Node, the
//! host's built command and the daemon; the wsp command (`<home>/bin/wsp`) runs that command on that Node behind the
//! daemon's forwarder; and the unit the host's own `wsp up --service` writes runs the wsp command. Releases decide the
//! rest, as `desktop:host-lifecycle.ts` `openHost` reads them: a host or a wsp command of a newer release than the
//! bundle's is left as it is and opened on; one of an older release is taken over, the command written to run this
//! bundle and the service host restarted onto it by `restart_host` (an update's swap, or the open that refused it);
//! one of the same release has the command written where it names other files, and its host restarted before the
//! open only where the files it ran went. A wsp command today's app wrote, naming an
//! app of this release that stands, is that app's: it and its service are left alone.

mod appimage;
pub mod npm;
pub mod shim;

use std::cmp::Ordering;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use wsp_host_client::start::{self, Start, System};
use wsp_host_client::{release_order, Env};

use crate::desktop::{log, switch};

pub use appimage::Image;

/// The three files the bundle carries for the host, at the paths the packaging puts them (`scripts/package.sh`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bundled {
    /// The Node the host runs on, beside the app's executable.
    pub node: PathBuf,
    /// The host's built command, the one today's app bundles (`desktop:cli.ts`), with its chunks beside it.
    pub command: PathBuf,
    /// This computer's wsp-daemon, among the host's assets as the host's own asset table reads them.
    pub daemon: PathBuf,
}

/// The two bundle shapes the packaging builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    /// `<name>.app/Contents/MacOS/<exe>`, the host under `Contents/Resources`.
    Mac,
    /// An AppImage's `usr/bin/<exe>`, the host under `usr/lib/wsp`.
    Linux,
}

impl Os {
    pub fn here() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        }
    }
}

/// The daemon's target triple for a chip, as `DAEMON_TARGETS` names it: musl on Linux, so one file runs on any.
pub fn daemon_triple(os: Os, arch: &str) -> String {
    match os {
        Os::Mac => format!("{arch}-apple-darwin"),
        Os::Linux => format!("{arch}-unknown-linux-musl"),
    }
}

impl Bundled {
    /// Where a bundle of `os` keeps the host for an executable at `exe`, laid out as today's app lays out its own
    /// (`app/main/cli.mjs`, `app/assets/daemon/<triple>/wsp-daemon`) under the bundle's resources.
    pub fn beside(exe: &Path, os: Os, arch: &str) -> Bundled {
        let bin = exe.parent().unwrap_or(Path::new(""));
        let contents = bin.parent().unwrap_or(Path::new(""));
        let resources = match os {
            Os::Mac => contents.join("Resources"),
            Os::Linux => contents.join("lib").join("wsp"),
        };
        let app = resources.join("app");
        Bundled {
            node: bin.join("node"),
            command: app.join("main").join("cli.mjs"),
            daemon: app.join("assets").join("daemon").join(daemon_triple(os, arch)).join("wsp-daemon"),
        }
    }

    /// The same files as they stand once `from` is copied to `to`; nothing where a file is not under `from`.
    fn moved(&self, from: &Path, to: &Path) -> Option<Bundled> {
        let moved = |path: &Path| Some(to.join(path.strip_prefix(from).ok()?));
        Some(Bundled { node: moved(&self.node)?, command: moved(&self.command)?, daemon: moved(&self.daemon)? })
    }

    /// The release the bundled host is, off its own `package.json`.
    pub fn release(&self) -> Option<String> {
        shim::release_of(&self.command)
    }

    fn paths(&self) -> [&Path; 3] {
        [&self.node, &self.command, &self.daemon]
    }

    /// The first of the three that is not there, which a build carrying no host lacks.
    fn missing(&self) -> Option<&Path> {
        self.paths().into_iter().find(|path| !path.is_file())
    }
}

/// The host this running app carries: the bundled Node, command and daemon, at absolute paths, refused in words where
/// a file is not there, as in a build out of cargo.
pub fn bundled() -> Result<Bundled, String> {
    let exe =
        std::env::current_exe().and_then(std::path::absolute).map_err(|e| format!("this app could not read where it runs from: {e}"))?;
    let here = Bundled::beside(&exe, Os::here(), std::env::consts::ARCH);
    match here.missing() {
        Some(path) => Err(format!("this build carries no host of its own: {} is not there", path.display())),
        None => Ok(here),
    }
}

/// Where a release stands against this bundle's (`openHost`'s `compareVersions(theirs, VERSION)`): the one reading
/// the launch, an older host's restart and an update's swap all take, so a newer host is never restarted onto this
/// bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Against {
    Older,
    Same,
    Newer,
}

impl Against {
    pub fn of(theirs: &str, mine: &str) -> Against {
        match release_order(theirs, mine) {
            Ordering::Less => Against::Older,
            Ordering::Equal => Against::Same,
            Ordering::Greater => Against::Newer,
        }
    }
}

/// Why the host serving is to be restarted onto the wsp command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restart {
    /// It is of an older release, which the open refuses: restarted by `restart_host` once its turns are counted or
    /// the person said so (`replaceHost`), or by the update's swap.
    Older,
    /// It is of this release on files the command named before and that went, a move or a trashed copy: restarted
    /// before the open (`staleShim`).
    FilesWent,
}

/// The wsp command this app owns, and why the host serving is to be restarted onto it, where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owned {
    pub wsp: PathBuf,
    pub restart: Option<Restart>,
}

/// Why a bundle of release `mine` leaves the wsp command of the home `home` and its service as they are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Left {
    /// `theirs`, the release serving the home or the one the command runs, as `what` says, is newer.
    Newer { theirs: String, what: &'static str },
    /// The command runs today's app at this path, of no older release, which stands.
    Today(PathBuf),
}

impl Left {
    /// Why, in words a person reads, for a bundle of release `mine`.
    pub fn said(&self, mine: &str) -> String {
        match self {
            Left::Newer { theirs, what } => format!("wsp {theirs} {what}, newer than wsp {mine}"),
            Left::Today(app) => format!("the wsp command on this computer runs today's wsp app at {}", app.display()),
        }
    }
}

/// Whether `claim` leaves the wsp command of `home` and its service as they are for a bundle of release `mine`, with
/// `serving` the release the host serving names: a host, or a command's files, of a newer release, or today's app's
/// command naming an app of no older release that stands. The update reads it before a swap starts.
pub fn left_as_is(home: &Path, serving: Option<&str>, mine: &str) -> Option<Left> {
    let was = std::fs::read_to_string(shim::path(home)).ok();
    let named = was.as_deref().and_then(shim::command_of).and_then(|command| shim::release_of(&command));
    let against = |theirs: &str| Against::of(theirs, mine);
    let newer = [(serving, "serves this home"), (named.as_deref(), "is what the wsp command runs")]
        .into_iter()
        .find_map(|(theirs, what)| theirs.filter(|theirs| against(theirs) == Against::Newer).map(|theirs| (theirs, what)));
    if let Some((theirs, what)) = newer {
        return Some(Left::Newer { theirs: theirs.to_owned(), what });
    }
    let older = named.as_deref().is_some_and(|theirs| against(theirs) == Against::Older);
    switch::named_app(home).filter(|_| !older).map(Left::Today)
}

/// What a launch makes of the wsp command for the home `home`, with `serving` the release the host serving names.
/// A host, or a command's files, of a newer release than the bundle's stays as it is (none); so does today's app's
/// command naming an app of this release that stands. Otherwise the command is written to run `here`, or under an
/// AppImage a copy of it that outlives the launch, and kept where it already says exactly that; the host serving is to
/// be restarted onto it where it is of an older release, or of this one with the files it ran gone. The refusal is a
/// line for the app's log.
pub fn claim(home: &Path, here: Result<Bundled, String>, image: Option<&Image>, serving: Option<&str>) -> Result<Option<Owned>, String> {
    let wsp = shim::path(home);
    let here = here?;
    let mine = here.release().ok_or_else(|| format!("this build's host names no release: {} has none", here.command.display()))?;
    match left_as_is(home, serving, &mine) {
        Some(Left::Newer { theirs, what }) => {
            log::info(&format!(
                "wsp {theirs} {what}, newer than this app's {mine}, so this app leaves the wsp command at {} and the service as they are",
                wsp.display()
            ));
            return Ok(None);
        }
        Some(Left::Today(app)) => {
            log::info(&format!(
                "the wsp command at {} runs today's wsp app at {}, so this app leaves it and that app's service as they are",
                wsp.display(),
                app.display()
            ));
            return Ok(None);
        }
        None => {}
    }
    let was = std::fs::read_to_string(&wsp).ok();
    let named = was.as_deref().and_then(shim::command_of).and_then(|command| shim::release_of(&command));
    let against = |theirs: &str| Against::of(theirs, &mine);
    let done = install_command(home, &wsp, here, image, &mine, was.as_deref())?;
    let restart = match serving.map(against) {
        Some(Against::Older) => Some(Restart::Older),
        Some(Against::Same) if done == shim::Done::Written && named.is_none() => Some(Restart::FilesWent),
        _ => None,
    };
    Ok(Some(Owned { wsp, restart }))
}

/// The wsp command at `wsp` written to run `here`, or under an AppImage a copy of it that outlives the launch, and
/// kept where it already says exactly that.
fn install_command(
    home: &Path,
    wsp: &Path,
    here: Bundled,
    image: Option<&Image>,
    mine: &str,
    was: Option<&str>,
) -> Result<shim::Done, String> {
    let target = match image {
        Some(image) => appimage::keep(&here, image, home, mine).map_err(|e| format!("the host's files could not be kept: {e}"))?,
        None => here,
    };
    if let Some(path) = target.paths().into_iter().find(|path| !path.is_absolute()) {
        return Err(format!("the wsp command was not written at {}: {} is not an absolute path", wsp.display(), path.display()));
    }
    let done = shim::install(wsp, &shim::text(&target), was)
        .map_err(|e| format!("the wsp command could not be written at {}: {e}", wsp.display()))?;
    log::info(&format!("wsp command {} at {}", done.word(), wsp.display()));
    Ok(done)
}

/// The wsp command of `home` written to run `here` whatever release it or the host serving names, for a swap that put
/// this app back after the new app had moved the host onto its own files: those files are the failed update's, so
/// claim's newer-host rule does not hold (Electron's swap script restarts the service back whenever it restarted it).
pub fn take_back(home: &Path, here: Result<Bundled, String>, image: Option<&Image>) -> Result<PathBuf, String> {
    let here = here?;
    let mine = here.release().ok_or_else(|| format!("this build's host names no release: {} has none", here.command.display()))?;
    let wsp = shim::path(home);
    let was = std::fs::read_to_string(&wsp).ok();
    install_command(home, &wsp, here, image, &mine, was.as_deref())?;
    Ok(wsp)
}

/// What `restart_host` did with the host serving: restarted it onto this bundle, or left it as it is, with why in words
/// a person reads. A stand-in until the answer `restart_host` gives of its own lands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Followed {
    Restarted,
    Left(String),
}

/// The launch's state and environment, which a restart after an update reads.
static LAUNCH: OnceLock<(PathBuf, Env)> = OnceLock::new();
/// The wsp command this launch owns, which every later start of the host hands the unit.
static OWNED: OnceLock<PathBuf> = OnceLock::new();

/// `start::serving_release` from any thread: a runtime of its own on a thread of its own, so a caller on the app's
/// runtime, or on none, blocks nothing of it.
pub fn serving_now(state: &Path) -> Option<String> {
    std::thread::scope(|scope| {
        let read = scope
            .spawn(|| tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?.block_on(start::serving_release(state)));
        read.join().ok().flatten()
    })
}

/// `claim` for this launch, on its state file's home and the release serving it, with what it said in the app's log.
/// Where it owns the command, npm's `wsp` links on the login PATH of an older release are pointed at it
/// (`replaceOlderOnPath`), off the open's road since the login shell may take seconds.
pub fn on_launch(state: &Path, env: &Env) -> Option<Owned> {
    let _ = LAUNCH.set((state.to_owned(), env.clone()));
    let home = state.parent().unwrap_or(Path::new(""));
    let here = bundled();
    let mine = here.as_ref().ok().and_then(Bundled::release);
    match claim(home, here, Image::of(env).as_ref(), serving_now(state).as_deref()) {
        Ok(owned) => {
            if let (Some(owned), Some(mine)) = (&owned, mine) {
                let _ = OWNED.set(owned.wsp.clone());
                let (home, vars) = (home.to_owned(), env.clone());
                std::thread::spawn(move || {
                    for line in npm::replace_older_on_path(&home, || npm::login_path(&vars), &mine, &npm::own_home()) {
                        log::info(&line);
                    }
                });
            }
            owned
        }
        Err(why) => {
            log::info(&why);
            None
        }
    }
}

/// The wsp command this launch owns, for a start of the host after the launch's.
pub fn owned() -> Option<PathBuf> {
    OWNED.get().cloned()
}

/// Restarts the service host onto this bundle, for an update that swapped the bundle under it or an older host the
/// open refused: the wsp command is written again to name this bundle's files, and the host serving the launch's
/// state file is asked to restart on the road it came up on, where `claim` reads it as older or on files that went.
/// Nothing serving, or a host of this release on files that stand, is nothing to restart; a host of a newer release,
/// a wsp command today's app owns and a unit running another wsp are left as they are. The refusal is in words a
/// person reads.
pub async fn restart_host() -> Result<Followed, String> {
    let (state, env) = LAUNCH.get().cloned().unwrap_or_else(|| (default_state(), std::env::vars().collect()));
    let home = state.parent().unwrap_or(Path::new(""));
    let serving = start::serving_release(&state).await;
    let mine = bundled().ok().and_then(|here| here.release()).unwrap_or_default();
    let Some(owned) = claim(home, bundled(), Image::of(&env).as_ref(), serving.as_deref())? else {
        let left = left_as_is(home, serving.as_deref(), &mine).map(|left| left.said(&mine));
        return Ok(Followed::Left(
            left.unwrap_or_else(|| format!("this app left the wsp command at {} as it is", shim::path(home).display())),
        ));
    };
    if owned.restart.is_none() {
        return Ok(Followed::Left(match serving {
            Some(theirs) => format!("wsp {theirs} already serves {} on files that stand", state.display()),
            None => format!("no host serving {} said its release", state.display()),
        }));
    }
    let mut road = Start::here(&state, &env);
    road.wsp = Some(owned.wsp);
    road.restart = true;
    start::restart_service(&road, &System::new(&env)).await.map(|()| Followed::Restarted)
}

/// `take_back` for this launch, then the service host restarted onto the command as it now stands.
pub async fn restart_back() -> Result<(), String> {
    let (state, env) = LAUNCH.get().cloned().unwrap_or_else(|| (default_state(), std::env::vars().collect()));
    let home = state.parent().unwrap_or(Path::new(""));
    let wsp = take_back(home, bundled(), Image::of(&env).as_ref())?;
    let mut road = Start::here(&state, &env);
    road.wsp = Some(wsp);
    road.restart = true;
    start::restart_service(&road, &System::new(&env)).await
}

/// The state file a launch means, as the Electron app picks it: WSP_HOME names the home, made absolute against the
/// current folder, else ~/.wsp. An empty WSP_HOME names no home.
pub fn default_state() -> PathBuf {
    let home = std::env::var_os("WSP_HOME")
        .filter(|home| !home.is_empty())
        .map(|home| resolved(Path::new(&home), &std::env::current_dir().unwrap_or_default()))
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".wsp"));
    home.join("state.json")
}

/// A path made absolute against `cwd` and tidied by its spelling alone, links left as they are (node's `path.resolve`).
pub fn resolved(path: &Path, cwd: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in cwd.join(path).components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            part => out.push(part),
        }
    }
    out
}

#[cfg(test)]
mod tests;
