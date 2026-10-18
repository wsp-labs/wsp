// SPDX-License-Identifier: AGPL-3.0-only
//! What the page follows live while it is drawn, after `activity/live.tsx` and the web's daemon wiring: the threads
//! (the host's workspaces and sessions, with the rows the host pushes laid on them), and one computer's daemon link,
//! which carries its process frames, its ports and its memory readings, beside the app's own processes where the host
//! runs. The page that opened a channel owns its retry (`daemon-channel.ts`), so the link pings, notices a channel
//! that is gone and dials again as `daemon-link.ts` does. Each road is a task on the host's runtime held by the page's
//! state, so dropping that state closes the link and ends the watches.

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

/// `linkBackoffMs` (`wire/limits.ts`): the wait before dial `attempt`, half a second doubling to ten, with up to a
/// quarter second of jitter.
pub fn backoff(attempt: u32, jitter: f64) -> Duration {
    let base = 500u64.saturating_mul(1 << attempt.saturating_sub(1).min(5)).min(10_000);
    Duration::from_millis(base + (jitter.clamp(0., 1.) * 249.) as u64)
}

fn jitter() -> f64 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    f64::from(nanos % 1000) / 1000.
}

pub(crate) async fn wait(attempt: u32) {
    tokio::time::sleep(backoff(attempt, jitter())).await;
}

/// The host's workspaces and sessions as JSON, the page reading the few fields it needs.
#[derive(Clone, Default)]
pub struct Listing {
    pub workspaces: Vec<Value>,
    pub sessions: Vec<Value>,
}

fn field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

impl Listing {
    /// `withPushedRow`: a pushed row in the place of the row under its id, or after the rest as a new turn; a push
    /// with no row takes that row out, and one that leaves out the opening prompt keeps the one the thread holds.
    fn push_row(&mut self, e: &Value) {
        let Some(id) = field(e, "id") else { return };
        let at = self.sessions.iter().position(|r| field(r, "id") == Some(id));
        let Some(row) = e.get("row").filter(|r| r.is_object()) else {
            if let Some(at) = at {
                self.sessions.remove(at);
            }
            return;
        };
        let mut row = row.clone();
        if row.get("workspaceId").is_none() {
            if let Some(w) = e.get("workspaceId") {
                row["workspaceId"] = w.clone();
            }
        }
        if row.get("prompt").is_none() {
            let thread = field(&row, "threadId");
            let held = at.and_then(|at| self.sessions[at].get("prompt")).or_else(|| {
                let thread = thread?;
                self.sessions.iter().find(|r| field(r, "threadId") == Some(thread)).and_then(|r| r.get("prompt"))
            });
            if let Some(prompt) = held.cloned() {
                row["prompt"] = prompt;
            }
        }
        match at {
            Some(at) => self.sessions[at] = row,
            None => self.sessions.push(row),
        }
    }

    fn set_workspace(&mut self, workspace: &Value) {
        let Some(id) = field(workspace, "id") else { return };
        self.workspaces.retain(|w| field(w, "id") != Some(id));
        self.workspaces.push(workspace.clone());
    }

    fn drop_workspace(&mut self, id: &str) {
        self.workspaces.retain(|w| field(w, "id") != Some(id));
        self.sessions.retain(|s| field(s, "workspaceId") != Some(id));
    }

    fn set_sessions_of(&mut self, id: &str, rows: Vec<Value>) {
        self.sessions.retain(|s| field(s, "workspaceId") != Some(id));
        self.sessions.extend(rows);
    }
}

/// The least time between two listings handed to the page, however fast the rows move.
const LIST_EVERY: Duration = Duration::from_millis(1500);

fn to_values<T: serde::Serialize>(rows: &[T]) -> Vec<Value> {
    rows.iter().filter_map(|r| serde_json::to_value(r).ok()).collect()
}

async fn listing(host: &HostClient) -> Option<Listing> {
    let Workspaces { workspaces } = host.call(req::<WorkspacesListRequest>(json!({}))).await.ok()?;
    let Sessions { sessions } = host.call(req::<SessionsListRequest>(json!({}))).await.ok()?;
    Some(Listing { workspaces: to_values(&workspaces), sessions: to_values(&sessions) })
}

/// What one host event does to the listing, as the web's store applies it: whether the listing moved.
async fn apply(host: &HostClient, listing: &mut Listing, e: &Value) -> bool {
    match field(e, "type") {
        Some("session.row") => listing.push_row(e),
        Some("workspace.created") => match e.get("workspace") {
            Some(w) => listing.set_workspace(w),
            None => return false,
        },
        Some("workspace.deleted") => match field(e, "workspaceId") {
            Some(id) => listing.drop_workspace(id),
            None => return false,
        },
        Some("thread.rewound") => {
            let Some(id) = field(e, "workspaceId") else { return false };
            let Ok(Sessions { sessions }) = host.call(req::<SessionsListRequest>(json!({ "workspaceId": id }))).await else { return false };
            listing.set_sessions_of(id, to_values(&sessions));
        }
        _ => return false,
    }
    true
}

/// Reads the lists, then lays on them each row the host pushes, handing them to the page at most once every
/// `LIST_EVERY`; a socket that ended is subscribed to and read again whole.
pub fn follow_lists(host: Arc<HostClient>, runtime: &Runtime, tx: UnboundedSender<Listing>) -> Held {
    let task = runtime.spawn(async move {
        let mut attempt = 0;
        loop {
            if !lists_once(&host, &tx, &mut attempt).await {
                return;
            }
            attempt += 1;
            wait(attempt).await;
        }
    });
    Held { task: task.abort_handle(), close: None }
}

/// One subscription's life; false once the page has gone.
async fn lists_once(host: &HostClient, tx: &UnboundedSender<Listing>, attempt: &mut u32) -> bool {
    let Ok(mut events) = host.events().await else { return true };
    let Some(mut held) = listing(host).await else { return true };
    *attempt = 0;
    if tx.unbounded_send(held.clone()).is_err() {
        return false;
    }
    let mut sent_at = Instant::now();
    let mut moved = false;
    loop {
        let due = tokio::time::Instant::from_std(sent_at + LIST_EVERY);
        tokio::select! {
            next = events.next() => {
                let Some(next) = next else { return true };
                let Ok(Event::Host(e)) = next else { continue };
                let Ok(e) = serde_json::to_value(&*e) else { continue };
                moved |= apply(host, &mut held, &e).await;
            }
            _ = tokio::time::sleep_until(due), if moved => {
                moved = false;
                sent_at = Instant::now();
                if tx.unbounded_send(held.clone()).is_err() {
                    return false;
                }
            }
        }
    }
}

/// Which link a computer is read over: this computer's own, or a box's through one of its folders.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Place(String),
    Workspace(String),
}

/// What the link brings the page.
pub enum Arrival {
    /// The link is open on this channel, a ping proved it.
    Opened(String),
    /// A dial failed or the open channel was lost; the link dials again on its own.
    Down,
    /// A daemon event: a process frame, a port opening or closing, a reading.
    Event(Value),
    Ports(Vec<KnownPort>),
    /// The answer to a proc.watch: none where it streams, else the daemon's refusal.
    Watch(Option<String>),
    App(Vec<AppPart>),
}

/// The cgroups the page's process watch reads, none for every process: the page changes it after the link opened, and
/// each dial asks for what it holds then.
pub type Reads = Arc<Mutex<Option<Vec<String>>>>;

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

/// How often an open channel is pinged; one ping unanswered by the next beat abandons it (`DEFAULT_HEARTBEAT_MS`).
const BEAT: Duration = Duration::from_secs(10);
/// The attempt a refused door waits as, the backoff's ceiling (`REFUSED_ATTEMPT`).
const REFUSED_ATTEMPT: u32 = 32;
/// The failure kinds a dial at the usual pace can mend (`PASSING_KINDS`); a failure with no kind is one nothing answered.
const PASSING_KINDS: [&str; 3] = ["transient", "concurrency", "unknown"];

/// The link's dial count, as `connectDaemonLink` keeps it: what the next wait is after a dial fails or a channel goes.
#[derive(Debug, Default)]
struct Redial {
    attempt: u32,
    refused: bool,
}

impl Redial {
    /// The attempt to wait as after an open that failed with this kind: a door's answer, and every dial while one is
    /// held, waits at the ceiling until something past the door answers.
    fn failed(&mut self, kind: Option<&str>) -> u32 {
        match kind {
            Some("reauth") => self.refused = false,
            Some(k) if !PASSING_KINDS.contains(&k) => self.refused = true,
            _ => {}
        }
        if self.refused {
            return REFUSED_ATTEMPT;
        }
        self.attempt += 1;
        self.attempt
    }

    /// The attempt to wait as after an open channel was lost.
    fn lost(&mut self) -> u32 {
        self.refused = false;
        self.attempt += 1;
        self.attempt
    }

    fn live(&mut self) {
        self.attempt = 0;
        self.refused = false;
    }
}

/// The heartbeat: a ping each beat, and the channel abandoned at a beat that finds the last ping unanswered.
#[derive(Debug, Default)]
struct Beat {
    awaiting: bool,
}

impl Beat {
    /// Whether to ping now; false means the last ping went a whole beat unanswered.
    fn tick(&mut self) -> bool {
        if self.awaiting {
            return false;
        }
        self.awaiting = true;
        true
    }

    fn pong(&mut self) {
        self.awaiting = false;
    }
}

/// The channel the host answered with, for the hold's close to end, and whether the hold is gone, so an open that
/// lands after it is ended at once (`daemon-link.ts`, `if (closed)` after the open).
#[derive(Default)]
struct Open {
    closed: bool,
    channel: Option<String>,
}

type Shared = Arc<Mutex<Open>>;

fn close_channel(host: &Arc<HostClient>, channel: String) {
    let host = host.clone();
    tokio::spawn(async move {
        _ = host.call(req::<DaemonCloseRequest>(json!({ "channel": channel }))).await;
    });
}

/// Opens the computer's link and follows it: the process watch over what `reads` holds, the ports, the readings, and
/// where the host runs here, the app's own processes. A lost channel is closed and dialled again with the backoff; the
/// channel is closed and the app's watch ended when the hold is dropped.
pub fn follow_link(
    host: Arc<HostClient>,
    runtime: &Runtime,
    target: Target,
    here: bool,
    reads: Reads,
    tx: UnboundedSender<Arrival>,
) -> Held {
    let shared: Shared = Arc::default();
    let held = shared.clone();
    let closer = host.clone();
    let rt = runtime.handle().clone();
    let task = runtime.spawn(async move {
        let mut redial = Redial::default();
        loop {
            let Some(attempt) = link_once(&host, &target, here, &reads, &held, &tx, &mut redial).await else { return };
            if tx.unbounded_send(Arrival::Down).is_err() {
                return;
            }
            wait(attempt).await;
        }
    });
    let close = move || {
        let channel = {
            let mut open = shared.lock().unwrap();
            open.closed = true;
            open.channel.take()
        };
        rt.spawn(async move {
            if let Some(channel) = channel {
                _ = send(&closer, &channel, json!({ "op": "proc.unwatch" })).await;
                _ = closer.call(req::<DaemonCloseRequest>(json!({ "channel": channel }))).await;
            }
            if here {
                _ = closer.call(req::<AppProcsUnwatchRequest>(json!({}))).await;
            }
        });
    };
    Held { task: task.abort_handle(), close: Some(Box::new(close)) }
}

/// One dial and the channel's life: the attempt to wait as before the next dial, or none once the page has gone.
async fn link_once(
    host: &Arc<HostClient>,
    target: &Target,
    here: bool,
    reads: &Reads,
    shared: &Shared,
    tx: &UnboundedSender<Arrival>,
    redial: &mut Redial,
) -> Option<u32> {
    // A fresh subscription each dial: one whose host socket went ended with it, and this dials the host again.
    let Ok(mut events) = host.events().await else { return Some(redial.failed(None)) };
    let ask = match target {
        Target::Place(id) => json!({ "placeId": id }),
        Target::Workspace(id) => json!({ "workspaceId": id }),
    };
    // Its own task, so an open the page leaves while it is in flight still lands here and is closed.
    let (opener, landed) = (host.clone(), shared.clone());
    let opened = tokio::spawn(async move {
        let reply = opener.call(req::<DaemonOpenRequest>(ask)).await?;
        let mut open = landed.lock().unwrap();
        if open.closed {
            close_channel(&opener, reply.channel);
            return Err(wsp_host_client::Failure::new("closed"));
        }
        open.channel = Some(reply.channel.clone());
        Ok(reply.channel)
    });
    let channel = match opened.await {
        Ok(Ok(channel)) => channel,
        Ok(Err(f)) => return Some(redial.failed(f.kind.as_deref())),
        Err(_) => return Some(redial.failed(None)),
    };
    let abandon = |channel: String| {
        let mut open = shared.lock().unwrap();
        if open.channel.as_deref() == Some(channel.as_str()) {
            open.channel = None;
        }
        drop(open);
        close_channel(host, channel);
    };
    // A ping before live, so a channel that dies on opening never reads as open.
    if send(host, &channel, json!({ "op": "ping" })).await.is_err() {
        abandon(channel);
        return Some(redial.lost());
    }
    redial.live();
    tx.unbounded_send(Arrival::Opened(channel.clone())).ok()?;
    let scope = reads.lock().unwrap().clone();
    let watch = send(host, &channel, watch_frame(scope.as_deref())).await;
    tx.unbounded_send(Arrival::Watch(watch.err())).ok()?;
    if let Ok(reply) = send(host, &channel, json!({ "op": "ports.watch" })).await {
        if let Some(ports) = ports_of_reply(&reply) {
            tx.unbounded_send(Arrival::Ports(ports)).ok()?;
        }
    }
    _ = send(host, &channel, json!({ "op": "sys.watch" })).await;
    if here {
        if let Ok(AppProcs { procs }) = host.call(req::<AppProcsWatchRequest>(json!({}))).await {
            tx.unbounded_send(Arrival::App(app_parts(&procs))).ok()?;
        }
    }
    let mut beat = Beat::default();
    let mut beats = tokio::time::interval_at(tokio::time::Instant::now() + BEAT, BEAT);
    let mut ping: Option<std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>> = None;
    loop {
        let arrival = tokio::select! {
            next = events.next() => match next {
                None => break,
                Some(Ok(Event::Daemon(e))) => {
                    let Ok(frame) = serde_json::to_value(&*e) else { continue };
                    if field(&frame, "channel") != Some(channel.as_str()) {
                        continue;
                    }
                    match (field(&frame, "type"), frame.get("event")) {
                        (Some("daemon.event"), Some(event)) => Arrival::Event(event.clone()),
                        (Some("daemon.closed"), _) => break,
                        _ => continue,
                    }
                }
                Some(Ok(Event::AppProcs(e))) if here => Arrival::App(app_parts(&e.procs)),
                _ => continue,
            },
            _ = beats.tick() => {
                if !beat.tick() {
                    break;
                }
                let (host, channel) = (host.clone(), channel.clone());
                ping = Some(Box::pin(async move { send(&host, &channel, json!({ "op": "ping" })).await.is_ok() }));
                continue;
            }
            answered = async { ping.as_mut().expect("a ping is out").await }, if ping.is_some() => {
                ping = None;
                if answered {
                    beat.pong();
                }
                continue;
            }
        };
        tx.unbounded_send(arrival).ok()?;
    }
    abandon(channel);
    Some(redial.lost())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backoff_doubles_from_half_a_second_to_ten_with_a_quarter_second_of_jitter() {
        let ms = |attempt| backoff(attempt, 0.).as_millis();
        assert_eq!((1..=7).map(ms).collect::<Vec<_>>(), vec![500, 1000, 2000, 4000, 8000, 10_000, 10_000]);
        assert_eq!(ms(0), 500);
        assert_eq!(ms(REFUSED_ATTEMPT), 10_000);
        assert_eq!(backoff(1, 1.).as_millis(), 749);
        assert!((0..50).all(|_| backoff(3, jitter()) >= Duration::from_millis(2000) && backoff(3, jitter()) < Duration::from_millis(2250)));
    }

    #[test]
    fn a_failed_dial_counts_up_and_a_live_link_starts_again() {
        let mut r = Redial::default();
        assert_eq!([r.failed(None), r.failed(Some("transient")), r.failed(Some("unknown"))], [1, 2, 3]);
        r.live();
        assert_eq!(r.failed(None), 1);
        r.live();
        assert_eq!([r.lost(), r.lost()], [1, 2]);
    }

    #[test]
    fn a_refused_door_waits_at_the_ceiling_until_something_past_it_answers() {
        let mut r = Redial::default();
        assert_eq!(r.failed(Some("not-found")), REFUSED_ATTEMPT);
        assert_eq!(r.failed(None), REFUSED_ATTEMPT, "a dial nothing answered keeps the refusal's pace");
        assert_eq!(r.failed(Some("reauth")), 1);
        assert_eq!(r.failed(Some("usage")), REFUSED_ATTEMPT);
        r.live();
        assert_eq!(r.failed(None), 1);
    }

    #[test]
    fn a_ping_unanswered_for_a_whole_beat_abandons_the_channel() {
        let mut b = Beat::default();
        assert!(b.tick());
        b.pong();
        assert!(b.tick());
        assert!(!b.tick());
    }

    #[test]
    fn a_pushed_row_lands_as_the_web_store_lays_it() {
        let mut l = Listing {
            workspaces: vec![json!({ "id": "w1" })],
            sessions: vec![json!({ "id": "a", "workspaceId": "w1", "threadId": "t", "prompt": "fix it", "status": "completed" })],
        };
        l.push_row(&json!({ "type": "session.row", "workspaceId": "w1", "threadId": "t", "id": "b",
            "row": { "id": "b", "threadId": "t", "status": "running" } }));
        assert_eq!(l.sessions.len(), 2);
        assert_eq!(l.sessions[1]["prompt"], "fix it");
        assert_eq!(l.sessions[1]["workspaceId"], "w1");
        l.push_row(
            &json!({ "type": "session.row", "workspaceId": "w1", "id": "b", "row": { "id": "b", "threadId": "t", "status": "completed" } }),
        );
        assert_eq!(l.sessions.len(), 2);
        assert_eq!(l.sessions[1]["status"], "completed");
        l.push_row(&json!({ "type": "session.row", "workspaceId": "w1", "id": "a" }));
        assert_eq!(l.sessions.iter().map(|s| s["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["b"]);
        l.drop_workspace("w1");
        assert!(l.workspaces.is_empty() && l.sessions.is_empty());
    }

    /// Runs until `done` holds, in real time.
    fn waits(what: &str, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !done() {
            assert!(Instant::now() < deadline, "never came to: {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn an_open_that_lands_after_the_page_went_closes_its_channel() {
        use crate::screens::settings::door::{self, Door};
        let door = Door::start(json!({ "daemon.open": { "reply": { "channel": "c1" }, "delayMs": 300 }, "daemon.close": { "reply": {} } }));
        let runtime = door::runtime();
        let (tx, _rx) = futures::channel::mpsc::unbounded();
        let held = follow_link(door.client(&runtime), &runtime, Target::Place("here".into()), false, Reads::default(), tx);
        waits("the open asked", || door.asked_of("daemon.open").len() == 1);
        drop(held);
        waits("the channel it answered closed", || door.asked_of("daemon.close").iter().any(|f| f["channel"] == "c1"));
    }

    #[test]
    fn a_refused_listing_is_asked_again_and_lands() {
        use crate::screens::settings::door::{self, Door};
        use futures::StreamExt as _;
        let door = Door::start(json!({ "workspaces.list": { "error": "Not now." }, "sessions.list": { "reply": { "sessions": [] } } }));
        let runtime = door::runtime();
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let _held = follow_lists(door.client(&runtime), &runtime, tx);
        waits("the first listing asked", || door.asked_of("workspaces.list").len() == 1);
        door.rules(json!({ "workspaces.list": { "reply": { "workspaces": [] } }, "sessions.list": { "reply": { "sessions": [] } } }));
        let landed = runtime.block_on(async { tokio::time::timeout(Duration::from_secs(10), rx.next()).await });
        assert!(matches!(landed, Ok(Some(_))), "the listing was asked again after the refusal and handed to the page");
        assert_eq!(door.asked_of("workspaces.list").len(), 2);
    }
}
