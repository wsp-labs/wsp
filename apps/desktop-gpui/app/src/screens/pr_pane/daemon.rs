// SPDX-License-Identifier: AGPL-3.0-only
//! The workspace's daemon link (`files/wire.ts`, `terminal/daemon-link.ts`) the Changes and Files panes send `git.*`
//! and `fs.*` over: one channel per workspace, dialled on the first request and proven with a ping, dialled again
//! once the host says it closed, and closed when the last pane holding it goes. It also hears where each thread's agent
//! moved its tool shell, which the folder rule follows.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::future::{select, Either};
use futures::lock::Mutex as Dial;
use futures::StreamExt as _;
use gpui_kit::{App, AppContext, Context, Entity, Task, WeakEntity};
use serde_json::{json, Map, Value};
use wsp_host_client::ops::terminal::DaemonSent;
use wsp_host_client::ops::thread::History;
use wsp_host_client::protocol::{
    DaemonCloseRequest, DaemonOpenReply, DaemonOpenRequest, DaemonSendRequest, DaemonSendRequestFrame, SessionsHistoryRequest,
};
use wsp_host_client::{Event, Failure, HostClient};

use super::files::paths::{root_of, roots};
use crate::screens::Host;

/// The daemon's refusal of a request, with the typed code the files and diff ops send (`DaemonRequestError`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub message: String,
    pub code: Option<String>,
}

impl From<Failure> for Refused {
    fn from(failure: Failure) -> Self {
        Refused { message: failure.message, code: None }
    }
}

/// The hellos kept that no dial of this link has claimed yet: every link's pushes ride the one host socket, so the
/// others' are heard too, and only the newest few are kept.
const UNCLAIMED: usize = 8;

/// What the link's listener tells the panes' side: its channel or folder moved, or a tool call of `thread` moved the
/// agent's shell to `dir`, at `pos` in the workspace's transcript.
#[derive(Debug, PartialEq, Eq)]
enum Told {
    Moved,
    Shell { thread: String, pos: Option<u64>, dir: String },
}

/// What a link holds: the channel the last dial opened and the folder its hello named, the hellos heard before their
/// dial stored its channel, whether the host's pushes are read now (a hello pushed before that is lost), whether the
/// link was let go, after which a dial in flight keeps nothing, and where it tells the link's panes its channel moved.
#[derive(Default)]
struct Held {
    channel: Option<String>,
    root: Option<String>,
    unclaimed: VecDeque<(String, String)>,
    listening: bool,
    released: bool,
    moved: Option<UnboundedSender<Told>>,
}

impl Held {
    fn claim(&mut self, channel: String) {
        self.root = self.unclaimed.iter().position(|(on, _)| *on == channel).and_then(|at| self.unclaimed.remove(at)).map(|(_, root)| root);
        self.channel = Some(channel);
        self.tell();
    }

    fn heard(&mut self, channel: String, root: String) {
        if self.channel.as_deref() == Some(channel.as_str()) {
            self.root = Some(root);
            self.tell();
            return;
        }
        if self.unclaimed.len() == UNCLAIMED {
            self.unclaimed.pop_front();
        }
        self.unclaimed.push_back((channel, root));
    }

    fn let_go(&mut self) -> Option<String> {
        self.root = None;
        let gone = self.channel.take();
        if gone.is_some() {
            self.tell();
        }
        gone
    }

    fn tell(&self) {
        if let Some(moved) = &self.moved {
            _ = moved.unbounded_send(Told::Moved);
        }
    }
}

pub struct DaemonLink {
    host: Host,
    workspace_id: String,
    held: Arc<Mutex<Held>>,
    /// One dial at a time: requests that find no channel wait on the first one's.
    dial: Arc<Dial<()>>,
    /// The threads whose shell folder the listener reads off their history, as each joins.
    asked: UnboundedSender<String>,
    _events: Task<()>,
}

thread_local! {
    static LINKS: RefCell<HashMap<String, WeakEntity<DaemonLink>>> = RefCell::new(HashMap::new());
    /// The folder each thread's panes are pinned to, by workspace and thread (`files/root.ts`), with the order it was
    /// pinned in: kept past its panes and its link, as the web app's store keeps it until the window reloads.
    static PINS: RefCell<HashMap<(String, String), (u64, String)>> = RefCell::new(HashMap::new());
    /// Where each thread's agent last moved its tool shell, by workspace and thread, with the event's pos: kept while
    /// the workspace's link is, and read again off the history by the next one.
    static SHELLS: RefCell<HashMap<(String, String), (u64, String)>> = RefCell::new(HashMap::new());
}

/// Past this many pins the oldest goes: a person pins a handful of threads, so this only bounds a long run.
const KEPT_PINS: usize = 256;

fn keep_pin(pins: &mut HashMap<(String, String), (u64, String)>, key: (String, String), folder: String) {
    let next = pins.values().map(|(n, _)| n + 1).max().unwrap_or(0);
    pins.insert(key, (next, folder));
    if pins.len() > KEPT_PINS {
        if let Some(oldest) = pins.iter().min_by_key(|(_, (n, _))| *n).map(|(key, _)| key.clone()) {
            pins.remove(&oldest);
        }
    }
}

/// Keeps `dir` as `key`'s shell folder unless a later event already named one; true when the folder moved.
fn keep_shell(shells: &mut HashMap<(String, String), (u64, String)>, key: (String, String), pos: Option<u64>, dir: String) -> bool {
    let kept = shells.get(&key);
    if kept.is_some_and(|(at, _)| pos.is_some_and(|pos| pos < *at)) {
        return false;
    }
    let moved = kept.is_none_or(|(_, was)| *was != dir);
    let at = pos.or(kept.map(|(at, _)| *at)).unwrap_or(0);
    shells.insert(key, (at, dir));
    moved
}

impl DaemonLink {
    /// The workspace's link, shared by every pane that holds it; it closes once none does.
    pub fn of(host: &Host, workspace_id: &str, cx: &mut App) -> Entity<DaemonLink> {
        if let Some(link) = LINKS.with(|links| links.borrow().get(workspace_id).and_then(WeakEntity::upgrade)) {
            return link;
        }
        let link = cx.new(|cx| DaemonLink::new(host.clone(), workspace_id.to_owned(), cx));
        LINKS.with(|links| {
            let mut links = links.borrow_mut();
            links.retain(|_, weak| weak.upgrade().is_some());
            links.insert(workspace_id.to_owned(), link.downgrade());
        });
        link
    }

    fn new(host: Host, workspace_id: String, cx: &mut Context<Self>) -> Self {
        let (moved, mut heard) = unbounded();
        let (asked, wanted) = unbounded();
        let held = Arc::new(Mutex::new(Held { moved: Some(moved.clone()), ..Held::default() }));
        let dial = Arc::new(Dial::new(()));
        let events = host.run({
            let (workspace_id, held, dial) = (workspace_id.clone(), held.clone(), dial.clone());
            move |client| listen(client, workspace_id, held, dial, wanted, moved)
        });
        // Each move of the channel, its folder or a thread's shell is the link's notify, so its panes read the folder
        // again and ask again for what failed while it was down.
        let shells_of = workspace_id.clone();
        let _events = cx.spawn(async move |this, cx| {
            let _listening = events;
            while let Some(told) = heard.next().await {
                let moved = match told {
                    Told::Moved => true,
                    Told::Shell { thread, pos, dir } => {
                        SHELLS.with(|shells| keep_shell(&mut shells.borrow_mut(), (shells_of.clone(), thread), pos, dir))
                    }
                };
                if moved && this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }
        });
        cx.on_release(|this: &mut DaemonLink, cx| {
            SHELLS.with(|shells| shells.borrow_mut().retain(|(workspace, _), _| *workspace != this.workspace_id));
            let mut held = this.held.lock().unwrap();
            held.released = true;
            let Some(channel) = held.let_go() else { return };
            drop(held);
            let closing =
                this.host.run(move |client| async move { client.call(DaemonCloseRequest(DaemonOpenReply { channel })).await.map(|_| ()) });
            cx.background_spawn(async move { _ = closing.await }).detach();
        })
        .detach();
        DaemonLink { host, workspace_id, held, dial, asked, _events }
    }

    /// Whether a proven channel is open now; false before the first dial and after the host closed it.
    pub fn up(&self) -> bool {
        self.held.lock().unwrap().channel.is_some()
    }

    /// The folder `thread`'s panes stay in while the agent moves, where the person pinned them.
    pub fn pinned(&self, thread: &str) -> Option<String> {
        PINS.with(|pins| pins.borrow().get(&(self.workspace_id.clone(), thread.to_owned())).map(|(_, folder)| folder.clone()))
    }

    /// Pins `thread`'s panes to `folder`; every pane holding the link hears it through its notify.
    pub fn pin(&mut self, thread: &str, folder: &str, cx: &mut Context<Self>) {
        PINS.with(|pins| keep_pin(&mut pins.borrow_mut(), (self.workspace_id.clone(), thread.to_owned()), folder.to_owned()));
        cx.notify();
    }

    pub fn unpin(&mut self, thread: &str, cx: &mut Context<Self>) {
        if PINS.with(|pins| pins.borrow_mut().remove(&(self.workspace_id.clone(), thread.to_owned()))).is_some() {
            cx.notify();
        }
    }

    /// Where `thread`'s agent last moved its tool shell, as its tool calls said; none until one did.
    pub fn shell(&self, thread: &str) -> Option<String> {
        SHELLS.with(|shells| shells.borrow().get(&(self.workspace_id.clone(), thread.to_owned())).map(|(_, dir)| dir.clone()))
    }

    /// Reads where `thread`'s agent moved its shell before this pane came, off its newest page of history, once per
    /// link; the moves after it are heard as they land.
    pub fn follow_shell(&self, thread: &str) {
        _ = self.asked.unbounded_send(thread.to_owned());
    }

    /// The folder the panes show, as `selectRoot` in `files/root.ts` picks it: the pin, else the agent's shell folder
    /// where a root holds it, else the thread's own folder, else the project's folder once the hello lists it among the
    /// roots, else the daemon's home; none before.
    pub fn folder(&self, thread: Option<&str>, followed: Option<&str>, project: Option<&str>) -> Option<String> {
        let root = self.root();
        let browsed = roots(root.as_deref(), project);
        let shell = thread.and_then(|t| self.shell(t)).filter(|dir| root_of(&browsed, dir).is_some());
        let listed = followed.or(project.filter(|_| root.is_some())).map(str::to_owned);
        thread.and_then(|t| self.pinned(t)).or(shell).or(listed).or(root)
    }

    /// The folder the daemon's hello named, which every path the panes send must resolve inside; none before it came.
    pub fn root(&self) -> Option<String> {
        let held = self.held.lock().unwrap();
        held.channel.as_ref().and(held.root.clone())
    }

    /// One request, `{ op, ...params }`, answered with the daemon's reply once its `ok` is true. Await it from a GPUI
    /// task; the request runs on the host's runtime and stops when the returned future is dropped.
    pub fn request(&self, op: &str, params: Value) -> impl std::future::Future<Output = Result<Map<String, Value>, Refused>> + 'static {
        let (workspace_id, held, dial) = (self.workspace_id.clone(), self.held.clone(), self.dial.clone());
        let mut frame = match params {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        frame.insert("op".into(), Value::String(op.to_owned()));
        let sent = self.host.run(move |client| async move { Ok(send(&client, &workspace_id, &held, &dial, frame).await) });
        async move { sent.await.map_err(Refused::from)? }
    }
}

/// Sends `frame` on the link's channel, dialling one first when none is open; a channel the host no longer answers on
/// is closed and dialled again once.
async fn send(
    client: &Arc<HostClient>,
    workspace_id: &str,
    held: &Arc<Mutex<Held>>,
    dial: &Arc<Dial<()>>,
    frame: Map<String, Value>,
) -> Result<Map<String, Value>, Refused> {
    for attempt in 0..2 {
        let channel = open(client, workspace_id, held, dial).await?;
        match on(client, &channel, frame.clone()).await {
            Err(failure) if attempt == 0 => {
                let dropped = {
                    let mut kept = held.lock().unwrap();
                    (kept.channel.as_deref() == Some(channel.as_str())).then(|| kept.let_go()).flatten()
                };
                if let Some(channel) = dropped {
                    _ = client.call(DaemonCloseRequest(DaemonOpenReply { channel })).await;
                }
                eprintln!("desktop-gpui: the daemon link for {workspace_id} dropped ({}); dialling again", failure.message);
            }
            Err(failure) => return Err(failure.into()),
            Ok(reply) => return refusal(reply),
        }
    }
    unreachable!("the second attempt returns")
}

/// A channel the host opened that the link does not hold yet: closed on the host if the dial stops before storing it,
/// whether its ping failed, its task was dropped or the link was let go meanwhile.
struct Unclaimed {
    client: Arc<HostClient>,
    channel: Option<String>,
}

impl Drop for Unclaimed {
    fn drop(&mut self) {
        let Some(channel) = self.channel.take() else { return };
        let Ok(runtime) = tokio::runtime::Handle::try_current() else { return };
        let client = self.client.clone();
        runtime.spawn(async move {
            _ = client.call(DaemonCloseRequest(DaemonOpenReply { channel })).await;
        });
    }
}

async fn open(client: &Arc<HostClient>, workspace_id: &str, held: &Arc<Mutex<Held>>, dial: &Arc<Dial<()>>) -> Result<String, Refused> {
    if let Some(channel) = held.lock().unwrap().channel.clone() {
        return Ok(channel);
    }
    let dialling = dial.clone().lock_owned().await;
    // The dial runs to its end on a task of its own: a pane let go while the host opens the channel would otherwise
    // drop the reply naming it, and the host would hold that channel until the socket closes.
    let (client, workspace_id, held) = (client.clone(), workspace_id.to_owned(), held.clone());
    tokio::spawn(async move {
        let _dialling = dialling;
        dial_once(&client, &workspace_id, &held).await
    })
    .await
    .unwrap_or_else(|e| Err(Refused { message: e.to_string(), code: None }))
}

async fn dial_once(client: &Arc<HostClient>, workspace_id: &str, held: &Mutex<Held>) -> Result<String, Refused> {
    for _ in 0..200 {
        if held.lock().unwrap().listening {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    if let Some(channel) = held.lock().unwrap().channel.clone() {
        return Ok(channel);
    }
    let opened = client.call(DaemonOpenRequest { workspace_id: Some(workspace_id.to_owned()), place_id: None }).await?;
    let mut unclaimed = Unclaimed { client: client.clone(), channel: Some(opened.channel.clone()) };
    let mut ping = Map::new();
    ping.insert("op".into(), Value::String("ping".into()));
    refusal(on(client, &opened.channel, ping).await?)?;
    let mut kept = held.lock().unwrap();
    if kept.released {
        return Err(Refused { message: "the link was let go".into(), code: None });
    }
    kept.claim(opened.channel.clone());
    unclaimed.channel = None;
    Ok(opened.channel)
}

async fn on(client: &HostClient, channel: &str, mut frame: Map<String, Value>) -> Result<Value, Failure> {
    let op = match frame.remove("op") {
        Some(Value::String(op)) => op,
        _ => return Err(Failure::new("a daemon frame names its op")),
    };
    let request = DaemonSendRequest { channel: channel.to_owned(), frame: DaemonSendRequestFrame { op, extra: frame } };
    let DaemonSent { reply } = client.call(request).await?;
    Ok(reply)
}

/// The daemon's reply when its `ok` is true, else its own sentence and code as the refusal.
fn refusal(reply: Value) -> Result<Map<String, Value>, Refused> {
    let Value::Object(reply) = reply else { return Err(Refused { message: "the daemon answered with no object".into(), code: None }) };
    if reply.get("ok") == Some(&Value::Bool(true)) {
        return Ok(reply);
    }
    let message = reply.get("error").and_then(Value::as_str).unwrap_or("The daemon refused without saying why.").to_owned();
    let code = reply.get("code").and_then(Value::as_str).map(str::to_owned);
    Err(Refused { message, code })
}

/// A tool call's move of its thread's shell in `workspace_id`, from a `session.delta` frame (`adapt/session.ts`).
fn shell_of(frame: &Value, workspace_id: &str) -> Option<(String, Option<u64>, String)> {
    if frame.get("type")?.as_str()? != "session.delta" || frame.get("workspaceId")?.as_str()? != workspace_id {
        return None;
    }
    let dir = frame.get("cwd")?.as_str().filter(|d| !d.is_empty())?;
    Some((frame.get("threadId")?.as_str()?.to_owned(), frame.get("pos").and_then(Value::as_u64), dir.to_owned()))
}

/// The newest shell move of `thread` its first page of history holds, as the web app's thread view reads it.
async fn last_shell(client: &HostClient, workspace_id: &str, thread: &str) -> Option<(Option<u64>, String)> {
    let asked: SessionsHistoryRequest = serde_json::from_value(json!({ "workspaceId": workspace_id, "threadId": thread })).ok()?;
    let events = match client.call(asked).await.ok()? {
        History::Page(page) => page.events,
        History::Whole { events } => events,
    };
    events.iter().rev().filter_map(|e| shell_of(&serde_json::to_value(e).ok()?, workspace_id)).map(|(_, pos, dir)| (pos, dir)).next()
}

fn tell_shell(frame: &Value, workspace_id: &str, told: &UnboundedSender<Told>) {
    if let Some((thread, pos, dir)) = shell_of(frame, workspace_id) {
        _ = told.unbounded_send(Told::Shell { thread, pos, dir });
    }
}

async fn tell_last_shell(client: &HostClient, workspace_id: &str, thread: &str, told: &UnboundedSender<Told>) {
    if let Some((pos, dir)) = last_shell(client, workspace_id, thread).await {
        _ = told.unbounded_send(Told::Shell { thread: thread.to_owned(), pos, dir });
    }
}

/// Reads the host's pushes for the link's channel, the hello's folder, the close that drops it and its threads' shell
/// moves, for the link's whole life: once the socket's stream ends, its channel went with it, the pushes are read again
/// a second later with each asked thread's history, and a channel the socket took is dialled again, as the web app's
/// wire is.
async fn listen(
    client: Arc<HostClient>,
    workspace_id: String,
    held: Arc<Mutex<Held>>,
    dial: Arc<Dial<()>>,
    mut wanted: UnboundedReceiver<String>,
    told: UnboundedSender<Told>,
) -> Result<(), Failure> {
    let mut rejoin = false;
    let mut threads: HashSet<String> = HashSet::new();
    loop {
        if let Ok(mut events) = client.events().await {
            held.lock().unwrap().listening = true;
            if std::mem::take(&mut rejoin) {
                _ = open(&client, &workspace_id, &held, &dial).await;
            }
            for thread in &threads {
                tell_last_shell(&client, &workspace_id, thread, &told).await;
            }
            loop {
                let event = match select(std::pin::pin!(events.next()), wanted.next()).await {
                    Either::Left((Some(event), _)) => event,
                    Either::Left((None, _)) => break,
                    Either::Right((Some(thread), _)) => {
                        if threads.insert(thread.clone()) {
                            tell_last_shell(&client, &workspace_id, &thread, &told).await;
                        }
                        continue;
                    }
                    Either::Right((None, _)) => return Ok(()),
                };
                let pushed = match event {
                    Ok(Event::Daemon(event)) => event,
                    Ok(Event::Host(event)) => {
                        tell_shell(&serde_json::to_value(&*event).unwrap_or_default(), &workspace_id, &told);
                        continue;
                    }
                    Ok(Event::Other(frame)) => {
                        tell_shell(&frame, &workspace_id, &told);
                        continue;
                    }
                    _ => continue,
                };
                // The union's variants differ by a `type` word typify does not name them by, so the frame is read by its keys.
                let Ok(frame) = serde_json::to_value(&*pushed) else { continue };
                let channel = frame.get("channel").and_then(Value::as_str).unwrap_or_default().to_owned();
                let mut held = held.lock().unwrap();
                match frame.get("type").and_then(Value::as_str) {
                    Some("daemon.event") if frame["event"]["type"] == "daemon.hello" => {
                        if let Some(root) = frame["event"]["root"].as_str() {
                            held.heard(channel, root.to_owned());
                        }
                    }
                    Some("daemon.closed") if held.channel.as_deref() == Some(channel.as_str()) => _ = held.let_go(),
                    _ => {}
                }
            }
            let mut held = held.lock().unwrap();
            held.listening = false;
            rejoin = held.let_go().is_some();
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::{keep_pin, keep_shell, shell_of, Held, KEPT_PINS, UNCLAIMED};

    #[test]
    fn past_the_cap_the_oldest_pin_goes() {
        let mut pins = HashMap::new();
        for n in 0..=KEPT_PINS {
            keep_pin(&mut pins, ("w".into(), format!("t{n}")), format!("/r/{n}"));
        }
        assert_eq!(pins.len(), KEPT_PINS);
        assert!(!pins.contains_key(&("w".to_owned(), "t0".to_owned())), "the first pin went");
        keep_pin(&mut pins, ("w".into(), "t1".into()), "/r/again".into());
        keep_pin(&mut pins, ("w".into(), "new".into()), "/r/new".into());
        assert!(pins.contains_key(&("w".to_owned(), "t1".to_owned())), "a pin set again counts as new");
        assert!(!pins.contains_key(&("w".to_owned(), "t2".to_owned())), "the oldest pin then goes");
    }

    #[test]
    fn a_tool_calls_shell_move_is_read_off_its_delta_and_a_later_one_wins() {
        let delta = json!({ "type": "session.delta", "workspaceId": "w", "threadId": "t", "pos": 7, "kind": "tool_use", "cwd": "/r/src" });
        assert_eq!(shell_of(&delta, "w"), Some(("t".into(), Some(7), "/r/src".into())));
        assert_eq!(shell_of(&delta, "other"), None, "another workspace's move");
        assert_eq!(shell_of(&json!({ "type": "session.delta", "workspaceId": "w", "threadId": "t", "text": "ok" }), "w"), None, "no move");
        assert_eq!(shell_of(&json!({ "type": "session.start", "workspaceId": "w", "threadId": "t", "cwd": "/r" }), "w"), None);

        let mut shells = HashMap::new();
        let key = || ("w".to_owned(), "t".to_owned());
        assert!(keep_shell(&mut shells, key(), Some(7), "/r/src".into()));
        assert!(!keep_shell(&mut shells, key(), Some(3), "/r".into()), "an older event, as a history page read late, keeps the newer");
        assert_eq!(shells[&key()].1, "/r/src");
        assert!(!keep_shell(&mut shells, key(), Some(9), "/r/src".into()), "the same folder again moves nothing");
        assert!(keep_shell(&mut shells, key(), None, "/r/notes".into()), "a push with no pos is the newest");
        assert_eq!(shells[&key()], (9, "/r/notes".to_owned()));
    }

    #[test]
    fn a_link_keeps_the_folder_its_own_channels_hello_named() {
        let mut held = Held::default();
        held.heard("theirs".into(), "/home/b".into());
        held.heard("mine".into(), "/home/a".into());
        held.claim("mine".into());
        assert_eq!(held.root.as_deref(), Some("/home/a"), "a hello heard before the dial stored its channel is claimed by its channel");
        held.heard("later".into(), "/home/c".into());
        assert_eq!(held.root.as_deref(), Some("/home/a"), "another link's hello on the same socket leaves this root");
        assert_eq!(held.let_go().as_deref(), Some("mine"));
        assert_eq!(held.root, None, "the root goes with its channel");

        let mut late = Held::default();
        late.claim("mine".into());
        assert_eq!(late.root, None, "no hello yet");
        late.heard("theirs".into(), "/home/b".into());
        assert_eq!(late.root, None, "another channel's hello is not this link's");
        late.heard("mine".into(), "/home/a".into());
        assert_eq!(late.root.as_deref(), Some("/home/a"), "a hello after the claim is taken");
    }

    #[test]
    fn only_the_newest_unclaimed_hellos_are_kept() {
        let mut held = Held::default();
        for n in 0..=UNCLAIMED {
            held.heard(format!("c{n}"), format!("/r{n}"));
        }
        assert_eq!(held.unclaimed.len(), UNCLAIMED);
        held.claim("c0".into());
        assert_eq!(held.root, None, "the oldest hello went past the cap");
        held.let_go();
        held.claim(format!("c{UNCLAIMED}"));
        assert_eq!(held.root, Some(format!("/r{UNCLAIMED}")));
    }
}
