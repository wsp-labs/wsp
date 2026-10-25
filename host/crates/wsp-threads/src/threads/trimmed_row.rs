// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/threads/trimmed-row.ts at 65f6b89c7. Spec: spec/trimmed_row.md.
use wsp_core::types::internal::ThreadWords;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::TurnStatus;

use crate::pending::HeldThread;

/// The three reads a trimmed row makes of the runtime.
pub trait TrimmedCtx {
    /// `ctx.startedAs(workspaceId, threadId)`: the session the transcript says the thread ran as.
    fn started_as(&self, workspace_id: &str, thread_id: &str) -> Option<String>;
    /// `ctx.folderOf(workspaceId, session)`.
    fn folder_of(&self, workspace_id: &str, session: &str) -> Option<String>;
    /// `ctx.transcriptIndex.get(workspaceId)?.words.get(threadId)`.
    fn thread_words(&self, workspace_id: &str, thread_id: &str) -> Option<ThreadWords>;
}

/// trimmed-row.ts:9. `ended` is the TS default parameter: None reads `record.ended`. Rules T1-T7.
pub fn trimmed_row(ctx: &dyn TrimmedCtx, thread_id: &str, held: &HeldThread, ended: Option<TurnStatus>) -> SessionView {
    todo!()
}
