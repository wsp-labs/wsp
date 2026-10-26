// SPDX-License-Identifier: AGPL-3.0-only
//! turns.ts:291-569: a turn's adapter events turned into rows. Spec: spec/turns_forward.md.
use std::rc::Rc;

use futures::FutureExt;
use indexmap::IndexMap;
use wsp_adapters::port::{AdapterEvent, DeltaKind, TurnDelta};
use wsp_core::types::internal::{write_lines, CallSeen, ToldAs, TurnLive};
use wsp_frames::True;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::usage::{HarnessLimit, UsageAccountEvent, UsageTokens};
use wsp_wire::views::session_events::{
    PlanStep, SessionDeltaEvent, SessionDoneEvent, SessionEndEvent, SessionHarness, SessionStartEvent, SlateLit, TurnResult, TurnStatus,
    TurnTokens,
};
use wsp_wire::views::session_events_more::SessionSubagentEvent;
use wsp_wire::views::session_events_more::{SessionCompactedEvent, SessionContextEvent, SessionEvent, SessionPlanEvent, SessionRetryEvent};
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_words::exit::HostError;
use wsp_words::words::thread::still_running_line;
use wsp_words::words::tools::tool_call_facts;
use wsp_words::words::turn::{notify_body, turn_lines, NotifyLength};

use super::run::{detach, session_status_of, TurnState};
use super::{BurnAsk, LedgerUse, LimitAsk};
use crate::threads::threads::Row;

/// One row of what a turn used: a model's share of its tokens and cost, or the whole turn where no model names one.
struct Use {
    model: Option<String>,
    tokens: Option<TurnTokens>,
    cost: Option<f64>,
}

/// A model's five counts as the ledger takes them, for a turn that names each model it used.
fn tokens_of(tokens: &UsageTokens) -> TurnTokens {
    TurnTokens {
        input: tokens.input,
        output: tokens.output,
        cached: Some(tokens.cached),
        cache_write: Some(tokens.cache_write),
        reasoning: Some(tokens.reasoning),
        context: None,
        window: None,
    }
}

impl TurnState {
    /// turns.ts:291. Rules F1-F18.
    pub fn forward(self: &Rc<Self>, event: AdapterEvent) {
        match event {
            AdapterEvent::TurnUnread { ids, .. } => {
                *self.unread.borrow_mut() = ids;
            }
            other => {
                if self.ended.get() {
                    return;
                }
                self.dispatch(other);
            }
        }
    }

    fn dispatch(self: &Rc<Self>, event: AdapterEvent) {
        let session_id = event.session_id().to_string();
        match event {
            AdapterEvent::SessionStart { model, cwd, tools, harness, prompted, .. } => {
                self.on_start(&session_id, model, cwd, tools, harness, prompted)
            }
            AdapterEvent::TurnDelta(delta) => self.on_delta(&session_id, delta),
            AdapterEvent::TurnDone { result, held, .. } => self.on_done(&session_id, result, held == Some(true)),
            AdapterEvent::TurnAnchor { anchor, kept, .. } => {
                *self.anchor.borrow_mut() = Some(anchor);
                *self.kept_why.borrow_mut() = kept;
            }
            AdapterEvent::Subagent { task, state, parent_tool_use_id, title, summary, depth, model, asked, usage, .. } => {
                if self.replay(|c| &mut c.replaying_subagents) {
                    return;
                }
                let line = self.count(|c| &mut c.subagents);
                self.ctx.record(SessionEvent::SessionSubagentEvent(SessionSubagentEvent {
                    workspace_id: self.workspace_id.clone(),
                    session_id,
                    at: None,
                    turn_id: Some(self.turn_id.clone()),
                    thread_id: Some(self.thread_id.clone()),
                    pos: None,
                    copied_from: None,
                    task,
                    state,
                    parent_tool_use_id,
                    title,
                    summary,
                    depth: depth.map(JsInt),
                    model,
                    asked,
                    usage,
                    line: Some(JsInt(JsNumber(line as f64))),
                }));
            }
            AdapterEvent::Limit { limit, .. } => self.on_limit(limit),
            AdapterEvent::TurnUsage { tokens, context, window, at, .. } => self.on_usage(&session_id, tokens, context, window, at),
            AdapterEvent::TurnCompacted { before, after, .. } => {
                if self.replay(|c| &mut c.replaying_compactions) {
                    return;
                }
                let line = self.count(|c| &mut c.compactions);
                self.ctx.record(SessionEvent::SessionCompactedEvent(SessionCompactedEvent {
                    workspace_id: self.workspace_id.clone(),
                    session_id,
                    at: None,
                    turn_id: Some(self.turn_id.clone()),
                    thread_id: Some(self.thread_id.clone()),
                    pos: None,
                    copied_from: None,
                    before: before.map(JsNumber),
                    after: after.map(JsNumber),
                    line: Some(JsInt(JsNumber(line as f64))),
                }));
            }
            AdapterEvent::TurnRetry { attempt, of, why, .. } => {
                if self.replay(|c| &mut c.replaying_retries) {
                    return;
                }
                self.ctx.agent_retried(&self.turn_id, why.as_deref());
                let line = self.count(|c| &mut c.retries);
                self.ctx.record(SessionEvent::SessionRetryEvent(SessionRetryEvent {
                    workspace_id: self.workspace_id.clone(),
                    session_id,
                    at: None,
                    turn_id: Some(self.turn_id.clone()),
                    thread_id: Some(self.thread_id.clone()),
                    pos: None,
                    copied_from: None,
                    attempt: JsInt(JsNumber(attempt)),
                    of: of.map(|n| JsInt(JsNumber(n))),
                    why,
                    line: Some(JsInt(JsNumber(line as f64))),
                }));
            }
            AdapterEvent::TurnPlan { steps, text, .. } => {
                self.ctx.record(SessionEvent::SessionPlanEvent(SessionPlanEvent {
                    workspace_id: self.workspace_id.clone(),
                    session_id,
                    at: None,
                    turn_id: Some(self.turn_id.clone()),
                    thread_id: Some(self.thread_id.clone()),
                    pos: None,
                    copied_from: None,
                    steps: steps.map(|steps: Vec<PlanStep>| steps),
                    text,
                }));
            }
            AdapterEvent::TurnTasks { running, replied, .. } => self.on_tasks(running, replied),
            AdapterEvent::PermissionAsk { ask, .. } => {
                let ask = self.named(&ask);
                self.open.borrow_mut().insert(ask.ask_id.clone(), ask.clone());
                self.reads_open();
                let event = self.permission_event(&session_id, &ask);
                self.ctx.record(SessionEvent::SessionPermissionEvent(event));
                let view = self.row.borrow().view.clone();
                self.ctx.end_snooze_for(&view);
            }
            AdapterEvent::PermissionClose { ask_id, outcome, option_id, .. } => self.close_ask(&ask_id, outcome, option_id.as_deref()),
            AdapterEvent::SessionEnd { exit_code, saw_result, failure, .. } => self.on_end(&session_id, exit_code, saw_result, failure),
            AdapterEvent::TurnUnread { .. } => {}
        }
    }

    /// Whether a replayed row is skipped: a count above zero is spent and the row writes nothing.
    fn replay(&self, pick: impl Fn(&mut super::run::Counts) -> &mut u64) -> bool {
        let mut counts = self.counts.borrow_mut();
        let left = pick(&mut counts);
        if *left > 0 {
            *left -= 1;
            return true;
        }
        false
    }

    /// The next number of a counted row kind.
    fn count(&self, pick: impl Fn(&mut super::run::Counts) -> &mut u64) -> u64 {
        let mut counts = self.counts.borrow_mut();
        let n = pick(&mut counts);
        *n += 1;
        *n
    }

    fn on_start(
        self: &Rc<Self>,
        session_id: &str,
        model: Option<String>,
        cwd: Option<String>,
        tools: Option<Vec<String>>,
        harness: Option<SessionHarness>,
        prompted: Option<bool>,
    ) {
        let start_recorded = self.counts.borrow().start_recorded;
        {
            let mut row = self.row.borrow_mut();
            row.view.claude_session_id = Some(session_id.to_string());
            if let Some(cwd) = &cwd {
                row.view.cwd = Some(cwd.clone());
            }
            if let Some(model) = &model {
                row.view.model = Some(model.clone());
            }
            if let Some(asked) = row.asked.as_mut() {
                if prompted == Some(false) && !start_recorded {
                    asked.awaits_prompt = Some(true);
                } else if prompted != Some(false) {
                    asked.awaits_prompt = None;
                }
            }
        }
        let record = {
            let entry = self.entry.borrow();
            let mut record = entry.record.borrow_mut();
            record.claude_session_id = Some(session_id.to_string());
            record.clone()
        };
        let ctx = self.ctx.clone();
        detach(
            async move {
                let _ = ctx.persist_record(record).await;
            }
            .boxed_local(),
        );
        self.persist();
        if prompted != Some(false) {
            if let Some(request) = &self.opening.request_id {
                self.ctx.prompt_held(request);
            }
        }
        if let Some(title) = &self.opening.title {
            if !start_recorded {
                let view = self.row.borrow().view.clone();
                let ctx = self.ctx.clone();
                let title = title.clone();
                detach(
                    async move {
                        let _ = ctx.name_in_harness(view, &title).await;
                    }
                    .boxed_local(),
                );
            }
        }
        if start_recorded {
            return;
        }
        self.counts.borrow_mut().start_recorded = true;
        let view = self.row.borrow().view.clone();
        let opening = &self.opening;
        self.ctx.record(SessionEvent::SessionStartEvent(SessionStartEvent {
            workspace_id: self.workspace_id.clone(),
            session_id: session_id.to_string(),
            at: None,
            turn_id: Some(self.turn_id.clone()),
            thread_id: Some(self.thread_id.clone()),
            pos: None,
            copied_from: None,
            prompt: Some(opening.prompt.clone()),
            attachments: opening.attachments.clone(),
            request_id: opening.request_id.clone(),
            via: opening.via_slate.then_some(SlateLit),
            after_cut: opening.after_cut.then_some(True),
            after_limit: opening.after_limit.map(JsNumber),
            opens_thread: opening.opens_thread.then_some(True),
            started_by: view.started_by,
            model,
            cwd,
            permission_mode: view.permission_mode.clone(),
            effort: view.effort.clone(),
            agent: Some(view.harness.clone()),
            tools,
            harness,
        }));
        if self.ctx.source_of(&view).as_deref() == Some("seed") {
            let state = self.clone();
            let ctx = self.ctx.clone();
            let row: Row = self.row.clone();
            let (workspace_id, thread_id) = (self.workspace_id.clone(), self.thread_id.clone());
            ctx.hold_ask(
                async move {
                    let made: Result<(), HostError> = async {
                        state.ctx.refresh_title(&row, false).await?;
                        state.ctx.make_title(&row).await
                    }
                    .await;
                    if let Err(e) = made {
                        wsp_core::console::warn(&wsp_core::types::internal::no_made_title_log_line(&thread_id, &workspace_id, &e.message));
                    }
                }
                .boxed_local(),
            );
        }
    }

    fn on_delta(&self, session_id: &str, delta: TurnDelta) {
        let TurnDelta {
            kind,
            text,
            message_id,
            tool_name,
            tool_use_id,
            is_error,
            parent_tool_use_id,
            cwd,
            bytes,
            exit_code,
            duration_ms,
            patch,
            ..
        } = delta;
        if kind == DeltaKind::ToolUse {
            if let (Some(id), Some(name)) = (&tool_use_id, &tool_name) {
                self.row
                    .borrow_mut()
                    .calls
                    .get_or_insert_with(IndexMap::new)
                    .insert(id.clone(), CallSeen { tool_name: name.clone(), input: text.clone() });
                if let Some(wrote) = &self.wrote {
                    for path in tool_call_facts(name, &text).changed_files.unwrap_or_default() {
                        wrote.borrow_mut().insert(path);
                    }
                }
            }
        } else if kind == DeltaKind::ToolResult {
            if let (Some(id), Some(calls)) = (&tool_use_id, self.row.borrow_mut().calls.as_mut()) {
                calls.shift_remove(id);
            }
        }
        if self.replay(|c| &mut c.replaying_lines) {
            return;
        }
        let line = self.count(|c| &mut c.lines);
        self.ctx.record(SessionEvent::SessionDeltaEvent(SessionDeltaEvent {
            workspace_id: self.workspace_id.clone(),
            session_id: session_id.to_string(),
            at: None,
            turn_id: Some(self.turn_id.clone()),
            thread_id: Some(self.thread_id.clone()),
            pos: None,
            copied_from: None,
            kind,
            text,
            message_id,
            tool_name,
            tool_use_id,
            is_error,
            parent_tool_use_id,
            cwd,
            line: Some(JsInt(JsNumber(line as f64))),
            cut: None,
            bytes: bytes.map(|b| JsInt(JsNumber(b))),
            exit_code: exit_code.map(|c| JsInt(JsNumber(c as f64))),
            duration_ms: duration_ms.map(JsNumber),
            patch,
            patch_cut: None,
        }));
    }

    fn on_done(self: &Rc<Self>, session_id: &str, result: TurnResult, held: bool) {
        if result.tokens.as_ref().is_some_and(|t| t.context.is_some()) {
            self.result_held.set(true);
        }
        let recorded = self.counts.borrow_mut().reply_recorded.take();
        if let Some(recorded) = recorded {
            self.row.borrow_mut().turn_live.get_or_insert_with(TurnLive::default).reply.get_or_insert(recorded);
            return;
        }
        let status = result.status;
        self.row.borrow_mut().turn_live.get_or_insert_with(TurnLive::default).reply = Some(status);
        let result = self.with_waited(result);
        {
            let lines = turn_lines(&result, None);
            let mut row = self.row.borrow_mut();
            write_lines(&mut row.view, lines.last_line, lines.failure);
            if result.refusal.is_some() {
                row.view.refusal = result.refusal;
            }
            if result.limit.is_some() {
                row.view.limit = result.limit.clone();
            }
            if let Some(cost) = result.cost_usd {
                row.view.cost_usd = Some(JsNumber(row.view.cost_usd.map_or(0.0, |c| c.0) + cost.0));
            }
        }
        if result.tokens.is_some() || result.cost_usd.is_some() {
            self.file_use(&result, session_id);
        }
        self.persist();
        let told = self.after_told(&result, held);
        if let (Some(notify), Some(told)) = (&self.notify, told) {
            if !self.line_try() {
                self.ctx.notify_end(&self.row, notify, told, false);
            }
        }
        self.ctx.record(SessionEvent::SessionDoneEvent(SessionDoneEvent {
            workspace_id: self.workspace_id.clone(),
            session_id: session_id.to_string(),
            at: None,
            turn_id: Some(self.turn_id.clone()),
            thread_id: Some(self.thread_id.clone()),
            pos: None,
            copied_from: None,
            result,
        }));
        self.read_changes(session_id);
    }

    /// turns.ts:409-433: what the turn used, filed under the sign-in it ran on.
    fn file_use(&self, result: &TurnResult, session_id: &str) {
        let harness = self.row.borrow().view.harness.clone();
        let account = self.ctx.usage_account_of(&self.entry, &harness, self.turn_account.borrow().clone());
        let view_model = self.row.borrow().view.model.clone();
        let model = result.model.clone().or(view_model);
        let uses: Vec<Use> = match &result.models {
            Some(models) if !models.is_empty() => models
                .iter()
                .map(|m| Use { model: Some(m.model.clone()), tokens: Some(tokens_of(&m.tokens)), cost: m.cost_usd.map(|c| c.0) })
                .collect(),
            _ => vec![Use { model: model.clone(), tokens: result.tokens.clone(), cost: result.cost_usd.map(|c| c.0) }],
        };
        let counted = uses.iter().position(|u| u.model == model).unwrap_or(0);
        let record = self.entry.borrow().record.borrow().clone();
        let computer = self.ctx.usage_computer_of(&record);
        let at = self.ctx.clock().now();
        let session = self.row.borrow().view.claude_session_id.clone().unwrap_or_else(|| session_id.to_string());
        for (index, u) in uses.into_iter().enumerate() {
            let row = LedgerUse {
                at,
                agent: harness.clone(),
                account: account.key.clone(),
                account_label: account.label.clone(),
                computer: computer.clone(),
                project: record.project.clone(),
                turns: if index == counted { 1 } else { 0 },
                model: u.model,
                tokens: u.tokens,
                cost_usd: u.cost,
                session: Some(session.clone()),
                source: "wsp".to_string(),
            };
            let ctx = self.ctx.clone();
            let turn = self.turn_id.clone();
            detach(
                async move {
                    if let Err(e) = ctx.ledger_add(row).await {
                        wsp_core::console::warn(&format!("the use of turn {turn} was not filed: {}", e.message));
                    }
                }
                .boxed_local(),
            );
        }
    }

    /// turns.ts:471-483: a usage limit the harness reported, kept and pushed to the Usage page.
    fn on_limit(&self, limit: HarnessLimit) {
        if let Some(account) = &limit.account {
            *self.turn_account.borrow_mut() = Some((account.id.clone(), account.label.clone()));
        }
        let harness = self.row.borrow().view.harness.clone();
        let account = self.ctx.usage_account_of(&self.entry, &harness, self.turn_account.borrow().clone());
        let record = self.entry.borrow().record.borrow().clone();
        let ask = LimitAsk {
            key: account.key.clone(),
            agent: harness,
            label: account.label.clone(),
            road: account.road.clone(),
            computer: self.ctx.usage_computer_of(&record),
            limit,
        };
        let ctx = self.ctx.clone();
        let key = account.key;
        detach(
            async move {
                let kept: Result<(), HostError> = async {
                    let (before, after) = ctx.ledger_limit(ask).await?;
                    ctx.alerts_read(before, after).await?;
                    let answer = ctx.usage_accounts().await?;
                    if let Some(row) = answer.accounts.into_iter().find(|a| a.key == key) {
                        ctx.emit(EventUnion::UsageAccount(Sequenced { event: UsageAccountEvent { key: key.clone(), row }, seq: None }));
                    }
                    Ok(())
                }
                .await;
                if let Err(e) = kept {
                    wsp_core::console::warn(&format!("the limits of {key} were not kept: {}", e.message));
                }
            }
            .boxed_local(),
        );
    }

    /// turns.ts:485-492: burns the usage; the context passes on the bus and is not recorded.
    fn on_usage(&self, session_id: &str, tokens: f64, context: Option<f64>, window: Option<f64>, at: Option<f64>) {
        let harness = self.row.borrow().view.harness.clone();
        let account = self.ctx.usage_account_of(&self.entry, &harness, self.turn_account.borrow().clone()).key;
        let replayed = match at {
            Some(at) if self.reopened => Some((self.turn_id.clone(), at)),
            _ => None,
        };
        self.ctx.burn_add(BurnAsk { account, thread_id: self.thread_id.clone(), tokens, replayed });
        if let Some(context) = context {
            *self.held.borrow_mut() = Some((context, window));
            self.ctx.pass(EventUnion::SessionContext(Sequenced {
                event: SessionContextEvent {
                    workspace_id: self.workspace_id.clone(),
                    session_id: session_id.to_string(),
                    at: Some(JsNumber(self.ctx.clock().now() as f64)),
                    turn_id: Some(self.turn_id.clone()),
                    thread_id: Some(self.thread_id.clone()),
                    pos: None,
                    copied_from: None,
                    context: JsNumber(context),
                    window: window.map(JsNumber),
                },
                seq: None,
            }));
        }
    }

    /// turns.ts:522-531: the tasks the turn started are running; a reply given over them is told once and says so.
    fn on_tasks(self: &Rc<Self>, running: u32, replied: Option<TurnResult>) {
        self.tasks_running.set(running > 0);
        self.reads_waiting();
        let Some(replied) = replied else {
            return;
        };
        let reply = self.with_waited(replied);
        let said = reply.text.clone().unwrap_or_default();
        let promised = reply.status == TurnStatus::Completed && running > 0;
        let replied = if promised {
            let text = [said.as_str(), still_running_line(running as f64).as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            TurnResult { text: Some(text), ..reply.clone() }
        } else {
            reply.clone()
        };
        let told = ToldAs { status: replied.status, body: notify_body(&reply, Some(NotifyLength::Whole)), said: Some(said), promised };
        self.row.borrow_mut().turn_live.get_or_insert_with(TurnLive::default).told_as = Some(told);
        if !self.replay(|c| &mut c.replaying_told) {
            if let Some(notify) = &self.notify {
                self.ctx.notify_end(&self.row, notify, replied, promised);
                let mut row = self.row.borrow_mut();
                let live = row.turn_live.get_or_insert_with(TurnLive::default);
                live.told = Some(live.told.unwrap_or(0) + 1);
            }
        }
        self.persist();
    }

    /// turns.ts:546-567: the exit closes what is still open, settles the row, then writes the end.
    fn on_end(
        self: &Rc<Self>,
        session_id: &str,
        exit_code: Option<i32>,
        saw_result: bool,
        failure: Option<wsp_adapters::port::ExecFailure>,
    ) {
        if let Some(failure) = failure {
            self.row.borrow_mut().turn_live.get_or_insert_with(TurnLive::default).failure = Some(HostError::new(failure.message));
        }
        self.close_open_asks();
        if let Some(reply) = self.live_reply() {
            self.row.borrow_mut().view.status = session_status_of(reply);
        }
        let now = self.ctx.clock().now() as f64;
        let promptless = self.row.borrow().asked.as_ref().is_some_and(|asked| asked.awaits_prompt == Some(true));
        self.row.borrow_mut().view.ended_at = Some(JsNumber(now));
        self.ctx.record(SessionEvent::SessionEndEvent(SessionEndEvent {
            workspace_id: self.workspace_id.clone(),
            session_id: session_id.to_string(),
            at: None,
            turn_id: Some(self.turn_id.clone()),
            thread_id: Some(self.thread_id.clone()),
            pos: None,
            copied_from: None,
            exit_code: exit_code.map(|c| JsNumber(c as f64)),
            saw_result,
            reason: None,
            unstarted: None,
            prompt: None,
            promptless: promptless.then_some(True),
        }));
        self.turn_over();
        self.read_changes(session_id);
    }
}
