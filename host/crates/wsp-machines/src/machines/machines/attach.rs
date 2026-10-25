// SPDX-License-Identifier: AGPL-3.0-only
//! machines.ts:270-301,364-587 at 65f6b89c7: the live entry, its engine hooks, and drop. Spec: spec/machines_attach.md.

use std::rc::Rc;
use std::sync::Arc;

use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::lifecycle::WorkspacePhase as EnginePhase;
use wsp_engine::machine::Machine;
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_words::exit::HostError;

use super::{Entry, MachinesArea};

/// `enginePhaseOf` (machines.ts:367): the engine's three phases off the record's seven.
pub fn engine_phase_of(phase: WorkspacePhase) -> EnginePhase {
    todo!()
}

impl MachinesArea {
    /// `watched` (machines.ts:274): the machine with `exec` and `run` read for the provider's unreachable and stopped
    /// answers, on behalf of `entry`.
    pub fn watched(self: &Rc<Self>, entry: &Entry, machine: Arc<dyn Machine>) -> Arc<dyn Machine> {
        todo!()
    }

    /// `followMachine` (machines.ts:386).
    pub fn follow_machine(&self, entry: &Entry) {
        todo!()
    }

    /// `attach` (machines.ts:395): the new live entry, set in `live`, its engine workspace given the hooks of spec
    /// rules 4-7.
    pub fn attach(self: &Rc<Self>, record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry {
        todo!()
    }

    /// `idleWindowOf` (machines.ts:517): `None` is off.
    pub fn idle_window_of(&self, record: &WorkspaceRecord) -> Option<f64> {
        todo!()
    }

    /// `endSessions` (machines.ts:520).
    pub fn end_sessions(&self, workspace_id: &str, reason: &str) {
        todo!()
    }

    /// `drop` (machines.ts:526), spec rule 10, in that order.
    pub async fn drop(self: &Rc<Self>, id: &str) -> Result<(), HostError> {
        todo!()
    }
}
