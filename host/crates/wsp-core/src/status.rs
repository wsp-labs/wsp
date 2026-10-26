// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/status.ts at 65f6b89c7. Specs: spec/status_probe.md (this file),
//! spec/status_meter.md (`meter.rs`), spec/status_poll.md (`poll.rs`).
//!
//! Enriched workspace status and awake-time cost metering. Machine state is the runtime's own phase plus a reach
//! probe. The provider is asked only after a failed reach (at most once per reconcile window) or on an explicit
//! refresh: every GET /sandboxes/:id resets the provider's idle timer, so a poller that asked per tick kept every
//! workspace awake and billing forever.
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use async_trait::async_trait;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use wsp_engine::errors::EngineError;
use wsp_engine::machine::ExecResult;
use wsp_wire::types::changes::Checkout;
use wsp_wire::types::pull_request::PullRequestSeen;
use wsp_wire::types::tree::TreeFact;
use wsp_wire::views::place_more::{PlaceSpend, PlaceView};
use wsp_wire::views::workspace::{MachineFacts, MachineState, ReachState, WorkspacePhase};
use wsp_wire::views::workspace_events::WorkspaceCostEvent;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{WorkspaceStatus, WorkspaceStatusWakeAsk, WorkspaceView};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::events_union::EventUnion;

use crate::clock::{real_clock, Cancel, Clock};
use crate::types::pending::Store;
use futures::{FutureExt, StreamExt};
use std::time::Duration;
use wsp_engine::errors::road_failed;
use wsp_engine::fetch::{platform_fetch, Abort, FetchRequest};
use wsp_words::js::number_string;
use wsp_words::js_date::iso_string;

mod meter;
mod poll;

// /** The provider word the runtime's own phase implies: a wake in flight or a queued spawn is a machine starting, a pause
//  * in flight still runs, and a released sub-machine's machine is gone. */
// export function machineStateOf(phase: WorkspacePhase): MachineState {
//   switch (phase) {
//     case "running":
//     case "pausing":
//       return "running";
//     case "napping":
//       return "paused";
//     case "waking":
//     case "queued":
//       return "starting";
//     case "gone":
//     case "released":
//       return "gone";
//     default: {
//       const _exhaustive: never = phase;
//       return "running";
//     }
//   }
// }
pub fn machine_state_of(phase: WorkspacePhase) -> MachineState {
    match phase {
        WorkspacePhase::Running | WorkspacePhase::Pausing => MachineState::Running,
        WorkspacePhase::Napping => MachineState::Paused,
        WorkspacePhase::Waking | WorkspacePhase::Queued => MachineState::Starting,
        WorkspacePhase::Gone | WorkspacePhase::Released => MachineState::Gone,
    }
}

// /** What a state read makes of a record marked gone: a machine the provider still holds unmakes the verdict, running
//  * as running and paused as napping, and any other answer leaves the record gone. Every road out of gone (a wake, a
//  * rebuild, the sweep, the record load) reads it here, so none of them keeps a rule of its own. */
// export function phaseLeavingGone(read: MachineState): "running" | "napping" | undefined {
//   return read === "running" ? "running" : read === "paused" ? "napping" : undefined;
// }
/// `Running` or `Napping`, the phase a record leaves gone for; `None` leaves it gone.
pub fn phase_leaving_gone(read: MachineState) -> Option<WorkspacePhase> {
    match read {
        MachineState::Running => Some(WorkspacePhase::Running),
        MachineState::Paused => Some(WorkspacePhase::Napping),
        _ => None,
    }
}

// /** The provider's answer as a row or a log line quotes it: its status and message when the call answered with both.
//  * The typed refusal's message is wsp's own sentence, so the words quoted are the provider's it carries. */
// export function providerSaid(e: unknown): string {
//   const message = e instanceof MachineUnreachableError ? e.said : e instanceof Error ? e.message : String(e);
//   const status = (e as { status?: unknown } | null)?.status;
//   return typeof status === "number" ? `${status} ${message}` : message;
// }
pub fn provider_said(e: &EngineError) -> String {
    let message = match e {
        EngineError::MachineUnreachable { said, .. } => said.clone(),
        other => other.to_string(),
    };
    match e.status() {
        Some(status) => format!("{status} {message}"),
        None => message,
    }
}

// export interface ProbeOptions {
//   /** An answer slower than this is the edge being slow, whatever its status. */
//   promptMs: number;
//   /** No answer by this is unreachable. Must tolerate a slow edge (measured 502s after 5 to 11 s). */
//   timeoutMs: number;
// }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeOptions {
    pub prompt_ms: i64,
    pub timeout_ms: i64,
}

// /** The daemon's ws server answers a plain HTTP GET with Upgrade Required and nothing else it serves does: it is the
//  * one status that came from inside the guest. */
// const DAEMON_ANSWER = 426;
pub const DAEMON_ANSWER: u16 = 426;

// /** What one round trip found: the state to show, and whether the guest itself answered. */
// export interface Probed {
//   state: ReachState;
//   /** The daemon answered. Any other answer is the edge speaking for the machine (an edge auth refusal, a route to a
//    * paused machine), which says nothing about the guest either way, so it decides nothing on its own and only sends
//    * the caller to the provider for the machine's real state. */
//   fromDaemon: boolean;
//   /** The request failed before it left this computer (no DNS, no route out): the silence is the computer's, and
//    * says nothing about the machine. */
//   offline?: boolean;
// }
#[derive(Debug, Clone, PartialEq)]
pub struct Probed {
    pub state: ReachState,
    pub from_daemon: bool,
    /// The TS's `offline?: true`: absent is false.
    pub offline: bool,
}

// /** The silence a failed request is: the machine's, or this computer's when the request never got out. */
// export function missed(e: unknown): Probed {
//   return { state: "unreachable", fromDaemon: false, ...(roadFailed(e) ? { offline: true } : {}) };
// }
pub fn missed(e: &EngineError) -> Probed {
    Probed { state: ReachState::Unreachable, from_daemon: false, offline: road_failed(e) }
}

// /** One HTTP round trip against the minted URL. A prompt 502 means the edge
//  * dialed the guest and nothing listens on the daemon port. A late answer, 502
//  * included, is a provider slow spell: the machine is there, the edge is not
//  * keeping up. Silence is unreachable; a request that never got out is the
//  * computer offline. */
// export async function probeReach(url: string, o: ProbeOptions, now: () => number = Date.now): Promise<Probed> {
//   const started = now();
//   try {
//     const res = await fetch(url, { signal: AbortSignal.timeout(o.timeoutMs) });
//     await res.text().catch(() => "");
//     const fromDaemon = res.status === DAEMON_ANSWER;
//     if (now() - started > o.promptMs) return { state: "slow", fromDaemon };
//     return { state: res.status === 502 ? "no-daemon" : "reachable", fromDaemon };
//   } catch (e) {
//     return missed(e);
//   }
// }
/// One GET through `wsp_engine::fetch::platform_fetch`, capped at `o.timeout_ms`.
pub async fn probe_reach(url: &str, o: ProbeOptions, now: &dyn Fn() -> i64) -> Probed {
    let started = now();
    let abort = Abort::new(Some(Duration::from_millis(o.timeout_ms.max(0) as u64)), None);
    let req = FetchRequest { method: "GET".into(), url: url.into(), headers: Vec::new(), body: None, abort: Some(abort) };
    match platform_fetch().fetch_stream(req).await {
        Ok(mut res) => {
            if let Some(mut body) = res.body.take() {
                while let Some(Ok(_)) = body.next().await {}
            }
            let from_daemon = res.status == DAEMON_ANSWER;
            if now() - started > o.prompt_ms {
                return Probed { state: ReachState::Slow, from_daemon, offline: false };
            }
            let state = if res.status == 502 { ReachState::NoDaemon } else { ReachState::Reachable };
            Probed { state, from_daemon, offline: false }
        }
        Err(e) => missed(&EngineError::Road(e)),
    }
}

/// A machine's own answer for its daemon: `answers(Some(timeout_ms))`.
pub type DaemonAnswers = Rc<dyn Fn(Option<i64>) -> LocalBoxFuture<'static, Result<bool, EngineError>>>;

// /** One look at a machine that answers for its own daemon, over the road its own calls take. A guest that says the
//  * daemon's port is closed is the dead daemon behind a live machine that a prompt 502 is on a backend with an edge;
//  * a guest that will not answer at all is silence, and a call that never left this computer is the computer's. */
// export async function askDaemon(
//   answers: (opts?: { timeoutMs?: number }) => Promise<boolean>,
//   o: ProbeOptions,
//   now: () => number = Date.now,
// ): Promise<Probed> {
//   const started = now();
//   try {
//     const up = await bounded(answers({ timeoutMs: o.timeoutMs }), o.timeoutMs);
//     if (now() - started > o.promptMs) return { state: "slow", fromDaemon: up };
//     return { state: up ? "reachable" : "no-daemon", fromDaemon: up };
//   } catch (e) {
//     return missed(e);
//   }
// }
/// The bound and the elapsed read run on `clock` (rule 3.15).
pub async fn ask_daemon(answers: &DaemonAnswers, o: ProbeOptions, clock: &Rc<dyn Clock>) -> Probed {
    let started = clock.now();
    match bounded(answers(Some(o.timeout_ms)), o.timeout_ms, clock).await {
        Ok(up) => {
            if clock.now() - started > o.prompt_ms {
                return Probed { state: ReachState::Slow, from_daemon: up, offline: false };
            }
            let state = if up { ReachState::Reachable } else { ReachState::NoDaemon };
            Probed { state, from_daemon: up, offline: false }
        }
        Err(e) => missed(&e),
    }
}

// /** Rejects once ms pass; the underlying promise is left to settle on its own. */
// function bounded<T>(p: Promise<T>, ms: number): Promise<T> {
//   return new Promise<T>((resolve, reject) => {
//     const timer = setTimeout(() => reject(new Error(`timed out after ${ms} ms`)), ms);
//     p.then(
//       v => { clearTimeout(timer); resolve(v); },
//       e => { clearTimeout(timer); reject(e); },
//     );
//   });
// }
/// The future is spawned and left to finish on its own; past `ms` on the clock the answer is
/// `EngineError::Other { message: "timed out after <ms> ms", code: None }`.
pub async fn bounded<T: 'static>(
    work: LocalBoxFuture<'static, Result<T, EngineError>>,
    ms: i64,
    clock: &Rc<dyn Clock>,
) -> Result<T, EngineError> {
    let expired_words = || EngineError::Other { message: format!("timed out after {ms} ms"), code: None };
    let (answer_tx, answer) = tokio::sync::oneshot::channel();
    let (expired_tx, expired) = tokio::sync::oneshot::channel::<()>();
    let cancel = clock.schedule(
        Box::new(move || {
            let _ = expired_tx.send(());
        }),
        ms,
    );
    tokio::task::spawn_local(async move {
        let _ = answer_tx.send(work.await);
    });
    let outcome = tokio::select! {
        biased;
        r = answer => r.unwrap_or_else(|_| Err(expired_words())),
        _ = expired => Err(expired_words()),
    };
    cancel();
    outcome
}

// /** Whose run of probes a status read joins. The dampening rule wants two silences in a row before a row's word
//  * turns, so every read is dampened by the run it belongs to and moves that run on: the app keeps the one a sidebar
//  * row rides, and every listing door shares another. An agent polling `wsp workspaces` therefore cannot spend the
//  * silence the person's row is granted, and a table on a host with no app open is still dampened by its own last
//  * read rather than calling a machine dark on one blip. */
// export type StatusReader = "app" | "table";
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum StatusReader {
    #[default]
    App,
    Table,
}

/// `"always" | "on-failure"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Reconcile {
    #[default]
    Always,
    OnFailure,
}

// export interface StatusListOptions {
//   probeTimeoutMs?: number;
//   /** Which run of probes this read joins; the app's when unsaid, since the poller and its snapshot do not say. */
//   reader?: StatusReader;
//   promptMs?: number;
//   /** "always" asks the provider for every machine (an explicit refresh; what a
//    * bare list() does). "on-failure" asks only after a failed reach and reuses
//    * that answer for the reconcile window; the poller runs this way. */
//   reconcile?: "always" | "on-failure";
//   /** Whether a machine dark past the zombie window may be settled here with an exec probe into the guest, which
//    * costs up to zombieProbeTimeoutMs on top of the reach probe. The poller pays it, since the verdict is its to
//    * keep; false bounds a caller's wait to one reach probe and shows the word the poller last settled on. */
//   zombieProbe?: boolean;
// }
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusListOptions {
    pub probe_timeout_ms: Option<i64>,
    pub reader: Option<StatusReader>,
    pub prompt_ms: Option<i64>,
    pub reconcile: Option<Reconcile>,
    pub zombie_probe: Option<bool>,
}

// export interface StatusWatchOptions {
//   costIntervalMs?: number;
//   pollIntervalMs?: number;
//   probeTimeoutMs?: number;
//   promptMs?: number;
//   /** How long the poller believes a provider answer before a failed reach may ask again. */
//   reconcileMinMs?: number;
//   /** Reach slow or unreachable for this long on a machine the provider calls running earns one exec probe. */
//   zombieWindowMs?: number;
//   /** The exec probe's bound; past it the guest counts as dead. */
//   zombieProbeTimeoutMs?: number;
// }
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusWatchOptions {
    pub cost_interval_ms: Option<i64>,
    pub poll_interval_ms: Option<i64>,
    pub probe_timeout_ms: Option<i64>,
    pub prompt_ms: Option<i64>,
    pub reconcile_min_ms: Option<i64>,
    pub zombie_window_ms: Option<i64>,
    pub zombie_probe_timeout_ms: Option<i64>,
}

/// `Pick<PreviewReach, "url" | "expiresAt">`: where the daemon answers and when that route expires.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteReach {
    pub url: String,
    pub expires_at: f64,
}

pub type RecordCall<T> = Rc<dyn Fn() -> LocalBoxFuture<'static, Result<T, EngineError>>>;
/// `exec(cmd, { timeoutMs })`.
pub type RecordExec = Rc<dyn Fn(String, Option<i64>) -> LocalBoxFuture<'static, Result<ExecResult, EngineError>>>;

// /** A workspace as the tracker needs it: the view, the size the provider built
//  * (views carry no size), and roads to the machine that never go through
//  * backend.get. daemonReach is absent on a machine whose kind has no road to a daemon. */
// export interface StatusRecord extends WorkspaceView {
//   size: WorkspaceSize;
//   /** The awake rate for this workspace's size on its own backend: a cloud fork's from the provider's pricing, a local
//    * workspace's zero. Computed per record because the rate varies by the machine's kind, not by size alone. */
//   rateUsdPerHour: number;
//   /** Which write of the record this view is of; the poll drops a row built on a view the record has moved past. */
//   generation: number;
//   idleAt?: number;
//   /** A line the runtime keeps saying about this workspace, which every status built from the record carries: what a
//    * wake still asking the provider says, and what it left behind when its asking ran out, so the row neither falls
//    * silent between two asks nor forgets at the next tick that the rebuild is the road left. */
//   reason?: string;
//   /** Where this machine's daemon answers and when that route expires; the probe fetches the one and the row carries
//    * the other, so whatever else a kind's road hands out (a token) stays off this. */
//   daemonReach?: () => Promise<Pick<PreviewReach, "url" | "expiresAt">>;
//   /** Present on a machine that answers for its own daemon: the reach is asked of it rather than dialled from here,
//    * and the route above is then only what the app is handed to dial. */
//   daemonAnswers?: (opts?: { timeoutMs?: number }) => Promise<boolean>;
//   /** The machine's own prompt for that road, where its round trip is not one HTTP GET's. */
//   daemonAnswersPromptMs?: number;
//   providerState: () => Promise<MachineState>;
//   /** The host's own word on the machine; absent on backends without one. */
//   metrics?: () => Promise<void>;
//   /** What a machine that already existed says about itself; absent on a fork wsp made. */
//   facts?: () => Promise<MachineFacts>;
//   exec: (cmd: string, opts?: { timeoutMs?: number }) => Promise<ExecResult>;
//   /** Why nothing about this machine can be asked at all right now, in the words the person reads: the computer it
//    * lives on is not connected. The row says so and the provider is asked nothing, since the road to the provider
//    * is that computer. Absent on every machine that can be asked. */
//   away?: string;
//   /** The provider's sentence for a machine it answered that it cannot reach, standing until any command answers. */
//   unreached?: string;
// }
/// The runtime's extra fields on a record (`checkout`, `pr`, `tree`, `wake_ask`) ride onto the row as the TS spread does.
#[derive(Clone)]
pub struct StatusRecord {
    pub view: WorkspaceView,
    pub size: WorkspaceSize,
    pub rate_usd_per_hour: f64,
    pub generation: i64,
    pub idle_at: Option<f64>,
    pub reason: Option<String>,
    pub checkout: Option<Checkout>,
    pub pr: Option<PullRequestSeen>,
    pub tree: Option<TreeFact>,
    pub wake_ask: Option<WorkspaceStatusWakeAsk>,
    pub daemon_reach: Option<RecordCall<RouteReach>>,
    pub daemon_answers: Option<DaemonAnswers>,
    pub daemon_answers_prompt_ms: Option<i64>,
    pub provider_state: RecordCall<MachineState>,
    pub metrics: Option<RecordCall<()>>,
    pub facts: Option<RecordCall<MachineFacts>>,
    pub exec: RecordExec,
    pub away: Option<String>,
    pub unreached: Option<String>,
}

/// A bus listener as `on` takes it.
pub type Listener = Rc<dyn Fn(&EventUnion)>;

// export interface StatusTrackerOptions {
//   records(): Promise<StatusRecord[]>;
//   /** Holds each workspace's folded cost history so the series and the meter behind it outlive the process. */
//   store: Store;
//   emit(event: EventUnion): void;
//   /** To every listener and into no replay list, for a cost tick whose figures repeat the last ones emitted. */
//   pass(event: EventUnion): void;
//   on(type: EventUnion["type"] | "*", listener: (e: EventUnion) => void): () => void;
//   defaults?: StatusWatchOptions;
//   /** Every status the poll built, on every tick, ahead of the bus dropping the ones that did not change. What a
//    * client needs is the changes; what the runtime needs about its own machines is the measurement, and a machine
//    * parked in one state changes nothing for hours while staying just as dark. */
//   onPolled?(statuses: WorkspaceStatus[]): void;
//   /** A provider read that answered gone for a record not marked gone, with the words the record would carry. */
//   onGone?(id: string, machineId: string, reason: string): void;
//   /** Time source for the meters, the reconcile and zombie windows and the probe's elapsed read, and the timer the cost
//    * and poll ticks run on; tests inject one they can advance. */
//   clock?: Clock;
// }
pub struct StatusTrackerOptions {
    /// The records now; a failure is its message.
    pub records: Rc<dyn Fn() -> LocalBoxFuture<'static, Result<Vec<StatusRecord>, String>>>,
    /// Holds each workspace's folded cost history so the series and the meter behind it outlive the process.
    pub store: Rc<dyn Store>,
    pub emit: Rc<dyn Fn(EventUnion)>,
    /// To every listener and into no replay list.
    pub pass: Rc<dyn Fn(EventUnion)>,
    /// `on(type, listener)`; `type` is an event's `type` or `"*"`. The answer removes the listener.
    pub on: Rc<dyn Fn(&'static str, Listener) -> Rc<dyn Fn()>>,
    pub defaults: StatusWatchOptions,
    pub on_polled: Option<Rc<dyn Fn(&[WorkspaceStatus])>>,
    /// `(id, machine_id, reason)`.
    pub on_gone: Option<Rc<dyn Fn(&str, &str, &str)>>,
    /// Defaults to the real clock.
    pub clock: Option<Rc<dyn Clock>>,
    /// The zone `day_start` and `month_start` read; the system's when absent (spec status_meter, Decision 1).
    pub zone: Option<jiff::tz::TimeZone>,
}

// interface Meter {
//   awakeMs: number;
//   /** Set while running: when the current awake stretch began. */
//   mark?: number;
//   /** The newest instant anything proved the machine awake. A pause or a death the provider made ended the stretch
//    * here at the latest: nothing proves a machine awake while the host is down or unwatched, so the gap is not billed. */
//   awakeUntil?: number;
// }
#[derive(Debug, Clone, Default, PartialEq)]
struct Meter {
    awake_ms: f64,
    mark: Option<i64>,
    awake_until: Option<i64>,
}

// const COST_HISTORIES = "cost-histories";
pub const COST_HISTORIES: &str = "cost-histories";

// /** What a workspace's document keeps of where it stood, which is what says whose money it spent once the workspace
//  * itself is gone. The fields the one reading of that takes (workspacePlaceId) and no others. */
// type Stood = Pick<WorkspaceView, "kind" | "machineId" | "place" | "provider">;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stood {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<WorkspaceKind>,
    pub machine_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

// /** One store document per workspace: its folded series as of the tick that last added a point, where the workspace
//  * stood, and whether it has been deleted, which is what keeps a deleted workspace's share of the month on the
//  * place that billed for it. */
// interface CostHistoryRecord {
//   workspaceId: string;
//   points: WorkspaceCostEvent[];
//   where?: Stood;
//   /** Set when the workspace was deleted; its series never grows again and the load drops it once its last tick
//    * falls out of the month, so a person who makes and deletes workspaces all year keeps one month of documents. */
//   ended?: boolean;
// }
/// The one value the `type` key of a cost tick takes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
enum CostKind {
    #[default]
    #[serde(rename = "workspace.cost")]
    Cost,
}

/// A cost tick as the TS builds and stores it, `type` first (status.ts:808-821): what the stored document holds and
/// `history()` hands to the wire. The wire's own struct has no `type`, since the event union writes it as its tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostPoint {
    #[serde(rename = "type", default)]
    kind: CostKind,
    #[serde(flatten)]
    pub event: WorkspaceCostEvent,
}

impl From<WorkspaceCostEvent> for CostPoint {
    fn from(event: WorkspaceCostEvent) -> Self {
        CostPoint { kind: CostKind::Cost, event }
    }
}

impl std::ops::Deref for CostPoint {
    type Target = WorkspaceCostEvent;
    fn deref(&self) -> &WorkspaceCostEvent {
        &self.event
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostHistoryRecord {
    pub workspace_id: String,
    pub points: Vec<CostPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "where")]
    pub where_: Option<Stood>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<bool>,
}

// const PROBE_TIMEOUT_MS = 10_000;
// const PROMPT_MS = 2_500;
// const RECONCILE_MIN_MS = 5 * 60_000;
// /** Both measured zombies sat slow or unreachable for well over this before anyone looked;
//  * a provider slow spell (5 to 11 s answers, minutes long) must not reach the probe. */
// const ZOMBIE_WINDOW_MS = 3 * 60_000;
// /** The zombies' own exec 502'd after 36 to 38 s; a live guest answers echo in under a second. */
// const ZOMBIE_PROBE_TIMEOUT_MS = 20_000;
// /** How often the poller reads the running records; the idle nap that failed is asked again on the same cadence. */
// export const POLL_INTERVAL_MS = 15_000;
// const ZOMBIE_PROBE_CMD = "echo ok";
//
// /** The longest a machine that will not say what it is goes unasked. Far enough out that a machine parked off costs
//  * a dial an hour rather than one a tick, near enough that a person who turns it back on sees its row fill in while
//  * they are still looking at it. */
// const FACTS_BACKOFF_MAX_MS = 5 * 60_000;
pub const PROBE_TIMEOUT_MS: i64 = 10_000;
pub const PROMPT_MS: i64 = 2_500;
pub const RECONCILE_MIN_MS: i64 = 5 * 60_000;
pub const ZOMBIE_WINDOW_MS: i64 = 3 * 60_000;
pub const ZOMBIE_PROBE_TIMEOUT_MS: i64 = 20_000;
pub const POLL_INTERVAL_MS: i64 = 15_000;
pub const ZOMBIE_PROBE_CMD: &str = "echo ok";
pub const FACTS_BACKOFF_MAX_MS: i64 = 5 * 60_000;
/// status.ts:340, the cost tick's default interval.
pub const COST_INTERVAL_MS: i64 = 5_000;

// /** How long to leave a machine alone after a facts read that never reached it: one poll, then twice that, widening
//  * until it answers. The read is a dial of its own on a machine that already exists, and a machine that is off
//  * answers nothing until its client's connect timeout runs out, so a read on every tick spends that timeout every
//  * tick for as long as the machine stays off. */
// function factsBackoffMs(misses: number): number {
//   return Math.min(FACTS_BACKOFF_MAX_MS, POLL_INTERVAL_MS * 2 ** Math.max(0, misses - 1));
// }
pub fn facts_backoff_ms(misses: i64) -> i64 {
    let exp = (misses - 1).clamp(0, 40);
    (POLL_INTERVAL_MS << exp).min(FACTS_BACKOFF_MAX_MS)
}

// /** A running machine whose reach has been slow or unreachable: when the spell
//  * began, the probe in flight if any, and the verdict once one was reached. */
// interface Suspect {
//   machineId: string;
//   badSince: number;
//   probe?: Promise<string | undefined>;
//   zombie?: string;
// }
#[derive(Clone)]
struct Suspect {
    machine_id: String,
    bad_since: i64,
    probe: Option<Shared<LocalBoxFuture<'static, Option<String>>>>,
    zombie: Option<String>,
}

// /** What the last probe of a workspace's current machine found, raw, and the reach word its row was last given. */
// interface Probes {
//   machineId: string;
//   last?: ReachState;
//   shown: ReachState;
// }
#[derive(Debug, Clone)]
struct Probes {
    machine_id: String,
    last: Option<ReachState>,
    shown: ReachState,
}

// const figuresOf = (tick: WorkspaceCostEvent): string => `${tick.phase} ${tick.rateUsdPerHour} ${tick.accruedUsd}`;
//
// /** One socket's filter: a workspace's spend goes out when its figures move. The timer ticks every workspace every 5 s,
//  * and 70 napping or gone ones with past spend sent the same figures 14 times a second. */
// export function costMoved(): (e: unknown) => boolean {
//   const sent = new Map<string, string>();
//   return e => {
//     const type = (e as { type?: unknown }).type;
//     if (type === "workspace.deleted") sent.delete((e as { workspaceId: string }).workspaceId);
//     if (type !== "workspace.cost") return true;
//     const cost = e as WorkspaceCostEvent;
//     const figures = figuresOf(cost);
//     if (sent.get(cost.workspaceId) === figures) return false;
//     sent.set(cost.workspaceId, figures);
//     return true;
//   };
// }
/// One socket's filter: a workspace's spend goes out when its figures move.
#[derive(Debug, Clone, Default)]
pub struct CostMoved {
    sent: IndexMap<String, String>,
}

impl CostMoved {
    /// Whether the event goes out on this socket.
    pub fn admit(&mut self, e: &EventUnion) -> bool {
        match e {
            EventUnion::WorkspaceDeleted(d) => {
                self.sent.shift_remove(&d.event.workspace_id);
                true
            }
            EventUnion::WorkspaceCost(c) => {
                let cost = &c.event;
                let phase = serde_json::to_value(cost.phase).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
                let figures = format!("{} {} {}", phase, number_string(cost.rate_usd_per_hour.0), number_string(cost.accrued_usd.0));
                if self.sent.get(&cost.workspace_id) == Some(&figures) {
                    return false;
                }
                self.sent.insert(cost.workspace_id.clone(), figures);
                true
            }
            _ => true,
        }
    }
}

pub fn cost_moved() -> CostMoved {
    CostMoved::default()
}

/// Everything the tracker holds between calls (status.ts:343-363, :770-775). The implementer may add private fields.
#[derive(Default)]
struct TrackerState {
    meters: IndexMap<String, Meter>,
    histories: IndexMap<String, Vec<WorkspaceCostEvent>>,
    stood: IndexMap<String, Stood>,
    ended: IndexSet<String>,
    reconciled: IndexMap<String, (MachineState, i64)>,
    gone_reasons: IndexMap<String, String>,
    suspects: IndexMap<String, Rc<RefCell<Suspect>>>,
    probes: IndexMap<StatusReader, IndexMap<String, Probes>>,
    doubted: IndexMap<String, String>,
    /// machine id, misses, next at.
    facts_dials: IndexMap<String, (String, i64, i64)>,
    /// mark, awake ms.
    closed_by_gone: IndexMap<String, (i64, f64)>,
    watchers: u32,
    stop_cost: Option<Cancel>,
    stop_poll: Option<Cancel>,
    last_emitted: IndexMap<String, String>,
    sent_figures: IndexMap<String, String>,
}

// export function createStatusTracker(o: StatusTrackerOptions): StatusApi {
//   const probeTimeoutMs = o.defaults?.probeTimeoutMs ?? PROBE_TIMEOUT_MS;
//   const promptMs = o.defaults?.promptMs ?? PROMPT_MS;
//   const reconcileMinMs = o.defaults?.reconcileMinMs ?? RECONCILE_MIN_MS;
//   const zombieWindowMs = o.defaults?.zombieWindowMs ?? ZOMBIE_WINDOW_MS;
//   const zombieProbeTimeoutMs = o.defaults?.zombieProbeTimeoutMs ?? ZOMBIE_PROBE_TIMEOUT_MS;
//   const costIntervalMs = o.defaults?.costIntervalMs ?? 5_000;
//   const pollIntervalMs = o.defaults?.pollIntervalMs ?? POLL_INTERVAL_MS;
//   const clock = o.clock ?? realClock;
/// One per runtime. Built by `StatusTracker::new`, which registers its bus listeners and starts the load.
pub struct StatusTracker {
    /// The tracker's own handle, for the listeners and timers it registers (`Rc::new_cyclic`).
    me: Weak<StatusTracker>,
    options: StatusTrackerOptions,
    state: RefCell<TrackerState>,
    /// The load of the stored series; ticks, history and spend wait on it.
    loading: RefCell<Option<Shared<LocalBoxFuture<'static, ()>>>>,
    /// The tail of the document writes (one chain, failures swallowed).
    writes: RefCell<Option<Shared<LocalBoxFuture<'static, ()>>>>,
    clock: Rc<dyn Clock>,
    zone: jiff::tz::TimeZone,
}

// status.ts:154
// export interface StatusApi {
//   list(opts?: StatusListOptions): Promise<WorkspaceStatus[]>;
//   watch(opts?: StatusWatchOptions): () => void;
//   watched(): boolean;
//   poll(ids: readonly string[]): Promise<void>;
//   history(workspaceId: string): Promise<WorkspaceCostEvent[]>;
//   spend(places: readonly PlaceView[], at?: number): Promise<PlaceSpend[]>;
// }
/// What the runtime and its callers hold of the tracker. A failure is its message.
#[async_trait(?Send)]
pub trait StatusApi {
    async fn list(&self, opts: StatusListOptions) -> Result<Vec<WorkspaceStatus>, String>;
    /// Refcounted: while one watcher holds it, the poller and the cost ticker run. The answer is the release.
    fn watch(&self, opts: StatusWatchOptions) -> Rc<dyn Fn()>;
    fn watched(&self) -> bool;
    async fn poll(&self, ids: &[String]);
    async fn history(&self, workspace_id: &str) -> Vec<CostPoint>;
    async fn spend(&self, places: &[PlaceView], at: Option<i64>) -> Result<Vec<PlaceSpend>, String>;
}

#[async_trait(?Send)]
impl StatusApi for StatusTracker {
    async fn list(&self, opts: StatusListOptions) -> Result<Vec<WorkspaceStatus>, String> {
        StatusTracker::list(self, opts).await
    }
    fn watch(&self, opts: StatusWatchOptions) -> Rc<dyn Fn()> {
        StatusTracker::watch(self, opts)
    }
    fn watched(&self) -> bool {
        StatusTracker::watched(self)
    }
    async fn poll(&self, ids: &[String]) {
        StatusTracker::poll(self, ids).await
    }
    async fn history(&self, workspace_id: &str) -> Vec<CostPoint> {
        StatusTracker::history(self, workspace_id).await
    }
    async fn spend(&self, places: &[PlaceView], at: Option<i64>) -> Result<Vec<PlaceSpend>, String> {
        StatusTracker::spend(self, places, at).await
    }
}

impl StatusTracker {
    /// `createStatusTracker(o)`: the listeners of spec status_meter Behaviour 3, in that order, and the load started.
    pub fn new(o: StatusTrackerOptions) -> Rc<StatusTracker> {
        let clock = o.clock.clone().unwrap_or_else(real_clock);
        let zone = o.zone.clone().unwrap_or_else(jiff::tz::TimeZone::system);
        let t = Rc::new_cyclic(|me: &Weak<StatusTracker>| StatusTracker {
            me: me.clone(),
            options: o,
            state: RefCell::new(TrackerState::default()),
            loading: RefCell::new(None),
            writes: RefCell::new(None),
            clock,
            zone,
        });
        t.register();
        t
    }

    fn register(self: &Rc<Self>) {
        self.listen("workspace.created", StatusTracker::meter_created);
        self.listen("workspace.woken", StatusTracker::meter_woken);
        self.listen("workspace.upgraded", StatusTracker::meter_upgraded);
        self.listen("workspace.napped", StatusTracker::meter_stopped);
        self.listen("workspace.gone", StatusTracker::meter_stopped);
        self.listen("workspace.deleted", StatusTracker::drop_workspace);
        self.listen("workspace.status", StatusTracker::note_pushed);
        self.listen("workspace.created", StatusTracker::tick_created);
        self.listen("workspace.woken", StatusTracker::tick_moved);
        self.listen("workspace.upgraded", StatusTracker::tick_moved);
        self.listen("workspace.napped", StatusTracker::tick_moved);
        self.listen("workspace.gone", StatusTracker::tick_moved);
        self.start_load();
    }

    /// The tracker's bus listener for `ty`: a weak handle, so the bus holds no tracker alive.
    fn listen(&self, ty: &'static str, handle: fn(&StatusTracker, &EventUnion)) {
        let me = self.me.clone();
        let listener: Listener = Rc::new(move |e: &EventUnion| {
            if let Some(t) = me.upgrade() {
                handle(&t, e);
            }
        });
        let _ = (self.options.on)(ty, listener);
    }

    fn start_load(&self) {
        let me = self.me.clone();
        let load: Shared<LocalBoxFuture<'static, ()>> = (async move {
            if let Some(t) = me.upgrade() {
                t.load_stored().await;
            }
        })
        .boxed_local()
        .shared();
        *self.loading.borrow_mut() = Some(load.clone());
        *self.writes.borrow_mut() = Some(load.clone());
        tokio::task::spawn_local(load);
    }

    /// Runs `f` on the state with the borrow held for `f` alone: no call out of the tracker goes inside it.
    fn st<R>(&self, f: impl FnOnce(&mut TrackerState) -> R) -> R {
        f(&mut self.state.borrow_mut())
    }

    async fn loaded(&self) {
        let load = self.loading.borrow().clone();
        if let Some(load) = load {
            load.await;
        }
    }

    async fn records(&self) -> Result<Vec<StatusRecord>, String> {
        (self.options.records)().await
    }
}

/// `new Date(ms).toISOString()`.
fn iso(ms: i64) -> String {
    iso_string(ms as f64).unwrap_or_default()
}
