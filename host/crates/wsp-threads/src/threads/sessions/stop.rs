// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:41-207, 977-990, 1482-1485: a stop on a turn and the holds on a thread's next turn.
//! Spec: spec/sessions_stop.md.
use std::rc::Rc;

use wsp_core::context::Joined;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::{SessionInterruptOutcome, SessionInterruptResult};
use wsp_words::exit::HostError;

use super::SessionsArea;
use crate::threads::threads::Row;

/// What `end_turn` answers (sessions.ts:151).
#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    pub outcome: SessionInterruptOutcome,
    pub left: Option<String>,
}

impl SessionsArea {
    /// sessions.ts:55. Rule K1.
    pub fn hold_next(&self, thread_id: &str, until: Joined, box_name: Option<&str>) {
        todo!()
    }

    /// sessions.ts:74. Rule K2.
    pub async fn stop_unheard(&self, place: &str, stopping: Option<Joined>) -> bool {
        todo!()
    }

    /// sessions.ts:102. `marked` lets a held turn's next look go on. Rules K3-K9.
    pub async fn stop_turn(&self, marked: Rc<dyn Fn()>, row_id: &str, origin: Option<&Caller>, task: Option<String>) -> Result<SessionInterruptResult, HostError> {
        todo!()
    }

    /// sessions.ts:151. `row_of` reads the row again by its key. Rules K10-K16.
    pub async fn end_turn(&self, row: Row, origin: Option<&Caller>, row_of: Rc<dyn Fn() -> Option<Row>>) -> Result<Ended, HostError> {
        todo!()
    }

    /// sessions.ts:977. Rules K17, K18.
    pub async fn interrupt_row(&self, row_id: &str, origin: Option<Caller>, task: Option<String>) -> Result<SessionInterruptResult, HostError> {
        todo!()
    }

    /// sessions.ts:1482. Rule K19.
    pub async fn stop_try(&self, row_id: &str) -> Result<(), HostError> {
        todo!()
    }
}
