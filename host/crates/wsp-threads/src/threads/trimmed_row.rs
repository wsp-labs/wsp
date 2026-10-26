// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/threads/trimmed-row.ts at 65f6b89c7. Spec: spec/trimmed_row.md.
use wsp_core::types::internal::{ThreadRecord, ThreadWords};
use wsp_wire::views::session::{SessionOrigin, SessionStatus, SessionView};
use wsp_wire::views::session_events::TurnStatus;

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
pub fn trimmed_row(ctx: &dyn TrimmedCtx, thread_id: &str, held: &ThreadRecord, ended: Option<TurnStatus>) -> SessionView {
    let ended = ended.or(held.ended);
    let status = match ended {
        Some(TurnStatus::Failed) => SessionStatus::Failed,
        Some(TurnStatus::Interrupted) => SessionStatus::Interrupted,
        Some(TurnStatus::Completed) | None => SessionStatus::Completed,
    };
    let resume = ctx.started_as(&held.workspace_id, thread_id);
    let cwd = resume.as_deref().and_then(|session| ctx.folder_of(&held.workspace_id, session));
    let words = ctx.thread_words(&held.workspace_id, thread_id);
    let times = words.as_ref().filter(|w| w.last.0 > 0.0).map(|w| w.last);
    SessionView {
        id: thread_id.to_string(),
        workspace_id: held.workspace_id.clone(),
        harness: held.harness.clone(),
        status,
        started_by: held.parent_thread_id.is_some().then_some(SessionOrigin::Agent),
        claude_session_id: resume,
        thread_id: Some(thread_id.to_string()),
        parent_thread_id: held.parent_thread_id.clone(),
        root_thread_id: held.root_thread_id.clone(),
        attempt: None,
        prompt: words.as_ref().and_then(|w| w.first.clone()),
        harness_title: None,
        title_source: None,
        started_at: times,
        ended_at: times,
        cwd: cwd,
        refusal: None,
        limit: None,
        resume_at: None,
        cost_usd: None,
        last_line: None,
        failure: None,
        model: None,
        effort: None,
        permission_mode: held.permission_mode.clone(),
        context_window: None,
        fast: None,
        asking: None,
        waiting_on: None,
        setup_refusal: None,
        project: None,
        computer_name: None,
        sub: None,
        disk_kept_until: None,
        disk_gone: None,
        capped: None,
        waits_for: None,
        pid: None,
        agent_pid: None,
        subagents: None,
        read_at: None,
        settled_at: None,
        pinned_at: None,
        order: None,
        folded_at: None,
        replaces: None,
        replaced_by: None,
        snoozed_until: None,
        woke_at: None,
        section: None,
        rewound_at: None,
        forked_from: held.forked_from.clone(),
    }
}
