// SPDX-License-Identifier: AGPL-3.0-only
//! A computer's readings while a pane draws them, after `machine/live.ts`, `machine/hostLive.ts` and the `sys.watch`
//! of `terminal/wiring.ts`. This computer's own workspace is read by the host and pushed as `workspace.sys` once asked
//! with `sys.subscribe`, which the host keeps once per socket and key, so the panes on one workspace share one ask,
//! made by the first and taken back by the last. Every other computer reads itself through its daemon, asked with
//! `sys.watch` down each pane's own channel of `daemon.open`. What arrived is kept per computer past the pane, so a
//! pane opened again draws it at once.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc;
use futures::channel::oneshot;
use futures::{FutureExt as _, StreamExt as _};
use gpui_kit::{App, AppContext as _, Context, Entity, Global, Task};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use wsp_host_client::protocol::{DaemonCloseRequest, DaemonOpenRequest, DaemonSendRequest, SysSubscribeRequest, SysUnsubscribeRequest};
use wsp_host_client::{Event, Failure, HostClient, Op};

use super::model::{Ring, Sample};
use crate::screens::Host;

/// How many computers' readings are kept at once; the one read longest ago goes first.
pub const KEPT: usize = 32;
/// The daemon link's app-level heartbeat.
const BEAT: Duration = Duration::from_secs(10);
/// How long a link that went down waits before it dials again.
const AGAIN: Duration = Duration::from_secs(2);

/// Whose readings: a workspace by its id, or this computer by the place the right panel shows with no thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Workspace(String),
    Here,
}

impl Target {
    /// The key its readings are kept under, `HERE_KEY` for this computer as the web keys it.
    pub fn key(&self) -> String {
        match self {
            Target::Workspace(id) => id.clone(),
            Target::Here => "place:here".to_owned(),
        }
    }
}

/// One computer's readings as they stand.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Readings {
    pub ring: Ring,
    /// Whether the road the readings ride is up; the newest sample is the last one before it went otherwise.
    pub live: bool,
    /// The daemon's refusal of `sys.watch`, which the slots say as unavailable.
    pub unavailable: Option<String>,
}

/// Every computer's kept readings and the host's asks the panes share.
#[derive(Default)]
pub struct Live {
    kept: Vec<(String, Readings)>,
    hosted: HashMap<String, Hosted>,
    /// A key's last `sys.unsubscribe`, which its next `sys.subscribe` waits out so the two reach the socket in order.
    taken_back: HashMap<String, oneshot::Receiver<()>>,
    /// The host's refusal of a key's readings, until a pane says it.
    refused: HashMap<String, String>,
    /// The keys whose refusal was said on the socket held now.
    said: HashSet<String>,
}

/// One workspace's ask of the host: how many panes hold it, and the task that reads what it sends.
struct Hosted {
    holders: usize,
    _task: Task<()>,
}

struct Held(Entity<Live>);

impl Global for Held {}

/// The app's readings.
pub fn live(cx: &mut App) -> Entity<Live> {
    if let Some(held) = cx.try_global::<Held>() {
        return held.0.clone();
    }
    let live = cx.new(|_| Live::default());
    cx.set_global(Held(live.clone()));
    live
}

impl Live {
    /// What the readings of `key` are now; nothing kept is no readings yet.
    pub fn readings(&self, key: &str) -> Readings {
        self.kept.iter().find(|(k, _)| k == key).map(|(_, r)| r.clone()).unwrap_or_default()
    }

    /// Changes what is kept of `key`, moving it to the newest end and dropping the oldest past [`KEPT`].
    pub fn keep(&mut self, key: &str, change: impl FnOnce(&mut Readings)) {
        let mut readings = self.kept.iter().position(|(k, _)| k == key).map(|at| self.kept.remove(at).1).unwrap_or_default();
        change(&mut readings);
        self.kept.push((key.to_owned(), readings));
        if self.kept.len() > KEPT {
            self.kept.remove(0);
        }
    }

    /// How many computers have readings kept.
    pub fn kept_count(&self) -> usize {
        self.kept.len()
    }

    /// How many panes hold the host's readings of `workspace_id`.
    pub fn holders(&self, workspace_id: &str) -> usize {
        self.hosted.get(workspace_id).map_or(0, |h| h.holders)
    }

    /// The host's refusal of `key`'s readings, once on each socket.
    pub fn unsaid(&mut self, key: &str) -> Option<String> {
        let words = self.refused.remove(key)?;
        self.said.insert(key.to_owned()).then_some(words)
    }

    /// Holds the host's readings of a local workspace: the first holder asks the host for them.
    pub fn hold_host(&mut self, host: &Host, workspace_id: &str, cx: &mut Context<Self>) {
        if let Some(hosted) = self.hosted.get_mut(workspace_id) {
            hosted.holders += 1;
            return;
        }
        let (tx, mut heard) = mpsc::unbounded();
        let (id, after) = (workspace_id.to_owned(), self.taken_back.remove(workspace_id));
        let run = host.run(move |client| by_host(client, id, after, tx));
        let key = workspace_id.to_owned();
        let task = cx.spawn(async move |this, cx| {
            let _run = run;
            while let Some(heard) = heard.next().await {
                if this.update(cx, |live, cx| live.hear(&key, heard, cx)).is_err() {
                    return;
                }
            }
        });
        self.hosted.insert(workspace_id.to_owned(), Hosted { holders: 1, _task: task });
    }

    /// Lets go of a hold; the last holder takes the ask back.
    pub fn release_host(&mut self, host: &Host, workspace_id: &str, cx: &mut Context<Self>) {
        let Some(hosted) = self.hosted.get_mut(workspace_id) else { return };
        hosted.holders -= 1;
        if hosted.holders > 0 {
            return;
        }
        self.hosted.remove(workspace_id);
        self.keep(workspace_id, |r| r.live = false);
        let (done, after) = oneshot::channel();
        self.taken_back.insert(workspace_id.to_owned(), after);
        let id = workspace_id.to_owned();
        let ask = host.run(move |client| async move {
            let answer = call::<SysUnsubscribeRequest>(&client, json!({ "workspaceId": id })).await.map(drop);
            _ = done.send(());
            answer
        });
        cx.background_spawn(ask).detach();
        cx.notify();
    }

    /// Takes in what a watch heard of `key`.
    pub fn hear(&mut self, key: &str, heard: Heard, cx: &mut Context<Self>) {
        match heard {
            Heard::Sample(sample) => self.keep(key, |r| r.ring.push(sample)),
            Heard::Live(live) => {
                self.keep(key, |r| r.live = live);
                if !live {
                    self.said.remove(key);
                }
            }
            Heard::Watch(refused) => self.keep(key, |r| r.unavailable = refused),
            Heard::Refused(words) => {
                self.keep(key, |r| r.live = false);
                self.refused.insert(key.to_owned(), words);
            }
        }
        cx.notify();
    }
}

/// What a watch hears, in order.
#[derive(Debug)]
pub enum Heard {
    Sample(Sample),
    /// The road is up, or went down.
    Live(bool),
    /// The daemon answered `sys.watch`: none, or its refusal.
    Watch(Option<String>),
    /// The host refused `sys.subscribe`, with its words.
    Refused(String),
}

async fn call<O: Op + DeserializeOwned>(client: &HostClient, params: Value) -> Result<O::Reply, Failure> {
    let op: O = serde_json::from_value(params).map_err(|e| Failure::new(format!("{}: {e}", O::NAME)))?;
    client.call(op).await
}

fn sample_of(value: impl serde::Serialize) -> Option<Sample> {
    serde_json::from_value(serde_json::to_value(value).ok()?).ok()
}

/// The daemon's answer to one frame: none when it took it, else its words.
fn refusal_of(reply: &Value) -> Option<String> {
    if reply.get("ok").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    Some(reply.get("error").and_then(Value::as_str).unwrap_or("refused").to_owned())
}

/// Asks the host for a local workspace's readings and sends each one, until the receiver goes. A refusal is said
/// once and the socket left alone until it closes; a socket that comes back is asked again.
async fn by_host(
    client: Arc<HostClient>,
    workspace_id: String,
    after: Option<oneshot::Receiver<()>>,
    tx: mpsc::UnboundedSender<Heard>,
) -> Result<(), Failure> {
    if let Some(after) = after {
        _ = after.await;
    }
    loop {
        let mut events = client.events().await?;
        match call::<SysSubscribeRequest>(&client, json!({ "workspaceId": workspace_id })).await {
            Ok(_) => _ = tx.unbounded_send(Heard::Live(true)),
            Err(refused) => _ = tx.unbounded_send(Heard::Refused(refused.message)),
        }
        while let Some(event) = events.next().await {
            if let Ok(Event::Sys(e)) = event {
                if e.workspace_id == workspace_id {
                    if let Some(sample) = sample_of(&e.sample) {
                        if tx.unbounded_send(Heard::Sample(sample)).is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }
        if tx.unbounded_send(Heard::Live(false)).is_err() {
            return Ok(());
        }
        tokio::time::sleep(AGAIN).await;
    }
}

/// Opens a daemon channel to the computer, proves it with a ping, asks `sys.watch` on it and sends each sample,
/// beating every ten seconds; a channel that closes or stops answering is dialled again.
async fn by_daemon(
    client: Arc<HostClient>,
    target: Value,
    tx: mpsc::UnboundedSender<Heard>,
    opened: Arc<std::sync::Mutex<Option<String>>>,
) {
    loop {
        let linked: Result<(), Failure> = async {
            let mut events = client.events().await?;
            let channel = call::<DaemonOpenRequest>(&client, target.clone()).await?.channel;
            *opened.lock().unwrap() = Some(channel.clone());
            let send = |frame: Value| {
                let (client, channel) = (client.clone(), channel.clone());
                async move { call::<DaemonSendRequest>(&client, json!({ "channel": channel, "frame": frame })).await.map(|sent| sent.reply) }
            };
            if let Some(refused) = refusal_of(&send(json!({ "op": "ping" })).await?) {
                return Err(Failure::new(refused));
            }
            tx.unbounded_send(Heard::Live(true)).map_err(|_| Failure::new("gone"))?;
            let watched = refusal_of(&send(json!({ "op": "sys.watch" })).await?);
            tx.unbounded_send(Heard::Watch(watched)).map_err(|_| Failure::new("gone"))?;
            loop {
                let mut beat = Box::pin(tokio::time::sleep(BEAT).fuse());
                futures::select! {
                    event = events.next().fuse() => {
                        let Some(event) = event else { return Err(Failure::new("the host closed the socket")) };
                        let Ok(Event::Daemon(e)) = event else { continue };
                        let frame = serde_json::to_value(&e).unwrap_or_default();
                        if frame["channel"].as_str() != Some(channel.as_str()) {
                            continue;
                        }
                        if frame["type"] == "daemon.closed" {
                            return Err(Failure::new(frame["reason"].as_str().unwrap_or("closed")));
                        }
                        if frame["event"]["type"] == "sys.sample" {
                            if let Some(sample) = sample_of(&frame["event"]) {
                                tx.unbounded_send(Heard::Sample(sample)).map_err(|_| Failure::new("gone"))?;
                            }
                        }
                    }
                    _ = beat => {
                        if let Some(refused) = refusal_of(&send(json!({ "op": "ping" })).await?) {
                            return Err(Failure::new(refused));
                        }
                    }
                }
            }
        }
        .await;
        if linked.is_ok() || tx.unbounded_send(Heard::Live(false)).is_err() {
            return;
        }
        let stale = opened.lock().unwrap().take();
        if let Some(channel) = stale {
            let _ = call::<DaemonCloseRequest>(&client, json!({ "channel": channel })).await;
        }
        tokio::time::sleep(AGAIN).await;
    }
}

/// A pane's watch down a daemon channel of its own: what it hears comes on the receiver, and it stops when the
/// returned future is dropped; [`close`] closes the channel.
pub fn watch_daemon(
    host: &Host,
    target: &Target,
) -> (impl std::future::Future<Output = ()> + 'static, mpsc::UnboundedReceiver<Heard>, Opened) {
    let (tx, rx) = mpsc::unbounded();
    let opened: Opened = Arc::new(std::sync::Mutex::new(None));
    let at = match target {
        Target::Workspace(id) => json!({ "workspaceId": id }),
        Target::Here => json!({ "placeId": "here" }),
    };
    let held = opened.clone();
    let run = host.run(move |client| async move {
        by_daemon(client, at, tx, held).await;
        Ok(())
    });
    (run.map(drop), rx, opened)
}

/// The daemon channel a watch holds, which [`close`] closes.
pub type Opened = Arc<std::sync::Mutex<Option<String>>>;

/// Closes a watch's daemon channel, on the host's runtime whatever holds it.
pub fn close(host: &Host, opened: &Opened, cx: &mut App) {
    let Some(channel) = opened.lock().unwrap().take() else { return };
    let ask = host.run(move |client| async move { call::<DaemonCloseRequest>(&client, json!({ "channel": channel })).await.map(drop) });
    cx.background_spawn(ask).detach();
}
