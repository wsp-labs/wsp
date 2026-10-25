// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:209-243, 1112-1173: settle, mark, restore, and the check a restart runs. Spec: spec/sessions_settle.md.
use std::rc::Rc;

use wsp_core::types::api::SettleAsk;
use wsp_wire::views::session::ThreadMarks;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::{SessionRestoreResult, SessionSettleResult};
use wsp_words::exit::HostError;

use super::SessionsArea;

/// What a settle and a restore read of the threads named (sessions.ts:233).
pub struct SettleReads {
    pub busy: Rc<dyn Fn(&str) -> bool>,
    pub settled: Rc<dyn Fn(&str) -> bool>,
    pub finished: Rc<dyn Fn(&str) -> bool>,
    /// `{ threadId, title }` of each id, in order.
    pub titled: Rc<dyn Fn(&[String]) -> Vec<(String, String)>>,
}

impl SessionsArea {
    /// sessions.ts:213. Rules E1, E2.
    pub async fn settle_reads(&self, thread_ids: &[String], origin: Option<&Caller>) -> Result<SettleReads, HostError> {
        todo!()
    }
    /// sessions.ts:238. Rule E3.
    pub async fn replaceable_check(&self, thread_id: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        todo!()
    }
    /// sessions.ts:1112. Rules E4-E7.
    pub async fn settle_threads(&self, thread_ids: Vec<String>, origin: Option<Caller>, ask: SettleAsk) -> Result<SessionSettleResult, HostError> {
        todo!()
    }
    /// sessions.ts:1145. Rules E8, E9.
    pub async fn mark_threads(&self, thread_ids: Vec<String>, marks: ThreadMarks, origin: Option<Caller>) -> Result<(), HostError> {
        todo!()
    }
    /// sessions.ts:1160. Rule E10.
    pub async fn restore_threads(&self, thread_ids: Vec<String>, origin: Option<Caller>) -> Result<SessionRestoreResult, HostError> {
        todo!()
    }
}
