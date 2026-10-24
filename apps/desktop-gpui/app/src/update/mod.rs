// SPDX-License-Identifier: AGPL-3.0-only
//! Where this app stands with a newer release and the one act that moves it on (`shell/update.ts`), read by the
//! sidebar's update card and General's Version card alike, and the road that gets it (`bundleShell` in
//! `get-bundle.ts`). The host reads the newest release; an installed app that can replace itself downloads it in the
//! background unless the person dismissed it, checks it, and offers Restart, which swaps it in and brings the host
//! and daemon over with it. Any other app downloads the Mac's disk image or the AppImage into Downloads and offers
//! Quit and open.

pub mod feed;
pub mod follow;
pub mod stage;
pub mod swap;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gpui_kit::{App, BorrowAppContext as _, Global};
use serde_json::Value;

use crate::screens::settings::pages::about::compare_versions;
use feed::{Feed, Kept, Said};
use stage::{Installed, Kind, Runner, Staged};

/// `UPDATE_WORDS` in `shell/update.ts`, with the hovers over Get (`get-bundle.ts`).
pub mod words {
    pub fn out(version: &str) -> String {
        format!("wsp {version} is out")
    }
    pub fn ready(version: &str) -> String {
        format!("wsp {version} is ready")
    }
    pub const WHATS_NEW: &str = "What's new";
    pub const DISMISS: &str = "Dismiss";
    pub const GET: &str = "Get";
    pub const DOWNLOADING: &str = "Downloading";
    pub const QUIT_AND_OPEN: &str = "Quit and open";
    pub const RESTART: &str = "Restart";
    pub fn keep_running(running: usize) -> String {
        let who = if running == 1 { "Your running thread carries".to_owned() } else { format!("Your {running} running threads carry") };
        format!("{who} on. Open terminals close.")
    }
    pub fn not_ready(version: &str, said: &str) -> String {
        format!("wsp {version} was not downloaded: {said}")
    }
    pub fn not_restarted(said: &str) -> String {
        format!("wsp did not restart: {said}")
    }
    pub const MAC_HOVER: &str =
        "Downloads the disk image and checks its sha256. Unsigned builds need Privacy & Security, Open Anyway, once.";
    pub const LINUX_HOVER: &str = "Downloads the AppImage into Downloads and checks its sha256. The app you run is not replaced.";
    pub const IN_PLACE_HOVER: &str = "Downloads the update, checks its sha256 and its signature, and installs it when the app restarts.";
}

/// The act a release waits on (`UpdateAct`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Link,
    Get,
    Downloading,
    Open,
    Restart,
    RestartHost,
}

/// Whether the release is on this computer already and only a restart or an open is left (`updateReady`).
pub fn ready(act: Act) -> bool {
    matches!(act, Act::Open | Act::Restart | Act::RestartHost)
}

/// The version the act brings, the page holding its notes, and the act.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    pub version: String,
    pub url: String,
    pub act: Act,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Downloading,
    Kept,
}

/// The shell's download of one release, shared by every card so a Get pressed on one shows Downloading on the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub version: String,
    pub phase: Phase,
}

/// `restartShown`: newer files installed under the host, a restart brings it back, and the host runs here.
pub fn restart_shown(release: &Value, here: bool) -> bool {
    release.get("installed").and_then(Value::as_str).is_some() && release.get("restartRefusal").is_none_or(Value::is_null) && here
}

/// `updateStage`: newer files installed under the host first, since a restart is all they need; then a release this
/// app or its host is behind. The app's own road counts only where the app itself is behind and the page is its own
/// host's, since a host that lags alone is updated its own way.
pub fn update_stage(
    release: Option<&Value>,
    app: &str,
    host: Option<&str>,
    here: bool,
    in_place: Option<bool>,
    download: Option<&Download>,
) -> Option<Stage> {
    let release = release?;
    let url = release.pointer("/latest/url").and_then(Value::as_str).unwrap_or(feed::RELEASES).to_owned();
    if let Some(installed) = release.get("installed").and_then(Value::as_str).filter(|_| restart_shown(release, here)) {
        return Some(Stage { version: installed.to_owned(), url, act: Act::RestartHost });
    }
    let latest = release.pointer("/latest/version").and_then(Value::as_str)?;
    let above = |running: &str| compare_versions(latest, running) == std::cmp::Ordering::Greater;
    let app = Some(app).filter(|a| !a.is_empty());
    if !(host.is_some_and(above) || app.is_some_and(above)) {
        return None;
    }
    let at = |act| Some(Stage { version: latest.to_owned(), url: url.clone(), act });
    let Some(in_place) = in_place.filter(|_| here && app.is_some_and(above)) else { return at(Act::Link) };
    match download.filter(|d| d.version == latest) {
        None => at(Act::Get),
        Some(Download { phase: Phase::Downloading, .. }) => at(Act::Downloading),
        Some(_) => at(if in_place { Act::Restart } else { Act::Open }),
    }
}

/// What the shell needs to get and open a release.
pub struct ShellDeps {
    pub feed: Feed,
    /// Where a download the person opens lands: their Downloads folder.
    pub downloads: PathBuf,
    /// This app's own version: only a release above it, and never a prerelease, is fetched.
    pub running: String,
    /// The installed app that replaces itself; absent, the disk image and AppImage road into Downloads.
    pub installed: Option<Installed>,
    /// The bundle this platform takes into Downloads, or nothing where none is built for it.
    pub road: Option<Road>,
    pub run: Arc<Runner>,
    pub uid: u32,
    pub pid: u32,
    pub log: PathBuf,
}

/// One platform's bundle into Downloads, and what makes it the person's next step once kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Road {
    /// Opening the disk image mounts it and opens its drag window.
    Mac,
    /// The AppImage is made runnable and shown in its folder.
    Linux,
}

impl Road {
    pub fn here() -> Option<Road> {
        if cfg!(target_os = "macos") {
            Some(Road::Mac)
        } else if cfg!(target_os = "linux") {
            Some(Road::Linux)
        } else {
            None
        }
    }

    fn asset(self, version: &str) -> String {
        let names = feed::bundle_names(version);
        match self {
            Road::Mac => names.mac,
            Road::Linux => names.app_image,
        }
    }
}

/// What an open asks of the app: quit now that a swap waits on it or the disk image opened, or show the AppImage in
/// its folder and quit.
#[derive(Debug, PartialEq)]
pub enum Opened {
    Swapping,
    Open(PathBuf),
    Reveal(PathBuf),
}

/// The road's state (`bundleShell`): open reaches only what a get kept, with the sum it matched when it was kept,
/// or the checked copy a swap starts from.
pub struct Shell {
    deps: ShellDeps,
    kept: Option<Kept>,
    staged: Option<(Staged, String, String)>,
}

impl Shell {
    pub fn new(deps: ShellDeps) -> Shell {
        Shell { deps, kept: None, staged: None }
    }

    pub fn in_place(&self) -> bool {
        self.deps.installed.is_some()
    }

    /// The release `version` fetched and checked: into a private folder beside the installed app, checked and staged
    /// where the app replaces itself, else into Downloads.
    pub fn get(&mut self, version: &str) -> Result<(), Said> {
        let Some(version) = feed::asked_version(version) else { return Err(Said::new(feed::words::NOT_A_VERSION)) };
        if version.contains('-') || compare_versions(version, &self.deps.running) != std::cmp::Ordering::Greater {
            return Err(Said::new(feed::words::not_newer(version)));
        }
        self.kept = None;
        let Some(installed) = self.deps.installed.clone() else {
            let road = self.deps.road.ok_or_else(|| Said::new(feed::words::no_bundle(std::env::consts::OS)))?;
            self.kept = Some(self.deps.feed.get(version, &self.deps.downloads, &road.asset(version))?);
            return Ok(());
        };
        if self.staged.as_ref().is_some_and(|(copy, v, _)| v == version && copy.app.exists()) {
            return Ok(());
        }
        if self.staged.take().is_some() {
            stage::discard(&installed.path);
        }
        let root = stage::stage_of(&installed.path);
        std::fs::create_dir_all(&root).map_err(|e| Said::new(e.to_string()))?;
        let got = self.deps.feed.get(version, &root, &installed.asset(version))?;
        let ready = stage::stage(&got.file, version, &installed, self.deps.run.as_ref(), self.deps.uid)?;
        self.staged = Some((ready, version.to_owned(), got.sum));
        Ok(())
    }

    /// The kept release opened: the swap started where the app replaces itself, else the kept file checked again and
    /// handed to the person's next step on this platform. Either way the app quits after.
    pub fn open(&mut self) -> Result<Opened, Said> {
        if let Some(installed) = self.deps.installed.clone() {
            let Some((copy, version, sum)) = self.staged.clone() else { return Err(Said::new(feed::words::NOTHING_KEPT)) };
            if !copy.app.exists() {
                self.staged = None;
                return Err(Said::new(feed::words::GONE));
            }
            let swap = swap::Swap {
                pid: self.deps.pid,
                installed: installed.path,
                kind: installed.kind,
                sum: if installed.kind == Kind::AppImage { sum } else { String::new() },
                staged: copy,
                version,
                log: self.deps.log.clone(),
                patience: swap::PATIENCE_S,
                grace: swap::GRACE_S,
            };
            swap::start(&swap, &swap::env(|n| std::env::var(n).ok())).map_err(|e| Said::new(e.to_string()))?;
            return Ok(Opened::Swapping);
        }
        let Some(Kept { file, sum }) = self.kept.clone() else { return Err(Said::new(feed::words::NOTHING_KEPT)) };
        if feed::sum_of(&file).as_deref() != Some(sum.as_str()) {
            self.kept = None;
            return Err(Said::new(feed::words::changed(&file.file_name().unwrap_or_default().to_string_lossy())));
        }
        match self.deps.road {
            Some(Road::Mac) => match (self.deps.run)("/usr/bin/open", &[&file.to_string_lossy()]) {
                (0, _) => Ok(Opened::Open(file)),
                (_, said) => Err(Said::new(said.trim().lines().next().unwrap_or("the disk image did not open").to_owned())),
            },
            Some(Road::Linux) => {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).map_err(|e| Said::new(e.to_string()))?;
                Ok(Opened::Reveal(file))
            }
            None => Err(Said::new(feed::words::no_bundle(std::env::consts::OS))),
        }
    }

    /// Deletes the checked copy an app that replaces itself holds, which dismissing the card asks for.
    pub fn discard(&mut self) {
        if let (Some(installed), Some(_)) = (&self.deps.installed, self.staged.take()) {
            stage::discard(&installed.path);
        }
    }
}

/// The app's update state, read by every card: the shared download, the road, and the releases already asked for in
/// the background this run.
pub struct Updates {
    pub download: Option<Download>,
    /// Whether the road is the app's own here: the installed app replacing itself, or Downloads; nothing where none.
    pub in_place: Option<bool>,
    /// Why this app cannot replace itself, said beside Get.
    pub why: Option<&'static str>,
    asked: std::collections::HashSet<String>,
    shell: Arc<Mutex<Shell>>,
}

impl Global for Updates {}

/// The installed app this process runs from, where it runs from one.
pub fn installed_here() -> Option<Installed> {
    Installed::of(&std::env::current_exe().ok()?, std::env::var_os("APPIMAGE").as_deref(), std::env::var_os("APPDIR").as_deref())
}

/// The road for this run: an installed app replacing itself where it can, else Downloads. A development build alone
/// takes the release base from the environment.
pub fn init(installed: Option<Installed>, logs: &Path, cx: &mut App) {
    let why = installed.as_ref().and_then(stage::in_place_refusal);
    let installed = installed.filter(|_| why.is_none());
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let api = if cfg!(debug_assertions) { std::env::var(feed::RELEASE_API_ENV).ok() } else { None };
    let running = wsp_host_client::HostClient::app_release();
    let road = Road::here();
    let deps = ShellDeps {
        feed: Feed::new(api, format!("wsp/{running}")),
        downloads: home.join("Downloads"),
        running,
        installed: installed.clone(),
        road,
        run: Arc::new(stage::command_runner),
        // SAFETY: getuid reads this process's user and cannot fail.
        uid: unsafe { libc::getuid() },
        pid: std::process::id(),
        log: logs.join("update.log"),
    };
    let in_place = if installed.is_some() { Some(true) } else { road.map(|_| false) };
    cx.set_global(Updates { download: None, in_place, why, asked: Default::default(), shell: Arc::new(Mutex::new(Shell::new(deps))) });
}

/// Read by a development build alone: the update it checked is opened as Restart would open it.
pub const RESTART_WHEN_READY_ENV: &str = "WSP_UPDATE_RESTART_WHEN_READY";

fn updates(cx: &App) -> Option<&Updates> {
    cx.try_global::<Updates>()
}

/// The stage for the release the host last read, as this app's cards show it.
pub fn stage(release: Option<&Value>, host: Option<&str>, here: bool, cx: &App) -> Option<Stage> {
    let app = wsp_host_client::HostClient::app_release();
    let u = updates(cx);
    update_stage(release, &app, host, here, u.and_then(|u| u.in_place), u.and_then(|u| u.download.as_ref()))
}

/// The words over Get on this road (`bundleHover`).
pub fn hover(cx: &App) -> Option<&'static str> {
    match updates(cx)?.in_place? {
        true => Some(words::IN_PLACE_HOVER),
        false => Road::here().map(|r| if r == Road::Mac { words::MAC_HOVER } else { words::LINUX_HOVER }),
    }
}

/// Why this app cannot replace itself, said beside Get (`updateWhy`).
pub fn why(cx: &App) -> Option<&'static str> {
    updates(cx)?.why
}

fn set_download(download: Option<Download>, cx: &mut App) {
    if cx.has_global::<Updates>() {
        cx.update_global::<Updates, _>(|u, _| u.download = download);
    }
}

/// `work` on a thread of its own, since a download or a host's restart blocks for as long as it takes.
pub fn off_main<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> futures::channel::oneshot::Receiver<T> {
    let (done, answer) = futures::channel::oneshot::channel();
    std::thread::spawn(move || _ = done.send(work()));
    answer
}

/// One step on the shell's road, off the main thread, its ending in the app log and handed back to `done`.
fn step<T: Send + 'static>(
    name: String,
    work: impl FnOnce(&mut Shell) -> Result<T, Said> + Send + 'static,
    done: impl FnOnce(Result<T, Said>, &mut App) + 'static,
    cx: &mut App,
) {
    let Some(shell) = updates(cx).map(|u| u.shell.clone()) else { return };
    crate::desktop::log::info(&format!("update: {name}"));
    let task = off_main(move || work(&mut shell.lock().unwrap_or_else(|e| e.into_inner())));
    cx.spawn(async move |cx| {
        let Ok(got) = task.await else { return };
        let line = feed::log_line(&name, &got.as_ref().map(drop).map_err(Clone::clone));
        if got.is_ok() {
            crate::desktop::log::info(&line);
        } else {
            crate::desktop::log::error(&line);
        }
        cx.update(|cx| done(got, cx));
    })
    .detach();
}

/// The shell's download of a release (`getUpdate`); a refusal puts Get back and says why.
pub fn get(version: &str, cx: &mut App) {
    let version = version.to_owned();
    set_download(Some(Download { version: version.clone(), phase: Phase::Downloading }), cx);
    let asked = version.clone();
    step(
        format!("get {version}"),
        move |shell| shell.get(&asked),
        move |got, cx| match got {
            Err(said) => {
                set_download(None, cx);
                crate::ui::notify(cx, crate::ui::Notice::error(words::not_ready(&version, &said.error)));
            }
            // Dismissed while it downloaded: the copy just checked goes as a ready one's would.
            Ok(()) if updates(cx).and_then(|u| u.download.as_ref()).is_none_or(|d| d.version != version) => discard(cx),
            Ok(()) => {
                set_download(Some(Download { version, phase: Phase::Kept }), cx);
                // A development build's live check, which no one is at the screen to press for, presses Restart itself.
                if cfg!(debug_assertions) && std::env::var_os(RESTART_WHEN_READY_ENV).is_some() {
                    open(cx);
                }
            }
        },
        cx,
    );
}

/// Opens what the download kept: the restart into the checked copy, or the disk image or AppImage, then quits.
fn open(cx: &mut App) {
    let in_place = updates(cx).and_then(|u| u.in_place) == Some(true);
    step(
        "open".into(),
        |shell| shell.open(),
        move |opened, cx| match opened {
            Ok(Opened::Swapping) => cx.quit(),
            Ok(Opened::Open(_)) => cx.quit(),
            Ok(Opened::Reveal(file)) => {
                cx.reveal_path(&file);
                cx.quit();
            }
            Err(said) => {
                set_download(None, cx);
                let text = if in_place { words::not_restarted(&said.error) } else { said.error };
                crate::ui::notify(cx, crate::ui::Notice::error(text));
            }
        },
        cx,
    );
}

fn discard(cx: &mut App) {
    step(
        "discard".into(),
        |shell| {
            shell.discard();
            Ok(())
        },
        |_, _| {},
        cx,
    );
}

/// The stage's act, pressed (`runUpdate`): Downloading presses nothing, and Restart host is the caller's, which holds
/// the host.
pub fn run(stage: &Stage, cx: &mut App) {
    match stage.act {
        Act::Link => cx.open_url(&stage.url),
        Act::Get if updates(cx).and_then(|u| u.in_place).is_some() => get(&stage.version, cx),
        Act::Get => cx.open_url(&stage.url),
        Act::Open | Act::Restart => open(cx),
        Act::Downloading | Act::RestartHost => {}
    }
}

/// The person is done with this release (`dismissUpdate`): the caller keeps the version in the host's preferences,
/// and a copy the shell checked is deleted.
pub fn dismiss(stage: &Stage, cx: &mut App) {
    if updates(cx).and_then(|u| u.in_place) != Some(true) || !matches!(stage.act, Act::Restart | Act::Downloading) {
        return;
    }
    set_download(None, cx);
    if stage.act == Act::Restart {
        discard(cx);
    }
}

/// `useUpdateInBackground`: on an app that replaces itself, a release it is behind is downloaded and checked in the
/// background, once per run and release, unless the person dismissed it. `dismissed` is nothing until the host's
/// preferences are read, so a dismissed release is never fetched on the first paint.
pub fn background(stage: Option<&Stage>, dismissed: Option<Option<&str>>, cx: &mut App) {
    let Some(stage) = stage.filter(|s| s.act == Act::Get) else { return };
    let Some(dismissed) = dismissed else { return };
    let Some(u) = updates(cx) else { return };
    if u.in_place != Some(true) || dismissed == Some(stage.version.as_str()) || u.asked.contains(&stage.version) {
        return;
    }
    let version = stage.version.clone();
    cx.update_global::<Updates, _>(|u, _| u.asked.insert(version.clone()));
    get(&version, cx);
}

/// What a launch does with what the stage said before it opens its window: the host follows this app onto its files
/// where a swap waits on it, then the swap is answered; it follows back where a swap put this app back. A swap starts
/// only from Restart, which the person pressed knowing the running threads carry on, so the host goes at once.
pub fn after_swap(settled: Option<swap::Settled>, version: &str, restart: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    match settled {
        Some(swap::Settled::Waiting(waiting)) => {
            waiting.restarting().map_err(|e| e.to_string())?;
            restart()?;
            waiting.up(version).map_err(|e| e.to_string())
        }
        Some(swap::Settled::Back { restart: true }) => restart(),
        _ => Ok(()),
    }
}

/// The service's restart onto this bundle, from a thread of its own; the launch's wsp command written first, which the
/// restart reads.
pub fn restart_host_blocking() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    runtime.block_on(crate::service::restart_host())
}

/// `after_swap` off the main thread, then `open`. A new app whose host did not follow exits before it is up, so the
/// swap puts the old app back.
pub fn launch(settled: Option<swap::Settled>, state: PathBuf, vars: wsp_host_client::Env, open: Box<dyn FnOnce(&mut App)>, cx: &mut App) {
    let follows = matches!(settled, Some(swap::Settled::Waiting(_)) | Some(swap::Settled::Back { restart: true }));
    if !follows {
        return open(cx);
    }
    let waiting = matches!(settled, Some(swap::Settled::Waiting(_)));
    let version = wsp_host_client::HostClient::app_release();
    crate::desktop::log::info(&format!("update: moving the host onto wsp {version}"));
    crate::screens::dialogs::startup::hold_opening(true, cx);
    let task = off_main(move || {
        crate::service::on_launch(&state, &vars);
        after_swap(settled, &version, restart_host_blocking)
    });
    cx.spawn(async move |cx| {
        let followed = task.await.unwrap_or_else(|_| Err("the restart stopped before it answered".into()));
        if let Err(said) = &followed {
            crate::desktop::log::error(&format!("update: the host did not follow: {said}"));
            if waiting {
                std::process::exit(1);
            }
        }
        cx.update(|cx| {
            crate::screens::dialogs::startup::hold_opening(false, cx);
            open(cx);
        });
    })
    .detach();
}
