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
    point, px, size, AnyWindowHandle, App, AsyncApp, Bounds, Global, PromptLevel, Task, TitlebarOptions, WindowBounds, WindowOptions,
};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{AgentsAddToolsRequest, AgentsReadRequest, GoldenGetRequest, WorkspacesListRequest};
use wsp_host_client::{Env, Failure, HostClient};

use super::{onboarding, starting};
use crate::screens::Host;
use crate::TITLE;

/// `STARTING_SHOWN_MS`: an open that attaches to a host already up is quicker than this, and shows no starting window.
pub const STARTING_SHOWN: Duration = Duration::from_millis(800);

/// What a failed launch's box is titled, as `main.ts` names the launch's.
pub const START_FAILED: &str = "wsp could not start";
pub const OPEN_LOG: &str = "Open log";
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
    make_home(home)?;
    let file = OpenOptions::new().create(true).truncate(false).write(true).open(home.join(LOCK_FILE))?;
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            raise(home);
            return Ok(Claim::Taken);
        }
        Err(std::fs::TryLockError::Error(e)) => return Err(e),
    }
    let socket = home.join(RAISE_SOCKET);
    let (raised, listener) = listen(&socket).unzip();
    Ok(Claim::Held(Lock { raised, listener, socket, _file: file }))
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
}

impl Drop for Lock {
    fn drop(&mut self) {
        if let Some(Listener { stop }) = self.listener.take() {
            stop.store(true, Ordering::SeqCst);
            // The thread wakes on this knock, finds the flag set and ends, closing the channel and the socket.
            raise(self.socket.parent().unwrap_or(Path::new(".")));
        }
        _ = std::fs::remove_file(&self.socket);
    }
}

#[cfg(unix)]
fn listen(socket: &Path) -> Option<(mpsc::UnboundedReceiver<()>, Listener)> {
    use std::os::unix::net::UnixListener;
    // The holder of the lock owns the socket's name; a socket left by a holder that died is gone with it.
    _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket).ok()?;
    let (tx, rx) = mpsc::unbounded();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    std::thread::Builder::new()
        .name("wsp-raise".into())
        .spawn(move || {
            for knock in listener.incoming() {
                if stopped.load(Ordering::SeqCst) || knock.is_err() || tx.unbounded_send(()).is_err() {
                    return;
                }
            }
        })
        .ok()?;
    Some((rx, Listener { stop }))
}

#[cfg(not(unix))]
fn listen(_: &Path) -> Option<(mpsc::UnboundedReceiver<()>, Listener)> {
    None
}

/// Knocks on the holder's socket; whether anyone answered.
#[cfg(unix)]
fn raise(home: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(home.join(RAISE_SOCKET)).is_ok()
}

#[cfg(not(unix))]
fn raise(_: &Path) -> bool {
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
        let said = std::process::Command::new("/usr/sbin/scutil").args(["--get", "ComputerName"]).output().ok();
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

/// The log Open log opens: the host's beside the state file where it wrote one, else the state's folder.
pub fn log_of(state: &Path) -> PathBuf {
    let log = state.with_file_name("host.log");
    if log.exists() {
        log
    } else {
        home_of(state)
    }
}

/// The window for this launch (`openOnHost`): the starting window after 800 ms, then onboarding on a host that holds
/// nothing and the app otherwise. A host that refuses this computer's token opens the app, whose window pairs.
pub fn open(state: PathBuf, env: Env, runtime: Arc<Runtime>, app: Rc<dyn Fn(&mut App)>, cx: &mut App) {
    let starting = starting_after(STARTING_SHOWN, starting::open, cx);
    let asked = runtime.spawn({
        let (state, env) = (state.clone(), env.clone());
        async move {
            let client = HostClient::connect(&state, &env).await?;
            let next = next(&client).await?;
            Ok::<_, Failure>((client, next))
        }
    });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let asked = asked.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
        cx.update(|cx| match asked {
            Ok((_, Next::App)) => {
                app(cx);
                starting.end(cx);
            }
            Ok((client, Next::Onboarding)) => {
                onboarding::open(Host::new(Arc::new(client), runtime), app, cx);
                starting.end(cx);
            }
            Err(failure) if failure.kind.as_deref() == Some("auth") => {
                app(cx);
                starting.end(cx);
            }
            Err(failure) => say_failed(&failure, &state, starting, cx),
        });
    })
    .detach();
}

/// `sayStartFailed`: the title, the failure's first line, Open log and Quit, on the starting window, which stands for
/// it if it did not yet. Escape is Quit. The starting window goes once it is answered.
fn say_failed(failure: &Failure, state: &Path, mut starting: Starting, cx: &mut App) {
    eprintln!("desktop-gpui: {START_FAILED}: {}", failure.message);
    let Some(window) = starting.stand(cx) else {
        cx.quit();
        return;
    };
    let line = first_line(&failure.message).to_owned();
    let answer = window.update(cx, |_, window, cx| window.prompt(PromptLevel::Critical, START_FAILED, Some(&line), &[OPEN_LOG, QUIT], cx));
    let log = log_of(state);
    cx.spawn(async move |cx: &mut AsyncApp| {
        let picked = match answer {
            Ok(answer) => answer.await.ok(),
            Err(_) => None,
        };
        cx.update(|cx| {
            if picked == Some(0) {
                cx.open_with_system(&log);
            } else {
                cx.quit();
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
/// opens one where none stands.
pub fn follow_raises(mut lock: Lock, reopen: Rc<dyn Fn(&mut App)>, cx: &mut App) {
    let raised = lock.raised();
    cx.set_global(HeldLock { _lock: lock });
    let Some(mut raised) = raised else { return };
    cx.spawn(async move |cx: &mut AsyncApp| {
        while raised.next().await.is_some() {
            cx.update(|cx| {
                cx.activate(true);
                match cx.windows().last().copied() {
                    Some(window) => _ = window.update(cx, |_, window, _| window.activate_window()),
                    None => reopen(cx),
                }
            });
        }
    })
    .detach();
}
