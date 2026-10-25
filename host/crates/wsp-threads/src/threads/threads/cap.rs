// SPDX-License-Identifier: AGPL-3.0-only
//! threads_cap: a computer's threads at once. threads.ts:364-521. Spec: spec/threads_cap.md.
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::session::ThreadCapWait;
use wsp_wire::views::session_events_more::SessionUncappedEventWhy;
use wsp_words::exit::HostError;

use super::ThreadsArea;

/// Where a waiting turn stands: its thread's ids for the uncapped line (`at`), what its row last said, and the two
/// ends of its wait.
#[derive(Clone)]
pub struct CapWaiting {
    pub wait: ThreadCapWait,
    pub workspace_id: String,
    pub thread_id: String,
    pub session_id: String,
    pub wake: Rc<dyn Fn()>,
    pub stop: Rc<dyn Fn(&str)>,
}

/// context.ts:731 `CapHeld`.
#[derive(Clone)]
pub struct CapHeld {
    pub place_id: Option<String>,
    pub lenders: Vec<String>,
    pub order: u64,
    pub stopped: Option<String>,
    /// The stops being read, each settled by its release (`stops`).
    pub stops: Vec<Shared<LocalBoxFuture<'static, ()>>>,
    pub waiting: Option<CapWaiting>,
    /// `(why, atOnce)` once a look let a waiting turn through.
    pub through: Option<(SessionUncappedEventWhy, i64)>,
}

/// `capWait`'s `at` (threads.ts:442).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CapAt {
    pub workspace_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub session_id: String,
    pub request_id: Option<String>,
    pub prompt: String,
}

impl ThreadsArea {
    /// threads.ts:392: C1.
    pub fn cap_hold(&self, record: &WorkspaceRecord, turn_id: &str, lender: Option<&str>) {
        todo!()
    }
    /// threads.ts:398: C2.
    pub fn cap_lend(&self, turn_id: &str, lender: &str) {
        todo!()
    }
    /// threads.ts:411: C3. Synchronous.
    pub fn cap_full(&self, record: &WorkspaceRecord, turn_id: &str) -> Option<ThreadCapWait> {
        todo!()
    }
    /// threads.ts:442: C4. Every effect lands in the call; the future only settles.
    pub fn cap_wait(&self, at: CapAt, wait: ThreadCapWait) -> LocalBoxFuture<'static, Result<(), HostError>> {
        todo!()
    }
    /// threads.ts:489: C5.
    pub fn cap_stop(&self, turn_id: &str, reason: &str) -> bool {
        todo!()
    }
    /// threads.ts:499: C6. The release.
    pub fn cap_stopping(&self, turn_id: &str) -> Box<dyn Fn()> {
        todo!()
    }
    /// threads.ts:514: C7.
    pub fn cap_left(&self, turn_id: &str) {
        todo!()
    }
}
