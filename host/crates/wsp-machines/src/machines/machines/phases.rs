// SPDX-License-Identifier: AGPL-3.0-only
//! machines.ts:589-899 at 65f6b89c7: nap, adopt, gone, the idle wiring and the wake proof. Spec:
//! spec/machines_phases.md.

use std::rc::Rc;
use std::sync::Arc;

use wsp_core::types::wiring::GoneOutcome;
use wsp_engine::machine::{MachineBackend, MachineState};
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_words::exit::HostError;

use super::{Entry, MachinesArea};

/// What `goneConfirmed` answers: the provider's state, or the error its reads ended on (machines.ts:691).
#[derive(Clone, Debug, PartialEq)]
pub enum GoneRead {
    State(MachineState),
    Failed(String),
}

impl MachinesArea {
    /// `napWith` (machines.ts:592), spec rule 1; joined on the entry's `napping`.
    pub async fn nap_with(self: &Rc<Self>, id: &str, reason: Option<&str>) -> Result<WorkspaceView, HostError> {
        todo!()
    }

    /// `armLateRead` (machines.ts:640).
    pub fn arm_late_read(self: &Rc<Self>, entry: &Entry) {
        todo!()
    }

    /// `adoptPause` (machines.ts:651).
    pub async fn adopt_pause(self: &Rc<Self>, entry: &Entry) -> Result<(), HostError> {
        todo!()
    }

    /// `runsUnderNapping` (machines.ts:667).
    pub async fn runs_under_napping(&self, entry: &Entry) -> bool {
        todo!()
    }

    /// `adoptRunning` (machines.ts:671), joined on the entry's `adopting`.
    pub async fn adopt_running(self: &Rc<Self>, entry: &Entry) -> Result<(), HostError> {
        todo!()
    }

    /// `goneConfirmed` (machines.ts:691).
    pub async fn gone_confirmed(&self, backend: &Arc<dyn MachineBackend>, machine_id: &str) -> GoneRead {
        todo!()
    }

    /// `settleGone` (machines.ts:700).
    pub async fn settle_gone(self: &Rc<Self>, entry: &Entry, reason: &str) -> Result<GoneOutcome, HostError> {
        todo!()
    }

    /// `markGone` (machines.ts:712).
    pub async fn mark_gone(self: &Rc<Self>, entry: &Entry, machine_id: &str, reason: &str) -> Result<GoneOutcome, HostError> {
        todo!()
    }

    /// `adoptGone` (machines.ts:732): a second caller while one reads returns at once.
    pub async fn adopt_gone(self: &Rc<Self>, entry: &Entry, reason: &str) -> Result<(), HostError> {
        todo!()
    }

    /// `recoverGone` (machines.ts:745): the phase the record left gone for, or `None`.
    pub async fn recover_gone(self: &Rc<Self>, entry: &Entry) -> Result<Option<WorkspacePhase>, HostError> {
        todo!()
    }

    /// `quietOf` (machines.ts:777).
    pub async fn quiet_of(&self, id: &str) -> Option<u64> {
        todo!()
    }

    /// The idle policy's `windowOf` (machines.ts:790): `None` no entry, `Some(None)` off, `Some(Some(ms))` a window.
    pub fn idle_window_of_id(&self, id: &str) -> Option<Option<f64>> {
        todo!()
    }

    /// The idle policy's `onIdle` (machines.ts:797): `Err` only for an unanswered move, which the policy retries.
    pub async fn on_idle(self: &Rc<Self>, id: &str, window_ms: f64) -> Result<(), HostError> {
        todo!()
    }

    /// The idle policy's `busy` (machines.ts:823).
    pub fn idle_busy(&self, id: &str) -> bool {
        todo!()
    }

    /// The idle policy's `onBackstop` (machines.ts:829).
    pub fn on_backstop(self: &Rc<Self>, id: &str, until_ms: i64) {
        todo!()
    }

    /// `proven` (machines.ts:853), spec rule 14; joined on the entry's `waking`.
    pub async fn proven(self: &Rc<Self>, entry: &Entry) -> Result<WorkspaceView, HostError> {
        todo!()
    }
}
