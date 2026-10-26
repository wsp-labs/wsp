// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:992-1110, 1189-1262: steer, answer, access, rename, read, aside and run. Spec: spec/sessions_acts.md.
use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Map};
use wsp_adapters::port::{AsideQuestion, Env, ServerValues, SessionRenameWrite, TurnImage};
use wsp_catalog::signin_rows::server_values_of;
use wsp_core::console;
use wsp_core::harness_catalog::harness_catalog;
use wsp_core::types::api::{AnswerAsk, SteerAsk};
use wsp_core::types::internal::ThreadRecord;
use wsp_machines::machines::boot::entries::RowAt;
use wsp_wire::types::attachments::AttachmentRecord;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::run_block::RUN_OUTPUT_MAX_CHARS;
use wsp_wire::views::harness::{start_picks, StartPicks};
use wsp_wire::views::place_more::AsideTextEvent;
use wsp_wire::views::session::{thread_key_of, SessionStatus, SessionView, TitleSource};
use wsp_wire::views::session_events::RUN_PERSONS_LINE;
use wsp_wire::views::session_events_more::{RunState, RunStep, SessionEvent, SessionRunEvent};
use wsp_wire::views::workspace::{scope_of, Caller};
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_wire::types::wsp_tools::MCP_SERVER_NAME;
use wsp_wire::wire::runtime::{
    SessionAccessOutcome, SessionAccessResult, SessionAnswerOutcome, SessionAnswerResult, SessionAsideResult, SessionRenameOutcome, SessionRenameResult,
    SessionSteerOutcome, SessionSteerResult,
};
use wsp_words::attachments::{attachment_record, files_refusal, steer_files_blocked};
use wsp_words::exit::{not_found_refusal, HostError, Kind};
use wsp_words::run_block::run_output_tail;
use wsp_words::words::computer::thread_word;
use wsp_words::words::thread::{aside_unsupported_line, ASIDE_EMPTY_LINE, ASIDE_NO_SESSION_LINE, BLANK_ASIDE_LINE, EMPTY_TITLE_LINE};
use wsp_words::workspace_state::{action_refusal, send_refusal, workspace_state, SendRefusalKind, WorkspaceStateInput};

use super::{SessionsArea, StampsOf};
use crate::threads::agents::launch::AdapterAsk;
use crate::threads::launch::served_to;
use crate::threads::threads::{Entry, Row, Sessions};

fn row_at(view: &SessionView) -> RowAt {
    RowAt { thread_id: view.thread_id.clone(), workspace_id: view.workspace_id.clone() }
}

fn row_by_key(sessions: &Sessions, id: &str) -> Option<Row> {
    sessions.borrow().get(id).cloned()
}

fn row_for_view(sessions: &Sessions, view: &SessionView) -> Option<Row> {
    let rows: Vec<Row> = sessions.borrow().values().cloned().collect();
    rows.into_iter().find(|r| {
        let r = r.borrow();
        r.view.id == view.id && r.view.workspace_id == view.workspace_id
    })
}

/// The refusal a send to this workspace meets, the name in it where `named` (workspace-state.ts sendRefusal).
fn send_refusal_of(entry: &Entry, named: bool) -> Option<String> {
    let record = entry.borrow().record.borrow().clone();
    let state = workspace_state(&WorkspaceStateInput { phase: record.phase.clone(), machine_state: None, reach: None, wake_refused: None });
    send_refusal(&SendRefusalKind::State(state), record.gone.as_deref(), named.then_some(record.name.as_str()))
}

fn rename_refusal_of(entry: &Entry) -> Option<String> {
    let record = entry.borrow().record.borrow().clone();
    let state = workspace_state(&WorkspaceStateInput { phase: record.phase.clone(), machine_state: None, reach: None, wake_refused: None });
    action_refusal(&state, "rename", record.gone.as_deref(), Some(record.name.as_str()))
}

fn workspace_record(entry: &Entry) -> wsp_core::types::wiring::WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

fn steer_answer(outcome: SessionSteerOutcome) -> SessionSteerResult {
    SessionSteerResult { outcome }
}

fn access_answer(outcome: SessionAccessOutcome) -> SessionAccessResult {
    SessionAccessResult { outcome }
}

fn rename_answer(outcome: SessionRenameOutcome, error: Option<String>) -> SessionRenameResult {
    SessionRenameResult { outcome, error }
}

impl SessionsArea {
    /// sessions.ts:992. Rules C0-C3.
    pub async fn steer_row(&self, row_id: &str, ask: SteerAsk, origin: Option<Caller>) -> Result<SessionSteerResult, HostError> {
        self.ctx.ready().await?;
        let Some(row) = row_by_key(&self.ctx.sessions(), row_id) else {
            return Ok(steer_answer(SessionSteerOutcome::NotFound));
        };
        let view = row.borrow().view.clone();
        let Some(entry) = self.ctx.entry_of_row(&row_at(&view), origin.as_ref(), None, false).await? else {
            return Ok(steer_answer(SessionSteerOutcome::NotFound));
        };
        if let Some(refusal) = send_refusal_of(&entry, true) {
            return Err(HostError::new(refusal));
        }
        let (running, handle) = {
            let r = row.borrow();
            (r.view.status == SessionStatus::Running, r.handle.clone())
        };
        let Some(handle) = handle.filter(|_| running) else {
            return Ok(steer_answer(SessionSteerOutcome::NotRunning));
        };
        let attachments = ask.attachments.clone().unwrap_or_default();
        let records: Vec<AttachmentRecord> = attachments.iter().map(attachment_record).collect();
        if !records.is_empty() {
            let blocked = match files_refusal(&records) {
                Some(refusal) => Some(refusal),
                None => {
                    let built = self.ctx.adapter_for(&entry, AdapterAsk { named: Some(view.harness.clone()), turn_env: None, waiting: None, servers: Env::new(), thread: None, asks_until_stopped: false })?;
                    steer_files_blocked(&records, built.adapter.steers_images(), &view.harness)
                }
            };
            if let Some(blocked) = blocked {
                return Err(HostError::new(blocked));
            }
        }
        let steer_id = self.ctx.ids().uuid();
        let images: Vec<TurnImage> = attachments.iter().map(|a| TurnImage { media_type: a.media_type.clone(), bytes: a.bytes.clone(), path: None }).collect();
        let Some(steer) = handle.steer(ask.prompt.clone(), Some(steer_id.clone()), images) else {
            return Ok(steer_answer(SessionSteerOutcome::Unsupported));
        };
        let outcome = match steer.await? {
            wsp_core::types::wiring::SteerTaken::Accepted => SessionSteerOutcome::Accepted,
            wsp_core::types::wiring::SteerTaken::NotRunning => SessionSteerOutcome::NotRunning,
        };
        if outcome == SessionSteerOutcome::Accepted {
            self.ctx.record_steer(&row, row_id, &ask, origin.as_ref(), &steer_id);
        }
        Ok(steer_answer(outcome))
    }

    /// sessions.ts:1012. Rules C4, C5.
    pub async fn answer_row(&self, row_id: &str, ask: AnswerAsk, origin: Option<Caller>) -> Result<SessionAnswerResult, HostError> {
        self.ctx.ready().await?;
        let sessions = self.ctx.sessions();
        let keyed = row_by_key(&sessions, row_id);
        let keyed_running = keyed.as_ref().is_some_and(|r| r.borrow().view.status == SessionStatus::Running);
        let row = if keyed_running {
            keyed
        } else {
            let running_on_session = {
                let rows: Vec<Row> = sessions.borrow().values().cloned().collect();
                rows.into_iter().find(|r| {
                    let r = r.borrow();
                    r.view.claude_session_id.as_deref() == Some(row_id) && r.view.status == SessionStatus::Running
                })
            };
            running_on_session.or(keyed)
        };
        let Some(row) = row else {
            return Ok(SessionAnswerResult { outcome: SessionAnswerOutcome::NotFound });
        };
        let view = row.borrow().view.clone();
        let Some(entry) = self.ctx.entry_of_row(&row_at(&view), origin.as_ref(), None, false).await? else {
            return Ok(SessionAnswerResult { outcome: SessionAnswerOutcome::NotFound });
        };
        if let Some(refusal) = send_refusal_of(&entry, true) {
            return Err(HostError::new(refusal));
        }
        let handle = row.borrow().handle.clone();
        let Some(handle) = handle else {
            return Ok(SessionAnswerResult { outcome: SessionAnswerOutcome::Gone });
        };
        match handle.answer(ask.ask_id, ask.option_id, ask.reason) {
            Some(answer) => answer.await,
            None => Ok(SessionAnswerResult { outcome: SessionAnswerOutcome::Unsupported }),
        }
    }

    /// sessions.ts:1027. Rules C6-C9.
    pub async fn access_row(&self, id: &str, permission_mode: &str, origin: Option<Caller>) -> Result<SessionAccessResult, HostError> {
        self.ctx.ready().await?;
        let sessions = self.ctx.sessions();
        let found = {
            let by_key = row_by_key(&sessions, id);
            match by_key {
                Some(row) => Some(row),
                None => {
                    let rows: Vec<Row> = sessions.borrow().values().cloned().collect();
                    rows.into_iter().filter(|r| r.borrow().view.thread_id.as_deref() == Some(id)).last()
                }
            }
        };
        let record = match &found {
            Some(_) => None,
            None => {
                let records = self.ctx.records();
                let held: Option<ThreadRecord> = records.borrow().get(id).cloned();
                held
            }
        };
        let (at, harness) = match (&found, &record) {
            (Some(row), _) => {
                let view = row.borrow().view.clone();
                (row_at(&view), view.harness.clone())
            }
            (None, Some(record)) => (RowAt { thread_id: Some(id.to_string()), workspace_id: record.workspace_id.clone() }, record.harness.clone()),
            (None, None) => return Ok(access_answer(SessionAccessOutcome::NotFound)),
        };
        let Some(entry) = self.ctx.entry_of_row(&at, origin.as_ref(), None, false).await? else {
            return Ok(access_answer(SessionAccessOutcome::NotFound));
        };
        if let Some(refusal) = send_refusal_of(&entry, true) {
            return Err(HostError::new(refusal));
        }
        let built = self.ctx.launch_adapter_for(&entry, &harness, None).await?;
        if harness_catalog(&built.harness).is_some() {
            let catalog = self.ctx.catalog_on(&built.harness, &entry, &*built.adapter).await?;
            let picks = StartPicks { permission_mode: Some(permission_mode.to_string()), ..StartPicks::default() };
            start_picks(catalog.as_ref(), &picks, false, None).map_err(|e| {
                let kind = if e.kind == "invalid" { Kind::Invalid } else { Kind::Usage };
                HostError::with_kind(e.message, kind)
            })?;
        }
        let Some(row) = found else {
            let Some(record) = record else {
                return Ok(access_answer(SessionAccessOutcome::NotFound));
            };
            let workspace_id = record.workspace_id.clone();
            let records = self.ctx.records();
            records.borrow_mut().insert(id.to_string(), ThreadRecord { permission_mode: Some(permission_mode.to_string()), ..record });
            let _ = self.ctx.persist_sessions(&workspace_id).await;
            self.ctx.push_head(id);
            return Ok(access_answer(SessionAccessOutcome::Set));
        };
        let view = row.borrow().view.clone();
        let thread_id = view.thread_id.clone();
        let running: Option<Row> = match &thread_id {
            None => (view.status == SessionStatus::Running && row.borrow().handle.is_some()).then(|| row.clone()),
            Some(thread) => self.ctx.running_on(thread),
        };
        let latest_view = match &thread_id {
            None => view.clone(),
            Some(thread) => self.ctx.latest_on(thread).unwrap_or_else(|| view.clone()),
        };
        let latest_row = match &thread_id {
            None => Some(row.clone()),
            Some(_) => row_for_view(&sessions, &latest_view),
        };
        if latest_view.status != SessionStatus::Running {
            self.land_access(&view, thread_id.as_deref(), false, latest_row.as_ref(), permission_mode).await;
            return Ok(access_answer(SessionAccessOutcome::Set));
        }
        let outcome = match running {
            None => SessionAccessOutcome::NotRunning,
            Some(running) => {
                let handle = running.borrow().handle.clone();
                match handle.and_then(|h| h.set_access(permission_mode.to_string())) {
                    None => SessionAccessOutcome::Unsupported,
                    Some(moved) => match moved.await? {
                        wsp_core::types::wiring::AccessMove::Set => SessionAccessOutcome::Set,
                        wsp_core::types::wiring::AccessMove::Refused => SessionAccessOutcome::Unsupported,
                        wsp_core::types::wiring::AccessMove::Gone => SessionAccessOutcome::NotRunning,
                    },
                }
            }
        };
        self.land_access(&view, thread_id.as_deref(), true, latest_row.as_ref(), permission_mode).await;
        Ok(access_answer(match outcome {
            SessionAccessOutcome::Set => SessionAccessOutcome::Set,
            SessionAccessOutcome::Unsupported => SessionAccessOutcome::Unsupported,
            _ => SessionAccessOutcome::NotRunning,
        }))
    }

    /// The pick lands on the thread's record, and on its latest row where that row is not running (sessions.ts:1049-1066).
    async fn land_access(&self, view: &SessionView, thread_id: Option<&str>, latest_running: bool, latest_row: Option<&Row>, permission_mode: &str) {
        if let Some(thread) = thread_id {
            let records = self.ctx.records();
            let base = records.borrow().get(thread).cloned().unwrap_or_else(|| ThreadRecord {
                workspace_id: view.workspace_id.clone(),
                harness: view.harness.clone(),
                ..ThreadRecord::default()
            });
            records.borrow_mut().insert(thread.to_string(), ThreadRecord { permission_mode: Some(permission_mode.to_string()), ..base });
        }
        if !latest_running {
            if let Some(latest) = latest_row {
                latest.borrow_mut().view.permission_mode = Some(permission_mode.to_string());
            }
        }
        let _ = self.ctx.persist_sessions(&view.workspace_id).await;
        self.ctx.push_head(&thread_key_of(view));
    }

    /// sessions.ts:1075. Rules C10-C12.
    pub async fn rename_row(&self, row_id: &str, title: &str, origin: Option<Caller>) -> Result<SessionRenameResult, HostError> {
        self.ctx.ready().await?;
        let named = title.trim().to_string();
        if named.is_empty() {
            return Err(HostError::new(EMPTY_TITLE_LINE));
        }
        let sessions = self.ctx.sessions();
        let Some(row) = row_by_key(&sessions, row_id) else {
            return Ok(rename_answer(SessionRenameOutcome::NotFound, None));
        };
        let view = row.borrow().view.clone();
        let Some(entry) = self.ctx.entry_of_row(&row_at(&view), origin.as_ref(), None, false).await? else {
            return Ok(rename_answer(SessionRenameOutcome::NotFound, None));
        };
        if let Some(refusal) = rename_refusal_of(&entry) {
            return Err(HostError::new(refusal));
        }
        self.ctx.copy_blocked(&entry).await?;
        let built = self.ctx.launch_adapter_for(&entry, &view.harness, None).await?;
        if !built.adapter.renames() {
            return Ok(rename_answer(SessionRenameOutcome::Unsupported, None));
        }
        let Some(session) = view.claude_session_id.clone() else {
            return Ok(rename_answer(SessionRenameOutcome::NoSession, None));
        };
        match self.ctx.write_session(&session, &entry, built.adapter.clone(), &named).await? {
            SessionRenameWrite::Failed { error } => return Ok(rename_answer(SessionRenameOutcome::Failed, Some(error))),
            SessionRenameWrite::NoSession => return Ok(rename_answer(SessionRenameOutcome::NoSession, None)),
            SessionRenameWrite::Written => {}
        }
        let workspace_id = workspace_record(&entry).id;
        let rows: Vec<Row> = sessions.borrow().values().cloned().collect();
        for other in rows {
            let mut other = other.borrow_mut();
            if other.view.workspace_id == workspace_id && other.view.claude_session_id.as_deref() == Some(session.as_str()) {
                other.view.harness_title = Some(named.clone());
                other.view.title_source = Some(TitleSource::Person);
            }
        }
        self.ctx.persist_sessions(&workspace_id).await?;
        self.ctx.push_head(&thread_key_of(&view));
        Ok(rename_answer(SessionRenameOutcome::Renamed, None))
    }

    /// sessions.ts:1108. Rule C13.
    pub async fn read_thread(&self, thread_id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        let now = self.ctx.clock().now();
        let stamps: StampsOf = Rc::new(move |_id: &str| {
            let mut stamp = Map::new();
            stamp.insert("readAt".into(), json!(now));
            stamp
        });
        self.ctx.mark(&[thread_id.to_string()], stamps, origin.as_ref()).await
    }

    /// sessions.ts:1189. Rules C14-C17.
    pub async fn aside_row(&self, row_id: &str, question: &str, origin: Option<Caller>, ask_id: Option<String>) -> Result<SessionAsideResult, HostError> {
        self.ctx.ready().await?;
        if question.trim().is_empty() {
            return Err(HostError::new(BLANK_ASIDE_LINE));
        }
        let not_found = || not_found_refusal(&format!("no session {row_id}"));
        let Some(row) = row_by_key(&self.ctx.sessions(), row_id) else {
            return Err(not_found());
        };
        let view = row.borrow().view.clone();
        let Some(entry) = self.ctx.entry_of_row(&row_at(&view), origin.as_ref(), None, false).await? else {
            return Err(not_found());
        };
        if let Some(refusal) = send_refusal_of(&entry, false) {
            return Err(HostError::new(refusal));
        }
        let latest = match &view.thread_id {
            None => view.clone(),
            Some(thread) => self.ctx.latest_on(thread).unwrap_or_else(|| view.clone()),
        };
        let servers: Env = server_values_of(&self.ctx.vault());
        let launched = self
            .ctx
            .launch_adapter_for(&entry, &latest.harness, Some(ServerValues { entries: None, config: Some(servers.clone()) }))
            .await?;
        let harness = launched.harness.clone();
        let bare = launched.adapter.clone();
        if !bare.asides() {
            return Err(HostError::new(aside_unsupported_line(&harness)));
        }
        let Some(session) = latest.claude_session_id.clone() else {
            return Err(HostError::new(ASIDE_NO_SESSION_LINE));
        };
        let thread_id = thread_key_of(&latest);
        let on_text: Option<Arc<dyn Fn(String)>> = ask_id.map(|ask_id| {
            let ctx = self.ctx.clone();
            let workspace_id = latest.workspace_id.clone();
            let thread = thread_id.clone();
            Arc::new(move |text: String| {
                ctx.pass(EventUnion::AsideText(Sequenced {
                    event: AsideTextEvent { workspace_id: workspace_id.clone(), thread_id: Some(thread.clone()), ask_id: ask_id.clone(), text },
                    seq: None,
                }))
            }) as Arc<dyn Fn(String)>
        });
        let cut = {
            let records = self.ctx.records();
            let cut = records.borrow().get(&thread_id).and_then(|r| r.cut_owed.clone());
            cut
        };
        let mut ask = AsideQuestion {
            session: session.clone(),
            question: question.to_string(),
            cut,
            cwd: latest.cwd.clone(),
            model: latest.model.clone(),
            effort: latest.effort.clone(),
            context_window: latest.context_window.clone(),
            fast: (latest.fast == Some(true)).then_some(true),
            mcp_servers: None,
            server_values: None,
            on_text,
        };
        let record = workspace_record(&entry);
        if (!bare.aside_servers() || !bare.mcp_servers()) && !self.ctx.brokered(&record) {
            return self.aside_answer(&*bare, ask).await;
        }
        let root = self.ctx.root_of(&thread_id);
        let launch = self.ctx.thread_launch(&entry, &thread_id, &root, true).await?;
        let answer: Result<SessionAsideResult, HostError> = async {
            let built = self.ctx.adapter_for(
                &entry,
                AdapterAsk { named: Some(harness.clone()), turn_env: Some(launch.env.clone()), waiting: None, servers: servers.clone(), thread: None, asks_until_stopped: false },
            )?;
            let adapter = built.adapter.clone();
            let served = if adapter.aside_servers() && adapter.mcp_servers() { served_to(launch.wsp.as_ref(), &root, &thread_id) } else { None };
            let aside_in = match &latest.cwd {
                Some(cwd) => Some(cwd.clone()),
                None => self.ctx.folder_of(&latest.workspace_id, &session),
            };
            let server_values = match aside_in {
                Some(folder) if adapter.aside_servers() && !self.ctx.brokered(&record) => self.ctx.server_values_for(&entry, &harness, &folder).await?,
                _ => None,
            };
            ask.mcp_servers = served.map(|spec| {
                let port = wsp_adapters::port::McpServerSpec { command: spec.command, args: spec.args, no_slate: spec.no_slate.map(|_| true) };
                IndexMap::from([(MCP_SERVER_NAME.to_string(), port)])
            });
            ask.server_values = server_values;
            self.aside_answer(&*adapter, ask).await
        }
        .await;
        if let Some(device) = launch.scoped_device_id.clone() {
            if let Err(e) = self.ctx.revoke_device(&device).await {
                console::warn(&format!("the token of a side question on thread {} was not taken away: {}", thread_word(&thread_id), e.message));
            }
        }
        answer
    }

    /// The answer of one aside: the harness's text, which an empty answer refuses (sessions.ts:1199).
    async fn aside_answer(&self, adapter: &dyn wsp_adapters::port::HarnessAdapter, ask: AsideQuestion) -> Result<SessionAsideResult, HostError> {
        let text = self.ctx.aside(adapter, ask).await?;
        if text.trim().is_empty() {
            return Err(HostError::new(ASIDE_EMPTY_LINE));
        }
        Ok(SessionAsideResult { text })
    }

    /// sessions.ts:1232. Rules C18-C20.
    pub async fn run_step(&self, step: RunStep, origin: Option<Caller>) -> Result<SessionRunEvent, HostError> {
        self.ctx.ready().await?;
        if scope_of(origin.as_ref()).is_some() {
            return Err(HostError::new(RUN_PERSONS_LINE));
        }
        let sessions = self.ctx.sessions();
        let rowed = {
            let rows: Vec<Row> = sessions.borrow().values().cloned().collect();
            rows.into_iter().find(|r| r.borrow().view.thread_id.as_deref() == Some(step.thread_id.as_str())).map(|r| r.borrow().view.workspace_id.clone())
        };
        let workspace_id = match rowed {
            Some(workspace_id) => workspace_id,
            None => {
                let records = self.ctx.records();
                let held = records.borrow().get(&step.thread_id).map(|r| r.workspace_id.clone());
                match held {
                    Some(workspace_id) => workspace_id,
                    None => return Err(not_found_refusal(&format!("no thread {}", thread_word(&step.thread_id)))),
                }
            }
        };
        let at = RowAt { thread_id: Some(step.thread_id.clone()), workspace_id: workspace_id.clone() };
        if self.ctx.entry_of_row(&at, origin.as_ref(), None, false).await?.is_none() {
            return Err(not_found_refusal(&format!("no thread {}", thread_word(&step.thread_id))));
        }
        for event in self.ctx.open_thread(&workspace_id, &step.thread_id).await? {
            if let SessionEvent::SessionRunEvent(run) = event {
                if run.run_id == step.run_id && run.state != RunState::Running {
                    return Ok(run);
                }
            }
        }
        let latest = self.ctx.latest_on(&step.thread_id);
        let session_id = latest.as_ref().and_then(|l| l.claude_session_id.clone().or_else(|| Some(l.id.clone()))).unwrap_or_else(|| step.turn_id.clone());
        let event = SessionRunEvent {
            workspace_id,
            session_id,
            at: None,
            turn_id: Some(step.turn_id.clone()),
            thread_id: Some(step.thread_id.clone()),
            pos: None,
            copied_from: None,
            run_id: step.run_id.clone(),
            block: step.block.clone(),
            command: step.command.clone(),
            state: step.state,
            pty_id: step.pty_id.clone(),
            exit_code: step.exit_code,
            signal: step.signal,
            output: step.output.as_deref().map(|output| run_output_tail(output, RUN_OUTPUT_MAX_CHARS as usize)),
        };
        self.ctx.record(SessionEvent::SessionRunEvent(event.clone()));
        let mut answer = event;
        answer.at = Some(JsNumber::from(self.ctx.clock().now()));
        Ok(answer)
    }
}
