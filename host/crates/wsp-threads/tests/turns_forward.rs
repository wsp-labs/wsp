// SPDX-License-Identifier: AGPL-3.0-only
//! spec/turns_forward.md: a turn's adapter events turned into rows. The replay counts are invariant 2's road.
#[path = "support_a/mod.rs"]
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use support::*;
use wsp_core::types::internal::EndRow;
use wsp_wire::types::usage::AccountsAnswer;

/// A turn `turn-1` of thread `t1` on workspace `w1`, launched on session `sess-1`.
struct Turn {
    host: Rc<FakeHost>,
    forward: OnEvent,
    session: Arc<FakeSession>,
    turn: Row,
    handle: Rc<dyn SessionHandle>,
}

impl Turn {
    fn send(&self, doc: Value) {
        (self.forward)(adapter_event(doc))
    }
    fn row(&self) -> Row {
        self.host.rows.borrow().get("sess-1").cloned().expect("the turn's row under its session id")
    }
    fn view(&self) -> SessionView {
        self.turn.borrow().view.clone()
    }
    fn end_fn(&self) -> EndRow {
        self.row().borrow().end.clone().expect("the row carries its end")
    }
    fn live(&self) -> TurnLive {
        self.row().borrow().turn_live.clone().expect("the row carries its live state")
    }
    fn asked(&self) -> TurnAsked {
        self.row().borrow().asked.clone().expect("the row carries what was asked")
    }
}

struct Setup {
    written: Option<TurnWritten>,
    notify: Option<Vec<String>>,
    asked: Option<TurnAsked>,
    turn_live: Option<TurnLive>,
    title: Option<String>,
    request_id: Option<String>,
    reports_edits: bool,
    view: Value,
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            written: None,
            notify: None,
            asked: None,
            turn_live: None,
            title: None,
            request_id: None,
            reports_edits: false,
            view: json!({}),
        }
    }
}

fn turn(setup: Setup) -> Turn {
    let host = FakeHost::new();
    let entry = host.add_entry(entry("w1", json!({})));
    let mut doc = json!({ "threadId": "t1", "startedBy": "person", "prompt": "fix the upload test", "startedAt": T0 });
    for (k, v) in setup.view.as_object().unwrap() {
        doc[k] = v.clone();
    }
    let turn = row(view("turn-1", "w1", doc), "turn-1");
    turn.borrow_mut().asked = setup.asked;
    turn.borrow_mut().turn_live = setup.turn_live;
    let session = FakeSession::new("sess-1");
    let caught: Rc<RefCell<Option<OnEvent>>> = Rc::new(RefCell::new(None));
    let (s, c) = (session.clone(), caught.clone());
    let area = TurnsArea::new(host.clone());
    let handle = area
        .run_turn(TurnStart {
            entry,
            row: turn.clone(),
            written: setup.written,
            thread_id: "t1".into(),
            turn_id: "turn-1".into(),
            notify: setup.notify,
            notify_by: None,
            notify_road: None,
            turn_token: Some("tok".into()),
            scope_device_id: None,
            outcome: serde_json::from_value(json!("started")).unwrap(),
            opening: Opening {
                prompt: "fix the upload test".into(),
                request_id: setup.request_id,
                title: setup.title,
                ..Opening::default()
            },
            resume: None,
            images_dir: None,
            snapshot: None,
            reports_edits: setup.reports_edits,
            waiting: Some(Rc::new(std::cell::Cell::new(false))),
            keep: None,
            open: Box::new(move |forward: OnEvent| {
                *c.borrow_mut() = Some(forward);
                Ok(s as Arc<dyn HarnessSession>)
            }),
        })
        .expect("the turn opens");
    let forward = caught.borrow().clone().expect("open was handed the forward");
    Turn { host, forward, session, turn, handle }
}

fn base(kind: &str) -> Value {
    json!({ "type": kind, "workspaceId": "w1", "sessionId": "sess-1", "turnId": "turn-1", "threadId": "t1" })
}

fn with(mut v: Value, extra: Value) -> Value {
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

fn delta(text: &str) -> Value {
    json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": text })
}

fn delta_row(line: u64, text: &str) -> Value {
    with(base("session.delta"), json!({ "line": line, "kind": "text", "text": text }))
}

fn written(lines: u64, subagents: u64, compactions: u64, retries: u64, started: bool, reply: Option<&str>) -> TurnWritten {
    TurnWritten {
        lines,
        subagents,
        compactions,
        retries,
        reply: reply.map(|r| serde_json::from_value(json!(r)).unwrap()),
        started,
        changes: false,
    }
}

// F1: turns.ts:292-296
#[test]
fn unread_ids_are_kept_after_the_end() {
    run_local(async {
        let t = turn(Setup::default());
        let sent: Rc<RefCell<Vec<(Vec<String>, bool)>>> = Rc::default();
        let s = sent.clone();
        t.host.hooks.set::<dyn Fn(&Row, Vec<String>, bool) -> ()>(
            "TurnsCtx::send_back",
            Rc::new(move |_r: &Row, unread: Vec<String>, stopped: bool| s.borrow_mut().push((unread, stopped))),
        );
        let end = t.end_fn();
        end("machine paused", Some(true), None);
        t.send(json!({ "type": "turn.unread", "sessionId": "sess-1", "ids": ["m1", "m2"] }));
        t.session.finish(result(json!({ "status": "interrupted" })));
        settle().await;
        assert_eq!(*sent.borrow(), vec![(vec!["m1".to_string(), "m2".to_string()], true)]);
    });
}

// F1: turns.ts:296
#[test]
fn events_after_an_end_here_write_nothing() {
    run_local(async {
        let t = turn(Setup::default());
        t.send(json!({ "type": "session.start", "sessionId": "sess-1" }));
        let end = t.end_fn();
        end("machine paused", None, None);
        let before = t.host.recorded_json();
        t.send(delta("late"));
        t.send(json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "late" } }));
        t.send(json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        assert_eq!(t.host.recorded_json(), before);
    });
}

// F2: turns.ts:299-313
#[test]
fn the_start_names_the_session_on_the_view_and_the_record() {
    run_local(async {
        let t = turn(Setup { request_id: Some("req-1".into()), ..Setup::default() });
        t.send(json!({ "type": "session.start", "sessionId": "sess-9", "cwd": "/work/w1", "model": "claude-opus-5-5" }));
        let v = t.view();
        assert_eq!(
            (v.claude_session_id.as_deref(), v.cwd.as_deref(), v.model.as_deref()),
            (Some("sess-9"), Some("/work/w1"), Some("claude-opus-5-5"))
        );
        assert_eq!(t.host.live("w1").unwrap().borrow().record.borrow().claude_session_id.as_deref(), Some("sess-9"));
        settle().await;
        assert_eq!(t.host.hooks.called("prompt_held"), vec!["prompt_held(req-1)"]);
        assert!(!t.host.hooks.called("persist_record").is_empty(), "the record is persisted with its session");
    });
}

// F2: trap #1620, turns.ts:306-307, 563
#[test]
fn a_start_before_the_prompt_marks_the_row_awaiting_it() {
    run_local(async {
        let asked: TurnAsked = serde_json::from_value(json!({ "prompt": "fix it" })).unwrap();
        let t = turn(Setup { asked: Some(asked), request_id: Some("req-1".into()), ..Setup::default() });
        t.send(json!({ "type": "session.start", "sessionId": "sess-1", "prompted": false }));
        assert_eq!(t.asked().awaits_prompt, Some(true));
        settle().await;
        assert_eq!(t.host.hooks.called("prompt_held"), Vec::<String>::new(), "a start not prompted holds no prompt");
        t.send(json!({ "type": "session.start", "sessionId": "sess-1" }));
        assert_eq!(t.asked().awaits_prompt, None);
        settle().await;
        assert_eq!(t.host.hooks.called("prompt_held"), vec!["prompt_held(req-1)"]);
        assert_eq!(t.host.recorded_of("session.start").len(), 1);
        t.row().borrow_mut().asked.as_mut().expect("asked").awaits_prompt = Some(true);
        t.send(json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": false }));
        assert_eq!(
            t.host.recorded_of("session.end"),
            vec![with(base("session.end"), json!({ "exitCode": 0, "sawResult": false, "promptless": true }))]
        );
    });
}

// F2: turns.ts:313
#[test]
fn a_title_given_at_the_start_is_written_into_the_harness_once() {
    run_local(async {
        let t = turn(Setup { title: Some("Upload test".into()), ..Setup::default() });
        t.send(json!({ "type": "session.start", "sessionId": "sess-1" }));
        t.send(json!({ "type": "session.start", "sessionId": "sess-1" }));
        settle().await;
        assert_eq!(t.host.hooks.called("name_in_harness"), vec!["name_in_harness(Upload test)"]);
    });
}

// F3: turns.ts:315-338
#[test]
fn one_start_row_however_often_the_harness_announces() {
    run_local(async {
        let t = turn(Setup {
            request_id: Some("req-1".into()),
            view: json!({ "permissionMode": "acceptEdits", "effort": "high" }),
            ..Setup::default()
        });
        t.send(json!({ "type": "session.start", "sessionId": "sess-1", "model": "claude-opus-5-5", "cwd": "/work/w1", "tools": ["Bash"] }));
        t.send(json!({ "type": "session.start", "sessionId": "sess-1", "model": "claude-opus-5-5" }));
        assert_eq!(
            t.host.recorded_of("session.start"),
            vec![with(
                base("session.start"),
                json!({ "prompt": "fix the upload test", "requestId": "req-1", "startedBy": "person", "model": "claude-opus-5-5", "cwd": "/work/w1", "permissionMode": "acceptEdits", "effort": "high", "agent": "claude", "tools": ["Bash"] })
            )]
        );
    });
}

// F3, invariant 2: turns.ts:102, 315
#[test]
fn invariant_2_a_written_start_row_is_not_written_again() {
    run_local(async {
        let t = turn(Setup { written: Some(written(0, 0, 0, 0, true, None)), ..Setup::default() });
        t.send(json!({ "type": "session.start", "sessionId": "sess-1" }));
        t.send(delta("one"));
        assert_eq!(t.host.recorded_of("session.start"), Vec::<Value>::new());
        assert_eq!(t.host.recorded_of("session.delta"), vec![delta_row(1, "one")]);
    });
}

// F3: turns.ts:345-351
#[test]
fn a_seed_titled_thread_asks_for_a_title_once() {
    run_local(async {
        let t = turn(Setup::default());
        t.host
            .hooks
            .set::<dyn Fn(&SessionView) -> Option<String>>("TurnsCtx::source_of", Rc::new(|_v: &SessionView| Some("seed".to_string())));
        t.send(json!({ "type": "session.start", "sessionId": "sess-1" }));
        t.send(json!({ "type": "session.start", "sessionId": "sess-1" }));
        settle().await;
        assert_eq!(t.host.hooks.called("hold_ask").len(), 1);
    });
}

// F4: turns.ts:363-383
#[test]
fn deltas_are_numbered_from_one() {
    run_local(async {
        let t = turn(Setup::default());
        t.send(delta("one"));
        t.send(json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "tool_use", "text": "{\"command\":\"ls\"}", "toolName": "Bash", "toolUseId": "toolu_1", "parentToolUseId": "toolu_0" }));
        assert_eq!(
            t.host.recorded_of("session.delta"),
            vec![
                delta_row(1, "one"),
                with(
                    base("session.delta"),
                    json!({ "line": 2, "kind": "tool_use", "text": "{\"command\":\"ls\"}", "toolName": "Bash", "toolUseId": "toolu_1", "parentToolUseId": "toolu_0" })
                ),
            ]
        );
    });
}

// F4, invariant 2: subagents.test.ts:161, 246; turns.ts:359-362
#[test]
fn invariant_2_a_reopened_turn_writes_no_delta_twice() {
    run_local(async {
        let t = turn(Setup { written: Some(written(2, 0, 0, 0, true, None)), ..Setup::default() });
        for text in ["one", "two", "three"] {
            t.send(delta(text));
        }
        assert_eq!(
            t.host.recorded_of("session.delta"),
            vec![delta_row(3, "three")],
            "the two written lines are read past, the third is line 3"
        );
    });
}

// F4: turns.ts:355-358
#[test]
fn open_calls_follow_tool_use_and_result_even_while_replaying() {
    run_local(async {
        let t = turn(Setup { written: Some(written(2, 0, 0, 0, true, None)), ..Setup::default() });
        t.send(json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "tool_use", "text": "{}", "toolName": "Bash", "toolUseId": "toolu_1" }));
        t.send(json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "tool_use", "text": "{\"q\":1}", "toolName": "wsp_wait", "toolUseId": "toolu_2" }));
        t.send(json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "tool_result", "text": "ok", "toolUseId": "toolu_1" }));
        let row = t.row();
        let row = row.borrow();
        let calls = row.calls.as_ref().expect("the row carries its open calls");
        let open: Vec<(String, String, String)> = calls.iter().map(|(id, c)| (id.clone(), c.tool_name.clone(), c.input.clone())).collect();
        assert_eq!(open, vec![("toolu_2".to_string(), "wsp_wait".to_string(), "{\"q\":1}".to_string())]);
        assert_eq!(
            t.host.recorded_of("session.delta"),
            vec![with(base("session.delta"), json!({ "line": 3, "kind": "tool_result", "text": "ok", "toolUseId": "toolu_1" }))]
        );
    });
}

// F5, invariant 2: turns.ts:388-392
#[test]
fn invariant_2_a_written_reply_is_not_written_again() {
    run_local(async {
        let t = turn(Setup {
            written: Some(written(1, 0, 0, 0, true, Some("completed"))),
            turn_live: Some(TurnLive::default()),
            notify: Some(vec!["lead".into()]),
            ..Setup::default()
        });
        t.send(delta("one"));
        t.send(json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "failed", "error": "replayed differently" } }));
        assert_eq!(t.host.recorded_of("session.done"), Vec::<Value>::new());
        assert_eq!(serde_json::to_value(t.live().reply).unwrap(), json!("completed"), "the reply written stands");
        assert_eq!(t.host.hooks.called("notify_end"), Vec::<String>::new(), "a lead is never told one reply twice");
        t.send(json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "a second reply" } }));
        assert_eq!(t.host.recorded_of("session.done").len(), 1, "only the one skip is spent");
    });
}

// F6: turn-costs.test.ts; turns.ts:396-407
#[test]
fn the_reply_writes_its_lines_refusal_limit_and_adds_its_cost() {
    run_local(async {
        let t = turn(Setup { view: json!({ "costUsd": 0.25 }), ..Setup::default() });
        let reply = json!({ "status": "completed", "text": "Fixed it.\nAll green.", "costUsd": 0.5 });
        t.send(json!({ "type": "turn.done", "sessionId": "sess-1", "result": reply }));
        let v = t.view();
        let mut expected = view(
            "turn-1",
            "w1",
            json!({ "threadId": "t1", "startedBy": "person", "prompt": "fix the upload test", "startedAt": T0, "costUsd": 0.75 }),
        );
        let lines = wsp_words::words::turn::turn_lines(&result(reply.clone()), None);
        wsp_core::types::internal::write_lines(&mut expected, lines.last_line.clone(), lines.failure.clone());
        assert_eq!(
            (v.cost_usd, v.last_line.clone(), v.failure.clone(), v.status.clone()),
            (expected.cost_usd, expected.last_line, expected.failure, expected.status),
            "the row stays running until the exit"
        );
        assert_eq!(serde_json::to_value(t.live().reply).unwrap(), json!("completed"));
        assert_eq!(t.host.recorded_of("session.done"), vec![with(base("session.done"), json!({ "result": reply }))]);
    });
}

// F7: usage-ledger.test.ts; turns.ts:409-433
#[test]
fn a_turn_counts_once_on_its_own_models_ledger_row() {
    run_local(async {
        let t = turn(Setup::default());
        let filed: Rc<RefCell<Vec<LedgerUse>>> = Rc::default();
        let f = filed.clone();
        t.host.hooks.set::<dyn Fn(LedgerUse) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "TurnsCtx::ledger_add",
            Rc::new(move |row: LedgerUse| {
                f.borrow_mut().push(row);
                async { Ok(()) }.boxed_local()
            }),
        );
        let tokens = json!({ "input": 10, "output": 2, "cached": 0, "cacheWrite": 0, "reasoning": 0 });
        t.send(json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "model": "opus", "models": [
            { "model": "haiku", "tokens": tokens, "costUsd": 0.01 },
            { "model": "opus", "tokens": tokens, "costUsd": 0.2 },
        ], "tokens": tokens, "costUsd": 0.21 } }));
        settle().await;
        let rows: Vec<(String, u32, Option<f64>, Option<String>, String)> = filed
            .borrow()
            .iter()
            .map(|r| (r.model.clone().unwrap_or_default(), r.turns, r.cost_usd, r.session.clone(), r.source.clone()))
            .collect();
        assert_eq!(
            rows,
            vec![
                ("haiku".to_string(), 0, Some(0.01), Some("sess-1".to_string()), "wsp".to_string()),
                ("opus".to_string(), 1, Some(0.2), Some("sess-1".to_string()), "wsp".to_string()),
            ]
        );
        let first = filed.borrow()[0].clone();
        assert_eq!(
            (first.at, first.agent.as_str(), first.account.as_str(), first.computer.as_str(), first.project.as_str()),
            (T0, "claude", "claude:default", "this-computer", "p1")
        );
    });
}

// F7: turns.ts:409
#[test]
fn no_tokens_and_no_cost_files_nothing() {
    run_local(async {
        let t = turn(Setup::default());
        t.send(json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "done" } }));
        settle().await;
        assert_eq!(t.host.hooks.called("ledger_add"), Vec::<String>::new());
    });
}

// F8: turns.ts:436-439
#[test]
fn the_end_is_told_to_its_targets_once_unless_a_lines_try() {
    run_local(async {
        let t = turn(Setup { notify: Some(vec!["lead".into()]), ..Setup::default() });
        let told: Rc<RefCell<Vec<(Vec<String>, Value, bool)>>> = Rc::default();
        let k = told.clone();
        t.host.hooks.set::<dyn Fn(&Row, &[String], TurnResult, bool) -> ()>(
            "TurnsCtx::notify_end",
            Rc::new(move |_r: &Row, to: &[String], res: TurnResult, open: bool| {
                k.borrow_mut().push((to.to_vec(), serde_json::to_value(res).unwrap(), open))
            }),
        );
        t.send(json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "done" } }));
        assert_eq!(*told.borrow(), vec![(vec!["lead".to_string()], json!({ "status": "completed", "text": "done" }), false)]);
        let order: Vec<String> = t.host.recorded_json().iter().map(|e| e["type"].as_str().unwrap().to_string()).collect();
        assert_eq!(order, vec!["session.done"]);

        let t2 = turn(Setup { notify: Some(vec!["lead".into()]), ..Setup::default() });
        t2.host.hooks.set::<dyn Fn(&Row) -> bool>("TurnsCtx::line_try", Rc::new(|_r: &Row| true));
        t2.send(json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "done" } }));
        assert_eq!(t2.host.hooks.called("notify_end"), Vec::<String>::new(), "a line's try tells nobody");
    });
}

// F10, F13, F14, invariant 2: subagents.test.ts:269, 294
#[test]
fn invariant_2_no_subagent_compaction_or_retry_row_twice() {
    run_local(async {
        let t = turn(Setup { written: Some(written(1, 1, 1, 1, true, None)), ..Setup::default() });
        t.send(json!({ "type": "subagent", "sessionId": "sess-1", "task": "a1", "state": "running", "parentToolUseId": "toolu_a1" }));
        t.send(json!({ "type": "turn.compacted", "sessionId": "sess-1", "before": 10, "after": 5 }));
        t.send(json!({ "type": "turn.retry", "sessionId": "sess-1", "attempt": 1, "of": 10, "why": "Anthropic is overloaded" }));
        t.send(delta("one"));
        t.send(json!({ "type": "subagent", "sessionId": "sess-1", "task": "a1", "state": "done", "parentToolUseId": "toolu_a1", "summary": "Counted." }));
        t.send(json!({ "type": "turn.compacted", "sessionId": "sess-1", "before": 9, "after": 2 }));
        t.send(json!({ "type": "turn.retry", "sessionId": "sess-1", "attempt": 1, "of": 10, "why": "Anthropic did not answer" }));
        t.send(delta("two"));
        assert_eq!(
            t.host.recorded_json(),
            vec![
                with(
                    base("session.subagent"),
                    json!({ "line": 2, "task": "a1", "state": "done", "parentToolUseId": "toolu_a1", "summary": "Counted." })
                ),
                with(base("session.compacted"), json!({ "line": 2, "before": 9, "after": 2 })),
                with(base("session.retry"), json!({ "line": 2, "attempt": 1, "of": 10, "why": "Anthropic did not answer" })),
                delta_row(2, "two"),
            ]
        );
        assert_eq!(
            t.host.hooks.called("agent_retried"),
            vec!["agent_retried(turn-1, Some(\"Anthropic did not answer\"))".to_string()],
            "a replayed retry tells waits nothing"
        );
    });
}

// F11: usage-limit-resume.test.ts; turns.ts:471-483
#[test]
fn a_limit_is_kept_and_pushes_its_usage_row() {
    run_local(async {
        let t = turn(Setup::default());
        let asked: Rc<RefCell<Vec<LimitAsk>>> = Rc::default();
        let a = asked.clone();
        t.host.hooks.set::<dyn Fn(LimitAsk) -> LocalBoxFuture<'static, Result<(Value, Value), HostError>>>(
            "TurnsCtx::ledger_limit",
            Rc::new(move |ask: LimitAsk| {
                a.borrow_mut().push(ask);
                async { Ok((json!({ "used": 10 }), json!({ "used": 90 }))) }.boxed_local()
            }),
        );
        let codex = json!({ "key": "codex:default", "agent": "codex", "label": "Codex", "computers": [] });
        let claude = json!({ "key": "claude:default", "agent": "claude", "label": "Claude", "computers": [], "plan": "max" });
        let answer: AccountsAnswer =
            serde_json::from_value(json!({ "accounts": [codex, claude.clone()] })).expect("an accounts answer the fixture builds");
        t.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<AccountsAnswer, HostError>>>(
            "TurnsCtx::usage_accounts",
            Rc::new(move || {
                let answer = answer.clone();
                async move { Ok(answer) }.boxed_local()
            }),
        );
        let limit = json!({ "windows": [], "plan": "max" });
        t.send(json!({ "type": "limit", "sessionId": "sess-1", "limit": limit }));
        settle().await;
        assert_eq!(
            asked.borrow().iter().map(|a| (a.key.clone(), a.agent.clone(), a.computer.clone())).collect::<Vec<_>>(),
            vec![("claude:default".to_string(), "claude".to_string(), "this-computer".to_string())]
        );
        assert_eq!(t.host.hooks.called("alerts_read").len(), 1);
        assert_eq!(*t.host.emitted.borrow(), vec![json!({ "type": "usage.account", "key": "claude:default", "row": claude })]);
    });
}

// F12: turns.ts:485-492
#[test]
fn usage_is_burned_and_the_context_passes_unrecorded() {
    run_local(async {
        let t = turn(Setup::default());
        let burned: Rc<RefCell<Vec<BurnAsk>>> = Rc::default();
        let b = burned.clone();
        t.host.hooks.set::<dyn Fn(BurnAsk) -> ()>("TurnsCtx::burn_add", Rc::new(move |ask: BurnAsk| b.borrow_mut().push(ask)));
        t.send(json!({ "type": "turn.usage", "sessionId": "sess-1", "tokens": 1200, "context": 45000, "window": 200000, "at": 5 }));
        assert_eq!(
            *burned.borrow(),
            vec![BurnAsk { account: "claude:default".into(), thread_id: "t1".into(), tokens: 1200.0, replayed: None }],
            "a new turn's usage is never marked replayed"
        );
        assert_eq!(*t.host.passed.borrow(), vec![with(base("session.context"), json!({ "at": T0, "context": 45000, "window": 200000 }))]);
        assert_eq!(t.host.recorded_json(), Vec::<Value>::new());
    });
}

// F12: turns.ts:487
#[test]
fn only_a_reopened_turn_marks_its_usage_replayed() {
    run_local(async {
        let t = turn(Setup { written: Some(written(0, 0, 0, 0, true, None)), ..Setup::default() });
        let burned: Rc<RefCell<Vec<BurnAsk>>> = Rc::default();
        let b = burned.clone();
        t.host.hooks.set::<dyn Fn(BurnAsk) -> ()>("TurnsCtx::burn_add", Rc::new(move |ask: BurnAsk| b.borrow_mut().push(ask)));
        t.send(json!({ "type": "turn.usage", "sessionId": "sess-1", "tokens": 7, "at": 1234 }));
        t.send(json!({ "type": "turn.usage", "sessionId": "sess-1", "tokens": 8 }));
        assert_eq!(
            *burned.borrow(),
            vec![
                BurnAsk {
                    account: "claude:default".into(),
                    thread_id: "t1".into(),
                    tokens: 7.0,
                    replayed: Some(("turn-1".into(), 1234.0))
                },
                BurnAsk { account: "claude:default".into(), thread_id: "t1".into(), tokens: 8.0, replayed: None },
            ]
        );
    });
}

// F14: turns.ts:500-507
#[test]
fn the_agents_retry_tells_waits_and_is_numbered() {
    run_local(async {
        let t = turn(Setup::default());
        t.send(json!({ "type": "turn.retry", "sessionId": "sess-1", "attempt": 2 }));
        t.send(json!({ "type": "turn.retry", "sessionId": "sess-1", "attempt": 3, "of": 10, "why": "Anthropic was overloaded" }));
        assert_eq!(
            t.host.recorded_of("session.retry"),
            vec![
                with(base("session.retry"), json!({ "line": 1, "attempt": 2 })),
                with(base("session.retry"), json!({ "line": 2, "attempt": 3, "of": 10, "why": "Anthropic was overloaded" })),
            ]
        );
        assert_eq!(
            t.host.hooks.called("agent_retried"),
            vec!["agent_retried(turn-1, None)".to_string(), "agent_retried(turn-1, Some(\"Anthropic was overloaded\"))".to_string()]
        );
    });
}

// F15: turn-plan.test.ts; turns.ts:508-510
#[test]
fn a_plan_is_written_every_time() {
    run_local(async {
        let t = turn(Setup { written: Some(written(0, 0, 0, 0, true, None)), ..Setup::default() });
        t.send(json!({ "type": "turn.plan", "sessionId": "sess-1", "text": "first" }));
        t.send(json!({ "type": "turn.plan", "sessionId": "sess-1", "text": "first" }));
        assert_eq!(
            t.host.recorded_of("session.plan"),
            vec![with(base("session.plan"), json!({ "text": "first" })), with(base("session.plan"), json!({ "text": "first" }))]
        );
    });
}

// F16: codex-held-turn.test.ts; turns.ts:511-531
#[test]
fn a_reply_over_running_work_is_told_once_and_says_it_is_still_open() {
    run_local(async {
        let t = turn(Setup { notify: Some(vec!["lead".into()]), ..Setup::default() });
        let told: Rc<RefCell<Vec<(Value, bool)>>> = Rc::default();
        let k = told.clone();
        t.host.hooks.set::<dyn Fn(&Row, &[String], TurnResult, bool) -> ()>(
            "TurnsCtx::notify_end",
            Rc::new(move |_r: &Row, _to: &[String], res: TurnResult, open: bool| {
                k.borrow_mut().push((serde_json::to_value(res).unwrap(), open))
            }),
        );
        t.send(json!({ "type": "turn.tasks", "sessionId": "sess-1", "running": 2, "replied": { "status": "completed", "text": "Started both builds." } }));
        let line = wsp_words::words::thread::still_running_line(2.0);
        assert_eq!(*told.borrow(), vec![(json!({ "status": "completed", "text": format!("Started both builds.\n\n{line}") }), true)]);
        assert_eq!(t.live().told, Some(1));
        let told_as = t.live().told_as.expect("the told line is kept");
        assert_eq!((told_as.said.as_deref(), told_as.promised), (Some("Started both builds."), true));
        assert!(t.view().status == view("x", "w1", json!({})).status, "the turn goes on");
        let waiting = t.host.hooks.calls();
        assert!(waiting.iter().any(|c| c.starts_with("persist_sessions")), "the told line is persisted");
    });
}

// F16: turns.ts:524
#[test]
fn a_reopened_turn_does_not_tell_a_told_reply_again() {
    run_local(async {
        let t = turn(Setup {
            written: Some(written(0, 0, 0, 0, true, None)),
            notify: Some(vec!["lead".into()]),
            turn_live: Some(TurnLive { told: Some(1), ..TurnLive::default() }),
            ..Setup::default()
        });
        t.send(
            json!({ "type": "turn.tasks", "sessionId": "sess-1", "running": 1, "replied": { "status": "completed", "text": "Started." } }),
        );
        assert_eq!(t.host.hooks.called("notify_end"), Vec::<String>::new());
        assert_eq!(t.live().told, Some(1));
        t.send(json!({ "type": "turn.tasks", "sessionId": "sess-1", "running": 0, "replied": { "status": "completed", "text": "Both done." } }));
        assert_eq!(t.host.hooks.called("notify_end").len(), 1, "the next reply is told");
        assert_eq!(t.live().told, Some(2));
    });
}

// F17: permission-relay.test.ts; turns.ts:532-545
#[test]
fn an_ask_is_written_with_named_options_and_ends_a_snooze() {
    run_local(async {
        let t = turn(Setup::default());
        t.host.hooks.set::<dyn Fn(&str, &str) -> Option<String>>(
            "TurnsCtx::mode_label",
            Rc::new(|_h: &str, m: &str| if m == "acceptEdits" { Some("Edit files".to_string()) } else { None }),
        );
        let ask = json!({ "askId": "ask-1", "toolName": "Bash", "input": "{\"command\":\"rm -rf build\"}", "options": [
            { "id": "allow", "label": "Allow", "effect": "allow" },
            { "id": "mode", "label": "acceptEdits", "effect": "mode", "mode": "acceptEdits" },
            { "id": "deny", "label": "Deny", "effect": "deny" },
        ] });
        t.send(json!({ "type": "permission.ask", "sessionId": "sess-1", "ask": ask }));
        let label = wsp_words::words::tools::permission_mode_option_label("Edit files");
        let rows = t.host.recorded_of("session.permission");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["options"][1], json!({ "id": "mode", "label": label, "effect": "mode", "mode": "acceptEdits" }));
        assert_eq!(rows[0]["options"][0], json!({ "id": "allow", "label": "Allow", "effect": "allow" }));
        assert_eq!(
            (rows[0]["askId"].clone(), rows[0]["toolName"].clone(), rows[0]["turnId"].clone()),
            (json!("ask-1"), json!("Bash"), json!("turn-1"))
        );
        assert!(t.view().asking.is_some(), "the row says the thread is asking");
        assert!(t.host.lead_asks.borrow().contains_key("t1"));
        assert_eq!(t.host.hooks.called("end_snooze_for").len(), 1);
        t.send(json!({ "type": "permission.close", "sessionId": "sess-1", "askId": "ask-1", "outcome": "allowed", "optionId": "allow" }));
        assert_eq!(
            t.host.recorded_of("session.permission.closed"),
            vec![with(base("session.permission.closed"), json!({ "askId": "ask-1", "outcome": "allowed", "optionId": "allow" }))]
        );
        assert_eq!(t.view().asking, None);
        assert!(!t.host.lead_asks.borrow().contains_key("t1"));
    });
}

// F18: turns.ts:546-567
#[test]
fn the_exit_closes_open_asks_and_settles_the_row_before_its_end_row() {
    run_local(async {
        let t = turn(Setup::default());
        let ask = json!({ "askId": "ask-1", "toolName": "Bash", "input": "{}", "options": [{ "id": "allow", "label": "Allow", "effect": "allow" }] });
        t.send(json!({ "type": "permission.ask", "sessionId": "sess-1", "ask": ask }));
        t.send(json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "done" } }));
        let status_at_end: Rc<RefCell<Option<Value>>> = Rc::default();
        t.host.clock.advance(5_000);
        t.send(json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        let _ = status_at_end;
        let order: Vec<String> = t.host.recorded_json().iter().map(|e| e["type"].as_str().unwrap().to_string()).collect();
        assert_eq!(order, vec!["session.permission", "session.done", "session.permission.closed", "session.end"]);
        assert_eq!(t.host.recorded_of("session.permission.closed")[0]["outcome"], "cancelled");
        assert_eq!(t.host.recorded_of("session.end"), vec![with(base("session.end"), json!({ "exitCode": 0, "sawResult": true }))]);
        let v = t.view();
        assert_eq!((serde_json::to_value(&v.status).unwrap(), v.ended_at.map(|n| n.0)), (json!("completed"), Some((T0 + 5_000) as f64)));
        assert_eq!(t.handle.view().status, v.status, "the handle's copy reads the settled row");
    });
}

// F18: turns.ts:563
#[test]
fn promptless_rides_the_end_of_a_turn_awaiting_its_prompt() {
    run_local(async {
        let asked: TurnAsked = serde_json::from_value(json!({ "prompt": "go", "awaitsPrompt": true })).unwrap();
        let t = turn(Setup { asked: Some(asked), ..Setup::default() });
        // `failure` is never serialized (adapter-port.ts: read in process), so it is set on the event, not in its JSON.
        let mut ended = adapter_event(json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 1, "sawResult": false }));
        if let wsp_adapters::port::AdapterEvent::SessionEnd { failure, .. } = &mut ended {
            *failure = Some(wsp_adapters::port::ExecFailure {
                kind: wsp_adapters::port::ExecFailureKind::Other,
                message: "the stream broke".into(),
            });
        }
        (t.forward)(ended);
        assert_eq!(
            t.host.recorded_of("session.end"),
            vec![with(base("session.end"), json!({ "exitCode": 1, "sawResult": false, "promptless": true }))]
        );
        assert!(t.live().failure.is_some(), "the stream's failure is kept on the turn");
        assert_eq!(serde_json::to_value(&t.view().status).unwrap(), json!("running"), "with no reply the status waits for the finish");
    });
}
