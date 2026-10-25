// SPDX-License-Identifier: AGPL-3.0-only
//! `turns-help.ts:482-863` at 65f6b89c7: starting a turn and following it, waiting on threads, restarting the host,
//! reading a thread and its head. Spec: `spec/turns-follow.md`.

use std::sync::{Arc, Mutex};

use wsp_wire::types::thread_read::ThreadMessage;
use wsp_wire::views::preferences::ThreadHead;
use wsp_wire::views::session::{SessionOrigin, ThreadView};
use wsp_wire::views::session_events::{SessionQueuedEvent, SessionSteerEvent, SessionStartingEvent};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;

use super::{Turn, Waited};
use crate::verbs::client::{BoxFuture, Frame, HostClient, VerbDeps};

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

/// `Sent` and `sent(start)` (turns-help.ts:581): the start with a fresh `requestId` (a v4 UUID) put last; the id
/// is stripped again before the frame is sent and re-added after `startedBy`.
pub fn sent(start: &Frame) -> Frame {
    let _ = start;
    todo!("turns-help.ts:581")
}

/// `begin(client, start, startedBy, news)` (turns-help.ts:537): `client.events()` once, then
/// `sessions.start {...asked, startedBy, requestId, answerHeld: true}` (the start's fields without its `requestId`,
/// in order, then those three), hearing `news.queued` and `news.steered` for frames carrying this `requestId` while
/// the start is under way and `news.starting` until it stops mattering. A reply that is not a `SessionStartResult`
/// fails with `other_version("sessions.start")`; a session with no `threadId` fails
/// `the runtime stamped no thread on the session`. Answers the turn (`notice` only where the reply carried one) and
/// the reply's `turnId`.
pub async fn begin(client: &dyn HostClient, start: &Frame, started_by: SessionOrigin, news: &FollowOn) -> Result<(Turn, String), HostError> {
    let _ = (client, start, started_by, news);
    todo!("turns-help.ts:537")
}

/// `redialUnanswered(failed, socket, redial, redialed)` (turns-help.ts:588): the socket to send a start again on
/// after `failed` ended the try before the host answered. A socket the host did not stop (close code 4001) fails with
/// `failed` as it is; one it stopped, with no `redial`, fails `NOT_DELIVERED_LINE`; with one, `redialed` is called and
/// the dial answers, a dial that fails being `NOT_DELIVERED_LINE` too.
pub async fn redial_unanswered(
    failed: HostError,
    socket: Arc<dyn HostClient>,
    redial: Option<Redial<'_>>,
    redialed: Option<&(dyn Fn() + Send + Sync)>,
) -> Result<Arc<dyn HostClient>, HostError> {
    let _ = (failed, socket, redial, redialed);
    todo!("turns-help.ts:588")
}

/// `startDetached(client, start, startedBy, onQueued, redial)` (turns-help.ts:601): answers once the turn is
/// started, hearing `session.queued` for a start held behind the thread's running turn. One `requestId` for every
/// send of the start, so a start sent again after the host stopped under it is the same message to the host that
/// comes back.
pub async fn start_detached(
    client: Arc<dyn HostClient>,
    start: &Frame,
    started_by: SessionOrigin,
    on_queued: Option<Arc<dyn Fn(&SessionQueuedEvent) + Send + Sync>>,
    redial: Option<Redial<'_>>,
) -> Result<Turn, HostError> {
    let _ = (client, start, started_by, on_queued, redial);
    todo!("turns-help.ts:601")
}

/// `follow(client, start, startedBy, on, redial)` (turns-help.ts:624): starts the turn (`followed: true` added
/// to the start) and follows it to its reply. Events are picked by the turn's id (`session.*` frames whose `turnId`
/// is the reply's). The follow ends at `session.done` (which sets `turn.result`), or at `session.end` (which sets
/// `turn.reason` where one rode it); `session.start` with `afterCut` sets `turn.afterCut`. Nothing after the first
/// done or end is handed on. A host that stops under the follow once the turn is named is dialled again through
/// `redial` (`on.redialed` first): `events()` on the new socket, then the thread's whole transcript is read
/// (`sessions.history {workspaceId, threadId, whole: true}`) and any done or end of this turn in it is taken; and a host that stops
/// before the start is answered is `redial_unanswered`. The held frames are stopped on every road out. Fails with
/// `close_words()` when the socket goes any other way.
pub async fn follow(client: Arc<dyn HostClient>, start: &Frame, started_by: SessionOrigin, on: &FollowOn, redial: Option<Redial<'_>>) -> Result<Turn, HostError> {
    let _ = (client, start, started_by, on, redial);
    todo!("turns-help.ts:624")
}

/// `waitThrough(deps, named, timeoutMs, redialed)` (turns-help.ts:685): `first_ended` on the host `deps.client()`
/// dials; a host that stops under the wait (close code 4001) is dialled again (`redialed` first) within
/// `min(deps.host_wait_ms or SERVICE_WAIT_MS, what is left of the deadline)`; a deadline that passes while the host is
/// still coming back is `Waited::TimedOutMs`, any other refusal stands. An end of a sub-thread is answered with
/// what the done after it read off its machine (`sessions.head {threadId, afterRead: true}`, its `facts.sub.result`).
pub async fn wait_through(deps: &VerbDeps, named: &[ThreadView], timeout_ms: Option<u64>, redialed: Option<&(dyn Fn() + Send + Sync)>) -> Result<Waited, HostError> {
    let _ = (deps, named, timeout_ms, redialed);
    todo!("turns-help.ts:685")
}

/// `hostDidNotStopLine(waitMs)` (turns-help.ts:708): `the host took the restart and was still serving after <fmt_duration>`.
pub fn host_did_not_stop_line(wait_ms: u64) -> String {
    let _ = wait_ms;
    todo!("turns-help.ts:708")
}

/// `hostRestartedLine(running)` (turns-help.ts:712): `the host restarted and serves again; ` then `no thread is
/// running on it`, or `1 thread is still running on it: <id>`, or `<n> threads are still running on it: <ids>`, each
/// id its first 8 characters, comma and space between.
pub fn host_restarted_line(running: &[String]) -> String {
    let _ = running;
    todo!("turns-help.ts:712")
}

/// `restartHost(deps)` (turns-help.ts:718): `host.restart` on `deps.client()`'s socket, then waits for that socket
/// to close or `SERVICE_WAIT_MS` (20 s) to pass: past it fails with `host_did_not_stop_line(SERVICE_WAIT_MS)`.
/// Then `host_back(deps)` and the ids of the threads `status == running` there, in the listing's order.
pub async fn restart_host(deps: &VerbDeps) -> Result<Vec<String>, HostError> {
    let _ = deps;
    todo!("turns-help.ts:718")
}

/// `withSubResult(client, named, ended)` (turns-help.ts:739): `sessions.head {threadId: row.id, afterRead: true}`
/// for a named thread that is a sub-thread, its `facts.sub.result` added to the end where there is one; any other
/// thread's end as it came, asking nothing.
pub async fn with_sub_result(client: &dyn HostClient, named: &[ThreadView], ended: super::Ended) -> Result<super::Ended, HostError> {
    let _ = (client, named, ended);
    todo!("turns-help.ts:739")
}

/// `endedOf(client, workspaceId, threadId, ended)` (turns-help.ts:753): the thread's latest turn as its
/// transcript ends it (`thread_result` of `history`), else `ended`.
pub async fn ended_of(client: &dyn HostClient, workspace_id: &str, thread_id: &str, ended: wsp_wire::views::session_events::TurnResult) -> Result<super::Ended, HostError> {
    let _ = (client, workspace_id, thread_id, ended);
    todo!("turns-help.ts:753")
}

/// `history(client, workspaceId, threadId)` (turns-help.ts:759): `sessions.history {workspaceId, threadId, whole: true}`, its `events`.
pub async fn history(client: &dyn HostClient, workspace_id: &str, thread_id: &str) -> Result<Vec<SessionEvent>, HostError> {
    let _ = (client, workspace_id, thread_id);
    todo!("turns-help.ts:759")
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
    let _ = (client, thread, last);
    todo!("turns-help.ts:765")
}

/// `readLine(read, last)` (turns-help.ts:774): no messages is `no_reply_line` (with `last`) or `no_messages_line`,
/// else `thread_read_text(messages)`. `offset_seconds_at` is the person's clock (UTC offset in seconds at an
/// instant), which JS reads off `TZ` itself.
pub fn read_line(read: &ThreadRead, last: bool, offset_seconds_at: &dyn Fn(f64) -> i64) -> String {
    let _ = (read, last, offset_seconds_at);
    todo!("turns-help.ts:774")
}

/// `threadHead(client, thread)` (turns-help.ts:778): `sessions.head {threadId: thread.id}`, only its `facts`,
/// `events`, `pos` and `total` kept.
pub async fn thread_head(client: &dyn HostClient, thread: &ThreadView) -> Result<ThreadHead, HostError> {
    let _ = (client, thread);
    todo!("turns-help.ts:778")
}

/// `headLine(head)` (turns-help.ts:785): the title; `<harness>[ on <model>], <permissionMode>?, <status>` joined
/// by `, `; the cwd where there is one; `the newest <events.len()> of <total> events`; and, where the events read as
/// messages, an empty line then `thread_read_text` of them; joined by newlines.
pub fn head_line(head: &ThreadHead, offset_seconds_at: &dyn Fn(f64) -> i64) -> String {
    let _ = (head, offset_seconds_at);
    todo!("turns-help.ts:785")
}

/// `firstEnded(client, named, timeoutMs)` (turns-help.ts:799): `events()`, then the listing (`threads`), after the
/// subscription, so an end between the two is a held frame. A named thread the listing lacks is over: answered
/// `{status: failed}` read through `ended_of`. The first named thread (in the order named) the listing shows not
/// running is answered through `ended_of` with its status. Else the first `session.done` or `session.end` pushed for
/// any named thread: a done carries its result; an end is read through `ended_of` as `{status: failed, error:
/// <reason>}` (error only where the end gave a reason). `timeout_ms` passing first is `Waited::TimedOutMs`. The
/// socket going first fails with `close_words()`. Held frames and the timer are stopped on every road out.
pub async fn first_ended(client: &dyn HostClient, named: &[ThreadView], timeout_ms: Option<u64>) -> Result<Waited, HostError> {
    let _ = (client, named, timeout_ms);
    todo!("turns-help.ts:799")
}
