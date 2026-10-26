// SPDX-License-Identifier: AGPL-3.0-only
//! spec/turns_run.md. TS cases: session-row.test.ts:145, permission-relay.test.ts:179-420 and :646-700,
//! turn-changes.test.ts:170 and :483-510, thread-keeps-process.test.ts:92. The event switch is turns_forward's:
//! events here only open an ask, name an anchor, report what the agent held or give a reply.
#[path = "support_a/mod.rs"]
mod support;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::channel::oneshot;
use support::*;
use wsp_core::console;
use wsp_core::types::internal::ToldAs;
use wsp_core::types::wiring::{AccessMove, SteerTaken};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::SessionStatus;
use wsp_wire::views::session_events_more::SessionPermissionEvent;
use wsp_wire::wire::runtime::{SessionAnswerOutcome, SessionAnswerResult, SessionStartOutcome};
use wsp_words::words::computer::thread_word;
use wsp_words::words::tools::{asking_line, denied_line, permission_mode_option_label};
use wsp_words::words::turn::turn_lines;

const W: &str = "w1";
const THREAD: &str = "t1";
const TURN: &str = "turn-1";
const SESS: &str = "sess-1";
const MIN: i64 = 60_000;

// ---- fixtures ----------------------------------------------------------------------------------------------------

fn world() -> (Rc<FakeHost>, Entry) {
    let host = FakeHost::new();
    let entry = host.add_entry(entry(W, json!({})));
    (host, entry)
}

/// A running row's JSON as a turn of thread `t1` on `w1` starts it, `extra` laid over.
fn doc(id: &str, extra: Value) -> Value {
    let mut d = json!({
        "id": id, "workspaceId": W, "harness": "claude", "status": "running", "threadId": THREAD, "startedAt": T0,
        "permissionMode": "bypassPermissions",
    });
    for (k, v) in extra.as_object().expect("extra is an object") {
        d[k] = v.clone();
    }
    d
}

fn turn_row(extra: Value) -> Row {
    row(serde_json::from_value(doc(TURN, extra)).expect("a session view"), TURN)
}

fn json_of(turn: &Row) -> Value {
    serde_json::to_value(&turn.borrow().view).expect("a view serializes")
}

/// The turn's live state as its row holds it now.
fn live_of(turn: &Row) -> TurnLive {
    turn.borrow().turn_live.clone().expect("the row's live state")
}

/// The row the start road keeps under the turn's id while the harness is still being reached.
fn launching(host: &FakeHost, turn: &Row) {
    host.add_row(TURN, turn.clone());
}

fn failed(message: &str) -> HostError {
    HostError::new(message)
}

fn turn_start(entry: &Entry, turn: &Row, session: &Arc<FakeSession>, slot: &Rc<RefCell<Option<OnEvent>>>) -> TurnStart {
    let (s, held) = (session.clone(), slot.clone());
    TurnStart {
        entry: entry.clone(),
        row: turn.clone(),
        written: None,
        thread_id: THREAD.into(),
        turn_id: TURN.into(),
        notify: None,
        notify_by: None,
        notify_road: None,
        turn_token: None,
        scope_device_id: None,
        outcome: SessionStartOutcome::Started,
        opening: Opening { prompt: "write it".into(), ..Opening::default() },
        resume: None,
        images_dir: None,
        snapshot: None,
        reports_edits: false,
        waiting: None,
        keep: None,
        open: Box::new(move |f: OnEvent| {
            *held.borrow_mut() = Some(f);
            Ok(s as Arc<dyn HarnessSession>)
        }),
    }
}

struct Rig {
    host: Rc<FakeHost>,
    turn: Row,
    session: Arc<FakeSession>,
    forward: OnEvent,
    handle: Rc<dyn SessionHandle>,
}

impl Rig {
    fn send(&self, doc: Value) {
        (self.forward)(adapter_event(doc))
    }
    fn row(&self) -> Row {
        self.host.rows.borrow().get(SESS).cloned().expect("the turn's row under its session id")
    }
    fn snapshot(&self) -> Option<String> {
        self.row().borrow().snapshot.clone()
    }
    fn token(&self) -> Option<String> {
        self.row().borrow().turn_token.clone()
    }
    fn pid(&self) -> Option<u32> {
        self.row().borrow().pid
    }
    fn calls_open(&self) -> usize {
        self.row().borrow().calls.as_ref().expect("calls").len()
    }
    fn end(&self, reason: &str, by_stop: bool, unreached: bool) {
        let end = self.row().borrow().end.clone().expect("the row's end");
        end(reason, Some(by_stop), Some(unreached))
    }
}

fn rig_on(host: &Rc<FakeHost>, entry: &Entry, session: Arc<FakeSession>, turn: Row, tweak: impl FnOnce(&mut TurnStart)) -> Rig {
    let slot: Rc<RefCell<Option<OnEvent>>> = Rc::new(RefCell::new(None));
    let mut t = turn_start(entry, &turn, &session, &slot);
    tweak(&mut t);
    let answers = session.answers;
    host.hooks.set::<dyn Fn(&dyn HarnessSession) -> bool>("TurnsCtx::session_answers", Rc::new(move |_s: &dyn HarnessSession| answers));
    let handle = TurnsArea::new(host.clone()).run_turn(t).ok().expect("the turn opens");
    let forward = slot.borrow().clone().expect("open handed the forward over");
    Rig { host: host.clone(), turn, session, forward, handle }
}

fn rig() -> Rig {
    let (host, entry) = world();
    let turn = turn_row(json!({}));
    launching(&host, &turn);
    rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {})
}

fn rig_with(session: Arc<FakeSession>, tweak: impl FnOnce(&mut TurnStart)) -> Rig {
    let (host, entry) = world();
    let turn = turn_row(json!({}));
    launching(&host, &turn);
    rig_on(&host, &entry, session, turn, tweak)
}

/// A turn's state with no session behind it, for the methods that need none.
fn state_on(host: &Rc<FakeHost>, entry: &Entry, turn: &Row, tweak: impl FnOnce(&mut TurnStart)) -> Rc<TurnState> {
    let slot: Rc<RefCell<Option<OnEvent>>> = Rc::new(RefCell::new(None));
    let mut t = turn_start(entry, turn, &FakeSession::new(SESS), &slot);
    tweak(&mut t);
    TurnState::new(host.clone(), &t)
}

fn only(host: &FakeHost, names: &[&str]) -> Vec<String> {
    host.hooks.calls().into_iter().filter(|c| names.iter().any(|n| c == n || c.starts_with(&format!("{n}(")))).collect()
}

fn after(host: &FakeHost, mark: usize) -> Vec<String> {
    host.hooks.calls()[mark..].to_vec()
}

fn ask_doc(id: &str) -> Value {
    json!({
        "askId": id, "toolName": "Write", "toolUseId": "toolu_1",
        "input": "{\"file_path\":\"/root/out.txt\",\"content\":\"hi\"}", "detail": "out.txt",
        "options": [
            { "id": "allow", "label": "Allow", "effect": "allow" },
            { "id": "deny", "label": "Deny", "effect": "deny" },
            { "id": "mode:acceptEdits", "label": "acceptEdits", "effect": "mode", "mode": "acceptEdits" },
        ],
    })
}

fn ask_event(id: &str) -> Value {
    json!({ "type": "permission.ask", "sessionId": SESS, "ask": ask_doc(id) })
}

fn question_event(id: &str) -> Value {
    let mut ask = ask_doc(id);
    ask["toolName"] = json!("AskUserQuestion");
    ask["options"] = json!([
        { "id": "q0:o0", "label": "Types", "effect": "answer" },
        { "id": "q0:o1", "label": "Lint", "effect": "answer" },
    ]);
    json!({ "type": "permission.ask", "sessionId": SESS, "ask": ask })
}

fn close_event(id: &str, outcome: &str) -> Value {
    json!({ "type": "permission.close", "sessionId": SESS, "askId": id, "outcome": outcome })
}

fn session_end() -> Value {
    json!({ "type": "session.end", "sessionId": SESS, "exitCode": 0, "sawResult": true })
}

fn done_event(res: Value) -> Value {
    json!({ "type": "turn.done", "sessionId": SESS, "result": res })
}

/// The asking line a row says for one of `ask_doc`'s prompts.
fn asking(ask: &Value) -> String {
    let mut d = ask.clone();
    d["workspaceId"] = json!(W);
    d["sessionId"] = json!(SESS);
    let ev: SessionPermissionEvent = serde_json::from_value(d).expect("a permission event");
    asking_line(&ev)
}

fn closed_row(ask_id: &str, outcome: &str, option: Option<&str>) -> Value {
    let mut v = json!({
        "type": "session.permission.closed", "workspaceId": W, "sessionId": SESS, "turnId": TURN, "threadId": THREAD,
        "askId": ask_id, "outcome": outcome,
    });
    if let Some(o) = option {
        v["optionId"] = json!(o);
    }
    v
}

/// A close row on the agent's own session id, which `closeAsk` writes where the view has one (turns.ts:255).
fn on_agent_session(mut row: Value) -> Value {
    row["sessionId"] = json!("agent-sess");
    row
}

fn finish(r: &Rig, res: Value) {
    r.session.finish(result(res));
}

fn num(n: i64) -> Option<JsNumber> {
    Some(JsNumber(n as f64))
}

fn scope() -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": "t-lead", "workspaceId": "w9", "rootThreadId": "t-lead" }))
        .expect("a scope")
}

fn answered(outcome: SessionAnswerOutcome) -> SessionAnswerResult {
    SessionAnswerResult { outcome }
}

fn kept_launch() -> KeptLaunch {
    let mut picks = IndexMap::new();
    picks.insert("model".to_string(), json!("opus"));
    KeptLaunch { fixed: "claude --print".into(), picks }
}

/// A kept agent that notes how it was closed.
struct FakeKept {
    closes: Arc<Mutex<Vec<bool>>>,
}

impl KeptAgent for FakeKept {
    fn next(&self, _turn: KeptTurn) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("no next turn here".into()))
    }
    fn close(&self, now: bool) -> LocalBoxFuture<'static, ()> {
        self.closes.lock().unwrap().push(now);
        futures::future::ready(()).boxed_local()
    }
    fn exited(&self) -> SharedFuture<()> {
        futures::future::ready(()).boxed_local().shared()
    }
}

fn keeper(closes: &Arc<Mutex<Vec<bool>>>) -> Box<dyn KeptAgent> {
    Box::new(FakeKept { closes: closes.clone() })
}

// ---- the row (N1, N2, N11-N13) -----------------------------------------------------------------------------------

// N1, N2, N13: session-row.test.ts:145
#[test]
fn a_new_turn_holds_one_row_under_the_sessions_id_and_drops_its_turn_key() {
    run_local(async {
        let (host, entry) = world();
        host.add_row("other", row(view("other", W, json!({ "threadId": "t0" })), "turn-0"));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let asked = TurnAsked {
            prompt: "write it".into(),
            effort: Some("high".into()),
            typed: None,
            request_id: None,
            owed: None,
            awaits_prompt: None,
        };
        turn.borrow_mut().asked = Some(asked.clone());
        let session = FakeSession::with(SESS, |s| {
            s.run = Some("run-1".into());
            s.from = Some(40);
        });
        let r = rig_on(&host, &entry, session, turn.clone(), |t| {
            t.notify = Some(vec!["thr_lead".into()]);
            t.notify_by = Some(scope());
            t.notify_road = Some(WorkspaceOrigin::Paired);
            t.turn_token = Some("tok-1".into());
            t.scope_device_id = Some("dev-1".into());
            t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c0".into())), cwd: "/work/w1".into() });
        });
        settle().await;
        assert_eq!(host.rows.borrow().keys().cloned().collect::<Vec<_>>(), vec!["other".to_string(), SESS.to_string()]);
        assert!(Rc::ptr_eq(&r.row(), &turn));
        let held = turn.borrow();
        assert_eq!(held.turn_id, TURN);
        assert_eq!(held.notify, Some(vec!["thr_lead".to_string()]));
        assert_eq!(held.notify_by, Some(scope()));
        assert_eq!(held.notify_road, Some(WorkspaceOrigin::Paired));
        assert_eq!(held.turn_token.as_deref(), Some("tok-1"));
        assert_eq!(held.scope_device_id.as_deref(), Some("dev-1"));
        assert_eq!(held.run.as_deref(), Some("run-1"));
        assert_eq!(held.from, Some(40));
        assert_eq!(held.asked, Some(asked));
        assert_eq!(held.snapshot.as_deref(), Some("c0"));
        assert_eq!(held.pid, None);
        assert_eq!(held.handle.clone().expect("the handle").id(), SESS);
        assert!(held.end.is_some() && held.calls.is_some());
        assert_eq!(held.turn_live, Some(TurnLive::default()));
        assert!(held.launch.is_none() && held.replaces.is_none() && !held.unanswered && !held.queued);
        drop(held);
        assert_eq!(json_of(&turn), doc(SESS, json!({})));
        assert_eq!(
            (r.handle.id(), r.handle.workspace_id(), r.handle.turn_id(), r.handle.outcome()),
            (SESS.to_string(), W.to_string(), TURN.to_string(), SessionStartOutcome::Started)
        );
        assert_eq!(host.hooks.calls(), vec!["idle_hold(w1)", "arm_disk_watch", "port_roots_moved(w1)", "persist_sessions(w1)"]);
    });
}

// N2: turns.ts:128
#[test]
fn the_row_rides_the_live_cell_the_turn_was_handed() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let live = TurnLive { reply: Some(TurnStatus::Completed), told: Some(1), waited: num(5_000), ..TurnLive::default() };
        turn.borrow_mut().turn_live = Some(live.clone());
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        assert_eq!(r.row().borrow().turn_live, Some(live));
        r.send(ask_event("ask_1"));
        host.clock.advance(2_000);
        r.send(close_event("ask_1", "allowed"));
        assert_eq!(live_of(&turn).waited, num(7_000), "the span adds to what the cell already held");
        assert_eq!(live_of(&turn).waiting_since, None);
        assert_eq!(live_of(&turn).reply, Some(TurnStatus::Completed));
        assert_eq!(live_of(&turn).told, Some(1));
    });
}

// N11, N13: turns.ts:597, 625
#[test]
fn a_reopened_row_keeps_the_id_a_client_holds() {
    run_local(async {
        let (host, entry) = world();
        host.add_row("a", row(view("a", W, json!({ "threadId": "t0" })), "turn-0"));
        let old = row(view("client-id", W, json!({ "threadId": THREAD })), "turn-1");
        old.borrow_mut().turn_token = Some("old-token".into());
        host.add_row("client-id", old);
        host.add_row("z", row(view("z", W, json!({ "threadId": "t9" })), "turn-9"));
        let turn = turn_row(json!({ "id": "client-id" }));
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| t.written = Some(TurnWritten::default()));
        settle().await;
        assert_eq!(
            host.rows.borrow().keys().cloned().collect::<Vec<_>>(),
            vec!["a", "client-id", "z"],
            "the row replaced the old one in its place"
        );
        let held = host.rows.borrow()["client-id"].clone();
        assert!(Rc::ptr_eq(&held, &turn));
        assert_eq!(held.borrow().turn_token, None, "the old row is replaced whole");
        assert_eq!(held.borrow().turn_id, TURN);
        assert_eq!(r.handle.id(), "client-id");
        assert_eq!(json_of(&turn)["id"], "client-id");
    });
}

// N11: turns.ts:597
#[test]
fn a_view_with_no_id_or_the_turns_own_takes_the_sessions_id() {
    for id in ["", TURN] {
        run_local(async move {
            let (host, entry) = world();
            let turn = turn_row(json!({ "id": id }));
            if id == TURN {
                launching(&host, &turn);
            }
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
            assert_eq!(host.rows.borrow().keys().cloned().collect::<Vec<_>>(), vec![SESS.to_string()], "view id {id:?}");
            assert_eq!(r.handle.id(), SESS);
            assert_eq!(json_of(&turn)["id"], SESS);
        });
    }
}

// N12: turns.ts:601-608
#[test]
fn a_resumed_turn_takes_over_its_rows_opener_prompt_and_cost() {
    run_local(async {
        let (host, entry) = world();
        host.add_row(
            SESS,
            row(view(SESS, W, json!({ "threadId": THREAD, "startedBy": "person", "prompt": "the first brief", "costUsd": 0.5 })), "turn-0"),
        );
        let turn = turn_row(json!({ "startedBy": "cli", "prompt": "go on" }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| t.resume = Some(SESS.into()));
        assert_eq!(json_of(&turn), doc(SESS, json!({ "startedBy": "person", "prompt": "the first brief", "costUsd": 0.5 })));
        assert_eq!(host.rows.borrow().keys().cloned().collect::<Vec<_>>(), vec![SESS.to_string()]);
        assert!(Rc::ptr_eq(&r.row(), &turn), "the new row took the old one's place");
    });
}

// N12
#[test]
fn a_resumed_row_that_names_nothing_leaves_the_view_alone() {
    run_local(async {
        let (host, entry) = world();
        host.add_row(SESS, row(view(SESS, W, json!({ "threadId": THREAD })), "turn-0"));
        let turn = turn_row(json!({ "startedBy": "cli", "prompt": "go on" }));
        launching(&host, &turn);
        rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| t.resume = Some(SESS.into()));
        assert_eq!(json_of(&turn), doc(SESS, json!({ "startedBy": "cli", "prompt": "go on" })));
    });
}

// N12
#[test]
fn a_turn_that_does_not_resume_takes_over_nothing() {
    run_local(async {
        let (host, entry) = world();
        host.add_row(
            SESS,
            row(view(SESS, W, json!({ "threadId": THREAD, "startedBy": "person", "prompt": "the first brief", "costUsd": 0.5 })), "turn-0"),
        );
        let turn = turn_row(json!({ "startedBy": "cli", "prompt": "go on" }));
        launching(&host, &turn);
        rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        assert_eq!(json_of(&turn), doc(SESS, json!({ "startedBy": "cli", "prompt": "go on" })));
    });
}

// ---- opening and the idle hold (N10) -----------------------------------------------------------------------------

// N10: turns.ts:571-580
#[test]
fn a_launch_that_throws_releases_the_idle_hold_and_fails() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let waiting = Rc::new(Cell::new(true));
        let seen: Rc<Cell<Option<bool>>> = Rc::new(Cell::new(None));
        let slot = Rc::new(RefCell::new(None));
        let mut t = turn_start(&entry, &turn, &FakeSession::new(SESS), &slot);
        t.waiting = Some(waiting.clone());
        let (s, w) = (seen.clone(), waiting.clone());
        t.open = Box::new(move |_f: OnEvent| {
            s.set(Some(w.get()));
            Err(HostError::with_kind("the launch was refused", Kind::Usage))
        });
        let err = TurnsArea::new(host.clone()).run_turn(t).err().expect("the launch fails");
        settle().await;
        assert_eq!(said(&err), ("the launch was refused".to_string(), None, Some(Kind::Usage)));
        assert_eq!(seen.get(), Some(false), "the waiting box was read again before the launch");
        assert_eq!(host.hooks.calls(), vec!["idle_hold(w1)", "idle_release(w1)"]);
        assert_eq!(host.rows.borrow().keys().cloned().collect::<Vec<_>>(), vec![TURN.to_string()]);
        assert_eq!(json_of(&turn)["id"], TURN);
    });
}

// N10: turns.ts:581-587
#[test]
fn the_hold_is_released_when_the_turn_finishes() {
    run_local(async {
        let r = rig();
        settle().await;
        finish(&r, json!({ "status": "completed", "text": "done" }));
        settle().await;
        assert_eq!(
            only(&r.host, &["idle_hold", "arm_disk_watch", "idle_release", "read_disk"]),
            vec!["idle_hold(w1)", "arm_disk_watch", "idle_release(w1)", "read_disk"]
        );
    });
}

// N10: turns.ts:585
#[test]
fn a_disk_that_does_not_read_after_the_turn_is_warned() {
    run_local(async {
        let lines = console::capture();
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "TurnsCtx::read_disk",
            Rc::new(|_e: &Entry| async { Err(failed("the disk is full")) }.boxed_local()),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(lines.lines(), vec!["disk of w1 not read after its turn: the disk is full".to_string()]);
    });
}

// N10: turns.ts:575
#[test]
fn a_kept_process_that_still_says_waiting_reads_the_box_again() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let waiting = Rc::new(Cell::new(true));
        let w = waiting.clone();
        rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| t.waiting = Some(w));
        assert!(!waiting.get());
    });
}

// ---- asks (N4-N8) ------------------------------------------------------------------------------------------------

// N4, N6, N8: permission-relay.test.ts:336, 351, 320
#[test]
fn an_open_ask_marks_the_row_asking_and_counts_the_time_on_the_person() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({ "claudeSessionId": "agent-sess" }));
        let waiting = Rc::new(Cell::new(false));
        turn.borrow_mut().turn_live = Some(TurnLive::default());
        let w = waiting.clone();
        let state = state_on(&host, &entry, &turn, |t| t.waiting = Some(w));
        state.forward(adapter_event(ask_event("ask_1")));
        settle().await;
        assert_eq!(turn.borrow().view.asking, Some(asking(&ask_doc("ask_1"))));
        let lead = host.lead_asks.borrow()[THREAD].clone();
        let mut named = ask_doc("ask_1");
        named["options"][2]["label"] = json!(permission_mode_option_label("acceptEdits"));
        assert_eq!(serde_json::to_value(&lead).unwrap(), named);
        assert!(waiting.get());
        assert_eq!((live_of(&turn).waiting_since, live_of(&turn).waited), (num(T0), num(0)));
        assert_eq!(host.hooks.called("persist_sessions").len(), 1);

        host.clock.advance(3 * MIN);
        state.forward(adapter_event(ask_event("ask_2")));
        assert_eq!(turn.borrow().view.asking, Some(asking(&ask_doc("ask_1"))), "the oldest open ask leads");
        assert_eq!(live_of(&turn).waiting_since, num(T0), "the span began at the first open");
        host.clock.advance(2 * MIN);
        state.close_ask("ask_1", PermissionOutcome::Allowed, Some("allow"));
        state.close_ask("ask_1", PermissionOutcome::Allowed, Some("allow"));
        settle().await;
        assert_eq!(turn.borrow().view.asking, Some(asking(&ask_doc("ask_2"))));
        assert_eq!(host.lead_asks.borrow()[THREAD].ask_id, "ask_2");
        assert!(waiting.get());
        assert_eq!(live_of(&turn).waiting_since, num(T0));
        assert_eq!(
            host.recorded_of("session.permission.closed"),
            vec![on_agent_session(closed_row("ask_1", "allowed", Some("allow")))],
            "a closed ask closes once"
        );

        host.clock.advance(15 * MIN);
        assert_eq!(state.waited_so_far(), (20 * MIN) as f64, "the open span counts to now");
        let reply = result(json!({ "status": "completed", "text": "ok" }));
        assert_eq!(state.with_waited(reply.clone()), result(json!({ "status": "completed", "text": "ok", "waitedMs": 20 * MIN })));

        state.close_ask("ask_2", PermissionOutcome::Cancelled, None);
        settle().await;
        assert_eq!(turn.borrow().view.asking, None);
        assert!(host.lead_asks.borrow().get(THREAD).is_none());
        assert!(!waiting.get());
        assert_eq!((live_of(&turn).waited, live_of(&turn).waiting_since), (num(20 * MIN), None));
        assert_eq!(
            host.recorded_of("session.permission.closed"),
            vec![on_agent_session(closed_row("ask_1", "allowed", Some("allow"))), on_agent_session(closed_row("ask_2", "cancelled", None))]
        );
        assert_eq!(host.hooks.called("persist_sessions").len(), 4, "one write for each open and each close");
        host.clock.advance(10 * MIN);
        assert_eq!(state.waited_so_far(), (20 * MIN) as f64, "a closed span stops counting");
    });
}

// N8
#[test]
fn a_reply_carries_waited_ms_only_where_the_turn_waited() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        let state = state_on(&host, &entry, &turn, |_| {});
        let reply = result(json!({ "status": "completed", "text": "ok" }));
        assert_eq!(state.waited_so_far(), 0.0);
        assert_eq!(state.with_waited(reply.clone()), reply);
    });
}

// N4, N8: turns.ts:176-179
#[test]
fn a_span_open_at_the_restart_is_counted_from_where_it_began() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        turn.borrow_mut().turn_live = Some(TurnLive { waited: num(1_000), waiting_since: num(T0 - 4_000), ..TurnLive::default() });
        let state = state_on(&host, &entry, &turn, |_| {});
        assert_eq!(state.waited_so_far(), 5_000.0);
        state.forward(adapter_event(ask_event("ask_1")));
        assert_eq!(live_of(&turn).waiting_since, num(T0 - 4_000), "an open span is kept, not restarted");
        host.clock.advance(1_000);
        state.close_ask("ask_1", PermissionOutcome::Denied, Some("deny"));
        assert_eq!((live_of(&turn).waited, live_of(&turn).waiting_since), (num(6_000), None));
    });
}

// N5: permission-relay.test.ts:179
#[test]
fn a_mode_option_reads_the_catalogs_label() {
    run_local(async {
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&str, &str) -> Option<String>>(
            "TurnsCtx::mode_label",
            Rc::new(|h: &str, m: &str| if (h, m) == ("claude", "acceptEdits") { Some("Accept edits".to_string()) } else { None }),
        );
        let turn = turn_row(json!({}));
        let state = state_on(&host, &entry, &turn, |_| {});
        let mut ask = ask_doc("ask_1");
        ask["options"] = json!([
            { "id": "allow", "label": "Allow", "effect": "allow" },
            { "id": "mode:acceptEdits", "label": "acceptEdits", "effect": "mode", "mode": "acceptEdits" },
            { "id": "mode:plan", "label": "plan", "effect": "mode", "mode": "plan" },
            { "id": "mode:odd", "label": "Odd", "effect": "mode" },
            { "id": "deny", "label": "Deny", "effect": "deny" },
        ]);
        let named = state.named(&serde_json::from_value(ask.clone()).expect("an ask"));
        let mut expected = ask.clone();
        expected["options"][1]["label"] = json!(permission_mode_option_label("Accept edits"));
        expected["options"][2]["label"] = json!(permission_mode_option_label("plan"));
        assert_eq!(serde_json::to_value(&named).unwrap(), expected);
        assert_eq!(host.hooks.called("mode_label"), vec!["mode_label(claude, acceptEdits)", "mode_label(claude, plan)"]);
    });
}

// N7: permission-relay.test.ts:206, 288, 303
#[test]
fn answer_allows_denies_and_carries_a_reason_only_on_a_deny() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.answers = true), |_| {});
        for id in ["ask_1", "ask_2", "ask_3", "ask_4", "ask_5"] {
            r.send(ask_event(id));
        }
        let ask = |id: &str, option: &str, reason: Option<&str>| {
            r.handle.answer(id.into(), option.into(), reason.map(String::from)).expect("an answer road")
        };
        let reason = "Run only the thread-rows file";
        for (id, option, reason) in [
            ("ask_1", "allow", None),
            ("ask_2", "deny", Some("  Run only the thread-rows file ")),
            ("ask_3", "deny", Some("   ")),
            ("ask_4", "mode:acceptEdits", None),
            ("ask_5", "allow", Some("because")),
        ] {
            assert_eq!(ask(id, option, reason).await.ok(), Some(answered(SessionAnswerOutcome::Answered)), "{id}");
        }
        assert_eq!(
            r.session.calls(),
            vec![
                format!("answer(ask_1, allow, Allowed, {}, \"\")", denied_line(None)),
                format!("answer(ask_2, deny, Denied, {}, {:?})", denied_line(Some(reason)), reason),
                format!("answer(ask_3, deny, Denied, {}, \"\")", denied_line(None)),
                format!("answer(ask_4, mode:acceptEdits, Allowed, {}, \"\")", denied_line(None)),
                format!("answer(ask_5, allow, Allowed, {}, \"\")", denied_line(None)),
            ]
        );
    });
}

// N7: permission-relay.test.ts:268, 303
#[test]
fn a_question_takes_a_pick_naming_several_and_an_answer_typed_in_other() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.answers = true), |_| {});
        r.send(question_event("ask_q"));
        r.send(question_event("ask_t"));
        let both = r.handle.answer("ask_q".into(), "q0:o0|q0:o1".into(), None).expect("a road").await;
        assert_eq!(both.ok(), Some(answered(SessionAnswerOutcome::Answered)));
        let typed = r.handle.answer("ask_t".into(), "q0:other:neither%2C%20C".into(), None).expect("a road").await;
        assert_eq!(typed.ok(), Some(answered(SessionAnswerOutcome::Answered)));
        assert_eq!(
            r.session.calls(),
            vec![
                format!("answer(ask_q, q0:o0|q0:o1, Allowed, {}, \"\")", denied_line(None)),
                format!("answer(ask_t, q0:other:neither%2C%20C, Allowed, {}, \"\")", denied_line(None)),
            ]
        );
    });
}

// N7: permission-relay.test.ts:206, 411
#[test]
fn answer_on_a_closed_ask_is_gone() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.answers = true), |_| {});
        r.send(ask_event("ask_1"));
        r.send(close_event("ask_1", "allowed"));
        for id in ["ask_1", "ask_9"] {
            let got = r.handle.answer(id.into(), "allow".into(), None).expect("a road").await;
            assert_eq!(got.ok(), Some(answered(SessionAnswerOutcome::Gone)), "{id}");
        }
        assert_eq!(r.session.calls(), Vec::<String>::new(), "nothing reaches the harness for an ask not open");
        r.send(ask_event("ask_2"));
        *r.session.answer_answer.lock().unwrap() = AnswerOutcome::Gone;
        let got = r.handle.answer("ask_2".into(), "allow".into(), None).expect("a road").await;
        assert_eq!(got.ok(), Some(answered(SessionAnswerOutcome::Gone)), "a harness that says gone reads gone");
    });
}

// N7: permission-relay.test.ts:411
#[test]
fn answer_naming_no_option_is_no_option() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.answers = true), |_| {});
        r.send(ask_event("ask_1"));
        for pick in ["mode:bypassPermissions", "allow|nope"] {
            let got = r.handle.answer("ask_1".into(), pick.into(), None).expect("a road").await;
            assert_eq!(got.ok(), Some(answered(SessionAnswerOutcome::NoOption)), "{pick}");
        }
        assert_eq!(r.session.calls(), Vec::<String>::new());
    });
}

// N7: turns.ts:271-279
#[test]
fn an_open_ask_with_no_harness_road_is_gone_after_its_option_is_picked() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        let state = state_on(&host, &entry, &turn, |_| {});
        state.forward(adapter_event(ask_event("ask_1")));
        assert_eq!(state.answer("ask_1", "nope", None).await, answered(SessionAnswerOutcome::NoOption));
        assert_eq!(state.answer("ask_1", "allow", None).await, answered(SessionAnswerOutcome::Gone));
    });
}

// N4, N7: turns.ts:271, 626
#[test]
fn an_ask_raised_while_the_session_opens_is_answered_through_its_road() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let waiting = Rc::new(Cell::new(false));
        let session = FakeSession::with(SESS, |s| s.answers = true);
        let slot = Rc::new(RefCell::new(None));
        let mut t = turn_start(&entry, &turn, &session, &slot);
        t.waiting = Some(waiting.clone());
        let s = session.clone();
        t.open = Box::new(move |f: OnEvent| {
            f(adapter_event(ask_event("ask_1")));
            Ok(s as Arc<dyn HarnessSession>)
        });
        let handle = TurnsArea::new(host.clone()).run_turn(t).ok().expect("the turn opens");
        assert!(waiting.get(), "the ask raised in the launch holds the box");
        assert_eq!(turn.borrow().view.asking, Some(asking(&ask_doc("ask_1"))));
        let got = handle.answer("ask_1".into(), "allow".into(), None).expect("a road").await;
        assert_eq!(got.ok(), Some(answered(SessionAnswerOutcome::Answered)));
        assert_eq!(session.calls(), vec![format!("answer(ask_1, allow, Allowed, {}, \"\")", denied_line(None))]);
    });
}

// N4: turns.ts:207-209, forward's turn.tasks
#[test]
fn work_still_running_holds_the_waiting_box_like_an_open_ask() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        let waiting = Rc::new(Cell::new(false));
        let w = waiting.clone();
        let state = state_on(&host, &entry, &turn, |t| t.waiting = Some(w));
        state.forward(adapter_event(json!({ "type": "turn.tasks", "sessionId": SESS, "running": 2 })));
        assert!(waiting.get());
        state.forward(adapter_event(ask_event("ask_1")));
        state.forward(adapter_event(json!({ "type": "turn.tasks", "sessionId": SESS, "running": 0 })));
        assert!(waiting.get(), "an open ask still holds it");
        state.close_ask("ask_1", PermissionOutcome::Allowed, None);
        assert!(!waiting.get());
    });
}

// ---- after_told (N9) ---------------------------------------------------------------------------------------------

fn told_state(told: Option<ToldAs>) -> (Rc<FakeHost>, Rc<TurnState>) {
    let (host, entry) = world();
    let turn = turn_row(json!({}));
    turn.borrow_mut().turn_live = Some(TurnLive { told_as: told, ..TurnLive::default() });
    let state = state_on(&host, &entry, &turn, |_| {});
    (host, state)
}

fn told(status: TurnStatus, body: Option<&str>, said: Option<&str>, promised: bool) -> Option<ToldAs> {
    Some(ToldAs { status, body: body.map(String::from), said: said.map(String::from), promised })
}

// N9: turns.ts:194-205
#[test]
fn after_told_sends_nothing_twice_and_the_rest_of_a_held_end() {
    run_local(async {
        let done = result(json!({ "status": "completed", "text": "All done.", "costUsd": 0.5, "error": "late warning" }));
        let outcome = result(json!({ "status": "completed", "costUsd": 0.5 }));

        let (_h, none_told) = told_state(None);
        assert_eq!(none_told.after_told(&done, true), None, "a held end with no line told sends nothing");
        assert_eq!(none_told.after_told(&done, false), Some(done.clone()), "an unheld end with none told sends the end whole");

        let same = told(TurnStatus::Completed, Some("All done."), Some("All done."), false);
        let (_h, s) = told_state(same.clone());
        assert_eq!(s.after_told(&done, false), None, "the same body and status, nothing promised: nothing again");
        let (_h, s) = told_state(told(TurnStatus::Completed, Some("All done."), Some("All done."), true));
        assert_eq!(s.after_told(&done, false), Some(outcome.clone()), "a promised line sends the outcome alone, no text and no error");

        let (_h, s) = told_state(told(TurnStatus::Completed, Some("something else"), Some("something else"), false));
        assert_eq!(s.after_told(&done, false), Some(done.clone()), "an unheld end whose body differs goes whole");

        let stopped = result(json!({ "status": "interrupted", "text": "All done." }));
        let (_h, s) = told_state(same.clone());
        assert_eq!(s.after_told(&stopped, false), Some(result(json!({ "status": "interrupted" }))), "a new status sends the outcome alone");

        let held_end = result(json!({ "status": "completed", "text": "All done.\n\nTask a finished.\n", "costUsd": 0.5 }));
        let (_h, s) = told_state(same.clone());
        assert_eq!(
            s.after_told(&held_end, true),
            Some(result(json!({ "status": "completed", "text": "Task a finished.", "costUsd": 0.5 }))),
            "a held end sends what follows the told words"
        );
        assert_eq!(s.after_told(&done, true), None, "a held end that is only the told words, same status: nothing");
        let elsewhere = result(json!({ "status": "completed", "text": "Something unrelated." }));
        assert_eq!(s.after_told(&elsewhere, true), None, "a held end that does not start with the told words: status matches, nothing");
        let (_h, s) = told_state(told(TurnStatus::Completed, Some("All done."), None, false));
        assert_eq!(s.after_told(&held_end, true), None, "no told words, no rest");
        let (_h, s) = told_state(told(TurnStatus::Failed, Some("All done."), Some("All done."), false));
        assert_eq!(s.after_told(&done, true), Some(outcome), "a held end of another status than the told one sends the outcome");
    });
}

// ---- the handle (N16) --------------------------------------------------------------------------------------------

// N16: turns.ts:617-633
#[test]
fn the_handle_has_only_the_roads_the_session_has() {
    run_local(async {
        let plain = rig();
        assert!(plain.handle.steer("go".into(), None, vec![]).is_none());
        assert!(plain.handle.answer("a".into(), "allow".into(), None).is_none());
        assert!(plain.handle.set_access("plan".into()).is_none());
        assert!(plain.handle.stop_task("task-1".into()).is_none());
        assert_eq!(plain.handle.tells_unread(), Some(false));
        assert_eq!(plain.handle.notice(), None);

        let full = rig_with(
            FakeSession::with(SESS, |s| {
                s.steers = true;
                s.answers = true;
                s.sets_access = true;
                s.stops_tasks = true;
                s.tells_unread = Some(true);
                *s.task_answer.lock().unwrap() = TaskStop::Refused { error: "no such task".into() };
            }),
            |_| {},
        );
        let steered = full.handle.steer("go on".into(), Some("m1".into()), vec![]).expect("a steer road").await.ok();
        assert_eq!(steered, Some(SteerTaken::Accepted));
        *full.session.steer_answer.lock().unwrap() = Ok(SteerOutcome::NotRunning);
        let steered = full.handle.steer("again".into(), None, vec![]).expect("a steer road").await.ok();
        assert_eq!(steered, Some(SteerTaken::NotRunning));
        let stop = full.handle.stop_task("task-1".into()).expect("a stop road").await.ok();
        assert_eq!(stop, Some(TaskStop::Refused { error: "no such task".into() }));
        assert_eq!(full.handle.tells_unread(), Some(true));
        assert_eq!(full.session.calls(), vec!["steer(go on, Some(\"m1\"))", "steer(again, None)", "stop_task(task-1)"]);
    });
}

// N16
#[test]
fn the_handle_reads_the_row_as_it_stands_and_finishes_with_the_session() {
    run_local(async {
        let r = rig();
        let mut copy = r.handle.view();
        copy.prompt = Some("changed in the copy".into());
        assert_eq!(r.turn.borrow().view.prompt, None, "view() hands a copy");
        r.turn.borrow_mut().view.claude_session_id = Some("agent-1".into());
        assert_eq!(r.handle.view().claude_session_id.as_deref(), Some("agent-1"));
        let done = result(json!({ "status": "completed", "text": "ok" }));
        r.session.finish(done.clone());
        settle().await;
        assert_eq!(r.handle.finished().await, done);
    });
}

// N16: permission-relay.test.ts:646, 679
#[test]
fn set_access_moves_the_row_only_when_set() {
    run_local(async {
        for (outcome, moved, expected_row) in [
            (AccessOutcome::Set, AccessMove::Set, "acceptEdits"),
            (AccessOutcome::Refused, AccessMove::Refused, "bypassPermissions"),
            (AccessOutcome::Gone, AccessMove::Gone, "bypassPermissions"),
        ] {
            let r = rig_with(
                FakeSession::with(SESS, |s| {
                    s.sets_access = true;
                    *s.access_answer.lock().unwrap() = outcome;
                }),
                |_| {},
            );
            assert_eq!(r.handle.view().permission_mode.as_deref(), Some("bypassPermissions"));
            let got = r.handle.set_access("acceptEdits".into()).expect("an access road").await.ok();
            assert_eq!(got, Some(moved));
            assert_eq!(r.handle.view().permission_mode.as_deref(), Some(expected_row), "{outcome:?}");
            assert_eq!(r.session.calls(), vec!["set_access(acceptEdits)"]);
        }
    });
}

// N16: stop-past-grace.test.ts:45 (the interrupt reaching the harness; the TERM and KILL are the adapter's)
#[test]
fn interrupt_marks_the_turn_stopped_and_reaches_the_session() {
    run_local(async {
        let r = rig();
        r.handle.interrupt().await;
        assert_eq!(r.session.calls(), vec!["interrupt"]);
        finish(&r, json!({ "status": "interrupted" }));
        settle().await;
        assert_eq!(only(&r.host, &["send_back"]), vec!["send_back(true)"]);
    });
}

// ---- end (N17) ---------------------------------------------------------------------------------------------------

// N17: permission-relay.test.ts:351, 362
#[test]
fn end_closes_open_asks_settles_the_cut_and_interrupts_once() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let waiting = Rc::new(Cell::new(false));
        let w = waiting.clone();
        let r = rig_on(&host, &entry, FakeSession::with(SESS, |s| s.answers = true), turn.clone(), |t| {
            t.waiting = Some(w);
            t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c0".into())), cwd: "/work/w1".into() });
        });
        r.send(ask_event("ask_1"));
        r.send(ask_event("ask_2"));
        settle().await;
        assert_eq!(r.snapshot().as_deref(), Some("c0"));
        let mark = host.hooks.calls().len();

        r.end("the machine is paused", true, true);
        settle().await;
        assert_eq!(
            host.recorded_of("session.permission.closed"),
            vec![closed_row("ask_1", "cancelled", None), closed_row("ask_2", "cancelled", None)]
        );
        assert_eq!(turn.borrow().view.asking, None);
        assert!(host.lead_asks.borrow().get(THREAD).is_none());
        assert!(!waiting.get());
        assert_eq!(r.snapshot(), None, "the launch's commit comes off the row");
        assert_eq!(r.session.calls(), vec!["interrupt"]);
        let calls = after(&host, mark);
        assert_eq!(only(&host, &["settle_cut"]), vec!["settle_cut(the machine is paused, true, true)"]);
        assert_eq!(calls.iter().filter(|c| c.starts_with("persist_sessions")).count(), 3, "two closes and the end");
        assert_eq!(calls.iter().filter(|c| c.starts_with("slates_turn_ended")).collect::<Vec<_>>(), vec!["slates_turn_ended(t1, turn-1)"]);
        assert_eq!(calls.iter().filter(|c| *c == "keep_checkpoint").count(), 1);

        r.send(ask_event("ask_3"));
        assert_eq!(turn.borrow().view.asking, None, "once ended the turn's events are dropped");
        let mark = host.hooks.calls().len();
        r.end("again", false, false);
        settle().await;
        assert_eq!(after(&host, mark), Vec::<String>::new());
        assert_eq!(r.session.calls(), vec!["interrupt"]);

        finish(&r, json!({ "status": "interrupted" }));
        settle().await;
        assert_eq!(turn.borrow().view.status, SessionStatus::Running, "an end here leaves the status to the cut");
        assert_eq!(only(&host, &["send_back"]), vec!["send_back(true)"]);
    });
}

// N17
#[test]
fn end_after_the_turn_settled_does_nothing() {
    run_local(async {
        let r = rig();
        finish(&r, json!({ "status": "completed", "text": "ok" }));
        settle().await;
        assert_eq!(r.turn.borrow().view.status, SessionStatus::Completed);
        let mark = r.host.hooks.calls().len();
        r.end("late", true, false);
        settle().await;
        assert_eq!(after(&r.host, mark), Vec::<String>::new());
        assert_eq!(r.session.calls(), Vec::<String>::new());

        let other = rig();
        other.turn.borrow_mut().view.status = SessionStatus::Interrupted;
        let mark = other.host.hooks.calls().len();
        other.end("late", true, false);
        settle().await;
        assert_eq!(after(&other.host, mark), Vec::<String>::new(), "a view no longer running is not ended");
        assert_eq!(other.session.calls(), Vec::<String>::new());
    });
}

// N17: RUNTIME-AGENTS.md #1646
#[test]
fn end_hands_settle_cut_the_turns_own_row_fields() {
    run_local(async {
        for with_all in [true, false] {
            let (host, entry) = world();
            let cut: Rc<RefCell<Vec<Row>>> = Rc::default();
            let c = cut.clone();
            host.hooks.set::<dyn Fn(&Row, &str, bool, bool) -> ()>(
                "TurnsCtx::settle_cut",
                Rc::new(move |row: &Row, _r: &str, _s: bool, _u: bool| c.borrow_mut().push(row.clone())),
            );
            let turn = turn_row(json!({}));
            launching(&host, &turn);
            let asked =
                TurnAsked { prompt: "write it".into(), effort: None, typed: None, request_id: None, owed: None, awaits_prompt: None };
            if with_all {
                turn.borrow_mut().asked = Some(asked.clone());
            }
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| {
                if with_all {
                    t.notify = Some(vec!["thr_lead".into()]);
                    t.notify_by = Some(scope());
                    t.notify_road = Some(WorkspaceOrigin::Relayed);
                }
            });
            r.end("cut", false, false);
            let rows = cut.borrow();
            assert_eq!(rows.len(), 1);
            assert!(Rc::ptr_eq(&rows[0], &turn));
            let row = rows[0].borrow();
            assert_eq!(row.turn_id, TURN);
            assert!(row.turn_live.is_some(), "the turn's live state");
            if with_all {
                assert_eq!(row.notify, Some(vec!["thr_lead".to_string()]));
                assert_eq!(row.notify_by, Some(scope()));
                assert_eq!(row.notify_road, Some(WorkspaceOrigin::Relayed));
                assert_eq!(row.asked, Some(asked));
            } else {
                assert_eq!((row.notify.clone(), row.notify_by.clone(), row.notify_road), (None, None, None));
                assert!(row.asked.is_none());
            }
            assert_eq!(host.hooks.called("settle_cut"), vec!["settle_cut(cut, false, false)"]);
        }
    });
}

// ---- turn over (N3, N18) -----------------------------------------------------------------------------------------

fn rewound() -> Value {
    json!({ "harness": "claude", "rewound": { "before": "abc", "at": 1 } })
}

// N18
#[test]
fn the_turns_end_drops_every_undo_on_its_workspace_and_says_so() {
    run_local(async {
        let (host, entry) = world();
        host.add_record("t1", W, thread_record(rewound()));
        host.add_record("t3", "w2", thread_record(rewound()));
        host.add_record("t4", W, thread_record(json!({ "harness": "claude" })));
        host.add_record("t2", W, thread_record(rewound()));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        settle().await;
        let mark = host.hooks.calls().len();
        r.send(session_end());
        settle().await;
        let rewound_of = |t: &str| host.records.borrow()[t].rewound.clone();
        assert_eq!((rewound_of("t1"), rewound_of("t2"), rewound_of("t4")), (None, None, None));
        assert!(rewound_of("t3").is_some(), "another workspace's undo stays");
        assert_eq!(host.emitted.borrow().clone(), vec![json!({ "type": "thread.marked", "workspaceId": W, "threadIds": ["t1", "t2"] })]);
        assert_eq!(after(&host, mark).iter().filter(|c| c.starts_with("persist_sessions")).count(), 1);

        let (host, entry) = world();
        host.add_record("t4", W, thread_record(json!({ "harness": "claude" })));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        settle().await;
        let mark = host.hooks.calls().len();
        r.send(session_end());
        settle().await;
        assert_eq!(host.emitted.borrow().clone(), Vec::<Value>::new());
        assert_eq!(after(&host, mark).iter().filter(|c| c.starts_with("persist_sessions")).count(), 0);
    });
}

// N18: runs once however many ends
#[test]
fn the_turn_is_over_once() {
    run_local(async {
        let r = rig();
        r.send(session_end());
        settle().await;
        let mark = r.host.hooks.calls().len();
        r.end("late", false, false);
        r.send(session_end());
        settle().await;
        assert_eq!(after(&r.host, mark).iter().filter(|c| c.starts_with("slates_turn_ended") || *c == "keep_checkpoint").count(), 0);
        assert_eq!(r.host.hooks.called("slates_turn_ended"), vec!["slates_turn_ended(t1, turn-1)"]);
    });
}

// N18, N3: turns.ts:132-154, 118
#[test]
fn the_turns_end_keeps_a_checkpoint_and_writes_what_the_agent_held() {
    run_local(async {
        let (host, entry) = world();
        let asks: Rc<RefCell<Vec<CheckpointAsk>>> = Rc::default();
        let (tx, rx) = oneshot::channel::<()>();
        let landing: Joined = rx.map(|_| ()).boxed_local().shared();
        let (a, l) = (asks.clone(), landing.clone());
        host.hooks.set::<dyn Fn(&Entry, CheckpointAsk) -> Joined>(
            "TurnsCtx::keep_checkpoint",
            Rc::new(move |_e: &Entry, ask: CheckpointAsk| {
                a.borrow_mut().push(ask);
                l.clone()
            }),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        assert_eq!(host.held_at_close.borrow().len(), 1, "the turn holds a close-time write while it runs");
        r.send(json!({ "type": "turn.usage", "sessionId": SESS, "tokens": 1200, "context": 5000, "window": 200000 }));
        r.send(json!({ "type": "turn.anchor", "sessionId": SESS, "anchor": "a-1", "kept": "cannot cut" }));
        assert_eq!(host.recorded_of("session.context"), Vec::<Value>::new(), "nothing is written while the turn runs");
        r.send(session_end());
        settle().await;
        assert_eq!(
            asks.borrow().clone(),
            vec![CheckpointAsk {
                session_id: SESS.into(),
                thread_id: THREAD.into(),
                turn_id: TURN.into(),
                anchor: Some("a-1".into()),
                kept: Some("cannot cut".into())
            }]
        );
        assert_eq!(
            host.recorded_of("session.context"),
            vec![
                json!({ "type": "session.context", "workspaceId": W, "sessionId": SESS, "turnId": TURN, "threadId": THREAD, "context": 5000, "window": 200000 })
            ]
        );
        assert_eq!(host.held_at_close.borrow().len(), 0);
        assert!(host.checkpoints_landing.borrow().contains_key(THREAD), "the thread's landing checkpoint is set");
        tx.send(()).unwrap();
        settle().await;
        assert!(!host.checkpoints_landing.borrow().contains_key(THREAD), "and cleared when it lands");
    });
}

// N18
#[test]
fn a_checkpoint_another_turn_replaced_stays_when_the_first_lands() {
    run_local(async {
        let (host, entry) = world();
        let (tx, rx) = oneshot::channel::<()>();
        let landing: Joined = rx.map(|_| ()).boxed_local().shared();
        let l = landing.clone();
        host.hooks.set::<dyn Fn(&Entry, CheckpointAsk) -> Joined>(
            "TurnsCtx::keep_checkpoint",
            Rc::new(move |_e: &Entry, _a: CheckpointAsk| l.clone()),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        r.send(session_end());
        let (_other_tx, other_rx) = oneshot::channel::<()>();
        let other: Joined = other_rx.map(|_| ()).boxed_local().shared();
        host.checkpoints_landing.borrow_mut().insert(THREAD.to_string(), other);
        tx.send(()).unwrap();
        settle().await;
        assert!(host.checkpoints_landing.borrow().contains_key(THREAD), "the later turn's checkpoint is not taken off");
    });
}

// N18: a turn with nothing to keep
#[test]
fn a_turn_with_nothing_held_and_no_anchor_keeps_a_bare_checkpoint() {
    run_local(async {
        let (host, entry) = world();
        let asks: Rc<RefCell<Vec<CheckpointAsk>>> = Rc::default();
        let a = asks.clone();
        host.hooks.set::<dyn Fn(&Entry, CheckpointAsk) -> Joined>(
            "TurnsCtx::keep_checkpoint",
            Rc::new(move |_e: &Entry, ask: CheckpointAsk| {
                a.borrow_mut().push(ask);
                done()
            }),
        );
        let turn = turn_row(json!({ "claudeSessionId": "agent-1" }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        r.send(session_end());
        settle().await;
        assert_eq!(
            asks.borrow().clone(),
            vec![CheckpointAsk { session_id: "agent-1".into(), thread_id: THREAD.into(), turn_id: TURN.into(), anchor: None, kept: None }],
            "the agent's session id, else the row's"
        );
        assert_eq!(host.recorded_of("session.context"), Vec::<Value>::new());
    });
}

// N3: turns.ts:118-121
#[test]
fn a_context_the_result_named_is_not_written_again() {
    run_local(async {
        let r = rig();
        r.send(json!({ "type": "turn.usage", "sessionId": SESS, "tokens": 1200, "context": 5000 }));
        r.send(done_event(json!({ "status": "completed", "text": "ok", "tokens": { "input": 10, "output": 5, "context": 5000 } })));
        r.send(session_end());
        settle().await;
        assert_eq!(r.host.recorded_of("session.context"), Vec::<Value>::new());
    });
}

// N3: a closing host runs what the turn registered
#[test]
fn a_closing_host_writes_what_a_running_turn_held() {
    run_local(async {
        let r = rig();
        r.send(json!({ "type": "turn.usage", "sessionId": SESS, "tokens": 1200, "context": 3000 }));
        let writes: Vec<Rc<dyn Fn()>> = r.host.held_at_close.borrow().clone();
        assert_eq!(writes.len(), 1);
        for w in writes {
            w();
        }
        assert_eq!(
            r.host.recorded_of("session.context"),
            vec![
                json!({ "type": "session.context", "workspaceId": W, "sessionId": SESS, "turnId": TURN, "threadId": THREAD, "context": 3000 })
            ],
            "no window key where the agent named none"
        );
    });
}

// ---- row, changes and the snapshot (N14, N15, N19, N20) ----------------------------------------------------------

// N14: turns.ts:651-657
#[test]
fn a_shared_daemon_turn_names_its_group() {
    run_local(async {
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry) -> bool>("TurnsCtx::shared_daemon", Rc::new(|_e: &Entry| true));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::with(SESS, |s| s.pid = Some(4321)), turn, |_| {});
        settle().await;
        assert_eq!(r.pid(), Some(4321));
        assert_eq!(
            host.hooks.calls(),
            vec!["idle_hold(w1)", "arm_disk_watch", "shared_daemon", "arm_roots_recheck", "port_roots_moved(w1)", "persist_sessions(w1)"]
        );
        let mut group = IndexMap::new();
        group.insert(4321u32, THREAD.to_string());
        let mut groups = IndexMap::new();
        groups.insert(W.to_string(), group.clone());
        assert_eq!(*host.turn_groups.borrow(), groups);

        let second = turn_row(json!({ "id": "turn-2", "threadId": "t2" }));
        let slot = Rc::new(RefCell::new(None));
        let session = FakeSession::with("sess-2", |s| s.pid = Some(77));
        let mut t = turn_start(&entry, &second, &session, &slot);
        t.thread_id = "t2".into();
        t.turn_id = "turn-2".into();
        TurnsArea::new(host.clone()).run_turn(t).ok().expect("a second turn");
        group.insert(77u32, "t2".to_string());
        groups.insert(W.to_string(), group);
        assert_eq!(*host.turn_groups.borrow(), groups, "a second turn joins the workspace's group");
    });
}

// N14
#[test]
fn a_turn_on_a_private_daemon_or_with_no_pid_names_no_group() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.pid = Some(4321)), |_| {});
        settle().await;
        assert_eq!(
            r.host.hooks.calls(),
            vec!["idle_hold(w1)", "arm_disk_watch", "shared_daemon", "port_roots_moved(w1)", "persist_sessions(w1)"]
        );
        assert!(r.host.turn_groups.borrow().is_empty());

        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry) -> bool>("TurnsCtx::shared_daemon", Rc::new(|_e: &Entry| true));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        settle().await;
        assert_eq!(host.hooks.calls(), vec!["idle_hold(w1)", "arm_disk_watch", "port_roots_moved(w1)", "persist_sessions(w1)"]);
        assert!(host.turn_groups.borrow().is_empty());
    });
}

fn changes_hook(host: &FakeHost, asks: &Rc<RefCell<Vec<TurnChangesAsk>>>, answer: Rc<RefCell<Option<oneshot::Receiver<bool>>>>) {
    let a = asks.clone();
    host.hooks.set::<dyn Fn(&Entry, TurnChangesAsk) -> LocalBoxFuture<'static, Result<bool, HostError>>>(
        "TurnsCtx::read_turn_changes",
        Rc::new(move |_e: &Entry, ask: TurnChangesAsk| {
            a.borrow_mut().push(ask);
            let rx = answer.borrow_mut().take();
            async move {
                match rx {
                    Some(rx) => Ok(rx.await.unwrap_or(false)),
                    None => Ok(true),
                }
            }
            .boxed_local()
        }),
    );
}

fn taking() -> (oneshot::Sender<Option<String>>, TurnSnapshot) {
    let (tx, rx) = oneshot::channel::<Option<String>>();
    let from = rx.map(|r| r.unwrap_or(None)).boxed_local().shared();
    (tx, TurnSnapshot { from: SnapshotFrom::Taking(from), cwd: "/work/w1".into() })
}

fn changes_ask(from: &str, started_at: i64, wrote: Option<BTreeSet<String>>) -> TurnChangesAsk {
    TurnChangesAsk {
        session_id: SESS.into(),
        turn_id: TURN.into(),
        thread_id: THREAD.into(),
        cwd: "/work/w1".into(),
        from: from.into(),
        started_at,
        wrote,
    }
}

// N15, N19, N20: turn-changes.test.ts:170, 483
#[test]
fn the_snapshot_rides_the_row_until_the_changes_are_read() {
    run_local(async {
        let (host, entry) = world();
        let asks: Rc<RefCell<Vec<TurnChangesAsk>>> = Rc::default();
        let (read_tx, read_rx) = oneshot::channel::<bool>();
        changes_hook(&host, &asks, Rc::new(RefCell::new(Some(read_rx))));
        let (snap_tx, snapshot) = taking();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
            t.snapshot = Some(snapshot);
            t.turn_token = Some("tok-1".into());
        });
        settle().await;
        assert_eq!(r.snapshot(), None, "no commit on the row while the snapshot is still being taken");
        let persists = host.hooks.called("persist_sessions").len();
        snap_tx.send(Some("c1".into())).unwrap();
        settle().await;
        assert_eq!(r.snapshot().as_deref(), Some("c1"));
        assert_eq!(host.hooks.called("persist_sessions").len(), persists + 1);

        r.send(done_event(json!({ "status": "completed", "text": "ok" })));
        settle().await;
        assert_eq!(asks.borrow().clone(), vec![changes_ask("c1", T0, None)]);
        r.send(session_end());
        settle().await;
        assert_eq!(asks.borrow().len(), 1, "the changes are read once");

        finish(&r, json!({ "status": "completed", "text": "ok" }));
        settle().await;
        assert_eq!(r.snapshot().as_deref(), Some("c1"), "the commit stays while the read is out");
        assert_eq!(r.token(), None, "the token goes with the exit");
        let persists = host.hooks.called("persist_sessions").len();
        read_tx.send(false).unwrap();
        settle().await;
        assert_eq!(r.snapshot(), None, "a read that lands after the end takes the commit off");
        assert_eq!(host.hooks.called("persist_sessions").len(), persists + 1);
    });
}

// N19, N20: turn-changes.test.ts:170
#[test]
fn the_commit_comes_off_at_the_end_when_the_read_was_already_in() {
    run_local(async {
        let (host, entry) = world();
        let asks: Rc<RefCell<Vec<TurnChangesAsk>>> = Rc::default();
        changes_hook(&host, &asks, Rc::default());
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
            t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c1".into())), cwd: "/work/w1".into() })
        });
        r.send(done_event(json!({ "status": "completed" })));
        settle().await;
        assert_eq!(asks.borrow().len(), 1);
        assert_eq!(r.snapshot().as_deref(), Some("c1"), "a read that lands before the end leaves the row alone");
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(r.snapshot(), None);
    });
}

// N19: turn-changes.test.ts:483
#[test]
fn a_read_that_fails_as_the_host_closes_leaves_the_commit_on_disk() {
    run_local(async {
        for (read, closing, written) in [(false, true, false), (true, true, true), (false, false, true)] {
            let (host, entry) = world();
            let asks: Rc<RefCell<Vec<TurnChangesAsk>>> = Rc::default();
            let (read_tx, read_rx) = oneshot::channel::<bool>();
            changes_hook(&host, &asks, Rc::new(RefCell::new(Some(read_rx))));
            let turn = turn_row(json!({}));
            launching(&host, &turn);
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
                t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c1".into())), cwd: "/work/w1".into() })
            });
            r.send(done_event(json!({ "status": "completed" })));
            finish(&r, json!({ "status": "completed" }));
            settle().await;
            host.closing.set(closing);
            let persists = host.hooks.called("persist_sessions").len();
            read_tx.send(read).unwrap();
            settle().await;
            let wrote = host.hooks.called("persist_sessions").len() - persists;
            assert_eq!(wrote, usize::from(written), "read {read}, closing {closing}");
        }
    });
}

// N19
#[test]
fn no_changes_are_read_without_a_snapshot_or_where_they_were_written() {
    run_local(async {
        for (snapshot, written) in [
            (None, false),
            (Some(TurnSnapshot { from: SnapshotFrom::Known(None), cwd: "/work/w1".into() }), false),
            (Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c1".into())), cwd: "/work/w1".into() }), true),
        ] {
            let (host, entry) = world();
            let asks: Rc<RefCell<Vec<TurnChangesAsk>>> = Rc::default();
            changes_hook(&host, &asks, Rc::default());
            let turn = turn_row(json!({}));
            launching(&host, &turn);
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
                t.snapshot = snapshot;
                if written {
                    t.written = Some(TurnWritten { changes: true, ..TurnWritten::default() });
                }
            });
            r.send(done_event(json!({ "status": "completed" })));
            r.send(session_end());
            settle().await;
            assert_eq!(host.hooks.called("read_turn_changes"), Vec::<String>::new());
        }
    });
}

// N19: turns.ts:88
#[test]
fn the_changes_ask_names_the_start_and_the_edits_only_where_the_harness_reports_them() {
    run_local(async {
        for (reports, expected_wrote) in [(true, Some(BTreeSet::<String>::new())), (false, None)] {
            let (host, entry) = world();
            let asks: Rc<RefCell<Vec<TurnChangesAsk>>> = Rc::default();
            changes_hook(&host, &asks, Rc::default());
            let turn = turn_row(json!({}));
            turn.borrow_mut().view.started_at = None;
            launching(&host, &turn);
            host.clock.advance(5_000);
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
                t.reports_edits = reports;
                t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c1".into())), cwd: "/work/w1".into() });
            });
            r.send(done_event(json!({ "status": "completed" })));
            settle().await;
            assert_eq!(
                asks.borrow().clone(),
                vec![changes_ask("c1", T0 + 5_000, expected_wrote)],
                "reports edits {reports}: a view with no start reads the clock"
            );
        }
    });
}

// N15: turns.ts:659-667
#[test]
fn a_snapshot_that_answers_nothing_or_after_the_row_moved_on_writes_nothing() {
    run_local(async {
        let (host, entry) = world();
        let (tx, snapshot) = taking();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| t.snapshot = Some(snapshot));
        settle().await;
        let persists = host.hooks.called("persist_sessions").len();
        tx.send(None).unwrap();
        settle().await;
        assert_eq!((r.snapshot(), host.hooks.called("persist_sessions").len()), (None, persists));

        for stale in ["not running", "another turn"] {
            let (host, entry) = world();
            let (tx, snapshot) = taking();
            let turn = turn_row(json!({}));
            launching(&host, &turn);
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| t.snapshot = Some(snapshot));
            settle().await;
            if stale == "not running" {
                turn.borrow_mut().view.status = SessionStatus::Completed;
            } else {
                host.rows.borrow()[SESS].borrow_mut().turn_id = "turn-2".into();
            }
            let persists = host.hooks.called("persist_sessions").len();
            tx.send(Some("c1".into())).unwrap();
            settle().await;
            assert_eq!((r.snapshot(), host.hooks.called("persist_sessions").len()), (None, persists), "{stale}");
        }
    });
}

// N20: turns.ts:672-673
#[test]
fn a_row_another_turn_took_over_keeps_its_snapshot_and_token() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
            t.turn_token = Some("tok-1".into());
            t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c1".into())), cwd: "/work/w1".into() });
        });
        {
            let rows = host.rows.borrow();
            let mut row = rows[SESS].borrow_mut();
            row.turn_id = "turn-2".into();
            row.turn_token = Some("tok-2".into());
            row.snapshot = Some("c9".into());
        }
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!((r.token(), r.snapshot()), (Some("tok-2".to_string()), Some("c9".to_string())));
    });
}

// ---- the exit (N21-N26) ------------------------------------------------------------------------------------------

type Holds = Rc<RefCell<Vec<(String, KeptHand)>>>;

fn hold_hook(host: &FakeHost, takes: bool) -> Holds {
    let holds: Holds = Rc::default();
    let h = holds.clone();
    host.hooks.set::<dyn Fn(&str, KeptHand) -> Option<KeptHand>>(
        "TurnsCtx::hold_kept",
        Rc::new(move |thread: &str, hold: KeptHand| {
            if !takes {
                return Some(hold);
            }
            h.borrow_mut().push((thread.to_string(), hold));
            None
        }),
    );
    holds
}

fn keeping(closes: &Arc<Mutex<Vec<bool>>>, pid: Option<u32>) -> Arc<FakeSession> {
    let c = closes.clone();
    FakeSession::with(SESS, move |s| {
        s.pid = pid;
        *s.kept.get_mut().unwrap() = Some(keeper(&c));
    })
}

// N21: thread-keeps-process.test.ts:92
#[test]
fn a_kept_agent_keeps_its_token_and_device() {
    run_local(async {
        let (host, entry) = world();
        let holds = hold_hook(&host, true);
        let closes = Arc::new(Mutex::new(Vec::new()));
        let turn = turn_row(json!({ "claudeSessionId": "agent-1" }));
        launching(&host, &turn);
        let waiting = Rc::new(Cell::new(false));
        let w = waiting.clone();
        let r = rig_on(&host, &entry, keeping(&closes, Some(99)), turn, |t| {
            t.keep = Some(kept_launch());
            t.turn_token = Some("tok-1".into());
            t.scope_device_id = Some("dev-1".into());
            t.waiting = Some(w);
        });
        finish(&r, json!({ "status": "completed", "text": "ok" }));
        settle().await;
        let holds = holds.borrow();
        assert_eq!(holds.len(), 1);
        let (thread, hold) = &holds[0];
        assert_eq!(thread, THREAD);
        assert_eq!(hold.workspace_id, W);
        assert_eq!(hold.launch, kept_launch());
        assert_eq!(hold.session.as_deref(), Some("agent-1"));
        assert_eq!(hold.turn_token, "tok-1");
        assert_eq!(hold.pid, Some(99));
        assert_eq!(hold.scope_device_id.as_deref(), Some("dev-1"));
        assert!(Rc::ptr_eq(&hold.waiting, &waiting));
        assert_eq!(*closes.lock().unwrap(), Vec::<bool>::new(), "a kept agent is not closed");
        assert_eq!(host.hooks.called("revoke_device"), Vec::<String>::new(), "a kept agent keeps its device");
        assert_eq!(r.token(), None, "the row no longer names the token");
    });
}

// N21: turns.ts:680-684
#[test]
fn a_kept_agent_with_no_pid_or_no_waiting_box_is_offered_with_a_fresh_box() {
    run_local(async {
        let (host, entry) = world();
        let holds = hold_hook(&host, true);
        let closes = Arc::new(Mutex::new(Vec::new()));
        let turn = turn_row(json!({ "claudeSessionId": "agent-1" }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, keeping(&closes, None), turn, |t| {
            t.keep = Some(kept_launch());
            t.turn_token = Some("tok-1".into());
        });
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        let holds = holds.borrow();
        assert_eq!(holds.len(), 1);
        assert_eq!((holds[0].1.pid, holds[0].1.scope_device_id.clone(), holds[0].1.waiting.get()), (None, None, false));
    });
}

// N21, N22: thread-keeps-process.test.ts:196
#[test]
fn an_agent_not_kept_is_closed_and_its_device_revoked() {
    run_local(async {
        // The runtime refusing the hold: the device goes with the turn.
        let (host, entry) = world();
        let holds = hold_hook(&host, false);
        let closes = Arc::new(Mutex::new(Vec::new()));
        let turn = turn_row(json!({ "claudeSessionId": "agent-1" }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, keeping(&closes, None), turn, |t| {
            t.keep = Some(kept_launch());
            t.turn_token = Some("tok-1".into());
            t.scope_device_id = Some("dev-1".into());
        });
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(holds.borrow().len(), 0, "the runtime refused the hold");
        assert_eq!(host.hooks.called("hold_kept"), vec!["hold_kept(t1)"]);
        assert_eq!(*closes.lock().unwrap(), vec![false], "the agent handed back is closed");
        assert_eq!(host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)"]);

        // A turn that cannot be kept: ended here, no token, or no agent session. The agent is closed, never offered.
        for variant in ["ended", "no token", "no session"] {
            let (host, entry) = world();
            let holds = hold_hook(&host, true);
            let closes = Arc::new(Mutex::new(Vec::new()));
            let turn = turn_row(if variant == "no session" { json!({}) } else { json!({ "claudeSessionId": "agent-1" }) });
            launching(&host, &turn);
            let r = rig_on(&host, &entry, keeping(&closes, None), turn, |t| {
                t.keep = Some(kept_launch());
                t.scope_device_id = Some("dev-1".into());
                if variant != "no token" {
                    t.turn_token = Some("tok-1".into());
                }
            });
            if variant == "ended" {
                r.end("cut", false, false);
            }
            finish(&r, json!({ "status": "completed" }));
            settle().await;
            assert_eq!(holds.borrow().len(), 0, "{variant}");
            assert_eq!(*closes.lock().unwrap(), vec![false], "{variant}: the agent is closed once");
            assert_eq!(host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)"], "{variant}");
        }
    });
}

// N21, N22
#[test]
fn a_turn_launched_with_no_keep_leaves_the_agent_alone_and_revokes_its_device() {
    run_local(async {
        let (host, entry) = world();
        let holds = hold_hook(&host, true);
        let closes = Arc::new(Mutex::new(Vec::new()));
        let turn = turn_row(json!({ "claudeSessionId": "agent-1" }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, keeping(&closes, None), turn, |t| {
            t.turn_token = Some("tok-1".into());
            t.scope_device_id = Some("dev-1".into());
        });
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(holds.borrow().len(), 0);
        assert_eq!(*closes.lock().unwrap(), Vec::<bool>::new());
        assert!(r.session.kept.lock().unwrap().is_some(), "the session was never asked for its agent");
        assert_eq!(host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)"]);
    });
}

// N22: turns.ts:687-691
#[test]
fn a_device_that_will_not_go_is_warned() {
    run_local(async {
        let lines = console::capture();
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "TurnsCtx::revoke_device",
            Rc::new(|_d: &str| async { Err(failed("the door is shut")) }.boxed_local()),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| t.scope_device_id = Some("dev-1".into()));
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(lines.lines(), vec![format!("the token of thread {} was not taken away: the door is shut", thread_word(THREAD))]);
    });
}

// N20: turns.ts:672
#[test]
fn the_turns_token_comes_off_its_row_at_the_exit_and_a_later_ones_stays() {
    run_local(async {
        let r = rig_with(FakeSession::new(SESS), |t| t.turn_token = Some("tok-1".into()));
        assert_eq!(r.token().as_deref(), Some("tok-1"));
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(r.token(), None);

        let r = rig_with(FakeSession::new(SESS), |t| t.turn_token = Some("tok-1".into()));
        r.host.rows.borrow()[SESS].borrow_mut().turn_token = Some("tok-2".into());
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(r.token().as_deref(), Some("tok-2"));
    });
}

// N23: turns.ts:693-707
#[test]
fn a_process_that_exits_with_no_result_reads_its_status_words() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        r.send(json!({ "type": "turn.delta", "sessionId": SESS, "kind": "tool_use", "text": "{}", "toolName": "Bash", "toolUseId": "toolu_1" }));
        let ask: PermissionAsk = serde_json::from_value(ask_doc("ask_1")).unwrap();
        host.lead_asks.borrow_mut().insert(THREAD.to_string(), ask);
        assert_eq!(r.calls_open(), 1);
        settle().await;
        let mark = host.hooks.calls().len();
        host.clock.advance(9_000);
        finish(&r, json!({ "status": "failed" }));
        settle().await;
        let lines = turn_lines(&result(json!({ "status": "failed" })), None);
        let mut expected = doc(SESS, json!({ "status": "failed", "endedAt": T0 + 9_000 }));
        for (k, v) in serde_json::to_value(&lines).unwrap().as_object().unwrap() {
            expected[k] = v.clone();
        }
        assert_eq!(json_of(&turn), expected);
        assert!(lines.failure.is_some());
        assert_eq!(r.calls_open(), 0, "its own calls follow nobody now");
        assert!(host.lead_asks.borrow().get(THREAD).is_none(), "nobody waits behind it any more");
        let calls = after(&host, mark);
        assert_eq!(calls.iter().filter(|c| *c == "end_snooze_for").count(), 1, "a failure ends the snooze");
        assert_eq!(calls.iter().filter(|c| *c == "port_roots_moved(w1)").count(), 1);
        assert_eq!(calls.iter().filter(|c| c.starts_with("persist_sessions")).count(), 1);
    });
}

// N23
#[test]
fn only_a_failed_exit_ends_a_snooze_and_every_status_is_the_views() {
    run_local(async {
        for status in ["completed", "interrupted", "failed"] {
            let r = rig();
            finish(&r, json!({ "status": status }));
            settle().await;
            assert_eq!(json_of(&r.turn)["status"], status);
            assert_eq!(r.host.hooks.called("end_snooze_for").len(), usize::from(status == "failed"), "{status}");
        }
    });
}

// N23
#[test]
fn a_reply_given_before_the_exit_keeps_its_lines_at_the_settle() {
    run_local(async {
        let r = rig();
        let reply = json!({ "status": "completed", "text": "Fixed it.\nAll files written." });
        r.send(done_event(reply.clone()));
        let before = json_of(&r.turn);
        finish(&r, reply.clone());
        settle().await;
        let lines = turn_lines(&result(reply), None);
        assert!(lines.last_line.is_some());
        let mut expected = before;
        expected["status"] = json!("completed");
        expected["endedAt"] = json!(T0);
        assert_eq!(json_of(&r.turn), expected, "the lines the reply wrote stay as they were");
        assert_eq!(expected["lastLine"], json!(lines.last_line.unwrap()));
    });
}

// N23
#[test]
fn an_end_time_already_set_is_not_set_again() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({ "endedAt": T0 + 1 }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        host.clock.advance(9_000);
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(json_of(&turn)["endedAt"], json!(T0 + 1));
    });
}

// N23: permission-relay.test.ts:691
#[test]
fn a_pick_landed_on_the_record_rides_the_ended_row() {
    run_local(async {
        let (host, entry) = world();
        host.add_record(THREAD, W, thread_record(json!({ "harness": "claude", "permissionMode": "dontAsk" })));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        assert_eq!(turn.borrow().view.permission_mode.as_deref(), Some("bypassPermissions"));
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(turn.borrow().view.permission_mode.as_deref(), Some("dontAsk"));

        let (host, entry) = world();
        host.add_record(THREAD, W, thread_record(json!({ "harness": "claude" })));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(turn.borrow().view.permission_mode.as_deref(), Some("bypassPermissions"), "a record with no mode changes nothing");
    });
}

fn chain() -> Vec<&'static str> {
    vec!["refresh_title(true)", "read_checkout(true)", "ran_on_branch(t1)", "read_pull_request(true)", "read_lead_of"]
}

const CHAIN: &[&str] =
    &["refresh_title", "read_checkout", "ran_on_branch", "read_pull_request", "read_lead_of", "pull_request_state", "settle_tree"];

// N24: turns.ts:709-723
#[test]
fn a_turns_own_end_reads_the_branch_then_the_pull_request_then_the_lead() {
    run_local(async {
        let r = rig();
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        let mut expected = chain();
        expected.push("pull_request_state");
        assert_eq!(only(&r.host, CHAIN), expected);
    });
}

// N24
#[test]
fn the_root_workspace_settles_again_when_its_pull_request_is_not_open() {
    run_local(async {
        for (state, root, settles) in [
            (Some("merged"), None, vec!["w1"]),
            (Some("open"), None, vec![]),
            (None, None, vec![]),
            (Some("closed"), Some("root-1"), vec!["w2"]),
        ] {
            let (host, entry) = world();
            host.add_entry(entry_for("w2"));
            host.add_row("root-sess", row(view("root-sess", "w2", json!({ "threadId": "root-1", "startedAt": T0 })), "turn-r"));
            host.hooks.set::<dyn Fn(&Entry) -> Option<String>>(
                "TurnsCtx::pull_request_state",
                Rc::new(
                    move |e: &Entry| if root.is_some() && id_of(e) == "w1" { Some("open".to_string()) } else { state.map(String::from) },
                ),
            );
            let settled: Rc<RefCell<Vec<String>>> = Rc::default();
            let s = settled.clone();
            host.hooks.set::<dyn Fn(&Entry) -> LocalBoxFuture<'static, Result<(), HostError>>>(
                "TurnsCtx::settle_tree",
                Rc::new(move |e: &Entry| {
                    s.borrow_mut().push(id_of(e));
                    async { Ok(()) }.boxed_local()
                }),
            );
            let extra = match root {
                Some(id) => json!({ "rootThreadId": id }),
                None => json!({}),
            };
            let turn = turn_row(extra);
            launching(&host, &turn);
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
            finish(&r, json!({ "status": "completed" }));
            settle().await;
            assert_eq!(*settled.borrow(), settles.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "pr {state:?}, root {root:?}");
        }
    });
}

fn entry_for(id: &str) -> Entry {
    entry(id, json!({}))
}

fn id_of(e: &Entry) -> String {
    e.borrow().record.borrow().id.clone()
}

// N24: turns.ts:716-722
#[test]
fn a_failed_branch_read_is_warned_and_the_rest_of_the_chain_stops() {
    run_local(async {
        let lines = console::capture();
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry, bool) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "TurnsCtx::read_checkout",
            Rc::new(|_e: &Entry, _f: bool| async { Err(failed("git is gone")) }.boxed_local()),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(
            only(&host, CHAIN),
            vec!["refresh_title(true)", "read_checkout(true)", "pull_request_state"],
            "the settle-again check still runs"
        );
        assert_eq!(lines.lines(), vec!["the branch of w1 was not read at a turn's end: git is gone".to_string()]);
    });
}

// N24: turns.ts:709
#[test]
fn a_turn_ended_here_reads_no_title_and_no_branch() {
    run_local(async {
        let r = rig();
        r.end("cut", false, false);
        finish(&r, json!({ "status": "completed", "text": "ok" }));
        settle().await;
        assert_eq!(only(&r.host, CHAIN), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("take_review"), Vec::<String>::new(), "a turn ended here takes no review");
        assert_eq!(json_of(&r.turn)["status"], "running", "the status is the cut's to set");
        assert_eq!(json_of(&r.turn)["endedAt"], json!(T0), "the end time is set all the same");
    });
}

// N25: turns.ts:724
#[test]
fn the_images_of_a_turn_are_dropped_however_it_ended() {
    run_local(async {
        for how in ["completed", "failed", "ended here"] {
            let r = rig_with(FakeSession::new(SESS), |t| t.images_dir = Some("/tmp/img-1".into()));
            if how == "ended here" {
                r.end("cut", false, false);
            }
            finish(&r, json!({ "status": if how == "failed" { "failed" } else { "completed" } }));
            settle().await;
            assert_eq!(r.host.hooks.called("drop_images"), vec!["drop_images(/tmp/img-1)"], "{how}");
        }
        let r = rig();
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(r.host.hooks.called("drop_images"), Vec::<String>::new());
    });
}

// N26: turns.ts:731-737
#[test]
fn a_completed_turn_takes_its_review() {
    run_local(async {
        let r = rig();
        finish(&r, json!({ "status": "completed", "text": "All done." }));
        settle().await;
        assert_eq!(only(&r.host, &["send_back", "take_review"]), vec!["send_back(false)", "take_review(t1, All done.)"]);

        let r = rig();
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(r.host.hooks.called("take_review"), vec!["take_review(t1, )"], "no text reads as empty");

        for status in ["failed", "interrupted"] {
            let r = rig();
            finish(&r, json!({ "status": status, "text": "x" }));
            settle().await;
            assert_eq!(r.host.hooks.called("take_review"), Vec::<String>::new(), "{status}");
        }
    });
}

// N26
#[test]
fn a_review_that_does_not_read_is_warned() {
    run_local(async {
        let lines = console::capture();
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry, &str, &str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "TurnsCtx::take_review",
            Rc::new(|_e: &Entry, _t: &str, _x: &str| async { Err(failed("the host went")) }.boxed_local()),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        finish(&r, json!({ "status": "completed", "text": "ok" }));
        settle().await;
        assert_eq!(lines.lines(), vec!["the review in w1 was not read: the host went".to_string()]);
    });
}

// N26: turns.ts:738-741
#[test]
fn a_failed_finish_settles_failed() {
    run_local(async {
        let r = rig();
        r.session.fail();
        settle().await;
        assert_eq!(json_of(&r.turn)["status"], "failed");
        assert_eq!(only(&r.host, &["end_snooze_for", "send_back", "take_review"]), vec!["end_snooze_for", "send_back(false)"]);
    });
}

// N26: turns.ts:730
#[test]
fn send_back_carries_the_unread_ids_and_whether_the_turn_was_stopped_and_only_the_turns_own_row() {
    run_local(async {
        let (host, entry) = world();
        let sent: Rc<RefCell<Vec<(Row, Vec<String>, bool)>>> = Rc::default();
        let s = sent.clone();
        host.hooks.set::<dyn Fn(&Row, Vec<String>, bool) -> ()>(
            "TurnsCtx::send_back",
            Rc::new(move |row: &Row, unread: Vec<String>, stopped: bool| s.borrow_mut().push((row.clone(), unread, stopped))),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let asked = TurnAsked { prompt: "x".into(), effort: None, typed: None, request_id: None, owed: None, awaits_prompt: None };
        turn.borrow_mut().asked = Some(asked.clone());
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| {
            t.notify = Some(vec!["thr_lead".into()]);
        });
        r.send(json!({ "type": "turn.unread", "sessionId": SESS, "ids": ["m1", "m2"] }));
        r.handle.interrupt().await;
        finish(&r, json!({ "status": "interrupted" }));
        settle().await;
        let sent = sent.borrow();
        assert_eq!(sent.len(), 1);
        let (row, unread, stopped) = &sent[0];
        assert_eq!((unread.clone(), *stopped), (vec!["m1".to_string(), "m2".to_string()], true));
        assert!(Rc::ptr_eq(row, &turn));
        assert!(Rc::ptr_eq(row, &r.row()));
        assert_eq!(row.borrow().turn_id, TURN);
        assert!(row.borrow().turn_live.is_some());
        assert_eq!(
            (row.borrow().notify.clone(), row.borrow().asked.clone()),
            (Some(vec!["thr_lead".to_string()]), Some(asked)),
            "the row is the turn's whole row"
        );
    });
}
