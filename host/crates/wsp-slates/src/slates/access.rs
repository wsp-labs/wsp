// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_access: slates.ts:294-381, 426-462, 476-555, 1095-1116, 1281-1288, 1450-1480, 1495-1563 at
//! 65f6b89c7. Spec: spec/slates_access.md.
use std::cell::Cell;
use std::collections::HashSet;
use std::future::Future;
use std::rc::{Rc, Weak};
use std::sync::OnceLock;

use futures::future::join_all;
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use regex::Regex;
use serde_json::Value;
use tokio::task::spawn_local;
use wsp_core::console;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::kept::{Kept, Record};
use wsp_wire::types::slate_wire::{SessionSlateEvent, SlateBy, SlateCause, SlateValuesEvent};
use wsp_wire::views::workspace::{scope_of, Caller};
use wsp_words::exit::{not_found_refusal, usage_refusal, HostError, Kind};
use wsp_words::words::computer::thread_word;

use super::batch::BatchOptions;
use super::model::{
    problem, refused, snap_of, trim_snapshots, usage, Approval, BatchBy, SlateRecord, SlateSnap, TurnMark, PIECES_NAMED, PROBLEMS_KEPT,
};
use super::{RecordRef, SlateEmit, SlatesInner, Tail, Views};
use crate::lazy_slates::SLATES;
use crate::pending::slate::{SlateProblem, SlateWrite};
use crate::pending::wire::{SlateEmpty, SlateView};
use crate::slate_mcp_on::box_slate_dir;
use crate::slate_runs::{restarted_record, RunApprovals, RunBy, RunRecord, RunState, HELD_APPROVAL};

/// The approvals the runs table reads, backed by the records.
pub struct RecordApprovals {
    pub(crate) area: Weak<SlatesInner>,
}

fn record_of_thread(area: &SlatesInner, thread_id: &str) -> Option<RecordRef> {
    area.records.borrow().get(thread_id).cloned()
}

impl RunApprovals for RecordApprovals {
    fn has(&self, thread_id: &str, key: &str) -> bool {
        let Some(area) = self.area.upgrade() else { return false };
        let Some(r) = record_of_thread(&area, thread_id) else { return false };
        let rec = r.borrow();
        rec.approvals.get(key).is_some_and(|approval| approval.state == "allowed")
    }

    fn allow(&self, thread_id: &str, key: &str) {
        let Some(area) = self.area.upgrade() else { return };
        let Some(r) = record_of_thread(&area, thread_id) else { return };
        let (run, cmd, scripts) = {
            let rec = r.borrow();
            area.approval_names(&rec, key)
        };
        let at = area.deps.clock().now();
        r.borrow_mut().approvals.insert(key.to_string(), Approval { state: "allowed".into(), at, run, cmd, scripts });
        let writer = area.clone();
        spawn_local(async move {
            let _ = writer.save(r).await;
        });
    }

    fn revoke(&self, thread_id: &str, key: &str) {
        let Some(area) = self.area.upgrade() else { return };
        let Some(r) = record_of_thread(&area, thread_id) else { return };
        if r.borrow_mut().approvals.shift_remove(key).is_none() {
            return;
        }
        let writer = area.clone();
        spawn_local(async move {
            let _ = writer.save(r).await;
        });
    }

    fn list(&self, thread_id: &str) -> Vec<String> {
        let Some(area) = self.area.upgrade() else { return Vec::new() };
        let Some(r) = record_of_thread(&area, thread_id) else { return Vec::new() };
        let rec = r.borrow();
        rec.approvals.iter().filter(|(_, approval)| approval.state == "allowed").map(|(key, _)| key.clone()).collect()
    }
}

fn coded(mut e: HostError, code: &str) -> HostError {
    e.code = Some(code.to_string());
    e
}

static PR_CHECKS: OnceLock<Regex> = OnceLock::new();

fn pr_checks() -> &'static Regex {
    PR_CHECKS.get_or_init(|| Regex::new(r"(?-u:\b)pr\.checks(?-u:\b)").expect("the pr.checks pattern"))
}

impl SlatesInner {
    /// The thread's chain (rule R2): `run` starts when the previous task settles; the caller gets its own result.
    pub(crate) fn serial<T: Clone + 'static, F: Future<Output = Result<T, HostError>> + 'static>(
        self: &Rc<Self>,
        thread_id: &str,
        run: impl FnOnce() -> F + 'static,
    ) -> impl Future<Output = Result<T, HostError>> + 'static {
        let prev: Option<Tail> = self.queues.borrow().get(thread_id).map(|(_, tail)| tail.clone());
        let task = async move {
            if let Some(prev) = prev {
                prev.await;
            }
            run().await
        }
        .boxed_local()
        .shared();
        spawn_local(task.clone());
        let id = self.chain_seq.get() + 1;
        self.chain_seq.set(id);
        let done = task.clone();
        let tail: Tail = async move {
            let _ = done.await;
        }
        .boxed_local()
        .shared();
        self.queues.borrow_mut().insert(thread_id.to_string(), (id, tail));
        task
    }

    pub(crate) async fn save(self: Rc<Self>, r: RecordRef) -> Result<(), HostError> {
        let now = self.deps.clock().now();
        let (thread_id, value) = {
            let mut rec = r.borrow_mut();
            rec.updated_at = now;
            let value = serde_json::to_value(&*rec).map_err(|e| HostError::new(e.to_string()))?;
            (rec.thread_id.clone(), value)
        };
        let store = self.deps.store();
        store.put(SLATES, &thread_id, value).await
    }

    pub(crate) fn folder_of(&self, thread_id: &str) -> Option<std::path::PathBuf> {
        self.deps.slates_dir().map(|dir| dir.join(thread_id))
    }

    pub(crate) fn moved(self: &Rc<Self>, thread_id: &str, run: &str, record: RunRecord) {
        if self.capturing.borrow().as_ref().is_some_and(|(t, r)| t == thread_id && r == run) {
            return;
        }
        let area = self.clone();
        let (t, name) = (thread_id.to_string(), run.to_string());
        let label = format!("the slate of thread {} lost a record of ${run}", thread_word(thread_id));
        let task = self.serial(thread_id, move || async move { area.run_moved(t, name, record).await });
        spawn_local(async move {
            if let Err(e) = task.await {
                console::warn(&format!("{label}: {}", e.message));
            }
        });
    }

    pub(crate) fn timed(self: &Rc<Self>, thread_id: &str, run: &str) {
        let area = self.clone();
        let (t, name) = (thread_id.to_string(), run.to_string());
        drop(self.serial(thread_id, move || async move {
            let found = record_of_thread(&area, &t);
            let Some(r) = found else { return Ok(()) };
            let declared = r.borrow().document.as_ref().is_some_and(|doc| doc.runs.contains_key(&name));
            if !declared {
                return Ok(());
            }
            let shown = area.windows.borrow().get(&t).copied().unwrap_or(0) > 0;
            if !shown && area.deps.settled(&t).await == Some(true) {
                area.skipped.borrow_mut().entry(t.clone()).or_default().insert(name.clone());
                return Ok(());
            }
            area.clone()
                .batch(r, Vec::new(), BatchBy::Timer, BatchOptions { starts: vec![(name, RunBy::Timer)], ..Default::default() })
                .await?;
            Ok(())
        }));
    }

    pub(crate) async fn ready(self: Rc<Self>) -> Result<(), HostError> {
        let existing = self.loaded.borrow().clone();
        let loading = match existing {
            Some(loading) => loading,
            None => {
                let area = self.clone();
                let loading = async move { area.load().await }.boxed_local().shared();
                *self.loaded.borrow_mut() = Some(loading.clone());
                loading
            }
        };
        loading.await
    }

    async fn load(self: Rc<Self>) -> Result<(), HostError> {
        self.deps.loaded().await?;
        let list = self.deps.store().list(SLATES).await?;
        for stored in list {
            if stored.get("schema").and_then(Value::as_i64) != Some(2) {
                continue;
            }
            let Some(thread_id) = stored.get("threadId").and_then(Value::as_str).map(String::from) else { continue };
            if self.records.borrow().contains_key(&thread_id) {
                continue;
            }
            let mut record: SlateRecord = serde_json::from_value(stored).map_err(|e| HostError::new(e.to_string()))?;
            record.revision.get_or_insert(record.version);
            self.records.borrow_mut().insert(thread_id, Rc::new(std::cell::RefCell::new(record)));
        }
        let all: Vec<RecordRef> = self.records.borrow().values().cloned().collect();
        for r in all {
            let thread_id = r.borrow().thread_id.clone();
            let area = self.clone();
            self.serial(&thread_id, move || async move { area.recover(r).await }).await?;
        }
        Ok(())
    }

    pub(crate) async fn recover(self: Rc<Self>, r: RecordRef) -> Result<(), HostError> {
        let thread_id = r.borrow().thread_id.clone();
        let now = self.deps.clock().now();
        let input = {
            let mut rec = r.borrow_mut();
            self.refresh_secrets(&mut rec);
            let Some(doc) = rec.document.as_ref() else { return Ok(()) };
            let mut input = Vec::new();
            for name in doc.runs.keys() {
                let Some(run) = rec.values.get(name).and_then(RunRecord::from_json) else { continue };
                if run.state() == Some(RunState::Running) {
                    input.push(SlateWrite { path: format!("${name}"), value: restarted_record(&run, now).to_json() });
                }
            }
            input
        };
        if input.is_empty() {
            self.clone().save(r.clone()).await?;
        } else {
            self.clone().batch(r.clone(), input, BatchBy::Run, BatchOptions::default()).await?;
        }
        self.arm_timers(&r.borrow(), &HashSet::new());
        let snap = r.borrow().clone();
        let views: Views = self.clone().views_for(&snap, &[]).await;
        let held: Vec<String> = {
            let rec = r.borrow();
            let declared: Vec<&String> = rec.document.as_ref().map(|doc| doc.runs.keys().collect()).unwrap_or_default();
            declared
                .into_iter()
                .filter(|name| {
                    rec.values
                        .get(*name)
                        .and_then(RunRecord::from_json)
                        .is_some_and(|run| run.state() == Some(RunState::Held) && run.why() == Some(HELD_APPROVAL))
                })
                .cloned()
                .collect()
        };
        for name in held {
            self.start_now(r.clone(), &name, RunBy::Person, &views, None);
        }
        if !self.runs().held(&thread_id).is_empty() {
            let area = self.clone();
            let again = r.clone();
            drop(self.serial(&thread_id, move || async move {
                let snap = again.borrow().clone();
                let views = area.clone().startable(&snap).await;
                let moved = area.held_again(again.clone(), &views);
                if !moved.is_empty() {
                    area.clone().save(again.clone()).await?;
                    area.push(&again, &moved);
                }
                Ok::<(), HostError>(())
            }));
        }
        Ok(())
    }

    pub(crate) fn refuse_sub_thread(&self, caller: Option<&Caller>) -> Result<(), HostError> {
        match scope_of(caller) {
            Some(scope) if scope.root_thread_id != scope.thread_id => Err(coded(
                HostError::with_kind(
                    "Z803 sub-thread: a thread another thread started has no slate; tell the thread that started you what to show, and it puts it on its own",
                    Kind::Usage,
                ),
                "Z803",
            )),
            _ => Ok(()),
        }
    }

    pub(crate) fn target_of(
        &self,
        thread_id: Option<&str>,
        turn_token: Option<&str>,
        caller: Option<&Caller>,
        write: bool,
    ) -> Result<String, HostError> {
        self.refuse_sub_thread(caller)?;
        if let Some(scope) = scope_of(caller) {
            let Some(asked) = thread_id.filter(|asked| *asked != scope.thread_id) else { return Ok(scope.thread_id) };
            if write {
                let text =
                    format!("Z800 thread-not-yours: a thread writes its own slate, and thread {} is not this one", thread_word(asked));
                return Err(coded(HostError::with_kind(text, Kind::Usage), "Z800"));
            }
            if !self.deps.under(&scope.thread_id).iter().any(|under| under == asked) {
                let text = format!("no thread {}", thread_word(asked));
                return Err(coded(not_found_refusal(&text), "Z801"));
            }
            return Ok(asked.to_string());
        }
        if let Some(asked) = thread_id {
            let known = self.records.borrow().contains_key(asked);
            if self.deps.thread(asked).is_none() && !known {
                return Err(not_found_refusal(&format!("no thread {}", thread_word(asked))));
            }
            return Ok(asked.to_string());
        }
        if let Some(token) = turn_token {
            return self.deps.thread_of_token(token);
        }
        Err(usage_refusal(
            "a slate belongs to a thread, and this line names none and runs inside no turn.",
            "Name the thread by id, as wsp threads lists it.",
        ))
    }

    /// `agent` with a token, else `person`.
    pub(crate) fn by_of(&self, caller: Option<&Caller>) -> SlateBy {
        if scope_of(caller).is_some() {
            SlateBy::Agent
        } else {
            SlateBy::Person
        }
    }

    pub(crate) async fn record_of(self: Rc<Self>, thread_id: &str) -> Result<Option<RecordRef>, HostError> {
        self.clone().ready().await?;
        Ok(record_of_thread(&self, thread_id))
    }

    pub(crate) async fn need_record(self: Rc<Self>, thread_id: &str) -> Result<RecordRef, HostError> {
        let found = self.clone().record_of(thread_id).await?;
        let Some(r) = found else {
            return Err(no_slate());
        };
        let fresh = {
            let rec = r.borrow();
            rec.document.is_none() && rec.version == 0
        };
        if fresh {
            return Err(no_slate());
        }
        Ok(r)
    }

    pub(crate) fn fresh_record(&self, thread_id: &str) -> Result<SlateRecord, HostError> {
        let facts = self.deps.thread(thread_id).ok_or_else(|| not_found_refusal(&format!("no thread {}", thread_word(thread_id))))?;
        Ok(SlateRecord {
            thread_id: thread_id.to_string(),
            workspace_id: facts.workspace_id,
            root_thread_id: facts.root_thread_id,
            schema: 2,
            version: 0,
            revision: Some(0),
            document: None,
            values: IndexMap::new(),
            empty: Some(SlateEmpty::None),
            turns: IndexMap::new(),
            kept: IndexMap::new(),
            comments: Vec::new(),
            approvals: IndexMap::new(),
            problems: Vec::new(),
            shown_once: false,
            updated_at: self.deps.clock().now(),
            previous: None,
            sends_allowed: None,
        })
    }

    pub(crate) fn check_version(&self, r: &SlateRecord, if_version: Option<i64>) -> Result<(), HostError> {
        match if_version {
            Some(expected) if expected != r.version => Err(refused(
                &problem(
                    "V750",
                    "version-behind",
                    &format!("the slate is at version {}, not {expected}; read it and write again", r.version),
                    None,
                ),
                Kind::Conflict,
            )),
            _ => Ok(()),
        }
    }

    pub(crate) fn announce(&self, r: &RecordRef, cause: SlateCause, by: SlateBy, pieces: &[String], run: Option<&str>) {
        let (thread_id, workspace_id, version) = {
            let rec = r.borrow();
            (rec.thread_id.clone(), rec.workspace_id.clone(), rec.version)
        };
        let facts = self.deps.thread(&thread_id);
        let event = SessionSlateEvent {
            workspace_id,
            session_id: facts.as_ref().map_or_else(|| thread_id.clone(), |f| f.session_id.clone()),
            at: None,
            turn_id: facts.as_ref().and_then(|f| f.turn_id.clone()),
            thread_id,
            pos: None,
            cause,
            version: JsInt::from(version),
            by,
            pieces: pieces.iter().take(PIECES_NAMED).cloned().collect(),
            run: run.map(String::from),
        };
        self.deps.record(event);
    }

    pub(crate) fn push(&self, r: &RecordRef, names: &[String]) {
        let event = {
            let rec = r.borrow();
            let mut values: IndexMap<String, Kept> = IndexMap::new();
            let mut seen: IndexSet<&String> = IndexSet::new();
            for name in names {
                if !seen.insert(name) {
                    continue;
                }
                if let Some(value) = rec.values.get(name) {
                    values.insert(format!("${name}"), Kept(value.clone()));
                }
            }
            if values.is_empty() {
                return;
            }
            SlateValuesEvent {
                workspace_id: rec.workspace_id.clone(),
                thread_id: rec.thread_id.clone(),
                version: JsInt::from(rec.version),
                revision: JsInt::from(rec.rev()),
                values: Record(values),
            }
        };
        self.deps.emit(SlateEmit::Values(event));
    }

    pub(crate) fn keep_problems(&self, r: &mut SlateRecord, found: &[SlateProblem]) {
        if found.is_empty() {
            return;
        }
        let mut all = std::mem::take(&mut r.problems);
        all.extend(found.iter().cloned());
        let skip = all.len().saturating_sub(PROBLEMS_KEPT);
        r.problems = all.into_iter().skip(skip).collect();
    }

    pub(crate) async fn settled(self: Rc<Self>) {
        let deliveries: Vec<Tail> = std::mem::take(&mut *self.deliveries.borrow_mut());
        join_all(deliveries).await;
        for _ in 0..50 {
            let pending: Vec<(u64, Tail)> = self.queues.borrow().values().cloned().collect();
            join_all(pending.iter().map(|(_, tail)| tail.clone())).await;
            let now: Vec<u64> = self.queues.borrow().values().map(|(id, _)| *id).collect();
            let before: Vec<u64> = pending.iter().map(|(id, _)| *id).collect();
            if now == before {
                return;
            }
        }
    }

    pub(crate) fn close(&self) {
        self.runs().close();
        self.mcp().close();
        self.ledger().close();
    }

    pub(crate) async fn get(self: Rc<Self>, thread_id: &str) -> Result<Option<SlateView>, HostError> {
        let found = self.clone().record_of(thread_id).await?;
        Ok(found.map(|r| {
            let rec = r.borrow();
            self.view_of(&rec)
        }))
    }

    pub(crate) async fn shown(self: Rc<Self>, thread_id: &str) -> Result<(), HostError> {
        let area = self.clone();
        let id = thread_id.to_string();
        self.serial(thread_id, move || async move {
            let found = area.clone().record_of(&id).await?;
            let Some(r) = found else { return Ok(()) };
            if r.borrow().shown_once {
                return Ok(());
            }
            r.borrow_mut().shown_once = true;
            area.save(r).await
        })
        .await
    }

    pub(crate) fn subscribe(self: &Rc<Self>, thread_id: &str, sources: &[String]) -> Rc<dyn Fn()> {
        {
            let mut holds = self.holds.borrow_mut();
            let held = holds.entry(thread_id.to_string()).or_default();
            for source in sources {
                *held.entry(source.clone()).or_insert(0) += 1;
            }
        }
        *self.windows.borrow_mut().entry(thread_id.to_string()).or_insert(0) += 1;
        self.runs().shown(thread_id, true);
        self.mcp().shown(thread_id, true);
        let skipped = self.skipped.borrow_mut().remove(thread_id).unwrap_or_default();
        for run in skipped {
            self.timed(thread_id, &run);
        }
        if sources.iter().any(|source| source == "pr") {
            let found = record_of_thread(self, thread_id);
            if let Some(r) = found {
                let workspace_id = r.borrow().workspace_id.clone();
                self.deps.watch_pr(&workspace_id);
            }
        }
        let weak: Weak<SlatesInner> = Rc::downgrade(self);
        let thread = thread_id.to_string();
        let held_sources = sources.to_vec();
        let released = Cell::new(false);
        Rc::new(move || {
            if released.replace(true) {
                return;
            }
            if let Some(area) = weak.upgrade() {
                area.release(&thread, &held_sources);
            }
        })
    }

    fn release(&self, thread_id: &str, sources: &[String]) {
        {
            let mut windows = self.windows.borrow_mut();
            let left = windows.get(thread_id).copied().unwrap_or(1) - 1;
            windows.insert(thread_id.to_string(), left.max(0));
        }
        let none_held = {
            let mut holds = self.holds.borrow_mut();
            match holds.get_mut(thread_id) {
                Some(held) => {
                    for source in sources {
                        let left = held.get(source).copied().unwrap_or(1) - 1;
                        if left <= 0 {
                            held.shift_remove(source);
                        } else {
                            held.insert(source.clone(), left);
                        }
                    }
                    held.is_empty()
                }
                None => true,
            }
        };
        if none_held {
            self.runs().shown(thread_id, false);
            self.mcp().shown(thread_id, false);
        }
    }

    pub(crate) async fn turn_ended(self: Rc<Self>, thread_id: &str, turn_id: &str) -> Result<(), HostError> {
        self.clone().ready().await?;
        let area = self.clone();
        let (thread, turn) = (thread_id.to_string(), turn_id.to_string());
        self.serial(thread_id, move || async move {
            let Some(r) = record_of_thread(&area, &thread) else { return Ok(()) };
            let now = area.deps.clock().now();
            {
                let mut rec = r.borrow_mut();
                let key = rec.rev().to_string();
                if !rec.kept.contains_key(&key) {
                    let snap = snap_of(&rec);
                    rec.kept.insert(key, snap);
                }
                rec.turns.shift_remove(&turn);
                let mark = TurnMark { at: now, version: rec.version, revision: rec.rev() };
                rec.turns.insert(turn, mark);
                trim_snapshots(&mut rec);
            }
            area.save(r).await
        })
        .await
    }

    pub(crate) async fn rewound(self: Rc<Self>, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), HostError> {
        self.clone().ready().await?;
        let area = self.clone();
        let (thread, turn, cut) = (thread_id.to_string(), turn_id.to_string(), cut.to_vec());
        self.serial(thread_id, move || async move {
            let Some(r) = record_of_thread(&area, &thread) else { return Ok(()) };
            {
                let mut rec = r.borrow_mut();
                let at_turn = rec.turns.get(&turn).map(|mark| mark.revision);
                let snap = at_turn.and_then(|revision| rec.kept.get(&revision.to_string()).cloned()).unwrap_or_else(|| SlateSnap {
                    document: None,
                    values: rec.values.clone(),
                    empty: Some(SlateEmpty::RewoundBefore),
                });
                area.become_snap(&mut rec, snap);
                for id in &cut {
                    rec.turns.shift_remove(id);
                }
                trim_snapshots(&mut rec);
                rec.version += 1;
                let revision = rec.rev() + 1;
                rec.revision = Some(revision);
            }
            area.clone().save(r.clone()).await?;
            area.announce(&r, SlateCause::Restore, SlateBy::Host, &[], None);
            let names: Vec<String> = r.borrow().values.keys().cloned().collect();
            area.push(&r, &names);
            Ok(())
        })
        .await
    }

    pub(crate) async fn forget(self: Rc<Self>, thread_id: &str, keep: bool) -> Result<(), HostError> {
        let area = self.clone();
        let thread = thread_id.to_string();
        self.serial(thread_id, move || async move {
            area.runs().drop_thread(&thread, keep);
            area.mcp().drop_thread(&thread, keep);
            area.pins().drop_thread(&thread);
            area.records.borrow_mut().shift_remove(&thread);
            if let Some(dir) = area.folder_of(&thread) {
                let _ = std::fs::remove_dir_all(dir);
            }
            if let Some(machine) = area.deps.machine_of(&thread) {
                let ledger = area.ledger();
                let path = box_slate_dir(&thread);
                spawn_local(async move {
                    ledger.remove(machine, &path).await;
                });
            }
            area.holds.borrow_mut().remove(&thread);
            area.skipped.borrow_mut().remove(&thread);
            area.windows.borrow_mut().remove(&thread);
            area.writes.delete(&thread);
            area.press_sent_at.borrow_mut().remove(&thread);
            area.reaction_sent_at.borrow_mut().remove(&thread);
            area.event_buckets.borrow_mut().remove(&thread);
            area.views_now.borrow_mut().remove(&thread);
            let prefix = format!("{thread}\u{0}");
            area.started_by.borrow_mut().retain(|key, _| !key.starts_with(&prefix));
            let store = area.deps.store();
            store.delete(SLATES, &thread).await
        })
        .await
    }

    pub(crate) fn watches_pr(&self, workspace_id: &str) -> bool {
        let records: Vec<RecordRef> = self.records.borrow().values().cloned().collect();
        for r in records {
            let rec = r.borrow();
            if rec.workspace_id != workspace_id {
                continue;
            }
            let pr_held = self.holds.borrow().get(&rec.thread_id).and_then(|held| held.get("pr")).is_some_and(|n| *n > 0);
            if pr_held {
                return true;
            }
            if let Some(doc) = rec.document.as_ref() {
                let text = serde_json::to_string(doc).unwrap_or_default();
                if pr_checks().is_match(&text) {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn close_servers(&self, thread_id: &str) {
        self.mcp().close_servers(thread_id);
    }

    pub(crate) fn napped(&self, workspace_id: &str) {
        let threads: Vec<String> = self
            .records
            .borrow()
            .values()
            .filter(|r| r.borrow().workspace_id == workspace_id)
            .map(|r| r.borrow().thread_id.clone())
            .collect();
        for thread in threads {
            self.mcp().close_servers(&thread);
        }
    }
}

fn no_slate() -> HostError {
    usage(&problem("Z802", "no-slate", "this thread has no slate yet; write one with slate_write", None))
}
