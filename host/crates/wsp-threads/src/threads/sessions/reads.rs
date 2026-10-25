// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:901-975, 1175-1187: the reads of threads. Spec: spec/sessions_reads.md.
use wsp_core::types::api::{HistoryAsk, PageWindow};
use wsp_wire::types::attachments::KeptAttachment;
use wsp_wire::views::preferences::{HistoryPage, ThreadHead};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::SessionSearchResult;
use wsp_words::exit::HostError;

use super::SessionsArea;

impl SessionsArea {
    /// sessions.ts:901. Rules Q1-Q3.
    pub async fn list_rows(&self, workspace_id: Option<String>, origin: Option<Caller>) -> Result<Vec<SessionView>, HostError> {
        todo!()
    }
    /// sessions.ts:928. Rule Q4.
    pub async fn history_of(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Vec<SessionEvent>, HostError> {
        todo!()
    }
    /// sessions.ts:936. Rule Q5.
    pub async fn thread_history_of(&self, workspace_id: &str, asked: HistoryAsk, origin: Option<Caller>) -> Result<Vec<SessionEvent>, HostError> {
        todo!()
    }
    /// sessions.ts:945. Rule Q6.
    pub async fn page_of(&self, workspace_id: &str, window: PageWindow, origin: Option<Caller>) -> Result<HistoryPage, HostError> {
        todo!()
    }
    /// sessions.ts:957. Rule Q7.
    pub async fn head_of(&self, thread_id: &str, origin: Option<Caller>, after_read: bool) -> Result<ThreadHead, HostError> {
        todo!()
    }
    /// sessions.ts:966. Rule Q8.
    pub async fn attachment_of(&self, workspace_id: &str, thread_id: &str, request_id: &str, index: u32, origin: Option<Caller>) -> Result<KeptAttachment, HostError> {
        todo!()
    }
    /// sessions.ts:1175. Rule Q9.
    pub async fn search_for(&self, query: &str, origin: Option<Caller>) -> Result<SessionSearchResult, HostError> {
        todo!()
    }
}
