// SPDX-License-Identifier: AGPL-3.0-only
//! boot.ts:673-1013 at 65f6b89c7: listings, thread facts, the row push and the marks. Spec: spec/boot_rows.md.

use std::rc::Rc;

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};

use wsp_core::types::internal::{ThreadRecord, ThreadStamps};
use wsp_wire::views::preferences::ThreadFacts;
use wsp_wire::views::session::{ForkedFrom, SessionView};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::{not_found_refusal, refusal, HostError, Kind};
use wsp_words::words::computer::thread_word;

use super::entries::RowAt;
use super::{BootArea, ComputerRow};
use crate::machines::machines::{Entry, Row};

/// What a mark puts on each thread: the same stamps, or stamps per thread (boot.ts:923). A key whose value is JSON
/// `null` stands for the TS `undefined`: it is taken off the record.
pub enum Stamped<'a> {
    Same(ThreadStamps),
    Each(&'a dyn Fn(&str) -> ThreadStamps),
}

/// A JSON object's value of a key, or none for a key the object does not hold.
fn field<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.get(key).filter(|x| !x.is_null())
}

fn text_of(v: &Value, key: &str) -> Option<String> {
    field(v, key).and_then(Value::as_str).map(str::to_string)
}

fn number_of(v: &Value, key: &str) -> Option<f64> {
    field(v, key).and_then(Value::as_f64)
}

/// A value read back into the type it is, or the host error its read gave.
fn parse<T: DeserializeOwned>(v: Value) -> Result<T, HostError> {
    serde_json::from_value(v).map_err(|e| HostError::new(e.to_string()))
}

/// A value as the JSON object it is, or an empty one.
fn object(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// The fold a listing answers a thread by, as a thread view's JSON (wsp-words `foldThreads`).
fn folded_thread(rows: &[SessionView]) -> Option<Value> {
    wsp_words::views::session::fold_threads(rows)
        .into_iter()
        .next()
        .and_then(|t| serde_json::to_value(t).ok())
}

impl BootArea {
    /// The rows the host holds now, cloned out of the map so no borrow is held across a call.
    pub(crate) fn held_rows(&self) -> Vec<Row> {
        self.rt.sessions().borrow().values().cloned().collect()
    }

    /// The rows of one thread, in map order.
    fn thread_rows(&self, thread_id: &str) -> Vec<Row> {
        self.held_rows()
            .into_iter()
            .filter(|s| wsp_wire::views::session::thread_key_of(&s.borrow().view) == thread_id)
            .collect()
    }

    /// `diskFacts` and `diskKeptUntil` (boot.ts:703-716): the disk line and the keep's end of a record.
    fn disk_facts(
        &self,
        record: Option<&wsp_core::types::wiring::WorkspaceRecord>,
    ) -> Map<String, Value> {
        let mut out = Map::new();
        if let Some(record) = record {
            if let Some(gone) = &record.disk_gone {
                out.insert("diskGone".into(), json!(gone));
            }
            if let Some(until) = self.rt.disk_kept_until(record) {
                out.insert("diskKeptUntil".into(), json!(until));
            }
        }
        out
    }

    /// `forkedFromNow` (boot.ts:675).
    pub fn forked_from_now(&self, from: &ForkedFrom) -> ForkedFrom {
        let rows = self.rt.rows_on(&from.thread_id);
        match wsp_words::views::session::fold_threads(&rows)
            .into_iter()
            .next()
        {
            None => from.clone(),
            Some(source) => ForkedFrom {
                title: source.title,
                ..from.clone()
            },
        }
    }

    /// `listedRows` (boot.ts:681), spec rule 1: each row as a listing answers it, in the order given.
    pub fn listed_rows(&self, held: &[Row]) -> Vec<SessionView> {
        let mut latest: IndexMap<String, Row> = IndexMap::new();
        for s in held {
            latest.insert(
                wsp_wire::views::session::thread_key_of(&s.borrow().view),
                s.clone(),
            );
        }
        let replaced_by = self.rt.restarts();
        let now = self.rt.clock().now() as f64;
        let reads_since = self.rt.reads_since();
        held.iter()
            .filter_map(|s| {
                let (view, turn_id, pid, run, replaces) = {
                    let e = s.borrow();
                    (
                        e.view.clone(),
                        e.turn_id.clone(),
                        e.pid,
                        e.run.clone(),
                        e.replaces.clone(),
                    )
                };
                let key = wsp_wire::views::session::thread_key_of(&view);
                let running = view.status == wsp_wire::views::session::SessionStatus::Running;
                let behind = if running {
                    self.rt.stopped_behind(s)
                } else {
                    None
                };
                let capped = self.rt.cap_wait(&turn_id);
                let marks_on = self.rt.thread_records().borrow().get(&key).cloned();
                let marks = marks_on
                    .as_ref()
                    .map(|t| serde_json::to_value(t).map(object).unwrap_or_default())
                    .unwrap_or_default();
                let marks = Value::Object(marks);
                let restart = replaced_by.get(&key).cloned();
                let replaces = field(&marks, "replaces")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or(replaces);
                let agent_pid = match (running, pid, run.as_deref()) {
                    (true, Some(_), Some(run)) => self.rt.agent_pid_of(run),
                    _ => None,
                };
                let children = if latest.get(&key).is_some_and(|l| Rc::ptr_eq(l, s)) {
                    view.thread_id
                        .as_ref()
                        .map(|t| self.rt.index_children(&view.workspace_id, t))
                } else {
                    None
                };
                let setup_refusal = {
                    let entry = self.rt.live().borrow().get(&view.workspace_id).cloned();
                    match entry {
                        Some(entry) if self.rt.setup_place(&entry).is_some() => {
                            self.rt.setup_refusal(&view.workspace_id, &view.harness)
                        }
                        _ => None,
                    }
                };
                let mut out = object(serde_json::to_value(&view).unwrap_or(Value::Null));
                if running {
                    if let Some(pid) = pid {
                        out.insert("pid".into(), json!(pid));
                    }
                }
                if let Some(agent_pid) = agent_pid {
                    out.insert("agentPid".into(), json!(agent_pid));
                }
                if let Some(behind) = behind {
                    out.insert("waitingOn".into(), behind);
                }
                if let Some(capped) = capped {
                    out.insert("capped".into(), capped);
                }
                if let Some(children) = children.filter(|c| !c.is_empty()) {
                    out.insert("subagents".into(), Value::Array(children));
                }
                if let Some(refused) = setup_refusal {
                    out.insert("setupRefusal".into(), json!(refused));
                }
                let read_at = match number_of(&marks, "readAt") {
                    Some(at) => at,
                    None => match view.ended_at.as_ref().map(|t| t.0) {
                        Some(ended) if ended < reads_since => ended,
                        _ => reads_since,
                    },
                };
                out.insert("readAt".into(), json!(read_at));
                for key_name in ["settledAt", "pinnedAt", "order", "foldedAt"] {
                    if let Some(v) = field(&marks, key_name) {
                        out.insert(key_name.into(), v.clone());
                    }
                }
                if let Some(replaces) = replaces {
                    let known = self.rt.thread_records().borrow().contains_key(&replaces)
                        || self.rt.latest_on(&replaces).is_some();
                    if known {
                        out.insert("replaces".into(), json!(replaces));
                    }
                }
                if let Some(restart) = restart {
                    out.insert("replacedBy".into(), json!(restart));
                }
                if let Some(until) = number_of(&marks, "snoozedUntil") {
                    out.insert(
                        if until > now {
                            "snoozedUntil"
                        } else {
                            "wokeAt"
                        }
                        .into(),
                        json!(until),
                    );
                }
                if let Some(section) = field(&marks, "section") {
                    out.insert("section".into(), section.clone());
                }
                if let Some(at) = field(&marks, "rewound").and_then(|r| number_of(r, "at")) {
                    out.insert("rewoundAt".into(), json!(at));
                }
                if let Some(limit) = field(&marks, "limitResume") {
                    if text_of(limit, "turnId").as_deref() == Some(turn_id.as_str()) {
                        if let Some(at) = number_of(limit, "at") {
                            out.insert("resumeAt".into(), json!(at));
                        }
                    }
                }
                if let Some(from) = field(&marks, "forkedFrom") {
                    if let Ok(from) = parse::<ForkedFrom>(from.clone()) {
                        if let Ok(now_from) = serde_json::to_value(self.forked_from_now(&from)) {
                            out.insert("forkedFrom".into(), now_from);
                        }
                    }
                }
                parse::<SessionView>(Value::Object(out)).ok()
            })
            .collect()
    }

    /// `threadFacts` (boot.ts:739), spec rule 2.
    pub fn thread_facts(&self, thread_id: &str) -> Option<ThreadFacts> {
        let held = self.thread_rows(thread_id);
        let views: Vec<SessionView> = held.iter().map(|s| s.borrow().view.clone()).collect();
        let listed = self.listed_rows(&held);
        let mut thread = folded_thread(&listed)?;
        if let Some(obj) = thread.as_object_mut() {
            obj.remove("subagents");
        }
        let latest = views.last()?.clone();
        let running = held
            .iter()
            .rev()
            .find(|s| s.borrow().view.status == wsp_wire::views::session::SessionStatus::Running)
            .map(|s| s.borrow().turn_id.clone());
        let workspace_id = text_of(&thread, "workspaceId").unwrap_or_default();
        let entry = self.rt.live().borrow().get(&workspace_id).cloned();
        let record = entry.map(|e| e.borrow().record.borrow().clone());
        let mut out = object(thread);
        if let Some(sub) = record.as_ref().and_then(|r| self.rt.sub_row_of(r)) {
            out.insert(
                "sub".into(),
                serde_json::to_value(sub).unwrap_or(Value::Null),
            );
        }
        out.extend(self.disk_facts(record.as_ref()));
        if let Some(model) = &latest.model {
            out.insert("model".into(), json!(model));
        }
        if let Some(effort) = &latest.effort {
            out.insert("effort".into(), json!(effort));
        }
        if let Some(window) = latest.context_window {
            out.insert("contextWindow".into(), json!(window));
        }
        if let Some(turn) = running {
            out.insert("turnId".into(), json!(turn));
        }
        parse::<ThreadFacts>(Value::Object(out)).ok()
    }

    /// `factsAfterRead` (boot.ts:760), spec rule 3.
    pub async fn facts_after_read(
        self: &Rc<Self>,
        thread_id: &str,
        origin: Option<&Caller>,
    ) -> Result<Option<ThreadFacts>, HostError> {
        let facts = self.thread_facts(thread_id);
        let Some(facts_ref) = facts.as_ref() else {
            return Ok(None);
        };
        let since = if facts_ref.status == wsp_wire::views::session::SessionStatus::Running {
            Some(self.rt.clock().now() as f64)
        } else {
            facts_ref.ended_at.as_ref().map(|t| t.0)
        };
        let (Some(_), Some(since)) = (facts_ref.sub.as_ref(), since) else {
            return Ok(facts);
        };
        let row = RowAt {
            thread_id: Some(
                facts_ref
                    .thread_id
                    .clone()
                    .unwrap_or_else(|| thread_id.to_string()),
            ),
            workspace_id: facts_ref.workspace_id.clone(),
        };
        if self
            .entry_of_row(&row, origin, None, false)
            .await?
            .is_none()
        {
            return Ok(facts);
        }
        self.rt.sub_read(&facts_ref.workspace_id, since).await?;
        Ok(self.thread_facts(thread_id))
    }

    /// `settledNow` (boot.ts:771), spec rule 4. `settle_ms` `None` never settles by quiet.
    pub fn settled_now(&self, thread_id: &str, settle_ms: Option<f64>) -> bool {
        let Some(t) = self.thread_facts(thread_id) else {
            let marked = self
                .rt
                .thread_records()
                .borrow()
                .get(thread_id)
                .and_then(|r| r.settled_at);
            return marked.is_some();
        };
        let facts = wsp_words::thread_state::SettleFacts {
            working: t.status == wsp_wire::views::session::SessionStatus::Running,
            asking: t.asking.is_some() || t.waiting_on.is_some(),
            failed: t.status == wsp_wire::views::session::SessionStatus::Failed,
            started_at: t.started_at.as_ref().map(|x| x.0),
            ended_at: t.ended_at.as_ref().map(|x| x.0),
            read_at: t.read_at.as_ref().map(|x| x.0),
            settled_at: t.settled_at.as_ref().map(|x| x.0),
        };
        wsp_words::thread_state::thread_settled(
            &facts,
            self.rt.clock().now() as f64,
            settle_ms,
            false,
        )
    }

    /// `pushHead` (boot.ts:778), spec rule 5.
    pub fn push_head(&self, thread_id: &str) -> Result<(), HostError> {
        let Some(facts) = self.thread_facts(thread_id) else {
            return Ok(());
        };
        let event = json!({
            "type": "thread.head",
            "workspaceId": facts.workspace_id,
            "threadId": thread_id,
            "facts": serde_json::to_value(&facts).unwrap_or(Value::Null),
            "pos": self.rt.index_pos(&facts.workspace_id),
        });
        self.rt.emit(parse::<EventUnion>(event)?)
    }

    /// `placedRows` (boot.ts:783), spec rule 6.
    pub async fn placed_rows(&self, listed: Vec<SessionView>) -> Vec<SessionView> {
        let computers = if listed.is_empty() {
            Vec::new()
        } else {
            self.computer_rows().await
        };
        let mut out = Vec::new();
        for view in listed {
            let entry = self.rt.live().borrow().get(&view.workspace_id).cloned();
            let record = entry.map(|e| e.borrow().record.borrow().clone());
            let project_id = record.as_ref().map(|r| r.project.clone());
            let project = project_id
                .as_ref()
                .and_then(|id| self.rt.projects_held().borrow().get(id).cloned());
            let sub = record.as_ref().and_then(|r| self.rt.sub_row_of(r));
            let at = match (&sub, &record) {
                (Some(_), Some(r)) => self.rt.place_id_of(r),
                _ => project.as_ref().map(|p| p.computer.clone()),
            };
            let computer = at
                .as_ref()
                .and_then(|at| computers.iter().find(|c| &c.id == at));
            let mut placed = object(serde_json::to_value(&view).unwrap_or(Value::Null));
            if let Some(sub) = &sub {
                placed.insert(
                    "sub".into(),
                    serde_json::to_value(sub).unwrap_or(Value::Null),
                );
            }
            placed.extend(self.disk_facts(record.as_ref()));
            let Some(project) = project else {
                out.push(parse::<SessionView>(Value::Object(placed)).unwrap_or(view));
                continue;
            };
            placed.insert(
                "project".into(),
                json!({ "id": project.id, "name": project.name }),
            );
            let said = match (computer, &sub) {
                (Some(computer), _) => {
                    Some(wsp_words::place_name::table_name(&place_ref(computer)))
                }
                (None, Some(sub)) => Some(wsp_words::sub_thread::cloud_said(&sub.on)),
                (None, None) => Some(project.computer.clone()),
            };
            if let Some(said) = said {
                placed.insert("computerName".into(), json!(said));
            }
            out.push(parse::<SessionView>(Value::Object(placed)).unwrap_or(view));
        }
        out
    }

    /// `rowMoved` (boot.ts:857), spec rule 7.
    pub fn row_moved(self: &Rc<Self>, workspace_id: &str, thread_id: &str) {
        self.queue_rows();
        self.rows_moved
            .borrow_mut()
            .insert(thread_id.to_string(), workspace_id.to_string());
    }

    /// `rowGone` (boot.ts:862), spec rule 7.
    pub fn row_gone(self: &Rc<Self>, id: &str, workspace_id: &str, thread_id: Option<&str>) {
        self.queue_rows();
        self.rows_gone.borrow_mut().insert(
            id.to_string(),
            (workspace_id.to_string(), thread_id.map(str::to_string)),
        );
    }

    /// `queueRows` (boot.ts:846): one push at the end of the current task.
    fn queue_rows(self: &Rc<Self>) {
        if self.rows_queued.replace(true) {
            return;
        }
        let area = self.clone();
        wsp_engine::local_task::try_spawn_local(async move {
            area.push_rows();
        });
    }

    /// `pushRows` (boot.ts:824-845): the moved rows built, the gone ids named, and the batch sent on the rows chain.
    fn push_rows(self: &Rc<Self>) {
        self.rows_queued.set(false);
        let moved: Vec<(String, String)> = self.rows_moved.borrow_mut().drain(..).collect();
        let gone: Vec<(String, (String, Option<String>))> = {
            let held = self.rt.sessions();
            let held = held.borrow();
            self.rows_gone
                .borrow_mut()
                .drain(..)
                .filter(|(id, _)| !held.contains_key(id))
                .collect()
        };
        let mut told: Vec<String> = Vec::new();
        let mut rows: Vec<SessionView> = Vec::new();
        for (thread_id, _) in &moved {
            let held = self.thread_rows(thread_id);
            let Some(latest) = held.last().cloned() else {
                continue;
            };
            let view = match self.listed_rows(&held).pop() {
                Some(view) => view,
                None => continue,
            };
            told.push(view.id.clone());
            let turn = latest.borrow().turn_id.clone();
            if view.waiting_on.is_some() {
                self.told_behind.borrow_mut().insert(turn);
            } else {
                self.told_behind.borrow_mut().shift_remove(&turn);
            }
            if self.told_rows.borrow().contains(&view.id) {
                let mut plain = view;
                plain.prompt = None;
                rows.push(plain);
            } else {
                rows.push(view);
            }
        }
        let mut seen = IndexMap::new();
        for (_, ws) in &moved {
            seen.insert(ws.clone(), ());
        }
        for ws in seen.keys() {
            if self.rt.boot_work().borrow().contains_key(ws) {
                continue;
            }
            let own: Vec<SessionView> = self
                .held_rows()
                .iter()
                .map(|s| s.borrow().view.clone())
                .filter(|v| &v.workspace_id == ws)
                .collect();
            for view in self.rt.title_rows(own) {
                self.rt.refresh_title(view);
            }
        }
        let area = self.clone();
        let previous = self.rows_out.borrow().clone();
        let chain: futures::future::Shared<LocalBoxFuture<'static, ()>> = async move {
            if let Some(previous) = previous {
                previous.await;
            }
            let out = area
                .send_rows(&moved, gone.clone(), rows, told.clone())
                .await;
            if let Err(e) = out {
                area.rt
                    .warn(&format!("a moved row was not pushed: {}", e.message));
                let mut queued = area.rows_moved.borrow_mut();
                for (thread_id, ws) in moved {
                    if !queued.contains_key(&thread_id) {
                        queued.insert(thread_id, ws);
                    }
                }
            }
        }
        .boxed_local()
        .shared();
        *self.rows_out.borrow_mut() = Some(chain.clone());
        wsp_engine::local_task::try_spawn_local(chain);
    }

    /// The batch of one push: the gone ids, then the placed rows, then the told bookkeeping (boot.ts:834-844).
    async fn send_rows(
        self: &Rc<Self>,
        _moved: &[(String, String)],
        gone: Vec<(String, (String, Option<String>))>,
        rows: Vec<SessionView>,
        told: Vec<String>,
    ) -> Result<(), HostError> {
        for (id, (workspace_id, thread_id)) in &gone {
            let mut event = json!({ "type": "session.row", "workspaceId": workspace_id, "id": id });
            if let Some(thread_id) = thread_id {
                event["threadId"] = json!(thread_id);
            }
            self.rt.pass(parse::<EventUnion>(event)?)?;
        }
        for row in self.placed_rows(rows).await {
            let mut event = json!({ "type": "session.row", "workspaceId": row.workspace_id, "id": row.id, "row": serde_json::to_value(&row).unwrap_or(Value::Null) });
            if let Some(thread_id) = &row.thread_id {
                event["threadId"] = json!(thread_id);
            }
            self.rt.pass(parse::<EventUnion>(event)?)?;
        }
        {
            let mut told_rows = self.told_rows.borrow_mut();
            for id in &told {
                told_rows.insert(id.clone());
            }
            for (id, _) in &gone {
                told_rows.shift_remove(id);
            }
        }
        let held_count = self.rt.sessions().borrow().len();
        if self.told_rows.borrow().len() > 2 * held_count + 64 {
            let held = self.rt.sessions();
            let held = held.borrow();
            self.told_rows
                .borrow_mut()
                .retain(|id| held.contains_key(id));
        }
        Ok(())
    }

    /// `rowsSent` (boot.ts:853): a queued push now, then the chain awaited.
    pub async fn rows_sent(self: &Rc<Self>) {
        if self.rows_queued.get() {
            self.push_rows();
        }
        let chain = self.rows_out.borrow().clone();
        if let Some(chain) = chain {
            chain.await;
        }
    }

    /// `setupRefusalMoved` (boot.ts:867).
    pub fn setup_refusal_moved(self: &Rc<Self>, place: &str, harness: &str) {
        for s in self.held_rows() {
            let (ws, agent, key) = {
                let e = s.borrow();
                (
                    e.view.workspace_id.clone(),
                    e.view.harness.clone(),
                    wsp_wire::views::session::thread_key_of(&e.view),
                )
            };
            let entry = self.rt.live().borrow().get(&ws).cloned();
            if agent == harness
                && entry
                    .as_ref()
                    .is_some_and(|e| self.rt.setup_place(e).as_deref() == Some(place))
            {
                self.row_moved(&ws, &key);
            }
        }
    }

    /// `mark` (boot.ts:923), spec rule 9.
    pub async fn mark(
        self: &Rc<Self>,
        thread_ids: &[String],
        stamped: Stamped<'_>,
        origin: Option<&Caller>,
    ) -> Result<(), HostError> {
        self.ready().await.map_err(|e| e.0)?;
        let mut found: Vec<(String, String, Map<String, Value>)> = Vec::new();
        for thread_id in thread_ids {
            let held = self.thread_rows(thread_id);
            let latest = held.last().map(|s| s.borrow().view.clone());
            let record = self.rt.thread_records().borrow().get(thread_id).cloned();
            let workspace_id = latest
                .as_ref()
                .map(|v| v.workspace_id.clone())
                .or_else(|| record.as_ref().map(|r| r.workspace_id.clone()));
            let Some(workspace_id) = workspace_id else {
                return Err(not_found_refusal(&format!(
                    "no thread {}",
                    thread_word(thread_id)
                )));
            };
            let row = RowAt {
                thread_id: Some(
                    latest
                        .as_ref()
                        .and_then(|v| v.thread_id.clone())
                        .unwrap_or_else(|| thread_id.clone()),
                ),
                workspace_id: workspace_id.clone(),
            };
            if self
                .entry_of_row(&row, origin, None, false)
                .await?
                .is_none()
            {
                return Err(not_found_refusal(&format!(
                    "no thread {}",
                    thread_word(thread_id)
                )));
            }
            let base = match record {
                Some(r) => object(serde_json::to_value(&r).unwrap_or(Value::Null)),
                None => {
                    let harness = latest
                        .as_ref()
                        .map(|v| v.harness.clone())
                        .unwrap_or_default();
                    let mut m = Map::new();
                    m.insert("workspaceId".into(), json!(workspace_id));
                    m.insert("harness".into(), json!(harness));
                    m
                }
            };
            found.push((thread_id.clone(), workspace_id, base));
        }
        let mut touched: IndexMap<String, Vec<String>> = IndexMap::new();
        for (thread_id, workspace_id, base) in found {
            let stamps: ThreadStamps = match &stamped {
                Stamped::Same(s) => s.clone(),
                Stamped::Each(f) => f(&thread_id),
            };
            let mut next = base;
            for (key, value) in &stamps {
                if value.is_null() {
                    next.remove(key);
                } else {
                    next.insert(key.clone(), value.clone());
                }
            }
            let on: ThreadRecord = parse(Value::Object(next.clone()))?;
            self.rt
                .thread_records()
                .borrow_mut()
                .insert(thread_id.clone(), on.clone());
            if next.get("settledAt").is_some_and(|v| !v.is_null()) {
                self.rt.reap_kept(&thread_id);
            }
            if let Some(until) = number_of(&Value::Object(next.clone()), "snoozedUntil") {
                self.rt.wake_at(&thread_id, until);
            }
            if stamps.contains_key("limitResume") {
                match field(&Value::Object(next.clone()), "limitResume")
                    .and_then(|l| number_of(l, "at"))
                {
                    Some(at) => self.rt.resume_on_reset(&thread_id, at),
                    None => self.rt.cancel_resume(&thread_id),
                }
            }
            touched.entry(workspace_id).or_default().push(thread_id);
        }
        for (workspace_id, ids) in touched {
            self.rt.persist_sessions(&workspace_id).await;
            self.rt.emit(parse::<EventUnion>(
                json!({ "type": "thread.marked", "workspaceId": workspace_id, "threadIds": ids }),
            )?)?;
        }
        Ok(())
    }

    /// `endSnoozeFor` (boot.ts:959), spec rule 10.
    pub fn end_snooze_for(self: &Rc<Self>, thread_id: Option<&str>, root_thread_id: Option<&str>) {
        let now = self.rt.clock().now() as f64;
        let mut standing: Vec<String> = Vec::new();
        for id in [thread_id, root_thread_id].into_iter().flatten() {
            let snoozed = self
                .rt
                .thread_records()
                .borrow()
                .get(id)
                .and_then(|r| r.snoozed_until.as_ref().map(|j| j.0))
                .unwrap_or(0.0);
            if snoozed > now && !standing.iter().any(|s| s == id) {
                standing.push(id.to_string());
            }
        }
        if standing.is_empty() {
            return;
        }
        let mut stamps = ThreadStamps::new();
        stamps.insert("snoozedUntil".into(), json!(now));
        let area = self.clone();
        wsp_engine::local_task::try_spawn_local(async move {
            if let Err(e) = area.mark(&standing, Stamped::Same(stamps), None).await {
                area.rt.warn(&format!(
                    "snooze not ended for {}: {}",
                    standing.join(", "),
                    e.message
                ));
            }
        });
    }

    /// `armResume` (boot.ts:966), spec rule 11.
    pub async fn arm_resume(
        self: &Rc<Self>,
        thread_ids: &[String],
        on: bool,
        origin: Option<&Caller>,
    ) -> Result<(), HostError> {
        if !on {
            let mut stamps = ThreadStamps::new();
            stamps.insert("limitResume".into(), Value::Null);
            return self.mark(thread_ids, Stamped::Same(stamps), origin).await;
        }
        self.ready().await.map_err(|e| e.0)?;
        tokio::task::yield_now().await;
        let mut arms: Vec<(String, Value)> = Vec::new();
        for thread_id in thread_ids {
            let held = self.thread_rows(thread_id);
            let Some(latest) = held.last().cloned() else {
                return Err(not_found_refusal(&format!(
                    "no thread {}",
                    thread_word(thread_id)
                )));
            };
            let (view, turn_id) = {
                let e = latest.borrow();
                (e.view.clone(), e.turn_id.clone())
            };
            let row = RowAt {
                thread_id: view.thread_id.clone(),
                workspace_id: view.workspace_id.clone(),
            };
            if self
                .entry_of_row(&row, origin, None, false)
                .await?
                .is_none()
            {
                return Err(not_found_refusal(&format!(
                    "no thread {}",
                    thread_word(thread_id)
                )));
            }
            let resets = view
                .limit
                .as_ref()
                .and_then(|l| serde_json::to_value(l).ok())
                .and_then(|l| number_of(&l, "resetsAt"));
            let Some(at) = resets else {
                return Err(refusal(
                    &format!(
                        "thread {} is not stopped at a usage limit with a known reset,",
                        thread_word(thread_id)
                    ),
                    "so there is nothing to resume at; send it a message to go on now.",
                    Some(Kind::Usage),
                ));
            };
            arms.push((thread_id.clone(), json!({ "at": at, "turnId": turn_id })));
        }
        for (thread_id, arm) in arms {
            let mut stamps = ThreadStamps::new();
            stamps.insert("limitResume".into(), arm);
            self.mark(
                std::slice::from_ref(&thread_id),
                Stamped::Same(stamps),
                origin,
            )
            .await?;
        }
        Ok(())
    }

    /// `resumeAfterLimit` (boot.ts:984), spec rule 12.
    pub async fn resume_after_limit(self: &Rc<Self>, thread_id: &str) -> Result<(), HostError> {
        self.ready().await.map_err(|e| e.0)?;
        let record = self.rt.thread_records().borrow().get(thread_id).cloned();
        let Some(record) = record else { return Ok(()) };
        let Some(arm) = record.limit_resume.clone() else {
            return Ok(());
        };
        let latest = self.thread_rows(thread_id).last().cloned();
        let entry = self.rt.live().borrow().get(&record.workspace_id).cloned();
        let (latest_turn, latest_view) = match &latest {
            Some(l) => {
                let e = l.borrow();
                (Some(e.turn_id.clone()), Some(e.view.clone()))
            }
            None => (None, None),
        };
        let stands = record.settled_at.is_none()
            && entry.is_some()
            && latest_turn.as_deref() == Some(arm.turn_id.as_str())
            && latest_view.as_ref().is_some_and(|v| v.limit.is_some());
        let facts = if stands {
            self.thread_facts(thread_id)
        } else {
            None
        };
        let waits = self.rt.lead_asks(thread_id)
            || facts
                .as_ref()
                .is_some_and(|f| f.asking.is_some() || f.waiting_on.is_some());
        let held = match (stands && !waits, entry.as_ref(), latest_view.as_ref()) {
            (true, Some(entry), Some(view)) => self.held_past(entry, &view.harness).await,
            _ => None,
        };
        let current = self.rt.thread_records().borrow().get(thread_id).cloned();
        let same_arm = current
            .as_ref()
            .and_then(|c| c.limit_resume.as_ref())
            .is_some_and(|a| a.at == arm.at && a.turn_id == arm.turn_id);
        if !same_arm {
            return Ok(());
        }
        let workspace_id = record.workspace_id.clone();
        if let Some(current) = self.rt.thread_records().borrow_mut().get_mut(thread_id) {
            current.limit_resume = None;
        }
        if let (Some(held), Some(latest)) = (held, latest.as_ref()) {
            let mut view =
                serde_json::to_value(latest.borrow().view.clone()).unwrap_or(Value::Null);
            view["limit"] = json!({ "resetsAt": held });
            let view: SessionView = parse(view)?;
            latest.borrow_mut().view = view;
        }
        self.rt.persist_sessions(&workspace_id).await;
        self.rt.emit(parse::<EventUnion>(json!({ "type": "thread.marked", "workspaceId": workspace_id, "threadIds": [thread_id] }))?)?;
        if !stands || waits || held.is_some() {
            return Ok(());
        }
        self.rt
            .resume_turn(&workspace_id, thread_id, arm.at.0)
            .await
    }

    /// `heldPast` (boot.ts:1008), spec rule 13.
    pub async fn held_past(&self, entry: &Entry, harness: &str) -> Option<f64> {
        let limits = self.rt.ledger_limits().await;
        let key = self.rt.account_key_on(entry, harness);
        let windows = limits
            .iter()
            .find(|l| l.key == key)
            .map(|l| l.windows.clone())
            .unwrap_or_default();
        wsp_words::usage::held_until(&windows, self.rt.clock().now() as f64)
    }
}

/// The table's `PlaceRef` a computer row names (place_word.rs), for its name in a listing.
fn place_ref(row: &ComputerRow) -> wsp_words::place_word::PlaceRef {
    let kind = match row.kind {
        wsp_wire::views::place_kind::PlaceKind::Computer => "computer",
        wsp_wire::views::place_kind::PlaceKind::Provider => "provider",
    };
    wsp_words::place_word::PlaceRef {
        id: row.id.clone(),
        name: row.name.clone(),
        kind: Some(kind.to_string()),
    }
}
