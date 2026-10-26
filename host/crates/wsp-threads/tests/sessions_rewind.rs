// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_rewind.md. TS cases: rewind.test.ts (the rewind and undo cases; the checkpoint-taking cases belong
//! to turns_run, the harness catalog and THREAD_OPS cases to their own units). The daemon is the `git_restore` hook,
//! the harness is `FakeAdapter` below, the transcript is the fake's recorded events with `drop_from_transcript`
//! cutting them as the host's own does.
#[path = "support_a/mod.rs"]
mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use support::*;
use wsp_core::console;
use wsp_core::types::internal::Rewound;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::sub_thread::SUB_TURN_GONE_LINE;
use wsp_words::exit::{not_found_refusal, HostError};
use wsp_words::words::computer::thread_word;
use wsp_words::words::thread::{
    rewind_beside_line, rewind_children_line, rewind_kept_line, rewind_no_anchor_line, CODEX_LEGACY_HISTORY, REWIND_COPIED_LINE,
    REWIND_LATEST_LINE, REWIND_NO_CHECKPOINT_LINE, REWIND_NO_UNDO_LINE, REWIND_SHARED_LINE, REWIND_WORKING_LINE,
};

const W: &str = "w1";
const T: &str = "thread-aa";
const SESSION: &str = "S1";
const COPY: &str = "c0c0c0c0-0000-4000-8000-000000000001";
const AGENT: &str = "Claude Code";

fn turn(n: u32) -> String {
    format!("turn-{n}")
}

fn checkpoint_ref(n: u32) -> String {
    format!("refs/wsp/checkpoints/stub-1/{T}/{}", turn(n))
}

fn norm(v: &Value) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64().expect("a number")),
        Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), norm(v))).collect()),
        other => other.clone(),
    }
}

fn conflict(line: &str) -> (String, Option<String>, Option<Kind>) {
    (line.to_string(), None, Some(Kind::Conflict))
}

/// What the harness can do with a conversation.
#[derive(Clone, Copy, PartialEq)]
enum Cuts {
    /// Rewinds onto a copy of its session through the kept turn's anchor (Claude Code).
    Copy,
    /// Cuts at once (Codex).
    Revert,
    /// Keeps its own history.
    None,
}

struct Shape {
    cuts: Cuts,
    revert: Result<Reverted, String>,
    /// The copy a fork answers: `Some(session)` is a copy the thread resumes, `None` a fork the start makes itself.
    fork: Result<Option<String>, String>,
    /// Turns from this number on name an anchor.
    anchors_from: u32,
    kept: Option<&'static str>,
    refs: bool,
    ends: Vec<(u32, Value)>,
    phase: &'static str,
    entry: Value,
    record: Option<Value>,
    rows: bool,
}

impl Shape {
    fn new(cuts: Cuts) -> Shape {
        Shape {
            cuts,
            revert: Ok(Reverted::Cut),
            fork: Ok(Some(COPY.to_string())),
            anchors_from: 1,
            kept: None,
            refs: true,
            ends: Vec::new(),
            phase: "running",
            entry: json!({}),
            record: Some(json!({ "harness": "claude" })),
            rows: true,
        }
    }
}

struct FakeAdapter {
    revert: Option<Result<Reverted, String>>,
    fork: Option<Result<Option<String>, String>>,
    log: Arc<Mutex<Vec<String>>>,
    reverted: Mutex<Vec<RevertAsk>>,
    forked: Mutex<Vec<(String, ForkTurn)>>,
}

impl HarnessAdapter for FakeAdapter {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        panic!("a rewind starts no turn")
    }
    fn steers(&self) -> bool {
        false
    }
    fn reverts(&self) -> bool {
        self.revert.is_some()
    }
    fn forks(&self) -> bool {
        self.fork.is_some()
    }
    fn revert(&self, ask: RevertAsk) -> Option<LocalBoxFuture<'static, Result<Reverted, String>>> {
        let answer = self.revert.clone()?;
        self.log.lock().unwrap().push("revert".to_string());
        self.reverted.lock().unwrap().push(ask);
        Some(futures::future::ready(answer).boxed_local())
    }
    fn fork_session(&self, session: String, turn: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        let answer = self.fork.clone()?;
        self.log.lock().unwrap().push("fork_session".to_string());
        self.forked.lock().unwrap().push((session.clone(), turn));
        let made = answer.map(|copy| match copy {
            Some(resume) => ForkedSession::Resume { resume, drop: Box::new(|| futures::future::ready(Ok(())).boxed_local()) },
            None => ForkedSession::Fork { session, turn: "t".to_string() },
        });
        Some(futures::future::ready(made).boxed_local())
    }
}

struct Rig {
    host: Rc<FakeHost>,
    area: SessionsArea,
    log: Arc<Mutex<Vec<String>>>,
    adapter: Arc<FakeAdapter>,
    /// The turns each `drop_from_transcript` was asked to cut.
    cut: Rc<RefCell<Vec<Option<Vec<String>>>>>,
    /// Each `slates_rewound`: thread, turn, cut.
    slates: Rc<RefCell<Vec<(String, String, Vec<String>)>>>,
    /// What `git_restore` answers for `files`; absent where none.
    files: Rc<RefCell<Option<Value>>>,
    /// The nth `git_restore` (from 0) that fails.
    restore_fails: Rc<RefCell<Option<(usize, HostError)>>>,
}

fn turn_events(n: u32, shape: &Shape) -> Vec<SessionEvent> {
    let at = T0 + i64::from(n) * 1000;
    let base = |kind: &str, k: i64, extra: Value| {
        let mut doc = json!({ "type": kind, "workspaceId": W, "sessionId": SESSION, "turnId": turn(n), "threadId": T, "at": at + k });
        for (key, v) in extra.as_object().expect("an object") {
            doc[key] = v.clone();
        }
        event(doc)
    };
    let ended = shape
        .ends
        .iter()
        .find(|(m, _)| *m == n)
        .map(|(_, r)| r.clone())
        .unwrap_or_else(|| json!({ "status": "completed", "text": format!("reply {n}") }));
    let mut checkpoint = json!({});
    if shape.refs {
        checkpoint["ref"] = json!(checkpoint_ref(n));
    }
    if n >= shape.anchors_from {
        checkpoint["anchor"] = json!(format!("a{n}"));
    }
    if let Some(why) = shape.kept {
        checkpoint["kept"] = json!(why);
    }
    vec![
        base("session.start", 0, json!({ "prompt": format!("prompt {n}") })),
        base("session.delta", 100, json!({ "kind": "text", "text": format!("reply {n}") })),
        base("session.done", 200, json!({ "result": ended })),
        base("session.end", 300, json!({ "exitCode": 0, "sawResult": true })),
        base("session.checkpoint", 500, checkpoint),
    ]
}

impl Rig {
    /// A thread of three finished turns, `turn-1` to `turn-3`, on one workspace.
    fn new(shape: Shape) -> Rig {
        let host = FakeHost::new();
        host.add_entry(entry(W, {
            let mut e = shape.entry.clone();
            e["phase"] = json!(shape.phase);
            e
        }));
        if shape.rows {
            for n in 1..=3u32 {
                let at = T0 + i64::from(n) * 1000;
                let mut v = view(
                    &turn(n),
                    W,
                    json!({ "threadId": T, "claudeSessionId": SESSION, "status": "completed", "startedAt": at, "endedAt": at + 900, "prompt": format!("prompt {n}") }),
                );
                if n == 3 {
                    v.last_line = Some("reply 3".to_string());
                }
                host.add_row(&turn(n), row(v, &turn(n)));
            }
        }
        if let Some(r) = &shape.record {
            host.add_record(T, W, thread_record(r.clone()));
        }
        for n in 1..=3 {
            for e in turn_events(n, &shape) {
                host.recorded.borrow_mut().push(e);
            }
        }
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let adapter = Arc::new(FakeAdapter {
            revert: (shape.cuts == Cuts::Revert).then(|| shape.revert.clone()),
            fork: (shape.cuts == Cuts::Copy).then(|| shape.fork.clone()),
            log: log.clone(),
            reverted: Mutex::new(Vec::new()),
            forked: Mutex::new(Vec::new()),
        });
        let cut: Rc<RefCell<Vec<Option<Vec<String>>>>> = Rc::new(RefCell::new(Vec::new()));
        let slates: Rc<RefCell<Vec<(String, String, Vec<String>)>>> = Rc::new(RefCell::new(Vec::new()));
        let files: Rc<RefCell<Option<Value>>> = Rc::new(RefCell::new(Some(json!(2))));
        let restore_fails: Rc<RefCell<Option<(usize, HostError)>>> = Rc::new(RefCell::new(None));

        let l = log.clone();
        let f = files.clone();
        let fails = restore_fails.clone();
        let asked = Rc::new(Cell::new(0usize));
        host.hooks.set::<dyn Fn(&Entry, &str, &str) -> LocalBoxFuture<'static, Result<Map<String, Value>, HostError>>>(
            "SessionsCtx::git_restore",
            Rc::new(move |_e: &Entry, _cwd: &str, checkpoint: &str| {
                l.lock().unwrap().push(format!("restore {checkpoint}"));
                let nth = asked.get();
                asked.set(nth + 1);
                let failure = fails.borrow().as_ref().filter(|(n, _)| *n == nth).map(|(_, e)| e.clone());
                let mut reply = Map::new();
                reply.insert("before".to_string(), json!(format!("{checkpoint}-before-1")));
                if let Some(n) = f.borrow().clone() {
                    reply.insert("files".to_string(), n);
                }
                async move {
                    match failure {
                        Some(e) => Err(e),
                        None => Ok(reply),
                    }
                }
                .boxed_local()
            }),
        );
        let a = adapter.clone();
        host.hooks.set::<dyn Fn(
            &Entry,
            &str,
            Option<wsp_adapters::port::ServerValues>,
        ) -> LocalBoxFuture<'static, Result<BuiltAdapter, HostError>>>(
            "SessionsCtx::launch_adapter_for",
            Rc::new(move |_e: &Entry, harness: &str, _v: Option<wsp_adapters::port::ServerValues>| {
                let built = BuiltAdapter { harness: harness.to_string(), adapter: a.clone() };
                async move { Ok::<BuiltAdapter, HostError>(built) }.boxed_local()
            }),
        );
        host.hooks.set::<dyn Fn(&str) -> String>(
            "SessionsCtx::agent_label",
            Rc::new(|h: &str| if h == "claude" { AGENT.to_string() } else { h.to_string() }),
        );
        let h = host.clone();
        let l = log.clone();
        let c = cut.clone();
        host.hooks.set::<dyn Fn(&str, &str, Option<&[String]>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::drop_from_transcript",
            Rc::new(move |_w: &str, thread: &str, turns: Option<&[String]>| {
                l.lock().unwrap().push("drop_from_transcript".to_string());
                c.borrow_mut().push(turns.map(|t| t.to_vec()));
                if let Some(turns) = turns {
                    h.recorded.borrow_mut().retain(|e| {
                        let v = serde_json::to_value(e).expect("an event serializes");
                        !(v["threadId"] == thread && turns.iter().any(|t| v["turnId"] == t.as_str()))
                    });
                }
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let l = log.clone();
        let s = slates.clone();
        host.hooks.set::<dyn Fn(&str, &str, &[String]) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::slates_rewound",
            Rc::new(move |thread: &str, turn_id: &str, cut: &[String]| {
                l.lock().unwrap().push("slates_rewound".to_string());
                s.borrow_mut().push((thread.to_string(), turn_id.to_string(), cut.to_vec()));
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&str, &str, &str)>(
            "SessionsCtx::move_session",
            Rc::new(move |thread: &str, from: &str, to: &str| l.lock().unwrap().push(format!("move_session {thread} {from} {to}"))),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&str, Option<&Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::wake",
            Rc::new(move |id: &str, _o: Option<&Caller>| {
                l.lock().unwrap().push(format!("wake {id}"));
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, ()>>(
            "SessionsCtx::persist_sessions",
            Rc::new(move |id: &str| {
                l.lock().unwrap().push(format!("persist_sessions {id}"));
                async {}.boxed_local()
            }),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::flush_transcript",
            Rc::new(move |id: &str| {
                l.lock().unwrap().push(format!("flush_transcript {id}"));
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let l = log.clone();
        let h = host.clone();
        host.hooks.set::<dyn Fn(EventUnion)>(
            "SessionsCtx::emit",
            Rc::new(move |event: EventUnion| {
                let event = serde_json::to_value(&event).expect("an event serializes");
                l.lock().unwrap().push(format!("emit {}", event["type"].as_str().unwrap_or("")));
                h.emitted.borrow_mut().push(event);
            }),
        );
        let l = log.clone();
        let h = host.clone();
        host.hooks.set::<dyn Fn(&str, &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>>(
            "SessionsCtx::open_thread",
            Rc::new(move |_w: &str, thread: &str| {
                l.lock().unwrap().push("open_thread".to_string());
                let events = h.thread_events(thread);
                async move { Ok::<Vec<SessionEvent>, HostError>(events) }.boxed_local()
            }),
        );
        let area = SessionsArea::new(host.clone());
        Rig { host, area, log, adapter, cut, slates, files, restore_fails }
    }

    /// Every call, the transcript reads included: only the order case pins those.
    fn full(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    fn log(&self) -> Vec<String> {
        self.full().into_iter().filter(|l| l != "open_thread").collect()
    }

    fn writes(&self) -> Vec<String> {
        self.log()
    }

    fn restores(&self) -> Vec<String> {
        self.log().into_iter().filter_map(|l| l.strip_prefix("restore ").map(String::from)).collect()
    }

    fn texts(&self) -> Vec<String> {
        self.host.recorded_of("session.delta").iter().map(|e| e["text"].as_str().expect("text").to_string()).collect()
    }

    fn emitted(&self) -> Vec<Value> {
        self.host.emitted.borrow().clone()
    }

    fn rewound(&self) -> Option<Rewound> {
        self.host.records.borrow().get(T).and_then(|h| h.rewound.clone())
    }

    fn reverted(&self) -> Vec<RevertAsk> {
        self.adapter.reverted.lock().unwrap().clone()
    }

    fn forked(&self) -> Vec<(String, ForkTurn)> {
        self.adapter.forked.lock().unwrap().clone()
    }

    /// Another thread of the person's in the same folder.
    fn beside(&self, status: &str, ended: Option<i64>) {
        let mut doc = json!({ "threadId": "thread-bb", "claudeSessionId": "S9", "status": status, "startedAt": T0 + 4000, "prompt": "move the pricing table" });
        if let Some(at) = ended {
            doc["endedAt"] = json!(at);
        }
        self.host.add_row("beside-1", row(view("beside-1", W, doc), "beside-turn"));
    }

    async fn rewind(&self, ask: RewindAsk) -> Result<Value, HostError> {
        self.area.rewind_thread(T, ask, None).await.map(|r| norm(&serde_json::to_value(&r).expect("an answer serializes")))
    }

    async fn to(&self, n: u32, files: bool) -> Result<Value, HostError> {
        self.rewind(RewindAsk { turn_id: Some(turn(n)), files: Some(files), undo: None }).await
    }

    async fn undo(&self) -> Result<Value, HostError> {
        self.rewind(RewindAsk { turn_id: None, files: None, undo: Some(true) }).await
    }
}

fn answer(v: Value) -> Value {
    norm(&v)
}

fn rewound_event() -> Value {
    json!({ "type": "thread.rewound", "workspaceId": W, "threadId": T })
}

fn undoable() -> Shape {
    let mut s = Shape::new(Cuts::Copy);
    s.record = Some(json!({ "harness": "claude", "rewound": { "before": format!("{}-before-1", checkpoint_ref(1)), "at": T0 } }));
    s
}

// W1: rewind.test.ts (no thread named)
#[test]
fn a_thread_nobody_holds_is_not_found() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        let e = r
            .area
            .rewind_thread("thr_nobody", RewindAsk { turn_id: Some(turn(1)), files: Some(true), undo: None }, None)
            .await
            .unwrap_err();
        assert_eq!(said(&e), said(&not_found_refusal(&format!("no thread {}", thread_word("thr_nobody")))));
        // A caller that does not reach the thread's workspace reads the same absence.
        r.host
            .hooks
            .set::<dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>>(
                "SessionsCtx::entry_of_row",
                Rc::new(|_at: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| {
                    async { Ok::<Option<Entry>, HostError>(None) }.boxed_local()
                }),
            );
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(said(&e), said(&not_found_refusal(&format!("no thread {}", thread_word(T)))));
        assert_eq!(r.log(), Vec::<String>::new());
    });
}

// W1
#[test]
fn the_workspace_is_the_first_rows_else_the_records() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        r.host.records.borrow_mut().get_mut(T).unwrap().workspace_id = "w2".to_string();
        r.to(1, false).await.unwrap();
        assert_eq!(r.host.hooks.called("entry_of_row"), vec![format!("entry_of_row(Some(\"{T}\"), {W}, false)")]);

        let mut shape = Shape::new(Cuts::Copy);
        shape.rows = false;
        let r = Rig::new(shape);
        r.host.records.borrow_mut().get_mut(T).unwrap().workspace_id = "w2".to_string();
        let e = r.to(1, false).await.unwrap_err();
        assert_eq!(e.kind, Some(Kind::NotFound), "w2 holds no live workspace here");
        assert_eq!(r.host.hooks.called("entry_of_row"), vec![format!("entry_of_row(Some(\"{T}\"), w2, false)")]);
    });
}

// W2: rewind.test.ts:431
#[test]
fn a_running_thread_is_not_rewound() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        r.host.rows.borrow()[&turn(3)].borrow_mut().view.status = serde_json::from_value(json!("running")).unwrap();
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_WORKING_LINE));
        assert_eq!(r.log(), Vec::<String>::new(), "no file moved and no row written");
        assert_eq!(r.host.hooks.called("reap_kept"), Vec::<String>::new(), "the working refusal comes before the kept agent is reaped");
        assert_eq!(r.texts().len(), 3);
    });
}

// W2: rewind.test.ts:446
#[test]
fn a_running_child_is_named() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        r.host.hooks.set::<dyn Fn(&str) -> Vec<String>>(
            "SessionsCtx::tree_under",
            Rc::new(|id: &str| if id == T { vec!["thread-cc".to_string()] } else { Vec::new() }),
        );
        r.host.add_row("child-1", row(view("child-1", W, json!({ "threadId": "thread-cc", "status": "running", "startedAt": T0 + 5000, "prompt": "fix the port list", "startedBy": "agent" })), "child-turn"));
        let e = r.to(1, false).await.unwrap_err();
        assert_eq!(said(&e), conflict(&rewind_children_line(&["fix the port list"])));
        assert_eq!(
            r.host.hooks.called("reap_kept"),
            vec![format!("reap_kept({T})")],
            "the thread's own kept agent is reaped before the children are read"
        );
        assert_eq!(r.log(), Vec::<String>::new());
        // Stopped, it stands in the way of nothing.
        r.host.rows.borrow()["child-1"].borrow_mut().view.status = serde_json::from_value(json!("interrupted")).unwrap();
        assert_eq!(r.to(1, false).await.unwrap(), answer(json!({ "turns": 2 })));
    });
}

// W3: rewind.test.ts:393
#[test]
fn undo_puts_the_files_back_and_drops_the_mark() {
    run_local(async {
        let r = Rig::new(undoable());
        assert_eq!(r.undo().await.unwrap(), answer(json!({ "turns": 0, "files": 2 })));
        assert_eq!(
            r.log(),
            vec![
                format!("restore {}-before-1", checkpoint_ref(1)),
                format!("persist_sessions {W}"),
                format!("flush_transcript {W}"),
                "emit thread.rewound".to_string(),
            ],
        );
        assert_eq!(r.emitted(), vec![rewound_event()]);
        assert_eq!(r.rewound(), None, "the mark is dropped");
        // The transcript is as the rewind left it, and the slate with it: undo brings the files back alone.
        assert_eq!(r.texts(), vec!["reply 1", "reply 2", "reply 3"]);
        assert_eq!(r.cut.borrow().len(), 0);
        assert_eq!(r.slates.borrow().len(), 0);
        assert_eq!(r.reverted().len() + r.forked().len(), 0);
        // Once.
        let e = r.undo().await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_NO_UNDO_LINE));
        assert_eq!(r.restores().len(), 1);
    });
}

// TS: sessions.ts:1286 `const cwd = ctx.checkoutOf(entry.record)` throws where the record's project is not held and the rewind rejects
// with it, before any file goes back.
#[test]
fn a_rewind_fails_with_the_checkout_read_that_threw_and_restores_nothing() {
    run_local(async {
        let r = Rig::new(undoable());
        r.host.hooks.set::<dyn Fn(&wsp_core::types::wiring::WorkspaceRecord) -> Result<String, HostError>>(
            "SessionsCtx::checkout_of",
            Rc::new(|_r: &wsp_core::types::wiring::WorkspaceRecord| Err(HostError::new("no project p1"))),
        );
        let e = r.undo().await.unwrap_err();
        assert_eq!(e.message, "no project p1");
        assert_eq!(r.restores().len(), 0);
        assert!(r.rewound().is_some(), "the mark stays");
    });
}

// W3
#[test]
fn undo_with_nothing_to_undo_is_refused() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        let e = r.undo().await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_NO_UNDO_LINE));
        let mut shape = Shape::new(Cuts::Copy);
        shape.record = None;
        let r = Rig::new(shape);
        let e = r.undo().await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_NO_UNDO_LINE), "a thread with no record has no rewind either");
        assert_eq!(r.log(), Vec::<String>::new());
    });
}

// W3
#[test]
fn an_undo_whose_daemon_names_no_files_answers_zero() {
    run_local(async {
        let r = Rig::new(undoable());
        *r.files.borrow_mut() = None;
        assert_eq!(r.undo().await.unwrap(), answer(json!({ "turns": 0, "files": 0 })));
    });
}

// W3: sub-lifecycle (a sub-thread's checkpoints went with its machine)
#[test]
fn undo_is_refused_for_a_turn_on_a_deleted_machine() {
    run_local(async {
        let sub = |released: i64| json!({ "sub": { "on": "boat", "repo": "r", "base": { "branch": "main", "sha": "abc" }, "branch": "b", "folder": "/f", "releasedAt": released } });
        let mut shape = undoable();
        shape.entry = sub(T0 + 10_000);
        let r = Rig::new(shape);
        let e = r.undo().await.unwrap_err();
        assert_eq!(said(&e), conflict(SUB_TURN_GONE_LINE));
        assert_eq!(r.log(), Vec::<String>::new());
        // A rewind made after the machine was released is not gone.
        let mut shape = undoable();
        shape.entry = sub(T0 - 10);
        let r = Rig::new(shape);
        assert_eq!(r.undo().await.unwrap(), answer(json!({ "turns": 0, "files": 2 })));
    });
}

// W3: rewind.test.ts:513
#[test]
fn undo_is_refused_while_another_thread_works() {
    run_local(async {
        let r = Rig::new(undoable());
        r.beside("running", None);
        let e = r.undo().await.unwrap_err();
        assert_eq!(said(&e), conflict(&rewind_beside_line("move the pricing table")));
        assert_eq!(r.log(), Vec::<String>::new(), "no file moved, nothing woken");
        r.host.rows.borrow()["beside-1"].borrow_mut().view.status = serde_json::from_value(json!("interrupted")).unwrap();
        assert_eq!(r.undo().await.unwrap(), answer(json!({ "turns": 0, "files": 2 })));
    });
}

// W3, W9: rewind.test.ts:411
#[test]
fn a_napped_machine_wakes_before_its_files_go_back() {
    run_local(async {
        let mut shape = undoable();
        shape.phase = "napping";
        let r = Rig::new(shape);
        let e = r.to(3, true).await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_LATEST_LINE));
        assert_eq!(r.log(), Vec::<String>::new(), "a refusal wakes nothing");
        assert_eq!(r.to(1, true).await.unwrap(), answer(json!({ "turns": 2, "files": 2 })));
        assert_eq!(r.log()[..2].to_vec(), vec![format!("wake {W}"), format!("restore {}", checkpoint_ref(1))]);
        let r = Rig::new({
            let mut s = undoable();
            s.phase = "napping";
            s
        });
        assert_eq!(r.undo().await.unwrap(), answer(json!({ "turns": 0, "files": 2 })));
        assert_eq!(r.log()[..2].to_vec(), vec![format!("wake {W}"), format!("restore {}-before-1", checkpoint_ref(1))]);
        // A running machine is not woken.
        let r = Rig::new(undoable());
        r.undo().await.unwrap();
        assert!(!r.log().iter().any(|l| l.starts_with("wake")));
    });
}

// W4: rewind.test.ts:439
#[test]
fn the_latest_turn_is_not_rewound() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        let e = r.to(3, false).await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_LATEST_LINE));
        let e = r.to(3, true).await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_LATEST_LINE));
        assert_eq!(r.log(), Vec::<String>::new());
    });
}

// W4
#[test]
fn a_turn_not_on_the_thread_is_not_found() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        let e = r.to(9, true).await.unwrap_err();
        assert_eq!(said(&e), said(&not_found_refusal(&format!("no turn turn-9 on thread {}", thread_word(T)))));
        // No turn named at all reads as an empty name.
        let e = r.rewind(RewindAsk { turn_id: None, files: Some(true), undo: None }).await.unwrap_err();
        assert_eq!(said(&e), said(&not_found_refusal(&format!("no turn  on thread {}", thread_word(T)))));
        assert_eq!(r.log(), Vec::<String>::new());
    });
}

// W4
#[test]
fn a_copied_turn_is_not_rewound() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        // The history a fork starts with: its first turn's rows name the thread they were copied from.
        {
            let mut recorded = r.host.recorded.borrow_mut();
            let first = serde_json::to_value(&recorded[0]).unwrap();
            let mut copied = first.clone();
            copied["copiedFrom"] = json!("thread-zz");
            recorded[0] = event(copied);
        }
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_COPIED_LINE));
        assert_eq!(r.log(), Vec::<String>::new());
        assert_eq!(r.to(2, false).await.unwrap(), answer(json!({ "turns": 1 })), "the fork's own replies still rewind");
    });
}

// W4: sub-lifecycle (a sub-thread's checkpoints went with its machine)
#[test]
fn a_turn_on_a_deleted_machine_is_refused() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Copy);
        // Released between turn 1's start (T0 + 1000) and turn 2's (T0 + 2000).
        shape.entry = json!({ "sub": { "on": "boat", "repo": "r", "base": { "branch": "main", "sha": "abc" }, "branch": "b", "folder": "/f", "releasedAt": T0 + 1500 } });
        let r = Rig::new(shape);
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(said(&e), conflict(SUB_TURN_GONE_LINE));
        assert_eq!(r.log(), Vec::<String>::new());
        assert_eq!(r.to(2, true).await.unwrap(), answer(json!({ "turns": 1, "files": 2 })));
    });
}

// W5: rewind.test.ts:441
#[test]
fn files_with_no_checkpoint_are_refused() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Copy);
        shape.refs = false;
        let r = Rig::new(shape);
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_NO_CHECKPOINT_LINE));
        assert_eq!(r.restores(), Vec::<String>::new());
        assert_eq!(r.texts().len(), 3);
        // The conversation alone still goes, off the anchor the turn kept.
        assert_eq!(r.to(1, false).await.unwrap(), answer(json!({ "turns": 2 })));
        assert_eq!(r.forked(), vec![(SESSION.to_string(), ForkTurn::Anchor("a1".to_string()))]);
    });
}

// W5
#[test]
fn the_newest_checkpoint_of_the_turn_is_kept() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        let older = event(
            json!({ "type": "session.checkpoint", "workspaceId": W, "sessionId": SESSION, "turnId": turn(1), "threadId": T, "at": T0, "ref": "refs/old", "anchor": "old" }),
        );
        r.host.recorded.borrow_mut().insert(0, older);
        r.to(1, true).await.unwrap();
        assert_eq!(r.restores(), vec![checkpoint_ref(1)]);
        assert_eq!(r.forked(), vec![(SESSION.to_string(), ForkTurn::Anchor("a1".to_string()))], "the anchor is the newest's too");
    });
}

// W6: rewind.test.ts:497
#[test]
fn files_another_thread_touched_stay_and_the_answer_says_so() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        r.beside("running", None);
        // A running row counts as ending never.
        assert_eq!(r.to(2, true).await.unwrap(), answer(json!({ "turns": 1, "kept": REWIND_SHARED_LINE })));
        assert_eq!(r.restores(), Vec::<String>::new());
        assert_eq!(r.rewound(), None);
        assert_eq!(r.texts(), vec!["reply 1", "reply 2"], "the conversation still goes");
        // Its turn is over, and it still ended after every checkpoint this thread holds.
        r.host.rows.borrow()["beside-1"].borrow_mut().view.status = serde_json::from_value(json!("completed")).unwrap();
        r.host.rows.borrow()["beside-1"].borrow_mut().view.ended_at = Some(JsNumber((T0 + 9000) as f64));
        assert_eq!(r.to(1, true).await.unwrap(), answer(json!({ "turns": 1, "kept": REWIND_SHARED_LINE })));
        assert_eq!(r.restores(), Vec::<String>::new());
    });
}

// W6
#[test]
fn files_go_where_the_other_thread_ended_before_the_checkpoint() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        // Turn 1's checkpoint is at T0 + 1500.
        r.beside("completed", Some(T0 + 1400));
        assert_eq!(r.to(1, true).await.unwrap(), answer(json!({ "turns": 2, "files": 2 })));
        assert_eq!(r.restores(), vec![checkpoint_ref(1)]);
    });
}

// W6
#[test]
fn a_thread_of_another_folder_does_not_share_the_files() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        r.host.add_row(
            "elsewhere-1",
            row(
                view("elsewhere-1", "w9", json!({ "threadId": "thread-dd", "status": "running", "startedAt": T0 + 4000 })),
                "elsewhere-turn",
            ),
        );
        assert_eq!(r.to(1, true).await.unwrap(), answer(json!({ "turns": 2, "files": 2 })));
    });
}

// W7: rewind.test.ts:300
#[test]
fn a_harness_that_keeps_the_conversation_and_no_files_is_refused() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Revert);
        shape.kept = Some(CODEX_LEGACY_HISTORY);
        let r = Rig::new(shape);
        let e = r.to(1, false).await.unwrap_err();
        assert_eq!(said(&e), conflict(&rewind_kept_line(CODEX_LEGACY_HISTORY, false)));
        assert_eq!(r.log(), Vec::<String>::new());
        // The files alone go back, and the harness is not asked.
        assert_eq!(r.to(1, true).await.unwrap(), answer(json!({ "turns": 0, "files": 2 })));
        assert_eq!(r.reverted(), Vec::new());
        assert_eq!(r.restores(), vec![checkpoint_ref(1)]);
        assert_eq!(r.texts(), vec!["reply 1", "reply 2", "reply 3"]);
        assert_eq!(r.cut.borrow().len(), 0);
        assert_eq!(r.rewound().map(|w| w.before), Some(format!("{}-before-1", checkpoint_ref(1))));
    });
}

// W7
#[test]
fn a_harness_that_keeps_the_conversation_and_a_shared_folder_give_the_shared_line() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Revert);
        shape.kept = Some(CODEX_LEGACY_HISTORY);
        let r = Rig::new(shape);
        r.beside("running", None);
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_SHARED_LINE));
        assert_eq!(r.log(), Vec::<String>::new());
    });
}

// W8: rewind.test.ts:313
#[test]
fn a_copy_with_no_anchor_is_refused() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Copy);
        shape.anchors_from = 99;
        let r = Rig::new(shape);
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(said(&e), conflict(&rewind_no_anchor_line(AGENT)));
        assert_eq!(r.log(), Vec::<String>::new(), "refused before the files move");
        assert_eq!(r.forked(), Vec::new());
    });
}

// W8: rewind.test.ts:324
#[test]
fn a_harness_that_cannot_cut_moves_the_files_alone() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::None));
        assert_eq!(r.to(1, true).await.unwrap(), answer(json!({ "turns": 0, "files": 2 })));
        assert_eq!(r.restores(), vec![checkpoint_ref(1)]);
        assert_eq!(r.texts(), vec!["reply 1", "reply 2", "reply 3"]);
        assert_eq!(r.cut.borrow().len(), 0);
        // Neither conversation nor files.
        let e = r.to(1, false).await.unwrap_err();
        assert_eq!(said(&e), conflict(&rewind_no_anchor_line(AGENT)));
        // Where another thread shares the folder the files stay, and the line says why.
        r.beside("running", None);
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(said(&e), conflict(REWIND_SHARED_LINE));
        let e = r.to(1, false).await.unwrap_err();
        assert_eq!(said(&e), conflict(&rewind_no_anchor_line(AGENT)), "no files asked, so nothing is shared");
    });
}

// W9: rewind.test.ts:306
#[test]
fn a_refused_revert_puts_the_files_back() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Revert);
        let why = format!("codex would not cut the thread: timed out shutting down thread {SESSION} before revert");
        shape.revert = Err(why.clone());
        let r = Rig::new(shape);
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!((e.message.clone(), e.kind.clone()), (why, None));
        assert_eq!(
            r.restores(),
            vec![checkpoint_ref(1), format!("{}-before-1", checkpoint_ref(1))],
            "to the checkpoint, then back to how they stood"
        );
        assert_eq!(
            r.writes(),
            vec![format!("restore {}", checkpoint_ref(1)), "revert".to_string(), format!("restore {}-before-1", checkpoint_ref(1)),]
        );
        assert_eq!(r.texts(), vec!["reply 1", "reply 2", "reply 3"]);
        assert_eq!(r.rewound(), None);
        assert_eq!(r.emitted(), Vec::<Value>::new());
    });
}

// W9: rewind.test.ts:320
#[test]
fn a_refused_copy_puts_the_files_back() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Copy);
        shape.fork = Err("claude would not copy the session".to_string());
        let r = Rig::new(shape);
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!((e.message.clone(), e.kind.clone()), ("claude would not copy the session".to_string(), None));
        assert_eq!(
            r.writes(),
            vec![format!("restore {}", checkpoint_ref(1)), "fork_session".to_string(), format!("restore {}-before-1", checkpoint_ref(1)),]
        );
        assert_eq!(r.texts(), vec!["reply 1", "reply 2", "reply 3"]);
        assert_eq!(r.rewound(), None);
        // A refusal with no files moved has nothing to put back.
        let mut shape = Shape::new(Cuts::Copy);
        shape.fork = Err("claude would not copy the session".to_string());
        let r = Rig::new(shape);
        r.to(1, false).await.unwrap_err();
        assert_eq!(r.restores(), Vec::<String>::new());
    });
}

// W9
#[test]
fn a_failed_put_back_is_warned_and_the_refusal_stands() {
    run_local(async {
        let lines = console::capture();
        let mut shape = Shape::new(Cuts::Copy);
        shape.fork = Err("claude would not copy the session".to_string());
        let r = Rig::new(shape);
        *r.restore_fails.borrow_mut() = Some((1, HostError::new("the daemon went away")));
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(e.message, "claude would not copy the session");
        assert_eq!(r.restores().len(), 2);
        assert_eq!(
            lines.lines(),
            vec![format!("the files of thread {} were not put back after a refused rewind: the daemon went away", thread_word(T))],
        );
    });
}

// W9
#[test]
fn a_refused_first_restore_moves_nothing_and_fails() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        *r.restore_fails.borrow_mut() = Some((0, HostError::new("Sandbox machine is not running.")));
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(e.message, "Sandbox machine is not running.");
        assert_eq!(r.log(), vec![format!("restore {}", checkpoint_ref(1))], "nothing to put back, no copy asked");
        assert_eq!(r.texts().len(), 3);
    });
}

// W10: rewind.test.ts:297
#[test]
fn a_reverting_harness_is_asked_to_cut_before_the_first_cut_turn() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Revert));
        assert_eq!(r.to(1, true).await.unwrap(), answer(json!({ "turns": 2, "files": 2 })));
        assert_eq!(
            r.reverted(),
            vec![RevertAsk { session: SESSION.to_string(), cwd: Some(format!("/work/{W}")), at: RevertAt::BeforeTurn("a2".to_string()) }]
        );
        assert_eq!(r.writes()[..2].to_vec(), vec![format!("restore {}", checkpoint_ref(1)), "revert".to_string()], "the files go first");
        assert!(!r.log().iter().any(|l| l.starts_with("move_session")), "a revert moves no session");
        assert_eq!(r.texts(), vec!["reply 1"]);
    });
}

// W10: rewind.test.ts:343
#[test]
fn a_harness_whose_turns_named_no_anchor_is_handed_how_each_cut_turn_ended() {
    run_local(async {
        let refused = json!({ "status": "failed", "error": "Codex is not signed in where this thread runs", "refusal": "sign-in" });
        let mut shape = Shape::new(Cuts::Revert);
        shape.anchors_from = 99;
        shape.ends = vec![(2, refused.clone())];
        let r = Rig::new(shape);
        assert_eq!(r.to(1, true).await.unwrap(), answer(json!({ "turns": 2, "files": 2 })));
        assert_eq!(
            r.reverted(),
            vec![RevertAsk {
                session: SESSION.to_string(),
                cwd: Some(format!("/work/{W}")),
                at: RevertAt::Turns(vec![
                    CountedTurn { anchor: None, result: Some(result(refused)) },
                    CountedTurn { anchor: None, result: Some(result(json!({ "status": "completed", "text": "reply 3" }))) },
                ]),
            }],
        );
        assert_eq!(r.texts(), vec!["reply 1"]);
    });
}

// W10: rewind.test.ts:354
#[test]
fn each_cut_turn_is_handed_its_anchor_beside_its_end_where_it_kept_one() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Revert);
        shape.anchors_from = 3;
        let r = Rig::new(shape);
        r.to(1, false).await.unwrap();
        assert_eq!(
            r.reverted(),
            vec![RevertAsk {
                session: SESSION.to_string(),
                cwd: Some(format!("/work/{W}")),
                at: RevertAt::Turns(vec![
                    CountedTurn { anchor: None, result: Some(result(json!({ "status": "completed", "text": "reply 2" }))) },
                    CountedTurn {
                        anchor: Some("a3".to_string()),
                        result: Some(result(json!({ "status": "completed", "text": "reply 3" })))
                    },
                ]),
            }],
        );
        assert_eq!(r.restores(), Vec::<String>::new());
    });
}

// W10
#[test]
fn a_thread_with_no_agent_session_is_cut_in_the_transcript_alone() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Revert));
        for n in 1..=3 {
            r.host.rows.borrow()[&turn(n)].borrow_mut().view.claude_session_id = None;
        }
        assert_eq!(r.to(1, false).await.unwrap(), answer(json!({ "turns": 2 })));
        assert_eq!(r.reverted(), Vec::new(), "no session to cut");
        assert_eq!(r.texts(), vec!["reply 1"]);
    });
}

// W11: rewind.test.ts:228
#[test]
fn a_copying_harness_moves_the_session() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        assert_eq!(r.to(1, true).await.unwrap(), answer(json!({ "turns": 2, "files": 2 })));
        // The copy ends at the kept turn's own end.
        assert_eq!(r.forked(), vec![(SESSION.to_string(), ForkTurn::Anchor("a1".to_string()))]);
        assert!(r.log().contains(&format!("move_session {T} {SESSION} {COPY}")));
        assert_eq!(
            r.writes()[..3].to_vec(),
            vec![format!("restore {}", checkpoint_ref(1)), "fork_session".to_string(), format!("move_session {T} {SESSION} {COPY}")]
        );
    });
}

// W11
#[test]
fn a_fork_that_names_no_copy_moves_no_session() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Copy);
        shape.fork = Ok(None);
        let r = Rig::new(shape);
        assert_eq!(r.to(1, false).await.unwrap(), answer(json!({ "turns": 2 })));
        assert_eq!(r.forked().len(), 1);
        assert!(!r.log().iter().any(|l| l.starts_with("move_session")));
    });
}

// W12: rewind.test.ts:228
#[test]
fn a_copy_pays_off_the_cut_owed_and_the_rewind_is_recorded() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Copy);
        shape.record = Some(json!({ "harness": "claude", "cutOwed": "owed-turn" }));
        let r = Rig::new(shape);
        r.to(1, true).await.unwrap();
        let held = r.host.records.borrow()[T].clone();
        assert_eq!(held.cut_owed, None);
        assert_eq!(held.workspace_id, W);
        assert_eq!(
            norm(&serde_json::to_value(&held.rewound).unwrap()),
            norm(&json!({ "before": format!("{}-before-1", checkpoint_ref(1)), "at": T0 }))
        );
        // A harness that cuts by itself leaves the owed cut where it was; no files moved, no rewound mark.
        let mut shape = Shape::new(Cuts::Revert);
        shape.record = Some(json!({ "harness": "claude", "cutOwed": "owed-turn" }));
        let r = Rig::new(shape);
        r.to(1, false).await.unwrap();
        let held = r.host.records.borrow()[T].clone();
        assert_eq!(held.cut_owed, Some("owed-turn".to_string()));
        assert_eq!(held.rewound, None);
    });
}

// W12
#[test]
fn a_thread_with_rows_and_no_record_gets_one() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Copy);
        shape.record = None;
        let r = Rig::new(shape);
        r.to(1, true).await.unwrap();
        let held = r.host.records.borrow()[T].clone();
        assert_eq!(held.workspace_id, W);
        assert_eq!(held.harness, "claude");
        assert_eq!(held.rewound.map(|w| w.before), Some(format!("{}-before-1", checkpoint_ref(1))));
    });
}

// W13: rewind.test.ts:361
#[test]
fn a_harness_that_keeps_the_conversation_whole_leaves_the_files_back_with_undo() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Revert);
        shape.revert = Ok(Reverted::Kept(CODEX_LEGACY_HISTORY.to_string()));
        let r = Rig::new(shape);
        let e = r.to(1, true).await.unwrap_err();
        assert_eq!(said(&e), conflict(&rewind_kept_line(CODEX_LEGACY_HISTORY, true)));
        assert_eq!(
            r.log().into_iter().filter(|l| l != "open_thread").collect::<Vec<_>>(),
            vec![
                format!("restore {}", checkpoint_ref(1)),
                "revert".to_string(),
                format!("persist_sessions {W}"),
                format!("flush_transcript {W}"),
                "emit thread.rewound".to_string(),
            ],
            "rows persisted, flushed and emitted, then the line",
        );
        assert_eq!(r.texts(), vec!["reply 1", "reply 2", "reply 3"]);
        assert_eq!(r.cut.borrow().len(), 0);
        assert_eq!(r.slates.borrow().len(), 0);
        assert_eq!(r.rewound().map(|w| w.before), Some(format!("{}-before-1", checkpoint_ref(1))));
        assert_eq!(r.undo().await.unwrap(), answer(json!({ "turns": 0, "files": 2 })));
        assert_eq!(r.rewound(), None);
        // The conversation alone, asked again, is kept the same way and the answer says no files moved.
        let e = r.to(1, false).await.unwrap_err();
        assert_eq!(said(&e), conflict(&rewind_kept_line(CODEX_LEGACY_HISTORY, false)));
        assert_eq!(r.emitted().len(), 3);
        assert_eq!(r.texts(), vec!["reply 1", "reply 2", "reply 3"]);
    });
}

// W14: rewind.test.ts:228
#[test]
fn a_rewind_cuts_the_later_turns_and_their_rows() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        // Another thread in the folder keeps its events.
        let other = event(
            json!({ "type": "session.delta", "workspaceId": W, "sessionId": "S9", "turnId": "other-1", "threadId": "thread-bb", "kind": "text", "text": "other reply" }),
        );
        r.host.recorded.borrow_mut().push(other.clone());
        assert_eq!(r.to(1, true).await.unwrap(), answer(json!({ "turns": 2, "files": 2 })));
        assert_eq!(r.cut.borrow().clone(), vec![Some(vec![turn(2), turn(3)])], "this thread's cut turns, in transcript order");
        assert_eq!(r.texts(), vec!["reply 1", "other reply"]);
        assert_eq!(r.host.thread_events("thread-bb"), vec![other]);
        assert!(r.host.thread_events(T).iter().all(|e| serde_json::to_value(e).unwrap()["turnId"] == turn(1).as_str()));
        assert_eq!(r.slates.borrow().clone(), vec![(T.to_string(), turn(1), vec![turn(2), turn(3)])]);
        assert_eq!(r.emitted(), vec![rewound_event()]);
        assert_eq!(
            r.full(),
            vec![
                "open_thread".to_string(),
                format!("restore {}", checkpoint_ref(1)),
                "fork_session".to_string(),
                format!("move_session {T} {SESSION} {COPY}"),
                "drop_from_transcript".to_string(),
                "open_thread".to_string(),
                "slates_rewound".to_string(),
                format!("persist_sessions {W}"),
                format!("flush_transcript {W}"),
                "emit thread.rewound".to_string(),
            ],
            "the turns are read, the files and the session go, then the cut, a read of what is left, the slate, then the rows",
        );
        assert_eq!(r.rewound().map(|w| w.before), Some(format!("{}-before-1", checkpoint_ref(1))));
        assert_eq!(r.host.hooks.called("reap_kept"), vec![format!("reap_kept({T})")]);
        // The rows stay: only the transcript is cut.
        assert_eq!(r.host.rows.borrow().len(), 3);
    });
}

// W14: rewind.test.ts:268
#[test]
fn a_rewind_to_turn_one_of_three_answers_two_turns() {
    run_local(async {
        let r = Rig::new(Shape::new(Cuts::Copy));
        assert_eq!(r.to(1, false).await.unwrap(), answer(json!({ "turns": 2 })));
        assert_eq!(r.restores(), Vec::<String>::new(), "the conversation alone leaves the files where they are");
        assert_eq!(r.rewound(), None);
        assert_eq!(r.texts(), vec!["reply 1"]);
        let r = Rig::new(Shape::new(Cuts::Copy));
        assert_eq!(r.to(2, false).await.unwrap(), answer(json!({ "turns": 1 })));
        assert_eq!(r.texts(), vec!["reply 1", "reply 2"]);
        assert_eq!(r.cut.borrow().clone(), vec![Some(vec![turn(3)])]);
    });
}

// W14: rewind.test.ts:276
#[test]
fn the_row_lists_the_turn_now_last() {
    run_local(async {
        let mut shape = Shape::new(Cuts::Copy);
        shape.ends = vec![(1, json!({ "status": "failed", "error": "API Error: 529 overloaded\nretry" }))];
        let r = Rig::new(shape);
        let latest = || r.host.rows.borrow()[&turn(3)].borrow().view.clone();
        assert_eq!(latest().last_line, Some("reply 3".to_string()));
        r.to(2, false).await.unwrap();
        assert_eq!((latest().last_line, latest().failure), (Some("reply 2".to_string()), None));
        r.to(1, false).await.unwrap();
        assert_eq!((latest().last_line, latest().failure), (None, Some("API Error: 529 overloaded".to_string())));
    });
}

// sessions.ts:1379-1382 writes the record in place: the owed cut goes and `rewound` lands after the keys held. Expected
// bytes: wsp-core/tests/fixtures/thread_record_order/gen.mjs, road `rewind`, with the checkpoint this world takes.
const ORDER_CASES: &str = include_str!("../../wsp-core/tests/fixtures/thread_record_order/cases.json");

#[test]
fn a_rewind_writes_the_threads_record_in_the_bytes_node_writes() {
    let all: Vec<Value> = serde_json::from_str(ORDER_CASES).expect("the cases are JSON");
    let rewinds: Vec<Value> = all.into_iter().filter(|c| c["road"] == "rewind").collect();
    assert_eq!(rewinds.len(), 5);
    for case in rewinds {
        let befores: Vec<Option<wsp_core::types::internal::ThreadRecord>> =
            if case["before"].is_object() { record_variants(&case["before"]).into_iter().map(Some).collect() } else { vec![None] };
        for before in befores {
            let case = case.clone();
            run_local(async move {
                let mut shape = Shape::new(Cuts::Copy);
                shape.record = None;
                let r = Rig::new(shape);
                if let Some(record) = before {
                    r.host.records.borrow_mut().insert(T.to_string(), record);
                }
                r.to(1, true).await.unwrap();
                let held = r.host.records.borrow()[T].clone();
                let before = format!("{}-before-1", checkpoint_ref(1));
                let expected = case["after"]
                    .as_str()
                    .expect("the bytes")
                    .replace("\"before\":\"abc\",\"at\":77", &format!("\"before\":{},\"at\":{}", json!(before), T0));
                assert_eq!(serde_json::to_string(&held).expect("serializes"), expected, "{}", case["name"]);
            });
        }
    }
}

/// The record as a road made it (its keys in the order the TS inserted them), and again as a restart reads it back,
/// which writes the struct's order, where that is the same record.
fn record_variants(before: &Value) -> Vec<wsp_core::types::internal::ThreadRecord> {
    use wsp_core::types::internal::ThreadRecord;
    let road = ThreadRecord::from_map(before.as_object().expect("a record").clone()).expect("the record literal");
    let hydrated: ThreadRecord = serde_json::from_value(before.clone()).expect("the record literal");
    let same = serde_json::to_string(&hydrated).expect("serializes") == serde_json::to_string(&road).expect("serializes");
    if same {
        vec![road, hydrated]
    } else {
        vec![road]
    }
}
