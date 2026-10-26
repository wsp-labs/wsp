// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:1398-1480: delete and forget a thread. Spec: spec/sessions_delete.md.
use indexmap::IndexSet;
use wsp_core::types::api::{DeleteThreadAsk, ThreadDeleted};
use wsp_core::types::internal::EndThread;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_machines::machines::boot::entries::RowAt;
use wsp_wire::types::projects::{THREAD_WORKING_LINE, WORKTREE_BUSY_LINE};
use wsp_wire::views::activity::ProcessesAsk;
use wsp_wire::views::session::{thread_ran, SessionStatus, SessionView};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::{not_found_refusal, HostError, Kind};
use wsp_words::projects::{runs_in_folder, thread_on_machine_line};
use wsp_words::words::computer::{thread_forget_refusal, thread_word};

use super::{HeldSend, SessionsArea, SessionsCtx};
use crate::activity::MeetVerb;
use crate::threads::threads::Entry;

fn record_of(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

fn conflict(line: &str) -> HostError {
    HostError::with_kind(line.to_string(), Kind::Conflict)
}

impl SessionsArea {
    /// sessions.ts:1398. Rules D1-D8.
    pub async fn delete_thread(&self, thread_id: &str, origin: Option<Caller>, ask: DeleteThreadAsk) -> Result<ThreadDeleted, HostError> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        let from_rows = ctx.rows_on(thread_id).first().map(|view| view.workspace_id.clone());
        let workspace_id = from_rows.or_else(|| ctx.records().borrow().get(thread_id).map(|record| record.workspace_id.clone()));
        let act = if ctx.lead_of_sub(workspace_id.as_deref(), origin.as_ref()) { "delete_sub" } else { "delete" };
        ctx.spawn_guard(act, origin.as_ref())?;
        let Some(workspace_id) = workspace_id else {
            return Err(not_found_refusal(&format!("no thread {}", thread_word(thread_id))));
        };
        let at = RowAt { thread_id: Some(thread_id.to_string()), workspace_id: workspace_id.clone() };
        let Some(entry) = ctx.entry_of_row(&at, origin.as_ref(), None, false).await? else {
            return Err(not_found_refusal(&format!("no thread {}", thread_word(thread_id))));
        };
        let record = record_of(&entry);
        if record.sub.is_some() {
            return ctx.sub_delete(&entry, thread_id, origin.as_ref(), ask).await;
        }
        if !runs_in_folder(&record.kind) {
            return Err(HostError::with_kind(thread_on_machine_line(&record.name), Kind::Usage));
        }
        let mut threads: IndexSet<String> = IndexSet::new();
        for (_, row) in ctx.sessions().borrow().iter() {
            let view = row.borrow().view.clone();
            if view.workspace_id == workspace_id {
                if let Some(thread) = view.thread_id {
                    threads.insert(thread);
                }
            }
        }
        for (id, held) in ctx.records().borrow().iter() {
            if held.workspace_id == workspace_id {
                threads.insert(id.clone());
            }
        }
        let worktree = match record.worktree.as_ref() {
            Some(tree) if tree.made && tree.gone.is_none() && threads.iter().all(|t| t == thread_id) => Some(tree.path.clone()),
            _ => None,
        };
        if worktree.is_some() && ctx.turn_runs(&workspace_id) {
            return Err(conflict(WORKTREE_BUSY_LINE));
        }
        let held: Vec<(String, HeldSend)> = self
            .stops
            .borrow()
            .held_sends
            .iter()
            .filter(|(_, send)| send.thread_id == thread_id)
            .map(|(id, send)| (id.clone(), send.clone()))
            .collect();
        let working = ctx.sessions().borrow().iter().any(|(id, row)| {
            let r = row.borrow();
            r.view.thread_id.as_deref() == Some(thread_id)
                && r.view.status == SessionStatus::Running
                && !self.stops.borrow().held_sends.contains_key(id)
        });
        if working {
            return Err(conflict(THREAD_WORKING_LINE));
        }
        let keep = ask.processes == Some(ProcessesAsk::Keep);
        if ask.check == Some(true) {
            if let Some(processes) = ask.processes {
                let processes = if processes == ProcessesAsk::Stop { ProcessesAsk::Keep } else { processes };
                ctx.activity().meet(&[thread_id.to_string()], processes, MeetVerb::Delete, origin.clone()).await?;
            }
            return Ok(deleted(&workspace_id, worktree, Vec::new()));
        }
        let ended = match ask.processes {
            None => Vec::new(),
            Some(processes) => ctx.activity().meet(&[thread_id.to_string()], processes, MeetVerb::Delete, origin.clone()).await?,
        };
        if let Some(worktree) = worktree {
            if !keep {
                if let Some(ending) = ctx.end_thread(&entry, thread_id, EndThread { remove: true, away: false }) {
                    ending.await?;
                }
            }
            if !keep {
                ctx.end_thread_shells(&entry, thread_id).await?;
            }
            ctx.delete_workspace(&workspace_id, origin.as_ref()).await?;
            return Ok(deleted(&workspace_id, Some(worktree), ended));
        }
        for (_, send) in &held {
            (send.give_up)();
        }
        for (id, _) in &held {
            let launch = ctx.sessions().borrow().get(id).and_then(|row| row.borrow().launch.clone());
            if let Some(launch) = launch {
                launch.await;
            }
        }
        if ctx.thread_runs(thread_id) {
            return Err(conflict(THREAD_WORKING_LINE));
        }
        SessionsCtx::reap_kept(&*ctx, thread_id);
        if !keep {
            if let Some(ending) = ctx.end_thread(&entry, thread_id, EndThread { remove: true, away: false }) {
                ending.await?;
            }
        }
        if !keep {
            ctx.end_thread_shells(&entry, thread_id).await?;
        }
        ctx.open_thread(&workspace_id, thread_id).await?;
        ctx.drop_checkpoints(&entry, thread_id).await?;
        ctx.drop_thread_files(&entry, &[thread_id.to_string()]).await?;
        ctx.drop_sent_images(&workspace_id, &[thread_id.to_string()]).await?;
        let rows: Vec<String> = ctx
            .sessions()
            .borrow()
            .iter()
            .filter(|(_, row)| row.borrow().view.thread_id.as_deref() == Some(thread_id))
            .map(|(id, _)| id.clone())
            .collect();
        for id in rows {
            ctx.sessions().borrow_mut().shift_remove(&id);
            ctx.row_gone(&id, &workspace_id, thread_id);
        }
        ctx.records().borrow_mut().shift_remove(thread_id);
        ctx.drop_from_transcript(&workspace_id, thread_id, None).await?;
        ctx.persist_sessions(&workspace_id).await;
        ctx.slates_forget(thread_id, keep).await?;
        Ok(deleted(&workspace_id, None, ended))
    }

    /// sessions.ts:1453. Rules D9-D11.
    pub async fn forget_thread(&self, thread_id: &str, origin: Option<Caller>, check: bool) -> Result<(), HostError> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        let held: Vec<(String, SessionView)> = ctx
            .sessions()
            .borrow()
            .iter()
            .filter(|(_, row)| row.borrow().view.thread_id.as_deref() == Some(thread_id))
            .map(|(id, row)| (id.clone(), row.borrow().view.clone()))
            .collect();
        let record = ctx.records().borrow().get(thread_id).cloned();
        let Some(workspace_id) =
            held.first().map(|(_, view)| view.workspace_id.clone()).or_else(|| record.as_ref().map(|r| r.workspace_id.clone()))
        else {
            return Err(not_found_refusal(&format!("no thread {}", thread_word(thread_id))));
        };
        let at = RowAt { thread_id: Some(thread_id.to_string()), workspace_id: workspace_id.clone() };
        let Some(entry) = ctx.entry_of_row(&at, origin.as_ref(), None, false).await? else {
            return Err(not_found_refusal(&format!("no thread {}", thread_word(thread_id))));
        };
        let views: Vec<SessionView> = held.iter().map(|(_, view)| view.clone()).collect();
        if thread_ran(&views) || record.as_ref().is_some_and(|r| r.worked != Some(false)) {
            return Err(conflict(&thread_forget_refusal(thread_id)));
        }
        if check {
            return Ok(());
        }
        ctx.open_thread(&workspace_id, thread_id).await?;
        ctx.drop_thread_files(&entry, &[thread_id.to_string()]).await?;
        ctx.drop_sent_images(&workspace_id, &[thread_id.to_string()]).await?;
        for (id, _) in &held {
            ctx.sessions().borrow_mut().shift_remove(id);
            ctx.row_gone(id, &workspace_id, thread_id);
        }
        ctx.records().borrow_mut().shift_remove(thread_id);
        ctx.slates_forget(thread_id, false).await?;
        ctx.drop_from_transcript(&workspace_id, thread_id, None).await?;
        ctx.persist_sessions(&workspace_id).await;
        Ok(())
    }
}

fn deleted(workspace_id: &str, worktree: Option<String>, ended: Vec<wsp_wire::views::activity::ActivityEnded>) -> ThreadDeleted {
    ThreadDeleted {
        workspace_id: workspace_id.to_string(),
        worktree,
        threads: 1,
        ended: if ended.is_empty() { None } else { Some(ended) },
        machine_deleted: false,
        on: None,
        question: None,
    }
}
