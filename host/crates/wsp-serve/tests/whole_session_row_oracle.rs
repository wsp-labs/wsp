// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/session-row-oracle.test.ts at 65f6b89c7, case for case. A window that read the rows once and
//! then folds in every pushed row, as the app's store does, holds what a fresh listing answers after every step a thread
//! takes: any difference is a row the app shows stale until it reconnects.
#[macro_use]
mod common;

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use common::*;
use serde_json::{json, Value};
use wsp_core::types::api::{AnswerAsk, StartOptions};
use wsp_wire::views::session::{SessionView, ThreadMarks};
use wsp_words::views::session::fold_threads;

fn ask_of(n: u32) -> Value {
    json!({
        "askId": format!("ask_{n}"),
        "toolName": "Read",
        "toolUseId": format!("toolu_read_{n}"),
        "input": "{\"file_path\":\"/root/hello.txt\"}",
        "detail": "hello.txt",
        "options": [
            { "id": "allow", "label": "Allow", "effect": "allow" },
            { "id": "deny", "label": "Deny", "effect": "deny" },
        ],
    })
}

fn opts(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn marks(doc: Value) -> ThreadMarks {
    serde_json::from_value(doc).expect("marks")
}

fn short(s: &Value) -> String {
    s.as_str().map(|s| s.chars().take(8).collect()).unwrap_or_default()
}

fn clip(v: Option<&Value>, n: usize) -> String {
    match v {
        Some(v) => v.to_string().chars().take(n).collect(),
        None => "undefined".into(),
    }
}

/// `withPushedRow(rows, e)` (protocol session-events.ts:287) over the rows as JSON.
fn with_pushed_row(rows: &mut Vec<Value>, e: &Value) {
    let at = rows.iter().position(|r| r["id"] == e["id"]);
    let row = &e["row"];
    if row.is_null() {
        if let Some(at) = at {
            rows.remove(at);
        }
        return;
    }
    let thread = row.get("threadId").filter(|t| !t.is_null());
    let prompt = row.get("prompt").cloned().or_else(|| at.and_then(|at| rows[at].get("prompt").cloned())).or_else(|| {
        rows.iter()
            .find(|r| thread.is_some() && r.get("threadId") == thread && r.get("prompt").is_some())
            .and_then(|r| r.get("prompt").cloned())
    });
    let mut row = row.clone();
    if let (Some(prompt), Some(o)) = (prompt, row.as_object_mut()) {
        o.insert("prompt".into(), prompt);
    }
    match at {
        None => rows.push(row),
        Some(at) => rows[at] = row,
    }
}

fn threads_of(rows: &[Value]) -> Vec<Value> {
    let views: Vec<SessionView> = rows.iter().map(|r| serde_json::from_value(r.clone()).expect("a session row")).collect();
    fold_threads(&views).iter().map(j).collect()
}

fn keys(a: &Value, b: &Value) -> BTreeSet<String> {
    [a, b].iter().filter_map(|v| v.as_object()).flat_map(|o| o.keys().cloned()).collect()
}

// "pushed rows against a fresh listing" > "every step leaves the mirror equal to a listing"
#[test]
#[allow(unused_assignments)]
fn every_step_leaves_the_mirror_equal_to_a_listing() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let driven = Driven::oracle();
        let fx = runtime(&backend).adapter("claude", driven.factory()).build();
        let rt = fx.rt.clone();
        let ws = create_on(&rt, CreateOn::golden("boat"), None).await.expect("a workspace").view.id;
        let mut mirror: Vec<Value> =
            j(&rt.sessions().list(Some(ws.clone()), None).await.expect("a list")).as_array().cloned().unwrap_or_default();
        let events = fx.events();
        let mut applied = events.len();
        let mut diffs: Vec<String> = vec![];
        let mut thread_diffs: Vec<String> = vec![];

        macro_rules! check {
            ($label:expr, $ms:expr) => {{
                let label: String = $label.to_string();
                sleep_ms($ms).await;
                let fresh_rows =
                    j(&rt.sessions().list(Some(ws.clone()), None).await.expect("a list")).as_array().cloned().unwrap_or_default();
                sleep_ms(10).await;
                for e in events.all().iter().skip(applied) {
                    if e["type"] == "session.row" && e["workspaceId"] == json!(ws) {
                        with_pushed_row(&mut mirror, e);
                    }
                }
                applied = events.len();
                let mut by_id: Vec<(Value, Value)> = mirror.iter().map(|r| (r["id"].clone(), r.clone())).collect();
                for f in &fresh_rows {
                    let at = by_id.iter().position(|(id, _)| id == &f["id"]);
                    let Some(at) = at else {
                        diffs.push(format!("{label}: row {} ({}) listed, not in the mirror", short(&f["id"]), short(&f["prompt"])));
                        continue;
                    };
                    let m = by_id.remove(at).1;
                    for k in keys(f, &m) {
                        if f.get(&k) != m.get(&k) {
                            diffs.push(format!(
                                "{label}: row {} ({}) .{k} listed {} mirror {}",
                                short(&f["id"]),
                                short(&f["prompt"]),
                                clip(f.get(&k), 400),
                                clip(m.get(&k), 400)
                            ));
                        }
                    }
                }
                for (id, m) in by_id {
                    diffs.push(format!("{label}: row {} ({}) in the mirror, not listed", short(&id), short(&m["prompt"])));
                }
                let folded: HashMap<String, Value> =
                    threads_of(&mirror).into_iter().map(|t| (t["id"].as_str().unwrap_or_default().to_string(), t)).collect();
                for t in threads_of(&fresh_rows) {
                    let id = t["id"].as_str().unwrap_or_default().to_string();
                    let Some(m) = folded.get(&id) else {
                        thread_diffs.push(format!("{label}: thread {} listed, not folded from the mirror", short(&t["id"])));
                        continue;
                    };
                    for k in keys(&t, m) {
                        if t.get(&k) != m.get(&k) {
                            thread_diffs.push(format!(
                                "{label}: thread {} ({}) .{k} listed {} mirror {}",
                                short(&t["id"]),
                                short(&t["title"]),
                                clip(t.get(&k), 70),
                                clip(m.get(&k), 70)
                            ));
                        }
                    }
                }
            }};
            ($label:expr) => {
                check!($label, 60)
            };
        }
        let sid = |id: &Rc<dyn wsp_core::types::wiring::SessionHandle>| j(&id.view())["id"].as_str().expect("an id").to_string();
        let tid = |id: &Rc<dyn wsp_core::types::wiring::SessionHandle>| j(&id.view())["threadId"].as_str().expect("a thread").to_string();

        // 1. a thread starts and ends
        let a = rt.sessions().start(&ws, opts("first thread"), None).await.expect("a start");
        let a_thread = tid(&a);
        check!("A start");
        driven.last().end(json!({}));
        a.finished().await;
        check!("A end", 120);

        // 2. a second turn in the same thread
        let a2 = rt
            .sessions()
            .start(&ws, StartOptions { prompt: "go on".into(), thread: Some(a_thread.clone()), ..Default::default() }, None)
            .await
            .expect("a start");
        check!("A turn 2 start");
        driven.last().emit(
            json!({ "type": "subagent", "task": "t1", "state": "running", "parentToolUseId": "toolu_t1", "title": "count", "depth": 1 }),
        );
        check!("A subagent start");
        driven
            .last()
            .emit(json!({ "type": "subagent", "task": "t1", "state": "done", "parentToolUseId": "toolu_t1", "summary": "counted" }));
        check!("A subagent end");
        driven.last().emit(json!({ "type": "permission.ask", "ask": ask_of(1) }));
        check!("A asks");
        rt.sessions()
            .answer(&sid(&a2), AnswerAsk { ask_id: "ask_1".into(), option_id: "allow".into(), reason: None }, None)
            .await
            .expect("an answer");
        check!("A answered");
        driven.last().end(json!({ "costUsd": 0.5 }));
        a2.finished().await;
        check!("A turn 2 end", 120);

        // 3. marks
        rt.sessions().read(&a_thread, None).await.expect("a read");
        check!("A read");
        rt.sessions().mark(vec![a_thread.clone()], marks(json!({ "pinned": true })), None).await.expect("a mark");
        check!("A pinned");
        rt.sessions().mark(vec![a_thread.clone()], marks(json!({ "folded": true })), None).await.expect("a mark");
        check!("A folded");
        rt.sessions().mark(vec![a_thread.clone()], marks(json!({ "snoozedUntil": now_ms() + 60_000 })), None).await.expect("a mark");
        check!("A snoozed");
        rt.sessions().mark(vec![a_thread.clone()], marks(json!({ "snoozedUntil": null })), None).await.expect("a mark");
        check!("A unsnoozed");
        rt.sessions().settle(vec![a_thread.clone()], None, Default::default()).await.expect("a settle");
        check!("A settled");
        rt.sessions().restore(vec![a_thread.clone()], None).await.expect("a restore");
        check!("A restored");

        // 4. rename and access
        let renamed = rt.sessions().rename(&sid(&a2), "named by hand", None).await.expect("a rename");
        check!(format!("A renamed ({})", j(&renamed)["outcome"].as_str().unwrap_or_default()));

        // 5. a follower stopped behind another thread's prompt, prompt first then the call
        let b = rt.sessions().start(&ws, opts("target thread"), None).await.expect("a start");
        let b_thread = tid(&b);
        let b_turn = driven.last();
        let c = rt.sessions().start(&ws, opts("caller thread"), None).await.expect("a start");
        let c_turn = driven.last();
        check!("B and C start");
        b_turn.emit(json!({ "type": "permission.ask", "ask": ask_of(2) }));
        check!("B asks");
        c_turn.calls("toolu_wait", "mcp__wsp__threads_wait", &json!({ "threads": [b_thread], "timeout": 60 }).to_string());
        check!("C follows B while B asks");
        rt.sessions()
            .answer(&sid(&b), AnswerAsk { ask_id: "ask_2".into(), option_id: "deny".into(), reason: None }, None)
            .await
            .expect("an answer");
        check!("B answered");
        b_turn.end(json!({}));
        b.finished().await;
        check!("B end", 120);
        c_turn.answers("toolu_wait");
        check!("C wait returns");

        // 6. a prompt that ends with its turn interrupted, never answered
        let d = rt.sessions().start(&ws, opts("interrupted while asking"), None).await.expect("a start");
        let d_turn = driven.last();
        c_turn.calls("toolu_wait2", "mcp__wsp__threads_wait", &json!({ "threads": [tid(&d)], "timeout": 60 }).to_string());
        d_turn.emit(json!({ "type": "permission.ask", "ask": ask_of(3) }));
        check!("D asks, C behind it");
        rt.sessions().interrupt(&sid(&d), None, None).await.expect("an interrupt");
        d.finished().await;
        check!("D interrupted while asking", 150);
        c_turn.answers("toolu_wait2");
        check!("C second wait returns");
        c_turn.end(json!({}));
        c.finished().await;
        check!("C end", 120);

        // 7. a start that never runs
        if let Ok(h) = rt.sessions().start(&ws, opts("boom never runs"), None).await {
            h.finished().await;
        }
        check!("boom start", 150);

        // 8. a limit on a turn's result
        let e = rt.sessions().start(&ws, opts("hits the limit"), None).await.expect("a start");
        driven.last().end(json!({ "status": "failed", "limit": { "resetsAt": now_ms() + 3_600_000 } }));
        e.finished().await;
        check!("E limit end", 150);

        // 9. a restart of a settled thread
        rt.sessions().settle(vec![b_thread.clone()], None, Default::default()).await.expect("a settle");
        check!("B settled");
        let r = rt
            .sessions()
            .start(&ws, StartOptions { prompt: "restart of B".into(), replaces: Some(b_thread.clone()), ..Default::default() }, None)
            .await;
        check!(
            format!("restart of settled B ({})", r.as_ref().err().map_or("started".to_string(), |e| e.message.chars().take(60).collect())),
            150
        );
        if let Ok(r) = r {
            driven.last().end(json!({}));
            r.finished().await;
            check!("restart end", 150);
        }
        // a restart of a thread not settled
        let f = rt.sessions().start(&ws, opts("to be replaced"), None).await.expect("a start");
        driven.last().end(json!({ "status": "interrupted" }));
        f.finished().await;
        check!("F end", 120);
        let f2 = rt
            .sessions()
            .start(&ws, StartOptions { prompt: "restart of F".into(), replaces: Some(tid(&f)), ..Default::default() }, None)
            .await;
        check!(
            format!(
                "restart of unsettled F ({})",
                f2.as_ref().err().map_or("started".to_string(), |e| e.message.chars().take(60).collect())
            ),
            150
        );
        if let Ok(f2) = f2 {
            driven.last().end(json!({}));
            f2.finished().await;
            check!("restart F end", 150);
        }

        rt.close().await.expect("a close");
        assert_eq!(diffs, Vec::<String>::new());
        assert_eq!(thread_diffs, Vec::<String>::new());
    });
}
