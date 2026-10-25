// SPDX-License-Identifier: AGPL-3.0-only
//! The row builder and the poller of packages/runtime/src/status.ts at 65f6b89c7. Spec: spec/status_poll.md.
use std::rc::Rc;

use wsp_wire::views::workspace_more::WorkspaceStatus;

use super::{StatusListOptions, StatusTracker, StatusWatchOptions};

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
    pub async fn list(self: &Rc<Self>, opts: StatusListOptions) -> Result<Vec<WorkspaceStatus>, String> {
        todo!()
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
    pub fn watch(self: &Rc<Self>, opts: StatusWatchOptions) -> Rc<dyn Fn()> {
        todo!()
    }

    /// Whether a watcher holds it now.
    pub fn watched(&self) -> bool {
        todo!()
    }

//   const poll: StatusApi["poll"] = async ids => {
//     if (watchers > 0 && ids.length > 0) await pollTick(new Set(ids)).catch((e: unknown) => console.warn("status poll tick failed", e));
//   };
    /// Polls these workspaces now, between ticks, and pushes each row that moved, while a watcher holds the poll.
    pub async fn poll(self: &Rc<Self>, ids: &[String]) {
        todo!()
    }
}
