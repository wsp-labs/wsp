// SPDX-License-Identifier: AGPL-3.0-only
//! boot.ts:673-1013 at 65f6b89c7: listings, thread facts, the row push and the marks. Spec: spec/boot_rows.md.

use std::rc::Rc;

use wsp_core::types::internal::ThreadStamps;
use wsp_wire::views::preferences::ThreadFacts;
use wsp_wire::views::session::{ForkedFrom, SessionView};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::BootArea;
use crate::machines::machines::{Entry, Row};

/// What a mark puts on each thread: the same stamps, or stamps per thread (boot.ts:923). A key whose value is JSON
/// `null` stands for the TS `undefined`: it is taken off the record.
pub enum Stamped<'a> {
    Same(ThreadStamps),
    Each(&'a dyn Fn(&str) -> ThreadStamps),
}

impl BootArea {
    /// `forkedFromNow` (boot.ts:675).
    pub fn forked_from_now(&self, from: &ForkedFrom) -> ForkedFrom {
        todo!()
    }

    /// `listedRows` (boot.ts:681), spec rule 1: each row as a listing answers it, in the order given.
    pub fn listed_rows(&self, held: &[Row]) -> Vec<SessionView> {
        todo!()
    }

    /// `threadFacts` (boot.ts:739), spec rule 2.
    pub fn thread_facts(&self, thread_id: &str) -> Option<ThreadFacts> {
        todo!()
    }

    /// `factsAfterRead` (boot.ts:760), spec rule 3.
    pub async fn facts_after_read(self: &Rc<Self>, thread_id: &str, origin: Option<&Caller>) -> Result<Option<ThreadFacts>, HostError> {
        todo!()
    }

    /// `settledNow` (boot.ts:771), spec rule 4. `settle_ms` `None` never settles by quiet.
    pub fn settled_now(&self, thread_id: &str, settle_ms: Option<f64>) -> bool {
        todo!()
    }

    /// `pushHead` (boot.ts:778), spec rule 5.
    pub fn push_head(&self, thread_id: &str) -> Result<(), HostError> {
        todo!()
    }

    /// `placedRows` (boot.ts:783), spec rule 6.
    pub async fn placed_rows(&self, listed: Vec<SessionView>) -> Vec<SessionView> {
        todo!()
    }

    /// `rowMoved` (boot.ts:857), spec rule 7.
    pub fn row_moved(self: &Rc<Self>, workspace_id: &str, thread_id: &str) {
        todo!()
    }

    /// `rowGone` (boot.ts:862), spec rule 7.
    pub fn row_gone(self: &Rc<Self>, id: &str, workspace_id: &str, thread_id: Option<&str>) {
        todo!()
    }

    /// `rowsSent` (boot.ts:853): a queued push now, then the chain awaited.
    pub async fn rows_sent(self: &Rc<Self>) {
        todo!()
    }

    /// `setupRefusalMoved` (boot.ts:867).
    pub fn setup_refusal_moved(self: &Rc<Self>, place: &str, harness: &str) {
        todo!()
    }

    /// `mark` (boot.ts:923), spec rule 9.
    pub async fn mark(self: &Rc<Self>, thread_ids: &[String], stamped: Stamped<'_>, origin: Option<&Caller>) -> Result<(), HostError> {
        todo!()
    }

    /// `endSnoozeFor` (boot.ts:959), spec rule 10.
    pub fn end_snooze_for(self: &Rc<Self>, thread_id: Option<&str>, root_thread_id: Option<&str>) {
        todo!()
    }

    /// `armResume` (boot.ts:966), spec rule 11.
    pub async fn arm_resume(self: &Rc<Self>, thread_ids: &[String], on: bool, origin: Option<&Caller>) -> Result<(), HostError> {
        todo!()
    }

    /// `resumeAfterLimit` (boot.ts:984), spec rule 12.
    pub async fn resume_after_limit(self: &Rc<Self>, thread_id: &str) -> Result<(), HostError> {
        todo!()
    }

    /// `heldPast` (boot.ts:1008), spec rule 13.
    pub async fn held_past(&self, entry: &Entry, harness: &str) -> Option<f64> {
        todo!()
    }
}
