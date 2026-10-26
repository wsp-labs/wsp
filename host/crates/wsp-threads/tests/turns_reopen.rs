// SPDX-License-Identifier: AGPL-3.0-only
//! spec/turns_reopen.md: the sweep, and the re-open of a turn the store left running (invariants 1, 2 and 5).
#[path = "support_a/mod.rs"]
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use support::*;

/// An adapter whose `attach` answers as the test says and keeps the options it was handed.
struct Attaching {
    answer: Mutex<Option<Result<Option<Arc<FakeSession>>, String>>>,
    asked: Mutex<Vec<AdapterAttachOptions>>,
    /// Events the session emits inside `attach`, before it answers.
    early: Vec<Value>,
}

impl HarnessAdapter for Attaching {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("this adapter only attaches".into()))
    }
    fn steers(&self) -> bool {
        false
    }
    fn attaches(&self) -> bool {
        true
    }
    fn attach(
        &self,
        options: AdapterAttachOptions,
    ) -> Option<LocalBoxFuture<'static, Result<Option<Arc<dyn HarnessSession>>, ExecFailure>>> {
        for doc in &self.early {
            (options.on_event)(adapter_event(doc.clone()));
        }
        self.asked.lock().unwrap().push(options);
        let answer = self.answer.lock().unwrap().take().expect("one attach");
        let answer = match answer {
            Ok(Some(s)) => Ok(Some(s as Arc<dyn HarnessSession>)),
            Ok(None) => Ok(None),
            Err(message) => Err(ExecFailure { kind: ExecFailureKind::Unreached, message }),
        };
        Some(futures::future::ready(answer).boxed_local())
    }
}

/// An adapter with no attach road.
struct NoAttach;

impl HarnessAdapter for NoAttach {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("unused".into()))
    }
    fn steers(&self) -> bool {
        false
    }
}

fn host_with(adapter: Arc<dyn HarnessAdapter>) -> Rc<FakeHost> {
    let host = FakeHost::new();
    host.add_entry(entry("w1", json!({})));
    host.hooks.set::<dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>>(
        "TurnsCtx::adapter_for",
        Rc::new(move |_e: &Entry, a: AdapterAsk| {
            Ok(BuiltAdapter { harness: a.named.clone().unwrap_or_default(), adapter: adapter.clone() })
        }),
    );
    host
}

/// The row a host before the restart left running: turn `turn-1` of thread `t1` on run `run-7`, session `sess-1`.
fn left_running(extra: Value) -> Row {
    let mut doc = json!({ "threadId": "t1", "claudeSessionId": "sess-1", "startedAt": T0 - 60_000, "cwd": "/work/w1", "model": "claude-opus-5-5", "prompt": "fix the upload test" });
    for (k, v) in extra.as_object().unwrap() {
        doc[k] = v.clone();
    }
    let left = row(view("sess-1", "w1", doc), "turn-1");
    {
        let mut r = left.borrow_mut();
        r.run = Some("run-7".into());
        r.turn_token = Some("tok-1".into());
    }
    left
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

/// What the transcript holds of the turn before the restart.
fn transcript(host: &FakeHost, docs: Vec<Value>) {
    for d in docs {
        host.recorded.borrow_mut().push(event(d));
    }
}

fn emit(session_events: &Mutex<Vec<AdapterAttachOptions>>, doc: Value) {
    let on = session_events.lock().unwrap()[0].on_event.clone();
    on(adapter_event(doc));
}

// P1, P2: turns.ts:748-766
#[test]
fn the_sweep_names_every_running_rows_run_on_the_computer() {
    run_local(async {
        let host = FakeHost::new();
        let w1 = host.add_entry(entry("w1", json!({})));
        host.add_entry(entry("w2", json!({})));
        let a = row(view("a", "w1", json!({ "threadId": "ta" })), "ta-1");
        a.borrow_mut().run = Some("run-a".into());
        let b = row(view("b", "w2", json!({ "threadId": "tb" })), "tb-1");
        b.borrow_mut().run = Some("run-b".into());
        let done = row(view("c", "w1", json!({ "threadId": "tc", "status": "completed" })), "tc-1");
        done.borrow_mut().run = Some("run-c".into());
        let no_run = row(view("d", "w1", json!({ "threadId": "td" })), "td-1");
        for (k, r) in [("a", a), ("b", b), ("c", done), ("d", no_run)] {
            host.add_row(k, r);
        }
        let held: Rc<RefCell<Vec<Vec<String>>>> = Rc::default();
        let h = held.clone();
        host.hooks.set::<dyn Fn(&Entry) -> Option<Sweep>>(
            "TurnsCtx::exec_sweep",
            Rc::new(move |_e: &Entry| {
                let h = h.clone();
                Some(Box::new(move |runs: Vec<String>| {
                    h.borrow_mut().push(runs);
                    async { Ok(vec!["run-old".to_string()]) }.boxed_local()
                }) as Sweep)
            }),
        );
        let warned = wsp_core::console::capture();
        TurnsArea::new(host.clone()).sweep_runs(&w1).await;
        assert_eq!(*held.borrow(), vec![vec!["run-a".to_string(), "run-b".to_string()]]);
        assert_eq!(warned.lines(), vec![wsp_core::types::internal::swept_runs_log_line("w1", &["run-old".to_string()])]);
    });
}

// P1: turns.ts:749-752
#[test]
fn a_napped_or_away_workspace_is_not_swept() {
    run_local(async {
        let host = FakeHost::new();
        let napped = host.add_entry(entry("w1", json!({ "phase": "napping" })));
        let away = host.add_entry(entry("w2", json!({})));
        host.hooks.set::<dyn Fn(&str) -> bool>("TurnsCtx::is_held_away", Rc::new(|w: &str| w == "w2"));
        let area = TurnsArea::new(host.clone());
        area.sweep_runs(&napped).await;
        area.sweep_runs(&away).await;
        assert_eq!(host.hooks.called("exec_sweep"), Vec::<String>::new(), "no factory is asked");
    });
}

// P3: turns.ts:761-765
#[test]
fn a_failed_sweep_is_warned_and_ends_nothing() {
    run_local(async {
        let host = FakeHost::new();
        let w1 = host.add_entry(entry("w1", json!({})));
        host.hooks.set::<dyn Fn(&Entry) -> Option<Sweep>>(
            "TurnsCtx::exec_sweep",
            Rc::new(|_e: &Entry| {
                Some(Box::new(|_runs: Vec<String>| async { Err(HostError::new("the machine did not answer")) }.boxed_local()) as Sweep)
            }),
        );
        let warned = wsp_core::console::capture();
        TurnsArea::new(host.clone()).sweep_runs(&w1).await;
        assert_eq!(warned.lines(), vec!["the runs on w1 were left as they are: the machine did not answer".to_string()]);
    });
}

// P4: turns.ts:778
#[test]
fn a_row_with_no_run_cannot_be_reopened_silently() {
    run_local(async {
        let host = host_with(Arc::new(NoAttach));
        let warned = wsp_core::console::capture();
        let r = left_running(json!({}));
        r.borrow_mut().run = None;
        assert_eq!(TurnsArea::new(host.clone()).reattach(r, false).await, Reopened::Cannot);
        host.live("w1").unwrap();
        let napped = FakeHost::new();
        napped.add_entry(entry("w1", json!({ "phase": "napping" })));
        assert_eq!(TurnsArea::new(napped.clone()).reattach(left_running(json!({})), false).await, Reopened::Cannot);
        assert_eq!(
            TurnsArea::new(host.clone()).reattach(left_running(json!({})), false).await,
            Reopened::Cannot,
            "an adapter with no attach"
        );
        assert_eq!(warned.lines(), Vec::<String>::new());
        assert_eq!(host.rows.borrow().len(), 0);
    });
}

// P5: turns.ts:786-790
#[test]
fn an_adapter_that_cannot_be_built_says_why() {
    run_local(async {
        let host = FakeHost::new();
        host.add_entry(entry("w1", json!({})));
        host.hooks.set::<dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>>(
            "TurnsCtx::adapter_for",
            Rc::new(|_e: &Entry, _a: AdapterAsk| Err(HostError::new("no adapter for codex in this host"))),
        );
        let warned = wsp_core::console::capture();
        assert_eq!(TurnsArea::new(host.clone()).reattach(left_running(json!({})), false).await, Reopened::Cannot);
        assert_eq!(warned.lines(), vec!["thread t1 on w1 cannot be re-opened: no adapter for codex in this host".to_string()]);
    });
}

// P6, invariant 5: transcript-index.test.ts; turns.ts:799-804
#[test]
fn invariant_5_a_transcript_that_does_not_read_leaves_the_turn_running_and_unreached() {
    run_local(async {
        let adapter = Arc::new(Attaching {
            answer: Mutex::new(Some(Ok(Some(FakeSession::new("sess-1"))))),
            asked: Mutex::new(Vec::new()),
            early: vec![],
        });
        let host = host_with(adapter.clone());
        host.hooks.set::<dyn Fn(&str, &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>>(
            "TurnsCtx::open_thread",
            Rc::new(|_w: &str, _t: &str| async { Err(HostError::new("the transcript of w1 does not parse")) }.boxed_local()),
        );
        let warned = wsp_core::console::capture();
        let r = left_running(json!({}));
        let held = r.clone();
        assert_eq!(TurnsArea::new(host.clone()).reattach(r, false).await, Reopened::Unreached);
        assert_eq!(adapter.asked.lock().unwrap().len(), 0, "the run is never opened");
        assert_eq!(serde_json::to_value(held.borrow().view.status).unwrap(), json!("running"));
        assert_eq!(host.rows.borrow().len(), 0, "no row is put");
        assert_eq!(warned.lines(), vec!["thread t1 on w1 was left running: the transcript of w1 does not parse".to_string()]);
    });
}

// P6, P7: turns.ts:794
#[test]
fn again_says_nothing() {
    run_local(async {
        let adapter = Arc::new(Attaching {
            answer: Mutex::new(Some(Err("no answer from the machine".into()))),
            asked: Mutex::new(Vec::new()),
            early: vec![],
        });
        let host = host_with(adapter);
        let warned = wsp_core::console::capture();
        assert_eq!(TurnsArea::new(host.clone()).reattach(left_running(json!({})), true).await, Reopened::Unreached);
        assert_eq!(warned.lines(), Vec::<String>::new());
    });
}

// P7: turns.ts:811-821
#[test]
fn the_attach_carries_what_the_row_kept() {
    run_local(async {
        let adapter = Arc::new(Attaching { answer: Mutex::new(Some(Ok(None))), asked: Mutex::new(Vec::new()), early: vec![] });
        let host = host_with(adapter.clone());
        let r = left_running(json!({}));
        r.borrow_mut().from = Some(4096);
        r.borrow_mut().asked =
            Some(serde_json::from_value(json!({ "prompt": "fix it\n\nfiles: a.txt", "effort": "high", "typed": "fix it" })).unwrap());
        let steered =
            json!({ "st-1": { "prompt": "also this", "startedBy": "person" }, "st-2": { "prompt": "and that", "startedBy": "person" } });
        r.borrow_mut().turn_live = Some(TurnLive {
            steered: Some(serde_json::from_value(steered).expect("steered rows the fixture builds")),
            ..TurnLive::default()
        });
        TurnsArea::new(host.clone()).reattach(r, false).await;
        let asked = adapter.asked.lock().unwrap();
        let a = &asked[0];
        assert_eq!(
            (
                a.run.as_str(),
                a.session_id.as_str(),
                a.started_at,
                a.model.as_deref(),
                a.cwd.as_deref(),
                a.prompt.as_deref(),
                a.effort.as_deref(),
                a.from,
                a.steered.clone()
            ),
            (
                "run-7",
                "sess-1",
                (T0 - 60_000) as f64,
                Some("claude-opus-5-5"),
                Some("/work/w1"),
                Some("fix it\n\nfiles: a.txt"),
                Some("high"),
                Some(4096),
                Some(vec!["st-1".to_string(), "st-2".to_string()])
            )
        );
    });
}

// P7: turns.ts:826
#[test]
fn a_run_the_machine_no_longer_holds_is_gone() {
    run_local(async {
        let adapter = Arc::new(Attaching { answer: Mutex::new(Some(Ok(None))), asked: Mutex::new(Vec::new()), early: vec![] });
        let host = host_with(adapter);
        let r = left_running(json!({}));
        let held = r.clone();
        assert_eq!(TurnsArea::new(host.clone()).reattach(r, false).await, Reopened::Gone);
        assert_eq!(serde_json::to_value(held.borrow().view.status).unwrap(), json!("running"), "the caller ends it on the gone line");
        assert_eq!(host.rows.borrow().len(), 0);
    });
}

// P7, invariant 1: local-exec.test.ts; turns.ts:822-825
#[test]
fn a_machine_that_answers_nothing_leaves_the_turn_unreached() {
    run_local(async {
        let adapter = Arc::new(Attaching {
            answer: Mutex::new(Some(Err("no answer from the machine".into()))),
            asked: Mutex::new(Vec::new()),
            early: vec![],
        });
        let host = host_with(adapter);
        let warned = wsp_core::console::capture();
        let r = left_running(json!({}));
        let held = r.clone();
        let before = held.borrow().view.clone();
        assert_eq!(TurnsArea::new(host.clone()).reattach(r, false).await, Reopened::Unreached);
        assert_eq!(held.borrow().view, before, "the turn is left exactly as it was");
        assert!(!host.live("w1").unwrap().borrow().unchecked);
        assert_eq!(warned.lines(), vec!["thread t1 on w1 was left running: no answer from the machine".to_string()]);
    });
}

// P8: turns.ts:805-821, 854-857
#[test]
fn events_before_the_row_exists_are_handed_over_in_order() {
    run_local(async {
        let session = FakeSession::new("sess-1");
        let adapter = Arc::new(Attaching {
            answer: Mutex::new(Some(Ok(Some(session)))),
            asked: Mutex::new(Vec::new()),
            early: vec![
                json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": "one" }),
                json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": "two" }),
            ],
        });
        let host = host_with(adapter.clone());
        transcript(&host, vec![with(base("session.start"), json!({ "prompt": "fix the upload test", "agent": "claude" }))]);
        assert_eq!(TurnsArea::new(host.clone()).reattach(left_running(json!({})), false).await, Reopened::Attached);
        emit(&adapter.asked, json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": "three" }));
        let texts: Vec<Value> = host.recorded_of("session.delta").iter().map(|e| json!([e["line"], e["text"]])).collect();
        assert_eq!(texts, vec![json!([1, "one"]), json!([2, "two"]), json!([3, "three"])]);
    });
}

// P9, invariant 1: local-exec.test.ts, subagents.test.ts:161; turns.ts:827-863
#[test]
fn invariant_1_reopens_the_run_by_its_handle_from_its_first_line() {
    run_local(async {
        let session = FakeSession::new("sess-1");
        let adapter =
            Arc::new(Attaching { answer: Mutex::new(Some(Ok(Some(session.clone())))), asked: Mutex::new(Vec::new()), early: vec![] });
        let host = host_with(adapter.clone());
        let _ = host.live("w1");
        transcript(
            &host,
            vec![
                with(base("session.start"), json!({ "prompt": "fix the upload test", "agent": "claude" })),
                with(base("session.delta"), json!({ "line": 1, "kind": "text", "text": "one" })),
                with(base("session.delta"), json!({ "line": 2, "kind": "text", "text": "two" })),
            ],
        );
        let written_before = host.recorded_json();
        assert_eq!(TurnsArea::new(host.clone()).reattach(left_running(json!({})), false).await, Reopened::Attached);
        assert_eq!(adapter.asked.lock().unwrap()[0].from, None, "a row with no byte kept reads its log from byte 0");
        // The machine replays the run from its first line.
        for doc in [
            json!({ "type": "session.start", "sessionId": "sess-1" }),
            json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": "one" }),
            json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": "two" }),
            json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": "three" }),
        ] {
            emit(&adapter.asked, doc);
        }
        let mut expected = written_before;
        expected.push(with(base("session.delta"), json!({ "line": 3, "kind": "text", "text": "three" })));
        assert_eq!(host.recorded_json(), expected, "no row written twice, the new line is line 3");
        let rows = host.rows.borrow();
        let r = rows.get("sess-1").expect("the row is held again under its id").borrow();
        assert_eq!((r.turn_id.as_str(), r.turn_token.as_deref(), r.run.as_deref()), ("turn-1", Some("tok-1"), Some("run-7")));
        assert_eq!(host.hooks.called("idle_hold"), vec!["idle_hold(w1)"]);
    });
}

// P9, invariant 1: local-exec.test.ts ("keeps a reply that landed meanwhile"); turns.ts:388-392
#[test]
fn invariant_1_keeps_a_reply_that_landed_before_the_restart() {
    run_local(async {
        let session = FakeSession::new("sess-1");
        let adapter =
            Arc::new(Attaching { answer: Mutex::new(Some(Ok(Some(session.clone())))), asked: Mutex::new(Vec::new()), early: vec![] });
        let host = host_with(adapter.clone());
        let reply = json!({ "status": "completed", "text": "Fixed." });
        transcript(
            &host,
            vec![
                with(base("session.start"), json!({ "prompt": "fix the upload test", "agent": "claude" })),
                with(base("session.done"), json!({ "result": reply })),
            ],
        );
        let r = left_running(json!({}));
        r.borrow_mut().turn_live =
            Some(TurnLive { reply: Some(serde_json::from_value(json!("completed")).unwrap()), ..TurnLive::default() });
        let held = r.clone();
        assert_eq!(TurnsArea::new(host.clone()).reattach(r, false).await, Reopened::Attached);
        assert_eq!(
            serde_json::to_value(held.borrow().view.status).unwrap(),
            json!("running"),
            "a reply that landed still reads working until the exit"
        );
        emit(&adapter.asked, json!({ "type": "turn.done", "sessionId": "sess-1", "result": reply }));
        emit(&adapter.asked, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        assert_eq!(host.recorded_of("session.done").len(), 1, "the reply is not written again");
        assert_eq!(host.recorded_of("session.end"), vec![with(base("session.end"), json!({ "exitCode": 0, "sawResult": true }))]);
        assert_eq!(serde_json::to_value(held.borrow().view.status).unwrap(), json!("completed"));
    });
}

// P9: turns.ts:853
#[test]
fn the_opening_is_what_was_typed_for_the_turn() {
    run_local(async {
        let session = FakeSession::new("sess-1");
        let adapter = Arc::new(Attaching { answer: Mutex::new(Some(Ok(Some(session)))), asked: Mutex::new(Vec::new()), early: vec![] });
        let host = host_with(adapter.clone());
        let r = left_running(json!({ "prompt": "the thread's first words" }));
        r.borrow_mut().asked =
            Some(serde_json::from_value(json!({ "prompt": "go on\n\nfiles: a.txt", "typed": "go on", "requestId": "req-9" })).unwrap());
        assert_eq!(TurnsArea::new(host.clone()).reattach(r, false).await, Reopened::Attached);
        emit(&adapter.asked, json!({ "type": "session.start", "sessionId": "sess-1" }));
        let start = host.recorded_of("session.start");
        assert_eq!(start.len(), 1);
        assert_eq!((start[0]["prompt"].clone(), start[0]["requestId"].clone()), (json!("go on"), json!("req-9")));
        assert!(!host.live("w1").unwrap().borrow().unchecked, "the run was read, so the machine needs no proof");
    });
}
