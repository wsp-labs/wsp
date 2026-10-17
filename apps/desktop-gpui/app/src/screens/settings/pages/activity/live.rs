// SPDX-License-Identifier: AGPL-3.0-only
//! What the page follows live while it is drawn, after `activity/live.tsx` and the web's daemon wiring: the threads
//! (the host's workspaces and sessions, read again as they move), and one computer's daemon link, which carries its
//! process frames, its ports and its memory readings, beside the app's own processes where the host runs. Each road is
//! a task on the host's runtime held by the page's state, so dropping that state closes the link and ends the watches.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedSender;
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::ops::activity::AppProcs;
use wsp_host_client::ops::sidebar::{Sessions, Workspaces};
use wsp_host_client::ops::terminal::DaemonSent;
use wsp_host_client::protocol::{
    AppProcsUnwatchRequest, AppProcsWatchRequest, DaemonCloseRequest, DaemonOpenRequest, DaemonSendRequest, SessionsListRequest,
    WorkspacesListRequest,
};
use wsp_host_client::{Event, HostClient};

use super::wire::{app_parts, ports_of_reply, AppPart, KnownPort};
use crate::screens::settings::model::req;

/// A task on the runtime, aborted with its holder, which then runs `close` once.
pub struct Held {
    task: tokio::task::AbortHandle,
    close: Option<Box<dyn FnOnce() + Send>>,
}

impl Drop for Held {
    fn drop(&mut self) {
        self.task.abort();
        if let Some(close) = self.close.take() {
            close();
        }
    }
}

/// The host's workspaces and sessions as JSON, the page reading the few fields it needs.
pub struct Listing {
    pub workspaces: Vec<Value>,
    pub sessions: Vec<Value>,
}

/// The host events that move what the threads read: a turn starting or ending, its run, a subagent, a workspace.
fn moves_threads(kind: &str) -> bool {
    matches!(
        kind,
        "session.start"
            | "session.end"
            | "session.done"
            | "session.run"
            | "session.subagent"
            | "session.row"
            | "session.starting"
            | "session.permission"
            | "session.permission.closed"
            | "thread.head"
            | "thread.marked"
            | "workspace.created"
            | "workspace.deleted"
            | "workspace.gone"
            | "workspace.renamed"
    )
}

/// The least time between two reads of the lists, however fast the events come.
const LIST_EVERY: Duration = Duration::from_millis(1500);

async fn listing(host: &HostClient) -> Option<Listing> {
    let workspaces = host.call(req::<WorkspacesListRequest>(json!({}))).await.ok()?;
    let sessions = host.call(req::<SessionsListRequest>(json!({}))).await.ok()?;
    let Workspaces { workspaces } = workspaces;
    let Sessions { sessions } = sessions;
    Some(Listing {
        workspaces: workspaces.iter().filter_map(|w| serde_json::to_value(w).ok()).collect(),
        sessions: sessions.iter().filter_map(|s| serde_json::to_value(s).ok()).collect(),
    })
}

/// Reads the lists once and again each time an event moves them, at most once every `LIST_EVERY`.
pub fn follow_lists(host: Arc<HostClient>, runtime: &Runtime, tx: UnboundedSender<Listing>) -> Held {
    let task = runtime.spawn(async move {
        let events = host.events().await;
        let Some(first) = listing(&host).await else { return };
        if tx.unbounded_send(first).is_err() {
            return;
        }
        let Ok(mut events) = events else { return };
        let mut read_at = Instant::now();
        let mut stale = false;
        while let Some(next) = events.next().await {
            if let Ok(Event::Host(e)) = &next {
                let kind = serde_json::to_value(&**e).ok().and_then(|v| v.get("type").and_then(Value::as_str).map(str::to_owned));
                stale |= kind.as_deref().is_some_and(moves_threads);
            }
            if stale && read_at.elapsed() >= LIST_EVERY {
                stale = false;
                read_at = Instant::now();
                let Some(list) = listing(&host).await else { continue };
                if tx.unbounded_send(list).is_err() {
                    return;
                }
            }
        }
    });
    Held { task: task.abort_handle(), close: None }
}

/// Which link a computer is read over: this computer's own, or a box's through one of its folders.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Place(String),
    Workspace(String),
}

/// What the link brings the page.
pub enum Arrival {
    /// The link is open on this channel.
    Opened(String),
    /// A daemon event: a process frame, a port opening or closing, a reading.
    Event(Value),
    Ports(Vec<KnownPort>),
    /// The answer to a proc.watch: none where it streams, else the daemon's refusal.
    Watch(Option<String>),
    App(Vec<AppPart>),
}

/// One frame to the daemon over the page's channel, its answer or its refusal in the daemon's words.
pub async fn send(host: &HostClient, channel: &str, frame: Value) -> Result<Value, String> {
    let sent: DaemonSent =
        host.call(req::<DaemonSendRequest>(json!({ "channel": channel, "frame": frame }))).await.map_err(|f| f.message)?;
    if sent.reply.get("ok").and_then(Value::as_bool) == Some(false) {
        let said = sent.reply.get("error").and_then(Value::as_str).unwrap_or("the daemon refused");
        return Err(said.to_owned());
    }
    Ok(sent.reply)
}

/// proc.watch, every process or the cgroups named.
pub fn watch_frame(cgroups: Option<&[String]>) -> Value {
    match cgroups {
        None => json!({ "op": "proc.watch" }),
        Some(cgroups) => json!({ "op": "proc.watch", "cgroups": cgroups }),
    }
}

/// Opens the computer's link and follows it: the process watch over `cgroups`, the ports, the readings, and where the
/// host runs here, the app's own processes. The channel is closed and the app's watch ended when the hold is dropped.
pub fn follow_link(
    host: Arc<HostClient>,
    runtime: &Runtime,
    target: Target,
    here: bool,
    cgroups: Option<Vec<String>>,
    tx: UnboundedSender<Arrival>,
) -> Held {
    let opened: Arc<Mutex<Option<String>>> = Arc::default();
    let held = opened.clone();
    let closer = host.clone();
    let rt = runtime.handle().clone();
    let task = runtime.spawn(async move {
        let Ok(mut events) = host.events().await else { return };
        let ask = match &target {
            Target::Place(id) => json!({ "placeId": id }),
            Target::Workspace(id) => json!({ "workspaceId": id }),
        };
        // A link the host cannot open leaves the page reading, as the web's parked link does.
        let Ok(reply) = host.call(req::<DaemonOpenRequest>(ask)).await else { return };
        let channel = reply.channel;
        *held.lock().unwrap() = Some(channel.clone());
        if tx.unbounded_send(Arrival::Opened(channel.clone())).is_err() {
            return;
        }
        let watch = send(&host, &channel, watch_frame(cgroups.as_deref())).await;
        _ = tx.unbounded_send(Arrival::Watch(watch.err()));
        if let Ok(reply) = send(&host, &channel, json!({ "op": "ports.watch" })).await {
            if let Some(ports) = ports_of_reply(&reply) {
                _ = tx.unbounded_send(Arrival::Ports(ports));
            }
        }
        _ = send(&host, &channel, json!({ "op": "sys.watch" })).await;
        if here {
            if let Ok(AppProcs { procs }) = host.call(req::<AppProcsWatchRequest>(json!({}))).await {
                let list = serde_json::to_value(&procs).unwrap_or(Value::Null);
                _ = tx.unbounded_send(Arrival::App(app_parts(&list)));
            }
        }
        while let Some(next) = events.next().await {
            let arrival = match next {
                Ok(Event::Daemon(e)) => {
                    let Ok(frame) = serde_json::to_value(&*e) else { continue };
                    if frame.get("channel").and_then(Value::as_str) != Some(channel.as_str()) {
                        continue;
                    }
                    match frame.get("event") {
                        Some(event) => Arrival::Event(event.clone()),
                        None => continue,
                    }
                }
                Ok(Event::AppProcs(e)) if here => {
                    let list = serde_json::to_value(&e.procs).unwrap_or(Value::Null);
                    Arrival::App(app_parts(&list))
                }
                _ => continue,
            };
            if tx.unbounded_send(arrival).is_err() {
                return;
            }
        }
    });
    let close = move || {
        let channel = opened.lock().unwrap().take();
        rt.spawn(async move {
            if let Some(channel) = channel {
                _ = closer.call(req::<DaemonCloseRequest>(json!({ "channel": channel }))).await;
            }
            if here {
                _ = closer.call(req::<AppProcsUnwatchRequest>(json!({}))).await;
            }
        });
    };
    Held { task: task.abort_handle(), close: Some(Box::new(close)) }
}
