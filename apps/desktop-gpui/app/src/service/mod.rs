// SPDX-License-Identifier: AGPL-3.0-only
//! The host this app carries and runs as this computer's service, as today's app runs it on Electron's own node
//! (`desktop:main.ts` `installCommand`, `desktop:host-lifecycle.ts` `ensureService`): the bundle holds a Node, the
//! host's built command and the daemon; the wsp command (`<home>/bin/wsp`) runs that command on that Node behind the
//! daemon's forwarder; and the unit the host's own `wsp up --service` writes runs the wsp command. Every launch
//! writes the command again where it names other files, and a service host still on the old ones is restarted.
//! A wsp command today's app wrote, naming an app that stands, is that app's: it and its service are left alone.

mod appimage;
pub mod shim;

use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use wsp_host_client::start::{self, Start, System};
use wsp_host_client::Env;

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

    /// The first of the three that is not there, which a build carrying no host lacks.
    fn missing(&self) -> Option<&Path> {
        [&self.node, &self.command, &self.daemon].into_iter().find(|path| !path.is_file()).map(PathBuf::as_path)
    }
}

/// The host this running app carries: the bundled Node, command and daemon, refused in words where a file is not
/// there, as in a build out of cargo.
pub fn bundled() -> Result<Bundled, String> {
    let exe = std::env::current_exe().map_err(|e| format!("this app could not read where it runs from: {e}"))?;
    let here = Bundled::beside(&exe, Os::here(), std::env::consts::ARCH);
    match here.missing() {
        Some(path) => Err(format!("this build carries no host of its own: {} is not there", path.display())),
        None => Ok(here),
    }
}

/// The wsp command this app owns, and whether this launch rewrote it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owned {
    pub wsp: PathBuf,
    pub rewrote: bool,
}

/// What a launch makes of the wsp command for the home `home`: today's app's, naming an app that stands, stays as it
/// is (none); else the command is written to run `here`, or under an AppImage a copy of it that outlives the launch,
/// and kept where it already says exactly that. The refusal is a line for the app's log.
pub fn claim(home: &Path, here: Result<Bundled, String>, image: Option<&Image>, release: &str) -> Result<Option<Owned>, String> {
    let wsp = shim::path(home);
    let here = here?;
    if let Some(app) = switch::named_app(home) {
        log::info(&format!(
            "the wsp command at {} runs today's wsp app at {}, so this app leaves it and that app's service as they are",
            wsp.display(),
            app.display()
        ));
        return Ok(None);
    }
    let target = match image {
        Some(image) => appimage::keep(&here, image, home, release).map_err(|e| format!("the host's files could not be kept: {e}"))?,
        None => here,
    };
    let done =
        shim::install(&wsp, &shim::text(&target)).map_err(|e| format!("the wsp command could not be written at {}: {e}", wsp.display()))?;
    log::info(&format!("wsp command {} at {}", done.word(), wsp.display()));
    Ok(Some(Owned { wsp, rewrote: done == shim::Done::Written }))
}

/// The launch's state and environment, which a restart after an update reads.
static LAUNCH: OnceLock<(PathBuf, Env)> = OnceLock::new();
/// The wsp command this launch owns, which every later start of the host hands the unit.
static OWNED: OnceLock<PathBuf> = OnceLock::new();

/// `claim` for this launch, on its state file's home, with what it said in the app's log.
pub fn on_launch(state: &Path, env: &Env) -> Option<Owned> {
    let _ = LAUNCH.set((state.to_owned(), env.clone()));
    let home = state.parent().unwrap_or(Path::new(""));
    match claim(home, bundled(), Image::of(env).as_ref(), env!("CARGO_PKG_VERSION")) {
        Ok(owned) => {
            if let Some(owned) = &owned {
                let _ = OWNED.set(owned.wsp.clone());
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

/// Restarts the service host onto this bundle, for an update that swapped the bundle under it: the wsp command is
/// written again to name this bundle's files, and the host serving the launch's state file is asked to restart on the
/// road it came up on. Nothing serving is nothing to restart, and a wsp command today's app owns leaves that app's
/// host alone. The refusal is in words a person reads.
pub async fn restart_host() -> Result<(), String> {
    let (state, env) = LAUNCH.get().cloned().unwrap_or_else(|| (default_state(), std::env::vars().collect()));
    let home = state.parent().unwrap_or(Path::new(""));
    let Some(owned) = claim(home, bundled(), Image::of(&env).as_ref(), env!("CARGO_PKG_VERSION"))? else { return Ok(()) };
    let mut road = Start::here(&state, &env);
    road.wsp = Some(owned.wsp);
    start::restart(&road, &System::new(&env)).await
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
