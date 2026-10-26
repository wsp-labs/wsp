// SPDX-License-Identifier: AGPL-3.0-only
//! The notifications a turn reads off its server: its items, usage, limits, plan, warnings, errors and completion, and
//! a subagent's own notifications. TS adapter-codex/src/adapter.ts:632-660 and 951-1060. Spec: spec/codex_turn.md
//! rules 4, 8, 9, 10 and 12. Each handler collects what it says, releases the turn's state, and only then calls
//! `on_event`.

use std::sync::Arc;

use serde_json::{Map, Value};

use super::fields::{count, rec, str};
use super::items::{failure_words, held_of, is_reconnecting, item_deltas, item_of, plan_steps, retry_of, total_before, UsageSeen};
use super::limits::USAGE_LIMIT_CODE;
use super::turn::{close_ask, foreign_thread, limit_event, note, open_steer, say_compacted, Compaction, LeadEnd, State, Turn};
use crate::pending::CODEX_LEGACY_HISTORY;
use crate::port::{AdapterEvent, PermissionOutcome};
use wsp_wire::types::js_number::JsNumber;

impl Turn {
    /// Every notification the server prints: progress first, then a subagent's own, then the lead's. TS adapter.ts:951.
    pub(super) fn on_notification(self: &Arc<Self>, method: &str, params: &Map<String, Value>, emitted: Option<f64>) {
        if method != "error" {
            Turn::progress(&mut self.st.borrow_mut());
        }
        if method != "serverRequest/resolved" {
            let task = foreign_thread(&self.st.borrow(), params);
            if let Some(task) = task {
                self.on_child(&task, method, params, emitted);
                return;
            }
        }
        match method {
            "thread/started" => self.thread_started(params),
            "turn/started" => self.turn_started(params),
            "item/started" => self.lead_item(false, params),
            "item/completed" => self.lead_item(true, params),
            "thread/tokenUsage/updated" => self.lead_usage(params, emitted),
            "account/rateLimits/updated" => self.rates_updated(params),
            "turn/plan/updated" => self.plan_updated(params),
            "configWarning" | "warning" => self.warning(params),
            "serverRequest/resolved" => self.request_resolved(params),
            "error" => self.lead_error(params),
            "turn/completed" => self.lead_completed(params),
            _ => {}
        }
    }

    /// The lead's thread, announced by its own notification. TS adapter.ts:952.
    fn thread_started(self: &Arc<Self>, params: &Map<String, Value>) {
        let mut out = Vec::new();
        {
            let mut st = self.st.borrow_mut();
            let thread = rec(params.get("thread"));
            let legacy = !st.announced && thread.and_then(|t| str(t.get("historyMode"))) == Some("legacy");
            st.legacy |= legacy;
            let id = str(thread.and_then(|t| t.get("id"))).map(String::from);
            let model = str(thread.and_then(|t| t.get("model"))).map(String::from);
            let cwd = str(thread.and_then(|t| t.get("cwd"))).map(String::from);
            self.announce(&mut st, id, model, cwd, &mut out);
        }
        self.emit_all(out);
    }

    /// The first turn that names an id opens steering and anchors the thread's history. TS adapter.ts:960.
    fn turn_started(self: &Arc<Self>, params: &Map<String, Value>) {
        let mut out = Vec::new();
        {
            let mut st = self.st.borrow_mut();
            let first = st.turn_id.is_none();
            if let Some(id) = str(rec(params.get("turn")).and_then(|t| t.get("id"))) {
                st.turn_id = Some(id.to_string());
            }
            if let (true, Some(turn)) = (first, st.turn_id.clone()) {
                open_steer(&mut st);
                if self.prompts_later && st.announced {
                    out.push(AdapterEvent::SessionStart {
                        session_id: st.thread_id.clone(),
                        model: st.model_used.clone(),
                        cwd: st.cwd_used.clone(),
                        tools: None,
                        harness: None,
                        prompted: None,
                    });
                }
                let kept = st.legacy.then(|| CODEX_LEGACY_HISTORY.to_string());
                out.push(AdapterEvent::TurnAnchor { session_id: st.thread_id.clone(), anchor: turn, kept });
            }
        }
        self.emit_all(out);
    }

    /// A command or file change the turn's lead ran or wrote, its deltas, and any subagent it signs. TS adapter.ts:968.
    fn lead_item(self: &Arc<Self>, done: bool, params: &Map<String, Value>) {
        let Some(item) = item_of(params) else { return };
        let mut out = Vec::new();
        let (ended, deltas) = {
            let mut st = self.st.borrow_mut();
            let thread = st.thread_id.clone();
            let deltas = item_deltas(done, &item, &thread);
            self.count_item(&mut st, &item, done);
            if done && str(item.get("type")) == Some("agentMessage") {
                st.last_text = str(item.get("text")).map(String::from);
            }
            if str(item.get("type")) == Some("contextCompaction") {
                if st.compaction.is_none() {
                    let before = st.usage.as_ref().map(|u| held_of(&u.last));
                    st.compaction = Some(Compaction { before, after: None, ended: false });
                }
                if done {
                    if let Some(c) = st.compaction.as_mut() {
                        c.ended = true;
                    }
                    if st.compaction.as_ref().is_some_and(|c| c.after.is_some()) {
                        say_compacted(&mut st, &mut out);
                    }
                }
            }
            let ended = st.children.child_signs(&thread, &item, done, &mut out);
            (ended, deltas)
        };
        self.emit_all(out);
        if ended {
            self.settle_lead();
        }
        self.emit_all(deltas);
    }

    /// Counts a command the turn or its background runs, and keeps a file change's changes by id. TS adapter.ts:644.
    pub(super) fn count_item(&self, st: &mut State, item: &Map<String, Value>, done: bool) {
        let id = str(item.get("id")).unwrap_or("").to_string();
        match str(item.get("type")) {
            Some("commandExecution") => {
                if done {
                    st.commands.shift_remove(&id);
                } else {
                    st.commands.insert(id.clone());
                }
                if let Some(bg) = &self.background {
                    let mut bg = bg.lock().unwrap();
                    if done {
                        bg.commands.shift_remove(&id);
                    } else {
                        bg.commands.insert(id.clone());
                    }
                }
            }
            Some("fileChange") => {
                st.changes_by_id.insert(id, item.get("changes").cloned().unwrap_or(Value::Null));
            }
            _ => {}
        }
    }

    /// The running total and the latest call's usage. TS adapter.ts:1036.
    fn lead_usage(self: &Arc<Self>, params: &Map<String, Value>, emitted: Option<f64>) {
        let reported = rec(params.get("tokenUsage"));
        let Some(last) = rec(reported.and_then(|r| r.get("last"))) else { return };
        let total = rec(reported.and_then(|r| r.get("total"))).cloned().unwrap_or_default();
        let window = count(reported.and_then(|r| r.get("modelContextWindow")));
        let mut out = Vec::new();
        {
            let mut st = self.st.borrow_mut();
            let tokens = count(last.get("inputTokens")).unwrap_or(0.0) + count(last.get("outputTokens")).unwrap_or(0.0);
            let context = Some(held_of(last));
            out.push(AdapterEvent::TurnUsage { session_id: st.thread_id.clone(), tokens, context, window, at: emitted });
            let before = st.usage.take().map(|u| u.before).unwrap_or_else(|| total_before(&total, last));
            st.usage = Some(UsageSeen { before, total: total.clone(), last: last.clone(), window });
            let mut said = false;
            if let Some(c) = st.compaction.as_mut() {
                if c.after.is_none() {
                    c.after = Some(held_of(last));
                    said = c.ended;
                }
            }
            if said {
                say_compacted(&mut st, &mut out);
            }
        }
        self.emit_all(out);
    }

    /// A rolling update of the plan's windows: what it leaves out or names null keeps the last reading. TS adapter.ts:1046.
    fn rates_updated(self: &Arc<Self>, params: &Map<String, Value>) {
        let snapshot = rec(params.get("rateLimits")).cloned().unwrap_or_default();
        if str(snapshot.get("limitId")).is_some_and(|id| id != "codex") {
            return;
        }
        let mut out = Vec::new();
        {
            let mut st = self.st.borrow_mut();
            let mut rates = st.rate_limits.clone().unwrap_or_default();
            for (key, value) in snapshot {
                if !value.is_null() {
                    rates.insert(key, value);
                }
            }
            st.rate_limits = Some(rates);
            out.extend(limit_event(&st, None));
        }
        self.emit_all(out);
    }

    /// The plan the server keeps as the turn's steps. TS adapter.ts:1056.
    fn plan_updated(self: &Arc<Self>, params: &Map<String, Value>) {
        let session_id = self.st.borrow().thread_id.clone();
        let steps = plan_steps(params.get("plan"));
        self.emit_all(vec![AdapterEvent::TurnPlan { session_id, steps: Some(steps), text: None }]);
    }

    /// A warning the server prints, as a note. TS adapter.ts:1063.
    fn warning(self: &Arc<Self>, params: &Map<String, Value>) {
        let Some(text) = str(params.get("summary")).or(str(params.get("message"))) else { return };
        let mut out = Vec::new();
        note(&mut self.st.borrow_mut(), text.to_string(), &mut out);
        self.emit_all(out);
    }

    /// The server closed a request it had raised: its open ask closes as cancelled. TS adapter.ts:1066.
    fn request_resolved(self: &Arc<Self>, params: &Map<String, Value>) {
        let ask_id = match params.get("requestId") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Number(n)) => n.as_f64().map(|n| JsNumber(n).js_string()),
            Some(Value::Null) => Some("null".to_string()),
            Some(Value::Bool(b)) => Some(b.to_string()),
            _ => None,
        };
        let mut out = Vec::new();
        if let Some(ask_id) = ask_id {
            let mut st = self.st.borrow_mut();
            if st.pending.contains_key(&ask_id) {
                out.push(close_ask(&mut st, &ask_id, PermissionOutcome::Cancelled, None));
            }
        }
        self.emit_all(out);
    }

    /// An error the server printed: its words, the usage limit it raised, a stall, and a retry it will make. TS adapter.ts:1071.
    fn lead_error(self: &Arc<Self>, params: &Map<String, Value>) {
        let error = rec(params.get("error"));
        let message = str(error.and_then(|e| e.get("message"))).unwrap_or("").to_string();
        let info = error.and_then(|e| e.get("codexErrorInfo"));
        let details = str(error.and_then(|e| e.get("additionalDetails"))).unwrap_or("");
        let will_retry = params.get("willRetry") == Some(&Value::Bool(true));
        let mut out = Vec::new();
        {
            let mut st = self.st.borrow_mut();
            st.last_error = Some(message.clone());
            if str(info) == Some(USAGE_LIMIT_CODE) && !will_retry {
                st.usage_limited = true;
            }
            if let Some(words) = failure_words(&format!("{message} {details}"), &self.ctx.login, self.ctx.key_env.as_deref()) {
                st.words = Some(words);
            }
            if is_reconnecting(&message) {
                self.reconnecting(&mut st);
            }
            if will_retry {
                if let Some(retry) = retry_of(&message, info, &st.thread_id) {
                    out.push(retry);
                }
            }
        }
        self.emit_all(out);
    }

    /// The lead's turn ended: its end is held until nothing it spawned still runs. TS adapter.ts:1090.
    fn lead_completed(self: &Arc<Self>, params: &Map<String, Value>) {
        let turn = rec(params.get("turn"));
        let id = str(turn.and_then(|t| t.get("id")));
        let error_obj = rec(turn.and_then(|t| t.get("error")));
        let mut out = Vec::new();
        {
            let mut st = self.st.borrow_mut();
            if let (Some(current), Some(id)) = (st.turn_id.as_deref(), id) {
                if id != current {
                    return;
                }
            }
            let error = str(error_obj.and_then(|e| e.get("message"))).map(String::from);
            if str(error_obj.and_then(|e| e.get("codexErrorInfo"))) == Some(USAGE_LIMIT_CODE) {
                st.usage_limited = true;
            }
            let status = str(turn.and_then(|t| t.get("status"))).unwrap_or("failed").to_string();
            st.lead_end = Some(LeadEnd { status, error });
            say_compacted(&mut st, &mut out);
        }
        self.emit_all(out);
        self.settle_lead();
    }

    /// A subagent's own notification, read against its child. TS adapter.ts:791 (`onChild`).
    fn on_child(self: &Arc<Self>, task: &str, method: &str, params: &Map<String, Value>, emitted: Option<f64>) {
        let mut out = Vec::new();
        let ended = {
            let mut st = self.st.borrow_mut();
            if matches!(method, "item/started" | "item/completed") {
                if let Some(item) = item_of(params) {
                    self.count_item(&mut st, &item, method == "item/completed");
                }
            }
            let lead = st.thread_id.clone();
            st.children.on_child(&lead, task, method, params, emitted, &mut out)
        };
        self.emit_all(out);
        if ended {
            self.settle_lead();
        }
    }
}
