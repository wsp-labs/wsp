// SPDX-License-Identifier: AGPL-3.0-only
//! threads_turn_end: checkpoints, taken turns, steers, and what a turn changed. threads.ts:956-1142. Spec:
//! spec/threads_turn_end.md.
use std::collections::BTreeSet;
use std::rc::Rc;

use futures::future::{Either, FutureExt, LocalBoxFuture, Shared};
use serde_json::{json, Value};
use wsp_account::account::core_types::AccountError;
use wsp_adapters::port::TaskStop;
use wsp_core::console;
use wsp_core::types::internal::{no_checkpoint_log_line, Taken, TakenOutcome, RESTARTED_REASON};
use wsp_core::types::wiring::{AccessMove, Local, SessionHandle, SteerTaken, WorkspaceRecord};
use wsp_frames::True;
use wsp_wire::types::attachments::{Attachment, AttachmentRecord};
use wsp_wire::views::session::{SessionOrigin, SessionView};
use wsp_wire::views::session_events::{SessionChangesEvent, SessionSteerEvent, SlateLit, TurnChangedFile, TurnResult, TurnStatus};
use wsp_wire::views::session_events_more::{SessionCheckpointEvent, SessionEvent};
use wsp_wire::views::workspace::{road_of, scope_of, Caller, ReachState};
use wsp_wire::wire::daemon::GitDiffReply;
use wsp_wire::wire::daemon_replies::GitWorktreesReply;
use wsp_wire::wire::runtime::{SessionAnswerResult, SessionStartOutcome};
use wsp_words::attachments::attachment_record;
use wsp_words::exit::HostError;
use wsp_words::project_path::{repo_path_of, worktree_of};
use wsp_words::projects::runs_in_folder;

use super::{Entry, Row, Sessions, ThreadsArea};
use crate::threads::unit;

/// A steer whose write is still out: once its turn ended with it unread, the row and whether a stop ended it.
#[derive(Clone, Default)]
pub struct SteerOut {
    pub ended: Option<(Row, bool)>,
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

/// A taken turn's handle, its outcome replaced by the taken one (threads.ts:985).
struct Retaken {
    inner: Rc<dyn SessionHandle>,
    outcome: SessionStartOutcome,
}

impl SessionHandle for Retaken {
    fn id(&self) -> String {
        self.inner.id()
    }
    fn workspace_id(&self) -> String {
        self.inner.workspace_id()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        self.inner.finished()
    }
    fn turn_id(&self) -> String {
        self.inner.turn_id()
    }
    fn outcome(&self) -> SessionStartOutcome {
        self.outcome
    }
    fn notice(&self) -> Option<String> {
        self.inner.notice()
    }
    fn view(&self) -> SessionView {
        self.inner.view()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.inner.interrupt()
    }
    fn steer(&self, prompt: String, id: Option<String>, images: Vec<wsp_adapters::port::TurnImage>) -> Option<Local<'static, SteerTaken>> {
        self.inner.steer(prompt, id, images)
    }
    fn tells_unread(&self) -> Option<bool> {
        self.inner.tells_unread()
    }
    fn answer(&self, ask_id: String, option_id: String, reason: Option<String>) -> Option<Local<'static, SessionAnswerResult>> {
        self.inner.answer(ask_id, option_id, reason)
    }
    fn set_access(&self, mode: String) -> Option<Local<'static, AccessMove>> {
        self.inner.set_access(mode)
    }
    fn stop_task(&self, task: String) -> Option<Local<'static, TaskStop>> {
        self.inner.stop_task(task)
    }
}

/// The handle of a turn the host re-reads off the transcript: its finish is settled, its view read from the rows.
struct TakenHandle {
    id: String,
    workspace_id: String,
    turn_id: String,
    outcome: SessionStartOutcome,
    finished: Shared<LocalBoxFuture<'static, TurnResult>>,
    sessions: Sessions,
    held_view: SessionView,
}

impl SessionHandle for TakenHandle {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn workspace_id(&self) -> String {
        self.workspace_id.clone()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        self.finished.clone()
    }
    fn turn_id(&self) -> String {
        self.turn_id.clone()
    }
    fn outcome(&self) -> SessionStartOutcome {
        self.outcome
    }
    fn notice(&self) -> Option<String> {
        None
    }
    fn view(&self) -> SessionView {
        let current = self.sessions.borrow().get(&self.id).map(|row| row.borrow().view.clone());
        current.unwrap_or_else(|| self.held_view.clone())
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        futures::future::ready(()).boxed_local()
    }
    fn steer(
        &self,
        _prompt: String,
        _id: Option<String>,
        _images: Vec<wsp_adapters::port::TurnImage>,
    ) -> Option<Local<'static, SteerTaken>> {
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        None
    }
    fn answer(&self, _ask_id: String, _option_id: String, _reason: Option<String>) -> Option<Local<'static, SessionAnswerResult>> {
        None
    }
    fn set_access(&self, _mode: String) -> Option<Local<'static, AccessMove>> {
        None
    }
    fn stop_task(&self, _task: String) -> Option<Local<'static, TaskStop>> {
        None
    }
}

fn start_outcome(outcome: TakenOutcome) -> SessionStartOutcome {
    match outcome {
        TakenOutcome::Started => SessionStartOutcome::Started,
        TakenOutcome::Steered => SessionStartOutcome::Steered,
    }
}

/// `String(value)` as the TS writes it, for the answer's fields a daemon may leave out.
fn js_string(value: Option<&Value>) -> String {
    match value {
        None => "undefined".to_string(),
        Some(Value::Null) => "null".to_string(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Number(number)) => match number.as_f64() {
            Some(float) if float.fract() == 0.0 && float.abs() < 1e15 => format!("{}", float as i64),
            Some(float) => float.to_string(),
            None => number.to_string(),
        },
        Some(other) => other.to_string(),
    }
}

fn failed_result(error: String) -> TurnResult {
    TurnResult {
        status: TurnStatus::Failed,
        duration_ms: None,
        waited_ms: None,
        cost_usd: None,
        tokens: None,
        model: None,
        models: None,
        text: None,
        error: Some(error),
        unreached: None,
        refusal: None,
        limit: None,
    }
}

fn records_of(attachments: &[Attachment]) -> Vec<AttachmentRecord> {
    attachments.iter().map(attachment_record).collect()
}

impl ThreadsArea {
    fn rows_now<T>(&self, f: impl FnOnce(&indexmap::IndexMap<String, Row>) -> T) -> T {
        let sessions = self.ctx.sessions();
        let sessions = sessions.borrow();
        f(&sessions)
    }

    /// threads.ts:957: E1.
    pub fn not_a_repo(&self, r: &WorkspaceRecord) -> bool {
        runs_in_folder(&r.kind) && self.ctx.project_held(&r.project).git.is_none()
    }

    /// threads.ts:963: E2.
    pub async fn keep_checkpoint(&self, entry: &Entry, turn: CheckpointAsk) {
        let record = entry.borrow().record.borrow().clone();
        let mut reference: Option<String> = None;
        let mut based = false;
        if !self.not_a_repo(&record) {
            let frame = json!({
                "op": "git.checkpoint",
                "cwd": self.ctx.checkout_of(&record),
                "thread": turn.thread_id.clone(),
                "turn": turn.turn_id.clone(),
                "scope": record.id.clone(),
            });
            match self.ctx.with_daemon(entry, frame).await {
                Ok(taken) => {
                    reference = taken.get("ref").and_then(Value::as_str).map(str::to_string);
                    based = taken.get("based").and_then(Value::as_bool) == Some(true);
                }
                Err(e) => console::warn(&no_checkpoint_log_line(&turn.thread_id, &record.id, &e.message())),
            }
        }
        if reference.is_none() && turn.anchor.is_none() && turn.kept.is_none() {
            return;
        }
        self.ctx.record(SessionEvent::SessionCheckpointEvent(SessionCheckpointEvent {
            workspace_id: record.id.clone(),
            session_id: turn.session_id,
            at: None,
            turn_id: Some(turn.turn_id),
            thread_id: Some(turn.thread_id),
            pos: None,
            copied_from: None,
            r#ref: reference.clone(),
            based: (reference.is_some() && based).then_some(True),
            anchor: turn.anchor,
            kept: turn.kept,
        }));
    }

    /// threads.ts:982: E3. A transcript that does not read rejects, as `await ctx.openThread` does (:986), so a request
    /// whose turn is unreadable is not started a second time.
    pub async fn taken_turn(&self, workspace_id: &str, taken: &Taken) -> Result<Option<Rc<dyn SessionHandle>>, HostError> {
        let sessions = self.ctx.sessions();
        let Some(held) = sessions.borrow().get(&taken.session_id).cloned() else {
            return Ok(None);
        };
        let (handle, held_turn, held_view) = {
            let row = held.borrow();
            (row.handle.clone(), row.turn_id.clone(), row.view.clone())
        };
        if let Some(handle) = handle {
            if held_turn == taken.turn_id {
                return Ok(Some(Rc::new(Retaken { inner: handle, outcome: start_outcome(taken.outcome) })));
            }
        }
        let events = self.ctx.open_thread(workspace_id, &taken.thread_id).await?;
        let done = events.iter().find_map(|event| match event {
            SessionEvent::SessionDoneEvent(done) if done.turn_id.as_deref() == Some(taken.turn_id.as_str()) => Some(done.result.clone()),
            _ => None,
        });
        let end_reason = events.iter().find_map(|event| match event {
            SessionEvent::SessionEndEvent(end) if end.turn_id.as_deref() == Some(taken.turn_id.as_str()) => Some(end.reason.clone()),
            _ => None,
        });
        let result = match done {
            Some(result) => result,
            None => failed_result(end_reason.flatten().unwrap_or_else(|| RESTARTED_REASON.to_string())),
        };
        let finished: Shared<LocalBoxFuture<'static, TurnResult>> = futures::future::ready(result).boxed_local().shared();
        Ok(Some(Rc::new(TakenHandle {
            id: taken.session_id.clone(),
            workspace_id: workspace_id.to_string(),
            turn_id: taken.turn_id.clone(),
            outcome: start_outcome(taken.outcome),
            finished,
            sessions,
            held_view,
        })))
    }

    /// threads.ts:995: E4.
    pub fn keep_on_turn(&self, row_id: &str, o: &SteerSent, caller: Option<&Caller>, steer_id: &str) -> LocalBoxFuture<'static, ()> {
        let Some(row) = self.rows_now(|rows| rows.get(row_id).cloned()) else {
            return futures::future::ready(()).boxed_local();
        };
        let workspace_id = {
            let mut row = row.borrow_mut();
            let workspace_id = row.view.workspace_id.clone();
            let Some(live) = row.turn_live.as_mut() else {
                return futures::future::ready(()).boxed_local();
            };
            let steered = live.steered.get_or_insert_with(Default::default);
            let attachments = records_of(&o.attachments);
            steered.insert(
                steer_id.to_string(),
                wsp_core::types::internal::Steered {
                    prompt: o.prompt.clone(),
                    started_by: o.started_by.unwrap_or(SessionOrigin::Person),
                    by: scope_of(caller),
                    road: road_of(caller),
                    request_id: o.request_id.clone(),
                    attachments: (!attachments.is_empty()).then_some(attachments),
                },
            );
            workspace_id
        };
        self.ctx.persist_sessions(&workspace_id)
    }

    /// threads.ts:1004: E5.
    pub fn keep_steer(&self, row_id: &str, o: &SteerSent, caller: Option<&Caller>, steer_id: &str) -> LocalBoxFuture<'static, ()> {
        self.steers_out.borrow_mut().insert(steer_id.to_string(), SteerOut::default());
        self.keep_on_turn(row_id, o, caller, steer_id)
    }

    /// threads.ts:1009: E6.
    pub fn drop_steer(&self, row_id: &str, steer_id: &str) {
        let Some(row) = self.rows_now(|rows| rows.get(row_id).cloned()) else {
            return;
        };
        let workspace_id = {
            let mut row = row.borrow_mut();
            let workspace_id = row.view.workspace_id.clone();
            let Some(live) = row.turn_live.as_mut() else {
                return;
            };
            let Some(steered) = live.steered.as_mut() else {
                return;
            };
            if steered.shift_remove(steer_id).is_none() {
                return;
            }
            workspace_id
        };
        self.ctx.spawn(unit(self.ctx.persist_sessions(&workspace_id)));
    }

    /// threads.ts:1017: E6.
    pub fn steer_lost(&self, row_id: &str, steer_id: &str) {
        self.steers_out.borrow_mut().shift_remove(steer_id);
        self.drop_steer(row_id, steer_id);
    }

    /// threads.ts:1025: E7.
    pub fn steer_answered(
        &self,
        row_id: &str,
        handle_id: &str,
        o: &SteerSent,
        caller: Option<&Caller>,
        steer_id: &str,
        landed: bool,
    ) -> bool {
        let ended = self.steers_out.borrow_mut().shift_remove(steer_id).and_then(|out| out.ended);
        match ended {
            None if landed => {
                self.record_steer(row_id, handle_id, o, caller, steer_id);
                true
            }
            Some((row, true)) => {
                self.send_back(&row, &[steer_id.to_string()], true);
                true
            }
            _ => {
                self.drop_steer(row_id, steer_id);
                false
            }
        }
    }

    /// threads.ts:1039: E8.
    pub fn record_steer(&self, row_id: &str, handle_id: &str, o: &SteerSent, caller: Option<&Caller>, steer_id: &str) {
        let Some(row) = self.rows_now(|rows| rows.get(row_id).cloned()) else {
            return;
        };
        let (workspace_id, thread_id, turn_id, claude_session_id, asking, kept_here) = {
            let row = row.borrow();
            let kept_here =
                row.turn_live.as_ref().and_then(|live| live.steered.as_ref()).is_some_and(|steered| steered.contains_key(steer_id));
            (
                row.view.workspace_id.clone(),
                row.view.thread_id.clone(),
                row.turn_id.clone(),
                row.view.claude_session_id.clone(),
                row.view.asking.is_some(),
                kept_here,
            )
        };
        if !kept_here {
            self.ctx.spawn(unit(self.keep_on_turn(row_id, o, caller, steer_id)));
        }
        let records = records_of(&o.attachments);
        if let Some(thread) = &thread_id {
            if !records.is_empty() {
                self.ctx.spawn(unit(self.ctx.keep_sent_images(&workspace_id, thread, o.request_id.clone(), o.attachments.clone())));
            }
        }
        self.ctx.record(SessionEvent::SessionSteerEvent(SessionSteerEvent {
            workspace_id: workspace_id.clone(),
            session_id: claude_session_id.unwrap_or_else(|| handle_id.to_string()),
            at: None,
            turn_id: Some(turn_id),
            thread_id,
            pos: None,
            copied_from: None,
            prompt: o.prompt.clone(),
            attachments: (!records.is_empty()).then_some(records),
            request_id: o.request_id.clone(),
            via: o.via_slate.then_some(SlateLit),
            waiting: asking.then_some(true),
        }));
    }

    /// threads.ts:1067: E9.
    pub async fn snapshot_of(&self, entry: &Entry, cwd: &str) -> Option<String> {
        let record = entry.borrow().record.borrow().clone();
        if self.not_a_repo(&record) || self.ctx.reach_of(entry) != ReachState::Reachable {
            return None;
        }
        let ms = self.ctx.turn_snapshot_ms().unwrap_or(3_000);
        let (late_sender, late) = futures::channel::oneshot::channel::<()>();
        let cancel = self.ctx.clock().schedule(
            Box::new(move || {
                let _ = late_sender.send(());
            }),
            ms,
        );
        let asked = self.ctx.with_daemon(entry, json!({ "op": "git.snapshot", "cwd": cwd }));
        let (name, folder) = (record.name.clone(), cwd.to_string());
        let taken = async move {
            match asked.await {
                Ok(answer) => Some(js_string(answer.get("commit"))),
                Err(e) => {
                    let silent = matches!(&e, AccountError::Refusal(refusal) if refusal.code.as_deref() == Some("not-a-git-repo"));
                    if !silent {
                        console::warn(&format!("no snapshot of {folder} on {name}: {}", e.message()));
                    }
                    None
                }
            }
        }
        .boxed_local();
        let outcome = futures::future::select(taken, late).await;
        cancel();
        match outcome {
            Either::Left((commit, _)) => commit,
            Either::Right(_) => {
                console::warn(&format!(
                    "no snapshot of {cwd} on {} inside {ms}ms; the turn runs without one and its reply lists no changes",
                    record.name
                ));
                None
            }
        }
    }

    /// threads.ts:1086: E10.
    pub async fn top_of(&self, entry: &Entry, cwd: &str) -> Option<String> {
        let reply = self.ctx.with_daemon(entry, json!({ "op": "git.worktrees", "cwd": cwd })).await.ok()?;
        let read: GitWorktreesReply = serde_json::from_value(reply).ok()?;
        let paths: Vec<&str> = read.worktrees.iter().filter(|w| w.prunable != Some(true)).map(|w| w.path.as_str()).collect();
        worktree_of(cwd, &paths)
    }

    /// threads.ts:1097: E11.
    pub async fn shared_folder(&self, entry: &Entry, thread_id: &str, cwd: &str, top: TopRead, from: i64, to: i64) -> bool {
        let workspace_id = entry.borrow().record.borrow().id.clone();
        let (from, to) = (from as f64, to as f64);
        let beside: Vec<String> = self.rows_now(|rows| {
            rows.values()
                .filter_map(|row| {
                    let row = row.borrow();
                    let view = &row.view;
                    let started = view.started_at.map_or(0.0, |at| at.0);
                    let ended = view.ended_at.map_or(to, |at| at.0);
                    let counts =
                        view.workspace_id == workspace_id && view.thread_id.as_deref() != Some(thread_id) && started <= to && ended >= from;
                    counts.then(|| view.cwd.clone()).flatten()
                })
                .collect()
        });
        if beside.iter().any(|folder| folder == cwd) {
            return true;
        }
        if beside.is_empty() {
            return false;
        }
        let Some(mine) = top().await else {
            return false;
        };
        let mut seen: Vec<String> = Vec::new();
        for folder in beside {
            if seen.contains(&folder) {
                continue;
            }
            if self.top_of(entry, &folder).await.as_deref() == Some(mine.as_str()) {
                return true;
            }
            seen.push(folder);
        }
        false
    }

    /// threads.ts:1110: E12.
    pub async fn own_in(&self, cwd: &str, top: TopRead, wrote: &BTreeSet<String>) -> Option<BTreeSet<String>> {
        if wrote.is_empty() {
            return Some(BTreeSet::new());
        }
        let root = top().await?;
        Some(wrote.iter().filter_map(|path| repo_path_of(path, cwd, &root)).collect())
    }

    /// threads.ts:1120: E13. True once the range was read.
    pub async fn read_turn_changes(&self, entry: &Entry, turn: TurnChangesAsk) -> bool {
        let Some(to) = self.snapshot_of(entry, &turn.cwd).await else {
            return false;
        };
        let frame = json!({ "op": "git.turn", "cwd": turn.cwd.clone(), "from": turn.from.clone(), "to": to.clone() });
        let range = match self.ctx.with_daemon(entry, frame).await {
            Ok(range) => range,
            Err(e) => {
                console::warn(&format!("what {} changed in {} was not read: {}", turn.turn_id, turn.cwd, e.message()));
                return false;
            }
        };
        let Ok(read) = serde_json::from_value::<GitDiffReply>(range) else {
            return false;
        };
        let files: Vec<TurnChangedFile> = read
            .files
            .iter()
            .map(|file| TurnChangedFile {
                path: file.path.clone(),
                kind: file.kind.clone(),
                additions: file.additions,
                deletions: file.deletions,
            })
            .collect();
        let moved = read.moved.clone();
        if files.is_empty() && moved.is_empty() {
            return true;
        }
        let Some(me) = self.me.upgrade() else {
            return false;
        };
        let memo: Shared<LocalBoxFuture<'static, Option<String>>> = {
            let (area, owned_entry, cwd) = (me.clone(), entry.clone(), turn.cwd.clone());
            async move { area.top_of(&owned_entry, &cwd).await }.boxed_local().shared()
        };
        let top: TopRead = Rc::new(move || {
            let memo = memo.clone();
            memo.boxed_local()
        });
        let now = self.ctx.clock().now();
        let shared = self.shared_folder(entry, &turn.thread_id, &turn.cwd, top.clone(), turn.started_at, now).await;
        let own = match (shared, &turn.wrote) {
            (true, Some(wrote)) => self.own_in(&turn.cwd, top, wrote).await,
            _ => None,
        };
        let (files, others) = match own {
            None => (files, None),
            Some(own) => {
                let (mine, rest): (Vec<_>, Vec<_>) = files.into_iter().partition(|file| own.contains(&file.path));
                (mine, Some(rest))
            }
        };
        let workspace_id = entry.borrow().record.borrow().id.clone();
        self.ctx.record(SessionEvent::SessionChangesEvent(SessionChangesEvent {
            workspace_id,
            session_id: turn.session_id,
            at: None,
            turn_id: Some(turn.turn_id),
            thread_id: Some(turn.thread_id),
            pos: None,
            copied_from: None,
            from: turn.from,
            to,
            files,
            moved,
            shared: shared.then_some(True),
            others,
        }));
        true
    }
}
