// SPDX-License-Identifier: AGPL-3.0-only
//! turns.ts:743-866: the sweep of runs no thread holds, and the re-open of a turn the store left running.
//! Spec: spec/turns_reopen.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use wsp_adapters::port::{AdapterAttachOptions, AdapterEvent, OnEvent};
use wsp_core::console;
use wsp_core::machine_exec::rules::TurnWaiting;
use wsp_core::types::internal::{swept_runs_log_line, turn_written, Reopened};
use wsp_wire::views::session::SessionStatus;
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::exit::HostError;
use wsp_words::words::computer::thread_word;

use super::{Open, Opening, SnapshotFrom, TurnSnapshot, TurnStart, TurnsArea};
use crate::threads::agents::launch::AdapterAsk;
use crate::threads::threads::{Entry, Row};

impl TurnsArea {
    /// turns.ts:748. Rules P1-P3.
    pub async fn sweep_runs(&self, entry: &Entry) {
        let (workspace_id, running) = {
            let live = entry.borrow();
            let record = live.record.borrow();
            (record.id.clone(), record.phase == WorkspacePhase::Running)
        };
        if !running || self.ctx.is_held_away(&workspace_id) {
            return;
        }
        let Some(sweep) = self.ctx.exec_sweep(entry) else {
            return;
        };
        let held: Vec<String> = {
            let sessions = self.ctx.sessions();
            let rows = sessions.borrow();
            let held = rows
                .values()
                .filter_map(|row| {
                    let row = row.borrow();
                    (row.view.status == SessionStatus::Running).then(|| row.run.clone()).flatten()
                })
                .collect();
            held
        };
        let swept = match sweep(held).await {
            Ok(swept) => swept,
            Err(e) => {
                console::warn(&format!("the runs on {workspace_id} were left as they are: {}", e.message));
                return;
            }
        };
        if !swept.is_empty() {
            console::warn(&swept_runs_log_line(&workspace_id, &swept));
        }
    }

    /// turns.ts:774. `again` is a second try at the same row. Rules P4-P9.
    pub async fn reattach(&self, row: Row, again: bool) -> Reopened {
        let (view, run, turn_id, asked, turn_live, from, notify, notify_by, notify_road, turn_token, scope_device_id, snapshot) = {
            let r = row.borrow();
            (
                r.view.clone(),
                r.run.clone(),
                r.turn_id.clone(),
                r.asked.clone(),
                r.turn_live.clone(),
                r.from,
                r.notify.clone(),
                r.notify_by.clone(),
                r.notify_road,
                r.turn_token.clone(),
                r.scope_device_id.clone(),
                r.snapshot.clone(),
            )
        };
        let thread_id = view.thread_id.clone();
        let entry = self.ctx.live_entry(&view.workspace_id);
        let (Some(run), Some(thread_id), Some(entry)) = (run, thread_id, entry) else {
            return Reopened::Cannot;
        };
        if entry.borrow().record.borrow().phase != WorkspacePhase::Running {
            return Reopened::Cannot;
        }
        let cannot = |words: &str| {
            console::warn(&format!("thread {} on {} cannot be re-opened: {words}", thread_word(&thread_id), view.workspace_id));
            Reopened::Cannot
        };
        let waiting: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let on_waiting = waiting.clone();
        let built = self.ctx.adapter_for(
            &entry,
            AdapterAsk {
                named: Some(view.harness.clone()),
                waiting: Some(Arc::new(move || on_waiting.get()) as TurnWaiting),
                ..AdapterAsk::default()
            },
        );
        let adapter = match built {
            Ok(built) => built.adapter,
            Err(e) => return cannot(&e.message),
        };
        let left = |e: &HostError| {
            if !again {
                console::warn(&format!("thread {} on {} was left running: {}", thread_word(&thread_id), view.workspace_id, e.message));
            }
            Reopened::Unreached
        };
        let transcript = match self.ctx.open_thread(&view.workspace_id, &thread_id).await {
            Ok(transcript) => transcript,
            Err(e) => return left(&e),
        };
        let held: Rc<RefCell<Vec<AdapterEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink: Rc<RefCell<Option<OnEvent>>> = Rc::new(RefCell::new(None));
        let (early, route) = (held.clone(), sink.clone());
        let on_event: OnEvent = Arc::new(move |event: AdapterEvent| {
            let forward = route.borrow().clone();
            match forward {
                Some(forward) => forward(event),
                None => early.borrow_mut().push(event),
            }
        });
        let options = AdapterAttachOptions {
            run,
            session_id: view.claude_session_id.clone().unwrap_or_else(|| view.id.clone()),
            started_at: view.started_at.map_or_else(|| self.ctx.clock().now() as f64, |n| n.0),
            model: view.model.clone(),
            cwd: view.cwd.clone(),
            prompt: asked.as_ref().map(|a| a.prompt.clone()),
            effort: asked.as_ref().and_then(|a| a.effort.clone()),
            from,
            steered: turn_live.as_ref().and_then(|live| live.steered.as_ref()).map(|steered| steered.keys().cloned().collect()),
            on_event,
        };
        let Some(attach) = adapter.attach(options) else {
            return Reopened::Cannot;
        };
        let session = match attach.await {
            Ok(Some(session)) => session,
            Ok(None) => return Reopened::Gone,
            Err(e) => return left(&HostError::new(e.message)),
        };
        entry.borrow_mut().unchecked = false;
        let written = turn_written(&transcript, &turn_id);
        let typed = asked
            .as_ref()
            .and_then(|a| a.typed.clone())
            .or_else(|| asked.as_ref().map(|a| a.prompt.clone()))
            .or_else(|| view.prompt.clone())
            .unwrap_or_default();
        let snapshot = match (snapshot, view.cwd.clone()) {
            (Some(commit), Some(cwd)) => Some(TurnSnapshot { from: SnapshotFrom::Known(Some(commit)), cwd }),
            _ => None,
        };
        let start = TurnStart {
            entry: entry.clone(),
            row: row.clone(),
            written: Some(written),
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            notify,
            notify_by,
            notify_road,
            turn_token,
            scope_device_id,
            outcome: SessionStartOutcome::Started,
            opening: Opening { prompt: typed, request_id: asked.as_ref().and_then(|a| a.request_id.clone()), ..Opening::default() },
            resume: None,
            images_dir: None,
            snapshot,
            reports_edits: adapter.reports_edits(),
            waiting: Some(waiting),
            keep: None,
            open: Box::new(move |forward: OnEvent| {
                *sink.borrow_mut() = Some(forward.clone());
                let pending: Vec<AdapterEvent> = held.borrow_mut().drain(..).collect();
                for event in pending {
                    forward(event);
                }
                Ok(session)
            }) as Open,
        };
        match self.run_turn(start) {
            Ok(_) => Reopened::Attached,
            Err(e) => cannot(&e.message),
        }
    }
}
