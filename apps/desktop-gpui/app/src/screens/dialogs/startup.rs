// SPDX-License-Identifier: AGPL-3.0-only
//! A launch (`desktop:main.ts`): one app per home, the starting window once the open takes a moment, then onboarding
//! where the host holds nothing (`desktop:host-lifecycle.ts` `firstLaunch`), else the app; a start that failed says so
//! in one box with a button to the log (`desktop:starting.ts`).

use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::{
    point, px, size, AnyWindowHandle, App, AsyncApp, Bounds, Global, PromptButton, PromptLevel, Task, TitlebarOptions, WindowBounds,
    WindowOptions,
};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{AgentsAddToolsRequest, AgentsReadRequest, GoldenGetRequest, WorkspacesListRequest};
use wsp_host_client::start::{self, Failed, Start, System};
use wsp_host_client::{Env, Failure, HostClient};

use super::{onboarding, starting};
use crate::desktop::switch;
use crate::screens::Host;
use crate::TITLE;

/// `STARTING_SHOWN_MS`: an open that attaches to a host already up is quicker than this, and shows no starting window.
pub const STARTING_SHOWN: Duration = Duration::from_millis(800);

/// Set while a launch's own open is finding its window, which a knock or a Dock click then leaves to it, as Electron's
/// open in flight is shared.
struct Opening(bool);

impl Global for Opening {}

/// Whether a launch's own open is still finding its window.
pub fn opening(cx: &App) -> bool {
    cx.try_global::<Opening>().is_some_and(|o| o.0)
}

/// What a failed launch's box is titled, as `main.ts` names the launch's.
pub const START_FAILED: &str = "wsp could not start";
/// What the box is titled for an open after the launch's: the Dock, or a second launch with no window standing.
pub const OPEN_FAILED: &str = "wsp could not open";
pub const OPEN_LOG: &str = "Open log";
/// The way back, offered first while the switch is on, so today's app sets wsp up and starts it.
pub const OPEN_CURRENT: &str = "Open the current wsp app";
pub const QUIT: &str = "Quit";

const LOCK_FILE: &str = "desktop-gpui.lock";
const RAISE_SOCKET: &str = "desktop-gpui.sock";

/// Every window a launch opens: the Electron app's size, and on a Mac its hiddenInset frame, no title bar and the
/// lights centred on the frame row's 26 px line.
pub fn window_options(cx: &mut App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
    let titlebar = if cfg!(target_os = "macos") {
        TitlebarOptions { title: Some(TITLE.into()), appears_transparent: true, traffic_light_position: Some(point(px(16.), px(19.))) }
    } else {
        TitlebarOptions { title: Some(TITLE.into()), ..Default::default() }
    };
    WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), titlebar: Some(titlebar), ..Default::default() }
}

/// The home a state file is served from, which the lock is keyed on as Electron keys it on the home's own folder.
pub fn home_of(state: &Path) -> PathBuf {
    state.parent().map(Path::to_owned).unwrap_or_default()
}

/// One app per home: held while the app runs, the lock file's own lock released by the OS when the holder ends.
pub struct Lock {
    raised: Option<mpsc::UnboundedReceiver<()>>,
    links: Option<mpsc::UnboundedReceiver<String>>,
    listener: Option<Listener>,
    socket: PathBuf,
    _file: File,
}

/// The flag that tells the thread answering later launches' knocks that the lock went.
struct Listener {
    stop: Arc<AtomicBool>,
}

pub enum Claim {
    Held(Lock),
    /// Another app holds this home; it has been asked to bring its window forward.
    Taken,
}

/// Takes the home's lock, or asks the app holding it to come forward (`app.requestSingleInstanceLock` and
/// `second-instance`).
pub fn claim(home: &Path) -> io::Result<Claim> {
    claim_with(home, None)
}

/// [`claim`] for a launch that carries a wsp:// link, which the app holding the home opens in place of a raise.
pub fn claim_with(home: &Path, link: Option<&str>) -> io::Result<Claim> {
    make_home(home)?;
    let file = OpenOptions::new().create(true).truncate(false).write(true).open(home.join(LOCK_FILE))?;
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            raise(home, link);
            return Ok(Claim::Taken);
        }
        Err(std::fs::TryLockError::Error(e)) => return Err(e),
    }
    let socket = home.join(RAISE_SOCKET);
    let (raised, links, listener) = match listen(&socket) {
        Some((raised, links, listener)) => (Some(raised), Some(links), Some(listener)),
        None => (None, None, None),
    };
    Ok(Claim::Held(Lock { raised, links, listener, socket, _file: file }))
}

/// The home at the person's own mode, as wsp makes it.
#[cfg(unix)]
fn make_home(home: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(home)
}

#[cfg(not(unix))]
fn make_home(home: &Path) -> io::Result<()> {
    std::fs::create_dir_all(home)
}

impl Lock {
    /// Each later launch on this home, once, as it asks this app to come forward.
    pub fn raised(&mut self) -> Option<mpsc::UnboundedReceiver<()>> {
        self.raised.take()
    }

    /// Each later launch on this home that carried a link, with the link as it came.
    pub fn links(&mut self) -> Option<mpsc::UnboundedReceiver<String>> {
        self.links.take()
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        if let Some(Listener { stop }) = self.listener.take() {
            stop.store(true, Ordering::SeqCst);
            // The thread wakes on this knock, finds the flag set and ends, closing the channel and the socket.
            raise(self.socket.parent().unwrap_or(Path::new(".")), None);
        }
        _ = std::fs::remove_file(&self.socket);
    }
}

/// The longest line a knock may carry, and how long the listener waits for it.
const KNOCK_MAX: u64 = 4096;
const KNOCK_WAIT: Duration = Duration::from_millis(500);

/// The bare knocks and the links a listener hears, and the flag that ends it.
type Knocks = (mpsc::UnboundedReceiver<()>, mpsc::UnboundedReceiver<String>, Listener);

#[cfg(unix)]
fn listen(socket: &Path) -> Option<Knocks> {
    use std::io::{BufRead as _, BufReader, Read as _};
    use std::os::unix::net::UnixListener;
    // The holder of the lock owns the socket's name; a socket left by a holder that died is gone with it.
    _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket).ok()?;
    let (raise_tx, raise_rx) = mpsc::unbounded();
    let (link_tx, link_rx) = mpsc::unbounded();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    std::thread::Builder::new()
        .name("wsp-raise".into())
        .spawn(move || {
            for knock in listener.incoming() {
                let Ok(knock) = knock else { return };
                if stopped.load(Ordering::SeqCst) {
                    return;
                }
                // A bare knock closes at once; a launch with a link writes it as one line first.
                _ = knock.set_read_timeout(Some(KNOCK_WAIT));
                let mut line = String::new();
                _ = BufReader::new(knock.take(KNOCK_MAX)).read_line(&mut line);
                let sent = match line.strip_suffix('\n').unwrap_or(&line) {
                    "" => raise_tx.unbounded_send(()).is_ok(),
                    link => link_tx.unbounded_send(link.to_owned()).is_ok(),
                };
                if !sent {
                    return;
                }
            }
        })
        .ok()?;
    Some((raise_rx, link_rx, Listener { stop }))
}

#[cfg(not(unix))]
fn listen(_: &Path) -> Option<Knocks> {
    None
}

/// Knocks on the holder's socket, with the launch's link as one line where it carries one; whether anyone answered.
#[cfg(unix)]
fn raise(home: &Path, link: Option<&str>) -> bool {
    use std::io::Write as _;
    let Ok(mut knock) = std::os::unix::net::UnixStream::connect(home.join(RAISE_SOCKET)) else { return false };
    match link {
        // A break inside the link becomes a space, which no link holds, so the holder reads it as today's app does: as nothing.
        Some(link) => knock.write_all(format!("{}\n", link.replace(['\n', '\r'], " ")).as_bytes()).is_ok(),
        None => true,
    }
}

#[cfg(not(unix))]
fn raise(_: &Path, _: Option<&str>) -> bool {
    false
}

/// The starting window, standing once `after` has passed unless the launch ended first.
pub struct Starting {
    show: fn(&mut App) -> Option<AnyWindowHandle>,
    shown: Rc<RefCell<Option<AnyWindowHandle>>>,
    timer: Option<Task<()>>,
}

/// `startingAfter`: shows the window `show` opens once `after` has passed. The caller ends it once the next window
/// stands, so a window stands the whole time: on Linux the last window closing quits the app.
pub fn starting_after(after: Duration, show: fn(&mut App) -> Option<AnyWindowHandle>, cx: &mut App) -> Starting {
    let shown = Rc::new(RefCell::new(None));
    let slot = shown.clone();
    let timer = cx.spawn(async move |cx: &mut AsyncApp| {
        cx.background_executor().timer(after).await;
        cx.update(|cx| *slot.borrow_mut() = show(cx));
    });
    Starting { show, shown, timer: Some(timer) }
}

impl Starting {
    pub fn shown(&self) -> Option<AnyWindowHandle> {
        *self.shown.borrow()
    }

    /// The window now, whether or not its time had come.
    pub fn stand(&mut self, cx: &mut App) -> Option<AnyWindowHandle> {
        self.timer = None;
        if self.shown().is_none() {
            let shown = (self.show)(cx);
            *self.shown.borrow_mut() = shown;
        }
        self.shown()
    }

    /// Takes the window away, if it stood; the timer goes with this.
    pub fn end(self, cx: &mut App) {
        let shown = self.shown.borrow_mut().take();
        if let Some(window) = shown {
            _ = window.update(cx, |_, window, _| window.remove_window());
        }
    }
}

/// Where a launch goes once the host answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Onboarding,
    App,
}

/// `holdsNothing`: no sealed golden to fork from and no workspace record, this computer's included.
pub fn holds_nothing(manifest: &Value, workspaces: usize) -> bool {
    let head = &manifest["head"];
    let sealed = !head.is_null() && manifest["versions"].as_array().is_some_and(|v| v.iter().any(|v| v["version"] == *head));
    !sealed && workspaces == 0
}

/// `firstLaunch`: asks the host `golden.get` and `workspaces.list`, and opens onboarding where it holds nothing.
pub async fn next(client: &HostClient) -> Result<Next, Failure> {
    // `openHostReady`: a host somewhere else is never this computer's first launch.
    if !client.here() {
        return Ok(Next::App);
    }
    let get: GoldenGetRequest = request(json!({ "name": "default" }))?;
    let manifest = client.call(get).await?.manifest.map(|m| serde_json::to_value(m).unwrap_or_default()).unwrap_or_default();
    let list: WorkspacesListRequest = request(json!({}))?;
    let workspaces = client.call(list).await?.workspaces.len();
    Ok(if holds_nothing(&manifest, workspaces) { Next::Onboarding } else { Next::App })
}

fn request<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, Failure> {
    serde_json::from_value(params).map_err(|e| Failure::new(e.to_string()))
}

/// One catalog agent as onboarding lists it (`agentsHere`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Agent {
    pub id: String,
    pub name: String,
    /// Its command answers on this computer's PATH.
    pub found: bool,
    /// That command installs the agent the first time it runs.
    pub installs: bool,
}

/// This computer's agents in the catalog's order, every one of them found or not: `agents.read` on this computer,
/// which reads what `agentsHere` reads.
pub async fn agents(client: &HostClient) -> Result<Vec<Agent>, Failure> {
    let read: AgentsReadRequest = request(json!({ "target": { "placeId": "here" } }))?;
    let report = client.call(read).await?.report;
    Ok(report
        .agents
        .into_iter()
        .map(|row| Agent { id: row.id, name: row.name, found: row.installed, installs: row.installs_on_first_run.is_some() })
        .collect())
}

/// The wsp tools into each agent `ids` names, one `agents.addTools` each; every agent whose config would not take
/// them, with the host's words.
pub async fn add_tools(client: &HostClient, ids: Vec<String>) -> Vec<(String, String)> {
    let mut refused = Vec::new();
    for id in ids {
        let written = match request::<AgentsAddToolsRequest>(json!({ "target": { "placeId": "here" }, "agent": id })) {
            Ok(add) => client.call(add).await.map(|_| ()),
            Err(f) => Err(f),
        };
        if let Err(f) = written {
            refused.push((id, f.message));
        }
    }
    refused
}

/// `computerNameHere`: the name the person gave this Mac, else the hostname lowercased.
pub fn computer_name() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let said = std::process::Command::new("/usr/sbin/scutil").env_clear().args(["--get", "ComputerName"]).output().ok();
        let said = said.filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
        if let Some(name) = said.filter(|n| !n.is_empty()) {
            return Some(name);
        }
    }
    let mut buf = [0u8; 256];
    // SAFETY: the buffer is writable for its whole length, which is what is passed.
    if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } != 0 {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let name = String::from_utf8_lossy(&buf[..end]).to_lowercase();
    (!name.is_empty()).then_some(name)
}

/// What a failed start's box says under its title: the failure's first line, never the log's.
pub fn first_line(message: &str) -> &str {
    message.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(START_FAILED)
}

/// The log Open log opens (`startFailedDialog`): the one a failed start names, else the host's beside the state file
/// where it wrote one, else the app's own, `app`.
pub fn log_of(state: &Path, failed: Option<&Path>, app: &Path) -> PathBuf {
    if let Some(log) = failed {
        return log.to_owned();
    }
    let log = state.with_file_name("host.log");
    if log.exists() {
        log
    } else {
        app.to_owned()
    }
}

/// The window for this launch (`openOnHost`): the starting window after 800 ms while the host is attached to, or
/// started from the unit today's app wrote and waited on (`openHostReady`), then onboarding on a host that holds
/// nothing and the app otherwise. A host that refuses this computer's token opens the app, whose window pairs.
pub fn open(state: PathBuf, env: Env, runtime: Arc<Runtime>, app: Rc<dyn Fn(&mut App)>, cx: &mut App) {
    open_titled(START_FAILED, state, env, runtime, app, cx);
}

/// An open after the launch's with no window standing: the same road, its box titled as a later open's. One host step
/// at a time: an open while another is still finding its window joins that one.
pub fn open_later(state: PathBuf, env: Env, runtime: Arc<Runtime>, app: Rc<dyn Fn(&mut App)>, cx: &mut App) {
    if !opening(cx) {
        open_titled(OPEN_FAILED, state, env, runtime, app, cx);
    }
}

fn open_titled(title: &'static str, state: PathBuf, env: Env, runtime: Arc<Runtime>, app: Rc<dyn Fn(&mut App)>, cx: &mut App) {
    let starting = starting_after(STARTING_SHOWN, starting::open, cx);
    cx.set_global(Opening(true));
    let road = Start::here(&state, &env);
    let asked = runtime.spawn(async move {
        start::ready(&road, &System::new(&road.env), |client: Arc<HostClient>| async move { next(&client).await }).await
    });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let asked = asked.await.unwrap_or_else(|e| Err(Failed::from(Failure::new(e.to_string()))));
        cx.update(|cx| cx.set_global(Opening(false)));
        cx.update(|cx| match asked {
            Ok((_, Next::App)) => {
                app(cx);
                starting.end(cx);
            }
            Ok((client, Next::Onboarding)) => {
                onboarding::open(Host::new(client, runtime), app, cx);
                starting.end(cx);
            }
            Err(failed) if failed.failure.kind.as_deref() == Some("auth") => {
                app(cx);
                starting.end(cx);
            }
            Err(failed) => say_failed(title, &failed, &state, starting, cx),
        });
    })
    .detach();
}

/// The start-failed box's answers as Electron's are: Open log the default, Quit the cancel. A Mac's alert gives the
/// cancel button Escape; GPUI's own box elsewhere takes no Escape at all. With the way `back` on offer, its button
/// comes first, the default a Mac's alert gives Return.
pub fn failed_answers(back: bool) -> Vec<PromptButton> {
    let today = [PromptButton::new(OPEN_LOG), PromptButton::cancel(QUIT)];
    back.then(|| PromptButton::new(OPEN_CURRENT)).into_iter().chain(today).collect()
}

/// Whether a failed start offers the way back: while the switch is on and the wsp command names today's app.
pub fn offers_back(cx: &App) -> bool {
    switch::on(cx) && cx.has_global::<switch::Back>()
}

/// `sayStartFailed`: the title, the failure's first line, the way back where offered, Open log and Quit, on the
/// starting window, which stands for it if it did not yet. The starting window goes once it is answered, except under
/// the way back, whose own failure box it holds.
fn say_failed(title: &'static str, failed: &Failed, state: &Path, mut starting: Starting, cx: &mut App) {
    crate::desktop::log::error(&format!("{title}: {}", failed.failure.message));
    let Some(window) = starting.stand(cx) else {
        cx.quit();
        return;
    };
    let line = first_line(&failed.failure.message).to_owned();
    let back = offers_back(cx);
    let answers = failed_answers(back);
    let answer = window.update(cx, |_, window, cx| window.prompt(PromptLevel::Critical, title, Some(&line), &answers, cx));
    let app_log = cx.try_global::<crate::desktop::Launched>().map(|l| l.logs().join(crate::desktop::log::APP_LOG)).unwrap_or_default();
    let log = log_of(state, failed.log.as_deref(), &app_log);
    cx.spawn(async move |cx: &mut AsyncApp| {
        let picked = match answer {
            Ok(answer) => answer.await.ok(),
            Err(_) => None,
        };
        cx.update(|cx| {
            match (back, picked) {
                (true, Some(0)) => return switch::back(cx),
                (true, Some(1)) | (false, Some(0)) => cx.open_with_system(&log),
                _ => cx.quit(),
            }
            starting.end(cx);
        });
    })
    .detach();
}

struct HeldLock {
    _lock: Lock,
}

impl Global for HeldLock {}

/// Holds the lock for the app's life, and brings this app's window forward on each later launch on this home, or
/// opens one where none stands; a launch's link opens as the system's would. A launch while this one's own open is
/// still finding its window moves nothing, and the open goes on.
pub fn follow_raises(mut lock: Lock, reopen: Rc<dyn Fn(&mut App)>, cx: &mut App) {
    let (raised, links) = (lock.raised(), lock.links());
    cx.set_global(HeldLock { _lock: lock });
    if let Some(mut links) = links {
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Some(link) = links.next().await {
                cx.update(|cx| crate::desktop::links::open(&link, cx));
            }
        })
        .detach();
    }
    let Some(mut raised) = raised else { return };
    cx.spawn(async move |cx: &mut AsyncApp| {
        while raised.next().await.is_some() {
            cx.update(|cx| {
                if opening(cx) {
                    return;
                }
                match cx.windows().last().copied() {
                    Some(window) => _ = window.update(cx, |_, window, cx| crate::desktop::frame::raise(window, cx)),
                    None => reopen(cx),
                }
            });
        }
    })
    .detach();
}
