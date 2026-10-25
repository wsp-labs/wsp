// SPDX-License-Identifier: AGPL-3.0-only
//! spec/answered_start.md. TS cases: thread-cap.test.ts:111 (a start that asks to hear of its hold is answered at once,
//! one that does not is answered once it starts), session-held.test.ts:11, serve.test.ts:634 (the answer of a start).
//! No TS test calls `answeredStart` or `startAt` alone, so the other rules are pinned by the TS source only.
#[path = "support_a/mod.rs"]
mod support;

use std::cell::RefCell;
use std::rc::Rc;

use futures::future::Shared;
use support::*;
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::wiring::{AccessMove, SteerTaken};
use wsp_threads::threads::answered_start::*;
use wsp_wire::wire::runtime::{SessionAnswerResult, SessionStartOutcome, SessionStartResult};

type Log = Rc<RefCell<Vec<String>>>;
type Answers = Rc<RefCell<Vec<Value>>>;
type Started = LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>>;

/// A started turn whose view, outcome and notice the test sets; every read of its view is logged.
struct FakeHandle {
    view: SessionView,
    turn_id: String,
    outcome: SessionStartOutcome,
    notice: Option<String>,
    log: Log,
}

impl SessionHandle for FakeHandle {
    fn id(&self) -> String {
        self.view.id.clone()
    }
    fn workspace_id(&self) -> String {
        self.view.workspace_id.clone()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        futures::future::pending::<TurnResult>().boxed_local().shared()
    }
    fn turn_id(&self) -> String {
        self.turn_id.clone()
    }
    fn outcome(&self) -> SessionStartOutcome {
        self.log.borrow_mut().push("outcome".into());
        self.outcome
    }
    fn notice(&self) -> Option<String> {
        self.notice.clone()
    }
    fn view(&self) -> SessionView {
        self.log.borrow_mut().push("view".into());
        self.view.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(&self, _prompt: String, _id: Option<String>, _images: Vec<TurnImage>) -> Option<Local<'static, SteerTaken>> {
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        None
    }
    fn answer(&self, _ask_id: String, _option_id: String, _reason: Option<String>) -> Option<Local<'static, SessionAnswerResult>> {
        None
    }
    fn set_access(&self, _mode: String) -> Option<Local<'static, AccessMove>> {
        None
    }
    fn stop_task(&self, _task: String) -> Option<Local<'static, TaskStop>> {
        None
    }
}

fn handle(log: &Log, outcome: SessionStartOutcome, notice: Option<&str>) -> Rc<dyn SessionHandle> {
    let view = view("sess-1", "w1", json!({ "threadId": "thr_1", "prompt": "go", "startedAt": T0 }));
    Rc::new(FakeHandle { view, turn_id: "turn-1".into(), outcome, notice: notice.map(str::to_string), log: log.clone() })
}

/// The row the handle above answers, as the wire carries it.
fn handle_row() -> Value {
    json!({ "id": "sess-1", "workspaceId": "w1", "harness": "claude", "status": "running", "threadId": "thr_1", "prompt": "go", "startedAt": T0 })
}

fn collector() -> (Answers, Rc<dyn Fn(SessionStartResult)>) {
    let answers: Answers = Rc::new(RefCell::new(Vec::new()));
    let into = answers.clone();
    (answers, Rc::new(move |r: SessionStartResult| into.borrow_mut().push(serde_json::to_value(r).expect("an answer serializes"))))
}

/// What a start the computer's threads hold back says to `on_held`, as thread-cap.test.ts:123 reads it.
fn held_start() -> HeldStart {
    let capped = json!({ "placeId": "here", "place": "this computer", "running": 1, "atOnce": 1 });
    HeldStart {
        view: view("turn-2", "w1", json!({ "threadId": "thr_2", "capped": capped })),
        turn_id: "turn-2".into(),
        wait: serde_json::from_value(capped).expect("a wait"),
    }
}

fn held_answer() -> Value {
    json!({
        "session": { "id": "turn-2", "workspaceId": "w1", "harness": "claude", "status": "running", "threadId": "thr_2", "capped": { "placeId": "here", "place": "this computer", "running": 1, "atOnce": 1 } },
        "outcome": "held",
        "turnId": "turn-2",
    })
}

fn refused(message: &str, fix: &str, kind: Kind) -> HostError {
    {
        let mut e = HostError::with_kind(message, kind);
        e.fix = Some(fix.into());
        e
    }
}

// A1, A3: thread-cap.test.ts:134 (a start that asks to hear of its hold and is never held is answered once its turn starts)
#[test]
fn answers_once_with_the_handle_when_the_turn_starts() {
    run_local(async {
        for (outcome, word) in [(SessionStartOutcome::Started, "started"), (SessionStartOutcome::Steered, "steered"), (SessionStartOutcome::Queued, "queued")] {
            let (answers, answer) = collector();
            let log: Log = Rc::default();
            let got: Rc<RefCell<Vec<bool>>> = Rc::default();
            let (seen, h) = (got.clone(), handle(&log, outcome, None));
            let done = answered_start(true, move |on_held: Option<OnHeld>| {
                seen.borrow_mut().push(on_held.is_some());
                async move { Ok(h) }.boxed_local() as Started
            }, answer)
            .await;
            assert!(done.is_ok(), "{word}");
            assert_eq!(*got.borrow(), vec![true], "{word}: start is called once, with a way to answer a hold");
            assert_eq!(*answers.borrow(), vec![json!({ "session": handle_row(), "outcome": word, "turnId": "turn-1" })], "{word}");
        }
    });
}

// A1: serve.test.ts:634 (a start that does not ask is answered once its turn starts)
#[test]
fn hands_no_on_held_when_the_caller_did_not_ask() {
    run_local(async {
        let (answers, answer) = collector();
        let log: Log = Rc::default();
        let got: Rc<RefCell<Vec<bool>>> = Rc::default();
        let (seen, h) = (got.clone(), handle(&log, SessionStartOutcome::Started, None));
        answered_start(false, move |on_held: Option<OnHeld>| {
            seen.borrow_mut().push(on_held.is_some());
            async move { Ok(h) }.boxed_local() as Started
        }, answer)
        .await
        .expect("the start answers");
        assert_eq!(*got.borrow(), vec![false]);
        assert_eq!(*answers.borrow(), vec![json!({ "session": handle_row(), "outcome": "started", "turnId": "turn-1" })]);
    });
}

// A3: answered-start.ts:20 (the notice key is written only where the handle carries one)
#[test]
fn carries_the_handles_notice_only_where_it_has_one() {
    run_local(async {
        let log: Log = Rc::default();
        let (answers, answer) = collector();
        let h = handle(&log, SessionStartOutcome::Queued, Some("it will start when the first turn ends"));
        answered_start(false, move |_o: Option<OnHeld>| async move { Ok(h) }.boxed_local() as Started, answer).await.expect("the start answers");
        assert_eq!(*answers.borrow(), vec![json!({ "session": handle_row(), "outcome": "queued", "turnId": "turn-1", "notice": "it will start when the first turn ends" })]);
        let (answers, answer) = collector();
        let h = handle(&log, SessionStartOutcome::Queued, Some(""));
        answered_start(false, move |_o: Option<OnHeld>| async move { Ok(h) }.boxed_local() as Started, answer).await.expect("the start answers");
        assert_eq!(answers.borrow()[0]["notice"], "", "an empty notice is still a notice");
        let (answers, answer) = collector();
        let h = handle(&log, SessionStartOutcome::Queued, None);
        answered_start(false, move |_o: Option<OnHeld>| async move { Ok(h) }.boxed_local() as Started, answer).await.expect("the start answers");
        assert_eq!(answers.borrow()[0].as_object().expect("an object").keys().collect::<Vec<_>>().len(), 3, "session, outcome and turnId, no notice key");
        assert_eq!(answers.borrow()[0].get("notice"), None);
    });
}

// A3: answered-start.ts:20 (the view is read once the start resolved, not before and not twice)
#[test]
fn reads_the_view_after_the_start_resolved() {
    run_local(async {
        let log: Log = Rc::default();
        let (answers, answer) = collector();
        let into = log.clone();
        let answers_at = answers.clone();
        let log_for_answer = log.clone();
        let answer_logged: Rc<dyn Fn(SessionStartResult)> = Rc::new(move |r: SessionStartResult| {
            log_for_answer.borrow_mut().push("answer".into());
            answer(r)
        });
        let h = handle(&log, SessionStartOutcome::Started, None);
        answered_start(false, move |_o: Option<OnHeld>| {
            async move {
                tokio::task::yield_now().await;
                into.borrow_mut().push(format!("resolved after {} answers", answers_at.borrow().len()));
                Ok(h)
            }
            .boxed_local() as Started
        }, answer_logged)
        .await
        .expect("the start answers");
        let log = log.borrow().clone();
        assert_eq!(log.iter().filter(|l| *l == "view").count(), 1, "the view is read once: {log:?}");
        assert_eq!(log[0], "resolved after 0 answers");
        assert_eq!(log.last().map(String::as_str), Some("answer"));
        assert!(log.iter().position(|l| l == "view") < log.iter().position(|l| l == "answer"));
        assert_eq!(answers.borrow().len(), 1);
    });
}

// A2, A4: thread-cap.test.ts:123, session-held.test.ts:11 (answered the moment it is held, not again at its start)
#[test]
fn answers_held_at_once_and_not_again_when_the_turn_starts() {
    run_local(async {
        let log: Log = Rc::default();
        let (answers, answer) = collector();
        let at_hold: Rc<RefCell<Vec<usize>>> = Rc::default();
        let (probe, seen_answers) = (at_hold.clone(), answers.clone());
        let h = handle(&log, SessionStartOutcome::Started, Some("a notice the held answer does not carry"));
        answered_start(true, move |on_held: Option<OnHeld>| {
            let on_held = on_held.expect("a caller that asked to hear of a hold is handed the way to say it");
            async move {
                on_held(held_start());
                probe.borrow_mut().push(seen_answers.borrow().len());
                tokio::task::yield_now().await;
                probe.borrow_mut().push(seen_answers.borrow().len());
                Ok(h)
            }
            .boxed_local() as Started
        }, answer)
        .await
        .expect("a held start is answered");
        assert_eq!(*at_hold.borrow(), vec![1, 1], "the answer is out before the start goes on");
        assert_eq!(*answers.borrow(), vec![held_answer()]);
        assert_eq!(*log.borrow(), Vec::<String>::new(), "the handle's own view is never read for an answer already given");
    });
}

// A2: answered-start.ts:14 (no guard: a second call answers a second time)
#[test]
fn a_second_held_call_answers_a_second_time() {
    run_local(async {
        let log: Log = Rc::default();
        let (answers, answer) = collector();
        let h = handle(&log, SessionStartOutcome::Started, None);
        answered_start(true, move |on_held: Option<OnHeld>| {
            let on_held = on_held.expect("handed");
            async move {
                on_held(held_start());
                on_held(held_start());
                Ok(h)
            }
            .boxed_local() as Started
        }, answer)
        .await
        .expect("answered");
        assert_eq!(*answers.borrow(), vec![held_answer(), held_answer()]);
    });
}

// A5: answered-start.ts:17-19
#[test]
fn a_failure_before_any_answer_is_the_callers() {
    run_local(async {
        for answer_held in [false, true] {
            let (answers, answer) = collector();
            let failure = refused("no thread thr_nope in this folder", "name a thread of this folder.", Kind::NotFound);
            let sent = failure.clone();
            let got = answered_start(answer_held, move |_o: Option<OnHeld>| async move { Err(sent) }.boxed_local() as Started, answer).await;
            assert_eq!(got.map_err(|e| said(&e)), Err(said(&failure)), "answer_held={answer_held}");
            assert_eq!(*answers.borrow(), Vec::<Value>::new(), "answer_held={answer_held}");
        }
    });
}

// A6: answered-start.ts:17-19 (session-held.test.ts:11: a held start goes on to its launch unheard)
#[test]
fn a_failure_after_a_held_answer_is_swallowed() {
    run_local(async {
        let (answers, answer) = collector();
        let failure = refused("the launch failed", "start it again.", Kind::Usage);
        let got = answered_start(true, move |on_held: Option<OnHeld>| {
            let on_held = on_held.expect("handed");
            async move {
                on_held(held_start());
                Err(failure)
            }
            .boxed_local() as Started
        }, answer)
        .await;
        assert!(got.is_ok(), "the failure is the turn's end, not this function's: {:?}", got.err().map(|e| e.message));
        assert_eq!(*answers.borrow(), vec![held_answer()]);
    });
}

/// A recording stand-in for the workspaces area: every call is logged with its arguments, and the answers are set.
struct Workspaces {
    calls: RefCell<Vec<String>>,
    asks: RefCell<Vec<FolderForAsk>>,
    folder: RefCell<Result<FolderFor, HostError>>,
    setup: RefCell<Result<Option<String>, HostError>>,
}

impl Workspaces {
    fn new() -> Self {
        let unset = || HostError::new("not set by the test");
        Workspaces { calls: RefCell::default(), asks: RefCell::default(), folder: RefCell::new(Err(unset())), setup: RefCell::new(Err(unset())) }
    }
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

#[async_trait(?Send)]
impl StartAtWorkspaces for Workspaces {
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError> {
        self.calls.borrow_mut().push(format!("folder_for({origin:?})"));
        self.asks.borrow_mut().push(ask);
        self.folder.borrow().clone()
    }
    async fn take_setup_line(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
        self.calls.borrow_mut().push(format!("take_setup_line({workspace_id}, {origin:?})"));
        self.setup.borrow().clone()
    }
}

fn found(id: &str, cwd: Option<&str>, setup: Option<&str>) -> FolderFor {
    let workspace = serde_json::from_value(json!({
        "id": id, "name": id, "machineId": format!("m-{id}"), "phase": "running", "golden": "g1", "createdAt": "2026-10-01T00:00:00.000Z",
        "project": { "id": "p1", "name": "spoo", "path": "/root/spoo", "computer": "here" },
    }))
    .expect("a workspace view");
    FolderFor { workspace, cwd: cwd.map(str::to_string), setup: setup.map(str::to_string) }
}

fn by_the_paired_road() -> Option<Caller> {
    Some(Caller::Road(WorkspaceOrigin::Paired))
}

fn project_msg() -> StartAtMsg {
    StartAtMsg { project: Some("spoo".into()), ..Default::default() }
}

// S1: answered-start.ts:32-35
#[test]
fn a_named_workspace_answers_its_own_cwd_and_reads_no_folder() {
    run_local(async {
        let ws = Workspaces::new();
        *ws.setup.borrow_mut() = Ok(Some("npm install".into()));
        let msg = StartAtMsg { workspace_id: Some("w1".into()), cwd: Some("/root/app/src".into()), project: Some("ignored".into()), branch: Some("ignored".into()), ..Default::default() };
        let at = start_at(&ws, msg, by_the_paired_road(), false).await.expect("a named workspace");
        assert_eq!(at, StartedAt { workspace_id: "w1".into(), cwd: Some("/root/app/src".into()), setup: None });
        assert_eq!(ws.calls(), Vec::<String>::new(), "no setup line is taken and no folder is asked");
        let at = start_at(&ws, StartAtMsg { workspace_id: Some("w1".into()), ..Default::default() }, None, false).await.expect("a named workspace");
        assert_eq!(at, StartedAt { workspace_id: "w1".into(), cwd: None, setup: None });
        assert_eq!(ws.calls(), Vec::<String>::new());
    });
}

// S1: answered-start.ts:32
#[test]
fn a_named_workspace_that_opens_takes_its_setup_line() {
    run_local(async {
        let ws = Workspaces::new();
        *ws.setup.borrow_mut() = Ok(Some("npm install".into()));
        let msg = StartAtMsg { workspace_id: Some("w1".into()), cwd: Some("/root/app".into()), ..Default::default() };
        let at = start_at(&ws, msg.clone(), by_the_paired_road(), true).await.expect("a named workspace that opens");
        assert_eq!(at, StartedAt { workspace_id: "w1".into(), cwd: Some("/root/app".into()), setup: Some("npm install".into()) });
        assert_eq!(ws.calls(), vec!["take_setup_line(w1, Some(Road(Paired)))"]);
        *ws.setup.borrow_mut() = Ok(None);
        let at = start_at(&ws, msg, None, true).await.expect("a named workspace that opens");
        assert_eq!(at, StartedAt { workspace_id: "w1".into(), cwd: Some("/root/app".into()), setup: None });
        assert_eq!(ws.calls().last().map(String::as_str), Some("take_setup_line(w1, None)"));
        assert_eq!(ws.calls().len(), 2, "folder_for is never asked of a named workspace");
    });
}

// S2, S3: answered-start.ts:36-52
#[test]
fn a_project_start_asks_the_folder_with_only_the_keys_named() {
    run_local(async {
        let ws = Workspaces::new();
        *ws.folder.borrow_mut() = Ok(found("w9", None, None));
        let msg = StartAtMsg { project: Some("spoo".into()), branch: Some("feat/side".into()), ..Default::default() };
        start_at(&ws, msg, by_the_paired_road(), false).await.expect("a folder");
        let msg = StartAtMsg { cwd: Some("/root/scratch".into()), ..Default::default() };
        start_at(&ws, msg, None, false).await.expect("a folder");
        start_at(&ws, StartAtMsg::default(), None, false).await.expect("a folder");
        assert_eq!(
            *ws.asks.borrow(),
            vec![
                FolderForAsk { project: Some("spoo".into()), branch: Some("feat/side".into()), cwd: None, picks: None, opens: false },
                FolderForAsk { project: None, branch: None, cwd: Some("/root/scratch".into()), picks: None, opens: false },
                FolderForAsk::default(),
            ]
        );
        assert_eq!(ws.calls(), vec!["folder_for(Some(Road(Paired)))", "folder_for(None)", "folder_for(None)"]);
    });
}

// S3: answered-start.ts:37-45 (`msg.fast === true`)
#[test]
fn fast_false_alone_names_no_picks() {
    run_local(async {
        let ws = Workspaces::new();
        *ws.folder.borrow_mut() = Ok(found("w9", None, None));
        start_at(&ws, StartAtMsg { fast: Some(false), ..project_msg() }, None, false).await.expect("a folder");
        start_at(&ws, StartAtMsg { fast: Some(false), harness: Some("codex".into()), ..project_msg() }, None, false).await.expect("a folder");
        start_at(&ws, StartAtMsg { fast: Some(true), ..project_msg() }, None, false).await.expect("a folder");
        let picks = |p: StartPicksAsked| FolderForAsk { project: Some("spoo".into()), picks: Some(p), ..Default::default() };
        assert_eq!(
            *ws.asks.borrow(),
            vec![
                FolderForAsk { project: Some("spoo".into()), ..Default::default() },
                picks(StartPicksAsked { harness: Some("codex".into()), ..Default::default() }),
                picks(StartPicksAsked { fast: Some(true), ..Default::default() }),
            ]
        );
    });
}

// S2, S3: answered-start.ts:37-51
#[test]
fn every_pick_named_rides_the_ask() {
    run_local(async {
        let ws = Workspaces::new();
        *ws.folder.borrow_mut() = Ok(found("w9", None, None));
        let msg = StartAtMsg {
            project: Some("spoo".into()),
            branch: Some("feat/side".into()),
            cwd: Some("/root/spoo/src".into()),
            harness: Some("codex".into()),
            model: Some("gpt-5".into()),
            effort: Some("high".into()),
            access: Some(AccessChoice::AutoEdit),
            permission_mode: Some("acceptEdits".into()),
            fast: Some(true),
            ..Default::default()
        };
        start_at(&ws, msg, None, true).await.expect("a folder that opens");
        let picks = StartPicksAsked {
            harness: Some("codex".into()),
            model: Some("gpt-5".into()),
            effort: Some("high".into()),
            access: Some(AccessChoice::AutoEdit),
            permission_mode: Some("acceptEdits".into()),
            fast: Some(true),
        };
        assert_eq!(*ws.asks.borrow(), vec![FolderForAsk { project: Some("spoo".into()), branch: Some("feat/side".into()), cwd: Some("/root/spoo/src".into()), picks: Some(picks), opens: true }]);
    });
}

// S4: answered-start.ts:53
#[test]
fn answers_the_folder_found_with_its_cwd_and_setup() {
    run_local(async {
        let ws = Workspaces::new();
        *ws.folder.borrow_mut() = Ok(found("w9", Some("/root/spoo-feat"), Some("pnpm install")));
        let at = start_at(&ws, StartAtMsg { cwd: Some("/the/message/cwd".into()), ..project_msg() }, None, true).await.expect("a folder");
        assert_eq!(at, StartedAt { workspace_id: "w9".into(), cwd: Some("/root/spoo-feat".into()), setup: Some("pnpm install".into()) }, "the cwd is the folder's, not the message's");
        *ws.folder.borrow_mut() = Ok(found("w9", None, None));
        let at = start_at(&ws, project_msg(), None, false).await.expect("a folder");
        assert_eq!(at, StartedAt { workspace_id: "w9".into(), cwd: None, setup: None });
        assert_eq!(ws.calls().len(), 2, "no setup line is taken for a folder found: it comes with the folder");
    });
}

// S5: answered-start.ts:36
#[test]
fn a_folder_refusal_is_the_callers() {
    run_local(async {
        let ws = Workspaces::new();
        let refusal = refused("no project named spoo", "add it with wsp projects add.", Kind::Usage);
        *ws.folder.borrow_mut() = Err(refusal.clone());
        let got = start_at(&ws, project_msg(), None, false).await;
        assert_eq!(got.map_err(|e| said(&e)), Err(said(&refusal)));
        let refusal = refused("the setup line could not be read", "try again.", Kind::NotFound);
        *ws.setup.borrow_mut() = Err(refusal.clone());
        let msg = StartAtMsg { workspace_id: Some("w1".into()), ..Default::default() };
        let got = start_at(&ws, msg, None, true).await;
        assert_eq!(got.map_err(|e| said(&e)), Err(said(&refusal)));
    });
}
