// SPDX-License-Identifier: AGPL-3.0-only
//! sub-life.ts:156-597: the area. A done reads the machine, keeps the session, arms the delete or holds the machine,
//! and naps it; the delete at `deleteAt` releases the record. Spec: spec/sub_life_timers.md.
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::Value;

use wsp_wire::views::sub_row::SubResult;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;
use wsp_words::sub_thread::SubUnpushed;

use super::SubLifeCtx;
use crate::machines::area_error::AreaResult;
use wsp_core::clock::Cancel;
use crate::machines::machines::Entry;

/// The answer of `sub_delete` (sub-life.ts:584): `machine` is `Some("deleted")` once deleted, `question` with `check`.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubDeleted {
    pub workspace_id: String,
    pub threads: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    pub on: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
}

/// `GoneOutcome` as `subLost` answers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoneOutcome {
    Settled,
}

/// One line waiting on a done's read (sub-life.ts:163).
pub(crate) struct ReadWait {
    pub since: f64,
    pub go: Rc<dyn Fn()>,
}

/// sub-life.ts:156 `subLifeArea(ctx)`.
pub struct SubLifeArea {
    pub(crate) ctx: Rc<dyn SubLifeCtx>,
    pub(crate) me: Weak<SubLifeArea>,
    /// sub-life.ts:158.
    pub(crate) timers: RefCell<IndexMap<String, Cancel>>,
    /// sub-life.ts:160. The record's chain tail; it never fails (rule 1).
    pub(crate) working: RefCell<IndexMap<String, Shared<LocalBoxFuture<'static, ()>>>>,
    /// sub-life.ts:163.
    pub(crate) read_waits: RefCell<IndexMap<String, Vec<Rc<ReadWait>>>>,
    /// sub-life.ts:168.
    pub(crate) done_started: RefCell<IndexMap<String, f64>>,
}

impl SubLifeArea {
    /// sub-life.ts:156. Builds the area and registers its listeners on `session.end`, `session.start`,
    /// `session.held`, `session.queued` and `workspace.deleted` (rules 13-15), each holding a `Weak` to the area.
    pub fn new(ctx: Rc<dyn SubLifeCtx>) -> Rc<SubLifeArea> {
        todo!()
    }

    /// sub-life.ts:170 `serial`: the record's chain, second form (rule 1).
    pub(crate) fn serial(&self, id: &str, work: Box<dyn FnOnce() -> LocalBoxFuture<'static, AreaResult<()>>>) -> Shared<LocalBoxFuture<'static, ()>> {
        todo!()
    }

    /// sub-life.ts:181 (rule 2).
    pub(crate) fn thread_of(&self, workspace_id: &str) -> Option<String> {
        todo!()
    }

    /// sub-life.ts:194 (rule 3).
    pub(crate) fn busy(&self, thread_id: &str) -> bool {
        todo!()
    }

    /// sub-life.ts:201 (rule 5).
    pub(crate) async fn read_result(&self, entry: &Entry) -> SubResult {
        todo!()
    }

    /// sub-life.ts:222 (rule 6).
    pub(crate) async fn push_refused_in(&self, workspace_id: &str, thread_id: &str) -> AreaResult<Option<String>> {
        todo!()
    }

    /// sub-life.ts:238 (rule 7).
    pub(crate) async fn keep_session(&self, entry: &Entry, thread_id: &str) {
        todo!()
    }

    /// sub-life.ts:275 (rule 8).
    pub(crate) fn arm(&self, entry: &Entry) {
        todo!()
    }

    /// sub-life.ts:295 (rule 9). `lost` and `cloud_gone` as the TS options.
    pub(crate) async fn release(&self, entry: &Entry, lost: Option<String>, cloud_gone: bool) -> AreaResult<()> {
        todo!()
    }

    /// sub-life.ts:330 (rules 10-11).
    pub(crate) async fn done(&self, id: &str) -> AreaResult<()> {
        todo!()
    }

    /// sub-life.ts:385 (rule 12).
    pub(crate) async fn expire(&self, id: &str) -> AreaResult<()> {
        todo!()
    }

    /// sub-life.ts:432 (rule 13).
    pub(crate) fn reclaim(&self, workspace_id: &str) {
        todo!()
    }

    /// sub-life.ts:476 (rule 17).
    pub(crate) async fn forgotten(&self, entry: &Entry) -> AreaResult<()> {
        todo!()
    }

    /// sub-life.ts:540 `subDone`: a done on the record's chain.
    pub async fn sub_done(&self, workspace_id: &str) {
        todo!()
    }

    /// sub-life.ts:491 (rule 18).
    pub async fn sub_read(&self, workspace_id: &str, since: f64) -> Option<SubResult> {
        todo!()
    }

    /// sub-life.ts:520 (rule 19).
    pub async fn sub_line(&self, workspace_id: &str, since: f64) -> Option<String> {
        todo!()
    }

    /// sub-life.ts:543 `subExpire`: the delete on the record's chain.
    pub async fn sub_expire(&self, workspace_id: &str) {
        todo!()
    }

    /// sub-life.ts:544 (rule 20).
    pub fn arm_subs(&self) {
        todo!()
    }

    /// sub-life.ts:554 (rule 21).
    pub async fn sub_lost(&self, entry: &Entry, reason: &str) -> AreaResult<GoneOutcome> {
        todo!()
    }

    /// sub-life.ts:558 (rule 21).
    pub async fn release_subs_on(&self, place_id: &str, forgot: bool) -> AreaResult<()> {
        todo!()
    }

    /// sub-life.ts:462 (rule 16).
    pub fn sub_unpushed(&self, entry: &Entry) -> Option<SubUnpushed> {
        todo!()
    }

    /// sub-life.ts:566 (rule 22).
    pub async fn sub_delete(&self, asked: &Entry, thread_id: &str, origin: Option<&Caller>, check: bool) -> Result<SubDeleted, HostError> {
        todo!()
    }

    /// sub-life.ts:584 (rule 23).
    pub fn refuse_sub_send(&self, entry: &Entry) -> Result<(), HostError> {
        todo!()
    }

    /// sub-life.ts:587 (rule 23).
    pub fn refuse_sub_wake(&self, entry: &Entry) -> Result<(), HostError> {
        todo!()
    }

    /// sub-life.ts:592 (rule 23).
    pub fn lead_of_sub(&self, workspace_id: Option<&str>, origin: Option<&Caller>) -> bool {
        todo!()
    }

    /// The `session.end` listener (rule 14), exposed for the area's own registration.
    pub(crate) fn on_session_end(&self, event: &Value) {
        todo!()
    }
}
