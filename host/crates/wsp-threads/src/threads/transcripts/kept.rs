// SPDX-License-Identifier: AGPL-3.0-only
//! transcripts_kept: sent images, the sessions document, daemon tokens. transcripts.ts:17-92, 513-545, 590-657,
//! 716-735. Spec: spec/transcripts_kept.md.
use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use serde_json::{json, Map, Value};
use wsp_core::clock::Clock;
use wsp_core::console;
use wsp_core::context::Joined;
use wsp_core::daemon_token::{daemon_token_for, daemon_token_path_of};
use wsp_core::types::internal::{KeptImage, KeptImages, SessionEntry, ThreadRecord};
use wsp_engine::machine::Machine;
use wsp_store::store::Store;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::harness_picks::{model_picks, recorded_picks, RanPicks};
use wsp_wire::views::session::{thread_key_of, thread_ran, SessionStatus, SessionView};
use wsp_wire::views::session_events::TurnStatus;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::attachments::{attachment_key, is_image};
use wsp_words::exit::HostError;

use super::super::threads::{Entry, Row};
use super::TranscriptsArea;

pub use wsp_core::types::internal::{ATTACHMENTS, ATTACHMENT_KEYS, DAEMON_TOKEN_MISS_TTL_MS, SESSIONS, SESSION_INDEX_CAP};
/// One step of the kept-images chain: the document as read, answering the document to write, or `None` where the
/// step wrote its own (transcripts.ts:24).
pub type KeptStep = Box<dyn FnOnce(KeptImages) -> LocalBoxFuture<'static, Result<Option<KeptImages>, HostError>>>;

/// Which threads' images `drop_sent_images` takes (transcripts.ts:86).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreadsNamed {
    All,
    These(Vec<String>),
}

/// transcripts.ts:67 `imageKeysOf`: K5.
pub fn image_keys_of(e: &SessionEvent) -> Vec<String> {
    let (thread, request, attachments) = match e {
        SessionEvent::SessionStartEvent(s) => (s.thread_id.as_deref(), s.request_id.as_deref(), s.attachments.as_deref()),
        SessionEvent::SessionSteerEvent(s) => (s.thread_id.as_deref(), s.request_id.as_deref(), s.attachments.as_deref()),
        _ => return Vec::new(),
    };
    let (Some(thread), Some(attachments)) = (thread, attachments) else {
        return Vec::new();
    };
    attachments
        .iter()
        .enumerate()
        .filter(|(_, a)| is_image(&a.media_type))
        .filter_map(|(index, _)| attachment_key(thread, request, index as f64))
        .collect()
}

/// A document or a value as the store keeps it. A value that does not serialize is never one these types hold.
fn to_json<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// Node's `Buffer.from(text, "base64")`: the alphabet's characters in order, any other skipped, the first `=` ends it.
fn base64_decode(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for c in text.bytes() {
        if c == b'=' {
            break;
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

/// Runs one step of the kept-images chain against the document it reads, and writes what it answers.
async fn run_kept_step(store: Rc<dyn Store>, workspace_id: &str, step: KeptStep) -> Result<(), HostError> {
    let held: KeptImages = match store.get(ATTACHMENT_KEYS, workspace_id).await? {
        Some(v) => serde_json::from_value(v).map_err(|e| HostError::new(e.to_string()))?,
        None => KeptImages::default(),
    };
    let Some(now) = step(held).await? else {
        return Ok(());
    };
    if now.threads.is_empty() {
        store.delete(ATTACHMENT_KEYS, workspace_id).await
    } else {
        store.put(ATTACHMENT_KEYS, workspace_id, to_json(&now)).await
    }
}

/// The lowercase hex of bytes, two digits each.
fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A row's live fields and its turn's, in the order the document writes them (transcripts.ts:631-652).
fn session_json(s: &SessionEntry) -> Value {
    let mut row = match to_json(&s.view) {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    row.insert("turnId".into(), json!(s.turn_id));
    if let Some(notify) = &s.notify {
        row.insert("notify".into(), to_json(notify));
    }
    if let Some(by) = &s.notify_by {
        row.insert("notifyBy".into(), to_json(by));
    }
    if let Some(road) = &s.notify_road {
        row.insert("notifyRoad".into(), to_json(road));
    }
    if s.view.status == SessionStatus::Running {
        if let Some(live) = &s.turn_live {
            if let Some(reply) = &live.reply {
                row.insert("reply".into(), to_json(reply));
            }
            if let Some(told) = &live.told {
                row.insert("told".into(), to_json(told));
            }
            if let Some(steered) = &live.steered {
                row.insert("steered".into(), to_json(steered));
            }
            if let Some(waited) = &live.waited {
                row.insert("waited".into(), to_json(waited));
            }
            if let Some(since) = &live.waiting_since {
                row.insert("waitingSince".into(), to_json(since));
            }
        }
        if let Some(run) = &s.run {
            row.insert("run".into(), to_json(run));
        }
        if let Some(from) = &s.from {
            row.insert("from".into(), to_json(from));
        }
        if let Some(asked) = &s.asked {
            row.insert("asked".into(), to_json(asked));
        }
        if let Some(token) = &s.turn_token {
            row.insert("turnToken".into(), to_json(token));
        }
        if let Some(device) = &s.scope_device_id {
            row.insert("scopeDeviceId".into(), to_json(device));
        }
    }
    if let Some(snapshot) = &s.snapshot {
        row.insert("snapshot".into(), to_json(snapshot));
    }
    Value::Object(row)
}

/// The thread status a finished row leaves on its thread's record (`ended`).
fn ended_of(status: SessionStatus) -> Option<TurnStatus> {
    match status {
        SessionStatus::Completed => Some(TurnStatus::Completed),
        SessionStatus::Interrupted => Some(TurnStatus::Interrupted),
        SessionStatus::Failed => Some(TurnStatus::Failed),
        SessionStatus::Running => None,
    }
}

fn started_at(view: &SessionView) -> f64 {
    view.started_at.as_ref().map_or(0.0, |j| j.0)
}

impl TranscriptsArea {
    /// transcripts.ts:24 `onKept`: K1.
    pub fn on_kept(&self, workspace_id: &str, step: KeptStep) -> LocalBoxFuture<'static, ()> {
        let previous = self.kept_writes.borrow().get(workspace_id).cloned();
        let store = self.ctx.store();
        let ws = workspace_id.to_string();
        let link: Joined = async move {
            if let Some(previous) = previous {
                previous.await;
            }
            if let Err(e) = run_kept_step(store, &ws, step).await {
                console::warn(&format!("the images {ws}'s messages carried were not kept as asked: {}", e.message));
            }
        }
        .boxed_local()
        .shared();
        self.kept_writes.borrow_mut().insert(workspace_id.to_string(), link.clone());
        tokio::task::spawn_local(link.clone());
        link.boxed_local()
    }

    /// transcripts.ts:39: K2.
    pub fn keep_sent_images(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: Option<&str>,
        attachments: &[Attachment],
    ) -> LocalBoxFuture<'static, ()> {
        let kept: Vec<(String, String, Vec<u8>)> = attachments
            .iter()
            .enumerate()
            .filter(|(_, a)| is_image(&a.media_type))
            .filter_map(|(index, a)| {
                attachment_key(thread_id, request_id, index as f64).map(|key| (key, a.media_type.clone(), base64_decode(&a.bytes)))
            })
            .collect();
        if kept.is_empty() {
            return futures::future::ready(()).boxed_local();
        }
        let thread = thread_id.to_string();
        let ws = workspace_id.to_string();
        let store = self.ctx.store();
        let step: KeptStep = Box::new(move |held: KeptImages| {
            async move {
                let mut threads = held.threads;
                // The entry before the bytes: a crash between the two leaves an entry whose blob is missing, which reads
                // as not found and goes with its thread, never a blob no document names and nothing can sweep.
                let mut list: Vec<_> =
                    threads.get(&thread).into_iter().flatten().filter(|h| !kept.iter().any(|k| k.0 == h.key)).cloned().collect();
                list.extend(kept.iter().map(|(key, media, _)| KeptImage { key: key.clone(), media_type: media.clone() }));
                threads.insert(thread, list);
                store.put(ATTACHMENT_KEYS, &ws, to_json(&KeptImages { threads })).await?;
                for (key, _, bytes) in kept {
                    store.put_blob(ATTACHMENTS, &key, bytes).await?;
                }
                Ok(None)
            }
            .boxed_local()
        });
        self.on_kept(workspace_id, step)
    }

    /// transcripts.ts:55: K3.
    pub fn drop_kept_keys(&self, workspace_id: &str, keys: &[String]) -> LocalBoxFuture<'static, ()> {
        let going: IndexSet<String> = keys.iter().cloned().collect();
        let store = self.ctx.store();
        self.on_kept(
            workspace_id,
            Box::new(move |held: KeptImages| {
                async move {
                    if !held.threads.values().any(|list| list.iter().any(|k| going.contains(&k.key))) {
                        return Ok(None);
                    }
                    for key in &going {
                        store.delete_blob(ATTACHMENTS, key).await?;
                    }
                    let threads = held
                        .threads
                        .into_iter()
                        .filter_map(|(id, list)| {
                            let left: Vec<_> = list.into_iter().filter(|k| !going.contains(&k.key)).collect();
                            (!left.is_empty()).then_some((id, left))
                        })
                        .collect();
                    Ok(Some(KeptImages { threads }))
                }
                .boxed_local()
            }),
        )
    }

    /// transcripts.ts:86: K4.
    pub fn drop_sent_images(&self, workspace_id: &str, threads: ThreadsNamed) -> LocalBoxFuture<'static, ()> {
        let store = self.ctx.store();
        self.on_kept(
            workspace_id,
            Box::new(move |held: KeptImages| {
                async move {
                    let going: Vec<String> = match &threads {
                        ThreadsNamed::All => held.threads.keys().cloned().collect(),
                        ThreadsNamed::These(ids) => ids.iter().filter(|id| held.threads.contains_key(*id)).cloned().collect(),
                    };
                    if going.is_empty() {
                        return Ok(None);
                    }
                    for id in &going {
                        for k in held.threads.get(id).into_iter().flatten() {
                            store.delete_blob(ATTACHMENTS, &k.key).await?;
                        }
                    }
                    let left = held.threads.into_iter().filter(|(id, _)| !going.contains(id)).collect();
                    Ok(Some(KeptImages { threads: left }))
                }
                .boxed_local()
            }),
        )
    }

    /// transcripts.ts:590: K6. Synchronous.
    pub fn cap_sessions(&self, workspace_id: &str) {
        let sessions = self.ctx.sessions();
        let rows: Vec<(String, Row)> = sessions
            .borrow()
            .iter()
            .filter(|(_, row)| row.borrow().view.workspace_id == workspace_id)
            .map(|(id, row)| (id.clone(), row.clone()))
            .collect();
        let excess = rows.len() as i64 - SESSION_INDEX_CAP as i64;
        if excess <= 0 {
            return;
        }
        // A thread's newest row stays: every thread of a project's folder shares this cap, and one thread's turns pushed
        // a quiet thread's only row off, so the thread left the sidebar and search.
        let mut newest: IndexMap<String, usize> = IndexMap::new();
        for (i, (_, row)) in rows.iter().enumerate() {
            let entry = row.borrow();
            let key = thread_key_of(&entry.view);
            let better = match newest.get(&key) {
                None => true,
                Some(&had) => started_at(&entry.view) >= started_at(&rows[had].1.borrow().view),
            };
            if better {
                newest.insert(key, i);
            }
        }
        let mut finished: Vec<(usize, f64)> = Vec::new();
        for (i, (_, row)) in rows.iter().enumerate() {
            let entry = row.borrow();
            if entry.view.status != SessionStatus::Running && newest.get(&thread_key_of(&entry.view)) != Some(&i) {
                finished.push((i, started_at(&entry.view)));
            }
        }
        finished.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        let records = self.ctx.thread_records();
        for (i, _) in finished.into_iter().take(excess as usize) {
            let (id, row) = &rows[i];
            sessions.borrow_mut().shift_remove(id);
            let entry = row.borrow();
            let Some(thread) = entry.view.thread_id.clone() else {
                continue;
            };
            let Some(status) = ended_of(entry.view.status) else {
                continue;
            };
            let held = records.borrow().get(&thread).cloned();
            if let Some(record) = held {
                records.borrow_mut().insert(thread, ThreadRecord { ended: Some(status), ..record });
            }
        }
    }

    /// transcripts.ts:617: K7. The document is built in the call; the future is the put on the chain.
    pub fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        if !self.ctx.is_live(workspace_id) {
            return futures::future::ready(()).boxed_local();
        }
        let sessions = self.ctx.sessions();
        let records = self.ctx.thread_records();
        for row in sessions.borrow().values() {
            let entry = row.borrow();
            let view = &entry.view;
            if view.workspace_id != workspace_id || view.status == SessionStatus::Running {
                continue;
            }
            let Some(thread) = view.thread_id.clone() else {
                continue;
            };
            let worked_false = records.borrow().get(&thread).is_some_and(|r| r.worked == Some(false));
            if worked_false && thread_ran(std::slice::from_ref(view)) {
                let held = records.borrow().get(&thread).cloned();
                if let Some(record) = held {
                    records.borrow_mut().insert(thread, ThreadRecord { worked: Some(true), ..record });
                }
            }
        }
        self.cap_sessions(workspace_id);
        let rows: Vec<Value> = sessions
            .borrow()
            .values()
            .filter_map(|row| {
                let entry = row.borrow();
                (entry.view.workspace_id == workspace_id && entry.launch.is_none()).then(|| session_json(&entry))
            })
            .collect();
        let threads: Map<String, Value> = records
            .borrow()
            .iter()
            .filter(|(_, record)| record.workspace_id == workspace_id)
            .map(|(id, record)| {
                let mut held = match to_json(record) {
                    Value::Object(map) => map,
                    _ => Map::new(),
                };
                held.remove("workspaceId");
                (id.clone(), Value::Object(held))
            })
            .collect();
        let marks = self.viewed_marks.borrow().get(workspace_id).filter(|m| !m.is_empty()).map(to_json);
        let mut doc = Map::new();
        doc.insert("workspaceId".into(), json!(workspace_id));
        doc.insert("sessions".into(), Value::Array(rows));
        doc.insert("threads".into(), Value::Object(threads));
        if let Some(marks) = marks {
            doc.insert("viewed".into(), marks);
        }
        let doc = Value::Object(doc);
        let store = self.ctx.store();
        let key = workspace_id.to_string();
        let flushes = self.ctx.index_flushes();
        let previous = flushes.with(|m| m.get(workspace_id).cloned());
        let link: Joined = async move {
            if let Some(previous) = previous {
                previous.await;
            }
            let _ = store.put(SESSIONS, &key, doc).await;
        }
        .boxed_local()
        .shared();
        flushes.with(|m| {
            m.insert(workspace_id.to_string(), link.clone());
        });
        tokio::task::spawn_local(link.clone());
        link.boxed_local()
    }

    /// transcripts.ts:521: K8.
    pub fn token_for_machine(&self, machine_id: &str) -> String {
        if let Some(held) = self.machine_tokens.borrow().get(machine_id).cloned() {
            return held;
        }
        let minted = match self.ctx.daemon_token_seed() {
            None => hex_of(&self.ctx.ids().bytes(24)),
            // The seed's shape was checked when the area was built, so the derivation cannot be refused here.
            Some(seed) => daemon_token_for(&seed, machine_id).unwrap_or_default(),
        };
        self.machine_tokens.borrow_mut().insert(machine_id.to_string(), minted.clone());
        minted
    }

    /// transcripts.ts:532: K9.
    pub fn daemon_token_of(
        &self,
        machine: Arc<dyn Machine>,
        path: Option<String>,
    ) -> LocalBoxFuture<'static, Result<Option<String>, HostError>> {
        let id = machine.id().to_string();
        let token = self.token_for_machine(&id);
        let clock: Rc<dyn Clock> = self.ctx.clock();
        let cached = self.daemon_tokens.borrow().get(&id).copied();
        if let Some(cached) = cached {
            if cached.has_daemon || clock.now() - cached.at < DAEMON_TOKEN_MISS_TTL_MS as i64 {
                return futures::future::ready(Ok(cached.has_daemon.then_some(token))).boxed_local();
            }
        }
        let existing = self.writing.borrow().get(&id).cloned();
        let wrote = match existing {
            Some(wrote) => wrote,
            None => {
                let machine_path = daemon_token_path_of(machine.as_ref(), path.as_deref());
                let wrote = self.ctx.rotate_daemon_token(machine.clone(), token.clone(), machine_path).shared();
                self.writing.borrow_mut().insert(id.clone(), wrote.clone());
                wrote
            }
        };
        let me = self.me.clone();
        async move {
            let answer = wrote.await;
            if let Some(area) = me.upgrade() {
                area.writing.borrow_mut().shift_remove(&id);
            }
            let has_daemon = answer?;
            if let Some(area) = me.upgrade() {
                area.daemon_tokens.borrow_mut().insert(id, super::DaemonAnswered { has_daemon, at: clock.now() });
            }
            Ok(has_daemon.then_some(token))
        }
        .boxed_local()
    }

    /// transcripts.ts:723: K11.
    pub fn ran_on(&self, workspace_id: &str, thread_id: &str, session: Option<&str>) -> RanPicks {
        let mut rows: Vec<SessionView> = self.ctx.rows_on(thread_id).into_iter().filter(|r| r.workspace_id == workspace_id).collect();
        rows.sort_by(|a, b| started_at(a).partial_cmp(&started_at(b)).unwrap_or(Ordering::Equal));
        let recorded = recorded_picks(&rows);
        let Some(session) = session else {
            return recorded;
        };
        let model = if recorded.model.is_none() { self.ctx.resumed_fact(workspace_id, session, "model") } else { None };
        let effort = if recorded.effort.is_none() { self.ctx.resumed_fact(workspace_id, session, "effort") } else { None };
        let mut picks = model.map(|m| model_picks(&m, None)).unwrap_or_default();
        if let Some(effort) = effort {
            picks.effort = Some(effort);
        }
        let RanPicks { model, context_window, effort, permission_mode } = recorded;
        if model.is_some() {
            picks.model = model;
        }
        if context_window.is_some() {
            picks.context_window = context_window;
        }
        if effort.is_some() {
            picks.effort = effort;
        }
        if permission_mode.is_some() {
            picks.permission_mode = permission_mode;
        }
        picks
    }

    /// transcripts.ts:731: K12.
    pub fn box_of(&self, thread_id: &str) -> Option<Entry> {
        let ws = match self.ctx.latest_on(thread_id) {
            Some(view) => view.workspace_id,
            None => {
                let records = self.ctx.thread_records();
                let held = records.borrow().get(thread_id).map(|r| r.workspace_id.clone())?;
                held
            }
        };
        let entry = self.ctx.entry(&ws)?;
        let local = {
            let live = entry.borrow();
            let record = live.record.borrow();
            wsp_words::workspace_state::is_local_workspace(Some(&record.kind))
        };
        (!local).then_some(entry)
    }
}
