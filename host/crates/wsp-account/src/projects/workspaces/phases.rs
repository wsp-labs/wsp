// SPDX-License-Identifier: AGPL-3.0-only
//! A workspace's machine moves (nap, wake, resize, upgrade, rebuild), the record's own changes, the daemon, delete
//! and forget. workspaces.ts:22-65, 289-645 at 65f6b89c7; spec/workspaces_phases.md. The engine's workspace and
//! machine calls go through the context (`ws_*`, `machine_*`), answering `EngineError` so the wake can tell an
//! unanswered resume, a deleted disk and a missing machine apart.
use std::rc::Rc;

use async_trait::async_trait;
use wsp_core::types::wiring::SizeAsk;
use wsp_engine::errors::EngineError;
use wsp_engine::lifecycle::WakeResult;
use wsp_wire::types::workspace_look::WorkspaceLook;
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::capabilities::Capabilities;
use wsp_wire::wire::machine_link::MachineShape;
use wsp_words::exit::HostError;

use super::seam::{Base, Entry};

/// How long the stop before a move to another size waits on the provider. workspaces.ts:24.
pub const RESIZE_NAP_MS: u64 = 30 * 60_000;

/// `lifecycleOf(entry).budgets.resumeAsks`: how long and how often the host asks a provider to resume again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResumeAsks {
    pub for_ms: i64,
    pub every_ms: i64,
}

/// What `machine.state()` reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineRead {
    Running,
    Paused,
    Gone,
    Other,
}

/// What the phases half reads off the runtime context, past `Base`.
#[async_trait(?Send)]
pub trait PhasesCtx: Base {
    async fn unfork(&self, entry: &Entry) -> Result<(), EngineError>;
    async fn sub_lost(&self, entry: &Entry, line: &str) -> Result<(), HostError>;
    fn tell_threads(&self, workspace_id: &str, line: &str);
    async fn emit_status(&self, entry: &Entry, reach: &str, words: Option<&str>) -> Result<(), HostError>;
    fn follow_machine(&self, entry: &Entry);
    fn reach_of(&self, entry: &Entry) -> String;
    async fn nap_with(&self, id: &str) -> Result<WorkspaceView, HostError>;
    fn refuse_pauseless(&self, entry: &Entry, act: &str) -> Result<(), HostError>;
    fn talks_to_tree_on(&self, id: &str, origin: Option<&Caller>) -> bool;
    fn refuse_sub_wake(&self, entry: &Entry) -> Result<(), HostError>;
    /// `ctx.recoverGone(entry)`: the phase a gone record left for, none where it stays gone.
    async fn recover_gone(&self, entry: &Entry) -> Result<Option<WorkspacePhase>, HostError>;
    /// `machine.state()`.
    async fn machine_state(&self, entry: &Entry) -> Result<MachineRead, EngineError>;
    /// `providerSaid(e)`.
    fn provider_said(&self, e: &EngineError) -> String;
    /// `settled(ctx.settleGone(entry, goneWords(...)))`.
    async fn settle_gone(&self, entry: &Entry, by: &str, at: i64, answer: Option<String>) -> Result<bool, HostError>;
    async fn adopt_pause(&self, entry: &Entry) -> Result<(), HostError>;
    async fn runs_under_napping(&self, entry: &Entry) -> Result<bool, HostError>;
    async fn adopt_running(&self, entry: &Entry) -> Result<(), HostError>;
    async fn proven(&self, entry: &Entry) -> Result<WorkspaceView, HostError>;
    fn resume_asks(&self, entry: &Entry) -> Option<ResumeAsks>;
    /// `ctx.sleeps(ms)` on the clock.
    async fn sleeps(&self, ms: i64);
    /// `tookTheResume(await readsState(machine))`.
    async fn took_the_resume(&self, entry: &Entry) -> bool;
    async fn ws_wake(&self, entry: &Entry, landed: bool) -> Result<WakeResult, EngineError>;
    async fn ws_rebuild(&self, entry: &Entry) -> Result<(), EngineError>;
    async fn ws_upgrade(&self, entry: &Entry) -> Result<(), EngineError>;
    fn ws_current_phase(&self, entry: &Entry) -> WorkspacePhase;
    fn ws_note_paused(&self, entry: &Entry);
    fn arm_late_read(&self, entry: &Entry);
    /// Whether `entry.machine.resumeAt` exists.
    fn can_resume_at(&self, entry: &Entry) -> bool;
    fn provider_of(&self, entry: &Entry) -> Option<String>;
    /// `backendFor(record).limits?.()` (failure ignored) then its capabilities.
    async fn backend_capabilities(&self, entry: &Entry) -> Capabilities;
    fn thread_runs(&self, thread_id: &str) -> bool;
    async fn shape_of(&self, entry: &Entry) -> Option<MachineShape>;
    fn size_built(&self, shape: Option<&MachineShape>, asked: SizeAsk) -> SizeAsk;
    async fn status_now(&self, entry: &Entry) -> Result<(), HostError>;
    fn refuse_cannot(&self, entry: &Entry, cap: &str, act: &str) -> Result<(), HostError>;
    fn keeps_images(&self, entry: &Entry) -> bool;
    fn pause_keeps_disk(&self, entry: &Entry) -> bool;
    fn name_given(&self, name: &str) -> String;
    fn name_refusal(&self, name: &str) -> Option<String>;
    /// `ctx.putLook(record, key, value)` on the entry's record.
    fn put_look(&self, entry: &Entry, key: &str, value: Option<serde_json::Value>);
    /// `ctx.servedByItsComputer(entry)`.
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String>;
    fn computer_of(&self, entry: &Entry) -> String;
    fn has_deploy_daemon(&self, entry: &Entry) -> bool;
    async fn deploy_daemon_on(&self, entry: &Entry) -> Result<(), HostError>;
    /// `ctx.deleteQueued(id)`: a future where the record is a held spawn.
    async fn delete_queued(&self, id: &str) -> Option<Result<(), HostError>>;
    async fn refuse_changed(&self, path: &str) -> Result<(), HostError>;
    fn path_exists(&self, path: &str) -> bool;
    async fn cut_off(&self, entry: &Entry) -> Result<(), HostError>;
    fn end_sessions(&self, id: &str, reason: &str);
    async fn drop_checkpoints(&self, entry: &Entry, thread_id: &str) -> Result<(), HostError>;
    async fn remove_worktree_ending(&self, entry: &Entry) -> Result<(), HostError>;
    async fn drop_thread_files(&self, entry: &Entry, thread_ids: Vec<String>) -> Result<(), HostError>;
    /// `kindWords(kind).driven`.
    fn driven(&self, entry: &Entry) -> bool;
    async fn machine_kill(&self, entry: &Entry) -> Result<(), EngineError>;
    async fn drop_workspace(&self, id: &str) -> Result<(), HostError>;
    async fn read_gone(&self, entry: &Entry) -> Result<String, HostError>;
    fn idle_touch(&self, id: &str);
    fn has_restart_daemon(&self, entry: &Entry) -> bool;
    async fn restart_daemon_of(&self, entry: &Entry) -> Result<(), HostError>;
    fn shared_daemon(&self, entry: &Entry) -> bool;
    fn shared_daemon_replaced(&self);
    fn forget_polled_reach(&self, id: &str);
    async fn push_status(&self, entry: &Entry) -> Result<(), HostError>;
    /// `ctx.failedCreates.delete(id)`: whether one was there.
    fn forget_failed_create(&self, id: &str) -> bool;
}

// ts: workspaces.ts:31-65
//|   /** A wake the provider refused because it deleted the paused machine's disk, which no later resume brings back: a
//|    * sub-thread is released and its next send starts anew, and any other workspace goes onto a new machine from the
//|    * image here, since the files the replacement would lose are already gone. */
//|   const diskGone = async (entry: LiveWorkspace, e: DiskGoneError): Promise<WorkspaceView> => {
//|     console.warn(`wake of ${entry.record.id}: ${e.machineId} cannot resume: ${e.said}`);
//|     if (entry.record.sub !== undefined) {
//|       const line = diskGoneReleasedLine(e.message);
//|       // Boat lists a box whose disk it deleted until someone deletes it, and the release forgets the box's id.
//|       await ctx.unfork(entry).catch((x: unknown) => console.warn(`wake of ${entry.record.id}: ${e.machineId} was not deleted: ${x instanceof Error ? x.message : String(x)}`));
//|       entry.record.diskGone = line;
//|       await ctx.subLost(entry, line);
//|       ctx.tellThreads(entry.record.id, () => line);
//|       throw Object.assign(new Error(line), { kind: "conflict" });
//|     }
//|     const old = entry.record.machineId;
//|     try {
//|       await entry.ws.rebuild();
//|     } catch (x) {
//|       entry.record.phase = entry.ws.currentPhase;
//|       await ctx.persist(entry.record);
//|       const words = `${e.message}, and no new machine started: ${x instanceof Error ? x.message : String(x)}`;
//|       await ctx.emitStatus(entry, "napping", words);
//|       throw new Error(words);
//|     }
//|     const line = diskGoneReplacedLine(e.message);
//|     ctx.followMachine(entry);
//|     delete entry.record.wakeRefused;
//|     entry.record.diskGone = line;
//|     await ctx.persist(entry.record);
//|     bus.emit({ type: "workspace.upgraded", workspaceId: entry.record.id, machineId: entry.record.machineId });
//|     console.warn(`wake of ${entry.record.id}: ${old} replaced by ${entry.record.machineId}`);
//|     ctx.tellThreads(entry.record.id, () => line);
//|     await ctx.emitStatus(entry, ctx.reachOf(entry), line);
//|     return ctx.view(entry.record);
//|   };
/// A wake the provider refused because it deleted the paused machine's disk (spec rule 2).
pub async fn disk_gone(ctx: &Rc<dyn PhasesCtx>, entry: &Entry, e: EngineError) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:289-293
//|     async nap(id, origin) {
//|       ctx.spawnGuard("pause", origin);
//|       ctx.refusePauseless(await ctx.entryOf(id, origin), "be paused");
//|       return ctx.napWith(id);
//|     },
pub async fn nap(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:295-412
//|     async wake(id, origin) {
//|       if (ctx.talksToTreeOn(id, origin)) return WorkspaceTalked.parse(await workspaces.wake(id));
//|       const entry = await ctx.entryOf(id, origin, { act: "wake" });
//|       ctx.refuseSubWake(entry);
//|       await ctx.copyBlocked(entry);
//|       if (entry.waking) return entry.waking;
//|       if (entry.record.phase === "gone") {
//|         const left = await ctx.recoverGone(entry);
//|         if (left === undefined) throw new Error(goneRefusal(entry.record.name, "wake", entry.record.gone));
//|         // A record that left gone for napping is a machine the provider holds paused: the wake goes on and resumes it.
//|         if (left === "running") return ctx.view(entry.record);
//|       }
//|       if (entry.napping) await entry.napping.catch(() => {});
//|       // A wake nobody should need is the one sign the provider paused the machine on its own, or lost it, and a wake of
//|       // a machine the provider runs would be refused with its words: one read settles any, and the record follows the fact.
//|       if (entry.record.phase === "running") {
//|         let answer: string | undefined;
//|         const read = await entry.machine.state().catch((e: unknown) => {
//|           if (!isMissing(e)) return "running";
//|           answer = providerSaid(e);
//|           return "gone";
//|         });
//|         if (read === "gone" && settled(await ctx.settleGone(entry, goneWords(entry.record.machineId, { by: "wake", at: clock.now(), ...(answer !== undefined ? { answer } : {}) })))) {
//|           throw new Error(goneRefusal(entry.record.name, "wake", entry.record.gone));
//|         }
//|         if (read === "paused") await ctx.adoptPause(entry);
//|       } else if (await ctx.runsUnderNapping(entry)) await ctx.adoptRunning(entry);
//|       // A wake that began while this one read the machine is the one this caller waits on.
//|       if (entry.waking) return entry.waking;
//|       // A running workspace has nothing to wake, whatever its kind; only a real resume asks the machine for one.
//|       if (entry.record.phase === "running") return entry.unchecked === true ? ctx.proven(entry) : ctx.view(entry.record);
//|       ctx.refusePauseless(entry, "be woken");
//|       entry.waking = (async () => {
//|         // The stop the person pulls from the row. It aborts the provider call the ask is on rather than walking away
//|         // from one that keeps running: an abandoned resume would go on to run its cap out, write its line back onto
//|         // a row that reads Paused, and leave a second lifecycle wake beside the next one.
//|         const stop = new AbortController();
//|         entry.wakeStop = stop;
//|         const stopped = (): boolean => stop.signal.aborted;
//|         /** Resolves as its promise does, or at once when the stop is pulled; only the wait between two asks needs
//|          * this, since the abort ends an ask on its own. */
//|         const orStopped = <T>(p: Promise<T>): Promise<T | "stopped"> =>
//|           stopped()
//|             ? Promise.resolve("stopped" as const)
//|             : Promise.race([p, new Promise<"stopped">(resolve => stop.signal.addEventListener("abort", () => resolve("stopped"), { once: true }))]);
//|         const began = clock.now();
//|         // How the backend has the host ask again after a resume its provider did not take; none means once.
//|         const asks = ctx.lifecycleOf(entry).budgets.resumeAsks;
//|         const wakeAsks = asks === undefined ? 1 : wakeAsksIn(asks.forMs, asks.everyMs);
//|         try {
//|           delete entry.deleteSaid;
//|           entry.record.phase = "waking";
//|           await ctx.persist(entry.record);
//|           await ctx.emitStatus(entry, "napping");
//|           // Ask 1 is the person's wake; every ask after it is the host's own, once a cadence apart, so a provider
//|           // that comes back inside its own outage wakes the machine without the person having to try again. The
//|           // record stays waking between two asks: nothing about the machine changed, only who is asking.
//|           // Set when the read before an ask found the machine already up: that ask sends no second resume and the
//|           // engine's wake goes straight to the guest check, first life ending there as on any other road.
//|           let landed = false;
//|           for (let ask = 1; ; ask++) {
//|             try {
//|               const result = await entry.ws.wake({ landed });
//|               if (stopped()) throw new Error(WAKE_STOPPED);
//|               ctx.followMachine(entry);
//|               delete entry.record.wakeRefused;
//|               await ctx.persist(entry.record);
//|               bus.emit({ type: "workspace.woken", workspaceId: id, machineId: entry.record.machineId });
//|               if (result.reason !== undefined) console.warn(`wake of ${id}: ${result.reason}`);
//|               await ctx.emitStatus(entry, ctx.reachOf(entry), result.reason);
//|               return ctx.view(entry.record);
//|             } catch (e) {
//|               if (!stopped() && e instanceof ResumeUnansweredError && ask < wakeAsks) {
//|                 entry.wakeAsk = { ask, of: wakeAsks };
//|                 delete entry.wakeSaid;
//|                 await ctx.emitStatus(entry, "napping");
//|                 // The cadence is wall time from the wake's start, so the half hour of asking is half an hour: a
//|                 // resume that sat on its cap for half the minute leaves half a minute to wait, and one that ran
//|                 // longer than the cadence is asked again at once.
//|                 if ((await orStopped(sleeps(Math.max(0, began + ask * asks!.everyMs - clock.now())))) !== "stopped") {
//|                   // The retry reads the machine before it asks: a call that hung at the provider can land in the
//|                   // minute since, and a resume is worth sending only while the machine still reads paused.
//|                   landed = tookTheResume(await readsState(entry.machine));
//|                   continue;
//|                 }
//|               }
//|               if (!stopped() && e instanceof DiskGoneError) return await diskGone(entry, e);
//|               // A resume the provider answered 404 for is a sighting like any other: the record settles gone only
//|               // where the reads agree, and the refusal says what went with the machine.
//|               if (!stopped() && isMissing(e) && settled(await ctx.settleGone(entry, goneWords(entry.record.machineId, { by: "wake", at: clock.now(), answer: providerSaid(e) })))) {
//|                 throw new Error(goneRefusal(entry.record.name, "wake", entry.record.gone));
//|               }
//|               // The provider would not resume it for the whole of the asking: the record carries the road out until
//|               // something replaces the machine, since nothing about it changes on its own from here.
//|               const gaveUp = !stopped() && e instanceof ResumeUnansweredError ? wakeGaveUpLine(ask, clock.now() - began) : undefined;
//|               if (gaveUp !== undefined) entry.record.wakeRefused = gaveUp;
//|               // A stop leaves the machine where the abort found it: paused, until the late read says otherwise.
//|               if (stopped()) entry.ws.notePaused();
//|               entry.record.phase = entry.ws.currentPhase;
//|               await ctx.persist(entry.record);
//|               delete entry.wakeAsk;
//|               const unfinished = !stopped() && e instanceof RestoreUnfinishedError ? noCommandsYetLine(entry.record.name, e.message) : undefined;
//|               const words = stopped() ? WAKE_STOPPED : (gaveUp ?? unfinished ?? (e instanceof Error ? e.message : String(e)));
//|               if (!stopped()) console.warn(`wake of ${id} failed: ${words}`);
//|               await ctx.emitStatus(entry, "napping", words);
//|               ctx.armLateRead(entry);
//|               throw stopped() ? new Error(WAKE_STOPPED) : gaveUp !== undefined ? new Error(gaveUp) : unfinished !== undefined ? new Error(unfinished) : e;
//|             }
//|           }
//|         } finally {
//|           delete entry.wakeStop;
//|           delete entry.wakeSaid;
//|           delete entry.wakeAsk;
//|           delete entry.waking;
//|         }
//|       })();
//|       return entry.waking;
//|     },
/// Wakes a workspace, joining a wake in flight (spec rule 4).
pub async fn wake(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:414-463
//|     async resize(id, size, origin) {
//|       const entry = await ctx.entryOf(id, origin, { act: "wake" });
//|       ctx.spawnGuard("resize", origin);
//|       ctx.refusePauseless(entry, "be moved to another size");
//|       ctx.refuseSubWake(entry);
//|       const at = ctx.backendFor(entry.record);
//|       if (entry.machine.resumeAt === undefined) throw Object.assign(new Error(refusalLine(resizeNoMoveLine(providerKeyName(ctx.providerOf(entry.record) ?? "the provider")), RESIZE_NO_MOVE_FIX)), { kind: "invalid" });
//|       await at.limits?.().catch(() => undefined);
//|       const offer = at.capabilities.sizes.find(s => s.cpu === size.cpu && s.memMb === size.memMb);
//|       if (offer === undefined) throw Object.assign(new Error(refusalLine(sizeRefusal(sizeWord(size), at.capabilities.sizes), SIZE_PICK_FIX)), { kind: "invalid" });
//|       if (entry.record.size.cpu === size.cpu && entry.record.size.memMb === size.memMb) return ctx.view(entry.record);
//|       const threads = new Set([...ctx.sessions.values()].filter(s => s.view.workspaceId === id && s.view.threadId !== undefined).map(s => s.view.threadId!));
//|       if ([...threads].some(t => ctx.threadRuns(t))) throw Object.assign(new Error(refusalLine(resizeBusyLine(entry.record.name), RESIZE_BUSY_FIX)), { kind: "conflict" });
//|       const before = entry.record.size;
//|       entry.napWaitMs = RESIZE_NAP_MS;
//|       try {
//|         await ctx.napWith(id);
//|       } finally {
//|         delete entry.napWaitMs;
//|       }
//|       entry.resizeTo = { cpu: size.cpu, memMb: size.memMb };
//|       entry.record.size = { ...entry.record.size, cpu: size.cpu, memMb: size.memMb };
//|       // The wake checks the machine against the shape it was made at, which a move changes on purpose.
//|       delete entry.record.shape;
//|       // The record takes the size the provider reports: a resume can land at the new size before the wake fails, and
//|       // every plain resume after keeps it. A shape nobody read stays unset, since a stale one fails every later wake.
//|       const follow = async (asked: { cpu: number; memMb: number }): Promise<{ cpu: number; memMb: number }> => {
//|         const shape = await ctx.shapeOf(entry.machine);
//|         if (shape !== undefined) entry.record.shape = shape;
//|         const landed = ctx.sizeBuilt(shape, asked);
//|         entry.record.size = { ...entry.record.size, cpu: landed.cpu, memMb: landed.memMb };
//|         const rate = at.capabilities.sizes.find(s => s.cpu === landed.cpu && s.memMb === landed.memMb)?.rateUsdPerHour;
//|         if (entry.record.sub !== undefined && rate !== undefined) entry.record.sub.rateUsdPerHour = rate;
//|         await ctx.persist(entry.record);
//|         return landed;
//|       };
//|       try {
//|         await workspaces.wake(id, origin);
//|       } catch (e) {
//|         await follow(before);
//|         throw e;
//|       } finally {
//|         delete entry.resizeTo;
//|       }
//|       const landed = await follow(size);
//|       const moved = landed.cpu === size.cpu && landed.memMb === size.memMb;
//|       await ctx.statusNow(entry);
//|       if (!moved) throw Object.assign(new Error(refusalLine(resizeLandedLine(entry.record.name, landed, size), RESIZE_AGAIN_FIX)), { kind: "conflict" });
//|       return ctx.view(entry.record);
//|     },
pub async fn resize(ctx: &Rc<dyn PhasesCtx>, id: &str, size: SizeAsk, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:465-471
//|     async stopWake(id, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       const waking = entry.waking;
//|       entry.wakeStop?.abort();
//|       await waking?.catch(() => {});
//|       return ctx.view(entry.record);
//|     },
pub async fn stop_wake(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:473-481
//|     async upgrade(id, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       ctx.refuseCannot(entry, "replacesMachine", "have its machine replaced");
//|       await entry.ws.upgrade();
//|       ctx.followMachine(entry);
//|       await ctx.persist(entry.record);
//|       bus.emit({ type: "workspace.upgraded", workspaceId: id, machineId: entry.record.machineId });
//|       return ctx.view(entry.record);
//|     },
pub async fn upgrade(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:483-527
//|     async rebuild(id, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       ctx.refuseCannot(entry, "replacesMachine", "be rebuilt");
//|       if (entry.waking) await entry.waking.catch(() => {});
//|       if ((await ctx.recoverGone(entry)) !== undefined) return ctx.view(entry.record);
//|       const at = ctx.backendFor(entry.record);
//|       // Where the pause keeps the disk no nap stored a vault, so a rebuild takes one off the running machine before
//|       // the replacement, over the cap or unreadable going on with none. A napped one holds its home on its paused
//|       // disk, so it is woken first and the vault taken live, not the one a nap stored (this rule stores none); a wake
//|       // that cannot land refuses the rebuild rather than replacing with no backup. Where the pause does not keep the
//|       // disk, the rebuild reads the vault the nap stored.
//|       if (ctx.keepsImages(at) && ctx.pauseKeepsDisk(at) && entry.record.phase !== "running") {
//|         const asleep = entry.record.machineId;
//|         try {
//|           await workspaces.wake(id, origin);
//|         } catch (e) {
//|           throw Object.assign(new Error(`${entry.record.name} could not be woken to back up before the rebuild: ${e instanceof Error ? e.message : String(e)}`), { kind: "conflict" });
//|         }
//|         // A wake that met a deleted disk has already moved the record onto a new machine from the image.
//|         if (entry.record.machineId !== asleep) return ctx.view(entry.record);
//|       }
//|       const old = entry.record.machineId;
//|       const fromRunning = ctx.keepsImages(at) && ctx.pauseKeepsDisk(at) && entry.record.phase === "running";
//|       const napVault = fromRunning ? undefined : (await store.getBlob(VAULTS, id)) !== undefined;
//|       if (fromRunning) {
//|         try {
//|           await entry.ws.upgrade();
//|         } catch (e) {
//|           console.warn(`rebuild of ${id}: the running machine gave no vault (${e instanceof Error ? e.message : String(e)}); replacing with none`);
//|           await entry.ws.rebuild();
//|         }
//|       } else {
//|         await entry.ws.rebuild();
//|       }
//|       ctx.followMachine(entry);
//|       delete entry.record.wakeRefused;
//|       await ctx.persist(entry.record);
//|       bus.emit({ type: "workspace.upgraded", workspaceId: id, machineId: entry.record.machineId });
//|       const reason = fromRunning
//|         ? `rebuilt: ${old} replaced by ${entry.record.machineId}`
//|         : `rebuilt: ${old} replaced by ${entry.record.machineId}, ${napVault ? "nap-time vault imported" : "no vault to import"}`;
//|       console.warn(`rebuild of ${id}: ${reason}`);
//|       await ctx.emitStatus(entry, ctx.reachOf(entry), reason);
//|       return ctx.view(entry.record);
//|     },
pub async fn rebuild(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:529-540
//|     async rename(id, typed, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       const name = ctx.nameGiven(typed);
//|       if (entry.record.name === name) return ctx.view(entry.record);
//|       const refusal = ctx.nameRefusal(name);
//|       if (refusal !== undefined) throw Object.assign(new Error(refusal), { kind: "conflict" });
//|       entry.record.name = name;
//|       await ctx.persist(entry.record);
//|       await store.put(WORKSPACE_NAMES, id, { workspaceId: id, name, project: entry.record.project } satisfies NamedWorkspace);
//|       bus.emit({ type: "workspace.renamed", workspaceId: id, name });
//|       return ctx.view(entry.record);
//|     },
pub async fn rename(ctx: &Rc<dyn PhasesCtx>, id: &str, typed: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:542-549
//|     async look(id, look, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       ctx.putLook(entry.record, "theme", look.theme);
//|       ctx.putLook(entry.record, "glyph", look.glyph);
//|       await ctx.persist(entry.record);
//|       bus.emit({ type: "workspace.look", workspaceId: id, theme: entry.record.theme ?? null, glyph: entry.record.glyph ?? null });
//|       return ctx.view(entry.record);
//|     },
pub async fn look(ctx: &Rc<dyn PhasesCtx>, id: &str, look: WorkspaceLook, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:551-560
//|     async updateDaemon(id, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       if (ctx.servedByItsComputer(entry) !== undefined) {
//|         throw new Error(placeServesDaemonLine(entry.record.name, ctx.computerOf(entry)));
//|       }
//|       const deploy = ctx.moduleOf(entry.record.kind).deployDaemon;
//|       if (deploy === undefined) throw new Error("this runtime cannot deploy a daemon; the host wires the bundle");
//|       if (entry.record.phase !== "running") throw new Error(`wake ${entry.record.name} before updating its daemon`);
//|       await ctx.deployDaemonOn(entry, deploy);
//|     },
pub async fn update_daemon(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
    todo!()
}

// ts: workspaces.ts:562-612
//|     async delete(id, origin) {
//|       ctx.spawnGuard("delete", origin);
//|       if (scopeOf(origin) === undefined && !live.has(id) && ctx.failedCreates.delete(id)) {
//|         bus.emit({ type: "workspace.deleted", workspaceId: id });
//|         return;
//|       }
//|       const entry = await ctx.entryOf(id, origin);
//|       if (entry.deleting) return entry.deleting;
//|       // A held spawn is stopped as a stop does, its machine going as it lands where one is being made.
//|       const held = entry.record.phase === "queued" ? ctx.deleteQueued(id) : undefined;
//|       if (held !== undefined) return held;
//|       // A worktree wsp made goes with its record, never over files no commit holds; the project folder, and a
//|       // worktree somebody else made, are never touched.
//|       const tree = entry.record.worktree;
//|       const takes = tree?.made === true && tree.gone !== true && existsSync(tree.path);
//|       if (takes) await ctx.refuseChanged(tree.path);
//|       entry.deleting = (async () => {
//|         try {
//|           delete entry.deleteSaid;
//|           await cutOff(ctx, entry);
//|           ctx.endSessions(id, DELETED_REASON);
//|           const threads = new Set([...threadRecords].flatMap(([threadId, held]) => (held.workspaceId === id ? [threadId] : [])));
//|           for (const s of sessions.values()) if (s.view.workspaceId === id && s.view.threadId !== undefined) threads.add(s.view.threadId);
//|           if (runsInFolder(entry.record.kind)) for (const threadId of threads) await ctx.dropCheckpoints(entry, threadId);
//|           if (takes) await ctx.removeWorktree(entry, false, { ending: true });
//|           // Before the machine goes: on a computer somebody owns the folders the files landed in outlive the workspace.
//|           await ctx.dropThreadFiles(entry, [...threadRecords].flatMap(([threadId, held]) => (held.workspaceId === id ? [threadId] : [])));
//|           // A machine wsp never forked reads running whatever is asked of it, so only a forked one is read back; a spawn
//|           // still waiting has none, a released record's box is gone already, and a record whose cloud left this host
//|           // has nobody to ask.
//|           const standing = entry.record.phase !== "released" && entry.record.phase !== "queued" && entry.record.sub?.cloudGone !== true;
//|           if (standing && !kindWords(entry.record.kind).driven) {
//|             await entry.machine.kill().catch((e: unknown) => {
//|               if (!isMissing(e)) throw e;
//|             });
//|           } else if (standing) {
//|             await ctx.unfork(entry).catch((e: unknown) => {
//|               if (!(e instanceof MachineAliveError)) throw e;
//|               const line = deleteRefusedLine(entry.record.name, e.machineId, e.state);
//|               entry.deleteSaid = { phase: entry.record.phase, line };
//|               console.warn(line);
//|               throw Object.assign(new Error(line), { kind: e.kind });
//|             });
//|           }
//|           await ctx.drop(id);
//|         } finally {
//|           delete entry.deleting;
//|         }
//|       })();
//|       return entry.deleting;
//|     },
/// Deletes a workspace, joining a delete in flight (spec rule 12).
pub async fn delete(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
    todo!()
}

// ts: workspaces.ts:614-624
//|     async forget(id, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       const kind = entry.record.kind;
//|       if (!kindWords(kind).driven) throw Object.assign(new Error(forgetUndrivenRefusal(entry.record.name, machineWord(kind))), { kind: "conflict" });
//|       const state = await readGone(ctx.backendFor(entry.record), entry.machine.id);
//|       if (state !== "gone") {
//|         throw Object.assign(new Error(`${entry.record.name}'s machine ${entry.machine.id} is still ${state}; pause it or delete it at the provider first`), { kind: "conflict" });
//|       }
//|       ctx.endSessions(id, DELETED_REASON);
//|       await ctx.drop(id);
//|     },
pub async fn forget(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
    todo!()
}

// ts: workspaces.ts:626-629
//|     async touch(id, origin) {
//|       await ctx.entryOf(id, origin);
//|       ctx.idle.touch(id);
//|     },
pub async fn touch(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
    todo!()
}

// ts: workspaces.ts:631-645
//|     async restartDaemon(id, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       const start = ctx.moduleOf(entry.record.kind).restartDaemon;
//|       if (start === undefined) throw new Error(`${entry.record.name}'s daemon runs on ${machineWord(entry.record.kind)}, which this host does not hold the process of`);
//|       try {
//|         await start(entry);
//|       } finally {
//|         if (ctx.moduleOf(entry.record.kind).sharedDaemon) ctx.sharedDaemonReplaced();
//|       }
//|       // The poll's last measurement is of the daemon that is gone, and the next one is a poll away: the row would
//|       // go on saying no daemon for that long over a daemon this host has just watched start. Dropped rather than
//|       // replaced with a claim, so the row falls back to what this kind's road says and the next poll measures.
//|       ctx.polledReach.delete(entry.record.id);
//|       await ctx.pushStatus(entry);
//|     },
pub async fn restart_daemon(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
    todo!()
}
