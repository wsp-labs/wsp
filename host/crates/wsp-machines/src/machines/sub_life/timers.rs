// SPDX-License-Identifier: AGPL-3.0-only
//! sub-life.ts:156-597: the area. A done reads the machine, keeps the session, arms the delete or holds the machine,
//! and naps it; the delete at `deleteAt` releases the record. Spec: spec/sub_life_timers.md.
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::place_state::DELETE_AFTER_MS;
use wsp_wire::types::projects::THREAD_WORKING_LINE;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::sub_row::{sub_all_pushed, SubHeld, SubResult};
use wsp_wire::views::workspace::{scope_of, Caller, ReachState, WorkspacePhase};
use wsp_words::date_parse::date_parse;
use wsp_words::exit::{HostError, Kind};
use wsp_words::js::js_trim;
use wsp_words::js::utf16::tail;
use wsp_words::js_date::iso_string;
use wsp_words::sub_thread::{
    cloud_said, is_git_push, push_refused_of, sub_cloud_gone_refusal, sub_delete_question, sub_delete_refused_line, sub_held_delete_refusal,
    sub_released_refusal, sub_result_line, KeptUntil, SubDeleteQuestion, SubResultFacts, SubUnpushed,
};
use wsp_words::views::session::fold_threads;
use wsp_words::words::units::fmt_bytes;

use super::{disk_kept_until, sub_result_command, sub_result_of, sub_session_pack_command, SessionFile, SubLifeCtx, DONE_NAP, PUSH_READ_BYTES, PUSH_READ_EVENTS, SUB_DELETE_RETRY_MS, SUB_LINE_WAIT_MS, SUB_SESSIONS, SUB_SESSION_MAX_BYTES, SUB_STOP_AFTER_DELETE_MS};
use crate::machines::area_error::{AreaError, AreaResult};
use crate::machines::machines::Entry;
use wsp_core::clock::Cancel;
use wsp_core::types::internal::GONE_REASON;
use wsp_core::types::wiring::{DeleteSaid, SubHeldAt, SubRecord, SubSession, WorkspaceRecord};

/// The answer of `sub_delete` (sub-life.ts:584): `machine` is `Some("deleted")` once deleted, `question` with `check`.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubDeleted {
    pub workspace_id: String,
    pub threads: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    pub on: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
}

/// `GoneOutcome` as `subLost` answers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoneOutcome {
    Settled,
}

/// One line waiting on a done's read (sub-life.ts:163).
pub(crate) struct ReadWait {
    pub since: f64,
    pub go: Rc<dyn Fn()>,
}

/// sub-life.ts:156 `subLifeArea(ctx)`.
pub struct SubLifeArea {
    pub(crate) ctx: Rc<dyn SubLifeCtx>,
    pub(crate) me: Weak<SubLifeArea>,
    /// sub-life.ts:158.
    pub(crate) timers: RefCell<IndexMap<String, Cancel>>,
    /// sub-life.ts:160. The record's chain tail; it never fails (rule 1).
    pub(crate) working: RefCell<IndexMap<String, Shared<LocalBoxFuture<'static, ()>>>>,
    /// sub-life.ts:163.
    pub(crate) read_waits: RefCell<IndexMap<String, Vec<Rc<ReadWait>>>>,
    /// sub-life.ts:168.
    pub(crate) done_started: RefCell<IndexMap<String, f64>>,
}

/// The two steps a record's chain runs.
#[derive(Clone, Copy)]
enum Step {
    Done,
    Expire,
}

fn step_work(me: Weak<SubLifeArea>, id: String, step: Step) -> Box<dyn FnOnce() -> LocalBoxFuture<'static, AreaResult<()>>> {
    Box::new(move || {
        async move {
            let Some(area) = me.upgrade() else { return Ok(()) };
            match step {
                Step::Done => area.done(&id).await,
                Step::Expire => area.expire(&id).await,
            }
        }
        .boxed_local()
    })
}

fn record_of(entry: &Entry) -> Rc<RefCell<WorkspaceRecord>> {
    entry.borrow().record.clone()
}

fn machine_of(entry: &Entry) -> Arc<dyn Machine> {
    entry.borrow().machine.clone()
}

fn snapshot(record: &RefCell<WorkspaceRecord>) -> WorkspaceRecord {
    record.borrow().clone()
}

fn phase_of(record: &RefCell<WorkspaceRecord>) -> WorkspacePhase {
    record.borrow().phase
}

fn phase_word(phase: WorkspacePhase) -> String {
    serde_json::to_value(phase).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn has_sub_record(record: &RefCell<WorkspaceRecord>) -> bool {
    record.borrow().sub.is_some()
}

fn with_sub<R>(record: &RefCell<WorkspaceRecord>, f: impl FnOnce(&mut SubRecord) -> R) -> Option<R> {
    record.borrow_mut().sub.as_mut().map(f)
}

fn held_of(result: &SubResult, at: f64) -> SubHeldAt {
    SubHeldAt { held: SubHeld { files: result.changed, commits: result.unpushed }, at: JsNumber(at) }
}

fn set_session(record: &RefCell<WorkspaceRecord>, session: SubSession) {
    with_sub(record, |sub| sub.session = Some(session));
}

fn lead_thread_of(record: &WorkspaceRecord) -> Option<String> {
    record.lead.as_ref().map(|lead| lead.thread_id.clone())
}

/// The first `size <n>` line of a pack's output, where `n` is all digits.
fn size_line(out: &str) -> Option<f64> {
    out.lines().find_map(|line| {
        line.strip_prefix("size ").filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())).and_then(|d| d.parse::<f64>().ok())
    })
}

/// The first `path <p>` line of a pack's output.
fn path_line(out: &str) -> Option<String> {
    out.lines().find_map(|line| line.strip_prefix("path ").filter(|p| !p.is_empty()).map(str::to_string))
}

fn exec_options() -> ExecOptions {
    ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }
}

impl SubLifeArea {
    /// sub-life.ts:156. Builds the area and registers its listeners on `session.end`, `session.start`,
    /// `session.held`, `session.queued` and `workspace.deleted` (rules 13-15), each holding a `Weak` to the area.
    pub fn new(ctx: Rc<dyn SubLifeCtx>) -> Rc<SubLifeArea> {
        Rc::new_cyclic(|me: &Weak<SubLifeArea>| {
            let on_end = me.clone();
            ctx.on_event(
                "session.end",
                Rc::new(move |event: &Value| {
                    if let Some(area) = on_end.upgrade() {
                        area.on_session_end(event);
                    }
                }),
            );
            for kind in ["session.start", "session.held", "session.queued"] {
                let me = me.clone();
                ctx.on_event(
                    kind,
                    Rc::new(move |event: &Value| {
                        if let (Some(area), Some(id)) = (me.upgrade(), event["workspaceId"].as_str()) {
                            area.reclaim(id);
                        }
                    }),
                );
            }
            let deleted = me.clone();
            ctx.on_event(
                "workspace.deleted",
                Rc::new(move |event: &Value| {
                    if let (Some(area), Some(id)) = (deleted.upgrade(), event["workspaceId"].as_str()) {
                        area.cancel_timer(id);
                        area.done_started.borrow_mut().shift_remove(id);
                    }
                }),
            );
            SubLifeArea {
                ctx,
                me: me.clone(),
                timers: RefCell::new(IndexMap::new()),
                working: RefCell::new(IndexMap::new()),
                read_waits: RefCell::new(IndexMap::new()),
                done_started: RefCell::new(IndexMap::new()),
            }
        })
    }

    /// sub-life.ts:170 `serial`: the record's chain, second form (rule 1).
    pub(crate) fn serial(&self, id: &str, work: Box<dyn FnOnce() -> LocalBoxFuture<'static, AreaResult<()>>>) -> Shared<LocalBoxFuture<'static, ()>> {
        let prev = self.working.borrow().get(id).cloned();
        let ctx = self.ctx.clone();
        let owner = id.to_string();
        let chain: LocalBoxFuture<'static, ()> = async move {
            if let Some(prev) = prev {
                prev.await;
            }
            if let Err(e) = work().await {
                ctx.warn(&format!("sub-thread machine {owner}: {}", e.message()));
            }
        }
        .boxed_local();
        let shared = chain.shared();
        self.working.borrow_mut().insert(id.to_string(), shared.clone());
        self.ctx.spawn(shared.clone().boxed_local());
        shared
    }

    /// sub-life.ts:181 (rule 2).
    pub(crate) fn thread_of(&self, workspace_id: &str) -> Option<String> {
        if let Some((thread_id, _)) = self.ctx.thread_records().into_iter().find(|(_, ws)| ws == workspace_id) {
            return Some(thread_id);
        }
        self.ctx.session_views().into_iter().find_map(|view| (view.workspace_id == workspace_id).then_some(view.thread_id).flatten())
    }

    /// sub-life.ts:194 (rule 3).
    pub(crate) fn busy(&self, thread_id: &str) -> bool {
        self.ctx.thread_runs(thread_id) || self.ctx.launching_on(thread_id)
    }

    /// sub-life.ts:201 (rule 5).
    pub(crate) async fn read_result(&self, entry: &Entry) -> SubResult {
        let record = record_of(entry);
        let read_at = iso_string(self.ctx.now()).unwrap_or_default();
        let sub = with_sub(&record, |sub| sub.clone());
        let prior = sub.as_ref().and_then(|sub| sub.result.clone());
        let branch = sub.as_ref().map_or_else(String::new, |sub| sub.branch.clone());
        let failed = |why: String| SubResult {
            branch: branch.clone(),
            pushed_sha: prior.as_ref().and_then(|p| p.pushed_sha.clone()),
            commits: JsInt::from(prior.as_ref().map_or(0, |p| p.commits.get())),
            unpushed: JsInt::from(0),
            changed: JsInt::from(0),
            on_branch: None,
            norepo: None,
            read_failed: Some(why),
            read_at: read_at.clone(),
            push_refused: None,
        };
        let phase = phase_of(&record);
        let Some(sub) = sub else { return failed(format!("its machine was {}", phase_word(phase))) };
        if phase != WorkspacePhase::Running {
            return failed(format!("its machine was {}", phase_word(phase)));
        }
        let machine = machine_of(entry);
        let said = match machine.exec(&sub_result_command(&sub.folder, &sub.branch, &sub.base.sha), exec_options()).await {
            Ok(said) => said,
            Err(e) => return failed(AreaError::Engine(e).message()),
        };
        let code = said.exit_code.get();
        if code != 0 {
            let err = js_trim(&said.stderr);
            let src = if err.is_empty() { js_trim(&said.stdout) } else { err };
            return failed(format!("exit {code}: {}", tail(src, 200)));
        }
        let Some(read) = sub_result_of(&said.stdout, &sub.branch, &read_at) else {
            return failed(format!("it answered no counts: {}", tail(js_trim(&said.stdout), 200)));
        };
        // A machine made after the delete holds no repository until its agent fetches one; the branch pushed before stands.
        match prior.filter(|p| read.norepo.is_some() && p.pushed_sha.is_some()) {
            Some(prior) => SubResult { norepo: None, pushed_sha: prior.pushed_sha.clone(), commits: prior.commits, ..read },
            None => read,
        }
    }

    /// sub-life.ts:222 (rule 6).
    pub(crate) async fn push_refused_in(&self, workspace_id: &str, thread_id: &str) -> AreaResult<Option<String>> {
        let events = self.ctx.read_transcript(workspace_id, thread_id, PUSH_READ_EVENTS, PUSH_READ_BYTES).await?;
        let mut results: IndexMap<String, &Value> = IndexMap::new();
        for event in &events {
            if event["type"] == "session.delta" && event["kind"] == "tool_result" {
                if let Some(id) = event["toolUseId"].as_str() {
                    results.insert(id.to_string(), event);
                }
            }
        }
        for event in events.iter().rev() {
            if event["type"] != "session.delta" || event["kind"] != "tool_use" {
                continue;
            }
            let Some(id) = event["toolUseId"].as_str() else { continue };
            let text = event["text"].as_str().unwrap_or("");
            if !is_git_push(text) {
                continue;
            }
            let Some(out) = results.get(id) else { return Ok(None) };
            let failed = out["isError"].as_bool() == Some(true) || out["exitCode"].as_f64().unwrap_or(0.0) != 0.0;
            return Ok(if failed { push_refused_of(out["text"].as_str().unwrap_or("")) } else { None });
        }
        Ok(None)
    }

    /// sub-life.ts:238 (rule 7).
    pub(crate) async fn keep_session(&self, entry: &Entry, thread_id: &str) {
        let record = record_of(entry);
        let Some(row) = self.ctx.latest_on(thread_id) else { return };
        let Some(id) = row.claude_session_id.clone() else { return };
        if phase_of(&record) != WorkspacePhase::Running {
            return;
        }
        let Some(file) = self.ctx.session_file(entry, &row.harness, &id) else { return };
        let machine = machine_of(entry);
        if !machine.offers().read_bytes {
            return;
        }
        let at = format!("/tmp/wsp-session-{thread_id}.gz");
        if let Err(e) = self.pack_and_keep(entry, &row, &id, &file, thread_id, &at).await {
            let why = e.message();
            self.ctx.warn(&format!("the session of sub-thread {thread_id} was not kept: {why}"));
            let _ = self.ctx.delete_blob(SUB_SESSIONS, thread_id).await;
            set_session(&record, SubSession { agent: row.harness.clone(), id: id.clone(), kept: None, bytes: None, path: None, why: Some(why) });
        }
        let _ = machine.exec(&format!("rm -f {}", shell_quote(&at)), ExecOptions::default()).await;
    }

    /// sub-life.ts:275 (rule 8).
    pub(crate) fn arm(&self, entry: &Entry) {
        let record = record_of(entry);
        let id = record.borrow().id.clone();
        self.cancel_timer(&id);
        let (delete_at, phase) = {
            let held = record.borrow();
            (held.sub.as_ref().and_then(|sub| sub.delete_at), held.phase)
        };
        let Some(at) = delete_at else { return };
        if phase == WorkspacePhase::Released {
            return;
        }
        let me = self.me.clone();
        let timer_id = id.clone();
        let delay = (at.0 - self.ctx.now()).max(0.0);
        let cancel = self.ctx.schedule(
            delay,
            Box::new(move || {
                let Some(area) = me.upgrade() else { return };
                area.timers.borrow_mut().shift_remove(&timer_id);
                drop(area.serial(&timer_id, step_work(area.me.clone(), timer_id.clone(), Step::Expire)));
            }),
        );
        self.timers.borrow_mut().insert(id, cancel);
    }

    /// sub-life.ts:295 (rule 9). `lost` and `cloud_gone` as the TS options.
    pub(crate) async fn release(&self, entry: &Entry, lost: Option<String>, cloud_gone: bool) -> AreaResult<()> {
        let record = record_of(entry);
        let id = record.borrow().id.clone();
        self.cancel_timer(&id);
        if phase_of(&record) == WorkspacePhase::Released {
            let newly = with_sub(&record, |sub| {
                if cloud_gone && sub.cloud_gone != Some(true) {
                    sub.cloud_gone = Some(true);
                    true
                } else {
                    false
                }
            });
            if newly == Some(true) {
                self.persist_record(&record).await?;
                self.told(&id);
            }
            return Ok(());
        }
        self.ctx.end_sessions(&id, GONE_REASON);
        let now = self.ctx.now();
        {
            let mut held = record.borrow_mut();
            held.phase = WorkspacePhase::Released;
            held.machine_id = String::new();
            held.gone = None;
            if let Some(sub) = held.sub.as_mut() {
                sub.released_at = Some(JsNumber(now));
                sub.delete_at = None;
                sub.held = None;
                if let Some(lost) = lost {
                    sub.lost = Some(lost);
                }
                if cloud_gone {
                    sub.cloud_gone = Some(true);
                }
            }
        }
        self.run_late_read(entry);
        self.ctx.forget_marks(&id);
        let released = self.ctx.attach_dead(entry, "");
        self.persist_record(&record).await?;
        self.ctx.idle_forget(&id);
        self.told(&id);
        self.ctx.emit(json!({ "type": "workspace.released", "workspaceId": id }));
        self.ctx.emit_status(&released, ReachState::Gone).await
    }

    /// sub-life.ts:330 (rules 10-11).
    pub(crate) async fn done(&self, id: &str) -> AreaResult<()> {
        let started = self.ctx.now();
        let result = self.done_steps(id).await;
        self.done_started.borrow_mut().insert(id.to_string(), started);
        self.read_done(id, started);
        result
    }

    /// sub-life.ts:385 (rule 12).
    pub(crate) async fn expire(&self, id: &str) -> AreaResult<()> {
        let Some(entry) = self.ctx.live(id) else { return Ok(()) };
        let record = record_of(&entry);
        let has_delete = with_sub(&record, |sub| sub.delete_at.is_some()).unwrap_or(false);
        if !has_delete || phase_of(&record) == WorkspacePhase::Released {
            return Ok(());
        }
        let thread = self.thread_of(id);
        if thread.as_deref().is_some_and(|t| self.busy(t)) {
            with_sub(&record, |sub| sub.delete_at = None);
            self.persist_record(&record).await?;
            return Ok(());
        }
        let napping = entry.borrow().napping.clone();
        if let Some(napping) = napping {
            let _ = napping.await;
        }
        let waking = entry.borrow().waking.clone();
        if let Some(waking) = waking {
            let _ = waking.await;
        }
        if phase_of(&record) == WorkspacePhase::Running {
            let again = self.read_result(&entry).await;
            let now = self.ctx.now();
            let all_pushed = sub_all_pushed(&again);
            with_sub(&record, |sub| {
                sub.result = Some(again.clone());
                if !all_pushed {
                    sub.held = Some(held_of(&again, now));
                    sub.delete_at = None;
                }
            });
            if !all_pushed {
                self.persist_record(&record).await?;
                self.told(id);
                return Ok(());
            }
            if let Some(thread_id) = &thread {
                self.keep_session(&entry, thread_id).await;
            }
        }
        let still_armed = with_sub(&record, |sub| sub.delete_at.is_some()).unwrap_or(false);
        if !still_armed || thread.as_deref().is_some_and(|t| self.busy(t)) {
            return Ok(());
        }
        self.ctx.cut_off(&entry).await;
        if let Err(e) = self.ctx.unfork(&entry).await {
            if !e.is_missing() {
                let on = with_sub(&record, |sub| cloud_said(&sub.on)).unwrap_or_default();
                let name = record.borrow().name.clone();
                let line = sub_delete_refused_line(&on, &name, &e.failure_words(), SUB_DELETE_RETRY_MS / 60_000.0);
                self.ctx.warn(&line);
                let phase = phase_of(&record);
                entry.borrow_mut().delete_said = Some(DeleteSaid { phase, line });
                let now = self.ctx.now();
                with_sub(&record, |sub| sub.delete_at = Some(JsNumber(now + SUB_DELETE_RETRY_MS)));
                self.persist_record(&record).await?;
                self.told(id);
                self.arm(&entry);
                return Ok(());
            }
        }
        entry.borrow_mut().delete_said = None;
        self.release(&entry, None, false).await
    }

    /// sub-life.ts:432 (rule 13).
    pub(crate) fn reclaim(&self, workspace_id: &str) {
        let Some(entry) = self.ctx.live(workspace_id) else { return };
        let record = record_of(&entry);
        let armed = with_sub(&record, |sub| sub.delete_at.is_some() || sub.held.is_some()).unwrap_or(false);
        if !armed {
            return;
        }
        self.cancel_timer(workspace_id);
        with_sub(&record, |sub| {
            sub.delete_at = None;
            sub.held = None;
        });
        self.ctx.broker_lift(workspace_id);
        let snap = snapshot(&record);
        let ctx = self.ctx.clone();
        let id = workspace_id.to_string();
        self.ctx.spawn(
            async move {
                if let Err(e) = ctx.persist(&snap).await {
                    ctx.warn(&format!("sub-thread machine {id}: {}", e.message()));
                }
            }
            .boxed_local(),
        );
    }

    /// sub-life.ts:476 (rule 17).
    pub(crate) async fn forgotten(&self, entry: &Entry) -> AreaResult<()> {
        let record = record_of(entry);
        let id = record.borrow().id.clone();
        self.cancel_timer(&id);
        with_sub(&record, |sub| {
            sub.delete_at = None;
            sub.cloud_gone = Some(true);
        });
        self.run_late_read(entry);
        let machine_id = record.borrow().machine_id.clone();
        self.ctx.attach_dead(entry, &machine_id);
        self.persist_record(&record).await?;
        self.ctx.idle_forget(&id);
        Ok(())
    }

    /// sub-life.ts:540 `subDone`: a done on the record's chain.
    pub async fn sub_done(&self, workspace_id: &str) {
        self.serial(workspace_id, step_work(self.me.clone(), workspace_id.to_string(), Step::Done)).await;
    }

    /// sub-life.ts:491 (rule 18).
    pub async fn sub_read(&self, workspace_id: &str, since: f64) -> Option<SubResult> {
        let entry = self.ctx.live(workspace_id)?;
        let record = record_of(&entry);
        if !has_sub_record(&record) {
            return None;
        }
        let started = self.done_started.borrow().get(workspace_id).copied().unwrap_or(f64::NEG_INFINITY);
        if self.read_since(workspace_id, since).is_none() && phase_of(&record) != WorkspacePhase::Released && started < since {
            self.wait_for_read(workspace_id, since).await;
        }
        self.read_since(workspace_id, since)
    }

    /// sub-life.ts:520 (rule 19).
    pub async fn sub_line(&self, workspace_id: &str, since: f64) -> Option<String> {
        let result = self.sub_read(workspace_id, since).await?;
        let entry = self.ctx.live(workspace_id)?;
        let record = record_of(&entry);
        let sub = with_sub(&record, |sub| sub.clone())?;
        let thread_id = self.thread_of(workspace_id)?;
        if self.busy(&thread_id) {
            return None;
        }
        let phase = phase_of(&record);
        let snap = snapshot(&record);
        let kept_until = match &sub.held {
            Some(held) if phase != WorkspacePhase::Released => disk_kept_until(self.ctx.as_ref(), &snap, Some(held.at.0)),
            _ => None,
        };
        let delete_in_ms = match sub.delete_at {
            Some(at) if phase != WorkspacePhase::Released => Some((at.0 - self.ctx.now()).max(0.0)),
            _ => None,
        };
        Some(sub_result_line(&SubResultFacts {
            lead: lead_thread_of(&snap).unwrap_or_else(|| thread_id.clone()),
            thread_id,
            result,
            base: sub.base.branch.clone(),
            delete_in_ms,
            kept: kept_until.map(|until| KeptUntil { on: cloud_said(&sub.on), until }),
        }))
    }

    /// sub-life.ts:543 `subExpire`: the delete on the record's chain.
    pub async fn sub_expire(&self, workspace_id: &str) {
        self.serial(workspace_id, step_work(self.me.clone(), workspace_id.to_string(), Step::Expire)).await;
    }

    /// sub-life.ts:544 (rule 20).
    pub fn arm_subs(&self) {
        for entry in self.ctx.live_all() {
            let record = record_of(&entry);
            let Some((delete_at, held, cloud_gone)) = with_sub(&record, |sub| (sub.delete_at.is_some(), sub.held.is_some(), sub.cloud_gone == Some(true))) else {
                continue;
            };
            let (phase, id) = {
                let rec = record.borrow();
                (rec.phase, rec.id.clone())
            };
            if phase == WorkspacePhase::Released || cloud_gone {
                continue;
            }
            if delete_at {
                self.arm(&entry);
            } else if !held {
                drop(self.serial(&id, step_work(self.me.clone(), id.clone(), Step::Done)));
            }
        }
    }

    /// sub-life.ts:554 (rule 21).
    pub async fn sub_lost(&self, entry: &Entry, reason: &str) -> AreaResult<GoneOutcome> {
        self.release(entry, Some(reason.to_string()), false).await?;
        Ok(GoneOutcome::Settled)
    }

    /// sub-life.ts:558 (rule 21).
    pub async fn release_subs_on(&self, place_id: &str, forgot: bool) -> AreaResult<()> {
        for entry in self.ctx.live_all() {
            let record = record_of(&entry);
            let snap = snapshot(&record);
            if !has_sub_record(&record) || self.ctx.place_id_of(&snap).as_deref() != Some(place_id) {
                continue;
            }
            if forgot && phase_of(&record) != WorkspacePhase::Released {
                self.forgotten(&entry).await?;
            } else {
                self.release(&entry, None, true).await?;
            }
        }
        Ok(())
    }

    /// sub-life.ts:462 (rule 16).
    pub fn sub_unpushed(&self, entry: &Entry) -> Option<SubUnpushed> {
        let record = record_of(entry);
        let sub = with_sub(&record, |sub| sub.clone())?;
        let (phase, id) = {
            let rec = record.borrow();
            (rec.phase, rec.id.clone())
        };
        if phase == WorkspacePhase::Released || phase == WorkspacePhase::Queued {
            return None;
        }
        let thread_id = self.thread_of(&id)?;
        let ran = self.ctx.latest_on(&thread_id).and_then(|view| view.started_at).map(|at| at.0);
        let unread = match &sub.result {
            None => ran.is_some(),
            Some(result) => ran.is_some_and(|ran| ran > date_parse(&result.read_at)),
        };
        if unread {
            return Some(SubUnpushed { files: 0.0, commits: 0.0, read_failed: None, unread: Some(true) });
        }
        let held = sub.held.as_ref()?;
        Some(SubUnpushed {
            files: held.held.files.get() as f64,
            commits: held.held.commits.get() as f64,
            read_failed: sub.result.as_ref().and_then(|result| result.read_failed.clone()),
            unread: None,
        })
    }

    /// sub-life.ts:566 (rule 22).
    pub async fn sub_delete(&self, asked: &Entry, thread_id: &str, origin: Option<&Caller>, check: bool) -> Result<SubDeleted, HostError> {
        let asked_id = record_of(asked).borrow().id.clone();
        let tail_done = self.working.borrow().get(&asked_id).cloned();
        if let Some(tail_done) = tail_done {
            tail_done.await;
        }
        let entry = self.ctx.live(&asked_id).unwrap_or_else(|| asked.clone());
        let record = record_of(&entry);
        let sub = with_sub(&record, |sub| sub.clone()).ok_or_else(|| HostError::new(format!("{asked_id} is not a sub-thread")))?;
        let phase = phase_of(&record);
        if phase != WorkspacePhase::Queued && self.busy(thread_id) {
            return Err(HostError::with_kind(THREAD_WORKING_LINE, Kind::Conflict));
        }
        let held = self.sub_unpushed(&entry);
        if let Some(held) = &held {
            if scope_of(origin).is_some() {
                return Err(sub_held_delete_refusal(thread_id, held, &sub.branch));
            }
        }
        let (workspace_id, name) = {
            let rec = record.borrow();
            (rec.id.clone(), rec.name.clone())
        };
        let on = cloud_said(&sub.on);
        if check {
            let rows: Vec<SessionView> = self.ctx.session_views().into_iter().filter(|view| view.thread_id.as_deref() == Some(thread_id)).collect();
            let title = fold_threads(&rows).into_iter().next().map_or(name, |t| t.title);
            let question = sub_delete_question(&SubDeleteQuestion { thread_id: thread_id.to_string(), title, on: on.clone(), branch: sub.branch.clone(), held });
            return Ok(SubDeleted { workspace_id, threads: 1, machine: None, on, question: Some(question) });
        }
        self.ctx.delete_workspace(&workspace_id).await.map_err(AreaError::into_host)?;
        Ok(SubDeleted { workspace_id, threads: 1, machine: Some("deleted".to_string()), on, question: None })
    }

    /// sub-life.ts:584 (rule 23).
    pub fn refuse_sub_send(&self, entry: &Entry) -> Result<(), HostError> {
        let record = record_of(entry);
        match with_sub(&record, |sub| (sub.cloud_gone == Some(true), cloud_said(&sub.on))) {
            Some((true, on)) => Err(sub_cloud_gone_refusal(&on)),
            _ => Ok(()),
        }
    }

    /// sub-life.ts:587 (rule 23).
    pub fn refuse_sub_wake(&self, entry: &Entry) -> Result<(), HostError> {
        let record = record_of(entry);
        let Some((cloud_gone, on)) = with_sub(&record, |sub| (sub.cloud_gone == Some(true), cloud_said(&sub.on))) else { return Ok(()) };
        if cloud_gone {
            return Err(sub_cloud_gone_refusal(&on));
        }
        if phase_of(&record) == WorkspacePhase::Released {
            let (id, name) = {
                let rec = record.borrow();
                (rec.id.clone(), rec.name.clone())
            };
            let thread = self.thread_of(&id).unwrap_or(name);
            return Err(sub_released_refusal(&thread, &on));
        }
        Ok(())
    }

    /// sub-life.ts:592 (rule 23).
    pub fn lead_of_sub(&self, workspace_id: Option<&str>, origin: Option<&Caller>) -> bool {
        let Some(entry) = workspace_id.and_then(|id| self.ctx.live(id)) else { return false };
        let lead = lead_thread_of(&snapshot(&record_of(&entry)));
        match (lead, scope_of(origin)) {
            (Some(lead), Some(scope)) => scope.thread_id == lead,
            _ => false,
        }
    }

    /// The `session.end` listener (rule 14), exposed for the area's own registration.
    pub(crate) fn on_session_end(&self, event: &Value) {
        if event["type"] != "session.end" {
            return;
        }
        let Some(id) = event["workspaceId"].as_str() else { return };
        let armed = self.ctx.live(id).is_some_and(|entry| has_sub_record(&record_of(&entry)));
        if !armed {
            return;
        }
        let me = self.me.clone();
        let id = id.to_string();
        self.ctx.spawn(
            async move {
                tokio::task::yield_now().await;
                if let Some(area) = me.upgrade() {
                    drop(area.serial(&id, step_work(area.me.clone(), id.clone(), Step::Done)));
                }
            }
            .boxed_local(),
        );
    }

    fn cancel_timer(&self, id: &str) {
        let cancel = self.timers.borrow_mut().shift_remove(id);
        if let Some(cancel) = cancel {
            cancel();
        }
    }

    /// `ctx.push_head` of the record's thread (sub-life.ts:179).
    fn told(&self, workspace_id: &str) {
        if let Some(thread_id) = self.thread_of(workspace_id) {
            self.ctx.push_head(&thread_id);
        }
    }

    async fn persist_record(&self, record: &RefCell<WorkspaceRecord>) -> AreaResult<()> {
        self.ctx.persist(&snapshot(record)).await
    }

    fn run_late_read(&self, entry: &Entry) {
        let late = entry.borrow_mut().late_read.take();
        if let Some(late) = late {
            late();
        }
    }

    /// `delayMs(entry)` (sub-life.ts:199): the place's delete delay, read at each done.
    fn delay_ms(&self, record: &WorkspaceRecord) -> f64 {
        let place = self.ctx.place_id_of(record);
        self.ctx.delete_after_ms(place.as_deref()).unwrap_or(DELETE_AFTER_MS as f64)
    }

    fn is_live(&self, id: &str, entry: &Entry) -> bool {
        matches!(self.ctx.live(id), Some(live) if Rc::ptr_eq(&live, entry))
    }

    /// The steps of a done (sub-life.ts:340-381), each a return point.
    async fn done_steps(&self, id: &str) -> AreaResult<()> {
        let Some(entry) = self.ctx.live(id) else { return Ok(()) };
        let record = record_of(&entry);
        if !has_sub_record(&record) || phase_of(&record) == WorkspacePhase::Released {
            return Ok(());
        }
        let Some(thread_id) = self.thread_of(id) else { return Ok(()) };
        if self.busy(&thread_id) {
            return Ok(());
        }
        if phase_of(&record) != WorkspacePhase::Running {
            let prior_read = with_sub(&record, |sub| sub.result.as_ref().map(|r| r.read_at.clone())).flatten();
            if let Some(read_at) = prior_read {
                let started = self.ctx.latest_on(&thread_id).and_then(|view| view.started_at).map_or(0.0, |at| at.0);
                if started <= date_parse(&read_at) {
                    return Ok(());
                }
            }
        }
        let read = self.read_result(&entry).await;
        let refused = if sub_all_pushed(&read) { None } else { self.push_refused_in(id, &thread_id).await.ok().flatten() };
        let result = match refused {
            Some(refused) => SubResult { push_refused: Some(refused), ..read },
            None => read,
        };
        self.keep_session(&entry, &thread_id).await;
        if self.busy(&thread_id) || !self.is_live(id, &entry) {
            return Ok(());
        }
        let now = self.ctx.now();
        let delay = self.delay_ms(&snapshot(&record));
        let all_pushed = sub_all_pushed(&result);
        with_sub(&record, |sub| {
            sub.result = Some(result.clone());
            if all_pushed {
                sub.delete_at = Some(JsNumber(now + delay));
                sub.held = None;
            } else {
                sub.held = Some(held_of(&result, now));
                sub.delete_at = None;
            }
        });
        self.persist_record(&record).await?;
        self.ctx.push_head(&thread_id);
        self.arm(&entry);
        self.read_done(id, now);
        if phase_of(&record) != WorkspacePhase::Running {
            return Ok(());
        }
        if let Err(e) = self.ctx.nap_with(id, DONE_NAP).await {
            let until = with_sub(&record, |sub| sub.delete_at).flatten();
            let Some(until) = until else { return Ok(()) };
            if !self.is_live(id, &entry) || phase_of(&record) != WorkspacePhase::Running {
                return Ok(());
            }
            let machine = machine_of(&entry);
            self.ctx.warn(&format!(
                "sub-thread machine {} of {id} did not nap ({}); it runs until its delete",
                machine.id(),
                e.message()
            ));
            let backend = self.ctx.backend_for(&snapshot(&record))?;
            if let Some(lifecycle) = backend.lifecycle().filter(|lc| lc.has_backstop()) {
                let until_ms = (until.0 + SUB_STOP_AFTER_DELETE_MS) as i64;
                if let Err(x) = lifecycle.backstop(machine.as_ref(), until_ms, true).await {
                    self.ctx.warn(&format!("the stop timer of {} was not moved: {}", machine.id(), AreaError::Engine(x).message()));
                }
            }
        }
        Ok(())
    }

    /// sub-life.ts:238 (rule 7): the pack and its keep; a failure reaches the caller, which drops the copy it held.
    async fn pack_and_keep(&self, entry: &Entry, row: &SessionView, id: &str, file: &SessionFile, thread_id: &str, at: &str) -> AreaResult<()> {
        let record = record_of(entry);
        let machine = machine_of(entry);
        let packed = machine.exec(&sub_session_pack_command(file, at), exec_options()).await.map_err(AreaError::Engine)?;
        if packed.exit_code.get() == 0 && packed.stdout.contains("nofile") {
            self.ctx.delete_blob(SUB_SESSIONS, thread_id).await?;
            let why = format!("no session file for {id} under {}", file.folder);
            set_session(&record, SubSession { agent: row.harness.clone(), id: id.to_string(), kept: None, bytes: None, path: None, why: Some(why) });
            return Ok(());
        }
        let code = packed.exit_code.get();
        let (size, path) = match (code, size_line(&packed.stdout), path_line(&packed.stdout)) {
            (0, Some(size), Some(path)) => (size, path),
            _ => {
                let err = js_trim(&packed.stderr);
                let src = if err.is_empty() { js_trim(&packed.stdout) } else { err };
                return Err(AreaError::Host(HostError::new(format!("the pack exited {code}: {}", tail(src, 200)))));
            }
        };
        if size > SUB_SESSION_MAX_BYTES as f64 {
            self.ctx.delete_blob(SUB_SESSIONS, thread_id).await?;
            let why = format!("the agent's session was too large to keep ({})", fmt_bytes(size));
            set_session(
                &record,
                SubSession { agent: row.harness.clone(), id: id.to_string(), kept: None, bytes: Some(JsNumber(size)), path: None, why: Some(why) },
            );
            return Ok(());
        }
        let bytes = machine.read_bytes(at, Some(size as u64), None).await.map_err(AreaError::Engine)?;
        self.ctx.put_blob(SUB_SESSIONS, thread_id, bytes).await?;
        let now = self.ctx.now();
        set_session(
            &record,
            SubSession {
                agent: row.harness.clone(),
                id: id.to_string(),
                kept: Some(JsNumber(now)),
                bytes: Some(JsNumber(size)),
                path: Some(path),
                why: None,
            },
        );
        Ok(())
    }

    /// The read's state where it is a read of a time at or after `since` (sub-life.ts:492).
    fn read_since(&self, workspace_id: &str, since: f64) -> Option<SubResult> {
        let entry = self.ctx.live(workspace_id)?;
        let result = with_sub(&record_of(&entry), |sub| sub.result.clone()).flatten()?;
        (date_parse(&result.read_at) >= since).then_some(result)
    }

    /// Waits for a done's read, or SUB_LINE_WAIT_MS (sub-life.ts:497-511).
    async fn wait_for_read(&self, workspace_id: &str, since: f64) {
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        let tx = Rc::new(RefCell::new(Some(tx)));
        let timer: Rc<RefCell<Option<Cancel>>> = Rc::new(RefCell::new(None));
        let key = workspace_id.to_string();
        let me = self.me.clone();
        let wait = Rc::new_cyclic(|weak: &Weak<ReadWait>| {
            let weak = weak.clone();
            let (tx, timer, key, me) = (tx.clone(), timer.clone(), key.clone(), me.clone());
            ReadWait {
                since,
                go: Rc::new(move || {
                    let cancel = timer.borrow_mut().take();
                    if let Some(cancel) = cancel {
                        cancel();
                    }
                    if let (Some(area), Some(wait)) = (me.upgrade(), weak.upgrade()) {
                        area.forget_read_wait(&key, &wait);
                    }
                    let sender = tx.borrow_mut().take();
                    if let Some(sender) = sender {
                        let _ = sender.send(());
                    }
                }),
            }
        });
        self.read_waits.borrow_mut().entry(key.clone()).or_default().push(wait.clone());
        let go = wait.go.clone();
        let cancel = self.ctx.schedule(SUB_LINE_WAIT_MS, Box::new(move || go()));
        *timer.borrow_mut() = Some(cancel);
        let _ = rx.await;
    }

    fn forget_read_wait(&self, key: &str, wait: &Rc<ReadWait>) {
        let mut waits = self.read_waits.borrow_mut();
        if let Some(list) = waits.get_mut(key) {
            list.retain(|w| !Rc::ptr_eq(w, wait));
            if list.is_empty() {
                waits.shift_remove(key);
            }
        }
    }

    /// `readDone` (sub-life.ts:335): wakes every wait the done started at `started` answers.
    fn read_done(&self, id: &str, started: f64) {
        let waits: Vec<Rc<ReadWait>> = self.read_waits.borrow().get(id).cloned().unwrap_or_default();
        for wait in waits {
            if wait.since <= started {
                (wait.go)();
            }
        }
    }
}
