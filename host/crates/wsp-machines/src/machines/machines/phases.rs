// SPDX-License-Identifier: AGPL-3.0-only
//! machines.ts:589-899 at 65f6b89c7: nap, adopt, gone, the idle wiring and the wake proof. Spec:
//! spec/machines_phases.md.

use std::rc::Rc;
use std::sync::Arc;

use futures::future::{Either, FutureExt};
use wsp_core::status::{phase_leaving_gone, provider_said};
use wsp_core::types::internal::{gone_log_line, GONE_REASON, PAUSED_REASON};
use wsp_core::types::wiring::{settled, GoneOutcome, Joined};
use wsp_engine::errors::{is_missing, EngineError};
use wsp_engine::machine::{MachineBackend, MachineState};
use wsp_engine::Cancel;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::workspace::{ReachState, WorkspacePhase};
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::not_found_refusal;
use wsp_words::exit::HostError;
use wsp_words::words::computer::{no_commands_yet_line, no_workspace_refusal, WAKE_STOPPED_UP};
use wsp_words::words::thread::{
    gone_words, idle_reason, GoneSeenBy, GoneSighting, ALREADY_RUNNING, GONE_UNCHECKED, NOT_GONE,
};

use super::create::{backstop_of, host_of, sleep_ms};
use super::{Entry, MachinesArea};
use crate::machines::reach::ForMachine;
use crate::unwound::unwound_or_failed;
use wsp_engine::gone::{machine_state_word, read_gone};

/// What `goneConfirmed` answers: the provider's state, or the error its reads ended on (machines.ts:691).
#[derive(Clone, Debug, PartialEq)]
pub enum GoneRead {
    State(MachineState),
    Failed(String),
}

/// The marks a nap's error carries to `on_idle`, which tells a move nobody answered and a nap refusal apart (machines.ts:441-451).
pub(crate) const MARK_MOVE_UNANSWERED: &str = "moveUnanswered";
pub(crate) const MARK_NAP_REFUSED: &str = "napRefused";

/// A wake's proof outcome: proven, stopped by the person, or the error the proof ended on.
enum Wake {
    Proven,
    Stopped,
    Failed(EngineError),
}

/// A nap's error as the host error its callers take, with the marks `on_idle` reads.
fn nap_error(e: EngineError) -> HostError {
    let mark = match &e {
        EngineError::MoveUnanswered { .. } => Some(MARK_MOVE_UNANSWERED),
        EngineError::NapRefused { .. } => Some(MARK_NAP_REFUSED),
        _ => None,
    };
    let mut host = host_of(e);
    host.marks.extend(mark.map(str::to_string));
    host
}

/// The provider's word for a host error that came from a nap, as `providerSaid` reads it (status.ts:49).
fn said_of(e: &HostError) -> String {
    match e.status {
        Some(status) => format!("{} {}", status as i64, e.message),
        None => e.message.clone(),
    }
}

impl MachinesArea {
    /// A snapshot of the entry's record, taken to write or to answer with.
    fn record_snapshot(entry: &Entry) -> wsp_core::types::wiring::WorkspaceRecord {
        entry.borrow().record.borrow().clone()
    }

    /// Writes the entry's record phase, and nothing else.
    fn set_phase(entry: &Entry, phase: WorkspacePhase) {
        entry.borrow().record.borrow_mut().phase = phase;
    }

    /// Before every write: a delete under way is waited for, and one that took the record leaves nothing to write
    /// (machines.ts:857).
    async fn still_live(&self, entry: &Entry) -> Result<(), HostError> {
        let deleting = entry.borrow().deleting.clone();
        if let Some(deleting) = deleting {
            let _ = deleting.await;
        }
        let (id, name) = {
            let e = entry.borrow();
            let r = e.record.borrow();
            (r.id.clone(), r.name.clone())
        };
        let current = self.rt.live().borrow().get(&id).cloned();
        if current.is_none_or(|c| !Rc::ptr_eq(&c, entry)) {
            return Err(not_found_refusal(&no_workspace_refusal(Some(&name))));
        }
        Ok(())
    }

    /// `napWith` (machines.ts:592), spec rule 1; joined on the entry's `napping`.
    pub async fn nap_with(
        self: &Rc<Self>,
        id: &str,
        reason: Option<&str>,
    ) -> Result<WorkspaceView, HostError> {
        let entry = self.rt.entry_of(id).await?;
        if self.runs_under_napping(&entry).await {
            self.adopt_running(&entry).await?;
        }
        let joined = entry.borrow().napping.clone();
        if let Some(joined) = joined {
            return joined.await;
        }
        let snapshot = Self::record_snapshot(&entry);
        if snapshot.phase != WorkspacePhase::Running {
            return Ok(self.rt.view(&snapshot));
        }
        let (area, held, id_owned, reason_owned) = (
            self.clone(),
            entry.clone(),
            id.to_string(),
            reason.map(str::to_string),
        );
        let nap: Joined<WorkspaceView> = async move {
            let out =
                unwound_or_failed(area.nap_run(&held, &id_owned, reason_owned.as_deref())).await;
            held.borrow_mut().napping = None;
            out
        }
        .boxed_local()
        .shared();
        entry.borrow_mut().napping = Some(nap.clone());
        nap.await
    }

    /// The body of a nap: the pause, its refusals, and the record after it (machines.ts:594-636).
    async fn nap_run(
        self: &Rc<Self>,
        entry: &Entry,
        id: &str,
        reason: Option<&str>,
    ) -> Result<WorkspaceView, HostError> {
        Self::set_phase(entry, WorkspacePhase::Pausing);
        self.rt.persist(&Self::record_snapshot(entry)).await?;
        self.rt
            .emit_status(entry, ReachState::Napping, None)
            .await?;
        let ws = entry.borrow().ws.clone();
        if let Err(e) = ws.nap().await {
            if is_missing(&e) {
                let snapshot = Self::record_snapshot(entry);
                let sighting = GoneSighting {
                    by: GoneSeenBy::Pause,
                    at: self.rt.clock().now() as f64,
                    answer: Some(provider_said(&e)),
                };
                let words = gone_words(&snapshot.machine_id, Some(&sighting));
                if settled(self.settle_gone(entry, &words).await?) {
                    return Err(nap_error(e));
                }
            }
            let machine_id = entry.borrow().machine.id().to_string();
            Self::set_phase(entry, wire_phase(ws.current_phase()));
            self.rt.persist(&Self::record_snapshot(entry)).await?;
            if let EngineError::NapRefused { said, .. } = &e {
                if self.rt.nap_refused_of(entry).is_none() {
                    self.rt.warn(&format!(
                        "the provider does not pause {machine_id} of {id} ({said}); an idle nap waits for the backstop and exports no vault until a pause lands"
                    ));
                }
                self.rt.nap_refusals().borrow_mut().insert(
                    id.to_string(),
                    ForMachine {
                        machine_id: machine_id.clone(),
                        value: said.clone(),
                    },
                );
            }
            if let EngineError::StopRefused { said, .. } = &e {
                self.rt.stop_refusals().borrow_mut().insert(
                    id.to_string(),
                    ForMachine {
                        machine_id: machine_id.clone(),
                        value: said.clone(),
                    },
                );
            }
            let reach = self.rt.reach_of(entry);
            self.rt
                .emit_status(entry, reach, Some(&e.to_string()))
                .await?;
            return Err(nap_error(e));
        }
        self.end_sessions(id, PAUSED_REASON);
        self.rt.nap_refusals().borrow_mut().shift_remove(id);
        self.rt.stop_refusals().borrow_mut().shift_remove(id);
        {
            let e = entry.borrow();
            let mut r = e.record.borrow_mut();
            r.phase = WorkspacePhase::Napping;
            r.napped_at = Some(JsNumber(self.rt.clock().now() as f64));
            r.disk_gone = None;
        }
        entry.borrow_mut().unchecked = false;
        self.rt.persist(&Self::record_snapshot(entry)).await?;
        self.rt.emit(EventUnion::WorkspaceNapped(
            wsp_wire::wire::events_union::Sequenced {
                event: wsp_wire::views::workspace_events::WorkspaceNappedEvent {
                    workspace_id: id.to_string(),
                    found: None,
                },
                seq: None,
            },
        ))?;
        self.rt
            .emit_status(entry, ReachState::Napping, reason)
            .await?;
        Ok(self.rt.view(&Self::record_snapshot(entry)))
    }

    /// `armLateRead` (machines.ts:640).
    pub fn arm_late_read(self: &Rc<Self>, entry: &Entry) {
        let armed = entry.borrow_mut().late_read.take();
        if let Some(cancel) = armed {
            cancel();
        }
        let (area, weak) = (self.clone(), Rc::downgrade(entry));
        let cancel = self.rt.clock().schedule(
            Box::new(move || {
                let Some(entry) = weak.upgrade() else { return };
                entry.borrow_mut().late_read = None;
                wsp_engine::local_task::try_spawn_local(async move {
                    let id = entry.borrow().record.borrow().id.clone();
                    let runs = area.runs_under_napping(&entry).await;
                    let out = if runs {
                        area.adopt_running(&entry).await
                    } else {
                        Ok(())
                    };
                    if let Err(e) = out {
                        area.rt.warn(&format!("late read of {id}: {}", e.message));
                    }
                });
            }),
            self.config.late_read_ms as i64,
        );
        entry.borrow_mut().late_read = Some(Box::new(move || cancel()));
    }

    /// `adoptPause` (machines.ts:651).
    pub async fn adopt_pause(self: &Rc<Self>, entry: &Entry) -> Result<(), HostError> {
        let (phase, napping, waking) = gate_of(entry);
        if phase != WorkspacePhase::Running || napping || waking {
            return Ok(());
        }
        let ws = entry.borrow().ws.clone();
        ws.note_paused();
        let id = entry.borrow().record.borrow().id.clone();
        self.rt.nap_refusals().borrow_mut().shift_remove(&id);
        {
            let e = entry.borrow();
            let mut r = e.record.borrow_mut();
            r.phase = WorkspacePhase::Napping;
            r.napped_at = Some(JsNumber(self.rt.clock().now() as f64));
            r.disk_gone = None;
        }
        entry.borrow_mut().unchecked = false;
        self.rt.persist(&Self::record_snapshot(entry)).await?;
        self.end_sessions(&id, PAUSED_REASON);
        self.rt.emit(EventUnion::WorkspaceNapped(
            wsp_wire::wire::events_union::Sequenced {
                event: wsp_wire::views::workspace_events::WorkspaceNappedEvent {
                    workspace_id: id.clone(),
                    found: Some(true),
                },
                seq: None,
            },
        ))?;
        self.rt
            .emit_status(entry, ReachState::Napping, Some("paused outside wsp"))
            .await
    }

    /// `runsUnderNapping` (machines.ts:667).
    pub async fn runs_under_napping(&self, entry: &Entry) -> bool {
        if entry.borrow().record.borrow().phase != WorkspacePhase::Napping {
            return false;
        }
        let machine = entry.borrow().machine.clone();
        machine.state().await.unwrap_or(MachineState::Paused) == MachineState::Running
    }

    /// `adoptRunning` (machines.ts:671), joined on the entry's `adopting`.
    pub async fn adopt_running(self: &Rc<Self>, entry: &Entry) -> Result<(), HostError> {
        let joined = entry.borrow().adopting.clone();
        if let Some(joined) = joined {
            return joined.await;
        }
        let (phase, napping, waking) = gate_of(entry);
        if phase != WorkspacePhase::Napping || napping || waking {
            return Ok(());
        }
        let (area, held) = (self.clone(), entry.clone());
        let adopting: Joined<()> = async move {
            let out = unwound_or_failed(area.adopt_running_run(&held)).await;
            held.borrow_mut().adopting = None;
            out
        }
        .boxed_local()
        .shared();
        entry.borrow_mut().adopting = Some(adopting.clone());
        adopting.await
    }

    /// The body of an adoption: the record follows the machine that runs (machines.ts:672-684).
    async fn adopt_running_run(self: &Rc<Self>, entry: &Entry) -> Result<(), HostError> {
        let ws = entry.borrow().ws.clone();
        ws.note_running();
        self.follow_machine(entry);
        entry.borrow_mut().unchecked = true;
        self.rt.persist(&Self::record_snapshot(entry)).await?;
        let (id, machine_id) = {
            let e = entry.borrow();
            let r = e.record.borrow();
            (r.id.clone(), r.machine_id.clone())
        };
        self.rt.emit(EventUnion::WorkspaceWoken(
            wsp_wire::wire::events_union::Sequenced {
                event: wsp_wire::views::workspace_events::WorkspaceWokenEvent {
                    workspace_id: id,
                    machine_id,
                },
                seq: None,
            },
        ))?;
        let reach = self.rt.reach_of(entry);
        self.rt
            .emit_status(entry, reach, Some(ALREADY_RUNNING))
            .await
    }

    /// `goneConfirmed` (machines.ts:691).
    pub async fn gone_confirmed(
        &self,
        backend: &Arc<dyn MachineBackend>,
        machine_id: &str,
    ) -> GoneRead {
        if self.config.gone_confirm_ms > 0 {
            sleep_ms(&self.rt.clock(), self.config.gone_confirm_ms as i64).await;
        }
        match read_gone(backend.as_ref(), machine_id).await {
            Ok(state) => GoneRead::State(state),
            Err(e) => GoneRead::Failed(e.to_string()),
        }
    }

    /// `settleGone` (machines.ts:700), spec rule 7.
    pub async fn settle_gone(
        self: &Rc<Self>,
        entry: &Entry,
        reason: &str,
    ) -> Result<GoneOutcome, HostError> {
        let snapshot = Self::record_snapshot(entry);
        let backend = self.rt.backend_for(&snapshot);
        let read = self.gone_confirmed(&backend, &snapshot.machine_id).await;
        match read {
            GoneRead::State(MachineState::Gone) => {
                self.mark_gone(entry, &snapshot.machine_id, reason).await
            }
            GoneRead::State(state) => {
                self.rt.warn(&format!(
                    "workspace {} is not gone: {reason}, and the state read that followed said {}",
                    snapshot.id,
                    machine_state_word(state)
                ));
                Ok(GoneOutcome::NotGone)
            }
            GoneRead::Failed(message) => {
                self.rt.warn(&format!("workspace {} is not gone: {reason}, and the reads that followed failed: {message}", snapshot.id));
                Ok(GoneOutcome::Unchecked)
            }
        }
    }

    /// `markGone` (machines.ts:712), spec rule 8.
    pub async fn mark_gone(
        self: &Rc<Self>,
        entry: &Entry,
        machine_id: &str,
        reason: &str,
    ) -> Result<GoneOutcome, HostError> {
        let snapshot = Self::record_snapshot(entry);
        if snapshot.phase == WorkspacePhase::Gone || snapshot.machine_id != machine_id {
            return Ok(GoneOutcome::Moot);
        }
        let current = self.rt.live().borrow().get(&snapshot.id).cloned();
        if current.is_none_or(|c| !Rc::ptr_eq(&c, entry)) || entry.borrow().deleting.is_some() {
            return Ok(GoneOutcome::Moot);
        }
        if snapshot.sub.is_some() {
            return self.rt.sub_lost(entry, reason).await;
        }
        Self::set_phase(entry, WorkspacePhase::Gone);
        entry.borrow_mut().unchecked = false;
        entry.borrow().record.borrow_mut().gone = Some(reason.to_string());
        self.rt.persist(&Self::record_snapshot(entry)).await?;
        self.end_sessions(&snapshot.id, GONE_REASON);
        self.rt.warn(&gone_log_line(&snapshot.id, reason));
        self.rt.emit(EventUnion::WorkspaceGone(
            wsp_wire::wire::events_union::Sequenced {
                event: wsp_wire::views::workspace_events::WorkspaceGoneEvent {
                    workspace_id: snapshot.id.clone(),
                    machine_id: snapshot.machine_id.clone(),
                    reason: reason.to_string(),
                },
                seq: None,
            },
        ))?;
        self.rt
            .emit_status(entry, ReachState::Gone, Some(reason))
            .await?;
        Ok(GoneOutcome::Settled)
    }

    /// `adoptGone` (machines.ts:732): a second caller while one reads returns at once.
    pub async fn adopt_gone(self: &Rc<Self>, entry: &Entry, reason: &str) -> Result<(), HostError> {
        let (id, phase, napping, waking) = {
            let e = entry.borrow();
            let r = e.record.borrow();
            (
                r.id.clone(),
                r.phase,
                e.napping.is_some(),
                e.waking.is_some(),
            )
        };
        if phase == WorkspacePhase::Gone
            || napping
            || waking
            || self.rereading.borrow().contains(&id)
        {
            return Ok(());
        }
        self.rereading.borrow_mut().insert(id.clone());
        let out = match self.settle_gone(entry, reason).await {
            Ok(GoneOutcome::Unchecked) => {
                let reach = self.rt.reach_of(entry);
                self.rt
                    .emit_status(entry, reach, Some(GONE_UNCHECKED))
                    .await
            }
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        };
        self.rereading.borrow_mut().shift_remove(&id);
        out
    }

    /// `recoverGone` (machines.ts:745): the phase the record left gone for, or `None`.
    pub async fn recover_gone(
        self: &Rc<Self>,
        entry: &Entry,
    ) -> Result<Option<WorkspacePhase>, HostError> {
        let (phase, napping, waking) = gate_of(entry);
        if phase != WorkspacePhase::Gone || napping || waking {
            return Ok(None);
        }
        let machine = entry.borrow().machine.clone();
        let read = machine.state().await.ok();
        let next = read.and_then(phase_leaving_gone);
        let Some(next) = next else { return Ok(None) };
        let ws = entry.borrow().ws.clone();
        if next == WorkspacePhase::Running {
            ws.note_running();
            self.follow_machine(entry);
        } else {
            ws.note_paused();
            {
                let e = entry.borrow();
                let mut r = e.record.borrow_mut();
                r.phase = next;
                r.gone = None;
                r.disk_gone = None;
            }
        }
        self.rt.persist(&Self::record_snapshot(entry)).await?;
        let (id, machine_id) = {
            let e = entry.borrow();
            let r = e.record.borrow();
            (r.id.clone(), r.machine_id.clone())
        };
        if next == WorkspacePhase::Running {
            self.rt.emit(EventUnion::WorkspaceWoken(
                wsp_wire::wire::events_union::Sequenced {
                    event: wsp_wire::views::workspace_events::WorkspaceWokenEvent {
                        workspace_id: id,
                        machine_id,
                    },
                    seq: None,
                },
            ))?;
        } else {
            self.rt.emit(EventUnion::WorkspaceNapped(
                wsp_wire::wire::events_union::Sequenced {
                    event: wsp_wire::views::workspace_events::WorkspaceNappedEvent {
                        workspace_id: id,
                        found: Some(true),
                    },
                    seq: None,
                },
            ))?;
        }
        let reach = if next == WorkspacePhase::Running {
            self.rt.reach_of(entry)
        } else {
            ReachState::Napping
        };
        self.rt.emit_status(entry, reach, Some(NOT_GONE)).await?;
        Ok(Some(next))
    }

    /// `quietOf` (machines.ts:777).
    pub async fn quiet_of(&self, id: &str) -> Option<u64> {
        let entry = self.rt.live().borrow().get(id).cloned()?;
        if entry.borrow().record.borrow().phase != WorkspacePhase::Running {
            return None;
        }
        let backend = self.rt.backend_for(&Self::record_snapshot(&entry));
        let machine = entry.borrow().machine.clone();
        let lifecycle = backend.lifecycle()?;
        if !lifecycle.has_quiet_for_ms() {
            return None;
        }
        match lifecycle.quiet_for_ms(machine.as_ref()).await {
            Ok(quiet) => quiet,
            Err(e) => {
                self.rt.warn(&format!("quiet figure of {id} not read: {e}"));
                None
            }
        }
    }

    /// The idle policy's `windowOf` (machines.ts:790): `None` no entry, `Some(None)` off, `Some(Some(ms))` a window.
    pub fn idle_window_of_id(&self, id: &str) -> Option<Option<f64>> {
        let entry = self.rt.live().borrow().get(id).cloned()?;
        let window = self.idle_window_of(&Self::record_snapshot(&entry));
        match window {
            Some(ms) if self.rt.nap_refused_of(&entry).is_some() => {
                Some(Some(backstop_of(Some(ms))))
            }
            other => Some(other),
        }
    }

    /// The idle policy's `onIdle` (machines.ts:797): `Err` only for an unanswered move, which the policy retries.
    pub async fn on_idle(self: &Rc<Self>, id: &str, window_ms: f64) -> Result<(), HostError> {
        if let Some(quiet) = self.quiet_of(id).await {
            if (quiet as f64) < window_ms {
                self.rt.idle().touch(id);
                return Ok(());
            }
        }
        let reason = idle_reason::of(window_ms);
        let Err(e) = self.nap_with(id, Some(&reason)).await else {
            return Ok(());
        };
        if e.marks.iter().any(|m| m == MARK_MOVE_UNANSWERED) {
            return Err(e);
        }
        if matches!(e.kind, Some(wsp_words::exit::Kind::Missing)) {
            return Ok(());
        }
        self.rt.idle().touch(id);
        let entry = self.rt.live().borrow().get(id).cloned();
        if let Some(entry) = entry {
            let line = self
                .rt
                .nap_refused_reason(&entry)
                .or_else(|| self.rt.stop_refused_reason(&entry))
                .unwrap_or_else(|| e.message.clone());
            let reach = self.rt.reach_of(&entry);
            self.rt.emit_status(&entry, reach, Some(&line)).await?;
        }
        if !e.marks.iter().any(|m| m == MARK_NAP_REFUSED) {
            let minutes = (window_ms / 60_000.0).round() as i64;
            self.rt.warn(&format!(
                "idle nap of {id} was answered with {}; a full {minutes} min window starts over",
                said_of(&e)
            ));
        }
        Ok(())
    }

    /// The idle policy's `busy` (machines.ts:823).
    pub fn idle_busy(&self, id: &str) -> bool {
        self.rt.sessions().borrow().values().any(|s| {
            let s = s.borrow();
            s.view.workspace_id == id
                && s.view.status == wsp_wire::views::session::SessionStatus::Running
        })
    }

    /// The idle policy's `onBackstop` (machines.ts:829).
    pub fn on_backstop(self: &Rc<Self>, id: &str, until_ms: i64) {
        let entry = self.rt.live().borrow().get(id).cloned();
        let Some(entry) = entry else { return };
        let (phase, sub_delete) = {
            let e = entry.borrow();
            let r = e.record.borrow();
            (r.phase, r.sub.as_ref().and_then(|s| s.delete_at))
        };
        if phase != WorkspacePhase::Running || sub_delete.is_some() {
            return;
        }
        let backend = self.rt.backend_for(&Self::record_snapshot(&entry));
        let machine = entry.borrow().machine.clone();
        let Some(lifecycle) = backend.lifecycle() else {
            return;
        };
        if !lifecycle.has_backstop() {
            return;
        }
        let area = self.clone();
        let id = id.to_string();
        wsp_engine::local_task::try_spawn_local(async move {
            if let Err(e) = lifecycle.backstop(machine.as_ref(), until_ms, false).await {
                area.rt.warn(&format!(
                    "backstop of {id} on {} not set: {e}",
                    machine.id()
                ));
            }
        });
    }

    /// `proven` (machines.ts:853), spec rule 14; joined on the entry's `waking`.
    pub async fn proven(self: &Rc<Self>, entry: &Entry) -> Result<WorkspaceView, HostError> {
        let joined = entry.borrow().waking.clone();
        if let Some(joined) = joined {
            return joined.await;
        }
        let machine = entry.borrow().machine.clone();
        if !machine.offers().prove_road {
            entry.borrow_mut().unchecked = false;
            return Ok(self.rt.view(&Self::record_snapshot(entry)));
        }
        let stop = Cancel::new();
        entry.borrow_mut().wake_stop = Some(stop.clone());
        let (area, held) = (self.clone(), entry.clone());
        let waking: Joined<WorkspaceView> = async move {
            let out = unwound_or_failed(area.proven_run(&held, machine.as_ref(), &stop)).await;
            held.borrow_mut().waking = None;
            held.borrow_mut().wake_stop = None;
            out
        }
        .boxed_local()
        .shared();
        entry.borrow_mut().waking = Some(waking.clone());
        waking.await
    }

    /// The body of a wake's proof (machines.ts:862-889).
    async fn proven_run(
        self: &Rc<Self>,
        entry: &Entry,
        machine: &dyn wsp_engine::machine::Machine,
        stop: &Cancel,
    ) -> Result<WorkspaceView, HostError> {
        self.still_live(entry).await?;
        Self::set_phase(entry, WorkspacePhase::Waking);
        self.rt.persist(&Self::record_snapshot(entry)).await?;
        self.rt
            .emit_status(entry, ReachState::Napping, None)
            .await?;
        let outcome = if stop.is_cancelled() {
            Wake::Stopped
        } else {
            let proving = machine.prove_road(Some(stop));
            let stopped = stop.cancelled();
            futures::pin_mut!(proving);
            futures::pin_mut!(stopped);
            match futures::future::select(proving, stopped).await {
                Either::Left((Ok(()), _)) => Wake::Proven,
                Either::Left((Err(e), _)) => Wake::Failed(e),
                Either::Right(((), _)) => Wake::Stopped,
            }
        };
        self.still_live(entry).await?;
        if let Wake::Failed(e) = &outcome {
            if is_missing(e) {
                Self::set_phase(entry, WorkspacePhase::Running);
                let snapshot = Self::record_snapshot(entry);
                let sighting = GoneSighting {
                    by: GoneSeenBy::Wake,
                    at: self.rt.clock().now() as f64,
                    answer: Some(provider_said(e)),
                };
                let words = gone_words(&snapshot.machine_id, Some(&sighting));
                let gone = self.settle_gone(entry, &words).await?;
                self.still_live(entry).await?;
                if settled(gone) {
                    let after = Self::record_snapshot(entry);
                    return Err(HostError::new(wsp_words::workspace_state::gone_refusal(
                        Some(&after.name),
                        "wake",
                        after.gone.as_deref(),
                    )));
                }
            }
        }
        if matches!(outcome, Wake::Proven) {
            entry.borrow_mut().unchecked = false;
        }
        let name = Self::record_snapshot(entry).name;
        let said = match &outcome {
            Wake::Proven => None,
            Wake::Stopped => Some(WAKE_STOPPED_UP.to_string()),
            Wake::Failed(EngineError::RestoreUnfinished { message, .. }) => {
                Some(no_commands_yet_line(&name, message))
            }
            Wake::Failed(e) => Some(e.to_string()),
        };
        Self::set_phase(entry, WorkspacePhase::Running);
        self.rt.persist(&Self::record_snapshot(entry)).await?;
        let reach = self.rt.reach_of(entry);
        self.rt.emit_status(entry, reach, said.as_deref()).await?;
        match outcome {
            Wake::Proven => Ok(self.rt.view(&Self::record_snapshot(entry))),
            Wake::Stopped => Err(HostError::new(said.unwrap_or_default())),
            Wake::Failed(EngineError::RestoreUnfinished { .. }) => {
                Err(HostError::new(said.unwrap_or_default()))
            }
            Wake::Failed(e) => Err(host_of(e)),
        }
    }
}

/// The record's phase and whether a nap or a wake is in flight: the gate most of this unit's roads read first.
fn gate_of(entry: &Entry) -> (WorkspacePhase, bool, bool) {
    let e = entry.borrow();
    let phase = e.record.borrow().phase;
    (phase, e.napping.is_some(), e.waking.is_some())
}

/// The wire phase of an engine phase: the three the engine keeps read as the record's own words.
fn wire_phase(phase: wsp_engine::lifecycle::WorkspacePhase) -> WorkspacePhase {
    match phase {
        wsp_engine::lifecycle::WorkspacePhase::Running => WorkspacePhase::Running,
        wsp_engine::lifecycle::WorkspacePhase::Napping => WorkspacePhase::Napping,
        wsp_engine::lifecycle::WorkspacePhase::Waking => WorkspacePhase::Waking,
    }
}
