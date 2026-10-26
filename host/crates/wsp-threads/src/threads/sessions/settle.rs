// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:209-243, 1112-1173: settle, mark, restore, and the check a restart runs. Spec: spec/sessions_settle.md.
use std::rc::Rc;

use indexmap::IndexSet;
use serde_json::{json, Map, Value};
use wsp_core::types::api::SettleAsk;
use wsp_core::types::internal::ThreadRecord;
use wsp_wire::types::general_prefs::SettleAfter;
use wsp_wire::views::session::{SessionStatus, ThreadMarks, ThreadMarksPinned};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::{SessionRestoreResult, SessionSettleResult, SessionSettleResultLeftItem, SettledThread};
use wsp_words::exit::{not_found_refusal, refusal, usage_refusal, HostError, Kind};
use wsp_words::words::computer::{
    not_under_line, replaced_already_fix, replaced_already_line, replaces_working_fix, replaces_working_line, subagent_settle_line, thread_word,
    NOT_UNDER_FIX, SETTLE_ALREADY, SETTLE_WORKING, SUBAGENT_SETTLE_FIX,
};

use super::{SessionsArea, StampsOf};
use crate::activity::MeetVerb;

/// What a settle and a restore read of the threads named (sessions.ts:233).
pub struct SettleReads {
    pub busy: Rc<dyn Fn(&str) -> bool>,
    pub settled: Rc<dyn Fn(&str) -> bool>,
    pub finished: Rc<dyn Fn(&str) -> bool>,
    /// `{ threadId, title }` of each id, in order.
    pub titled: Rc<dyn Fn(&[String]) -> Vec<(String, String)>>,
}

/// `SETTLE_MS` (protocol general-prefs.ts:38): the quiet time a settled thread waits, None for `never`.
fn settle_ms(after: SettleAfter) -> Option<f64> {
    match after {
        SettleAfter::V15m => Some(15.0 * 60_000.0),
        SettleAfter::V1h => Some(60.0 * 60_000.0),
        SettleAfter::V2h => Some(2.0 * 60.0 * 60_000.0),
        SettleAfter::V1d => Some(24.0 * 60.0 * 60_000.0),
        SettleAfter::Never => None,
    }
}

/// A stamp's value for a key the TS leaves undefined: the key stays, as null, and the writer takes it off.
fn stamp_or_null(value: Option<Value>) -> Value {
    value.unwrap_or(Value::Null)
}

impl SessionsArea {
    /// sessions.ts:213. Rules E1, E2.
    pub async fn settle_reads(&self, thread_ids: &[String], origin: Option<&Caller>) -> Result<SettleReads, HostError> {
        self.ctx.ready().await?;
        for id in thread_ids {
            let records = self.ctx.records();
            let has_record = records.borrow().contains_key(id);
            if self.ctx.thread_facts(id).is_none() && !has_record {
                if self.ctx.subagent_known(id) {
                    return Err(refusal(&subagent_settle_line(id), SUBAGENT_SETTLE_FIX, Some(Kind::Usage)));
                }
                return Err(not_found_refusal(&format!("no thread {}", thread_word(id))));
            }
            if !self.ctx.settles_thread(id, origin) {
                if self.ctx.drives_thread(Some(id), origin) {
                    return Err(refusal(&not_under_line(id), NOT_UNDER_FIX, Some(Kind::Usage)));
                }
                return Err(not_found_refusal(&format!("no thread {}", thread_word(id))));
            }
        }
        let settle_ms = settle_ms(self.ctx.preferences().await?.settle_after);
        let ctx = self.ctx.clone();
        let busy: Rc<dyn Fn(&str) -> bool> = Rc::new(move |id: &str| {
            ctx.thread_facts(id).is_some_and(|t| t.status == SessionStatus::Running || t.asking.is_some() || t.waiting_on.is_some())
        });
        let ctx = self.ctx.clone();
        let settled: Rc<dyn Fn(&str) -> bool> = Rc::new(move |id: &str| ctx.settled_now(id, settle_ms));
        let ctx = self.ctx.clone();
        let finished: Rc<dyn Fn(&str) -> bool> = Rc::new(move |id: &str| {
            ctx.thread_facts(id).is_some_and(|t| t.status == SessionStatus::Completed || t.status == SessionStatus::Interrupted)
        });
        let ctx = self.ctx.clone();
        let titled: Rc<dyn Fn(&[String]) -> Vec<(String, String)>> = Rc::new(move |ids: &[String]| {
            ids.iter().map(|id| (id.clone(), ctx.thread_facts(id).map(|t| t.title).unwrap_or_else(|| thread_word(id)))).collect()
        });
        Ok(SettleReads { busy, settled, finished, titled })
    }

    /// sessions.ts:238. Rule E3.
    pub async fn replaceable_check(&self, thread_id: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        let reads = self.settle_reads(&[thread_id.to_string()], origin).await?;
        if std::iter::once(thread_id.to_string()).chain(self.ctx.tree_under(thread_id)).any(|id| (reads.busy)(&id)) {
            return Err(refusal(&replaces_working_line(thread_id), &replaces_working_fix(thread_id), Some(Kind::Usage)));
        }
        if let Some(restart) = self.ctx.restarts().get(thread_id) {
            return Err(refusal(&replaced_already_line(thread_id, restart), &replaced_already_fix(restart), Some(Kind::Usage)));
        }
        Ok(())
    }

    /// sessions.ts:1112. Rules E4-E7.
    pub async fn settle_threads(&self, thread_ids: Vec<String>, origin: Option<Caller>, ask: SettleAsk) -> Result<SessionSettleResult, HostError> {
        let reads = self.settle_reads(&thread_ids, origin.as_ref()).await?;
        let mut take: Vec<String> = Vec::new();
        let mut left: Vec<SessionSettleResultLeftItem> = Vec::new();
        let mut named: Vec<String> = Vec::new();
        let mut add = |from: &str, ids: Vec<String>, take: &mut Vec<String>| {
            if !ids.is_empty() {
                named.push(from.to_string());
            }
            for id in ids {
                if !take.contains(&id) {
                    take.push(id);
                }
            }
        };
        let distinct: IndexSet<String> = thread_ids.iter().cloned().collect();
        for one in &distinct {
            let mut tree = vec![one.clone()];
            tree.extend(self.ctx.tree_under(one));
            let works = |id: &str| std::iter::once(id.to_string()).chain(self.ctx.tree_under(id)).any(|t| (reads.busy)(&t));
            if ask.finished == Some(true) {
                let stays: Vec<String> = tree[1..].iter().filter(|id| (reads.finished)(id) && !(reads.settled)(id)).cloned().collect();
                left.extend(stays.iter().filter(|id| works(id)).map(|id| SessionSettleResultLeftItem { thread_id: id.clone(), why: SETTLE_WORKING.to_string() }));
                add(one, stays.into_iter().filter(|id| !works(id)).collect(), &mut take);
            } else if tree.iter().any(|id| (reads.busy)(id)) {
                left.push(SessionSettleResultLeftItem { thread_id: one.clone(), why: SETTLE_WORKING.to_string() });
            } else {
                if (reads.settled)(one) {
                    left.push(SessionSettleResultLeftItem { thread_id: one.clone(), why: SETTLE_ALREADY.to_string() });
                }
                add(one, tree.into_iter().filter(|id| !(reads.settled)(id)).collect(), &mut take);
            }
        }
        let ended = match ask.processes {
            None => Vec::new(),
            Some(processes) => self.ctx.activity().meet(&take, processes, MeetVerb::Settle, origin.clone()).await?,
        };
        let at = {
            let stamped = (self.ctx.clock().now() as f64).max(self.last_settle_at.get() + 1.0);
            self.last_settle_at.set(stamped);
            stamped
        };
        if !take.is_empty() {
            let mut marked: Vec<String> = take.clone();
            for id in &named {
                if !marked.contains(id) {
                    marked.push(id.clone());
                }
            }
            let moved = take.clone();
            let named_now = named.clone();
            let stamps: StampsOf = Rc::new(move |id: &str| {
                let mut stamp = Map::new();
                if moved.iter().any(|m| m == id) {
                    stamp.insert("readAt".into(), json!(at));
                    stamp.insert("settledAt".into(), json!(at));
                }
                if named_now.iter().any(|m| m == id) {
                    stamp.insert("settleNamedAt".into(), json!(at));
                }
                stamp
            });
            self.ctx.mark(&marked, stamps, origin.as_ref()).await?;
        }
        let settled = (reads.titled)(&take).into_iter().map(|(thread_id, title)| SettledThread { thread_id, title }).collect();
        Ok(SessionSettleResult { settled, left, ended: (!ended.is_empty()).then_some(ended) })
    }

    /// sessions.ts:1145. Rules E8, E9.
    pub async fn mark_threads(&self, thread_ids: Vec<String>, marks: ThreadMarks, origin: Option<Caller>) -> Result<(), HostError> {
        let at = self.ctx.clock().now() as f64;
        let pinned_number = match &marks.pinned {
            Some(ThreadMarksPinned::V1(n)) => Some(n.clone()),
            _ => None,
        };
        let order_number = marks.order.clone().flatten();
        for key in [pinned_number, order_number].into_iter().flatten() {
            if !key.0.is_finite() {
                return Err(usage_refusal(
                    &format!("a thread's place in the sidebar is a number, and {} is not one.", key.js_string()),
                    "Send a finite number, or leave the key out.",
                ));
            }
        }
        if let Some(resume) = marks.resume_at_reset {
            self.ctx.arm_resume(&thread_ids, resume, origin.as_ref()).await?;
        }
        let mut stamps = Map::new();
        if let Some(pinned) = &marks.pinned {
            let value = match pinned {
                ThreadMarksPinned::V1(n) => Some(json!(n.0)),
                ThreadMarksPinned::V0(true) => Some(json!(at)),
                ThreadMarksPinned::V0(false) => None,
            };
            stamps.insert("pinnedAt".into(), stamp_or_null(value));
        }
        if let Some(order) = &marks.order {
            stamps.insert("order".into(), stamp_or_null(order.as_ref().map(|n| json!(n.0))));
        }
        if let Some(folded) = marks.folded {
            stamps.insert("foldedAt".into(), stamp_or_null(folded.then(|| json!(at))));
        }
        match &marks.snoozed_until {
            None => {}
            Some(None) => {
                stamps.insert("snoozedUntil".into(), Value::Null);
            }
            Some(Some(until)) => {
                stamps.insert("snoozedUntil".into(), json!(until.0));
                stamps.insert("readAt".into(), json!(at));
            }
        }
        match &marks.section {
            None => {}
            Some(None) => {
                stamps.insert("section".into(), Value::Null);
            }
            Some(Some(placement)) => {
                let value = serde_json::to_value(placement).map_err(|e| HostError::new(e.to_string()))?;
                stamps.insert("section".into(), value);
            }
        }
        if marks.resume_at_reset.is_none() || !stamps.is_empty() {
            let stamps: StampsOf = Rc::new(move |_id: &str| stamps.clone());
            self.ctx.mark(&thread_ids, stamps, origin.as_ref()).await?;
        }
        Ok(())
    }

    /// sessions.ts:1160. Rule E10.
    pub async fn restore_threads(&self, thread_ids: Vec<String>, origin: Option<Caller>) -> Result<SessionRestoreResult, HostError> {
        let reads = self.settle_reads(&thread_ids, origin.as_ref()).await?;
        let records = self.ctx.records();
        let stamp_of = |record: Option<&ThreadRecord>| -> f64 {
            let settled = record.and_then(|r| r.settled_at.map(|n| n.0)).unwrap_or(0.0);
            let named = record.and_then(|r| r.settle_named_at.map(|n| n.0)).unwrap_or(0.0);
            settled.max(named)
        };
        let mut back: IndexSet<String> = IndexSet::new();
        for one in &thread_ids {
            let record = records.borrow().get(one).cloned();
            let at = stamp_of(record.as_ref());
            back.insert(one.clone());
            for id in self.ctx.tree_under(one) {
                let settled_at = records.borrow().get(&id).and_then(|r| r.settled_at.map(|n| n.0));
                if at > 0.0 && settled_at == Some(at) {
                    back.insert(id);
                }
            }
        }
        let back: Vec<String> = back.into_iter().filter(|id| (reads.settled)(id)).collect();
        if !back.is_empty() {
            let now = self.ctx.clock().now();
            let stamps: StampsOf = Rc::new(move |_id: &str| {
                let mut stamp = Map::new();
                stamp.insert("readAt".into(), json!(now));
                stamp.insert("settledAt".into(), Value::Null);
                stamp
            });
            self.ctx.mark(&back, stamps, origin.as_ref()).await?;
        }
        let restored = (reads.titled)(&back).into_iter().map(|(thread_id, title)| SettledThread { thread_id, title }).collect();
        Ok(SessionRestoreResult { restored })
    }
}
