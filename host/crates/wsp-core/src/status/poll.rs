// SPDX-License-Identifier: AGPL-3.0-only
//! The row builder and the poller of packages/runtime/src/status.ts at 65f6b89c7. Spec: spec/status_poll.md.
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use futures::future::{join_all, LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{json, Map, Value};
use wsp_engine::errors::is_missing;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::workspace::{MachineFacts, MachineState, ReachState, ReachStatus, WorkspacePhase};
use wsp_wire::views::workspace_events::WorkspaceStatusEvent;
use wsp_wire::views::workspace_more::WorkspaceStatus;
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_words::js::{js_round, js_trim, number_string};
use wsp_words::words::thread::{gone_words, GoneSeenBy, GoneSighting};
use wsp_words::workspace_state::reach_shown;

use super::meter::meter_of;
use super::{
    ask_daemon, bounded, facts_backoff_ms, iso, machine_state_of, missed, probe_reach, provider_said, ProbeOptions, Probed, Probes,
    Reconcile, RecordCall, RecordExec, RouteReach, StatusListOptions, StatusReader, StatusRecord, StatusTracker, StatusWatchOptions,
    Suspect, COST_INTERVAL_MS, POLL_INTERVAL_MS, PROBE_TIMEOUT_MS, PROMPT_MS, RECONCILE_MIN_MS, ZOMBIE_PROBE_CMD, ZOMBIE_PROBE_TIMEOUT_MS,
    ZOMBIE_WINDOW_MS,
};
use crate::clock::{real_clock, Cancel, Clock};
use crate::console::warn;

// The provider asks, the zombie judge, the probe memory and the row builder, as the TS has them. Private helpers for
// each go in this file.
//
//   /** The host's own read of a machine the state read calls running, logged once per spell and never a verdict: one
//    * metrics 404 over a machine whose state read said running, and that a direct read found running two minutes
//    * later, killed two live turns on 2026-09-08. Gone is the state read's word alone. */
//   const noteMetricsGap = async (r: StatusRecord): Promise<void> => {
//     if (r.metrics === undefined) return;
//     let gap: string;
//     try {
//       await r.metrics();
//       doubted.delete(r.id);
//       return;
//     } catch (e) {
//       if (!isMissing(e)) return;
//       gap = providerSaid(e);
//     }
//     if (doubted.get(r.id) === r.machineId) return;
//     doubted.set(r.id, r.machineId);
//     console.warn(`host metrics for ${r.machineId} (workspace ${r.id}) answered ${gap}; the state read says running and the record follows the state read`);
//   };
//
//   /** The provider's word, or our own when it cannot be had (weather is not a reason to report a running workspace
//    * as anything else). Only the state read by id names a machine gone. */
//   const askProvider = async (r: StatusRecord): Promise<MachineState> => {
//     let state: MachineState;
//     let answer: string | undefined;
//     try {
//       state = await r.providerState();
//     } catch (e) {
//       if (!isMissing(e)) return machineStateOf(r.phase);
//       state = "gone";
//       answer = providerSaid(e);
//     }
//     if (state === "running") await noteMetricsGap(r);
//     reconciled.set(r.id, { state, at: clock.now() });
//     if (state === "running") sawAwake(r.id);
//     if (state === "gone") goneReasons.set(r.id, goneWords(r.machineId, { by: "status poll", at: clock.now(), ...(answer !== undefined ? { answer } : {}) }));
//     return state;
//   };
//
//   const machineState = async (r: StatusRecord, reconcile: StatusListOptions["reconcile"], reachFailed: boolean): Promise<MachineState> => {
//     if (reconcile === "always") return askProvider(r);
//     if (!reachFailed) return machineStateOf(r.phase);
//     const known = reconciled.get(r.id);
//     // A reading the record has moved past since, the paused one taken during a nap before the wake, is asked again.
//     if (known && known.state === machineStateOf(r.phase) && clock.now() - known.at < reconcileMinMs) return known.state;
//     return askProvider(r);
//   };
//
//   /** undefined when the guest answered; otherwise what went wrong and how long it took. */
//   const probeExec = async (r: StatusRecord): Promise<string | undefined> => {
//     const started = clock.now();
//     const took = () => `failed after ${clock.now() - started} ms`;
//     try {
//       const res = await bounded(r.exec(ZOMBIE_PROBE_CMD, { timeoutMs: zombieProbeTimeoutMs }), zombieProbeTimeoutMs);
//       if (res.exitCode === 0 && res.stdout.trim() === "ok") return undefined;
//       const stderr = res.stderr.trim().slice(0, 200);
//       return `${took()} (exit ${res.exitCode}${stderr === "" ? "" : `: ${stderr}`})`;
//     } catch (e) {
//       return `${took()} (${e instanceof Error ? e.message : String(e)})`;
//     }
//   };
//
//   /** This reader's probes of this workspace's current machine; a replaced machine starts clean, and a machine never
//    * probed is given the word the runtime claims for a running one until a probe says otherwise. Kept per reader, so
//    * one door's probe is never counted as the silence another door's row was waiting out. */
//   const probesOf = (r: StatusRecord, reader: StatusReader): Probes => {
//     let book = probes.get(reader);
//     if (book === undefined) probes.set(reader, (book = new Map()));
//     const known = book.get(r.id);
//     if (known !== undefined && known.machineId === r.machineId) return known;
//     const fresh: Probes = { machineId: r.machineId, shown: "reachable" };
//     book.set(r.id, fresh);
//     return fresh;
//   };
//
//   /** The spell this workspace's current machine is in; a replaced machine starts clean. */
//   const suspectOf = (r: StatusRecord): Suspect => {
//     const known = suspects.get(r.id);
//     if (known !== undefined && known.machineId === r.machineId) return known;
//     const fresh: Suspect = { machineId: r.machineId, badSince: clock.now() };
//     suspects.set(r.id, fresh);
//     return fresh;
//   };
//
//   /** A slow or unreachable reach on a running workspace. Inside the window it
//    * is weather. Past it, and only while the provider still says running, one
//    * exec probe decides: an answer restarts the window (a long slow spell with
//    * a live guest never flags), a failure marks the machine zombie until reach
//    * reads healthy again or the machine is replaced. A caller that will not
//    * wait out that probe leaves the verdict to the poller and reads the word it
//    * last settled on. */
//   const judge = async (
//     r: StatusRecord,
//     reach: ReachStatus,
//     opts: StatusListOptions | undefined,
//   ): Promise<{ state: MachineState; reach: ReachStatus; reason?: string }> => {
//     const reconcile = opts?.reconcile ?? "always";
//     const s = suspectOf(r);
//     const elapsed = clock.now() - s.badSince;
//     if (s.zombie === undefined && elapsed < zombieWindowMs) {
//       return { state: await machineState(r, reconcile, reach.state === "unreachable"), reach };
//     }
//     const provider = await machineState(r, reconcile, true);
//     if (provider !== "running") {
//       suspects.delete(r.id);
//       return { state: provider, reach };
//     }
//     if (s.zombie !== undefined) return { state: provider, reach: { ...reach, state: "zombie" }, reason: s.zombie };
//     if (opts?.zombieProbe === false) return { state: provider, reach };
//     s.probe ??= probeExec(r).finally(() => {
//       s.probe = undefined;
//     });
//     const fault = await s.probe;
//     if (fault === undefined) {
//       s.badSince = clock.now();
//       return { state: provider, reach };
//     }
//     const reason =
//       `${r.machineId} reports running at the provider; reach ${reach.state} since ${new Date(s.badSince).toISOString()} ` +
//       `(${Math.round(elapsed / 1000)} s); exec probe "${ZOMBIE_PROBE_CMD}" ${fault}`;
//     s.zombie = reason;
//     console.warn(`zombie on ${r.machineId} (workspace ${r.id}): ${reason}`);
//     return { state: provider, reach: { ...reach, state: "zombie" }, reason };
//   };
//
//   /** What the machine says it is, or nothing where it did not say. A read that never reached the machine backs the
//    * next one off, since the one before it already cost whatever a dial to a machine that is not there costs, and a
//    * row shows the same pending whether the read was skipped or failed. Held here rather than beside the reach,
//    * because a machine with no daemon has no reach probe to learn it from: nothing dials it but this. */
//   const factsRead = async (r: StatusRecord, read: () => Promise<MachineFacts>): Promise<MachineFacts | undefined> => {
//     const held = factsDials.get(r.id);
//     const dialled = held?.machineId === r.machineId ? held : undefined;
//     if (dialled !== undefined && clock.now() < dialled.nextAt) return undefined;
//     const said = await read().catch(() => undefined);
//     if (said !== undefined) {
//       factsDials.delete(r.id);
//       return said;
//     }
//     const misses = (dialled?.misses ?? 0) + 1;
//     factsDials.set(r.id, { machineId: r.machineId, misses, nextAt: clock.now() + factsBackoffMs(misses) });
//     return undefined;
//   };
//
//   const statusesOf = async (records: StatusRecord[], opts: StatusListOptions | undefined): Promise<WorkspaceStatus[]> => {
//     const probe: ProbeOptions = { promptMs: opts?.promptMs ?? promptMs, timeoutMs: opts?.probeTimeoutMs ?? probeTimeoutMs };
//     const reconcile = opts?.reconcile ?? "always";
//
//     return Promise.all(
//       records.map(async (r): Promise<WorkspaceStatus> => {
//         const { size, idleAt, daemonReach, daemonAnswers, daemonAnswersPromptMs, providerState, metrics, facts, exec, generation, away, unreached, ...view } = r;
//         void away;
//         void unreached;
//         void providerState;
//         void metrics;
//         void exec;
//         void generation;
//         // A machine that cannot say what it is this tick keeps its row; the facts are the one part left off it.
//         const said = facts === undefined ? undefined : await factsRead(r, facts);
//         const base = { ...view, size, rateUsdPerHour: r.rateUsdPerHour, ...(idleAt !== undefined ? { idleAt } : {}), ...(said !== undefined ? { facts: said } : {}) };
//         // One 404 is not gone: the row keeps the record's word until the runtime's confirmation settles the record.
//         const done = (state: MachineState, reach: WorkspaceStatus["reach"]): WorkspaceStatus => {
//           if (state === "gone") o.onGone?.(r.id, r.machineId, goneReasons.get(r.id) ?? goneWords(r.machineId));
//           return { ...base, machineState: state === "gone" ? machineStateOf(view.phase) : state, reach };
//         };
//
//         // A queued spawn has no machine yet: there is nothing to ask, and nothing gone.
//         if (view.phase === "queued") {
//           suspects.delete(r.id);
//           return { ...base, machineState: machineStateOf(view.phase), reach: { state: "napping" } };
//         }
//         // A gone record already holds the provider's last word, and a released one stands on no machine; asking again would only 404.
//         if (view.phase === "gone" || view.phase === "released") {
//           suspects.delete(r.id);
//           return { ...base, machineState: "gone", reach: { state: "gone" }, ...(view.gone !== undefined ? { reason: view.gone } : {}) };
//         }
//         // The computer this machine lives on is not connected: nothing is asked of it, the row says why, and the
//         // machine keeps the word it was left with until that computer dials in again.
//         if (r.away !== undefined) {
//           suspects.delete(r.id);
//           return {
//             ...base,
//             machineState: view.phase === "running" ? "running" : "paused",
//             reach: { state: view.phase === "running" ? "unreachable" : "napping" },
//             reason: r.away,
//           };
//         }
//         // Measured: the reach goes dark only while paused and works again on wake.
//         if (view.phase !== "running") {
//           suspects.delete(r.id);
//           return done(await machineState(r, reconcile, false), { state: "napping" });
//         }
//         let route: Pick<PreviewReach, "url" | "expiresAt"> | undefined;
//         let probed: Probed;
//         if (daemonAnswers !== undefined) {
//           // The machine's own road, which is the one its turns run on. The route is still minted for the app to
//           // dial, and a mint that fails says nothing about the guest: on such a machine the route is a road out of
//           // this computer, not the daemon's own word.
//           route = daemonReach === undefined ? undefined : await daemonReach().catch(() => undefined);
//           probed = await askDaemon(daemonAnswers, daemonAnswersPromptMs === undefined ? probe : { ...probe, promptMs: daemonAnswersPromptMs }, clock.now);
//         } else if (daemonReach !== undefined) {
//           try {
//             route = await daemonReach();
//             probed = await probeReach(route.url, probe, clock.now);
//           } catch (e) {
//             probed = missed(e);
//           }
//         } else {
//           // No way at all to ask: no route this computer could dial and no machine that answers for its own
//           // daemon. That, and nothing else, is what unsupported says.
//           return done(await machineState(r, reconcile, false), { state: "unsupported" });
//         }
//         const memory = probesOf(r, opts?.reader ?? "app");
//         const at = route === undefined ? {} : { url: route.url, expiresAt: route.expiresAt };
//         // Nothing was learnt about the machine: the row keeps its word, or the provider's sentence where one stands.
//         if (probed.offline === true) return { ...done(await machineState(r, reconcile, false), { state: r.unreached !== undefined ? "unreachable" : memory.shown, ...at, offline: true }), ...(r.unreached !== undefined ? { reason: r.unreached } : {}) };
//         if (probed.fromDaemon) sawAwake(r.id);
//         const shown = reachShown(memory.last, probed.state);
//         memory.last = probed.state;
//         memory.shown = shown;
//         // The provider's refusal is an answer, not a silence: the row turns at once, keeps its route, and one exec probe heals it.
//         if (r.unreached !== undefined) {
//           const s = suspectOf(r);
//           if (opts?.zombieProbe !== false) {
//             s.probe ??= probeExec(r).finally(() => {
//               s.probe = undefined;
//             });
//           }
//           const judged = await judge(r, { state: "unreachable", ...at }, opts);
//           memory.shown = judged.reach.state;
//           return { ...done(judged.state, judged.reach), ...(judged.state === "running" ? { reason: r.unreached } : {}) };
//         }
//         // A route the reach itself depended on and could not get is a failed reach with no zombie window: the guest
//         // was never dialled. Where the machine answered for its own daemon the route is only the app's road, and
//         // its absence says nothing about the guest.
//         if (route === undefined && daemonAnswers === undefined) return done(await machineState(r, reconcile, true), { state: shown });
//         const status: ReachStatus = { state: probed.state, ...at };
//         if (probed.state !== "slow" && probed.state !== "unreachable") {
//           suspects.delete(r.id);
//           // An answer the guest did not send is a reason to ask the provider, never a verdict on the guest: the
//           // state on show stays as it was, and a machine the provider has paused is caught by its own word. The
//           // ask runs on the raw answer; only the word the row carries waits out a window, as the silence does.
//           return done(await machineState(r, reconcile, !probed.fromDaemon), { ...status, state: shown });
//         }
//         // The window and the exec probe run on the raw silence; only the word on the row waits for a second one.
//         const judged = await judge(r, status, opts);
//         const reach = judged.reach.state === "zombie" ? judged.reach : { ...judged.reach, state: shown };
//         memory.shown = reach.state;
//         return { ...done(judged.state, reach), ...(judged.reason !== undefined ? { reason: judged.reason } : {}) };
//       }),
//     );
//   };
//
//   /** A tick that throws is a tick that failed, never an unhandled rejection: one of those takes the host down with it. */
//   const guarded = (what: string, run: () => Promise<void>) => (): void => {
//     void run().catch((e: unknown) => console.warn(`${what} tick failed`, e));
//   };
//
//   /** Runs fn every ms on the clock, the first time one interval from now; the returned function stops it. */
//   const every = (fn: () => void, ms: number): (() => void) => {
//     let cancel = (): void => {};
//     const arm = (): void => {
//       cancel = clock.schedule(() => { arm(); fn(); }, ms, { unref: true });
//     };
//     arm();
//     return () => cancel();
//   };
//
//   let watchers = 0;
//   let stopCost: (() => void) | undefined;
//   let stopPoll: (() => void) | undefined;
//   const lastEmitted = new Map<string, string>();
//   /** Per workspace, the phase, rate and total the last cost frame carried. */
//   const sentFigures = new Map<string, string>();
//   // A machine's uptime and facts, its route and the stamps of a read move on every read and no client draws them; a
//   // pushed row carries neither facts nor route, so counting them had every poll after a push resend the row.
//   const rowKey = (status: WorkspaceStatus): string =>
//     JSON.stringify({ ...status, facts: undefined, reach: { ...status.reach, url: undefined, expiresAt: undefined } }, (field, value: unknown) => (field === "uptimeMs" || field === "readAt" ? undefined : value));
//   // A status the runtime pushed between polls is a row a client has already been given, so it belongs in the
//   // baseline the next poll is held against. Without this a push moves the row and the poll's own word for the same
//   // machine reads as a repeat and is dropped: a line the runtime meant to flash once then sat on the row until
//   // some other fact about the machine happened to change.
//   o.on("workspace.status", e => {
//     if (e.type === "workspace.status") lastEmitted.set(e.status.id, rowKey(e.status));
//   });
//
//
//   const pollTick = async (only?: ReadonlySet<string>): Promise<void> => {
//     const records = (await o.records()).filter(r => only === undefined || only.has(r.id));
//     const statuses = await statusesOf(records, { reconcile: "on-failure" });
//     // A record written, marked or moved to another machine while its poll was in flight pushed its own row since, and so
//     // did one whose computer left or dialled back in: the poll's older row would put back what the record has left.
//     const stamp = (r: StatusRecord): string => `${r.generation} ${r.machineId} ${r.unreached ?? ""} ${r.away ?? ""}`;
//     const built = new Map(records.map(r => [r.id, stamp(r)]));
//     const now = new Map((await o.records()).map(r => [r.id, stamp(r)]));
//     o.onPolled?.(statuses.filter(status => now.get(status.id) === built.get(status.id)));
//     for (const status of statuses) {
//       if (now.get(status.id) !== built.get(status.id)) continue;
//       const key = rowKey(status);
//       if (lastEmitted.get(status.id) === key) continue;
//       lastEmitted.set(status.id, key);
//       o.emit({ type: "workspace.status", status });
//     }
//   };

impl StatusTracker {
    //   const list: StatusApi["list"] = async opts => statusesOf(await o.records(), opts);
    /// Rows for every record; `reconcile` defaults to always, `reader` to app.
    pub async fn list(&self, opts: StatusListOptions) -> Result<Vec<WorkspaceStatus>, String> {
        let records = self.records().await?;
        Ok(self.statuses_of(&records, opts).await)
    }

    //   const watch: StatusApi["watch"] = opts => {
    //     watchers++;
    //     if (watchers === 1) {
    //       stopCost = every(guarded("cost", () => costTick()), opts?.costIntervalMs ?? costIntervalMs);
    //       stopPoll = every(guarded("status poll", pollTick), opts?.pollIntervalMs ?? pollIntervalMs);
    //       // Every machine's state before the first cost tick: nothing polls while no client watches, so a client
    //       // attaching after a gap would otherwise meter a stretch the provider ended hours ago.
    //       guarded("status poll", pollTick)();
    //     }
    //     let released = false;
    //     return () => {
    //       if (released) return;
    //       released = true;
    //       watchers--;
    //       if (watchers === 0) {
    //         stopCost?.();
    //         stopPoll?.();
    //         stopCost = stopPoll = undefined;
    //         lastEmitted.clear();
    //         sentFigures.clear();
    //       }
    //     };
    //   };
    /// Refcounted: while one watcher holds it, the poller and the cost ticker run. The answer is the release.
    pub fn watch(&self, opts: StatusWatchOptions) -> Rc<dyn Fn()> {
        let first = self.st(|s| {
            s.watchers += 1;
            s.watchers == 1
        });
        if first {
            let cost_ms = opts.cost_interval_ms.or(self.options.defaults.cost_interval_ms).unwrap_or(COST_INTERVAL_MS);
            let poll_ms = opts.poll_interval_ms.or(self.options.defaults.poll_interval_ms).unwrap_or(POLL_INTERVAL_MS);
            let (me_cost, me_poll) = (self.me.clone(), self.me.clone());
            let stop_cost = every(
                self.clock.clone(),
                cost_ms,
                Rc::new(move || {
                    if let Some(t) = me_cost.upgrade() {
                        t.guard_cost(None);
                    }
                }),
            );
            let stop_poll = every(
                self.clock.clone(),
                poll_ms,
                Rc::new(move || {
                    if let Some(t) = me_poll.upgrade() {
                        t.guard_poll(None);
                    }
                }),
            );
            self.st(|s| {
                s.stop_cost = Some(stop_cost);
                s.stop_poll = Some(stop_poll);
            });
            self.guard_poll(None);
        }
        let released = Cell::new(false);
        let me = self.me.clone();
        Rc::new(move || {
            if released.replace(true) {
                return;
            }
            if let Some(t) = me.upgrade() {
                t.release_watch();
            }
        })
    }

    /// Whether a watcher holds it now.
    pub fn watched(&self) -> bool {
        self.st(|s| s.watchers > 0)
    }

    //   const poll: StatusApi["poll"] = async ids => {
    //     if (watchers > 0 && ids.length > 0) await pollTick(new Set(ids)).catch((e: unknown) => console.warn("status poll tick failed", e));
    //   };
    /// Polls these workspaces now, between ticks, and pushes each row that moved, while a watcher holds the poll.
    pub async fn poll(&self, ids: &[String]) {
        if self.watched() && !ids.is_empty() {
            let only: HashSet<String> = ids.iter().cloned().collect();
            if let Err(e) = self.poll_tick(Some(&only)).await {
                warn(&format!("status poll tick failed {e}"));
            }
        }
    }
}

/// The reach word a row carries, as the wire spells it.
fn reach_word(state: ReachState) -> &'static str {
    match state {
        ReachState::Reachable => "reachable",
        ReachState::NoDaemon => "no-daemon",
        ReachState::Unreachable => "unreachable",
        ReachState::Napping => "napping",
        ReachState::Unsupported => "unsupported",
        ReachState::Gone => "gone",
        ReachState::Slow => "slow",
        ReachState::Zombie => "zombie",
    }
}

fn reach_of(state: ReachState) -> ReachStatus {
    ReachStatus { state, url: None, expires_at: None, offline: None }
}

/// A reach word with the route the row was given, if any.
fn with_route(state: ReachState, route: &Option<RouteReach>) -> ReachStatus {
    ReachStatus {
        state,
        url: route.as_ref().map(|rt| rt.url.clone()),
        expires_at: route.as_ref().map(|rt| JsNumber(rt.expires_at)),
        offline: None,
    }
}

fn json_of<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// The fields of a record's view, with the enrichments `statusesOf` spreads onto it (`size`, `rateUsdPerHour`, `idleAt`,
/// `facts`, and the record's own `reason`, `checkout`, `pr`, `tree`, `wakeAsk`).
fn base_of(r: &StatusRecord, said: Option<MachineFacts>) -> Map<String, Value> {
    let mut base = match json_of(&r.view) {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    base.insert("size".into(), json_of(&r.size));
    base.insert("rateUsdPerHour".into(), json!(r.rate_usd_per_hour));
    if let Some(idle_at) = r.idle_at {
        base.insert("idleAt".into(), json!(idle_at));
    }
    if let Some(facts) = said {
        base.insert("facts".into(), json_of(&facts));
    }
    if let Some(reason) = &r.reason {
        base.insert("reason".into(), json!(reason));
    }
    if let Some(checkout) = &r.checkout {
        base.insert("checkout".into(), json_of(checkout));
    }
    if let Some(pr) = &r.pr {
        base.insert("pr".into(), json_of(pr));
    }
    if let Some(tree) = &r.tree {
        base.insert("tree".into(), json_of(tree));
    }
    if let Some(wake_ask) = &r.wake_ask {
        base.insert("wakeAsk".into(), json_of(wake_ask));
    }
    base
}

/// A row: the base with its machine state, its reach, and a reason where one is given (else the base's own).
fn row(base: &Map<String, Value>, state: MachineState, reach: &ReachStatus, reason: Option<&str>) -> WorkspaceStatus {
    let mut out = base.clone();
    out.insert("machineState".into(), json_of(&state));
    out.insert("reach".into(), json_of(reach));
    if let Some(reason) = reason {
        out.insert("reason".into(), json!(reason));
    }
    // A number JSON cannot carry went into the base as null, which is how it goes out of the TS; the row takes it back.
    let rate_unread = out.get("rateUsdPerHour") == Some(&Value::Null);
    if rate_unread {
        out.insert("rateUsdPerHour".into(), json!(0.0));
    }
    let idle_unread = out.get("idleAt") == Some(&Value::Null);
    if idle_unread {
        out.remove("idleAt");
    }
    let mut status: WorkspaceStatus =
        serde_json::from_value(Value::Object(out)).expect("a record's view with its enrichments is a workspace status");
    if rate_unread {
        status.rate_usd_per_hour = JsNumber(f64::NAN);
    }
    if idle_unread {
        status.idle_at = Some(JsNumber(f64::NAN));
    }
    status
}

/// The row key the poll holds a row against: the row without its facts, its route, and any `uptimeMs` or `readAt`.
fn row_key(status: &WorkspaceStatus) -> String {
    let mut v = json_of(status);
    if let Some(obj) = v.as_object_mut() {
        obj.remove("facts");
        if let Some(reach) = obj.get_mut("reach").and_then(Value::as_object_mut) {
            reach.remove("url");
            reach.remove("expiresAt");
        }
    }
    strip_clock_fields(&mut v);
    v.to_string()
}

fn strip_clock_fields(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.remove("uptimeMs");
            map.remove("readAt");
            for child in map.values_mut() {
                strip_clock_fields(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_clock_fields(item);
            }
        }
        _ => {}
    }
}

/// `slice(0, units)`: the longest prefix of `s` that is at most `units` UTF-16 units; a cut that falls inside a pair
/// leaves the lone half, which the line prints as U+FFFD.
fn first_units(s: &str, units: usize) -> String {
    let mut used = 0;
    for (i, c) in s.char_indices() {
        if used + c.len_utf16() > units {
            return if c.len_utf16() == 2 && used + 1 == units { format!("{}\u{FFFD}", &s[..i]) } else { s[..i].to_string() };
        }
        used += c.len_utf16();
    }
    s.to_string()
}

/// `probeExec(r)`: undefined when the guest answered `ok`, else what went wrong and how long it took.
async fn probe_exec(exec: RecordExec, timeout: i64, clock: Rc<dyn Clock>) -> Option<String> {
    let started = clock.now();
    let took = || format!("failed after {} ms", clock.now() - started);
    // status.ts:554 bounds the probe with a raw setTimeout, so its bound runs in real time, not on the injected clock.
    match bounded(exec(ZOMBIE_PROBE_CMD.to_string(), Some(timeout)), timeout, &real_clock()).await {
        Ok(res) => {
            let code = res.exit_code.0 .0;
            if code == 0.0 && js_trim(&res.stdout) == "ok" {
                return None;
            }
            let stderr = first_units(js_trim(&res.stderr), 200);
            let detail = if stderr.is_empty() { String::new() } else { format!(": {stderr}") };
            Some(format!("{} (exit {}{detail})", took(), number_string(code)))
        }
        Err(e) => Some(format!("{} ({e})", took())),
    }
}

/// Runs `fn` every `ms` on the clock, the first time one interval from now; the answer stops it.
fn every(clock: Rc<dyn Clock>, ms: i64, body: Rc<dyn Fn()>) -> Cancel {
    let latest: Rc<RefCell<Option<Cancel>>> = Rc::new(RefCell::new(None));
    arm_every(clock, ms, body, latest.clone());
    Rc::new(move || {
        let stop = latest.borrow_mut().take();
        if let Some(stop) = stop {
            stop();
        }
    })
}

fn arm_every(clock: Rc<dyn Clock>, ms: i64, body: Rc<dyn Fn()>, latest: Rc<RefCell<Option<Cancel>>>) {
    let next = (clock.clone(), body.clone(), latest.clone());
    let cancel = clock.schedule(
        Box::new(move || {
            arm_every(next.0, ms, next.1.clone(), next.2);
            next.1();
        }),
        ms,
    );
    *latest.borrow_mut() = Some(cancel);
}

/// The option a call gives, else the tracker's default for it, else the constant.
fn knob(given: Option<i64>, default: Option<i64>, constant: i64) -> i64 {
    given.or(default).unwrap_or(constant)
}

impl StatusTracker {
    /// Rows for `records`, in their order, each built on its own (statusesOf, :654-761).
    async fn statuses_of(&self, records: &[StatusRecord], opts: StatusListOptions) -> Vec<WorkspaceStatus> {
        let probe = self.probe_options(opts);
        let reconcile = opts.reconcile.unwrap_or_default();
        join_all(records.iter().map(|r| self.status_of(r, opts, probe, reconcile))).await
    }

    fn probe_options(&self, opts: StatusListOptions) -> ProbeOptions {
        let d = &self.options.defaults;
        ProbeOptions {
            prompt_ms: knob(opts.prompt_ms, d.prompt_ms, PROMPT_MS),
            timeout_ms: knob(opts.probe_timeout_ms, d.probe_timeout_ms, PROBE_TIMEOUT_MS),
        }
    }

    fn reconcile_min(&self) -> i64 {
        knob(None, self.options.defaults.reconcile_min_ms, RECONCILE_MIN_MS)
    }

    fn zombie_window(&self) -> i64 {
        knob(None, self.options.defaults.zombie_window_ms, ZOMBIE_WINDOW_MS)
    }

    fn zombie_probe_timeout(&self) -> i64 {
        knob(None, self.options.defaults.zombie_probe_timeout_ms, ZOMBIE_PROBE_TIMEOUT_MS)
    }

    /// One record's row (statusesOf, :658-761).
    async fn status_of(&self, r: &StatusRecord, opts: StatusListOptions, probe: ProbeOptions, reconcile: Reconcile) -> WorkspaceStatus {
        let id = r.view.id.as_str();
        let said = match &r.facts {
            Some(read) => self.facts_read(r, read).await,
            None => None,
        };
        let base = base_of(r, said);
        match r.view.phase {
            WorkspacePhase::Queued => {
                self.drop_suspect(id);
                return row(&base, MachineState::Starting, &reach_of(ReachState::Napping), None);
            }
            WorkspacePhase::Gone | WorkspacePhase::Released => {
                self.drop_suspect(id);
                return row(&base, MachineState::Gone, &reach_of(ReachState::Gone), r.view.gone.as_deref());
            }
            _ => {}
        }
        if let Some(away) = &r.away {
            self.drop_suspect(id);
            let (state, reach) = if r.view.phase == WorkspacePhase::Running {
                (MachineState::Running, ReachState::Unreachable)
            } else {
                (MachineState::Paused, ReachState::Napping)
            };
            return row(&base, state, &reach_of(reach), Some(away.as_str()));
        }
        if r.view.phase != WorkspacePhase::Running {
            self.drop_suspect(id);
            let state = self.machine_state(r, reconcile, false).await;
            return self.done(r, &base, state, reach_of(ReachState::Napping), None);
        }

        let clock = self.clock.clone();
        let now = move || clock.now();
        let route: Option<RouteReach>;
        let probed: Probed;
        if let Some(answers) = &r.daemon_answers {
            route = match &r.daemon_reach {
                Some(mint) => mint().await.ok(),
                None => None,
            };
            let p = ProbeOptions { prompt_ms: r.daemon_answers_prompt_ms.unwrap_or(probe.prompt_ms), ..probe };
            probed = ask_daemon(answers, p, &self.clock).await;
        } else if let Some(mint) = &r.daemon_reach {
            match mint().await {
                Ok(rt) => {
                    probed = probe_reach(&rt.url, probe, &now).await;
                    route = Some(rt);
                }
                Err(e) => {
                    route = None;
                    probed = missed(&e);
                }
            }
        } else {
            let state = self.machine_state(r, reconcile, false).await;
            return self.done(r, &base, state, reach_of(ReachState::Unsupported), None);
        }

        let reader = opts.reader.unwrap_or_default();
        let memory = self.probe_memory(r, reader);
        if probed.offline {
            let state = self.machine_state(r, reconcile, false).await;
            let word = if r.unreached.is_some() { ReachState::Unreachable } else { self.probe_memory(r, reader).shown };
            let mut reach = with_route(word, &route);
            reach.offline = Some(true);
            return self.done(r, &base, state, reach, r.unreached.as_deref());
        }
        if probed.from_daemon {
            let at = self.clock.now();
            self.st(|s| meter_of(s, id).awake_until = Some(at));
        }
        let shown = reach_shown(memory.last.as_ref(), &probed.state);
        self.update_probe(reader, id, |p| {
            p.last = Some(probed.state);
            p.shown = shown;
        });

        if let Some(unreached) = &r.unreached {
            self.suspect_of(r);
            if opts.zombie_probe != Some(false) {
                self.start_probe_if_idle(r);
            }
            let (state, judged, _) = self.judge(r, with_route(ReachState::Unreachable, &route), opts).await;
            self.update_probe(reader, id, |p| p.shown = judged.state);
            let reason = if state == MachineState::Running { Some(unreached.as_str()) } else { None };
            return self.done(r, &base, state, judged, reason);
        }
        if route.is_none() && r.daemon_answers.is_none() {
            let state = self.machine_state(r, reconcile, true).await;
            return self.done(r, &base, state, reach_of(shown), None);
        }
        let status = with_route(probed.state, &route);
        if probed.state != ReachState::Slow && probed.state != ReachState::Unreachable {
            self.drop_suspect(id);
            let state = self.machine_state(r, reconcile, !probed.from_daemon).await;
            let mut reach = status;
            reach.state = shown;
            return self.done(r, &base, state, reach, None);
        }
        let (state, judged, reason) = self.judge(r, status, opts).await;
        let reach = if judged.state == ReachState::Zombie { judged } else { ReachStatus { state: shown, ..judged } };
        self.update_probe(reader, id, |p| p.shown = reach.state);
        self.done(r, &base, state, reach, reason.as_deref())
    }

    /// `done(state, reach)`: a row in `state`, and the gone verdict told to `on_gone` (status_poll Behaviour 3).
    fn done(
        &self,
        r: &StatusRecord,
        base: &Map<String, Value>,
        state: MachineState,
        reach: ReachStatus,
        reason: Option<&str>,
    ) -> WorkspaceStatus {
        if state == MachineState::Gone {
            if let Some(on_gone) = &self.options.on_gone {
                let words = self.st(|s| s.gone_reasons.get(&r.view.id).cloned()).unwrap_or_else(|| gone_words(&r.view.machine_id, None));
                on_gone(&r.view.id, &r.view.machine_id, &words);
            }
            return row(base, machine_state_of(r.view.phase), &reach, reason);
        }
        row(base, state, &reach, reason)
    }

    /// `factsRead(r, read)`: what the machine says it is, or nothing where it did not say (status_poll Behaviour 13).
    async fn facts_read(&self, r: &StatusRecord, read: &RecordCall<MachineFacts>) -> Option<MachineFacts> {
        let id = r.view.id.as_str();
        let dialled = self.st(|s| s.facts_dials.get(id).filter(|d| d.0 == r.view.machine_id).cloned());
        if let Some(d) = &dialled {
            if self.clock.now() < d.2 {
                return None;
            }
        }
        if let Ok(said) = read().await {
            self.st(|s| s.facts_dials.shift_remove(id));
            return Some(said);
        }
        let misses = dialled.map_or(0, |d| d.1) + 1;
        let next_at = self.clock.now() + facts_backoff_ms(misses);
        self.st(|s| s.facts_dials.insert(id.to_string(), (r.view.machine_id.clone(), misses, next_at)));
        None
    }

    /// `askProvider(r)` (status_poll Behaviour 14): the provider's word, or ours where it cannot be had.
    async fn ask_provider(&self, r: &StatusRecord) -> MachineState {
        let id = r.view.id.as_str();
        let (state, answer) = match (r.provider_state)().await {
            Ok(state) => (state, None),
            Err(e) if is_missing(&e) => (MachineState::Gone, Some(provider_said(&e))),
            Err(_) => return machine_state_of(r.view.phase),
        };
        if state == MachineState::Running {
            self.note_metrics_gap(r).await;
        }
        let now = self.clock.now();
        self.st(|s| {
            s.reconciled.insert(id.to_string(), (state, now));
            if state == MachineState::Running {
                meter_of(s, id).awake_until = Some(now);
            }
            if state == MachineState::Gone {
                let sighting = GoneSighting { by: GoneSeenBy::StatusPoll, at: now as f64, answer };
                s.gone_reasons.insert(id.to_string(), gone_words(&r.view.machine_id, Some(&sighting)));
            }
        });
        state
    }

    /// `machineState(r, reconcile, reachFailed)` (status_poll Behaviour 16).
    async fn machine_state(&self, r: &StatusRecord, reconcile: Reconcile, reach_failed: bool) -> MachineState {
        if reconcile == Reconcile::Always {
            return self.ask_provider(r).await;
        }
        let phase_state = machine_state_of(r.view.phase);
        if !reach_failed {
            return phase_state;
        }
        let known = self.st(|s| s.reconciled.get(&r.view.id).cloned());
        if let Some((state, at)) = known {
            if state == phase_state && self.clock.now() - at < self.reconcile_min() {
                return state;
            }
        }
        self.ask_provider(r).await
    }

    /// `noteMetricsGap(r)` (status_poll Behaviour 15): logged once per spell, never a verdict.
    async fn note_metrics_gap(&self, r: &StatusRecord) {
        let Some(metrics) = r.metrics.clone() else { return };
        let id = r.view.id.as_str();
        let gap = match metrics().await {
            Ok(()) => {
                self.st(|s| s.doubted.shift_remove(id));
                return;
            }
            Err(e) if is_missing(&e) => provider_said(&e),
            Err(_) => return,
        };
        let machine = r.view.machine_id.as_str();
        let repeat = self.st(|s| {
            if s.doubted.get(id).map(String::as_str) == Some(machine) {
                return true;
            }
            s.doubted.insert(id.to_string(), machine.to_string());
            false
        });
        if !repeat {
            warn(&format!("host metrics for {machine} (workspace {id}) answered {gap}; the state read says running and the record follows the state read"));
        }
    }

    /// `probesOf(r, reader)`: this reader's probes of this workspace's current machine.
    fn probe_memory(&self, r: &StatusRecord, reader: StatusReader) -> Probes {
        self.st(|s| {
            let book = s.probes.entry(reader).or_default();
            if let Some(known) = book.get(&r.view.id) {
                if known.machine_id == r.view.machine_id {
                    return known.clone();
                }
            }
            let fresh = Probes { machine_id: r.view.machine_id.clone(), last: None, shown: ReachState::Reachable };
            book.insert(r.view.id.clone(), fresh.clone());
            fresh
        })
    }

    fn update_probe(&self, reader: StatusReader, id: &str, f: impl FnOnce(&mut Probes)) {
        self.st(|s| {
            if let Some(p) = s.probes.get_mut(&reader).and_then(|book| book.get_mut(id)) {
                f(p);
            }
        });
    }

    /// `suspectOf(r)`: the spell this workspace's current machine is in, begun now when there is none.
    /// The spell this workspace's current machine is in, as the object the TS passes around: a judge that holds it
    /// keeps reading and writing it after another read has dropped it from the map.
    fn suspect_of(&self, r: &StatusRecord) -> Rc<RefCell<Suspect>> {
        let now = self.clock.now();
        self.st(|s| {
            if let Some(known) = s.suspects.get(&r.view.id) {
                if known.borrow().machine_id == r.view.machine_id {
                    return known.clone();
                }
            }
            let fresh = Rc::new(RefCell::new(Suspect { machine_id: r.view.machine_id.clone(), bad_since: now, probe: None, zombie: None }));
            s.suspects.insert(r.view.id.clone(), fresh.clone());
            fresh
        })
    }

    fn drop_suspect(&self, id: &str) {
        self.st(|s| {
            s.suspects.shift_remove(id);
        });
    }

    /// Starts the exec probe of a suspect that has none in flight (status_poll Behaviour 8; not awaited).
    fn start_probe_if_idle(&self, r: &StatusRecord) {
        let spell = self.suspect_of(r);
        let idle = spell.borrow().probe.is_none();
        if idle {
            drop(self.start_probe(r, &spell));
        }
    }

    /// The exec probe of a suspect: one future, shared by every reader that waits on it and run to its end on its own.
    fn start_probe(&self, r: &StatusRecord, spell: &Rc<RefCell<Suspect>>) -> Shared<LocalBoxFuture<'static, Option<String>>> {
        let timeout = self.zombie_probe_timeout();
        let clock = self.clock.clone();
        let exec = r.exec.clone();
        let mine = spell.clone();
        let probe: Shared<LocalBoxFuture<'static, Option<String>>> = (async move {
            let fault = probe_exec(exec, timeout, clock).await;
            mine.borrow_mut().probe = None;
            fault
        })
        .boxed_local()
        .shared();
        spell.borrow_mut().probe = Some(probe.clone());
        tokio::task::spawn_local(probe.clone());
        probe
    }

    /// `judge(r, reach, opts)` (status_poll Behaviour 12): the verdict on a slow or silent reach of a running machine.
    async fn judge(&self, r: &StatusRecord, reach: ReachStatus, opts: StatusListOptions) -> (MachineState, ReachStatus, Option<String>) {
        let reconcile = opts.reconcile.unwrap_or_default();
        let id = r.view.id.as_str();
        let spell = self.suspect_of(r);
        let elapsed = self.clock.now() - spell.borrow().bad_since;
        if spell.borrow().zombie.is_none() && elapsed < self.zombie_window() {
            let state = self.machine_state(r, reconcile, reach.state == ReachState::Unreachable).await;
            return (state, reach, None);
        }
        let provider = self.machine_state(r, reconcile, true).await;
        if provider != MachineState::Running {
            self.drop_suspect(id);
            return (provider, reach, None);
        }
        let zombie = spell.borrow().zombie.clone();
        if let Some(zombie) = zombie {
            return (provider, ReachStatus { state: ReachState::Zombie, ..reach }, Some(zombie));
        }
        if opts.zombie_probe == Some(false) {
            return (provider, reach, None);
        }
        let held = spell.borrow().probe.clone();
        let probe = match held {
            Some(probe) => probe,
            None => self.start_probe(r, &spell),
        };
        let Some(fault) = probe.await else {
            spell.borrow_mut().bad_since = self.clock.now();
            return (provider, reach, None);
        };
        let bad_since = spell.borrow().bad_since;
        let reason = format!(
            "{} reports running at the provider; reach {} since {} ({} s); exec probe \"{}\" {}",
            r.view.machine_id,
            reach_word(reach.state),
            iso(bad_since),
            number_string(js_round(elapsed as f64 / 1000.0)),
            ZOMBIE_PROBE_CMD,
            fault
        );
        spell.borrow_mut().zombie = Some(reason.clone());
        warn(&format!("zombie on {} (workspace {id}): {reason}", r.view.machine_id));
        (provider, ReachStatus { state: ReachState::Zombie, ..reach }, Some(reason))
    }

    /// The bus's `workspace.status` listener: a row the runtime pushed is the baseline the next poll is held against.
    pub(super) fn note_pushed(&self, e: &EventUnion) {
        if let EventUnion::WorkspaceStatus(s) = e {
            let (id, key) = (s.event.status.id.clone(), row_key(&s.event.status));
            self.st(|st| {
                st.last_emitted.insert(id, key);
            });
        }
    }

    /// `guarded("cost", () => costTick(only))`: spawned, and a failure warned.
    pub(super) fn guard_cost(&self, only: Option<String>) {
        let me = self.me.clone();
        tokio::task::spawn_local(async move {
            let Some(t) = me.upgrade() else { return };
            if let Err(e) = t.cost_tick(only.as_deref()).await {
                warn(&format!("cost tick failed {e}"));
            }
        });
    }

    /// `guarded("status poll", () => pollTick(only))`: spawned, and a failure warned.
    fn guard_poll(&self, only: Option<HashSet<String>>) {
        let me = self.me.clone();
        tokio::task::spawn_local(async move {
            let Some(t) = me.upgrade() else { return };
            if let Err(e) = t.poll_tick(only.as_ref()).await {
                warn(&format!("status poll tick failed {e}"));
            }
        });
    }

    /// The last watcher's release stops both timers and forgets what was sent (status_poll Behaviour 19).
    fn release_watch(&self) {
        let (stop_cost, stop_poll) = self.st(|s| {
            s.watchers -= 1;
            if s.watchers > 0 {
                return (None, None);
            }
            s.last_emitted.clear();
            s.sent_figures.clear();
            (s.stop_cost.take(), s.stop_poll.take())
        });
        for stop in [stop_cost, stop_poll].into_iter().flatten() {
            stop();
        }
    }

    /// `pollTick(only)` (status_poll Behaviour 20): the rows of the records not moved while they were built, each
    /// emitted when it differs from the last row sent for its workspace.
    async fn poll_tick(&self, only: Option<&HashSet<String>>) -> Result<(), String> {
        let records: Vec<StatusRecord> =
            self.records().await?.into_iter().filter(|r| only.is_none_or(|ids| ids.contains(&r.view.id))).collect();
        let statuses = self.statuses_of(&records, StatusListOptions { reconcile: Some(Reconcile::OnFailure), ..Default::default() }).await;
        let stamp = |r: &StatusRecord| {
            format!("{} {} {} {}", r.generation, r.view.machine_id, r.unreached.as_deref().unwrap_or(""), r.away.as_deref().unwrap_or(""))
        };
        let built: IndexMap<String, String> = records.iter().map(|r| (r.view.id.clone(), stamp(r))).collect();
        let now: IndexMap<String, String> = self.records().await?.iter().map(|r| (r.view.id.clone(), stamp(r))).collect();
        let kept: Vec<WorkspaceStatus> = statuses.iter().filter(|s| now.get(&s.id) == built.get(&s.id)).cloned().collect();
        if let Some(on_polled) = &self.options.on_polled {
            on_polled(&kept);
        }
        for status in &statuses {
            if now.get(&status.id) != built.get(&status.id) {
                continue;
            }
            let key = row_key(status);
            if self.st(|s| s.last_emitted.get(&status.id) == Some(&key)) {
                continue;
            }
            self.st(|s| {
                s.last_emitted.insert(status.id.clone(), key);
            });
            (self.options.emit)(EventUnion::WorkspaceStatus(Sequenced {
                event: WorkspaceStatusEvent { status: status.clone() },
                seq: None,
            }));
        }
        Ok(())
    }
}
