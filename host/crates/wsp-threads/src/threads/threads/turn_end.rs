// SPDX-License-Identifier: AGPL-3.0-only
//! threads_turn_end: checkpoints, taken turns, steers, and what a turn changed. threads.ts:956-1142. Spec:
//! spec/threads_turn_end.md.
use std::collections::BTreeSet;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use wsp_core::types::internal::Taken;
use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
use wsp_wire::types::attachments::Attachment;
use wsp_wire::views::session::SessionOrigin;
use wsp_wire::views::workspace::Caller;

use super::{Entry, ThreadsArea, TurnRow};

/// A steer whose write is still out: once its turn ended with it unread, the row and whether a stop ended it.
#[derive(Clone, Debug, Default)]
pub struct SteerOut {
    pub ended: Option<(TurnRow, bool)>,
}

/// `{ sessionId, threadId, turnId, anchor?, kept? }` (threads.ts:963).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CheckpointAsk {
    pub session_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub anchor: Option<String>,
    pub kept: Option<String>,
}

/// What a steer carries (threads.ts:995, 1025).
#[derive(Clone, Debug, Default)]
pub struct SteerSent {
    pub prompt: String,
    pub request_id: Option<String>,
    /// `via: "slate"`.
    pub via_slate: bool,
    pub started_by: Option<SessionOrigin>,
    pub attachments: Vec<Attachment>,
}

/// `readTurnChanges`'s turn (threads.ts:1120).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TurnChangesAsk {
    pub session_id: String,
    pub turn_id: String,
    pub thread_id: String,
    pub cwd: String,
    pub from: String,
    pub started_at: i64,
    pub wrote: Option<BTreeSet<String>>,
}

/// The top read once per call (`topRead ??= topOf(..)`).
pub type TopRead = Rc<dyn Fn() -> LocalBoxFuture<'static, Option<String>>>;

impl ThreadsArea {
    /// threads.ts:957: E1.
    pub fn not_a_repo(&self, r: &WorkspaceRecord) -> bool {
        todo!()
    }
    /// threads.ts:963: E2.
    pub async fn keep_checkpoint(&self, entry: &Entry, turn: CheckpointAsk) {
        todo!()
    }
    /// threads.ts:982: E3.
    pub async fn taken_turn(&self, workspace_id: &str, taken: &Taken) -> Option<Rc<dyn SessionHandle>> {
        todo!()
    }
    /// threads.ts:995: E4.
    pub fn keep_on_turn(&self, row_id: &str, o: &SteerSent, caller: Option<&Caller>, steer_id: &str) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// threads.ts:1004: E5.
    pub fn keep_steer(&self, row_id: &str, o: &SteerSent, caller: Option<&Caller>, steer_id: &str) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// threads.ts:1009: E6.
    pub fn drop_steer(&self, row_id: &str, steer_id: &str) {
        todo!()
    }
    /// threads.ts:1017: E6.
    pub fn steer_lost(&self, row_id: &str, steer_id: &str) {
        todo!()
    }
    /// threads.ts:1025: E7.
    pub fn steer_answered(&self, row_id: &str, handle_id: &str, o: &SteerSent, caller: Option<&Caller>, steer_id: &str, landed: bool) -> bool {
        todo!()
    }
    /// threads.ts:1039: E8.
    pub fn record_steer(&self, row_id: &str, handle_id: &str, o: &SteerSent, caller: Option<&Caller>, steer_id: &str) {
        todo!()
    }
    /// threads.ts:1067: E9.
    pub async fn snapshot_of(&self, entry: &Entry, cwd: &str) -> Option<String> {
        todo!()
    }
    /// threads.ts:1086: E10.
    pub async fn top_of(&self, entry: &Entry, cwd: &str) -> Option<String> {
        todo!()
    }
    /// threads.ts:1097: E11.
    pub async fn shared_folder(&self, entry: &Entry, thread_id: &str, cwd: &str, top: TopRead, from: i64, to: i64) -> bool {
        todo!()
    }
    /// threads.ts:1110: E12.
    pub async fn own_in(&self, cwd: &str, top: TopRead, wrote: &BTreeSet<String>) -> Option<BTreeSet<String>> {
        todo!()
    }
    /// threads.ts:1120: E13. True once the range was read.
    pub async fn read_turn_changes(&self, entry: &Entry, turn: TurnChangesAsk) -> bool {
        todo!()
    }
}
