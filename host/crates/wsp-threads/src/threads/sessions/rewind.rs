// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:1264-1396: a thread back to one of its turns. Spec: spec/sessions_rewind.md.
use std::rc::Rc;

use serde_json::{json, Map, Value};
use wsp_adapters::port::{CountedTurn, ForkTurn, ForkedSession, RevertAsk, RevertAt, Reverted};
use wsp_core::types::api::RewindAsk;
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::internal_index::write_lines;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_machines::machines::boot::entries::RowAt;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::sub_thread::SUB_TURN_GONE_LINE;
use wsp_wire::views::session::{SessionStatus, SessionView};
use wsp_wire::views::sub_row::sub_turn_gone;
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::runtime::SessionRewindResult;
use wsp_words::exit::{not_found_refusal, HostError, Kind};
use wsp_words::thread_read::thread_result;
use wsp_words::views::session::fold_threads;
use wsp_words::words::computer::thread_word;
use wsp_words::words::thread::{
    rewind_beside_line, rewind_children_line, rewind_kept_line, rewind_no_anchor_line, REWIND_COPIED_LINE, REWIND_LATEST_LINE,
    REWIND_NO_CHECKPOINT_LINE, REWIND_NO_UNDO_LINE, REWIND_SHARED_LINE, REWIND_WORKING_LINE,
};
use wsp_words::words::turn::turn_lines;

use super::{SessionsArea, SessionsCtx};
use crate::threads::agents::launch::BuiltAdapter;
use crate::threads::threads::{Entry, Row};

fn conflict(line: &str) -> HostError {
    HostError::with_kind(line.to_string(), Kind::Conflict)
}

fn record_of(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

fn field<'a>(doc: &'a Value, key: &str) -> Option<&'a str> {
    doc.get(key).and_then(Value::as_str)
}

/// The turn an event belongs to, its thread and its start-or-end time, read off the event's own JSON.
fn event_doc(event: &impl serde::Serialize) -> Value {
    serde_json::to_value(event).unwrap_or(Value::Null)
}

/// The files go back to `before` after a refused rewind (TS `putBack`): a failure to put them back is warned.
async fn put_back(
    ctx: &Rc<dyn SessionsCtx>,
    entry: &Entry,
    cwd: &str,
    thread_id: &str,
    before: Option<&str>,
    error: HostError,
) -> HostError {
    if let Some(before) = before {
        if let Err(back) = ctx.git_restore(entry, cwd, before).await {
            wsp_core::console::warn(&format!(
                "the files of thread {} were not put back after a refused rewind: {}",
                thread_word(thread_id),
                back.message
            ));
        }
    }
    error
}

impl SessionsArea {
    /// sessions.ts:1264. Rules W1-W14.
    pub async fn rewind_thread(&self, thread_id: &str, ask: RewindAsk, origin: Option<Caller>) -> Result<SessionRewindResult, HostError> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        let rows: Vec<Row> =
            ctx.sessions().borrow().values().filter(|r| r.borrow().view.thread_id.as_deref() == Some(thread_id)).cloned().collect();
        let held: Option<ThreadRecord> = ctx.records().borrow().get(thread_id).cloned();
        let from_rows = rows.first().map(|r| r.borrow().view.workspace_id.clone());
        let Some(workspace_id) = from_rows.or_else(|| held.as_ref().map(|h| h.workspace_id.clone())) else {
            return Err(not_found_refusal(&format!("no thread {}", thread_word(thread_id))));
        };
        let at = RowAt { thread_id: Some(thread_id.to_string()), workspace_id: workspace_id.clone() };
        let Some(entry) = ctx.entry_of_row(&at, origin.as_ref(), None, false).await? else {
            return Err(not_found_refusal(&format!("no thread {}", thread_word(thread_id))));
        };
        if rows.iter().any(|r| r.borrow().view.status == SessionStatus::Running) {
            return Err(conflict(REWIND_WORKING_LINE));
        }
        SessionsCtx::reap_kept(&*ctx, thread_id);
        let tree = ctx.tree_under(thread_id);
        let all_views: Vec<SessionView> = ctx.sessions().borrow().values().map(|r| r.borrow().view.clone()).collect();
        let under =
            fold_threads(&all_views.iter().filter(|v| v.thread_id.as_ref().is_some_and(|t| tree.contains(t))).cloned().collect::<Vec<_>>());
        let running: Vec<String> = under.iter().filter(|t| t.status == SessionStatus::Running).map(|t| t.title.clone()).collect();
        if !running.is_empty() {
            return Err(conflict(&rewind_children_line(&running)));
        }
        let record = record_of(&entry);
        let cwd = ctx.checkout_of(&record);
        let sub_released = record.sub.as_ref().and_then(|s| s.released_at.map(|t| t.0));
        let gone = |started: Option<f64>| sub_turn_gone(sub_released, started);

        if ask.undo == Some(true) {
            let Some(rewound) = held.as_ref().and_then(|h| h.rewound.clone()) else {
                return Err(conflict(REWIND_NO_UNDO_LINE));
            };
            if gone(Some(rewound.at.0)) {
                return Err(conflict(SUB_TURN_GONE_LINE));
            }
            besides_running(&ctx, &workspace_id, thread_id)?;
            if record.phase != WorkspacePhase::Running {
                ctx.wake(&workspace_id, origin.as_ref()).await?;
            }
            let back = ctx.git_restore(&entry, &cwd, &rewound.before).await?;
            if let Some(record) = ctx.records().borrow_mut().get_mut(thread_id) {
                record.rewound = None;
            }
            done(&ctx, &workspace_id, thread_id).await?;
            return Ok(SessionRewindResult { turns: JsInt(JsNumber(0.0)), files: Some(JsInt(JsNumber(back_files(&back)))), kept: None });
        }

        let events = ctx.open_thread(&workspace_id, thread_id).await?;
        let mut order: Vec<String> = Vec::new();
        for event in &events {
            let doc = event_doc(event);
            if field(&doc, "threadId") == Some(thread_id) {
                if let Some(turn) = field(&doc, "turnId") {
                    if !order.iter().any(|t| t == turn) {
                        order.push(turn.to_string());
                    }
                }
            }
        }
        let position = match ask.turn_id.as_deref() {
            None => None,
            Some(turn) => order.iter().position(|t| t == turn),
        };
        let Some(at) = position else {
            return Err(not_found_refusal(&format!(
                "no turn {} on thread {}",
                ask.turn_id.clone().unwrap_or_default(),
                thread_word(thread_id)
            )));
        };
        let turn = order[at].clone();
        let started =
            rows.iter().find(|r| r.borrow().turn_id == turn).and_then(|r| r.borrow().view.started_at.map(|t| t.0)).or_else(|| {
                events
                    .iter()
                    .map(event_doc)
                    .find(|d| field(d, "turnId") == Some(turn.as_str()) && d.get("at").is_some_and(|a| !a.is_null()))
                    .and_then(|d| d["at"].as_f64())
            });
        if gone(started) {
            return Err(conflict(SUB_TURN_GONE_LINE));
        }
        if at == order.len() - 1 {
            return Err(conflict(REWIND_LATEST_LINE));
        }
        let docs: Vec<Value> = events.iter().map(event_doc).collect();
        if docs.iter().any(|d| {
            field(d, "threadId") == Some(thread_id)
                && field(d, "turnId") == Some(turn.as_str())
                && d.get("copiedFrom").is_some_and(|c| !c.is_null())
        }) {
            return Err(conflict(REWIND_COPIED_LINE));
        }
        let cut: Vec<String> = order[at + 1..].to_vec();
        let kept_of = |turn_id: &str| -> Option<Value> {
            docs.iter().rev().find(|d| field(d, "type") == Some("session.checkpoint") && field(d, "turnId") == Some(turn_id)).cloned()
        };
        let kept = kept_of(&turn);
        let kept_ref = kept.as_ref().and_then(|k| field(k, "ref").map(str::to_string));
        let kept_at = kept.as_ref().and_then(|k| k.get("at").and_then(Value::as_f64));
        let kept_anchor = kept.as_ref().and_then(|k| field(k, "anchor").map(str::to_string));
        let latest: Option<SessionView> = ctx.latest_on(thread_id).or_else(|| rows.first().map(|r| r.borrow().view.clone()));
        let harness = latest.as_ref().map(|v| v.harness.clone()).or_else(|| held.as_ref().map(|h| h.harness.clone())).unwrap_or_default();
        let BuiltAdapter { adapter, .. } = ctx.launch_adapter_for(&entry, &harness, None).await?;
        let agent = ctx.agent_label(&harness);
        let mut files = ask.files == Some(true);
        if files && kept_ref.is_none() {
            return Err(conflict(REWIND_NO_CHECKPOINT_LINE));
        }
        let kept_when = kept_at
            .or_else(|| rows.iter().find(|r| r.borrow().turn_id == turn).and_then(|r| r.borrow().view.ended_at.map(|t| t.0)))
            .unwrap_or(0.0);
        let shared = files
            && all_views.iter().any(|v| {
                v.workspace_id == workspace_id
                    && v.thread_id.as_ref().is_some_and(|t| t != thread_id)
                    && v.ended_at.map_or(f64::INFINITY, |t| t.0) > kept_when
            });
        if shared {
            files = false;
        }
        let uncut: Option<String> = docs
            .iter()
            .find(|d| {
                field(d, "type") == Some("session.checkpoint")
                    && field(d, "threadId") == Some(thread_id)
                    && d.get("kept").is_some_and(|k| !k.is_null())
            })
            .and_then(|d| field(d, "kept").map(str::to_string));
        if let (Some(why), false) = (uncut.as_deref(), files) {
            return Err(conflict(&if shared { REWIND_SHARED_LINE.to_string() } else { rewind_kept_line(why, false) }));
        }
        let (has_revert, has_fork) = (ctx.adapter_reverts(adapter.as_ref()), ctx.adapter_forks(adapter.as_ref()));
        let copies = !has_revert && has_fork;
        let cuts_conversation = (copies || has_revert) && uncut.is_none();
        if copies && kept_anchor.is_none() {
            return Err(conflict(&rewind_no_anchor_line(&agent)));
        }
        if !cuts_conversation && !files {
            return Err(conflict(&if shared { REWIND_SHARED_LINE.to_string() } else { rewind_no_anchor_line(&agent) }));
        }

        if record_of(&entry).phase != WorkspacePhase::Running {
            ctx.wake(&workspace_id, origin.as_ref()).await?;
        }
        let moved: Option<Map<String, Value>> =
            if files { Some(ctx.git_restore(&entry, &cwd, kept_ref.as_deref().unwrap_or_default()).await?) } else { None };
        let before: Option<String> = moved.as_ref().map(|m| {
            m.get("before")
                .map(|b| b.as_str().map(str::to_string).unwrap_or_else(|| b.to_string()))
                .unwrap_or_else(|| "undefined".to_string())
        });
        let mut kept_why: Option<String> = None;
        let latest_session = latest.as_ref().and_then(|v| v.claude_session_id.clone());
        if has_revert && cuts_conversation {
            if let Some(session) = latest_session.clone() {
                let first_cut_anchor = kept_of(&cut[0]).and_then(|k| field(&k, "anchor").map(str::to_string));
                let turns_for = |id: &str| -> CountedTurn {
                    let anchor = kept_of(id).and_then(|k| field(&k, "anchor").map(str::to_string));
                    let result = docs
                        .iter()
                        .rev()
                        .find(|d| field(d, "type") == Some("session.done") && field(d, "turnId") == Some(id))
                        .and_then(|d| d.get("result").cloned())
                        .and_then(|r| serde_json::from_value(r).ok());
                    CountedTurn { anchor, result }
                };
                let at_cut = match first_cut_anchor {
                    Some(anchor) => RevertAt::BeforeTurn(anchor),
                    None => RevertAt::Turns(cut.iter().map(|id| turns_for(id)).collect()),
                };
                let road = adapter.revert(RevertAsk { session, cwd: Some(cwd.clone()), at: at_cut });
                if let Some(road) = road {
                    let answer = road.await.map_err(HostError::new);
                    let answer = match answer {
                        Ok(answer) => answer,
                        Err(e) => return Err(put_back(&ctx, &entry, &cwd, thread_id, before.as_deref(), e).await),
                    };
                    if let Reverted::Kept(why) = answer {
                        kept_why = Some(why);
                    }
                }
            }
        }
        if copies && cuts_conversation {
            if let Some(session) = latest_session.clone() {
                let anchor = kept_anchor.clone().unwrap_or_default();
                if let Some(road) = adapter.fork_session(session.clone(), ForkTurn::Anchor(anchor)) {
                    let copy = match road.await {
                        Ok(copy) => copy,
                        Err(why) => return Err(put_back(&ctx, &entry, &cwd, thread_id, before.as_deref(), HostError::new(why)).await),
                    };
                    if let ForkedSession::Resume { resume, .. } = copy {
                        ctx.move_session(thread_id, &session, &resume);
                    }
                }
            }
        }
        {
            let records_cell = ctx.records();
            let mut records = records_cell.borrow_mut();
            let record = records.entry(thread_id.to_string()).or_insert_with(|| ThreadRecord {
                workspace_id: workspace_id.clone(),
                harness: harness.clone(),
                ..Default::default()
            });
            if copies && cuts_conversation {
                record.cut_owed = None;
            }
            if let Some(before) = before.clone() {
                record.rewound = Some(wsp_core::types::internal::Rewound { before, at: JsNumber(ctx.clock().now() as f64) });
            }
        }
        if let Some(why) = kept_why.clone() {
            done(&ctx, &workspace_id, thread_id).await?;
            return Err(conflict(&rewind_kept_line(&why, before.is_some())));
        }
        if cuts_conversation {
            ctx.drop_from_transcript(&workspace_id, thread_id, Some(cut.as_slice())).await?;
            let after = ctx.open_thread(&workspace_id, thread_id).await?;
            let now = thread_result(&after, thread_id);
            if let Some(latest) = latest.as_ref() {
                let (last_line, failure) = match now.as_ref() {
                    Some(result) => {
                        let lines = turn_lines(result, None);
                        (lines.last_line, lines.failure)
                    }
                    None => (None, None),
                };
                let rows_now: Vec<Row> = ctx.sessions().borrow().values().cloned().collect();
                for row in rows_now {
                    let same = {
                        let r = row.borrow();
                        r.view.id == latest.id && r.view.workspace_id == latest.workspace_id
                    };
                    if same {
                        write_lines(&mut row.borrow_mut().view, last_line.clone(), failure.clone());
                        break;
                    }
                }
            }
        }
        ctx.slates_rewound(thread_id, &turn, &cut).await?;
        done(&ctx, &workspace_id, thread_id).await?;
        Ok(SessionRewindResult {
            turns: JsInt(JsNumber(if cuts_conversation { cut.len() as f64 } else { 0.0 })),
            files: moved.as_ref().map(|m| JsInt(JsNumber(back_files_of(m)))),
            kept: if shared { Some(REWIND_SHARED_LINE.to_string()) } else { None },
        })
    }
}

fn back_files(back: &Map<String, Value>) -> f64 {
    back.get("files").and_then(Value::as_f64).unwrap_or(0.0)
}

fn back_files_of(back: &Map<String, Value>) -> f64 {
    back_files(back)
}

/// W3: another thread of the workspace that is working holds the folder the files go back in.
fn besides_running(ctx: &Rc<dyn SessionsCtx>, workspace_id: &str, thread_id: &str) -> Result<(), HostError> {
    let views: Vec<SessionView> = ctx.sessions().borrow().values().map(|r| r.borrow().view.clone()).collect();
    let beside: Vec<SessionView> =
        views.into_iter().filter(|v| v.workspace_id == workspace_id && v.thread_id.as_ref().is_some_and(|t| t != thread_id)).collect();
    match fold_threads(&beside).into_iter().find(|t| t.status == SessionStatus::Running) {
        Some(running) => Err(conflict(&rewind_beside_line(&running.title))),
        None => Ok(()),
    }
}

/// W3, W14: the rewind's end: rows persisted, transcript flushed, `thread.rewound` said.
async fn done(ctx: &Rc<dyn SessionsCtx>, workspace_id: &str, thread_id: &str) -> Result<(), HostError> {
    ctx.persist_sessions(workspace_id).await?;
    ctx.flush_transcript(workspace_id).await?;
    if let Ok(event) =
        serde_json::from_value::<EventUnion>(json!({ "type": "thread.rewound", "workspaceId": workspace_id, "threadId": thread_id }))
    {
        ctx.emit(event);
    }
    Ok(())
}
