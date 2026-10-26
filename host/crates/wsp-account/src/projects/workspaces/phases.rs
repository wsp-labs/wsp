// SPDX-License-Identifier: AGPL-3.0-only
//! A workspace's machine moves (nap, wake, resize, upgrade, rebuild), the record's own changes, the daemon, delete
//! and forget. workspaces.ts:22-65, 289-645 at 65f6b89c7; spec/workspaces_phases.md. The engine's workspace and
//! machine calls go through the context (`ws_*`, `machine_*`), answering `EngineError` so the wake can tell an
//! unanswered resume, a deleted disk and a missing machine apart.
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::{select, Either, FutureExt};
use serde_json::json;
use wsp_core::console;
use wsp_core::types::internal::{NamedWorkspace, DELETED_REASON, WORKSPACE_NAMES};
use wsp_core::types::wiring::{DeleteSaid, SizeAsk, WakeAsk, WorkspaceRecord, VAULTS};
use wsp_engine::errors::{is_missing, EngineError};
use wsp_engine::lifecycle::WakeResult;
use wsp_engine::Cancel;
use wsp_wire::types::workspace_look::WorkspaceLook;
use wsp_wire::views::workspace::{scope_of, Caller, WorkspacePhase};
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::capabilities::{Capabilities, MachineSizeOffer, WorkspaceSize};
use wsp_wire::wire::machine_link::{place_serves_daemon_line, MachineShape};
use wsp_words::exit::{HostError, Kind};
use wsp_words::place_word::provider_key_name;
use wsp_words::projects::runs_in_folder;
use wsp_words::words::computer::{forget_undriven_refusal, no_commands_yet_line, wake_asks_in, wake_gave_up_line, AcrossAct, WAKE_STOPPED};
use wsp_words::words::thread::{
    delete_refused_line, refusal_line, resize_busy_line, resize_landed_line, resize_no_move_line, RESIZE_AGAIN_FIX, RESIZE_BUSY_FIX,
    RESIZE_NO_MOVE_FIX,
};
use wsp_words::words::units::{disk_gone_released_line, disk_gone_replaced_line, size_refusal, size_word, SIZE_PICK_FIX};
use wsp_words::workspace_state::{gone_refusal, machine_word};

use super::create::{name_of, talked};
use super::seam::{record_of, Base, Entry, EntryAsk};

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
fn rec<T>(entry: &Entry, f: impl FnOnce(&mut WorkspaceRecord) -> T) -> T {
    let live = entry.borrow();
    let mut record = live.record.borrow_mut();

    f(&mut record)
}

fn read_rec<T>(entry: &Entry, f: impl FnOnce(&WorkspaceRecord) -> T) -> T {
    let live = entry.borrow();
    let record = live.record.borrow();

    f(&record)
}

fn phase_of(entry: &Entry) -> WorkspacePhase {
    read_rec(entry, |r| r.phase)
}

/// The engine's error as the TS rethrows it: its words, the kind it was born with (exit.ts:37 reads the exit class off
/// it), and the status and code a provider's answer carried.
fn engine(e: EngineError) -> HostError {
    let kind = match &e {
        EngineError::NotFirstLife { .. } => Some(Kind::NotFirstLife),
        EngineError::VaultTooLarge { .. } => Some(Kind::VaultTooLarge),
        EngineError::Exists { .. } => Some(Kind::Exists),
        EngineError::NoProvider { .. } => Some(Kind::NoProvider),
        EngineError::Invalid { .. } => Some(Kind::Invalid),
        EngineError::PlaceAbsent { .. } => Some(Kind::Other("absent".to_owned())),
        other => other.kind().and_then(|kind| serde_json::to_value(kind).ok()).and_then(|word| word.as_str().map(Kind::parse)),
    };
    HostError { kind, status: e.status().map(f64::from), code: e.code().map(str::to_owned), ..HostError::new(e.to_string()) }
}

fn conflict(message: String) -> HostError {
    HostError::with_kind(message, Kind::Conflict)
}

fn invalid(message: String) -> HostError {
    HostError::with_kind(message, Kind::Invalid)
}

fn workspace_size(size: SizeAsk) -> WorkspaceSize {
    WorkspaceSize { cpu: size.cpu, mem_mb: size.mem_mb }
}

/// The machine and the state a `machineAlive` refusal names. The engine carries it in the message, since it has no
/// variant for the refusal (golden.ts:45).
fn machine_alive(e: &EngineError) -> Option<(String, String)> {
    let EngineError::Other { message, code: Some(code) } = e else {
        return None;
    };
    if code != "machineAlive" {
        return None;
    }
    let machine = message.strip_prefix("machine ")?.split(" is still ").next()?.to_owned();
    let state = message.split(" is still ").nth(1)?.split(" after ").next()?.to_owned();
    Some((machine, state))
}

fn gone_refusal_of(entry: &Entry, action: &str) -> HostError {
    let (name, gone) = read_rec(entry, |r| (r.name.clone(), r.gone.clone()));
    HostError::new(gone_refusal(Some(&name), action, gone.as_deref()))
}

/// One attempt of a wake: the `try` of workspaces.ts:236-257.
async fn wake_attempt(ctx: &Rc<dyn PhasesCtx>, entry: &Entry, id: &str, landed: bool, stopped: &Cancel) -> Result<WorkspaceView, WakeFail> {
    let result = match select(Box::pin(ctx.ws_wake(entry, landed)), Box::pin(stopped.cancelled())).await {
        Either::Left((result, _)) => result.map_err(WakeFail::Engine)?,
        Either::Right(_) => return Err(WakeFail::Stopped),
    };
    if stopped.is_cancelled() {
        return Err(WakeFail::Stopped);
    }
    ctx.follow_machine(entry);
    rec(entry, |r| r.wake_refused = None);
    ctx.persist(record_of(entry)).await.map_err(WakeFail::Host)?;
    let machine_id = read_rec(entry, |r| r.machine_id.clone());
    ctx.emit(json!({ "type": "workspace.woken", "workspaceId": id, "machineId": machine_id })).map_err(WakeFail::Host)?;
    if let Some(reason) = &result.reason {
        console::warn(&format!("wake of {id}: {reason}"));
    }
    let reach = ctx.reach_of(entry);
    ctx.emit_status(entry, &reach, result.reason.as_deref()).await.map_err(WakeFail::Host)?;
    Ok(ctx.view(&record_of(entry)))
}

/// How a wake attempt ended short of an answer: the engine's error, a host error, or the stop the person pulled.
enum WakeFail {
    Engine(EngineError),
    Host(HostError),
    Stopped,
}

impl WakeFail {
    fn words(&self) -> String {
        match self {
            WakeFail::Engine(e) => e.to_string(),
            WakeFail::Host(h) => h.message.clone(),
            WakeFail::Stopped => WAKE_STOPPED.to_owned(),
        }
    }

    fn into_host(self) -> HostError {
        match self {
            WakeFail::Engine(e) => engine(e),
            WakeFail::Host(h) => h,
            WakeFail::Stopped => HostError::new(WAKE_STOPPED),
        }
    }
}

/// The asks of one wake, from the first to the last the backend allows (workspaces.ts:295-412).
#[allow(clippy::too_many_arguments)]
async fn wake_loop(
    ctx: &Rc<dyn PhasesCtx>,
    entry: &Entry,
    id: &str,
    stopped: &Cancel,
    began: i64,
    asks: Option<ResumeAsks>,
    wake_asks: f64,
) -> Result<WorkspaceView, HostError> {
    rec(entry, |r| r.phase = WorkspacePhase::Waking);
    ctx.persist(record_of(entry)).await?;
    ctx.emit_status(entry, "napping", None).await?;
    let mut landed = false;
    let mut ask: u32 = 1;
    loop {
        let fail = match wake_attempt(ctx, entry, id, landed, stopped).await {
            Ok(view) => return Ok(view),
            Err(fail) => fail,
        };
        if !stopped.is_cancelled() {
            if let WakeFail::Engine(EngineError::ResumeUnanswered { .. }) = &fail {
                if f64::from(ask) < wake_asks {
                    entry.borrow_mut().wake_ask = Some(WakeAsk { ask, of: wake_asks as u32 });
                    entry.borrow_mut().wake_said = None;
                    ctx.emit_status(entry, "napping", None).await?;
                    let every = asks.map_or(0, |a| a.every_ms);
                    let wait = (began + i64::from(ask) * every - ctx.clock().now()).max(0);
                    let stopped_meanwhile = stopped.is_cancelled()
                        || matches!(select(Box::pin(ctx.sleeps(wait)), Box::pin(stopped.cancelled())).await, Either::Right(_));
                    if !stopped_meanwhile {
                        landed = ctx.took_the_resume(entry).await;
                        ask += 1;
                        continue;
                    }
                }
            }
        }
        let fail = match fail {
            WakeFail::Engine(e) if !stopped.is_cancelled() && matches!(e, EngineError::DiskGone { .. }) => {
                return disk_gone(ctx, entry, e).await;
            }
            other => other,
        };
        let fail = match fail {
            WakeFail::Engine(e) if !stopped.is_cancelled() && is_missing(&e) => {
                let answer = ctx.provider_said(&e);
                let at = ctx.clock().now();
                if ctx.settle_gone(entry, "wake", at, Some(answer)).await? {
                    return Err(gone_refusal_of(entry, "wake"));
                }
                WakeFail::Engine(e)
            }
            other => other,
        };
        let gave_up = match &fail {
            WakeFail::Engine(EngineError::ResumeUnanswered { .. }) if !stopped.is_cancelled() => {
                Some(wake_gave_up_line(f64::from(ask), (ctx.clock().now() - began) as f64))
            }
            _ => None,
        };
        if let Some(line) = &gave_up {
            rec(entry, |r| r.wake_refused = Some(line.clone()));
        }
        if stopped.is_cancelled() {
            ctx.ws_note_paused(entry);
        }
        let phase = ctx.ws_current_phase(entry);
        rec(entry, |r| r.phase = phase);
        ctx.persist(record_of(entry)).await?;
        entry.borrow_mut().wake_ask = None;
        let unfinished = match &fail {
            WakeFail::Engine(EngineError::RestoreUnfinished { message, .. }) if !stopped.is_cancelled() => {
                let name = read_rec(entry, |r| r.name.clone());
                Some(no_commands_yet_line(&name, message))
            }
            _ => None,
        };
        let words = if stopped.is_cancelled() {
            WAKE_STOPPED.to_owned()
        } else {
            gave_up.clone().or_else(|| unfinished.clone()).unwrap_or_else(|| fail.words())
        };
        if !stopped.is_cancelled() {
            console::warn(&format!("wake of {id} failed: {words}"));
        }
        ctx.emit_status(entry, "napping", Some(&words)).await?;
        ctx.arm_late_read(entry);
        return Err(if stopped.is_cancelled() {
            HostError::new(WAKE_STOPPED)
        } else if let Some(line) = gave_up {
            HostError::new(line)
        } else if let Some(line) = unfinished {
            HostError::new(line)
        } else {
            fail.into_host()
        });
    }
}

/// The whole of a wake that is not joined: the stop, the asks, and the joins cleared however it ends.
async fn wake_run(ctx: Rc<dyn PhasesCtx>, entry: Entry, id: String, stopped: Cancel) -> Result<WorkspaceView, HostError> {
    let began = ctx.clock().now();
    let asks = ctx.resume_asks(&entry);
    let wake_asks = asks.map_or(1.0, |a| wake_asks_in(a.for_ms as f64, a.every_ms as f64));
    entry.borrow_mut().delete_said = None;
    let outcome = wake_loop(&ctx, &entry, &id, &stopped, began, asks, wake_asks).await;
    {
        let mut live = entry.borrow_mut();
        live.wake_stop = None;
        live.wake_said = None;
        live.wake_ask = None;
        live.waking = None;
    }
    outcome
}

/// A wake the provider refused because it deleted the paused machine's disk (spec rule 2).
pub async fn disk_gone(ctx: &Rc<dyn PhasesCtx>, entry: &Entry, e: EngineError) -> Result<WorkspaceView, HostError> {
    let id = read_rec(entry, |r| r.id.clone());
    let machine_id = match &e {
        EngineError::DiskGone { machine_id, .. } => machine_id.clone(),
        _ => read_rec(entry, |r| r.machine_id.clone()),
    };
    let said = match &e {
        EngineError::DiskGone { said, .. } => said.clone(),
        other => other.to_string(),
    };
    let message = e.to_string();
    console::warn(&format!("wake of {id}: {machine_id} cannot resume: {said}"));
    if read_rec(entry, |r| r.sub.is_some()) {
        let line = disk_gone_released_line(&message);
        if let Err(x) = ctx.unfork(entry).await {
            console::warn(&format!("wake of {id}: {machine_id} was not deleted: {x}"));
        }
        rec(entry, |r| r.disk_gone = Some(line.clone()));
        ctx.sub_lost(entry, &line).await?;
        ctx.tell_threads(&id, &line);
        return Err(conflict(line));
    }
    let old = read_rec(entry, |r| r.machine_id.clone());
    if let Err(x) = ctx.ws_rebuild(entry).await {
        let phase = ctx.ws_current_phase(entry);
        rec(entry, |r| r.phase = phase);
        ctx.persist(record_of(entry)).await?;
        let words = format!("{message}, and no new machine started: {x}");
        ctx.emit_status(entry, "napping", Some(&words)).await?;
        return Err(HostError::new(words));
    }
    let line = disk_gone_replaced_line(&message);
    ctx.follow_machine(entry);
    rec(entry, |r| {
        r.wake_refused = None;
        r.disk_gone = Some(line.clone());
    });
    ctx.persist(record_of(entry)).await?;
    let new_machine = read_rec(entry, |r| r.machine_id.clone());
    ctx.emit(json!({ "type": "workspace.upgraded", "workspaceId": id, "machineId": new_machine }))?;
    console::warn(&format!("wake of {id}: {old} replaced by {new_machine}"));
    ctx.tell_threads(&id, &line);
    let reach = ctx.reach_of(entry);
    ctx.emit_status(entry, &reach, Some(&line)).await?;
    Ok(ctx.view(&record_of(entry)))
}

// ts: workspaces.ts:289-293
//|     async nap(id, origin) {
//|       ctx.spawnGuard("pause", origin);
//|       ctx.refusePauseless(await ctx.entryOf(id, origin), "be paused");
//|       return ctx.napWith(id);
//|     },
pub async fn nap(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    let _ = ctx.spawn_guard("pause", origin.as_ref())?;
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    ctx.refuse_pauseless(&entry, "be paused")?;
    ctx.nap_with(id).await
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
    if ctx.talks_to_tree_on(id, origin.as_ref()) {
        let view = Box::pin(wake(ctx, id, None)).await?;
        return talked(view);
    }
    let ask = EntryAsk { act: Some(AcrossAct::Wake), ..EntryAsk::default() };
    let entry = ctx.entry_of(id, origin, ask).await?;
    ctx.refuse_sub_wake(&entry)?;
    ctx.copy_blocked(&entry).await?;
    let waking = entry.borrow().waking.clone();
    if let Some(waking) = waking {
        return waking.await;
    }
    if phase_of(&entry) == WorkspacePhase::Gone {
        match ctx.recover_gone(&entry).await? {
            None => return Err(gone_refusal_of(&entry, "wake")),
            Some(WorkspacePhase::Running) => return Ok(ctx.view(&record_of(&entry))),
            Some(_) => {}
        }
    }
    let napping = entry.borrow().napping.clone();
    if let Some(napping) = napping {
        let _ = napping.await;
    }
    if phase_of(&entry) == WorkspacePhase::Running {
        let mut answer = None;
        let read = match ctx.machine_state(&entry).await {
            Ok(read) => read,
            Err(e) if is_missing(&e) => {
                answer = Some(ctx.provider_said(&e));
                MachineRead::Gone
            }
            Err(_) => MachineRead::Running,
        };
        if read == MachineRead::Gone {
            let at = ctx.clock().now();
            if ctx.settle_gone(&entry, "wake", at, answer).await? {
                return Err(gone_refusal_of(&entry, "wake"));
            }
        }
        if read == MachineRead::Paused {
            ctx.adopt_pause(&entry).await?;
        }
    } else if ctx.runs_under_napping(&entry).await? {
        ctx.adopt_running(&entry).await?;
    }
    let waking = entry.borrow().waking.clone();
    if let Some(waking) = waking {
        return waking.await;
    }
    if phase_of(&entry) == WorkspacePhase::Running {
        if entry.borrow().unchecked {
            return ctx.proven(&entry).await;
        }
        return Ok(ctx.view(&record_of(&entry)));
    }
    ctx.refuse_pauseless(&entry, "be woken")?;
    let stop = Cancel::new();
    entry.borrow_mut().wake_stop = Some(stop.clone());
    let run = wake_run(ctx.clone(), entry.clone(), id.to_owned(), stop);
    let joined = run.boxed_local().shared();
    entry.borrow_mut().waking = Some(joined.clone());
    joined.await
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
    let entry = ctx.entry_of(id, origin.clone(), EntryAsk { act: Some(AcrossAct::Wake), ..EntryAsk::default() }).await?;
    ctx.spawn_guard("resize", origin.as_ref())?;
    ctx.refuse_pauseless(&entry, "be moved to another size")?;
    ctx.refuse_sub_wake(&entry)?;
    if !ctx.can_resume_at(&entry) {
        let provider = ctx.provider_of(&entry).unwrap_or_else(|| "the provider".to_owned());
        return Err(invalid(refusal_line(&resize_no_move_line(&provider_key_name(&provider)), RESIZE_NO_MOVE_FIX)));
    }
    let at: Capabilities = ctx.backend_capabilities(&entry).await;
    let want = workspace_size(size);
    let offered = at.sizes.iter().any(|s| s.cpu == size.cpu && s.mem_mb == size.mem_mb);
    if !offered {
        return Err(invalid(refusal_line(&size_refusal(&size_word(&want), &at.sizes), SIZE_PICK_FIX)));
    }
    let current = read_rec(&entry, |r| (r.size.cpu, r.size.mem_mb));
    if current == (size.cpu, size.mem_mb) {
        return Ok(ctx.view(&record_of(&entry)));
    }
    let mut threads: Vec<String> = Vec::new();
    for s in ctx.session_views() {
        if s.workspace_id == id {
            if let Some(thread) = s.thread_id {
                if !threads.contains(&thread) {
                    threads.push(thread);
                }
            }
        }
    }
    for thread in &threads {
        if ctx.thread_runs(thread) {
            let name = name_of(&entry);
            return Err(conflict(refusal_line(&resize_busy_line(&name), RESIZE_BUSY_FIX)));
        }
    }
    let before = read_rec(&entry, |r| SizeAsk { cpu: r.size.cpu, mem_mb: r.size.mem_mb });
    entry.borrow_mut().nap_wait_ms = Some(RESIZE_NAP_MS);
    let napped = ctx.nap_with(id).await;
    entry.borrow_mut().nap_wait_ms = None;
    napped?;
    entry.borrow_mut().resize_to = Some(size);
    rec(&entry, |r| {
        r.size.cpu = size.cpu;
        r.size.mem_mb = size.mem_mb;
        r.shape = None;
    });
    let woke = wake(ctx, id, origin.clone()).await;
    if let Err(e) = woke {
        let followed = follow_size(ctx, &entry, before, &at.sizes).await;
        entry.borrow_mut().resize_to = None;
        followed?;
        return Err(e);
    }
    entry.borrow_mut().resize_to = None;
    let landed = follow_size(ctx, &entry, size, &at.sizes).await?;
    let moved = landed.cpu == size.cpu && landed.mem_mb == size.mem_mb;
    ctx.status_now(&entry).await?;
    if !moved {
        let name = name_of(&entry);
        return Err(conflict(refusal_line(&resize_landed_line(&name, &workspace_size(landed), &want), RESIZE_AGAIN_FIX)));
    }
    Ok(ctx.view(&record_of(&entry)))
}

/// The size the provider reports after a move, kept on the record (workspaces.ts:324-332).
async fn follow_size(ctx: &Rc<dyn PhasesCtx>, entry: &Entry, asked: SizeAsk, sizes: &[MachineSizeOffer]) -> Result<SizeAsk, HostError> {
    let shape: Option<MachineShape> = ctx.shape_of(entry).await;
    if let Some(shape) = &shape {
        rec(entry, |r| r.shape = Some(shape.clone()));
    }
    let landed = ctx.size_built(shape.as_ref(), asked);
    rec(entry, |r| {
        r.size.cpu = landed.cpu;
        r.size.mem_mb = landed.mem_mb;
    });
    let rate = sizes.iter().find(|s| s.cpu == landed.cpu && s.mem_mb == landed.mem_mb).map(|s| s.rate_usd_per_hour);
    if let Some(rate) = rate {
        rec(entry, |r| {
            if let Some(sub) = r.sub.as_mut() {
                sub.rate_usd_per_hour = Some(rate);
            }
        });
    }
    ctx.persist(record_of(entry)).await?;
    Ok(landed)
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
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    let waking = entry.borrow().waking.clone();
    let stop = entry.borrow().wake_stop.clone();
    if let Some(stop) = stop {
        stop.cancel(None);
    }
    tokio::task::yield_now().await;
    if let Some(waking) = waking {
        let _ = waking.await;
    }
    Ok(ctx.view(&record_of(&entry)))
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
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    ctx.refuse_cannot(&entry, "replacesMachine", "have its machine replaced")?;
    ctx.ws_upgrade(&entry).await.map_err(engine)?;
    ctx.follow_machine(&entry);
    ctx.persist(record_of(&entry)).await?;
    let machine_id = read_rec(&entry, |r| r.machine_id.clone());
    ctx.emit(json!({ "type": "workspace.upgraded", "workspaceId": id, "machineId": machine_id }))?;
    Ok(ctx.view(&record_of(&entry)))
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
    let entry = ctx.entry_of(id, origin.clone(), EntryAsk::default()).await?;
    ctx.refuse_cannot(&entry, "replacesMachine", "be rebuilt")?;
    let waking = entry.borrow().waking.clone();
    if let Some(waking) = waking {
        let _ = waking.await;
    }
    if ctx.recover_gone(&entry).await?.is_some() {
        return Ok(ctx.view(&record_of(&entry)));
    }
    if ctx.keeps_images(&entry) && ctx.pause_keeps_disk(&entry) && phase_of(&entry) != WorkspacePhase::Running {
        let asleep = read_rec(&entry, |r| r.machine_id.clone());
        if let Err(e) = wake(ctx, id, origin.clone()).await {
            let name = name_of(&entry);
            return Err(conflict(format!("{name} could not be woken to back up before the rebuild: {}", e.message)));
        }
        if read_rec(&entry, |r| r.machine_id.clone()) != asleep {
            return Ok(ctx.view(&record_of(&entry)));
        }
    }
    let old = read_rec(&entry, |r| r.machine_id.clone());
    let from_running = ctx.keeps_images(&entry) && ctx.pause_keeps_disk(&entry) && phase_of(&entry) == WorkspacePhase::Running;
    let nap_vault = !from_running && ctx.store().get_blob(VAULTS, id).await?.is_some();
    if from_running {
        if let Err(e) = ctx.ws_upgrade(&entry).await {
            console::warn(&format!("rebuild of {id}: the running machine gave no vault ({e}); replacing with none"));
            ctx.ws_rebuild(&entry).await.map_err(engine)?;
        }
    } else {
        ctx.ws_rebuild(&entry).await.map_err(engine)?;
    }
    ctx.follow_machine(&entry);
    rec(&entry, |r| r.wake_refused = None);
    ctx.persist(record_of(&entry)).await?;
    let new = read_rec(&entry, |r| r.machine_id.clone());
    ctx.emit(json!({ "type": "workspace.upgraded", "workspaceId": id, "machineId": new }))?;
    let reason = if from_running {
        format!("rebuilt: {old} replaced by {new}")
    } else {
        let vault = if nap_vault { "nap-time vault imported" } else { "no vault to import" };
        format!("rebuilt: {old} replaced by {new}, {vault}")
    };
    console::warn(&format!("rebuild of {id}: {reason}"));
    let reach = ctx.reach_of(&entry);
    ctx.emit_status(&entry, &reach, Some(&reason)).await?;
    Ok(ctx.view(&record_of(&entry)))
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
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    let name = ctx.name_given(typed);
    if name_of(&entry) == name {
        return Ok(ctx.view(&record_of(&entry)));
    }
    if let Some(refusal) = ctx.name_refusal(&name) {
        return Err(conflict(refusal));
    }
    rec(&entry, |r| r.name = name.clone());
    ctx.persist(record_of(&entry)).await?;
    let project = read_rec(&entry, |r| r.project.clone());
    let named = NamedWorkspace { workspace_id: id.to_owned(), name: name.clone(), project: Some(project) };
    let value = serde_json::to_value(&named).map_err(|e| HostError::new(e.to_string()))?;
    ctx.store().put(WORKSPACE_NAMES, id, value).await?;
    ctx.emit(json!({ "type": "workspace.renamed", "workspaceId": id, "name": name }))?;
    Ok(ctx.view(&record_of(&entry)))
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
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    let theme = look.theme.map(|t| t.map_or(serde_json::Value::Null, |t| json!(t)));
    let glyph = look.glyph.map(|g| g.map_or(serde_json::Value::Null, |g| json!(g)));
    ctx.put_look(&entry, "theme", theme);
    ctx.put_look(&entry, "glyph", glyph);
    ctx.persist(record_of(&entry)).await?;
    let (theme, glyph) = read_rec(&entry, |r| (r.theme.clone(), r.glyph));
    ctx.emit(json!({ "type": "workspace.look", "workspaceId": id, "theme": theme, "glyph": glyph }))?;
    Ok(ctx.view(&record_of(&entry)))
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
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    if ctx.served_by_its_computer(&entry).is_some() {
        let (name, computer) = (name_of(&entry), ctx.computer_of(&entry));
        return Err(HostError::new(place_serves_daemon_line(&name, &computer)));
    }
    if !ctx.has_deploy_daemon(&entry) {
        return Err(HostError::new("this runtime cannot deploy a daemon; the host wires the bundle"));
    }
    if phase_of(&entry) != WorkspacePhase::Running {
        return Err(HostError::new(format!("wake {} before updating its daemon", name_of(&entry))));
    }
    ctx.deploy_daemon_on(&entry).await
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
    ctx.spawn_guard("delete", origin.as_ref())?;
    let live_has = ctx.live().borrow().contains_key(id);
    if scope_of(origin.as_ref()).is_none() && !live_has && ctx.forget_failed_create(id) {
        ctx.emit(json!({ "type": "workspace.deleted", "workspaceId": id }))?;
        return Ok(());
    }
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    let deleting = entry.borrow().deleting.clone();
    if let Some(deleting) = deleting {
        return deleting.await;
    }
    if phase_of(&entry) == WorkspacePhase::Queued {
        if let Some(held) = ctx.delete_queued(id).await {
            return held;
        }
    }
    let tree = read_rec(&entry, |r| r.worktree.clone());
    let takes = tree.as_ref().is_some_and(|t| t.made && t.gone.is_none() && ctx.path_exists(&t.path));
    if let (true, Some(tree)) = (takes, &tree) {
        ctx.refuse_changed(&tree.path).await?;
    }
    let joined = delete_run(ctx.clone(), entry.clone(), id.to_owned(), takes).boxed_local().shared();
    entry.borrow_mut().deleting = Some(joined.clone());
    joined.await
}

/// The thread ids a workspace's thread records name, then the threads of its sessions (workspaces.ts:495-500).
fn thread_ids_of(ctx: &Rc<dyn PhasesCtx>, id: &str) -> (Vec<String>, Vec<String>) {
    let records: Vec<String> =
        ctx.thread_records().into_iter().filter(|(_, held)| held.workspace_id == id).map(|(thread, _)| thread).collect();
    let mut threads = records.clone();
    for session in ctx.session_views() {
        if session.workspace_id == id {
            if let Some(thread) = session.thread_id {
                if !threads.contains(&thread) {
                    threads.push(thread);
                }
            }
        }
    }
    (threads, records)
}

async fn delete_run(ctx: Rc<dyn PhasesCtx>, entry: Entry, id: String, takes: bool) -> Result<(), HostError> {
    let outcome = delete_steps(&ctx, &entry, &id, takes).await;
    entry.borrow_mut().deleting = None;
    outcome
}

async fn delete_steps(ctx: &Rc<dyn PhasesCtx>, entry: &Entry, id: &str, takes: bool) -> Result<(), HostError> {
    entry.borrow_mut().delete_said = None;
    ctx.cut_off(entry).await?;
    ctx.end_sessions(id, DELETED_REASON);
    let (threads, records) = thread_ids_of(ctx, id);
    let kind = read_rec(entry, |r| r.kind);
    if runs_in_folder(&kind) {
        for thread in &threads {
            ctx.drop_checkpoints(entry, thread).await?;
        }
    }
    if takes {
        ctx.remove_worktree_ending(entry).await?;
    }
    ctx.drop_thread_files(entry, records).await?;
    let (phase, cloud_gone) = read_rec(entry, |r| (r.phase, r.sub.as_ref().and_then(|s| s.cloud_gone) == Some(true)));
    let standing = phase != WorkspacePhase::Released && phase != WorkspacePhase::Queued && !cloud_gone;
    if standing && !ctx.driven(entry) {
        match ctx.machine_kill(entry).await {
            Ok(()) => {}
            Err(e) if is_missing(&e) => {}
            Err(e) => return Err(engine(e)),
        }
    } else if standing {
        if let Err(e) = ctx.unfork(entry).await {
            let Some((machine, state)) = machine_alive(&e) else {
                return Err(engine(e));
            };
            let name = read_rec(entry, |r| r.name.clone());
            let line = delete_refused_line(&name, &machine, &state);
            entry.borrow_mut().delete_said = Some(DeleteSaid { phase, line: line.clone() });
            console::warn(&line);
            return Err(HostError::with_kind(line, Kind::MachineAlive));
        }
    }
    ctx.drop_workspace(id).await
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
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    if !ctx.driven(&entry) {
        let (name, kind) = read_rec(&entry, |r| (r.name.clone(), r.kind));
        return Err(conflict(forget_undriven_refusal(&name, &machine_word(&kind))));
    }
    let state = ctx.read_gone(&entry).await?;
    if state != "gone" {
        let (name, machine) = read_rec(&entry, |r| (r.name.clone(), r.machine_id.clone()));
        return Err(conflict(format!("{name}'s machine {machine} is still {state}; pause it or delete it at the provider first")));
    }
    ctx.end_sessions(id, DELETED_REASON);
    ctx.drop_workspace(id).await
}

// ts: workspaces.ts:626-629
//|     async touch(id, origin) {
//|       await ctx.entryOf(id, origin);
//|       ctx.idle.touch(id);
//|     },
pub async fn touch(ctx: &Rc<dyn PhasesCtx>, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
    ctx.entry_of(id, origin, EntryAsk::default()).await?;
    ctx.idle_touch(id);
    Ok(())
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
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    if !ctx.has_restart_daemon(&entry) {
        let (name, kind) = read_rec(&entry, |r| (r.name.clone(), r.kind));
        return Err(HostError::new(format!(
            "{name}'s daemon runs on {}, which this host does not hold the process of",
            machine_word(&kind)
        )));
    }
    let started = ctx.restart_daemon_of(&entry).await;
    if ctx.shared_daemon(&entry) {
        ctx.shared_daemon_replaced();
    }
    started?;
    ctx.forget_polled_reach(id);
    ctx.push_status(&entry).await
}
