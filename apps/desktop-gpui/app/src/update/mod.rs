// SPDX-License-Identifier: AGPL-3.0-only
//! Where this app stands with a newer release and the one act that moves it on (`shell/update.ts`), read by the
//! sidebar's update card and General's Version card alike, and the road that gets it (`bundleShell` in
//! `get-bundle.ts`). The host reads the newest release; an installed app that can replace itself downloads it in the
//! background unless the person dismissed it, checks it, and offers Restart, which swaps it in and brings the host
//! and daemon over with it. Any other app downloads the Mac's disk image or the AppImage into Downloads and offers
//! Quit and open.

pub mod feed;
pub mod follow;
pub mod image;
pub mod stage;
pub mod swap;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_swap;
#[cfg(test)]
mod tests_today;

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
    pub fn newer_command(theirs: &str, version: &str) -> String {
        format!("the wsp command on this computer runs wsp {theirs}, newer than {version}, so the host would not follow it; nothing was changed")
    }
    pub const SWAPPING: &str = "an update is already being installed";
    pub fn no_tools(missing: &str) -> String {
        format!("this computer has no {missing} where the update looks for it, so nothing was changed")
    }
    pub fn refused_before(asset: &str) -> String {
        format!("{asset} was refused before as another app than this one, so it was not downloaded again")
    }
    pub fn not_updated(said: &str) -> String {
        format!("wsp did not update: {said}")
    }
    pub const UNFINISHED: &str = "The last update did not finish, so the app it replaced is kept.";
    pub const PUT_BACK: &str = "Put it back";
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
    /// The launch's state file, whose host the swap's new app opens on, and whose home's wsp command says which app's
    /// host serves it.
    pub state: PathBuf,
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
    /// How long the swap waits for this app to quit and then for the new one to be up.
    pub patience: u32,
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
        let asset = installed.asset(version);
        let want = self.deps.feed.want(version, &asset)?;
        let root = stage::stage_of(&installed.path);
        std::fs::create_dir_all(&root).map_err(|e| Said::new(e.to_string()))?;
        let refused = self.refused();
        if std::fs::read_to_string(&refused).is_ok_and(|text| text.lines().any(|sum| sum == want.sum)) {
            return Err(Said::new(words::refused_before(&asset)));
        }
        let got = self.deps.feed.fetch(version, &root, &asset, &want)?;
        let ready = stage::stage(&got.file, version, &installed, self.deps.run.as_ref(), self.deps.uid).inspect_err(|said| {
            if *said == stage::words::not_this_app(&asset) {
                remember(&refused, &got.sum);
            }
        })?;
        self.staged = Some((ready, version.to_owned(), got.sum));
        Ok(())
    }

    /// The sha256 of every release asset refused as another app, so it is not downloaded again on a later launch.
    fn refused(&self) -> PathBuf {
        self.deps.log.with_file_name("update-refused")
    }

    /// Why no swap can start now, where one cannot: one is under way, the host would not follow the new app of
    /// `version` onto its files (`claim` leaves the command as it is) while the host serving is older than it, so its
    /// open would be refused, or a Linux tool the swap runs is missing.
    fn refusal(&self, installed: &Installed, version: Option<&str>) -> Option<Said> {
        if swap::swapping(&stage::stage_of(&installed.path)).is_some() {
            return Some(Said::new(words::SWAPPING));
        }
        if let Some(version) = version {
            let home = self.deps.state.parent().unwrap_or(Path::new(""));
            let serving = crate::service::serving_now(&self.deps.state);
            let refused =
                serving.as_deref().is_none_or(|theirs| crate::service::Against::of(theirs, version) == crate::service::Against::Older);
            if let Some(left) = crate::service::left_as_is(home, serving.as_deref(), version).filter(|_| refused) {
                return Some(Said::new(words::newer_command(&left.theirs, version)));
            }
        }
        let missing = if installed.kind == Kind::AppImage { swap::missing_tools(swap::path()) } else { Vec::new() };
        (!missing.is_empty()).then(|| Said::new(words::no_tools(&missing.join(", "))))
    }

    /// The swap of `staged` in for the installed app, started; the app quits after.
    fn begin(&self, installed: Installed, staged: Staged, version: String, sum: String) -> Result<Opened, Said> {
        let swap = swap::Swap {
            pid: self.deps.pid,
            installed: installed.path,
            kind: installed.kind,
            sum: if installed.kind == Kind::AppImage { sum } else { String::new() },
            staged,
            version,
            log: self.deps.log.clone(),
            patience: self.deps.patience,
            grace: swap::GRACE_S,
        };
        swap::start(&swap, &swap::env(|n| std::env::var(n).ok())).map_err(|e| Said::new(e.to_string()))?;
        Ok(Opened::Swapping)
    }

    /// The app an unfinished swap replaced, swapped back in for the running one.
    pub fn put_back(&mut self) -> Result<Opened, Said> {
        let Some(installed) = self.deps.installed.clone() else { return Err(Said::new(feed::words::NOTHING_KEPT)) };
        if let Some(said) = self.refusal(&installed, None) {
            return Err(said);
        }
        let staged = stage::restage(&installed, self.deps.run.as_ref(), self.deps.uid)?;
        let sum = if installed.kind == Kind::AppImage { feed::sum_of(&staged.app).unwrap_or_default() } else { String::new() };
        self.begin(installed, staged, String::new(), sum)
    }

    /// The kept release opened: the swap started where the app replaces itself, else the kept file checked again and
    /// handed to the person's next step on this platform. Either way the app quits after.
    pub fn open(&mut self) -> Result<Opened, Said> {
        if let Some(installed) = self.deps.installed.clone() {
            if self.staged.as_ref().is_none_or(|(copy, ..)| !copy.app.exists()) {
                return Err(Said::new(if self.staged.take().is_some() { feed::words::GONE } else { feed::words::NOTHING_KEPT }));
            }
            let version = self.staged.as_ref().map(|(_, version, _)| version.clone());
            if let Some(said) = self.refusal(&installed, version.as_deref()) {
                return Err(said);
            }
            let (copy, version, sum) = self.staged.take().expect("a checked copy, just looked at");
            return self.begin(installed, copy, version, sum);
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

/// Adds `sum` to the refused list at `file`, which keeps the newest few.
fn remember(file: &Path, sum: &str) {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let mut sums: Vec<&str> = text.lines().filter(|l| !l.is_empty() && *l != sum).collect();
    sums.push(sum);
    let from = sums.len().saturating_sub(16);
    _ = std::fs::write(file, sums[from..].iter().map(|s| format!("{s}\n")).collect::<String>());
}

/// The road for this run: an installed app replacing itself where it can, else Downloads. A development build alone
/// takes the release base from the environment. `state` is the launch's state file.
pub fn init(installed: Option<Installed>, state: &Path, logs: &Path, cx: &mut App) {
    let why = installed.as_ref().and_then(stage::in_place_refusal);
    let installed = installed.filter(|_| why.is_none());
    let user = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let api = if cfg!(debug_assertions) { std::env::var(feed::RELEASE_API_ENV).ok() } else { None };
    let running = wsp_host_client::HostClient::app_release();
    let road = Road::here();
    let deps = ShellDeps {
        feed: Feed::new(api, format!("wsp/{running}")),
        state: state.to_owned(),
        downloads: user.join("Downloads"),
        running,
        installed: installed.clone(),
        road,
        run: Arc::new(stage::command_runner),
        // SAFETY: getuid reads this process's user and cannot fail.
        uid: unsafe { libc::getuid() },
        pid: std::process::id(),
        log: logs.join("update.log"),
        patience: swap::PATIENCE_S,
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
/// where a swap waits on it, and the swap is handed back to be answered once this app is open on that host; a host
/// left as it was fails the swap with why. Where a swap put this app back after moving the host, `back` moves it back
/// onto these files. A swap starts only from Restart, which the person pressed knowing the running threads carry on,
/// so the host goes at once.
pub fn after_swap(
    settled: Option<swap::Settled>,
    restart: impl FnOnce() -> Result<crate::service::Followed, String>,
    back: impl FnOnce() -> Result<(), String>,
) -> Result<Option<swap::Waiting>, String> {
    match settled {
        Some(swap::Settled::Waiting(waiting)) => {
            waiting.restarting().map_err(|e| e.to_string())?;
            match restart()? {
                crate::service::Followed::Restarted | crate::service::Followed::Already | crate::service::Followed::NotServing => {
                    Ok(Some(waiting))
                }
                crate::service::Followed::Newer(why) => {
                    crate::desktop::log::info(&format!("update: {why}, so this app opens on it"));
                    waiting.not_restarted().map_err(|e| e.to_string())?;
                    Ok(Some(waiting))
                }
                crate::service::Followed::Left(why) => {
                    waiting.left(&why).and_then(|()| waiting.not_restarted()).map_err(|e| e.to_string())?;
                    Err(why)
                }
            }
        }
        Some(swap::Settled::Back { restart: true, .. }) => back().map(|()| None),
        _ => Ok(None),
    }
}

/// What a launch after a swap still owes it: `up` once this app is open on its host, and the offer of an app an
/// unfinished swap replaced.
#[derive(Default)]
struct AfterSwap {
    waiting: Option<(swap::Waiting, String)>,
    previous: Option<PathBuf>,
    /// Why the update this app was put back from did not stay.
    said: Option<String>,
}

impl Global for AfterSwap {}

/// This app's window is open on a host that accepted it: the swap waiting on it hears `up` now, not after a drawn frame,
/// since a locked screen or a window behind another draws none and the swap would put the old app back; and an app an
/// unfinished swap replaced is offered back.
pub fn opened(cx: &mut App) {
    if !cx.has_global::<AfterSwap>() {
        return;
    }
    let (waiting, previous, said) = cx.update_global::<AfterSwap, _>(|a, _| (a.waiting.take(), a.previous.take(), a.said.take()));
    if let Some(said) = said {
        crate::desktop::log::info(&format!("update: {}", words::not_updated(&said)));
        crate::ui::notify(cx, crate::ui::Notice::error(words::not_updated(&said)));
    }
    if let Some((waiting, version)) = waiting.filter(|_| !cx.windows().is_empty()) {
        match waiting.up(&version) {
            Ok(()) => crate::desktop::log::info(&format!("update: wsp {version} is open on its host")),
            Err(e) => crate::desktop::log::error(&format!("update: the swap could not be told wsp is up: {e}")),
        }
    }
    if let Some(previous) = previous {
        crate::desktop::log::info(&format!("update: the last swap never heard up; the app it replaced is kept at {}", previous.display()));
        let back = crate::ui::NoticeAction::new(words::PUT_BACK, |_, cx| put_back(cx));
        crate::ui::notify(cx, crate::ui::Notice::error(words::UNFINISHED).action(back));
    }
}

/// This app did not open on its host. Where a swap waits on it, it exits, and the swap puts the old app back.
pub fn not_opened(said: &str, cx: &App) {
    if let Some((waiting, version)) = cx.try_global::<AfterSwap>().and_then(|a| a.waiting.as_ref()) {
        crate::desktop::log::error(&format!("update: this app did not open on its host ({said}); the swap puts the old app back"));
        _ = waiting.left(&format!("wsp {version} did not open on its host: {said}"));
        std::process::exit(1);
    }
}

fn put_back(cx: &mut App) {
    step(
        "put back".into(),
        |shell| shell.put_back(),
        |done, cx| match done {
            Ok(_) => cx.quit(),
            Err(said) => crate::ui::notify(cx, crate::ui::Notice::error(words::not_restarted(&said.error))),
        },
        cx,
    );
}

/// The service's restart onto this bundle, from a thread of its own; the launch's wsp command written first, which the
/// restart reads.
pub fn restart_host_blocking() -> Result<crate::service::Followed, String> {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    runtime.block_on(crate::service::restart_host())
}

/// The service's restart back onto this bundle after a swap put it back, from a thread of its own.
pub fn restart_back_blocking() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    runtime.block_on(crate::service::restart_back())
}

/// The version a swap hears in `up`: the release of the host this bundle carries, which the package stamps with the
/// bundle's CFBundleShortVersionString, so a copy whose files say another release than its compiled one, as the live
/// check packs one build twice, answers the swap that checked those files; this app's own release where it carries none.
pub fn up_version(bundled: Result<crate::service::Bundled, String>) -> String {
    bundled.ok().and_then(|b| b.release()).unwrap_or_else(wsp_host_client::HostClient::app_release)
}

/// `after_swap` off the main thread, then `open`. A new app whose host did not follow, or that does not open on it,
/// exits before it is up, so the swap puts the old app back.
pub fn launch(settled: Option<swap::Settled>, state: PathBuf, vars: wsp_host_client::Env, open: Box<dyn FnOnce(&mut App)>, cx: &mut App) {
    match &settled {
        Some(swap::Settled::Unfinished { previous }) => cx.set_global(AfterSwap { previous: Some(previous.clone()), ..Default::default() }),
        Some(swap::Settled::Back { said: Some(said), .. }) => cx.set_global(AfterSwap { said: Some(said.clone()), ..Default::default() }),
        _ => {}
    }
    let follows = matches!(settled, Some(swap::Settled::Waiting(_)) | Some(swap::Settled::Back { restart: true, .. }));
    if !follows {
        return open(cx);
    }
    let waiting = matches!(settled, Some(swap::Settled::Waiting(_)));
    let version = up_version(crate::service::bundled());
    crate::desktop::log::info(&format!("update: moving the host onto wsp {version}"));
    crate::screens::dialogs::startup::hold_opening(true, cx);
    let task = off_main(move || {
        crate::service::on_launch(&state, &vars);
        after_swap(settled, restart_host_blocking, restart_back_blocking)
    });
    cx.spawn(async move |cx| {
        let followed = task.await.unwrap_or_else(|_| Err("the restart stopped before it answered".into()));
        match followed {
            Err(said) => {
                crate::desktop::log::error(&format!("update: the host did not follow: {said}"));
                if waiting {
                    std::process::exit(1);
                }
            }
            Ok(Some(answer)) => cx.update(|cx| cx.update_default_global::<AfterSwap, _>(|a, _| a.waiting = Some((answer, version)))),
            Ok(None) => {}
        }
        cx.update(|cx| {
            crate::screens::dialogs::startup::hold_opening(false, cx);
            open(cx);
        });
    })
    .detach();
}
