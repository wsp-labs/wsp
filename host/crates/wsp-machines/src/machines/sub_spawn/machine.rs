// SPDX-License-Identifier: AGPL-3.0-only
//! sub-spawn.ts:32-74, 283-341, 414-621: the machine of a spawn and of a resume. Spec: spec/sub_spawn_machine.md.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use wsp_engine::machine::MachineBackend;
use wsp_words::exit::HostError;
use wsp_words::sub_thread::SubFault;

use super::{Seed, SubSpawnArea};
use crate::machines::area_error::AreaError;
use crate::machines::area_error::AreaResult;
use crate::machines::machines::Entry;
use wsp_core::types::wiring::StageReport;
use wsp_core::types::wiring::WorkspaceRecord;

/// sub-spawn.ts:33 (rule 1).
pub fn fault_of(e: &AreaError) -> Option<SubFault> {
    todo!()
}

/// What `resume_sub` answers (sub-spawn.ts:605): the live entry now, the head its send opens with, the notices.
pub struct Resumed {
    pub entry: Entry,
    pub preamble: Option<String>,
    pub notices: Vec<String>,
}

impl SubSpawnArea {
    /// sub-spawn.ts:47 (rule 2).
    pub(crate) fn told(&self, record: &WorkspaceRecord) {
        todo!()
    }

    /// sub-spawn.ts:51 (rule 2). `mv` changes the record's spawn; nothing without one.
    pub(crate) fn step(&self, record: &Rc<RefCell<WorkspaceRecord>>, mv: &mut dyn FnMut(&mut wsp_wire::views::sub_row::SubSpawn)) {
        todo!()
    }

    /// sub-spawn.ts:58 (rule 3).
    pub(crate) fn logged(&self, record: &Rc<RefCell<WorkspaceRecord>>, report: StageReport) -> StageReport {
        todo!()
    }

    /// sub-spawn.ts:69 (rule 4).
    pub(crate) fn spawn_failed(&self, record: &Rc<RefCell<WorkspaceRecord>>, e: &AreaError) {
        todo!()
    }

    /// sub-spawn.ts:285 (rule 5). Answers the replacement's lines.
    pub(crate) async fn machine_for(&self, record: &Rc<RefCell<WorkspaceRecord>>, at: &Arc<dyn MachineBackend>, on: &str, seed: &Seed, report: StageReport, landed: Rc<dyn Fn()>) -> AreaResult<Vec<String>> {
        todo!()
    }

    /// sub-spawn.ts:314 (rule 6).
    pub async fn stand_up_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>, posted: Rc<dyn Fn()>, waited: bool) -> AreaResult<()> {
        todo!()
    }

    /// sub-spawn.ts:416 (rule 7).
    pub(crate) async fn stand_up(&self, record: &Rc<RefCell<WorkspaceRecord>>, on: &str, seed: &Seed, report: StageReport, landed: Rc<dyn Fn()>) -> AreaResult<()> {
        todo!()
    }

    /// sub-spawn.ts:476 (rule 8).
    pub(crate) async fn drop_machine(&self, at: &Arc<dyn MachineBackend>, record: &Rc<RefCell<WorkspaceRecord>>, r#box: Option<&str>) {
        todo!()
    }

    /// sub-spawn.ts:501 (rule 9).
    pub async fn resume_sub(&self, asked: &Entry, thread_id: &str, harness: &str, prompt: Option<&str>) -> Result<Resumed, HostError> {
        todo!()
    }

    /// sub-spawn.ts:518 (rule 10).
    pub(crate) async fn resume_once(&self, asked: &Entry, thread_id: &str, harness: &str, prompt: Option<&str>) -> Result<Resumed, HostError> {
        todo!()
    }

    /// sub-spawn.ts:587 (rule 11).
    pub(crate) async fn land_session(&self, entry: &Entry, thread_id: &str) {
        todo!()
    }

    /// sub-spawn.ts:606 (rule 12).
    pub async fn sub_killed_why(&self, entry: &Entry, started_at: f64) -> Option<String> {
        todo!()
    }
}

#[allow(dead_code)]
fn _uses(_: HostError) {}
