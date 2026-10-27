// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-index.test.ts at 65f6b89c7, case for case. The runtime's session index: the rows a
//! workspace keeps, what a restart reads back and cuts, a thread's folder and session across a restart, and what a send
//! into a thread does while its first turn is still reaching the machine.
#[macro_use]
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::comeback::{history, opts, rows};
use common::*;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AdapterEvent, ExecFailure, HarnessAdapter, HarnessCatalogAnswer, HarnessExec, HarnessSession, HarnessStartOptions,
    SharedFuture, SteerOutcome, TurnImage, TurnResult,
};
use wsp_core::console::capture;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};
use wsp_wire::views::session::{SessionOrigin, SessionView};
use wsp_words::thread_read::{thread_messages, thread_reply_rows, thread_result};
use wsp_words::views::session::fold_threads;
use wsp_words::words::turn::{notify_line, NotifyLength};

const HUNG_ID: &str = "55555555-5555-4555-8555-555555555555";
const CUT_ID: &str = "77777777-7777-4777-8777-777777777777";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// `turns()`: one completed turn per start, under the resume id when given, printed a tick after the start.
    Turn,
    /// `hung`: announces itself and never settles.
    Hung,
    /// `cutting`: the prompt `cut` is a transport cut, every other prompt completes.
    Cutting,
    /// `dying`: dies before init: a done and an end, under the resume id or a fresh local one.
    Dying,
    /// `bornDead`: the first start never announces a session; later ones are `Turn`.
    BornDead,
    /// `refusing`: the start throws.
    Refusing,
    /// `oneThenNothing`: one good turn, then a start that throws.
    OneThenNothing,
    /// `steering`: announces a tick after, never settles, takes a steer.
    Steering,
    /// `holding`: announces a tick after; ends when the test says; interrupt ends it as interrupted.
    Holding,
    /// `pending`: announces at once; settles (with nothing printed) when the test says.
    Pending,
}

struct Zoo {
    kind: Kind,
    /// `{ ...turns().adapter(ctx), probeCatalog: async () => (await probed, null) }`.
    probe: Option<Rc<Gate>>,
    /// `every.start({ ...o, resume: undefined })`.
    fresh: bool,
    /// `onPrompt(prompt, one, rest)`: this prompt goes to `one`.
    pick: Option<(String, Rc<Zoo>)>,
    starts: RefCell<Vec<Begun>>,
    ends: RefCell<Vec<Rc<dyn Fn(TurnResult)>>>,
    opened: Cell<u32>,
}

#[derive(Clone)]
struct Begun {
    prompt: String,
    resume: Option<String>,
    cwd: Option<String>,
}

fn zoo(kind: Kind) -> Rc<Zoo> {
    Rc::new(Zoo { kind, probe: None, fresh: false, pick: None, starts: RefCell::default(), ends: RefCell::default(), opened: Cell::new(0) })
}

fn zoo_with(kind: Kind, f: impl FnOnce(&mut Zoo)) -> Rc<Zoo> {
    let mut z = Rc::try_unwrap(zoo(kind)).ok().expect("a fresh zoo");
    f(&mut z);
    Rc::new(z)
}

fn turns() -> Rc<Zoo> {
    zoo(Kind::Turn)
}

fn on_prompt(prompt: &str, one: &Rc<Zoo>, rest: &Rc<Zoo>) -> Rc<Zoo> {
    zoo_with(rest.kind, |z| z.pick = Some((prompt.to_string(), one.clone())))
}

fn ev(doc: Value) -> AdapterEvent {
    serde_json::from_value(doc).expect("an adapter event")
}

fn result_of(doc: Value) -> TurnResult {
    serde_json::from_value(doc).expect("a turn result")
}

impl Zoo {
    fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let me = self.clone();
        Arc::new(move |_ctx: HarnessAdapterContext| Ok(Arc::new(ZooAdapter { me: me.clone() }) as Arc<dyn HarnessAdapter>))
    }
}

struct ZooAdapter {
    me: Rc<Zoo>,
}

fn emit(on: &wsp_adapters::port::OnEvent, lines: Vec<Value>) {
    for line in lines {
        on(ev(line));
    }
}

impl HarnessAdapter for ZooAdapter {
    fn start(&self, mut o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let me = &self.me;
        if let Some((prompt, one)) = &me.pick {
            if o.prompt == *prompt {
                return ZooAdapter { me: one.clone() }.start(o);
            }
        }
        if me.fresh {
            o.resume = None;
        }
        me.starts.borrow_mut().push(Begun { prompt: o.prompt.clone(), resume: o.resume.clone(), cwd: o.cwd.clone() });
        me.opened.set(me.opened.get() + 1);
        let n = me.opened.get();
        let on = o.on_event.clone().expect("an event sink");
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let tx = Rc::new(RefCell::new(Some(tx)));
        let cwd = o.cwd.clone().unwrap_or_else(|| "/root/work".into());
        let fresh_id = || format!("66666666-0000-4000-8000-{n:012}");
        let mut kind = me.kind;
        if kind == Kind::BornDead && n > 1 {
            kind = Kind::Turn;
        }
        let mut local = o.resume.clone().unwrap_or_else(fresh_id);
        let mut interrupt: Option<Rc<dyn Fn()>> = None;
        let mut steers = false;
        match kind {
            Kind::Refusing => return Err(AdapterError("the harness would not launch".into())),
            Kind::OneThenNothing if n > 1 => return Err(AdapterError("the harness would not launch".into())),
            Kind::Turn | Kind::Cutting | Kind::OneThenNothing => {
                let cut = kind == Kind::Cutting && o.prompt == "cut";
                if kind == Kind::Cutting {
                    local = o.resume.clone().unwrap_or_else(|| CUT_ID.to_string());
                }
                let res = if cut {
                    result_of(json!({ "status": "failed", "error": "stopped after 15m 00s with no output for 10m" }))
                } else {
                    result_of(
                        json!({ "status": "completed", "text": if kind == Kind::OneThenNothing { "all green".to_string() } else { o.prompt.clone() } }),
                    )
                };
                let (id, prompt) = (local.clone(), o.prompt.clone());
                let r = res.clone();
                let gate = me.probe.clone();
                let _ = gate;
                tokio::task::spawn_local(async move {
                    let mut lines = vec![
                        json!({ "type": "session.start", "sessionId": id, "model": "claude-sonnet-4-5", "cwd": if kind == Kind::OneThenNothing { "/root/app".to_string() } else { cwd } }),
                    ];
                    if kind == Kind::OneThenNothing {
                        lines.push(json!({ "type": "turn.delta", "sessionId": id, "kind": "text", "text": "all green" }));
                    }
                    let _ = prompt;
                    lines.push(json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&r).expect("a result") }));
                    lines.push(json!({ "type": "session.end", "sessionId": id, "exitCode": if cut { Value::Null } else { json!(0) }, "sawResult": !cut }));
                    emit(&on, lines);
                    if let Some(tx) = tx.borrow_mut().take() {
                        let _ = tx.send(r);
                    }
                });
            }
            Kind::Hung => {
                local = HUNG_ID.to_string();
                emit(
                    &on,
                    vec![json!({ "type": "session.start", "sessionId": HUNG_ID, "model": "claude-sonnet-4-5", "cwd": "/root/work" })],
                );
            }
            Kind::Dying => {
                let res = result_of(json!({ "status": "failed", "error": "claude exited before init (exit code 1)" }));
                let (id, r) = (local.clone(), res.clone());
                tokio::task::spawn_local(async move {
                    emit(
                        &on,
                        vec![
                            json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&r).expect("a result") }),
                            json!({ "type": "session.end", "sessionId": id, "exitCode": 1, "sawResult": true }),
                        ],
                    );
                    if let Some(tx) = tx.borrow_mut().take() {
                        let _ = tx.send(r);
                    }
                });
            }
            Kind::BornDead => {
                // The real adapter's local id for a turn that never announced a session, under which its events go.
                local = "local-dead".into();
                let res = result_of(
                    json!({ "status": "failed", "error": "the machine could not be reached from this computer after 6 attempts over 23s" }),
                );
                let r = res.clone();
                tokio::task::spawn_local(async move {
                    emit(
                        &on,
                        vec![
                            json!({ "type": "turn.done", "sessionId": "local-dead", "result": serde_json::to_value(&r).expect("a result") }),
                            json!({ "type": "session.end", "sessionId": "local-dead", "exitCode": null, "sawResult": false }),
                        ],
                    );
                    if let Some(tx) = tx.borrow_mut().take() {
                        let _ = tx.send(r);
                    }
                });
            }
            Kind::Steering => {
                steers = true;
                let id = local.clone();
                let on2 = on.clone();
                tokio::task::spawn_local(async move {
                    on2(ev(json!({ "type": "session.start", "sessionId": id, "model": "claude-sonnet-4-5", "cwd": "/root/app" })))
                });
            }
            Kind::Holding => {
                let id = local.clone();
                let on2 = on.clone();
                tokio::task::spawn_local(async move {
                    on2(ev(json!({ "type": "session.start", "sessionId": id, "model": "claude-sonnet-4-5", "cwd": "/root/app" })))
                });
                let (id, on3, slot) = (local.clone(), on.clone(), tx.clone());
                let end: Rc<dyn Fn(TurnResult)> = Rc::new(move |r: TurnResult| {
                    emit(
                        &on3,
                        vec![
                            json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&r).expect("a result") }),
                            json!({ "type": "session.end", "sessionId": id, "exitCode": 0, "sawResult": true }),
                        ],
                    );
                    if let Some(tx) = slot.borrow_mut().take() {
                        let _ = tx.send(r);
                    }
                });
                me.ends.borrow_mut().push(end.clone());
                interrupt = Some(Rc::new(move || end(result_of(json!({ "status": "interrupted" })))));
            }
            Kind::Pending => {
                emit(
                    &on,
                    vec![json!({ "type": "session.start", "sessionId": HUNG_ID, "model": "claude-sonnet-4-5", "cwd": "/root/work" })],
                );
                local = HUNG_ID.to_string();
                let slot = tx.clone();
                me.ends.borrow_mut().push(Rc::new(move |r: TurnResult| {
                    if let Some(tx) = slot.borrow_mut().take() {
                        let _ = tx.send(r);
                    }
                }));
            }
        }
        let finished: LocalBoxFuture<'static, TurnResult> = async move {
            match rx.await {
                Ok(r) => r,
                Err(_) => futures::future::pending().await,
            }
        }
        .boxed_local();
        Ok(Arc::new(ZooSession { local_id: local, finished: finished.shared(), interrupt, steers }))
    }

    fn steers(&self) -> bool {
        self.me.kind == Kind::Steering
    }

    fn probe_catalog(
        &self,
        _exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<HarnessCatalogAnswer>, ExecFailure>>> {
        let gate = self.me.probe.clone()?;
        Some(
            async move {
                gate.wait().await;
                Ok(None)
            }
            .boxed_local(),
        )
    }
}

struct ZooSession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
    interrupt: Option<Rc<dyn Fn()>>,
    steers: bool,
}

impl HarnessSession for ZooSession {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let f = self.interrupt.clone();
        async move {
            if let Some(f) = f {
                f();
            }
        }
        .boxed_local()
    }
    fn steer(
        &self,
        _p: String,
        _id: Option<String>,
        _i: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        self.steers.then(|| async { Ok(SteerOutcome::Accepted) }.boxed_local())
    }
}

type Handle = Rc<dyn wsp_core::types::wiring::SessionHandle>;

fn view(h: &Handle) -> Value {
    j(&h.view())
}

fn thread(h: &Handle) -> String {
    view(h)["threadId"].as_str().expect("a thread").to_string()
}

fn on(prompt: &str, thread: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), thread: Some(thread.into()), ..Default::default() }
}

fn named(prompt: &str, harness: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), harness: Some(harness.into()), ..Default::default() }
}

fn types_of(events: &[Value]) -> Vec<String> {
    events.iter().map(|e| e["type"].as_str().unwrap_or_default().to_string()).collect()
}

fn starts_of(events: &[Value]) -> Vec<Value> {
    events.iter().filter(|e| e["type"] == "session.start").cloned().collect()
}

fn folded(rows: &[Value]) -> Vec<Value> {
    let views: Vec<SessionView> = rows.iter().map(|r| serde_json::from_value(r.clone()).expect("a row")).collect();
    fold_threads(&views).iter().map(j).collect()
}

fn events_of(history: &[Value]) -> Vec<wsp_wire::views::session_events::SessionEvent> {
    history.iter().map(|e| serde_json::from_value(e.clone()).expect("an event")).collect()
}

fn blank_exec(backend: &Arc<StubBackend>) {
    backend.set_exec_impl(exec_fn(|_, _| Ok(exec_ok(""))));
}

// "runtime session index" > "writes the index to the store and lists the rows after a restart; a turn the restart cut reads as ended"
#[test]
fn writes_the_index_to_the_store_and_lists_the_rows_after_a_restart_a_turn_the_restart_cut_reads_as_ended() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let (t, hung) = (turns(), zoo(Kind::Hung));
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", t.factory()).adapter("hung", hung.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let mut first = opts("first");
        first.cwd = Some("/root/app".into());
        rt1.rt.sessions().start(&ws, first, None).await.expect("a start").finished().await;
        rt1.rt.sessions().start(&ws, named("second", "hung"), None).await.expect("a start");
        let before = rows(&rt1.rt, &ws).await;
        assert_eq!(before.iter().map(|s| s["status"].clone()).collect::<Vec<_>>(), ["completed", "running"]);
        rt1.rt.close().await.expect("a close");
        let stored = state.store.get("sessions", &ws).await.expect("a read").expect("a document");
        assert_eq!(stored["sessions"].as_array().map(Vec::len), Some(2));

        let rt2 = runtime(&backend).store(&state.store).build();
        let after = rows(&rt2.rt, &ws).await;
        assert_eq!(after.len(), 2);
        assert_eq!(after[0], before[0]);
        assert_matches(
            &after[0],
            json!({ "status": "completed", "prompt": "first", "cwd": "/root/app", "model": "claude-sonnet-4-5", "harness": "claude" }),
        );
        assert_matches(&after[1], json!({ "id": HUNG_ID, "prompt": "second", "status": "failed" }));
        assert!(after[1]["endedAt"].is_number());
        assert_eq!(after[1]["threadId"], before[1]["threadId"]);
        let hist = history(&rt2.rt, &ws).await;
        assert_matches(
            hist.last().expect("an event"),
            json!({ "type": "session.end", "sessionId": HUNG_ID, "threadId": before[1]["threadId"], "reason": "host restarted while the agent was working" }),
        );
        assert_eq!(hist[hist.len() - 1]["turnId"], hist[hist.len() - 2]["turnId"]);
        assert_eq!(j(&rt2.rt.sessions().interrupt(HUNG_ID, None, None).await.expect("an interrupt")), json!({ "outcome": "not-running" }));
        rt2.rt.close().await.expect("a close");
        // the cut turn is written back ended, so a second restart does not end it again
        let rt3 = runtime(&backend).store(&state.store).build();
        assert_eq!(types_of(&history(&rt3.rt, &ws).await).iter().filter(|t| *t == "session.end").count(), 2);
        rt3.rt.close().await.expect("a close");
    });
}

// "..." > "a resume after a turn the restart cut stamps afterCut on its start; the resume after that does not"
#[test]
fn a_resume_after_a_turn_the_restart_cut_stamps_aftercut_on_its_start_the_resume_after_that_does_not() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let (t, hung) = (turns(), zoo(Kind::Hung));
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", t.factory()).adapter("hung", hung.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let th = thread(&rt1.rt.sessions().start(&ws, named("first", "hung"), None).await.expect("a start"));
        rt1.rt.close().await.expect("a close");

        // The thread ran on hung and goes on running on hung: the agent a thread's rows carry is the one its next
        // turn runs on, and this host answers for it again after the restart.
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", t.factory()).adapter("hung", t.factory()).build();
        rt2.rt.sessions().start(&ws, on("second", &th), None).await.expect("a start").finished().await;
        rt2.rt.sessions().start(&ws, on("third", &th), None).await.expect("a start").finished().await;
        let starts = starts_of(&history(&rt2.rt, &ws).await);
        assert_eq!(
            starts.iter().map(|e| json!([e["prompt"], e.get("afterCut").cloned().unwrap_or(Value::Null)])).collect::<Vec<_>>(),
            [json!(["first", null]), json!(["second", true]), json!(["third", null])]
        );
        assert_eq!(starts.iter().map(|e| e["threadId"].to_string()).collect::<std::collections::HashSet<_>>().len(), 1);
        rt2.rt.close().await.expect("a close");
    });
}

// "..." > "the start of the turn that opened its thread says so, a later one does not, and each says who opened the thread"
#[test]
fn the_start_of_the_turn_that_opened_its_thread_says_so_a_later_one_does_not_and_each_says_who_opened_the_thread() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", turns().factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let first = fx
            .rt
            .sessions()
            .start(
                &ws,
                StartOptions {
                    prompt: "first".into(),
                    harness: Some("claude".into()),
                    started_by: Some(SessionOrigin::Cli),
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("a start");
        first.finished().await;
        fx.rt
            .sessions()
            .start(&ws, StartOptions { thread: Some(thread(&first)), started_by: Some(SessionOrigin::Agent), ..opts("second") }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        let starts = starts_of(&history(&fx.rt, &ws).await);
        let facts: Vec<Value> =
            starts.iter().map(|e| json!([e["prompt"], e.get("opensThread").cloned().unwrap_or(Value::Null), e["startedBy"]])).collect();
        assert_eq!(facts, [json!(["first", true, "cli"]), json!(["second", null, "cli"])]);
        fx.rt.close().await.expect("a close");
    });
}

// "..." > "a resume after a turn the transport cut stamps afterCut; a turn that failed with an exit code, or finished, leaves the next start plain"
#[test]
fn a_resume_after_a_turn_the_transport_cut_stamps_aftercut_a_turn_that_failed_with_an_exit_code_or_finished_leaves_the_next_start_plain() {
    drive(|| async {
        let adapter = on_prompt("dies", &zoo(Kind::Dying), &zoo(Kind::Cutting));
        let fx = runtime(&StubBackend::new(None)).adapter("claude", adapter.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let cut = fx.rt.sessions().start(&ws, opts("cut"), None).await.expect("a start");
        cut.finished().await;
        let th = thread(&cut);
        for p in ["second", "third", "dies", "fifth"] {
            fx.rt.sessions().start(&ws, on(p, &th), None).await.expect("a start").finished().await;
        }
        let starts = starts_of(&history(&fx.rt, &ws).await);
        assert_eq!(
            starts.iter().map(|e| json!([e["prompt"], e.get("afterCut").cloned().unwrap_or(Value::Null)])).collect::<Vec<_>>(),
            [json!(["cut", null]), json!(["second", true]), json!(["third", null]), json!(["fifth", null])]
        );
        // a fresh thread has no previous turn
        fx.rt.sessions().start(&ws, opts("cut"), None).await.expect("a start").finished().await;
        let all = starts_of(&history(&fx.rt, &ws).await);
        let opened = all.last().expect("a start");
        assert_eq!(opened["prompt"], "cut");
        assert!(opened.get("afterCut").is_none());
        fx.rt.close().await.expect("a close");
    });
}

// "..." > "a harness that dies before init leaves its row and the workspace without a session id: no harness announced one"
// (The TS adapter also hands back its local id as `claudeSessionId` until init re-keys it; `HarnessSession` has no such member, so
// that quirk is not modelled: the row must hold no session id either way.)
#[test]
fn a_harness_that_dies_before_init_leaves_its_row_and_the_workspace_without_a_session_id_no_harness_announced_one() {
    drive(|| async {
        let state = sqlite_state();
        let fx = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", zoo(Kind::Dying).factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let handle = fx.rt.sessions().start(&ws, opts("first"), None).await.expect("a start");
        handle.finished().await;
        let row = rows(&fx.rt, &ws).await.remove(0);
        assert_matches(&row, json!({ "id": handle.id(), "status": "failed", "prompt": "first" }));
        assert!(row["threadId"].is_string());
        assert!(row.get("claudeSessionId").is_none());
        assert!(j(&fx.rt.workspaces().get(&ws, None, None).await.expect("a workspace")).get("claudeSessionId").is_none());
        // the dead turn's events still carry the adapter's local id, so the row is found by its id, not by a session
        assert_eq!(
            history(&fx.rt, &ws).await.iter().map(|e| e["sessionId"].clone()).collect::<Vec<_>>(),
            [json!(handle.id()), json!(handle.id())]
        );
        fx.rt.close().await.expect("a close");
        let stored = state.store.get("sessions", &ws).await.expect("a read").expect("a document");
        assert_eq!(stored["sessions"].as_array().map(Vec::len), Some(1));
        assert!(stored["sessions"][0].get("claudeSessionId").is_none());
    });
}

// "..." > "a resumed turn that dies before init keeps the resume id on its row: the harness answered that id in an earlier turn"
#[test]
fn a_resumed_turn_that_dies_before_init_keeps_the_resume_id_on_its_row_the_harness_answered_that_id_in_an_earlier_turn() {
    drive(|| async {
        let state = sqlite_state();
        let adapter = on_prompt("again", &zoo(Kind::Dying), &turns());
        let fx = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", adapter.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        fx.rt.sessions().start(&ws, opts("first"), None).await.expect("a start").finished().await;
        let first = rows(&fx.rt, &ws).await.remove(0);
        let resume = first["claudeSessionId"].clone();
        fx.rt
            .sessions()
            .start(&ws, on("again", first["threadId"].as_str().expect("a thread")), None)
            .await
            .expect("a start")
            .finished()
            .await;
        let listed = rows(&fx.rt, &ws).await;
        assert_eq!(
            listed.iter().map(|r| json!([r["prompt"], r["status"], r["threadId"], r["claudeSessionId"]])).collect::<Vec<_>>(),
            [json!(["first", "failed", first["threadId"], resume])]
        );
        fx.rt.close().await.expect("a close");
        let stored = state.store.get("sessions", &ws).await.expect("a read").expect("a document");
        assert_eq!(stored["sessions"].as_array().expect("rows").iter().map(|r| r["claudeSessionId"].clone()).collect::<Vec<_>>(), [resume]);
    });
}

// "..." > "a start naming a thread whose harness never announced a session runs the message as a first turn on that thread; named again, the thread is resumed; an unknown thread is refused"
#[test]
fn a_start_naming_a_thread_whose_harness_never_announced_a_session_runs_the_message_as_a_first_turn_on_that_thread_named_again_the_thread_is_resumed_an_unknown_thread_is_refused(
) {
    drive(|| async {
        let born_dead = zoo(Kind::BornDead);
        let fx = runtime(&StubBackend::new(None)).adapter("claude", born_dead.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let other = create_on(&fx.rt, CreateOn::golden("b"), None).await.expect("a workspace").view.id;
        let dead = fx.rt.sessions().start(&ws, opts("first"), None).await.expect("a start");
        assert_eq!(j(&dead.finished().await)["status"], "failed");
        let th = thread(&dead);
        assert_eq!(
            rows(&fx.rt, &ws)
                .await
                .iter()
                .map(|r| json!([r["threadId"], r.get("claudeSessionId").cloned().unwrap_or(Value::Null)]))
                .collect::<Vec<_>>(),
            [json!([th, null])]
        );

        let again = fx.rt.sessions().start(&ws, on("again", &th), None).await.expect("a start");
        assert_eq!(j(&again.finished().await)["status"], "completed");
        assert_eq!(born_dead.starts.borrow()[1].resume, None);
        assert_eq!(thread(&again), th);
        let listed = rows(&fx.rt, &ws).await;
        assert_eq!(
            listed.iter().map(|r| json!([r["prompt"], r["status"], r["threadId"]])).collect::<Vec<_>>(),
            [json!(["first", "failed", th]), json!(["again", "completed", th])]
        );
        let session = listed[1]["claudeSessionId"].as_str().expect("a session").to_string();

        let third = fx.rt.sessions().start(&ws, on("more", &th), None).await.expect("a start");
        assert_eq!(j(&third.finished().await)["status"], "completed");
        assert_eq!(born_dead.starts.borrow()[2].resume.as_deref(), Some(session.as_str()));
        assert_eq!(thread(&third), th);

        let refused = fx.rt.sessions().start(&ws, on("x", "no-such-thread"), None).await.err().expect("a refusal");
        assert!(refused.message.contains("no thread no-such-thread in this folder"), "{}", refused.message);
        let refused = fx.rt.sessions().start(&other, on("x", &th), None).await.err().expect("a refusal");
        assert!(refused.message.contains(&format!("no thread {th} in this folder")), "{}", refused.message);
        assert_eq!(born_dead.starts.borrow().len(), 3);
    });
}

/// The slow-probe fixture of the cases that follow: a stub whose machine answers every command with nothing, and a harness
/// whose catalog probe waits on a gate the test opens.
fn slow(backend: &Arc<StubBackend>, kind: Kind) -> (Rc<Zoo>, Rc<Gate>) {
    blank_exec(backend);
    let gate = Gate::new();
    let g = gate.clone();
    (zoo_with(kind, move |z| z.probe = Some(g)), gate)
}

// "..." > "a thread whose first turn is still reaching the machine is listed, running, under one row; a start that never opens leaves none"
#[test]
fn a_thread_whose_first_turn_is_still_reaching_the_machine_is_listed_running_under_one_row_a_start_that_never_opens_leaves_none() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let (adapter, probed) = slow(&backend, Kind::Turn);
        let fx = runtime(&backend).adapter("claude", adapter.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let (rt, w) = (fx.rt.clone(), ws.clone());
        let starting = tokio::task::spawn_local(async move {
            rt.sessions()
                .start(&w, StartOptions { prompt: "build it".into(), started_by: Some(SessionOrigin::Cli), ..Default::default() }, None)
                .await
        });
        let (rt, w) = (fx.rt.clone(), ws.clone());
        wait!([rt, w] rows(&rt, &w).await.len() == 1);
        let pending = rows(&fx.rt, &ws).await.remove(0);
        assert_matches(
            &pending,
            json!({ "workspaceId": ws, "harness": "claude", "status": "running", "startedBy": "cli", "prompt": "build it" }),
        );
        assert_matches(&json!(folded(&rows(&fx.rt, &ws).await)), json!([{ "id": pending["threadId"], "status": "running", "turns": 1 }]));

        probed.open();
        let handle = starting.await.expect("the start").expect("a start");
        assert_eq!(thread(&handle), pending["threadId"].as_str().expect("a thread"));
        assert_eq!(j(&handle.finished().await)["status"], "completed");
        let listed = rows(&fx.rt, &ws).await;
        assert_eq!(
            listed.iter().map(|r| json!([r["prompt"], r["status"], r["threadId"]])).collect::<Vec<_>>(),
            [json!(["build it", "completed", pending["threadId"]])]
        );
        assert_eq!(listed[0]["id"], json!(handle.id()));
        fx.rt.close().await.expect("a close");
    });
}

// "..." > "a second send inside that window writes no row of its own and waits: the turn still reaching the machine is the one a wait is waiting on, and this send's fate follows it"
#[test]
fn a_second_send_inside_that_window_writes_no_row_of_its_own_and_waits_the_turn_still_reaching_the_machine_is_the_one_a_wait_is_waiting_on_and_this_sends_fate_follows_it(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        blank_exec(&backend);
        let probed = Gate::new();
        // `start` throws for "and this too"; every other start is the plain turn behind a probe that waits.
        let inner = zoo_with(Kind::Turn, |z| z.probe = Some(probed.clone()));
        let refuse = zoo(Kind::Refusing);
        let slow = zoo_with(Kind::Turn, |z| {
            z.probe = Some(probed.clone());
            z.pick = Some(("and this too".into(), refuse.clone()));
        });
        let fx = runtime(&backend).adapter("claude", slow.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view;
        let ends = fx.events();
        let (rt, w) = (fx.rt.clone(), ws.id.clone());
        let first = tokio::task::spawn_local(async move { rt.sessions().start(&w, opts("build it"), None).await });
        let (rt, w) = (fx.rt.clone(), ws.id.clone());
        wait!([rt, w] rows(&rt, &w).await.len() == 1);
        let th = rows(&fx.rt, &ws.id).await[0]["threadId"].as_str().expect("a thread").to_string();

        // The thread is already spoken for by a turn no harness holds yet, so this one waits for it rather than
        // writing a row beside it and becoming the row every client folds the thread off.
        let (rt, w, t) = (fx.rt.clone(), ws.id.clone(), th.clone());
        let second = tokio::task::spawn_local(async move { rt.sessions().start(&w, on("and this too", &t), None).await.map(|_| ()) });
        sleep_ms(50).await;
        let listed = rows(&fx.rt, &ws.id).await;
        assert_eq!(listed.len(), 1);
        assert_matches(&json!(folded(&listed)), json!([{ "id": th, "status": "running", "turns": 1 }]));

        probed.open();
        let handle = first.await.expect("the first").expect("a start");
        assert_eq!(j(&handle.finished().await)["status"], "completed");
        let refused = second.await.expect("the second").expect_err("a refusal");
        assert!(refused.message.contains("the harness would not launch"), "{}", refused.message);
        // The second send waited out the first turn and gave up only as the thread's next turn, resuming the session the
        // first announced: its end follows the first turn's own, so a wait following the thread for that turn got that
        // turn's fate and not this send's.
        let _ = inner;
        let seen: Vec<Value> = ends
            .of("session.end")
            .iter()
            .map(|e| json!([e["turnId"] == json!(handle.turn_id()), e.get("reason").cloned().unwrap_or(Value::Null)]))
            .collect();
        assert_eq!(seen, [json!([true, null]), json!([false, "the harness would not launch"])]);
        assert_matches(
            &json!(folded(&rows(&fx.rt, &ws.id).await)),
            json!([{ "id": th, "status": "completed", "cwd": j(&ws)["project"]["path"], "turns": 1 }]),
        );
        fx.rt.close().await.expect("a close");
    });
}

// "..." > "a send named at a thread while its first turn is still reaching the machine waits for that turn and resumes its session, with %s: one harness session per thread, sends taken as they reach the loop"
#[test]
fn a_send_named_at_a_thread_while_its_first_turn_is_still_reaching_the_machine_waits_for_that_turn_and_resumes_its_session_with_the_same_trips_as_the_first(
) {
    drive(|| async { waits_and_resumes(None).await });
}

#[test]
fn a_send_named_at_a_thread_while_its_first_turn_is_still_reaching_the_machine_waits_for_that_turn_and_resumes_its_session_with_its_access_named_which_skips_a_trip_the_first_turn_still_makes(
) {
    drive(|| async { waits_and_resumes(Some("bypassPermissions")).await });
}

async fn waits_and_resumes(access: Option<&str>) {
    let backend = StubBackend::new(None);
    let (t, probed) = slow(&backend, Kind::Turn);
    let fx = runtime(&backend).adapter("claude", t.factory()).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    let (rt, w) = (fx.rt.clone(), ws.clone());
    let first = tokio::task::spawn_local(async move { rt.sessions().start(&w, opts("A first"), None).await });
    let (rt, w) = (fx.rt.clone(), ws.clone());
    wait!([rt, w] rows(&rt, &w).await.len() == 1);
    let th = rows(&fx.rt, &ws).await[0]["threadId"].as_str().expect("a thread").to_string();
    let (rt, w, t2, access) = (fx.rt.clone(), ws.clone(), th.clone(), access.map(str::to_string));
    let second = tokio::task::spawn_local(async move {
        let mut o = on("B second", &t2);
        o.permission_mode = access;
        rt.sessions().start(&w, o, None).await
    });
    sleep_ms(50).await;
    assert!(t.starts.borrow().is_empty());

    probed.open();
    let a = first.await.expect("the first").expect("a start");
    assert_eq!(j(&a.finished().await)["status"], "completed");
    let b = second.await.expect("the second").expect("a start");
    assert_eq!(format!("{:?}", b.outcome()).to_lowercase(), "queued");
    assert_eq!(j(&b.finished().await)["status"], "completed");
    // The second send joins the session the first turn minted, which it could not have read when it arrived.
    let starts: Vec<(String, Option<String>)> = t.starts.borrow().iter().map(|s| (s.prompt.clone(), s.resume.clone())).collect();
    assert_eq!(starts, [("A first".to_string(), None), ("B second".to_string(), view(&a)["claudeSessionId"].as_str().map(str::to_string))]);
    assert_matches(
        &json!(folded(&rows(&fx.rt, &ws).await)),
        json!([{ "id": th, "status": "completed", "turns": 1, "claudeSessionId": view(&a)["claudeSessionId"] }]),
    );
    fx.rt.close().await.expect("a close");
}

// "..." > "a send named at a thread while its first turn is still reaching the machine tells whoever that thread tells: the notify the opener registered is on the thread from its row, not from its launch"
#[test]
fn a_send_named_at_a_thread_while_its_first_turn_is_still_reaching_the_machine_tells_whoever_that_thread_tells_the_notify_the_opener_registered_is_on_the_thread_from_its_row_not_from_its_launch(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let (t, probed) = slow(&backend, Kind::Turn);
        let fx = runtime(&backend).adapter("claude", t.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let (rt, w) = (fx.rt.clone(), ws.clone());
        let first = tokio::task::spawn_local(async move {
            rt.sessions().start(&w, StartOptions { notify: Some(vec!["me".into()]), ..opts("a") }, None).await
        });
        let (rt, w) = (fx.rt.clone(), ws.clone());
        wait!([rt, w] rows(&rt, &w).await.len() == 1);
        let th = rows(&fx.rt, &ws).await[0]["threadId"].as_str().expect("a thread").to_string();
        let (rt, w, t2) = (fx.rt.clone(), ws.clone(), th.clone());
        let second = tokio::task::spawn_local(async move { rt.sessions().start(&w, on("b", &t2), None).await });
        sleep_ms(50).await;
        probed.open();
        let a = first.await.expect("the first").expect("a start");
        let b = second.await.expect("the second").expect("a start");
        a.finished().await;
        b.finished().await;
        // Both turns tell the person, since a send into the thread inherits what its opener registered whether it arrived
        // inside the launch window or after it.
        let told: Vec<Value> = history(&fx.rt, &ws).await.into_iter().filter(|e| e["type"] == "session.notify").collect();
        let line = |text: &str| {
            notify_line(
                &th,
                &serde_json::from_value(json!({ "status": "completed", "text": text })).expect("a result"),
                Some(NotifyLength::Tail),
            )
        };
        assert_eq!(
            told.iter().map(|e| json!([e["turnId"] == json!(a.turn_id()), e["text"]])).collect::<Vec<_>>(),
            [json!([true, line("a")]), json!([false, line("b")])]
        );
        fx.rt.close().await.expect("a close");
    });
}

// "..." > "a row for a turn still reaching the machine never reaches the session file: another turn's write leaves it out, and a host reading that file back finds no turn to cut"
#[test]
fn a_row_for_a_turn_still_reaching_the_machine_never_reaches_the_session_file_another_turns_write_leaves_it_out_and_a_host_reading_that_file_back_finds_no_turn_to_cut(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        blank_exec(&backend);
        let state = sqlite_state();
        let probed = Gate::new();
        let g = probed.clone();
        let slow = zoo_with(Kind::Turn, move |z| z.probe = Some(g));
        let fx = runtime(&backend).store(&state.store).adapter("claude", slow.factory()).adapter("quick", turns().factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let (rt, w) = (fx.rt.clone(), ws.clone());
        let starting = tokio::task::spawn_local(async move { rt.sessions().start(&w, opts("build it"), None).await });
        let (rt, w) = (fx.rt.clone(), ws.clone());
        wait!([rt, w] rows(&rt, &w).await.len() == 1);
        // Another thread on the workspace runs to its end while the first is still reaching the machine, and its turn
        // writes the workspace's session file.
        fx.rt.sessions().start(&ws, named("quick one", "quick"), None).await.expect("a start").finished().await;
        let stored = |store: Rc<dyn wsp_store::store::Store>, ws: String| async move {
            store
                .get("sessions", &ws)
                .await
                .expect("a read")
                .map(|d| d["sessions"].as_array().cloned().unwrap_or_default().iter().map(|r| r["prompt"].clone()).collect::<Vec<_>>())
                .unwrap_or_default()
        };
        let (store, w) = (state.store.clone(), ws.clone());
        let check = stored;
        wait!([store, w, check] check(store.clone(), w.clone()).await.contains(&json!("quick one")));
        assert_eq!(stored(state.store.clone(), ws.clone()).await, [json!("quick one")]);
        // What a host reading that file back sees: no running row with nothing behind it, so no turn to settle as cut.
        let again = runtime(&backend).store(&state.store).adapter("claude", turns().factory()).adapter("quick", turns().factory()).build();
        assert_eq!(
            rows(&again.rt, &ws).await.iter().map(|r| json!([r["prompt"], r["status"]])).collect::<Vec<_>>(),
            [json!(["quick one", "completed"])]
        );
        assert_eq!(types_of(&history(&again.rt, &ws).await).iter().filter(|t| *t == "session.end").count(), 1);
        again.rt.close().await.expect("a close");

        probed.open();
        let handle = starting.await.expect("the start").expect("a start");
        assert_eq!(j(&handle.finished().await)["status"], "completed");
        fx.rt.close().await.expect("a close");
    });
}

// "..." > "a start whose harness will not open sends its turn's end with the reason and leaves no row: nobody waits on a turn that is not coming"
#[test]
fn a_start_whose_harness_will_not_open_sends_its_turns_end_with_the_reason_and_leaves_no_row_nobody_waits_on_a_turn_that_is_not_coming() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", zoo(Kind::Refusing).factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let ends = fx.events();
        let refused = fx.rt.sessions().start(&ws, opts("build it"), None).await.err().expect("a refusal");
        assert!(refused.message.contains("the harness would not launch"), "{}", refused.message);
        assert_matches(
            &json!(ends.of("session.end")),
            json!([{ "reason": "the harness would not launch", "exitCode": null, "sawResult": false }]),
        );
        // No turn ran, so the thread is on no listing and the transcript holds nothing of it either; the end is a wake
        // for whoever the row had already told this thread was working, and the reason rides that end.
        assert!(rows(&fx.rt, &ws).await.is_empty());
        assert!(history(&fx.rt, &ws).await.is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "..." > "a send that never launches leaves the thread's last turn exactly as it was: the row, the read and the reply all still say what that turn came to"
#[test]
fn a_send_that_never_launches_leaves_the_threads_last_turn_exactly_as_it_was_the_row_the_read_and_the_reply_all_still_say_what_that_turn_came_to(
) {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", zoo(Kind::OneThenNothing).factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let first = fx.rt.sessions().start(&ws, opts("build it"), None).await.expect("a start");
        first.finished().await;
        let th = thread(&first);
        let all = events_of(&history(&fx.rt, &ws).await);
        assert_eq!(thread_result(&all, &th).map(|r| j(&r)), Some(json!({ "status": "completed", "text": "all green" })));

        let refused = fx.rt.sessions().start(&ws, on("and then this", &th), None).await.err().expect("a refusal");
        assert!(refused.message.contains("the harness would not launch"), "{}", refused.message);
        let hist = history(&fx.rt, &ws).await;
        let after = events_of(&hist);
        // One fact, how the thread's last turn went, read the four ways every client reads it; the send that never
        // opened is on none of them, since it never became a turn.
        assert_eq!(types_of(&hist), ["session.start", "session.delta", "session.done", "session.end"]);
        assert_eq!(thread_result(&after, &th).map(|r| j(&r)), Some(json!({ "status": "completed", "text": "all green" })));
        assert_eq!(thread_reply_rows(&after, &th).iter().map(|m| j(m)["text"].clone()).collect::<Vec<_>>(), [json!("all green")]);
        assert_eq!(
            thread_messages(&after, &th).iter().map(|m| j(m)["text"].clone()).collect::<Vec<_>>(),
            [json!("build it"), json!("all green"), json!("completed")]
        );
        assert_matches(&json!(folded(&rows(&fx.rt, &ws).await)), json!([{ "id": th, "status": "completed", "turns": 1 }]));
        fx.rt.close().await.expect("a close");
    });
}

// "..." > "a message the running turn steers takes no row of its own: the thread keeps one turn, and its facts stay the running turn's"
#[test]
fn a_message_the_running_turn_steers_takes_no_row_of_its_own_the_thread_keeps_one_turn_and_its_facts_stay_the_running_turns() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", zoo(Kind::Steering).factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let running = fx.rt.sessions().start(&ws, opts("build it"), None).await.expect("a start");
        let th = thread(&running);
        let (rt, w) = (fx.rt.clone(), ws.clone());
        wait!([rt, w] !rows(&rt, &w).await.first().is_some_and(|r| r["claudeSessionId"].is_null()));
        let joined = fx.rt.sessions().start(&ws, on("and this too", &th), None).await.expect("a start");
        assert_eq!(format!("{:?}", joined.outcome()).to_lowercase(), "steered");
        assert_matches(
            &json!(folded(&rows(&fx.rt, &ws).await)),
            json!([{ "id": th, "status": "running", "sessionId": running.id(), "cwd": "/root/app", "turns": 1 }]),
        );
        fx.rt.close().await.expect("a close");
    });
}

// "..." > "a send that queues behind a running turn writes no second row: the thread's state, folder and stop stay the running turn's"
#[test]
fn a_send_that_queues_behind_a_running_turn_writes_no_second_row_the_threads_state_folder_and_stop_stay_the_running_turns() {
    drive(|| async {
        let holding = zoo(Kind::Holding);
        let fx = runtime(&StubBackend::new(None)).adapter("claude", holding.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let running = fx.rt.sessions().start(&ws, opts("build it"), None).await.expect("a start");
        let th = thread(&running);
        let (rt, w) = (fx.rt.clone(), ws.clone());
        wait!([rt, w] !rows(&rt, &w).await.first().is_some_and(|r| r["claudeSessionId"].is_null()));
        let (rt, w, t) = (fx.rt.clone(), ws.clone(), th.clone());
        let queued = tokio::task::spawn_local(async move { rt.sessions().start(&w, on("and then this", &t), None).await });
        sleep_ms(50).await;
        // The thread reads as its running turn does, and the row a client would stop is that turn's, not the queued one's.
        let f = folded(&rows(&fx.rt, &ws).await);
        assert_matches(&json!(f), json!([{ "id": th, "status": "running", "sessionId": running.id(), "cwd": "/root/app", "turns": 1 }]));
        assert_eq!(
            j(&fx.rt.sessions().interrupt(f[0]["sessionId"].as_str().expect("a session"), None, None).await.expect("an interrupt")),
            json!({ "outcome": "accepted" })
        );

        let second = queued.await.expect("the send").expect("a start");
        assert_eq!(format!("{:?}", second.outcome()).to_lowercase(), "queued");
        assert_eq!(thread(&second), th);
        let end = holding.ends.borrow()[1].clone();
        end(result_of(json!({ "status": "completed", "text": "second done" })));
        second.finished().await;
        // The queued turn resumed the first one's harness session, so it took over its row, as every resume does.
        assert_matches(&json!(folded(&rows(&fx.rt, &ws).await)), json!([{ "id": th, "status": "completed", "turns": 1 }]));
        fx.rt.close().await.expect("a close");
    });
}

/// `await store.deleteBlob("transcripts", ws)`: every event row of the workspace's transcript goes, its index stays.
fn drop_transcript(store: &Rc<dyn wsp_store::store::Store>, ws: &str) {
    let rows = store.transcripts().expect("transcript rows");
    let positions: Vec<i64> = rows.all(ws).expect("a read").events.iter().filter_map(|e| j(e)["pos"].as_i64()).collect();
    rows.remove(ws, &positions).expect("a removal");
}

// "..." > "a resume after a restart joins the persisted thread and hands the harness the session id and folder"
#[test]
fn a_resume_after_a_restart_joins_the_persisted_thread_and_hands_the_harness_the_session_id_and_folder() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", turns().factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        rt1.rt
            .sessions()
            .start(&ws, StartOptions { cwd: Some("/root/app".into()), ..opts("first") }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        let first = rows(&rt1.rt, &ws).await.remove(0);
        rt1.rt.close().await.expect("a close");
        // the transcript is gone but the index survives: the thread still folds
        drop_transcript(&state.store, &ws);

        let t = turns();
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", t.factory()).build();
        rt2.rt
            .sessions()
            .start(
                &ws,
                StartOptions {
                    thread: first["threadId"].as_str().map(str::to_string),
                    cwd: first["cwd"].as_str().map(str::to_string),
                    ..opts("more")
                },
                None,
            )
            .await
            .expect("a start")
            .finished()
            .await;
        {
            let starts = t.starts.borrow();
            assert_eq!(
                (starts[0].resume.clone(), starts[0].cwd.clone()),
                (first["claudeSessionId"].as_str().map(str::to_string), Some("/root/app".to_string()))
            );
        }
        // the resumed turn keeps the session id, so it takes over that row and its opening prompt, in memory and in the store
        let listed = rows(&rt2.rt, &ws).await;
        assert_eq!(listed.iter().map(|s| s["prompt"].clone()).collect::<Vec<_>>(), ["first"]);
        assert_eq!(listed[0]["threadId"], first["threadId"]);
        rt2.rt.close().await.expect("a close");
        let stored = state.store.get("sessions", &ws).await.expect("a read").expect("a document");
        assert_eq!(stored["sessions"].as_array().expect("rows").iter().map(|s| s["prompt"].clone()).collect::<Vec<_>>(), ["first"]);
        assert_eq!(stored["sessions"][0]["threadId"], first["threadId"]);
    });
}

// "..." > "rows of a workspace whose machine is gone are still listed, and a deleted workspace takes its rows with it"
#[test]
fn rows_of_a_workspace_whose_machine_is_gone_are_still_listed_and_a_deleted_workspace_takes_its_rows_with_it() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", turns().factory()).build();
        let a = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let b = create_on(&rt1.rt, CreateOn::golden("b"), None).await.expect("a workspace").view.id;
        rt1.rt.sessions().start(&a, opts("on a"), None).await.expect("a start").finished().await;
        rt1.rt.sessions().start(&b, opts("on b"), None).await.expect("a start").finished().await;
        rt1.rt.close().await.expect("a close");
        backend.machine(0).killed.set(true);

        let rt2 = runtime(&backend).store(&state.store).build();
        let (rt, id) = (rt2.rt.clone(), a.clone());
        wait!([rt, id] j(&rt.workspaces().get(&id, None, None).await.expect("a workspace"))["phase"] == "gone");
        assert_eq!(rows(&rt2.rt, &a).await.iter().map(|s| s["prompt"].clone()).collect::<Vec<_>>(), ["on a"]);
        assert_eq!(status_rows(&rt2.rt).await.iter().find(|w| w["id"] == json!(a)).map(|w| w["machineState"].clone()), Some(json!("gone")));
        rt2.rt.workspaces().delete(&b, None).await.expect("a delete");
        assert_eq!(rt2.rt.sessions().list(None, None).await.expect("a list").len(), 1);
        assert_eq!(state.store.get("sessions", &b).await.expect("a read"), None);
        rt2.rt.close().await.expect("a close");
    });
}

// "..." > "a resume runs in the folder its session started in, whatever folder the request names or none"
#[test]
fn a_resume_runs_in_the_folder_its_session_started_in_whatever_folder_the_request_names_or_none() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let t = turns();
        let rt = runtime(&backend).store(&state.store).adapter("claude", t.factory()).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        rt.rt
            .sessions()
            .start(&ws, StartOptions { cwd: Some("/root/app".into()), ..opts("first") }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        let first = rows(&rt.rt, &ws).await.remove(0);
        let th = first["threadId"].as_str().expect("a thread").to_string();
        rt.rt
            .sessions()
            .start(&ws, StartOptions { cwd: Some("/root/other".into()), ..on("from another folder", &th) }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        rt.rt.sessions().start(&ws, on("from the command line", &th), None).await.expect("a start").finished().await;
        assert_eq!(t.starts.borrow().iter().map(|s| s.cwd.clone()).collect::<Vec<_>>(), vec![Some("/root/app".to_string()); 3]);
        assert_eq!(rows(&rt.rt, &ws).await.iter().map(|s| s["cwd"].clone()).collect::<Vec<_>>(), ["/root/app"]);
        // a start without a thread still runs where it was asked to
        rt.rt
            .sessions()
            .start(&ws, StartOptions { cwd: Some("/root/other".into()), ..opts("new thread") }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        assert_eq!(t.starts.borrow()[3].cwd.as_deref(), Some("/root/other"));
        rt.rt.close().await.expect("a close");

        // the index is gone but the transcript keeps the start: the folder still comes from it
        state.store.delete("sessions", &ws).await.expect("a delete");
        let t2 = turns();
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", t2.factory()).build();
        rt2.rt
            .sessions()
            .start(&ws, StartOptions { cwd: Some("/root/other".into()), ..on("after a restart", &th) }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        assert_eq!(t2.starts.borrow()[0].cwd.as_deref(), Some("/root/app"));
        rt2.rt.close().await.expect("a close");
    });
}

// "..." > "a workspace deleted while a turn runs gets no document written back when the harness settles"
#[test]
fn a_workspace_deleted_while_a_turn_runs_gets_no_document_written_back_when_the_harness_settles() {
    drive(|| async {
        let state = sqlite_state();
        let pending = zoo(Kind::Pending);
        let fx = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", pending.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        fx.rt.sessions().start(&ws, opts("slow"), None).await.expect("a start");
        fx.rt.workspaces().delete(&ws, None).await.expect("a delete");
        assert_eq!(state.store.get("sessions", &ws).await.expect("a read"), None);
        // the real adapter settles when its exec stream dies, which can be after kill() returned
        let settle = pending.ends.borrow()[0].clone();
        settle(result_of(json!({ "status": "failed", "text": "" })));
        tokio::task::yield_now().await;
        fx.rt.close().await.expect("a close");
        assert!(state.store.list("sessions").await.expect("a list").is_empty());
    });
}

// "..." > "a sessions document without a rows array is logged and read as empty; the runtime still boots"
#[test]
fn a_sessions_document_without_a_rows_array_is_logged_and_read_as_empty_the_runtime_still_boots() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let rt1 = runtime(&backend).store(&state.store).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        rt1.rt.close().await.expect("a close");
        state.store.put("sessions", &ws, json!({ "workspaceId": ws })).await.expect("a put");
        let warned = capture();
        let rt2 = runtime(&backend).store(&state.store).build();
        assert_eq!(
            rt2.rt.workspaces().list(None).await.expect("a list").iter().map(|w| j(w)["id"].clone()).collect::<Vec<_>>(),
            [json!(ws)]
        );
        assert!(rows(&rt2.rt, &ws).await.is_empty());
        assert!(warned.lines().iter().any(|l| l.contains(&ws)), "{:?}", warned.lines());
        rt2.rt.close().await.expect("a close");
    });
}

// "..." > "lists every thread of a folder past the row cap: a thread's newest row never falls off for another thread's"
#[test]
fn lists_every_thread_of_a_folder_past_the_row_cap_a_threads_newest_row_never_falls_off_for_another_threads() {
    drive(|| async {
        let state = sqlite_state();
        let fx = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", turns().factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        for i in 1..=202 {
            fx.rt.sessions().start(&ws, opts(&format!("cap thread {i:03}")), None).await.expect("a start").finished().await;
        }
        let listed: Vec<Value> = rows(&fx.rt, &ws).await.iter().map(|s| s["prompt"].clone()).collect();
        assert!(listed.contains(&json!("cap thread 001")));
        assert!(listed.contains(&json!("cap thread 002")));
        assert_eq!(listed.iter().map(Value::to_string).collect::<std::collections::HashSet<_>>().len(), 202);
        let found = fx.rt.sessions().search("cap thread 001", None).await.expect("a search");
        assert!(!j(&found)["hits"].as_array().expect("hits").is_empty());
        fx.rt.close().await.expect("a close");
        let again = runtime(&StubBackend::new(None)).store(&state.store).build();
        assert!(rows(&again.rt, &ws).await.iter().any(|s| s["prompt"] == "cap thread 001"));
        again.rt.close().await.expect("a close");
    });
}

// "..." > "lists a thread whose rows an earlier host's cap took off its record, named by its first words"
#[test]
fn lists_a_thread_whose_rows_an_earlier_hosts_cap_took_off_its_record_named_by_its_first_words() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).adapter("claude", turns().factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let gone = fx.rt.sessions().start(&ws, opts("cap thread 001"), None).await.expect("a start");
        gone.finished().await;
        fx.rt.sessions().start(&ws, opts("cap thread 002"), None).await.expect("a start").finished().await;
        fx.rt.close().await.expect("a close");
        let mut doc = state.store.get("sessions", &ws).await.expect("a read").expect("a document");
        let gone_thread = thread(&gone);
        let kept: Vec<Value> =
            doc["sessions"].as_array().expect("rows").iter().filter(|s| s["threadId"] != json!(gone_thread)).cloned().collect();
        doc["sessions"] = json!(kept);
        state.store.put("sessions", &ws, doc).await.expect("a put");
        let again = runtime(&backend).store(&state.store).build();
        let listed = rows(&again.rt, &ws).await;
        let mut prompts: Vec<String> = listed.iter().map(|s| s["prompt"].as_str().unwrap_or_default().to_string()).collect();
        prompts.sort();
        assert_eq!(prompts, ["cap thread 001", "cap thread 002"]);
        let row = listed.iter().find(|s| s["threadId"] == json!(gone_thread)).expect("the thread's row");
        assert_matches(row, json!({ "status": "completed" }));
        assert!(row["startedAt"].is_number());
        again.rt.close().await.expect("a close");
    });
}

// "..." > "keeps at most 200 rows per workspace: the oldest finished row falls off first, a running row never does"
#[test]
fn keeps_at_most_200_rows_per_workspace_the_oldest_finished_row_falls_off_first_a_running_row_never_does() {
    drive(|| async {
        let state = sqlite_state();
        // A session of its own per turn, so one thread holds a row per turn.
        let fresh = zoo_with(Kind::Turn, |z| z.fresh = true);
        let fx = runtime(&StubBackend::new(None))
            .store(&state.store)
            .adapter("claude", fresh.factory())
            .adapter("hung", zoo(Kind::Hung).factory())
            .build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        fx.rt.sessions().start(&ws, named("stuck", "hung"), None).await.expect("a start");
        // Named: a start with no agent would take the hung one the first start left on the project.
        let first = fx.rt.sessions().start(&ws, named("t0", "claude"), None).await.expect("a start");
        first.finished().await;
        let th = thread(&first);
        for i in 1..199 {
            fx.rt.sessions().start(&ws, on(&format!("t{i}"), &th), None).await.expect("a start").finished().await;
        }
        let full = rows(&fx.rt, &ws).await;
        assert_eq!(full.len(), 200);
        assert_eq!(full.iter().take(2).map(|s| s["prompt"].clone()).collect::<Vec<_>>(), ["stuck", "t0"]);
        fx.rt.sessions().start(&ws, on("t199", &th), None).await.expect("a start").finished().await;
        let listed = rows(&fx.rt, &ws).await;
        let want: Vec<Value> = std::iter::once(json!("stuck")).chain((1..200).map(|i| json!(format!("t{i}")))).collect();
        assert_eq!(listed.iter().map(|s| s["prompt"].clone()).collect::<Vec<_>>(), want);
        fx.rt.close().await.expect("a close");
        let stored = state.store.get("sessions", &ws).await.expect("a read").expect("a document");
        assert_eq!(stored["sessions"].as_array().expect("rows").iter().map(|s| s["prompt"].clone()).collect::<Vec<_>>(), want);
    });
}
