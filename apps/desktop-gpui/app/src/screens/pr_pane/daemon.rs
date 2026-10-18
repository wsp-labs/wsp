// SPDX-License-Identifier: AGPL-3.0-only
//! The workspace's daemon link (`files/wire.ts`, `terminal/daemon-link.ts`) the Changes and Files panes send `git.*`
//! and `fs.*` over: one channel per workspace, dialled on the first request and proven with a ping, dialled again
//! once the host says it closed, and closed when the last pane holding it goes.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use futures::lock::Mutex as Dial;
use gpui_kit::{App, AppContext, Context, Entity, Task, WeakEntity};
use serde_json::{Map, Value};
use wsp_host_client::ops::terminal::DaemonSent;
use wsp_host_client::protocol::{DaemonCloseRequest, DaemonOpenReply, DaemonOpenRequest, DaemonSendRequest, DaemonSendRequestFrame};
use wsp_host_client::{Event, Failure, HostClient};

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

/// What a link holds: the channel the last dial opened and the folder its hello named, the hellos heard before their
/// dial stored its channel, whether the host's pushes are read now (a hello pushed before that is lost), and whether
/// the link was let go, after which a dial in flight keeps nothing.
#[derive(Default)]
struct Held {
    channel: Option<String>,
    root: Option<String>,
    unclaimed: VecDeque<(String, String)>,
    listening: bool,
    released: bool,
}

impl Held {
    fn claim(&mut self, channel: String) {
        self.root = self.unclaimed.iter().position(|(on, _)| *on == channel).and_then(|at| self.unclaimed.remove(at)).map(|(_, root)| root);
        self.channel = Some(channel);
    }

    fn heard(&mut self, channel: String, root: String) {
        if self.channel.as_deref() == Some(channel.as_str()) {
            self.root = Some(root);
            return;
        }
        if self.unclaimed.len() == UNCLAIMED {
            self.unclaimed.pop_front();
        }
        self.unclaimed.push_back((channel, root));
    }

    fn let_go(&mut self) -> Option<String> {
        self.root = None;
        self.channel.take()
    }
}

pub struct DaemonLink {
    host: Host,
    workspace_id: String,
    held: Arc<Mutex<Held>>,
    /// The folder each thread's panes are pinned to (`files/root.ts`); one entry per thread pinned, gone with the link.
    pins: HashMap<String, String>,
    /// One dial at a time: requests that find no channel wait on the first one's.
    dial: Arc<Dial<()>>,
    _events: Task<()>,
}

thread_local! {
    static LINKS: RefCell<HashMap<String, WeakEntity<DaemonLink>>> = RefCell::new(HashMap::new());
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
        let held = Arc::new(Mutex::new(Held::default()));
        let listening = held.clone();
        let events = host.run(move |client| listen(client, listening));
        let _events = cx.spawn(async move |this, cx| {
            _ = events.await;
            _ = this.update(cx, |_, cx| cx.notify());
        });
        cx.on_release(|this: &mut DaemonLink, cx| {
            let mut held = this.held.lock().unwrap();
            held.released = true;
            let Some(channel) = held.let_go() else { return };
            drop(held);
            let closing =
                this.host.run(move |client| async move { client.call(DaemonCloseRequest(DaemonOpenReply { channel })).await.map(|_| ()) });
            cx.background_spawn(async move { _ = closing.await }).detach();
        })
        .detach();
        DaemonLink { host, workspace_id, held, pins: HashMap::new(), dial: Arc::new(Dial::new(())), _events }
    }

    /// Whether a proven channel is open now; false before the first dial and after the host closed it.
    pub fn up(&self) -> bool {
        self.held.lock().unwrap().channel.is_some()
    }

    /// The folder `thread`'s panes stay in while the agent moves, where the person pinned them.
    pub fn pinned(&self, thread: &str) -> Option<&str> {
        self.pins.get(thread).map(String::as_str)
    }

    /// Pins `thread`'s panes to `folder`; every pane holding the link hears it through its notify.
    pub fn pin(&mut self, thread: &str, folder: &str, cx: &mut Context<Self>) {
        self.pins.insert(thread.to_owned(), folder.to_owned());
        cx.notify();
    }

    pub fn unpin(&mut self, thread: &str, cx: &mut Context<Self>) {
        if self.pins.remove(thread).is_some() {
            cx.notify();
        }
    }

    /// The folder the panes show, as `selectRoot` in `files/root.ts` picks it: the pin, else the thread's own folder,
    /// else the project's folder once the hello lists it among the roots, else the daemon's home; none before.
    pub fn folder(&self, thread: Option<&str>, followed: Option<&str>, project: Option<&str>) -> Option<String> {
        let root = self.root();
        thread.and_then(|t| self.pinned(t)).or(followed).or(project.filter(|_| root.is_some())).map(str::to_owned).or(root)
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
    held: &Mutex<Held>,
    dial: &Dial<()>,
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

async fn open(client: &Arc<HostClient>, workspace_id: &str, held: &Mutex<Held>, dial: &Dial<()>) -> Result<String, Refused> {
    if let Some(channel) = held.lock().unwrap().channel.clone() {
        return Ok(channel);
    }
    let _dialling = dial.lock().await;
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

/// Reads the host's pushes for the link's channel, the hello's folder and the close that drops it, for the link's
/// whole life: once the socket's stream ends, its channel went with it, and the pushes are read again a second later.
async fn listen(client: Arc<HostClient>, held: Arc<Mutex<Held>>) -> Result<(), Failure> {
    loop {
        if let Ok(mut events) = client.events().await {
            held.lock().unwrap().listening = true;
            while let Some(event) = events.next().await {
                let Ok(Event::Daemon(event)) = event else { continue };
                // The union's variants differ by a `type` word typify does not name them by, so the frame is read by its keys.
                let Ok(frame) = serde_json::to_value(&*event) else { continue };
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
            held.let_go();
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
