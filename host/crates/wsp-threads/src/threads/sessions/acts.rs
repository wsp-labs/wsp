// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:992-1110, 1189-1262: steer, answer, access, rename, read, aside and run. Spec: spec/sessions_acts.md.
use wsp_core::types::api::{AnswerAsk, SteerAsk};
use wsp_wire::views::session_events_more::{RunStep, SessionRunEvent};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::{SessionAccessResult, SessionAnswerResult, SessionAsideResult, SessionRenameResult, SessionSteerResult};
use wsp_words::exit::HostError;

use super::SessionsArea;

impl SessionsArea {
    /// sessions.ts:992. Rules C0-C3.
    pub async fn steer_row(&self, row_id: &str, ask: SteerAsk, origin: Option<Caller>) -> Result<SessionSteerResult, HostError> {
        todo!()
    }
    /// sessions.ts:1012. Rules C4, C5.
    pub async fn answer_row(&self, row_id: &str, ask: AnswerAsk, origin: Option<Caller>) -> Result<SessionAnswerResult, HostError> {
        todo!()
    }
    /// sessions.ts:1027. Rules C6-C9.
    pub async fn access_row(&self, id: &str, permission_mode: &str, origin: Option<Caller>) -> Result<SessionAccessResult, HostError> {
        todo!()
    }
    /// sessions.ts:1075. Rules C10-C12.
    pub async fn rename_row(&self, row_id: &str, title: &str, origin: Option<Caller>) -> Result<SessionRenameResult, HostError> {
        todo!()
    }
    /// sessions.ts:1108. Rule C13.
    pub async fn read_thread(&self, thread_id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        todo!()
    }
    /// sessions.ts:1189. Rules C14-C17.
    pub async fn aside_row(&self, row_id: &str, question: &str, origin: Option<Caller>, ask_id: Option<String>) -> Result<SessionAsideResult, HostError> {
        todo!()
    }
    /// sessions.ts:1232. Rules C18-C20.
    pub async fn run_step(&self, step: RunStep, origin: Option<Caller>) -> Result<SessionRunEvent, HostError> {
        todo!()
    }
}
