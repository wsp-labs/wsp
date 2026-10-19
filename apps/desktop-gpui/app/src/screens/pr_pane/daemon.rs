// SPDX-License-Identifier: AGPL-3.0-only
//! The workspace's daemon link (`files/wire.ts`) the Changes and Files panes send `git.*` and `fs.*` over: the road of
//! the computer's one channel the terminal opens (`terminal/link.rs`), held while a pane is, its requests sent beside
//! the shells. It also hears where each thread's agent moved its tool shell, which the folder rule follows.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::channel::oneshot;
use futures::future::{select, Either};
use futures::StreamExt as _;
use gpui_kit::{App, AppContext, Context, Entity, Subscription, Task, WeakEntity};
use serde_json::{json, Map, Value};
use wsp_host_client::ops::terminal::DaemonSent;
use wsp_host_client::ops::thread::History;
use wsp_host_client::protocol::{DaemonSendRequest, DaemonSendRequestFrame, SessionsHistoryRequest};
use wsp_host_client::{Event, Failure, HostClient};

use super::files::paths::{root_of, roots};
use crate::screens::terminal::link::{Link, LinkHold, Road, Status, Target};
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

/// A tool call of `thread` moved the agent's shell to `dir`, at `pos` in the workspace's transcript.
#[derive(Debug, PartialEq, Eq)]
struct Shell {
    thread: String,
    pos: Option<u64>,
    dir: String,
}

pub struct DaemonLink {
    host: Host,
    workspace_id: String,
    link: Entity<Link>,
    road: Arc<Mutex<Road>>,
    _hold: LinkHold,
    /// The threads whose shell folder the listener reads off their history, as each joins.
    asked: UnboundedSender<String>,
    _events: Task<()>,
    _link: Subscription,
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
    /// The workspace's link, shared by every pane that holds it; the computer's channel is let go as the terminal's
    /// is, a while after nothing holds it.
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
        let link = Link::get(&host, Target::Workspace(workspace_id.clone()), cx);
        let (hold, road) = link.update(cx, |link, _| (link.hold(), link.road()));
        // Each move of the channel or its folder is the link's notify, so its panes read the folder again and ask
        // again for what failed while it was down.
        let observing = cx.observe(&link, |_, _, cx| cx.notify());
        let (told, mut heard) = unbounded();
        let (asked, wanted) = unbounded();
        let events = host.run({
            let workspace_id = workspace_id.clone();
            move |client| listen(client, workspace_id, wanted, told)
        });
        let shells_of = workspace_id.clone();
        let _events = cx.spawn(async move |this, cx| {
            let _listening = events;
            while let Some(Shell { thread, pos, dir }) = heard.next().await {
                let moved = SHELLS.with(|shells| keep_shell(&mut shells.borrow_mut(), (shells_of.clone(), thread), pos, dir));
                if moved && this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }
        });
        cx.on_release(|this: &mut DaemonLink, _| {
            SHELLS.with(|shells| shells.borrow_mut().retain(|(workspace, _), _| *workspace != this.workspace_id));
        })
        .detach();
        DaemonLink { host, workspace_id, link, road, _hold: hold, asked, _events, _link: observing }
    }

    /// Whether a proven channel is open now; false before the first dial and after the host closed it.
    pub fn up(&self) -> bool {
        self.road.lock().unwrap().channel.is_some()
    }

    /// Dials now a link waiting out its backoff, as a pane that wants a read asks; one live or dialling is left be,
    /// and a door that refused is dialled only at the backoff's ceiling unless `woke`, the workspace just running.
    pub fn redial(this: &Entity<DaemonLink>, woke: bool, cx: &mut App) {
        let link = this.read(cx).link.clone();
        link.update(cx, |link, cx| {
            if woke || !matches!(link.status(), Status::Refused(_)) {
                link.redial(cx);
            }
        });
    }

    /// How the computer's channel stands, in the terminal's words: what the panes say while it is down.
    pub fn status(&self) -> Status {
        self.road.lock().unwrap().status.clone()
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
        let road = self.road.lock().unwrap();
        road.channel.as_ref().and(road.root.clone())
    }

    /// One request, `{ op, ...params }`, answered with the daemon's reply once its `ok` is true. A dial out is waited
    /// for; a link down otherwise answers at once with its words, as `wire.ts` rejects. Await it
    /// from a GPUI task; the request runs on the host's runtime and stops when the returned future is dropped.
    pub fn request(&self, op: &str, params: Value) -> impl std::future::Future<Output = Result<Map<String, Value>, Refused>> + 'static {
        let (host, road) = (self.host.clone(), self.road.clone());
        let mut frame = match params {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        frame.insert("op".into(), Value::String(op.to_owned()));
        async move {
            let channel = proven(&road).await?;
            let sent = host.run(move |client| async move { on(&client, &channel, frame).await });
            refusal(sent.await?)
        }
    }
}

/// The channel once a ping proved it; a dial out is waited for, up to the first-answer bound.
async fn proven(road: &Mutex<Road>) -> Result<String, Refused> {
    loop {
        let changed = {
            let mut road = road.lock().unwrap();
            if let Some(channel) = &road.channel {
                return Ok(channel.clone());
            }
            if !road.dialling || road.status == Status::Unanswered {
                let message = match &road.status {
                    Status::Refused(reason) | Status::Dead(reason) if !reason.is_empty() => reason.to_string(),
                    _ => "not answering".to_owned(),
                };
                return Err(Refused { message, code: None });
            }
            let (tx, rx) = oneshot::channel();
            road.waiters.push(tx);
            rx
        };
        if changed.await.is_err() {
            return Err(Refused { message: "connection lost".into(), code: None });
        }
    }
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

fn tell_shell(frame: &Value, workspace_id: &str, told: &UnboundedSender<Shell>) {
    if let Some((thread, pos, dir)) = shell_of(frame, workspace_id) {
        _ = told.unbounded_send(Shell { thread, pos, dir });
    }
}

async fn tell_last_shell(client: &HostClient, workspace_id: &str, thread: &str, told: &UnboundedSender<Shell>) {
    if let Some((pos, dir)) = last_shell(client, workspace_id, thread).await {
        _ = told.unbounded_send(Shell { thread: thread.to_owned(), pos, dir });
    }
}

/// Reads the host's pushes for its threads' shell moves, for the link's whole life: once the socket's stream ends, the
/// pushes are read again a second later with each asked thread's history.
async fn listen(
    client: Arc<HostClient>,
    workspace_id: String,
    mut wanted: UnboundedReceiver<String>,
    told: UnboundedSender<Shell>,
) -> Result<(), Failure> {
    let mut threads: HashSet<String> = HashSet::new();
    loop {
        if let Ok(mut events) = client.events().await {
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
                match event {
                    Ok(Event::Host(event)) => tell_shell(&serde_json::to_value(&*event).unwrap_or_default(), &workspace_id, &told),
                    Ok(Event::Other(frame)) => tell_shell(&frame, &workspace_id, &told),
                    _ => {}
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::{keep_pin, keep_shell, shell_of, KEPT_PINS};

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
}
