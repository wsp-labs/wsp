// SPDX-License-Identifier: AGPL-3.0-only
//! threads_owed: lines into threads, kept until a turn takes them, and the cut end. threads.ts:24-62, 586-955.
//! Spec: spec/threads_owed.md.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_core::types::wiring::SessionHandle;
use wsp_wire::types::attachments::{Attachment, AttachmentRecord};
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::session::SessionOrigin;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};

use super::tree::TellAs;
use super::{Row, ThreadsArea};


pub use wsp_core::types::internal::{NOTIFY_OWED, OWED_RETRY_MS, OWED_RETRY_MAX_MS, OWED_FOR_MS};
pub use wsp_wire::views::session_events::{NOTIFY_ME};
/// threads.ts:25 `PersonRow`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonRow {
    pub workspace_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub text: String,
}

/// threads.ts:33 `OwedImages`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwedImages {
    pub under: String,
    pub records: Vec<AttachmentRecord>,
}

/// threads.ts:31 `Owed`: O1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Owed {
    pub id: String,
    pub from: String,
    pub notify: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<ThreadScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<WorkspaceOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_by: Option<SessionOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_person: Option<PersonRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tries: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<OwedImages>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub awaits_result: Option<String>,
}

/// threads.ts:45 `readOwed`: O1.
pub fn read_owed(raw: &Value) -> Option<Owed> {
    todo!()
}

/// The handle a try is settled on: the row id and turn id, and its finish where it has one (threads.ts:745).
#[derive(Clone)]
pub struct TryHandle {
    pub id: String,
    pub turn_id: String,
    pub finished: Option<futures::future::Shared<futures::future::LocalBoxFuture<'static, TurnResult>>>,
    pub tells_unread: Option<bool>,
}

impl TryHandle {
    /// A handle a start answered.
    pub fn of(handle: &dyn SessionHandle) -> TryHandle {
        todo!()
    }
}

impl ThreadsArea {
    /// threads.ts:677 `deliver`: O2 to O16. Synchronous up to its detached task.
    pub fn deliver(&self, line: Owed, woke: bool) {
        todo!()
    }
    /// threads.ts:622: O13.
    pub fn took_request(&self, workspace_id: &str, request_id: &str) -> bool {
        todo!()
    }
    /// threads.ts:629: O13. The row's id names it in `sessions`.
    pub fn line_try(&self, row_id: &str) -> bool {
        todo!()
    }
    /// threads.ts:637: a try's agent now holds the request's prompt.
    pub fn prompt_held(&self, request_id: &str) {
        todo!()
    }
    /// threads.ts:641: the person stopped a dark try's turn.
    pub fn try_stopped(&self, turn_id: &str) {
        todo!()
    }
    /// threads.ts:655: O14. The row id.
    pub fn carrying(&self, thread_id: &str, request_id: &str) -> Option<String> {
        todo!()
    }
    /// threads.ts:852: O15.
    pub async fn kept_images(&self, workspace_id: &str, thread_id: &str, images: &OwedImages) -> Vec<Attachment> {
        todo!()
    }
    /// threads.ts:870: O17.
    pub fn send_back(&self, s: &Row, ids: &[String], stopped: bool) {
        todo!()
    }
    /// threads.ts:905: O19. `open`: a reply given over work still running.
    pub fn notify_end(&self, s: &Row, notify: &[String], named: &TellAs, result: &TurnResult, open: bool) {
        todo!()
    }
    /// threads.ts:925: O20.
    pub async fn deliver_owed(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    /// threads.ts:936: O21. Changes the row `row_id` names in `sessions`; `cut_line` gets the end's time.
    pub fn settle_cut(&self, row_id: &str, reason: &str, cut_line: &dyn Fn(i64) -> String, stopped: bool, unreached: bool) {
        todo!()
    }
}
