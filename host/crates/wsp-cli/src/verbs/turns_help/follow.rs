// SPDX-License-Identifier: AGPL-3.0-only
//! `turns-help.ts:482-863` at 65f6b89c7: starting a turn and following it, waiting on threads, restarting the host,
//! reading a thread and its head. Spec: `spec/turns-follow.md`.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rand_core::{OsRng, RngCore};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use wsp_wire::types::check::parse;
use wsp_wire::types::thread_read::ThreadMessage;
use wsp_wire::views::preferences::ThreadHead;
use wsp_wire::views::session::{SessionOrigin, SessionStatus, ThreadView};
use wsp_wire::views::session_events::{SessionQueuedEvent, SessionStartingEvent, SessionSteerEvent, TurnResult};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::sub_row::SubResult;
use wsp_wire::wire::runtime::{SessionStartOutcome, SessionStartResult};
use wsp_words::exit::HostError;
use wsp_words::thread_read::{thread_messages, thread_reply_rows, thread_result};
use wsp_words::words::thread::NOT_DELIVERED_LINE;
use wsp_words::words::turn::{no_messages_line, no_reply_line, thread_read_text};
use wsp_words::words::units::fmt_duration;

use super::picks::reply_field;
use super::{Ended, Turn, Waited};
use crate::host_lock::SERVICE_WAIT_MS;
use crate::verbs::client::{
    host_back, other_version, pick, stopped_under, thread_id_of, until_settled, BoxFuture, Frame, FrameOff, FrameSink, HostClient,
    PushedFrames, VerbDeps,
};
use crate::verbs::workspaces_help::threads;

/// The live turn `follow` fills in as its events land (turns-help.ts:624): `on.started` is handed it, and whoever
/// waited on the turn reads its end off it.
pub type SharedTurn = Arc<Mutex<Turn>>;

/// `redial` (turns-help.ts:601, 620): dials the host again after it stopped under the start or the follow.
pub type Redial<'a> = &'a (dyn Fn() -> BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>> + Send + Sync);

/// `StartNews` (turns-help.ts:518-526) and `follow`'s `on` (turns-help.ts:620): what a start's pushed frames say.
/// `queued` hears every `session.queued` the start's own `requestId` minted and `steered` every `session.steer`
/// before the start is answered; `starting` hears `session.starting` until the session starts, the message steers a
/// running turn, or the named turn ends; `started` hears the turn the moment it is named; `redialed` hears a host
/// that stopped under the follow and was dialled again; `event` hears every session event of the turn, with the
/// turn so far.
#[derive(Clone, Default)]
pub struct FollowOn {
    pub queued: Option<Arc<dyn Fn(&SessionQueuedEvent) + Send + Sync>>,
    pub starting: Option<Arc<dyn Fn(&SessionStartingEvent) + Send + Sync>>,
    pub steered: Option<Arc<dyn Fn(&SessionSteerEvent) + Send + Sync>>,
    pub started: Option<Arc<dyn Fn(&SharedTurn) + Send + Sync>>,
    pub redialed: Option<Arc<dyn Fn() + Send + Sync>>,
    pub event: Option<Arc<dyn Fn(&SessionEvent, &Turn) + Send + Sync>>,
}

/// What `follow` hands the verb for each event of the turn: the host's frame as it came, keys in its order, with the
/// turn so far.
pub type RawEvent = Arc<dyn Fn(&Frame, &Turn) + Send + Sync>;

/// A session event read off a frame; a frame the typed event cannot hold (an `afterCut` of `false`) is read again
/// without that key, since the TS hears every frame as it is.
pub fn event_of(frame: &Frame) -> Option<SessionEvent> {
    if let Ok(event) = serde_json::from_value::<SessionEvent>(Value::Object(frame.clone())) {
        return Some(event);
    }
    let mut lenient = frame.clone();
    lenient.shift_remove("afterCut");
    serde_json::from_value::<SessionEvent>(Value::Object(lenient)).ok()
}

/// `Sent` and `sent(start)` (turns-help.ts:581): the start with a fresh `requestId` (a v4 UUID) put last; the id
/// is stripped again before the frame is sent and re-added after `startedBy`.
pub fn sent(start: &Frame) -> Frame {
    let mut going = start.clone();
    going.insert("requestId".into(), json!(v4_uuid()));
    going
}

fn v4_uuid() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

fn is_session_event(frame: &Frame) -> bool {
    frame.get("type").and_then(Value::as_str).is_some_and(|kind| kind.starts_with("session."))
}

fn kind_of(frame: &Frame) -> Option<&str> {
    frame.get("type").and_then(Value::as_str)
}

/// Starts a turn as `startedBy` and answers with it the moment the runtime names it: what a detached start returns
/// and what a follow goes on from. The send carries its own request id so an app view with the same text in flight
/// cannot take this turn's start for its own, and so each notice is known to be this start's.
pub async fn begin(
    client: &dyn HostClient,
    start: &Frame,
    started_by: SessionOrigin,
    news: &FollowOn,
) -> Result<(Turn, String), HostError> {
    client.events().await?;
    let mut asked = start.clone();
    let request_id = asked.shift_remove("requestId").and_then(|id| id.as_str().map(str::to_owned)).unwrap_or_default();
    let (queued, steered, id) = (news.queued.clone(), news.steered.clone(), request_id.clone());
    let news_off = client.on_frame(Arc::new(move |f: &Frame| {
        if f.get("requestId").and_then(Value::as_str) != Some(id.as_str()) {
            return;
        }
        match kind_of(f) {
            Some("session.queued") => {
                if let (Some(cb), Ok(e)) = (&queued, serde_json::from_value::<SessionQueuedEvent>(Value::Object(f.clone()))) {
                    cb(&e);
                }
            }
            Some("session.steer") => {
                if let (Some(cb), Ok(e)) = (&steered, serde_json::from_value::<SessionSteerEvent>(Value::Object(f.clone()))) {
                    cb(&e);
                }
            }
            _ => {}
        }
    }));
    let turn_of: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let starting_off: Arc<Mutex<Option<FrameOff>>> = Arc::new(Mutex::new(None));
    let (starting, turn_cell, off_cell, id) = (news.starting.clone(), turn_of.clone(), starting_off.clone(), request_id.clone());
    let starting_sink: FrameSink = Arc::new(move |f: &Frame| {
        let stop = || {
            let off = off_cell.lock().unwrap().take();
            if let Some(off) = off {
                off.off();
            }
        };
        let turn = turn_cell.lock().unwrap().clone();
        if kind_of(f) == Some("session.end") && turn.is_some() && f.get("turnId").and_then(Value::as_str) == turn.as_deref() {
            return stop();
        }
        if f.get("requestId").and_then(Value::as_str) != Some(id.as_str()) {
            return;
        }
        if kind_of(f) == Some("session.starting") {
            if let (Some(cb), Ok(e)) = (&starting, serde_json::from_value::<SessionStartingEvent>(Value::Object(f.clone()))) {
                cb(&e);
            }
        }
        if matches!(kind_of(f), Some("session.starting" | "session.start" | "session.steer")) {
            stop();
        }
    });
    *starting_off.lock().unwrap() = Some(client.on_frame(starting_sink));
    let stop_starting = || {
        let off = starting_off.lock().unwrap().take();
        if let Some(off) = off {
            off.off();
        }
    };
    let mut params = asked;
    params.insert("startedBy".into(), serde_json::to_value(started_by).expect("an origin writes as JSON"));
    params.insert("requestId".into(), json!(request_id));
    params.insert("answerHeld".into(), json!(true));
    let answer = client.request("sessions.start", params).await;
    news_off.off();
    let answer = match answer {
        Ok(answer) => answer,
        Err(e) => {
            stop_starting();
            return Err(e);
        }
    };
    let Ok(reply) = parse::<SessionStartResult>(&Value::Object(answer)) else {
        stop_starting();
        return Err(HostError::new(other_version("sessions.start")));
    };
    *turn_of.lock().unwrap() = Some(reply.turn_id.clone());
    if reply.outcome == SessionStartOutcome::Steered {
        stop_starting();
    }
    let Some(thread_id) = reply.session.thread_id.clone() else {
        return Err(HostError::new("the runtime stamped no thread on the session"));
    };
    let turn = Turn {
        session: reply.session,
        thread_id,
        outcome: reply.outcome,
        result: None,
        reason: None,
        after_cut: false,
        notice: reply.notice,
    };
    Ok((turn, reply.turn_id))
}

/// The socket to send a start again on, after `failed` ended the try before the host answered it: a host that stopped
/// under it is dialled again, and the host that comes back knows the start by its request id and answers with the turn
/// it already opened where it took the message, so whether a message landed is the host's to say. Any other failure is
/// the start's own, and a host that did not come back in time, or no road back, is a message nobody took: the line says
/// so rather than that a turn goes on.
pub async fn redial_unanswered(
    failed: HostError,
    socket: Arc<dyn HostClient>,
    redial: Option<Redial<'_>>,
    redialed: Option<&(dyn Fn() + Send + Sync)>,
) -> Result<Arc<dyn HostClient>, HostError> {
    if !stopped_under(socket.as_ref()) {
        return Err(failed);
    }
    let Some(redial) = redial else {
        return Err(HostError::new(NOT_DELIVERED_LINE));
    };
    if let Some(redialed) = redialed {
        redialed();
    }
    redial().await.map_err(|_| HostError::new(NOT_DELIVERED_LINE))
}

/// A start whose caller does not stay for the reply: the turn runs on, and a wait on the thread or its notify
/// carries the end. Answers once the turn is started, so a start queued behind the thread's running turn answers
/// when that turn has ended and this one began. A host that stops before it answers is dialled again through
/// `redial`, and the same start goes to the host that comes back.
pub async fn start_detached(
    client: Arc<dyn HostClient>,
    start: &Frame,
    started_by: SessionOrigin,
    on_queued: Option<Arc<dyn Fn(&SessionQueuedEvent) + Send + Sync>>,
    redial: Option<Redial<'_>>,
) -> Result<Turn, HostError> {
    let going = sent(start);
    let news = FollowOn { queued: on_queued, ..FollowOn::default() };
    let mut socket = client;
    loop {
        let answer = begin(socket.as_ref(), &going, started_by, &news).await;
        match answer {
            Ok((turn, _)) => return Ok(turn),
            Err(e) => socket = redial_unanswered(e, socket, redial, None).await?,
        }
    }
}

/// Folds one of the named turn's frames into the turn and hands it to `on.event` (turns-help.ts:633-641): a cut start,
/// a done's result, an end's reason; the frame that ends the follow sets `over`, and nothing after it is taken.
fn take(f: &Frame, turn: &SharedTurn, on: &FollowOn, raw: Option<&RawEvent>, over: &Mutex<bool>) {
    if *over.lock().unwrap() {
        return;
    }
    let kind = kind_of(f);
    let mut live = turn.lock().unwrap();
    if kind == Some("session.start") && f.get("afterCut") == Some(&Value::Bool(true)) {
        live.after_cut = true;
    }
    if kind == Some("session.done") {
        live.result = f.get("result").and_then(|result| serde_json::from_value(result.clone()).ok());
    }
    if kind == Some("session.end") {
        if let Some(reason) = f.get("reason").and_then(Value::as_str) {
            live.reason = Some(reason.to_owned());
        }
    }
    if let Some(raw) = raw {
        raw(f, &live);
    }
    if let (Some(event), Some(e)) = (&on.event, event_of(f)) {
        event(&e, &live);
    }
    drop(live);
    *over.lock().unwrap() = matches!(kind, Some("session.done") | Some("session.end"));
}

/// One try of `follow` on `socket`: the start when no turn is named yet, else the subscription again, then the turn's
/// frames until its done or end. On a host that is not the first one, the transcript is read first.
async fn follow_on(
    socket: &Arc<dyn HostClient>,
    original: &Arc<dyn HostClient>,
    going: &Frame,
    started_by: SessionOrigin,
    on: &FollowOn,
    raw: Option<&RawEvent>,
    pushed: &PushedFrames,
    named: &mut Option<(SharedTurn, String)>,
) -> Result<Turn, HostError> {
    if named.is_none() {
        let (turn, turn_id) = begin(socket.as_ref(), going, started_by, on).await?;
        let turn = Arc::new(Mutex::new(turn));
        if let Some(started) = &on.started {
            started(&turn);
        }
        *named = Some((turn, turn_id));
    } else {
        socket.events().await?;
    }
    let (turn, turn_id) = named.clone().expect("a named turn");
    let (done_tx, done_rx) = oneshot::channel::<()>();
    let done = Arc::new(Mutex::new(Some(done_tx)));
    let over = Arc::new(Mutex::new(false));
    let (live, on_c, raw_c, over_c, done_c, id) = (turn.clone(), on.clone(), raw.cloned(), over.clone(), done.clone(), turn_id.clone());
    pushed.follow(
        Arc::new(move |f: &Frame| is_session_event(f) && f.get("turnId").and_then(Value::as_str) == Some(id.as_str())),
        Arc::new(move |f: &Frame| {
            take(f, &live, &on_c, raw_c.as_ref(), &over_c);
            if *over_c.lock().unwrap() {
                let tx = done_c.lock().unwrap().take();
                if let Some(tx) = tx {
                    let _ = tx.send(());
                }
            }
        }),
    );
    if !Arc::ptr_eq(socket, original) {
        let (workspace_id, thread_id) = {
            let live = turn.lock().unwrap();
            (live.session.workspace_id.clone(), live.thread_id.clone())
        };
        for event in history_frames(socket.as_ref(), &workspace_id, &thread_id).await? {
            if let Value::Object(frame) = event {
                if frame.get("turnId").and_then(Value::as_str) == Some(turn_id.as_str())
                    && matches!(kind_of(&frame), Some("session.done") | Some("session.end"))
                {
                    take(&frame, &turn, on, raw, &over);
                }
            }
        }
        if *over.lock().unwrap() {
            let settled = turn.lock().unwrap().clone();
            return Ok(settled);
        }
    }
    let ended = Box::pin(async move {
        if done_rx.await.is_err() {
            return std::future::pending().await;
        }
        Ok(turn.lock().unwrap().clone())
    });
    until_settled(socket.as_ref(), ended).await
}

/// Starts a turn as `startedBy` and follows it to its reply: `on.queued` when the runtime says the start waits behind
/// the thread's running turn, `on.started` the thread as soon as the runtime names it, `on.event` every event of the
/// turn with the turn so far. Events are picked by the turn's id: a start that waited behind the thread's running
/// turn must not read that turn's end as its own. The follow ends at session.done, which carries the whole reply:
/// session.end follows the runtime's exit read and reap, minutes later when the machine is slow to answer. A turn the
/// runtime ended itself has no done, so its end is the last event instead. A host that stops under the follow once
/// the turn is named is dialled again through `redial`, `on.redialed` says so, and the follow goes on from the new
/// host, which re-opens the run: an end the transcript already holds is read off it, and the rest arrive as they
/// come. A host that stops before it answers the start is dialled again the same way and sent the same start, which
/// it answers with the turn it opened where it took it; none back in time is a message not delivered. Fails when the
/// host goes away any other way.
pub async fn follow(
    client: Arc<dyn HostClient>,
    start: &Frame,
    started_by: SessionOrigin,
    on: &FollowOn,
    redial: Option<Redial<'_>>,
) -> Result<Turn, HostError> {
    follow_heard(client, start, started_by, on, None, redial).await
}

/// `follow` with `raw` hearing each event's frame as the host wrote it, beside the typed `on.event`.
pub async fn follow_heard(
    client: Arc<dyn HostClient>,
    start: &Frame,
    started_by: SessionOrigin,
    on: &FollowOn,
    raw: Option<RawEvent>,
    redial: Option<Redial<'_>>,
) -> Result<Turn, HostError> {
    let mut asked = start.clone();
    asked.insert("followed".into(), json!(true));
    let going = sent(&asked);
    let original = client.clone();
    let mut socket = client;
    let mut named: Option<(SharedTurn, String)> = None;
    loop {
        let pushed = PushedFrames::new(socket.as_ref());
        let attempt = follow_on(&socket, &original, &going, started_by, on, raw.as_ref(), &pushed, &mut named).await;
        pushed.stop();
        match attempt {
            Ok(turn) => return Ok(turn),
            Err(e) => {
                if named.is_none() {
                    socket = redial_unanswered(e, socket, redial, on.redialed.as_deref()).await?;
                    continue;
                }
                let Some(redial) = redial.filter(|_| stopped_under(socket.as_ref())) else {
                    return Err(e);
                };
                if let Some(redialed) = &on.redialed {
                    redialed();
                }
                socket = redial().await?;
            }
        }
    }
}

/// The first of the named threads to leave running, as `firstEnded` answers it, dialled through a host that stops
/// under the wait: the threads went on without it, so the question is asked again of the host that comes back. A
/// deadline that passes while that host is still coming back is the deadline's answer, as it is anywhere else.
pub async fn wait_through(
    deps: &VerbDeps,
    named: &[ThreadView],
    timeout_ms: Option<u64>,
    redialed: Option<&(dyn Fn() + Send + Sync)>,
) -> Result<Waited, HostError> {
    let until = timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
    let left = move || until.map(|until| until.saturating_duration_since(Instant::now()).as_millis() as u64);
    let timed_out = || Waited::TimedOutMs(timeout_ms.unwrap_or_default() as f64);
    let mut client = deps.client(None).await?;
    loop {
        let attempt: Result<Waited, HostError> = async {
            match first_ended(client.as_ref(), named, left()).await? {
                Waited::TimedOutMs(_) => Ok(timed_out()),
                Waited::Ended(ended) => Ok(Waited::Ended(with_sub_result(client.as_ref(), named, ended).await?)),
            }
        }
        .await;
        match attempt {
            Ok(waited) => return Ok(waited),
            Err(e) => {
                if !stopped_under(client.as_ref()) {
                    return Err(e);
                }
                if let Some(redialed) = redialed {
                    redialed();
                }
                let host_wait = deps.host_wait_ms.unwrap_or(SERVICE_WAIT_MS);
                match host_back(deps, Some(host_wait.min(left().unwrap_or(host_wait)))).await {
                    Ok(back) => client = back,
                    Err(refused) => {
                        if timeout_ms.is_some() && left() == Some(0) {
                            return Ok(timed_out());
                        }
                        return Err(refused);
                    }
                }
            }
        }
    }
}

/// `hostDidNotStopLine(waitMs)` (turns-help.ts:708): `the host took the restart and was still serving after <fmt_duration>`.
pub fn host_did_not_stop_line(wait_ms: u64) -> String {
    format!("the host took the restart and was still serving after {}", fmt_duration(wait_ms as f64, None))
}

/// `hostRestartedLine(running)` (turns-help.ts:712): `the host restarted and serves again; ` then `no thread is
/// running on it`, or `1 thread is still running on it: <id>`, or `<n> threads are still running on it: <ids>`, each
/// id its first 8 characters, comma and space between.
pub fn host_restarted_line(running: &[String]) -> String {
    let ids: Vec<String> = running.iter().map(|id| id.chars().take(8).collect()).collect();
    let state = match running.len() {
        0 => "no thread is running on it".to_owned(),
        1 => format!("1 thread is still running on it: {}", ids.join(", ")),
        n => format!("{n} threads are still running on it: {}", ids.join(", ")),
    };
    format!("the host restarted and serves again; {state}")
}

/// Has the host restart on the road it came up on and answers, once the host that replaced it serves, with the
/// threads running there. The host answers the ask before it closes, so the close after the answer is the restart
/// under way; a road that would not bring it back refuses the ask in its own words, before anything stops.
pub async fn restart_host(deps: &VerbDeps) -> Result<Vec<String>, HostError> {
    let client = deps.client(None).await?;
    client.request("host.restart", Frame::new()).await?;
    let stopped = tokio::time::timeout(Duration::from_millis(SERVICE_WAIT_MS), client.closed()).await.is_ok();
    if !stopped {
        return Err(HostError::new(host_did_not_stop_line(SERVICE_WAIT_MS)));
    }
    let back = host_back(deps, None).await?;
    let rows = threads(back.as_ref(), None).await?;
    Ok(rows.iter().filter(|t| t.status == SessionStatus::Running).map(thread_id_of).collect())
}

/// A sub-thread's end with what the done after it read off its machine, asked of the host once that read is in; any
/// other thread's end as it came.
pub async fn with_sub_result(client: &dyn HostClient, named: &[ThreadView], ended: Ended) -> Result<Ended, HostError> {
    let Some(row) = named.iter().find(|t| thread_id_of(t) == ended.thread_id) else {
        return Ok(ended);
    };
    if row.sub.is_none() {
        return Ok(ended);
    }
    let frame = client.request("sessions.head", pick(vec![("threadId", Some(json!(row.id))), ("afterRead", Some(json!(true)))])).await?;
    let result = frame.get("facts").and_then(|facts| facts.get("sub")).and_then(|sub| sub.get("result")).filter(|result| !result.is_null());
    let mut ended = ended;
    if let Some(result) = result {
        ended.sub = Some(serde_json::from_value::<SubResult>(result.clone()).map_err(|_| HostError::new(other_version("sessions.head")))?);
    }
    Ok(ended)
}

/// The thread's latest turn as its transcript ends it (`thread_result` of `history`), else `ended`.
pub async fn ended_of(client: &dyn HostClient, workspace_id: &str, thread_id: &str, ended: TurnResult) -> Result<Ended, HostError> {
    let events = history(client, workspace_id, thread_id).await?;
    Ok(Ended { thread_id: thread_id.to_owned(), result: thread_result(&events, thread_id).unwrap_or(ended), sub: None })
}

/// `history(client, workspaceId, threadId)` (turns-help.ts:759): `sessions.history {workspaceId, threadId, whole: true}`, its `events`.
pub async fn history(client: &dyn HostClient, workspace_id: &str, thread_id: &str) -> Result<Vec<SessionEvent>, HostError> {
    let params = pick(vec![("workspaceId", Some(json!(workspace_id))), ("threadId", Some(json!(thread_id))), ("whole", Some(json!(true)))]);
    let frame = client.request("sessions.history", params).await?;
    reply_field(&frame, "sessions.history", "events")
}

/// `history` as the host wrote it: the `events` of `sessions.history`, each frame with its keys in the host's order.
pub async fn history_frames(client: &dyn HostClient, workspace_id: &str, thread_id: &str) -> Result<Vec<Value>, HostError> {
    let params = pick(vec![("workspaceId", Some(json!(workspace_id))), ("threadId", Some(json!(thread_id))), ("whole", Some(json!(true)))]);
    let frame = client.request("sessions.history", params).await?;
    match frame.get("events") {
        Some(Value::Array(events)) => Ok(events.clone()),
        _ => Err(HostError::new(other_version("sessions.history"))),
    }
}

/// What `readThread` answers: the thread's full id and its messages.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadRead {
    pub thread_id: String,
    pub messages: Vec<ThreadMessage>,
}

/// `readThread(client, thread, last)` (turns-help.ts:765): the transcript (`history`), then `sessions.read
/// {threadId: thread.id}` (the stamp moves, so the thread stops reading Done), the messages of `thread_messages`, or
/// with `last` the final reply's rows of `thread_reply_rows`. Nothing is woken.
pub async fn read_thread(client: &dyn HostClient, thread: &ThreadView, last: bool) -> Result<ThreadRead, HostError> {
    let thread_id = thread_id_of(thread);
    let events = history(client, &thread.workspace_id, &thread_id).await?;
    client.request("sessions.read", pick(vec![("threadId", Some(json!(thread.id)))])).await?;
    let messages = if last { thread_reply_rows(&events, &thread_id) } else { thread_messages(&events, &thread_id) };
    Ok(ThreadRead { thread_id, messages })
}

/// `readLine(read, last)` (turns-help.ts:774): no messages is `no_reply_line` (with `last`) or `no_messages_line`,
/// else `thread_read_text(messages)`. `offset_seconds_at` is the person's clock (UTC offset in seconds at an
/// instant), which JS reads off `TZ` itself.
pub fn read_line(read: &ThreadRead, last: bool, offset_seconds_at: &dyn Fn(f64) -> i64) -> String {
    if read.messages.is_empty() {
        return if last { no_reply_line(&read.thread_id) } else { no_messages_line(&read.thread_id) };
    }
    thread_read_text(&read.messages, offset_seconds_at)
}

/// What `threadHead` answers: the host's `facts`, `events`, `pos` and `total` as it wrote them (unknown keys kept, its
/// key order), and the same read into the typed head that `head_line` takes. It writes as the raw head and reads as the typed one.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadRead {
    pub typed: ThreadHead,
    pub raw: Value,
}

impl std::ops::Deref for HeadRead {
    type Target = ThreadHead;

    fn deref(&self) -> &ThreadHead {
        &self.typed
    }
}

impl serde::Serialize for HeadRead {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}

/// `threadHead(client, thread)` (turns-help.ts:778): `sessions.head {threadId: thread.id}`, only its `facts`,
/// `events`, `pos` and `total` kept, as the host wrote them.
pub async fn thread_head(client: &dyn HostClient, thread: &ThreadView) -> Result<HeadRead, HostError> {
    let frame = client.request("sessions.head", pick(vec![("threadId", Some(json!(thread.id)))])).await?;
    let typed = ThreadHead {
        facts: reply_field(&frame, "sessions.head", "facts")?,
        events: reply_field(&frame, "sessions.head", "events")?,
        pos: reply_field(&frame, "sessions.head", "pos")?,
        total: reply_field(&frame, "sessions.head", "total")?,
        trimmed: None,
    };
    let mut raw = serde_json::Map::new();
    for key in ["facts", "events", "pos", "total"] {
        if let Some(part) = frame.get(key) {
            raw.insert(key.to_owned(), part.clone());
        }
    }
    Ok(HeadRead { typed, raw: Value::Object(raw) })
}

/// `headLine(head)` (turns-help.ts:785): the title; `<harness>[ on <model>], <permissionMode>?, <status>` joined
/// by `, `; the cwd where there is one; `the newest <events.len()> of <total> events`; and, where the events read as
/// messages, an empty line then `thread_read_text` of them; joined by newlines.
pub fn head_line(head: &ThreadHead, offset_seconds_at: &dyn Fn(f64) -> i64) -> String {
    let facts = &head.facts;
    let status = serde_json::to_value(facts.status).ok().and_then(|word| word.as_str().map(str::to_owned)).unwrap_or_default();
    let runs = [match &facts.model {
        Some(model) => format!("{} on {model}", facts.harness),
        None => facts.harness.clone(),
    }]
    .into_iter()
    .chain(facts.permission_mode.clone())
    .chain([status])
    .collect::<Vec<_>>()
    .join(", ");
    let mut lines = vec![facts.title.clone(), runs];
    if let Some(cwd) = &facts.cwd {
        lines.push(cwd.clone());
    }
    lines.push(format!("the newest {} of {} events", head.events.0.len(), head.total.get()));
    let rows = thread_messages(&head.events.0, facts.thread_id.as_deref().unwrap_or(&facts.id));
    if !rows.is_empty() {
        lines.push(String::new());
        lines.push(thread_read_text(&rows, offset_seconds_at));
    }
    lines.join("\n")
}

/// The answer for a named thread a frame ended: a done carries its result as it came; an end is read through
/// `ended_of` with the failure the end names, if it names one.
async fn answer_of(client: &dyn HostClient, frame: &Frame) -> Result<Ended, HostError> {
    let thread_id = frame.get("threadId").and_then(Value::as_str).unwrap_or_default().to_owned();
    if kind_of(frame) == Some("session.done") {
        let result = serde_json::from_value::<TurnResult>(frame.get("result").cloned().unwrap_or(Value::Null))
            .map_err(|_| HostError::new(other_version("session.done")))?;
        return Ok(Ended { thread_id, result, sub: None });
    }
    let workspace_id = frame.get("workspaceId").and_then(Value::as_str).unwrap_or_default();
    ended_of(client, workspace_id, &thread_id, failed_result(frame.get("reason").and_then(Value::as_str))).await
}

/// A turn's result that says it failed, with the reason where there is one.
fn failed_result(error: Option<&str>) -> TurnResult {
    let mut result = serde_json::Map::new();
    result.insert("status".into(), json!("failed"));
    if let Some(error) = error {
        result.insert("error".into(), json!(error));
    }
    serde_json::from_value(Value::Object(result)).expect("a failed turn reads as a result")
}

/// Blocks until one of the named threads leaves running and answers with that thread's end: at once for one already
/// over, else on the first done or end the host pushes for any of them; the deadline when `timeoutMs` passes first.
/// The rows are listed after the subscription, so an end between the two is a held frame and not a gap. A named
/// thread the listing no longer holds is over too: its only turn was given up before it opened, between the naming
/// and this call, and its end went out before the subscription, so it is answered failed off what the transcript
/// holds of it, which is nothing but the status. A done carries its result; an end without one in hand is read off
/// the transcript, since the reply may have landed before this call. Fails when the host goes away first.
pub async fn first_ended(client: &dyn HostClient, named: &[ThreadView], timeout_ms: Option<u64>) -> Result<Waited, HostError> {
    let pushed = PushedFrames::new(client);
    let answer = first_ended_on(client, named, timeout_ms, &pushed).await;
    pushed.stop();
    answer
}

async fn first_ended_on(
    client: &dyn HostClient,
    named: &[ThreadView],
    timeout_ms: Option<u64>,
    pushed: &PushedFrames,
) -> Result<Waited, HostError> {
    client.events().await?;
    let rows = threads(client, None).await?;
    if let Some(gone) = named.iter().find(|t| rows.iter().all(|r| thread_id_of(r) != thread_id_of(t))) {
        return Ok(Waited::Ended(ended_of(client, &gone.workspace_id, &thread_id_of(gone), failed_result(None)).await?));
    }
    if let Some(over) = named.iter().find_map(|t| rows.iter().find(|r| r.id == t.id).filter(|r| r.status != SessionStatus::Running)) {
        let status = serde_json::to_value(over.status).expect("a status writes as JSON");
        let result = serde_json::from_value::<TurnResult>(json!({ "status": status }))
            .map_err(|_| HostError::new(other_version("sessions.list")))?;
        return Ok(Waited::Ended(ended_of(client, &over.workspace_id, &thread_id_of(over), result).await?));
    }
    let ids: HashSet<String> = named.iter().map(thread_id_of).collect();
    let (tx, rx) = oneshot::channel::<Frame>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    pushed.follow(
        Arc::new(move |f: &Frame| {
            is_session_event(f)
                && matches!(kind_of(f), Some("session.done") | Some("session.end"))
                && f.get("threadId").and_then(Value::as_str).is_some_and(|id| ids.contains(id))
        }),
        Arc::new(move |f: &Frame| {
            let tx = tx.lock().unwrap().take();
            if let Some(tx) = tx {
                let _ = tx.send(f.clone());
            }
        }),
    );
    let answer = Box::pin(async move {
        tokio::select! {
            frame = rx => match frame {
                Ok(frame) => answer_of(client, &frame).await.map(Waited::Ended),
                Err(_) => std::future::pending().await,
            },
            _ = deadline_passed(timeout_ms) => Ok(Waited::TimedOutMs(timeout_ms.unwrap_or_default() as f64)),
        }
    });
    until_settled(client, answer).await
}

/// Resolves once the deadline `timeout_ms` has passed; never, where there is none.
async fn deadline_passed(timeout_ms: Option<u64>) {
    match timeout_ms {
        Some(ms) => tokio::time::sleep(Duration::from_millis(ms)).await,
        None => std::future::pending().await,
    }
}
