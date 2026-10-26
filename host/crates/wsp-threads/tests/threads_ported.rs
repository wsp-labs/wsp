// SPDX-License-Identifier: AGPL-3.0-only
//! The threads area's tests ported from the TS suite (packages/runtime/test), at the area's edge: the store rows, the
//! events `record` was handed, the calls out in order, the warnings word for word. The cases the spec tables name are
//! in threads_{tree,kept,cap,owed,turn_end}.rs; these are the strong TS cases those files do not already cover.
//! Expected values read off threads.ts at 65f6b89c7 and the TS case named above each test.
#[path = "support_b/mod.rs"]
mod support;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::atomic::Ordering;

use futures::future::{FutureExt, LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::{json, Value};
use support::threads_ctx::{entry, held_project, FakeThreadsCtx};
use support::Lend;
use support::{local, record, row, rows_of, settle, silent, view, FakeKeptAgent};
use wsp_adapters::port::{SessionFile, SessionRenameWrite};
use wsp_core::context::Joined;
use wsp_core::types::internal::{CallSeen, KeptLaunch, Steered, Taken, TakenOutcome, TurnAsked, TurnLive, WarmKeep};
use wsp_core::types::wiring::{SessionHandle, SteerTaken};
use wsp_threads::threads::threads::cap::CapAt;
use wsp_threads::threads::threads::kept::{AGENTS_KEPT, AGENT_KEEP_MS, AGENT_WARM_MS};
use wsp_threads::threads::threads::tree::{CallsOf, TellAs};
use wsp_threads::threads::threads::turn_end::{SteerSent, TopRead, TurnChangesAsk};
use wsp_threads::threads::threads::{Entry, KeptHand, Owed, PersonRow, Row, ThreadsArea};
use wsp_wire::views::session::{SessionView, ThreadCapWait, ThreadPrompt};
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::session_events_more::SessionUncappedEventWhy;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::words::turn::{notify_line, unread_line, NotifyLength};

const FROM: &str = "child0001";
const PARENT: &str = "parent0001";
const HOUR: i64 = 3_600_000;
const NO_SUCH_TURN: &str =
    "no turn on this host carries that token; only wsp running inside a turn has one, and a turn that ended has none";

// ---- shared builders ------------------------------------------------------------------------------------------------

/// A handle a start answered, its finish settled through the sender (copied from threads_owed.rs).
struct FakeHandle {
    id: String,
    turn: String,
    outcome: SessionStartOutcome,
    finished: Shared<LocalBoxFuture<'static, TurnResult>>,
    tells_unread: Option<bool>,
}

type Finish = Rc<RefCell<Option<futures::channel::oneshot::Sender<TurnResult>>>>;

fn handle(id: &str, turn: &str, outcome: SessionStartOutcome, tells_unread: Option<bool>) -> (Rc<dyn SessionHandle>, Finish) {
    let (tx, rx) = futures::channel::oneshot::channel::<TurnResult>();
    let finished = rx.map(|r| r.unwrap_or_else(|_| result("failed", None))).boxed_local().shared();
    (Rc::new(FakeHandle { id: id.into(), turn: turn.into(), outcome, finished, tells_unread }), Rc::new(RefCell::new(Some(tx))))
}

impl SessionHandle for FakeHandle {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn workspace_id(&self) -> String {
        "ws1".into()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        self.finished.clone()
    }
    fn turn_id(&self) -> String {
        self.turn.clone()
    }
    fn outcome(&self) -> SessionStartOutcome {
        self.outcome.clone()
    }
    fn notice(&self) -> Option<String> {
        None
    }
    fn view(&self) -> SessionView {
        view(json!({ "id": self.id }))
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(
        &self,
        _prompt: String,
        _id: Option<String>,
        _images: Vec<wsp_adapters::port::TurnImage>,
    ) -> Option<wsp_core::types::wiring::Local<'static, SteerTaken>> {
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        self.tells_unread
    }
    fn answer(
        &self,
        _ask: String,
        _option: String,
        _reason: Option<String>,
    ) -> Option<wsp_core::types::wiring::Local<'static, wsp_wire::wire::runtime::SessionAnswerResult>> {
        None
    }
    fn set_access(&self, _mode: String) -> Option<wsp_core::types::wiring::Local<'static, wsp_core::types::wiring::AccessMove>> {
        None
    }
    fn stop_task(&self, _task: String) -> Option<wsp_core::types::wiring::Local<'static, wsp_adapters::port::TaskStop>> {
        None
    }
}

fn result(status: &str, error: Option<&str>) -> TurnResult {
    let mut v = json!({ "status": status });
    if let Some(e) = error {
        v["error"] = json!(e);
    }
    serde_json::from_value(v).expect("a TurnResult")
}

fn result_of(v: Value) -> TurnResult {
    serde_json::from_value(v).expect("a TurnResult")
}

fn finish(tx: &Finish, r: TurnResult) {
    let _ = tx.borrow_mut().take().expect("finished once").send(r);
}

fn scope_of(thread: &str, workspace: &str, root: &str) -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": root }))
        .expect("a scope")
}

fn origin(word: &str) -> WorkspaceOrigin {
    serde_json::from_value(json!(word)).expect("an origin")
}

fn scoped(thread: &str, workspace: &str, root: &str) -> Caller {
    Caller::Scoped { origin: origin("relayed"), by: scope_of(thread, workspace, root) }
}

/// A host whose parent thread has a settled row with an agent session on a running workspace `ws1`.
fn owed_host() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "phase": "running" }));
    ctx.add_entry(e);
    ctx.sessions.with(|s| {
        s.insert(
            "r-p".into(),
            row(view(json!({ "id": "r-p", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "completed", "startedAt": 1 })), "u-p"),
        );
    });
    let area = ctx.area();
    (ctx, area)
}

fn person_row() -> PersonRow {
    PersonRow {
        workspace_id: "ws-c".into(),
        session_id: "cs-c".into(),
        turn_id: "u-c".into(),
        text: "thread child000 finished (completed)".into(),
    }
}

fn line(id: &str) -> Owed {
    Owed {
        id: id.into(),
        from: FROM.into(),
        notify: PARENT.into(),
        text: "thread child000 finished (completed): the report".into(),
        by: None,
        road: None,
        started_by: None,
        to_person: Some(person_row()),
        since: None,
        request_id: None,
        tries: None,
        next: None,
        images: None,
        awaits_result: None,
    }
}

fn told_person(events: &[Value]) -> Vec<Value> {
    events.iter().filter(|e| e["type"] == "session.notify" && e["notify"] == "me").cloned().collect()
}

fn set_phase(ctx: &FakeThreadsCtx, ws: &str, phase: &str) {
    let entries = ctx.entries.borrow();
    let e = entries.get(ws).expect("live").borrow();
    let moved = serde_json::from_value({
        let mut v = serde_json::to_value(&*e.record.borrow()).expect("a record");
        v["phase"] = json!(phase);
        v
    })
    .expect("a record");
    *e.record.borrow_mut() = moved;
}

fn child_row(claude: Option<&str>, told: Option<u64>) -> Row {
    let mut v = json!({ "id": "r-c", "workspaceId": "ws-c", "threadId": FROM });
    if let Some(c) = claude {
        v["claudeSessionId"] = json!(c);
    }
    let r = row(view(v), "u-c");
    r.borrow_mut().turn_live = told.map(|n| TurnLive { told: Some(n), ..Default::default() });
    r
}

fn taken_by(ctx: &FakeThreadsCtx, request: &str, row: &str, turn: &str) {
    ctx.taken.borrow_mut().insert(
        ("ws1".into(), request.into()),
        Taken { session_id: row.into(), thread_id: PARENT.into(), turn_id: turn.into(), outcome: TakenOutcome::Started },
    );
}

fn woken(ws: &str) -> Value {
    json!({ "type": "workspace.woken", "workspaceId": ws, "machineId": "m1" })
}

fn turn_row(ctx: &FakeThreadsCtx, id: &str) -> Row {
    ctx.sessions.with(|s| {
        let r = s.get(id).expect("a row").borrow();
        let copy = row(r.view.clone(), &r.turn_id);
        copy.borrow_mut().turn_live = r.turn_live.clone();
        copy
    })
}

fn steer_sent(prompt: &str) -> SteerSent {
    SteerSent { prompt: prompt.into(), request_id: Some("req-s".into()), via_slate: false, started_by: None, attachments: vec![] }
}

fn steered_on(ctx: &FakeThreadsCtx) -> Vec<String> {
    ctx.sessions.with(|s| {
        let steered: Vec<String> = s
            .get("r1")
            .map(|r| {
                r.borrow().turn_live.as_ref().and_then(|l| l.steered.as_ref()).map(|m| m.keys().cloned().collect()).unwrap_or_default()
            })
            .unwrap_or_default();
        steered
    })
}

fn unread_row(steered: Value) -> Row {
    let steered: Steered = serde_json::from_value(steered).expect("a steer");
    let r = row(view(json!({ "id": "r-t", "workspaceId": "ws1", "threadId": PARENT, "claudeSessionId": "cs-p" })), "u-t");
    r.borrow_mut().turn_live = Some(TurnLive { steered: Some([("st1".to_string(), steered)].into_iter().collect()), ..Default::default() });
    r
}

// =====================================================================================================================
// owed-lines.test.ts: a child's finished line its lead cannot take yet
// =====================================================================================================================

// TS: owed-lines.test.ts:214
#[test]
fn a_try_whose_launch_failed_silent_goes_again_under_the_same_request_and_the_next_try_takes_the_line() {
    local(async {
        let (ctx, area) = owed_host();
        let (h1, tx1) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        let (h2, _tx2) = handle("r-try2", "u-try2", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h1));
        ctx.starts.borrow_mut().push_back(Ok(h2));
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-try", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "failed", "startedAt": 9 })),
                "u-try",
            );
            r.borrow_mut().turn_live =
                Some(TurnLive { failure: Some(silent("the machine did not answer", "LaunchUnanswered")), ..Default::default() });
            s.insert("r-try".into(), r);
        });
        area.deliver(line("L1"), false);
        settle().await;
        let warned = wsp_core::console::capture();
        finish(&tx1, result("failed", Some("the launch went unanswered")));
        settle().await;
        assert_eq!(
            warned.lines(),
            vec![
                "thread child000 ended, but its line did not reach thread parent00 yet, and goes again in 30 s: the launch went unanswered"
            ]
        );
        assert_eq!(ctx.started.borrow().len(), 1, "the next try waits its thirty seconds");
        ctx.clock.advance(30_000);
        settle().await;
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 2);
            assert_eq!(started[0].1.request_id.as_deref(), Some("uuid-1"));
            assert_eq!(started[1].1.request_id.as_deref(), Some("uuid-1"), "every try goes under the one request id");
        }
        taken_by(&ctx, "uuid-1", "r-try2", "u-try2");
        area.prompt_held("uuid-1");
        settle().await;
        assert_eq!(
            ctx.store.logged(),
            vec!["put notify-owed/L1", "put notify-owed/L1", "delete notify-owed/L1"],
            "kept before the first try, again with its spacing, and gone once taken"
        );
        assert!(ctx.called().contains(&format!("mark {FROM} 1030000")));
        assert!(told_person(&ctx.recorded_json()).is_empty());
        assert!(area.sending.borrow().is_empty());
    });
}

// TS: owed-lines.test.ts:233
#[test]
fn a_try_that_fails_after_the_leads_agent_announced_itself_is_not_sent_again() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver(line("L1"), false);
        settle().await;
        let warned = wsp_core::console::capture();
        taken_by(&ctx, "uuid-1", "r-try", "u-try");
        finish(&tx, result("failed", Some("the launch was lost")));
        settle().await;
        ctx.clock.advance(2 * HOUR);
        settle().await;
        assert!(warned.lines().is_empty(), "the agent had the line, so nothing failed");
        assert_eq!(ctx.started.borrow().len(), 1);
        assert_eq!(ctx.store.logged(), vec!["put notify-owed/L1", "delete notify-owed/L1"]);
        assert!(told_person(&ctx.recorded_json()).is_empty());
        assert!(ctx.called().contains(&format!("mark {FROM} 1000000")), "a lead that took a child's line has read the child");
    });
}

// TS: owed-lines.test.ts:268
#[test]
fn a_try_whose_agent_had_not_announced_itself_at_the_restart_is_taken_when_it_does() {
    local(async {
        let (ctx, area) = owed_host();
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-dark", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                "u-dark",
            );
            r.borrow_mut().asked = Some(TurnAsked {
                prompt: "x".into(),
                effort: None,
                typed: None,
                request_id: Some("req-d".into()),
                owed: Some(true),
                awaits_prompt: None,
            });
            s.insert("r-dark".into(), r);
        });
        let mut l = line("L1");
        l.since = Some(1_000_000 - 60_000);
        l.request_id = Some("req-d".into());
        let warned = wsp_core::console::capture();
        area.deliver(l, false);
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "the try out is read, not sent again");
        assert!(ctx.store.logged().is_empty());
        taken_by(&ctx, "req-d", "r-dark", "u-dark");
        area.prompt_held("req-d");
        settle().await;
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
        ctx.clock.advance(2 * HOUR);
        settle().await;
        assert!(!ctx.called().iter().any(|c| c.starts_with("stopTry")), "nothing reaches a dark turn to stop it");
        assert!(ctx.started.borrow().is_empty());
        assert!(warned.lines().is_empty());
        assert!(told_person(&ctx.recorded_json()).is_empty());
        assert!(ctx.called().contains(&format!("mark {FROM} 1000000")));
    });
}

// TS: owed-lines.test.ts:286
#[test]
fn a_line_still_owed_at_the_restart_is_sent_again_under_its_request_by_the_next_host() {
    local(async {
        let (ctx, area) = owed_host();
        let owed = Owed {
            since: Some(1_000_000 - 300_000),
            request_id: Some("req-k".into()),
            tries: Some(1),
            next: Some(1_000_000 - 1_000),
            ..line("L1")
        };
        ctx.store.seed("notify-owed", "L1", serde_json::to_value(&owed).expect("a line"));
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver_owed().await.expect("listed");
        settle().await;
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1, "its next time is past, so it goes now");
            assert_eq!(started[0].1.request_id.as_deref(), Some("req-k"));
            assert_eq!(started[0].1.prompt, "thread child000 finished (completed): the report");
        }
        assert!(ctx.store.logged().is_empty(), "a line with its request and its start is not written again");
        taken_by(&ctx, "req-k", "r-try", "u-try");
        area.prompt_held("req-k");
        settle().await;
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
        assert!(area.sending.borrow().is_empty());
        assert!(told_person(&ctx.recorded_json()).is_empty());
    });
}

// TS: owed-lines.test.ts:367, 389
#[test]
fn a_steer_whose_write_outlasts_the_leads_turn_goes_as_its_next_turn_or_tells_the_person_once() {
    for stopped in [false, true] {
        local(async move {
            let (ctx, area) = owed_host();
            ctx.sessions.with(|s| {
                let r = row(view(json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "cs-1", "status": "running" })), "u1");
                r.borrow_mut().turn_live = Some(TurnLive::default());
                s.insert("r1".into(), r);
            });
            area.keep_steer("r1", &steer_sent("also this"), None, "st1").await;
            // The turn ends with the message unread while its write is still out: the start road owns it.
            let ended = turn_row(&ctx, "r1");
            area.send_back(&ended, &["st1".to_string()], stopped);
            settle().await;
            assert!(ctx.started.borrow().is_empty(), "the turn's end does not send a message whose write is out");
            assert!(ctx.recorded.borrow().is_empty());
            assert!(ctx.store.logged().is_empty());
            let landed = area.steer_answered("r1", "h1", &steer_sent("also this"), None, "st1", true);
            settle().await;
            assert!(!area.steers_out.borrow().contains_key("st1"));
            if stopped {
                assert!(landed, "the person is told, so the caller hears it landed");
                let told = ctx.recorded_json();
                assert_eq!(told.len(), 1);
                assert_eq!(
                    (told[0]["type"].clone(), told[0]["notify"].clone(), told[0]["text"].clone()),
                    (json!("session.notify"), json!("me"), json!("the agent never read this message: also this"))
                );
            } else {
                assert!(!landed, "the start road sends it as the thread's next turn");
                assert!(steered_on(&ctx).is_empty(), "the turn keeps no message it will not read");
                assert!(ctx.recorded.borrow().is_empty(), "and nobody is told it went wrong");
            }
            assert!(ctx.started.borrow().is_empty());
        });
    }
}

// TS: owed-lines.test.ts:442
#[test]
fn a_try_whose_row_ends_is_read_off_that_end_as_stopped_or_failed() {
    for (status, reason, warning) in [
        ("interrupted", "stopped", "thread child000 ended, but the turn carrying its line into thread parent00 was stopped"),
        ("failed", "the stream broke", "thread child000 ended, but its line did not reach thread parent00: the stream broke"),
    ] {
        local(async move {
            let (ctx, area) = owed_host();
            let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
            ctx.sessions.with(|s| {
                s.insert("r-try".into(), row(view(json!({ "id": "r-try", "status": status })), "u-try"));
            });
            area.deliver(line("L1"), false);
            settle().await;
            let warned = wsp_core::console::capture();
            ctx.bus.emit_json(json!({ "type": "session.end", "workspaceId": "ws1", "sessionId": "cs", "turnId": "u-try", "exitCode": null, "sawResult": false, "reason": reason }));
            settle().await;
            assert_eq!(warned.lines(), vec![warning]);
            assert!(ctx.store.get_now("notify-owed", "L1").is_none());
            assert_eq!(told_person(&ctx.recorded_json()).len(), 1, "a line that fell tells the person once");
            assert_eq!(ctx.started.borrow().len(), 1);
        });
    }
}

// TS: owed-lines.test.ts:458
#[test]
fn a_line_the_leads_computer_keeps_failing_tells_the_person_once_and_the_lead_nothing() {
    local(async {
        let (ctx, area) = owed_host();
        for _ in 0..12 {
            ctx.starts.borrow_mut().push_back(Err(silent("the computer did not answer", "MachineUnreached")));
        }
        area.deliver(line("L1"), false);
        settle().await;
        for w in [30, 60, 120, 240, 480, 600, 600, 600, 600, 270] {
            ctx.clock.advance(w * 1000);
            settle().await;
        }
        assert!(ctx.started.borrow().len() > 5, "the line was tried many times");
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1, "no try tells the lead anything: the person gets the one row, at the hour");
        assert_eq!(said[0]["type"], "session.notify");
        assert_eq!(said[0]["notify"], "me");
        assert_eq!(
            (said[0]["workspaceId"].clone(), said[0]["sessionId"].clone(), said[0]["turnId"].clone(), said[0]["threadId"].clone()),
            (json!("ws-c"), json!("cs-c"), json!("u-c"), json!(FROM))
        );
        assert_eq!(said[0]["text"], "thread child000 finished (completed)");
        assert!(ctx.store.get_now("notify-owed", "L1").is_none());
    });
}

// TS: owed-lines.test.ts:591
#[test]
fn the_persons_stop_of_a_turn_nothing_reaches_settles_its_line_without_stopping_the_turn() {
    local(async {
        let (ctx, area) = owed_host();
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-dark", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                "u-dark",
            );
            r.borrow_mut().asked = Some(TurnAsked {
                prompt: "x".into(),
                effort: None,
                typed: None,
                request_id: Some("req-d".into()),
                owed: Some(true),
                awaits_prompt: None,
            });
            s.insert("r-dark".into(), r);
        });
        let mut l = line("L1");
        l.since = Some(1_000_000 - 60_000);
        l.request_id = Some("req-d".into());
        area.deliver(l, false);
        settle().await;
        let warned = wsp_core::console::capture();
        area.try_stopped("u-dark");
        settle().await;
        assert_eq!(warned.lines(), vec!["thread child000 ended, but the turn carrying its line into thread parent00 was stopped"]);
        assert!(!ctx.called().iter().any(|c| c.starts_with("stopTry")), "nothing reaches the turn itself");
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
        assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
        ctx.clock.advance(2 * HOUR);
        settle().await;
        assert!(ctx.started.borrow().is_empty());
        assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
    });
}

// TS: owed-lines.test.ts:661
#[test]
fn a_steered_line_whose_turn_ended_with_no_word_on_unread_messages_falls_to_the_person() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, tx) = handle("r-run", "u-run", SessionStartOutcome::Started, None);
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-run", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                "u-run",
            );
            let steered: Steered =
                serde_json::from_value(json!({ "prompt": "x", "startedBy": "agent", "requestId": "req-s" })).expect("a steer");
            r.borrow_mut().turn_live =
                Some(TurnLive { steered: Some([("st1".to_string(), steered)].into_iter().collect()), ..Default::default() });
            r.borrow_mut().handle = Some(h);
            s.insert("r-run".into(), r);
        });
        finish(&tx, result("completed", None));
        let warned = wsp_core::console::capture();
        let mut l = line("L1");
        l.since = Some(1_000_000);
        l.request_id = Some("req-s".into());
        area.deliver(l, false);
        settle().await;
        assert_eq!(warned.lines(), vec!["thread child000 ended, and nothing says whether thread parent00 read its line"]);
        assert!(ctx.started.borrow().is_empty());
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
        assert_eq!(told_person(&ctx.recorded_json()).len(), 1, "told, as a client with no unread report was, and never lost");
    });
}

// TS: owed-lines.test.ts:531
#[test]
fn a_steered_line_on_a_turn_no_handle_reaches_is_asked_again_every_thirty_seconds_until_the_turn_is_over() {
    local(async {
        let (ctx, area) = owed_host();
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-run", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                "u-run",
            );
            let steered: Steered =
                serde_json::from_value(json!({ "prompt": "x", "startedBy": "agent", "requestId": "req-s" })).expect("a steer");
            r.borrow_mut().turn_live =
                Some(TurnLive { steered: Some([("st1".to_string(), steered)].into_iter().collect()), ..Default::default() });
            s.insert("r-run".into(), r);
        });
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let warned = wsp_core::console::capture();
        let mut l = line("L1");
        l.since = Some(1_000_000);
        l.request_id = Some("req-s".into());
        area.deliver(l, false);
        settle().await;
        assert!(ctx.started.borrow().is_empty());
        ctx.clock.advance(29_999);
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "the wait on a turn nothing answers for is thirty seconds on the clock");
        ctx.clock.advance(1);
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "the turn still runs, so the line asks again");
        assert!(area.sending.borrow().contains_key("L1"));
        ctx.sessions.with(|s| s.shift_remove("r-run"));
        ctx.clock.advance(30_000);
        settle().await;
        let started = ctx.started.borrow();
        assert_eq!(started.len(), 1, "once the turn is over the line goes under its request");
        assert_eq!(started[0].1.request_id.as_deref(), Some("req-s"));
        assert!(warned.lines().is_empty());
        assert!(ctx.store.logged().is_empty(), "a line that had its request and start is never written for waiting");
    });
}

// TS: owed-lines.test.ts:683
#[test]
fn a_line_whose_try_failed_and_whose_lead_napped_past_its_hour_is_tried_at_the_wake() {
    local(async {
        let (ctx, area) = owed_host();
        set_phase(&ctx, "ws1", "napping");
        let warned = wsp_core::console::capture();
        let mut l = line("L1");
        l.since = Some(1_000_000 - 2 * HOUR);
        l.request_id = Some("req-n".into());
        l.tries = Some(1);
        l.next = Some(1_000_000 - HOUR);
        area.deliver(l, false);
        settle().await;
        assert_eq!(area.held_lines.borrow().get("ws1").map(|l| l.len()), Some(1), "held for the wake, past its hour or not");
        assert!(ctx.started.borrow().is_empty());
        assert!(warned.lines().is_empty(), "the hour bounds tries while the computer answers: nothing gives up");
        set_phase(&ctx, "ws1", "running");
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        ctx.bus.emit_json(woken("ws1"));
        settle().await;
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1, "the wake gets a try");
            assert_eq!(started[0].1.request_id.as_deref(), Some("req-n"));
            assert!(!started[0].1.asks_until_stopped, "its hour is gone, so it does not ask until stopped");
            assert!(started[0].1.wakes_lead);
        }
        taken_by(&ctx, "req-n", "r-try", "u-try");
        area.prompt_held("req-n");
        settle().await;
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
        assert!(warned.lines().is_empty());
        assert!(told_person(&ctx.recorded_json()).is_empty());
    });
}

// TS: owed-lines.test.ts:817, 830
#[test]
fn a_try_that_ends_before_its_agent_started_leaves_the_line_to_the_person_with_the_turns_words() {
    for (error, said) in [
        (None, "the turn ended failed before its agent started"),
        (Some("the model is not available on this plan"), "the model is not available on this plan"),
    ] {
        local(async move {
            let (ctx, area) = owed_host();
            let (h, tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
            area.deliver(line("L1"), false);
            settle().await;
            let warned = wsp_core::console::capture();
            finish(&tx, result("failed", error));
            settle().await;
            ctx.clock.advance(2 * HOUR);
            settle().await;
            assert_eq!(warned.lines(), vec![format!("thread child000 ended, but its line did not reach thread parent00: {said}")]);
            assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
            assert_eq!(ctx.started.borrow().len(), 1, "never tried again");
            assert!(ctx.store.get_now("notify-owed", "L1").is_none());
        });
    }
}

// TS: owed-lines.test.ts:1044
#[test]
fn a_next_host_reads_the_try_only_once_its_reopen_answered_whether_it_comes_up_within_the_hour_or_past_it() {
    for after_minutes in [61_i64, 30] {
        local(async move {
            let (ctx, area) = owed_host();
            let (tx, rx) = futures::channel::oneshot::channel::<()>();
            let gate: Joined = rx.map(|_| ()).boxed_local().shared();
            ctx.boot_work.borrow_mut().insert("ws1".into(), gate);
            let (h, _htx) = handle("r-run", "u-run", SessionStartOutcome::Started, None);
            ctx.sessions.with(|s| {
                let r = row(
                    view(json!({ "id": "r-run", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                    "u-run",
                );
                r.borrow_mut().asked = Some(TurnAsked {
                    prompt: "x".into(),
                    effort: None,
                    typed: None,
                    request_id: Some("req-w".into()),
                    owed: Some(true),
                    awaits_prompt: None,
                });
                r.borrow_mut().handle = Some(h);
                s.insert("r-run".into(), r);
            });
            let warned = wsp_core::console::capture();
            let mut l = line("L1");
            l.since = Some(1_000_000 - after_minutes * 60_000);
            l.request_id = Some("req-w".into());
            area.deliver(l, false);
            settle().await;
            assert!(ctx.store.logged().is_empty(), "nothing is read before the re-open answered");
            assert!(ctx.started.borrow().is_empty());
            assert!(!ctx.called().iter().any(|c| c.starts_with("stopTry")));
            assert!(warned.lines().is_empty());
            // The agent took the line while the host was down, so the re-open finds it taken.
            taken_by(&ctx, "req-w", "r-run", "u-run");
            let _ = tx.send(());
            settle().await;
            assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"], "the lead took the line: nobody is told it failed");
            assert!(warned.lines().is_empty());
            assert!(told_person(&ctx.recorded_json()).is_empty());
            assert!(ctx.started.borrow().is_empty());
        });
    }
}

// =====================================================================================================================
// runtime-notify.test.ts: a thread whose start named who to tell
// =====================================================================================================================

// TS: runtime-notify.test.ts:10
#[test]
fn a_running_parent_is_told_by_one_steer_the_line_with_outcome_duration_cost_and_the_reply_whole() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, _tx) = handle("r-p", "u-p2", SessionStartOutcome::Steered, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let done = result_of(
            json!({ "status": "completed", "text": "Ran the gate.\n\nAll 12 tests green.\n", "durationMs": 492_000, "costUsd": 1.94 }),
        );
        area.notify_end(&child_row(Some("cs-c"), None), &[PARENT.to_string()], &TellAs::default(), &done, false);
        settle().await;
        let line = "thread child000 finished (completed, 8m 12s, $1.94): Ran the gate.\n\nAll 12 tests green.";
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        for (k, v) in [
            ("type", json!("session.notify")),
            ("workspaceId", json!("ws-c")),
            ("sessionId", json!("cs-c")),
            ("turnId", json!("u-c")),
            ("threadId", json!(FROM)),
            ("notify", json!(PARENT)),
            ("text", json!(line)),
        ] {
            assert_eq!(said[0][k], v, "{k}");
        }
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1);
            assert_eq!(started[0].1.prompt, line);
            assert_eq!((started[0].1.harness.as_deref(), started[0].1.thread.as_deref()), (Some("claude"), Some(PARENT)));
            assert!(started[0].1.wakes_lead && started[0].1.owed);
        }
        assert_eq!(ctx.store.logged(), vec![format!("put notify-owed/u-c:0:{PARENT}"), format!("delete notify-owed/u-c:0:{PARENT}")]);
        assert!(ctx.called().contains(&format!("mark {FROM} 1000000")));
    });
}

// TS: runtime-notify.test.ts:32
#[test]
fn an_idle_parent_is_told_by_a_turn_of_its_own_with_the_line_as_its_prompt() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let done = result_of(json!({ "status": "completed", "text": "done", "durationMs": 1_500, "costUsd": 0.0042 }));
        area.notify_end(&child_row(Some("cs-c"), None), &[PARENT.to_string()], &TellAs::default(), &done, false);
        settle().await;
        let line = "thread child000 finished (completed, 1.5s, $0.00): done";
        assert_eq!(ctx.started.borrow().len(), 1);
        assert_eq!(ctx.started.borrow()[0].1.prompt, line);
        let kept = ctx.store.get_now("notify-owed", &format!("u-c:0:{PARENT}")).expect("owed until the turn takes it");
        assert_eq!(kept["text"], json!(line));
        assert_eq!(kept["toPerson"]["text"], json!(line));
        taken_by(&ctx, "uuid-1", "r-n", "u-n");
        area.prompt_held("uuid-1");
        settle().await;
        assert!(ctx.store.get_now("notify-owed", &format!("u-c:0:{PARENT}")).is_none());
    });
}

// TS: runtime-notify.test.ts:57
#[test]
fn a_childs_line_taken_by_its_lead_marks_the_child_read_by_steer_or_by_a_turn_of_the_leads_own() {
    for steers in [true, false] {
        local(async move {
            let (ctx, area) = owed_host();
            let outcome = if steers { SessionStartOutcome::Steered } else { SessionStartOutcome::Started };
            let (h, _tx) = handle("r-n", "u-n", outcome, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
            area.notify_end(&child_row(Some("cs-c"), None), &[PARENT.to_string()], &TellAs::default(), &result("completed", None), false);
            ctx.clock.advance(5_000);
            settle().await;
            if !steers {
                assert!(!ctx.called().iter().any(|c| c.starts_with("mark")), "not read until a turn of the lead took the line");
                taken_by(&ctx, "uuid-1", "r-n", "u-n");
                area.prompt_held("uuid-1");
                settle().await;
            }
            let marks: Vec<String> = ctx.called().into_iter().filter(|c| c.starts_with("mark")).collect();
            assert_eq!(
                marks,
                vec![format!("mark {FROM} 1005000")],
                "the child is read as of the moment its line was taken (steers: {steers})"
            );
        });
    }
}

// TS: runtime-notify.test.ts:82, 196
#[test]
fn a_line_queued_behind_the_parents_running_process_stays_owed_and_goes_once_as_a_turn_of_its_own() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, _tx) = handle("r-q", "u-q", SessionStartOutcome::Queued, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.notify_end(
            &child_row(Some("cs-c"), None),
            &[PARENT.to_string()],
            &TellAs::default(),
            &result_of(json!({ "status": "completed", "text": "done" })),
            false,
        );
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        assert_eq!(
            (said[0]["notify"].clone(), said[0]["text"].clone()),
            (json!(PARENT), json!("thread child000 finished (completed): done"))
        );
        assert_eq!(ctx.started.borrow().len(), 1, "queued behind its process, as any send is, and never steered");
        let key = format!("u-c:0:{PARENT}");
        assert!(ctx.store.get_now("notify-owed", &key).is_some(), "owed until the queued turn takes it");
        assert_eq!(ctx.store.logged(), vec![format!("put notify-owed/{key}")]);
        taken_by(&ctx, "uuid-1", "r-q", "u-q");
        area.prompt_held("uuid-1");
        settle().await;
        assert_eq!(ctx.store.logged(), vec![format!("put notify-owed/{key}"), format!("delete notify-owed/{key}")]);
        assert_eq!(ctx.started.borrow().len(), 1);
    });
}

// TS: runtime-notify.test.ts:217
#[test]
fn failed_and_interrupted_ends_carry_their_words_and_me_starts_nothing() {
    let (ctx, area) = owed_host();
    let failing = row(view(json!({ "id": "r-1", "workspaceId": "ws-c", "threadId": "failing01", "claudeSessionId": "cs-1" })), "u1");
    let stopped = row(view(json!({ "id": "r-2", "workspaceId": "ws-c", "threadId": "stopped01", "claudeSessionId": "cs-2" })), "u2");
    area.notify_end(
        &failing,
        &["me".to_string()],
        &TellAs::default(),
        &result_of(json!({ "status": "failed", "error": "the harness died", "durationMs": 3_000 })),
        false,
    );
    area.notify_end(
        &stopped,
        &["me".to_string()],
        &TellAs::default(),
        &result_of(json!({ "status": "interrupted", "text": "Stopped mid-way.", "durationMs": 12_000, "costUsd": 0.03 })),
        false,
    );
    let said = ctx.recorded_json();
    assert_eq!(said.len(), 2);
    assert_eq!(
        (said[0]["threadId"].clone(), said[0]["notify"].clone(), said[0]["text"].clone()),
        (json!("failing01"), json!("me"), json!("thread failing0 finished (failed, 3.0s): the harness died"))
    );
    assert_eq!(
        (said[1]["threadId"].clone(), said[1]["notify"].clone(), said[1]["text"].clone()),
        (json!("stopped01"), json!("me"), json!("thread stopped0 finished (interrupted, 12s, $0.03): Stopped mid-way."))
    );
    assert!(ctx.started.borrow().is_empty());
    assert!(ctx.store.logged().is_empty(), "me is the person's row alone: nothing is kept owed");
}

// TS: runtime-notify.test.ts:296
#[test]
fn a_notify_that_leads_back_to_a_thread_shows_in_its_reach_at_any_length_of_the_chain() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        let a = row(view(json!({ "id": "ra", "threadId": "a" })), "ua");
        let b = row(view(json!({ "id": "rb", "threadId": "b" })), "ub");
        b.borrow_mut().notify = Some(vec!["a".into()]);
        let c = row(view(json!({ "id": "rc", "threadId": "c" })), "uc");
        c.borrow_mut().notify = Some(vec!["b".into()]);
        *s = rows_of(vec![a, b, c]);
    });
    let area = ctx.area();
    let reach = |from: &[&str]| area.notify_reach(&from.iter().map(|s| s.to_string()).collect::<Vec<_>>()).into_iter().collect::<Vec<_>>();
    assert_eq!(reach(&["b"]), vec!["b", "a"], "a start into a naming b would loop: b already notifies a");
    assert_eq!(reach(&["c"]), vec!["c", "b", "a"], "and so would one naming c, a chain of three");
    assert_eq!(reach(&["a"]), vec!["a"], "a chain that does not come back is fine: a fresh thread naming c is in no one's reach");
    assert!(!reach(&["c"]).contains(&"d".to_string()));
}

// TS: runtime-notify.test.ts:341
#[test]
fn a_line_held_for_a_napping_parent_outlives_a_host_restart_and_goes_when_the_workspace_wakes() {
    local(async {
        let (ctx, area) = owed_host();
        set_phase(&ctx, "ws1", "napping");
        let owed = Owed { since: Some(1_000_000 - 30_000), request_id: Some("req-h".into()), ..line("L1") };
        ctx.store.seed("notify-owed", "L1", serde_json::to_value(&owed).expect("a line"));
        area.deliver_owed().await.expect("listed");
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "the next host holds it again for the wake");
        assert_eq!(area.held_lines.borrow().get("ws1").map(|l| l.len()), Some(1));
        assert!(area.sending.borrow().contains_key("L1"));
        assert!(ctx.store.get_now("notify-owed", "L1").is_some());
        set_phase(&ctx, "ws1", "running");
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        ctx.bus.emit_json(woken("ws1"));
        settle().await;
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1);
            assert_eq!(started[0].1.request_id.as_deref(), Some("req-h"));
        }
        assert!(area.held_lines.borrow().get("ws1").is_none());
    });
}

// TS: runtime-notify.test.ts:370
#[test]
fn a_line_is_held_for_every_phase_that_cannot_take_a_start_and_sent_at_once_for_the_rest() {
    local(async {
        for (phase, held) in [
            ("pausing", true),
            ("napping", true),
            ("waking", true),
            ("gone", true),
            ("released", true),
            ("queued", false),
            ("running", false),
        ] {
            let (ctx, area) = owed_host();
            set_phase(&ctx, "ws1", phase);
            let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
            area.deliver(line("L1"), false);
            settle().await;
            assert_eq!(area.held_lines.borrow().get("ws1").map(|l| l.len()), if held { Some(1) } else { None }, "{phase}");
            assert_eq!(ctx.started.borrow().len(), usize::from(!held), "{phase}");
        }
        // A workspace that is not live is not held: the start decides.
        let (ctx, area) = owed_host();
        ctx.entries.borrow_mut().shift_remove("ws1");
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver(line("L1"), false);
        settle().await;
        assert!(area.held_lines.borrow().is_empty());
        assert_eq!(ctx.started.borrow().len(), 1);
    });
}

fn running_row(ctx: &FakeThreadsCtx, thread: &str, notify: Option<Vec<&str>>) {
    ctx.sessions.with(|s| {
        let r = row(view(json!({ "id": "r1", "workspaceId": "ws1", "threadId": thread, "status": "running" })), "u1");
        r.borrow_mut().notify = notify.map(|n| n.into_iter().map(String::from).collect());
        r.borrow_mut().turn_live = None;
        s.insert("r1".into(), r);
    });
}

// TS: runtime-notify.test.ts:395
#[test]
fn a_turn_the_runtime_ends_is_told_to_its_target_as_failed_with_the_reason() {
    let (ctx, area) = owed_host();
    running_row(&ctx, "t1", Some(vec!["me"]));
    area.settle_cut(
        "r1",
        "machine paused while the agent was working",
        &|_| "machine paused while the agent was working".into(),
        false,
        false,
    );
    let said = ctx.recorded_json();
    assert_eq!(
        said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(),
        vec![json!("session.notify"), json!("session.end")],
        "the line goes before the end, where a follower stops reading"
    );
    assert_eq!(
        (said[0]["notify"].clone(), said[0]["text"].clone()),
        (json!("me"), json!("thread t1 finished (failed): machine paused while the agent was working"))
    );
    assert_eq!(said[1]["reason"], json!("machine paused while the agent was working"));
    assert_eq!(said[1]["sawResult"], json!(false));
    assert_eq!(ctx.sessions.with(|s| support::json_of(&s.get("r1").expect("row").borrow().view.status)), json!("failed"));
}

// TS: runtime-notify.test.ts:409
#[test]
fn a_turn_a_host_restart_cut_tells_its_parent_in_another_workspace_the_same_way() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        ctx.sessions.with(|s| {
            let r = row(view(json!({ "id": "r-kid", "workspaceId": "ws-kid", "threadId": "kid00001", "claudeSessionId": "cs-kid", "status": "running" })), "u-kid");
            r.borrow_mut().notify = Some(vec![PARENT.into()]);
            s.insert("r-kid".into(), r);
        });
        area.settle_cut(
            "r-kid",
            "host restarted while the agent was working",
            &|_| "cut by a host restart after 12m 03s".into(),
            false,
            false,
        );
        settle().await;
        let line = "thread kid00001 finished (failed): cut by a host restart after 12m 03s";
        let said = ctx.recorded_json();
        assert_eq!(said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(), vec![json!("session.notify"), json!("session.end")]);
        for (k, v) in [
            ("workspaceId", json!("ws-kid")),
            ("sessionId", json!("cs-kid")),
            ("turnId", json!("u-kid")),
            ("threadId", json!("kid00001")),
            ("notify", json!(PARENT)),
            ("text", json!(line)),
        ] {
            assert_eq!(said[0][k], v, "{k}");
        }
        assert_eq!(said[1]["reason"], json!("host restarted while the agent was working"));
        let started = ctx.started.borrow();
        assert_eq!(started.len(), 1);
        assert_eq!((started[0].0.as_str(), started[0].1.prompt.as_str(), started[0].1.thread.as_deref()), ("ws1", line, Some(PARENT)));
    });
}

// TS: runtime-notify.test.ts:449
#[test]
fn a_turn_a_host_restart_cut_that_was_to_tell_me_records_the_line_in_its_thread() {
    let (ctx, area) = owed_host();
    running_row(&ctx, "t1", Some(vec!["me"]));
    area.settle_cut("r1", "host restarted while the agent was working", &|_| "cut by a host restart after 0m 45s".into(), false, false);
    let said = ctx.recorded_json();
    assert_eq!(said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(), vec![json!("session.notify"), json!("session.end")]);
    assert_eq!(
        (said[0]["turnId"].clone(), said[0]["notify"].clone(), said[0]["text"].clone()),
        (json!("u1"), json!("me"), json!("thread t1 finished (failed): cut by a host restart after 0m 45s"))
    );
    assert_eq!(said[1]["reason"], json!("host restarted while the agent was working"));
    assert!(ctx.started.borrow().is_empty());
    assert!(ctx.store.logged().is_empty());
}

// TS: runtime-notify.test.ts:474
#[test]
fn two_turns_each_carry_a_token_and_each_token_names_its_own_thread() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        let one = row(view(json!({ "id": "a", "threadId": "thread-one", "status": "running" })), "u1");
        one.borrow_mut().turn_token = Some("0123456789abcdef0123456789abcdef".into());
        let two = row(view(json!({ "id": "b", "threadId": "thread-two", "status": "running" })), "u2");
        two.borrow_mut().turn_token = Some("fedcba9876543210fedcba9876543210".into());
        *s = rows_of(vec![one, two]);
    });
    let area = ctx.area();
    assert_eq!(area.thread_of_token("0123456789abcdef0123456789abcdef").expect("the first"), "thread-one");
    assert_eq!(area.thread_of_token("fedcba9876543210fedcba9876543210").expect("the second"), "thread-two");
}

// TS: runtime-notify.test.ts:500
#[test]
fn a_turn_the_runtime_ended_rather_than_its_own_process_leaves_its_token_naming_nobody() {
    let (ctx, area) = owed_host();
    running_row(&ctx, "t1", None);
    ctx.sessions.with(|s| s.get("r1").expect("row").borrow_mut().turn_token = Some("tok-nap".into()));
    assert_eq!(area.thread_of_token("tok-nap").expect("a running turn's token names its thread"), "t1");
    area.settle_cut("r1", "machine paused while the agent was working", &|_| "machine paused".into(), false, false);
    let refused = area.thread_of_token("tok-nap").expect_err("the nap ended the turn from this side");
    assert_eq!(refused.message, NO_SUCH_TURN);
    assert_eq!(refused.kind, None);
}

// TS: runtime-notify.test.ts:542
#[test]
fn several_targets_are_each_told_once_with_the_same_whole_line() {
    local(async {
        let (ctx, area) = owed_host();
        ctx.sessions.with(|s| {
            s.insert("r-p2".into(), row(view(json!({ "id": "r-p2", "threadId": "parent0002", "claudeSessionId": "cs-p2", "status": "completed", "startedAt": 2 })), "u-p2"));
        });
        for n in 0..2 {
            let (h, _tx) = handle(&format!("r-n{n}"), &format!("u-n{n}"), SessionStartOutcome::Steered, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
        }
        let done = result_of(json!({ "status": "completed", "text": "Ran the gate.\nAll 12 tests green." }));
        area.notify_end(&child_row(Some("cs-c"), None), &[PARENT.to_string(), "parent0002".to_string()], &TellAs::default(), &done, false);
        settle().await;
        let line = notify_line(FROM, &done, Some(NotifyLength::Whole));
        let said = ctx.recorded_json();
        assert_eq!(said.iter().map(|e| e["notify"].clone()).collect::<Vec<_>>(), vec![json!(PARENT), json!("parent0002")]);
        assert_eq!(said.iter().map(|e| e["text"].clone()).collect::<Vec<_>>(), vec![json!(line), json!(line)]);
        let started = ctx.started.borrow();
        assert_eq!(
            started.iter().map(|s| s.1.thread.clone().unwrap_or_default()).collect::<Vec<_>>(),
            vec![PARENT.to_string(), "parent0002".to_string()]
        );
        assert_eq!(started.iter().map(|s| s.1.prompt.clone()).collect::<Vec<_>>(), vec![line.clone(), line.clone()]);
        let log = ctx.store.logged();
        assert_eq!(log.len(), 4, "one line per target, each its own row, each written and then taken");
        for target in [PARENT, "parent0002"] {
            let put = log.iter().position(|l| *l == format!("put notify-owed/u-c:0:{target}")).expect("written");
            let delete = log.iter().position(|l| *l == format!("delete notify-owed/u-c:0:{target}")).expect("taken");
            assert!(put < delete, "{target}");
        }
    });
}

// TS: runtime-notify.test.ts:561
#[test]
fn a_target_whose_thread_is_gone_falls_back_to_the_person_once_with_me_already_named() {
    local(async {
        let (ctx, area) = owed_host();
        let done = result_of(json!({ "status": "completed", "text": "done" }));
        area.notify_end(&child_row(Some("cs-c"), None), &["gone0001".to_string(), "me".to_string()], &TellAs::default(), &done, false);
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(said.iter().map(|e| e["notify"].clone()).collect::<Vec<_>>(), vec![json!("me")], "told once however many fell away");
        assert_eq!(said[0]["text"], json!(notify_line(FROM, &done, Some(NotifyLength::Tail))));
        assert!(ctx.started.borrow().is_empty());
        assert!(ctx.store.logged().is_empty());
    });
}

// TS: runtime-notify.test.ts:600
#[test]
fn the_line_into_a_thread_carries_the_final_message_whole_and_the_persons_stays_its_last_line() {
    local(async {
        let (ctx, area) = owed_host();
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Steered, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let report = result_of(
            json!({ "status": "completed", "text": "Ran the gate.\n\nAll 12 tests green.\n", "durationMs": 492_000, "costUsd": 1.94 }),
        );
        area.notify_end(&child_row(Some("cs-c"), None), &[PARENT.to_string()], &TellAs::default(), &report, false);
        let mine = row(view(json!({ "id": "r-m", "workspaceId": "ws-c", "threadId": "mine0001", "claudeSessionId": "cs-m" })), "u-m");
        area.notify_end(&mine, &["me".to_string()], &TellAs::default(), &report, false);
        settle().await;
        let said = ctx.recorded_json();
        let whole = "thread child000 finished (completed, 8m 12s, $1.94): Ran the gate.\n\nAll 12 tests green.";
        assert_eq!((said[0]["notify"].clone(), said[0]["text"].clone()), (json!(PARENT), json!(whole)));
        assert_eq!(
            (said[1]["notify"].clone(), said[1]["text"].clone()),
            (json!("me"), json!(notify_line("mine0001", &report, Some(NotifyLength::Tail))))
        );
        assert_ne!(said[1]["text"].as_str().map(|t| t.contains("Ran the gate.")), Some(true), "the person's row is one line, the last");
        assert_eq!(ctx.started.borrow()[0].1.prompt, whole);
    });
}

// =====================================================================================================================
// steer-unread.test.ts: a steered message its agent never read
// =====================================================================================================================

// TS: steer-unread.test.ts:131
#[test]
fn an_unread_message_goes_back_opened_by_whoever_opened_it_and_as_whoever_steered_it() {
    let lead = json!({ "kind": "thread", "threadId": "lead", "workspaceId": "ws1", "rootThreadId": "lead" });
    for (steered, started_by, caller) in [
        (json!({ "prompt": "also add a test", "startedBy": "person" }), "person", None),
        (
            json!({ "prompt": "thread 1234abcd finished (completed): reply with the single word BANANA", "startedBy": "agent", "by": lead, "road": "relayed" }),
            "agent",
            Some(Caller::Scoped { origin: origin("relayed"), by: scope_of("lead", "ws1", "lead") }),
        ),
    ] {
        local(async move {
            let (ctx, area) = owed_host();
            let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
            let prompt = steered["prompt"].as_str().expect("a prompt").to_string();
            area.send_back(&unread_row(steered), &["st1".to_string()], false);
            settle().await;
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1);
            assert_eq!(started[0].1.prompt, prompt);
            assert_eq!(
                started[0].1.started_by,
                Some(serde_json::from_value(json!(started_by)).expect("an origin")),
                "opened by whoever opened it"
            );
            assert_eq!(started[0].2, caller, "under the caller that steered it");
            assert!(!started[0].1.wakes_lead, "a steered message is no child's report");
            assert_eq!(started[0].1.thread.as_deref(), Some(PARENT));
        });
    }
}

// TS: steer-unread.test.ts:117, 340
#[test]
fn an_unread_message_steered_from_a_paired_computer_is_told_to_the_person_and_starts_no_turn() {
    local(async {
        let (ctx, area) = owed_host();
        let warned = wsp_core::console::capture();
        area.send_back(
            &unread_row(json!({ "prompt": "also add a test", "startedBy": "person", "road": "paired" })),
            &["st1".to_string()],
            false,
        );
        settle().await;
        assert_eq!(
            warned.lines(),
            vec!["thread parent00 ended, but its line was registered from a paired computer, which starts no turn on thread parent00"]
        );
        assert!(ctx.started.borrow().is_empty());
        let told = told_person(&ctx.recorded_json());
        assert_eq!(told.len(), 1);
        assert_eq!((told[0]["threadId"].clone(), told[0]["text"].clone()), (json!(PARENT), json!(unread_line("also add a test"))));
        assert!(ctx.store.get_now("notify-owed", "u-t:unread:st1").is_none(), "the line fell and left the store");
    });
}

// TS: steer-unread.test.ts:152, 174
#[test]
fn an_unread_message_held_for_a_napping_workspace_goes_at_the_wake_with_its_images_after_a_restart() {
    local(async {
        let (ctx1, area1) = owed_host();
        set_phase(&ctx1, "ws1", "napping");
        let steered = json!({ "prompt": "\n  look at this  \nmore", "startedBy": "person", "requestId": "req-s", "attachments": [{ "mediaType": "image/png", "bytes": 3 }] });
        area1.send_back(&unread_row(steered), &["st1".to_string()], false);
        settle().await;
        assert!(ctx1.started.borrow().is_empty(), "held for the wake");
        let kept = ctx1.store.get_now("notify-owed", "u-t:unread:st1").expect("kept as a line");
        assert_eq!(kept["images"], json!({ "under": "req-s", "records": [{ "mediaType": "image/png", "bytes": 3 }] }));
        // The next host reads the store and holds the line again, since the workspace still naps.
        let (ctx2, area2) = owed_host();
        set_phase(&ctx2, "ws1", "napping");
        ctx2.store.seed("notify-owed", "u-t:unread:st1", kept.clone());
        ctx2.store.seed_blob("attachments", &format!("{PARENT}.req-s.0"), &[1, 2, 3]);
        area2.deliver_owed().await.expect("listed");
        settle().await;
        assert!(ctx2.started.borrow().is_empty());
        assert_eq!(area2.held_lines.borrow().get("ws1").map(|l| l.len()), Some(1));
        set_phase(&ctx2, "ws1", "running");
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx2.starts.borrow_mut().push_back(Ok(h));
        ctx2.bus.emit_json(woken("ws1"));
        settle().await;
        let started = ctx2.started.borrow();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].1.prompt, "\n  look at this  \nmore");
        assert_eq!(started[0].1.request_id.as_deref(), kept["requestId"].as_str());
        assert_eq!(
            support::json_of(started[0].1.attachments.as_ref().expect("its images")),
            json!([{ "mediaType": "image/png", "bytes": "AQID" }]),
            "read back from what the host kept of the steer"
        );
    });
}

// =====================================================================================================================
// steer-lines.test.ts: a child's finished lines into its lead
// =====================================================================================================================

// TS: steer-lines.test.ts:101, 115
#[test]
fn a_reply_over_background_work_and_the_answer_after_it_are_two_lines_and_only_the_open_one_skips_the_done_read() {
    local(async {
        let (ctx, area) = owed_host();
        let (sub, _) = entry(
            json!({ "id": "ws-c", "phase": "running", "sub": { "on": "hetzner", "repo": "r", "base": { "branch": "main", "sha": "abc" }, "branch": "b", "folder": "/f" } }),
        );
        ctx.add_entry(sub);
        set_phase(&ctx, "ws1", "napping");
        let reply =
            result_of(json!({ "status": "completed", "text": "Built the parser; 40 tests pass.\n\n1 background task is still running" }));
        let answer = result_of(json!({ "status": "completed", "text": "BANANA" }));
        area.notify_end(&child_row(Some("cs-c"), Some(0)), &[PARENT.to_string()], &TellAs::default(), &reply, true);
        area.notify_end(&child_row(Some("cs-c"), Some(1)), &[PARENT.to_string()], &TellAs::default(), &answer, false);
        settle().await;
        let first = ctx.store.get_now("notify-owed", &format!("u-c:0:{PARENT}")).expect("the reply's line");
        let second = ctx.store.get_now("notify-owed", &format!("u-c:1:{PARENT}")).expect("the answer's line, kept apart");
        assert_eq!(first["text"], json!(notify_line(FROM, &reply, Some(NotifyLength::Whole))));
        assert_eq!(second["text"], json!(notify_line(FROM, &answer, Some(NotifyLength::Whole))));
        assert_eq!(first.get("awaitsResult"), None, "a reply given over work still running has no done behind it yet");
        assert_eq!(second["awaitsResult"], json!("ws-c"), "a sub-thread's own end reads its done off the machine");
        assert!(!ctx.called().iter().any(|c| c.starts_with("subLine")), "held for the wake, so nothing is read yet");
        let held: Vec<String> =
            area.held_lines.borrow().get("ws1").map(|l| l.iter().map(|x| x.borrow().id.clone()).collect()).unwrap_or_default();
        assert_eq!(held, vec![format!("u-c:0:{PARENT}"), format!("u-c:1:{PARENT}")], "held in the order told");
    });
}

// =====================================================================================================================
// thread-cap.test.ts: a computer's threads at once
// =====================================================================================================================

fn cap_host(at_once: i64, running: usize) -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>, wsp_core::types::wiring::WorkspaceRecord) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "kind": "local" }));
    let record = e.borrow().record.borrow().clone();
    ctx.add_entry(e);
    ctx.place_ids.borrow_mut().insert("ws1".into(), "here".into());
    ctx.threads_at.borrow_mut().insert("here".into(), at_once);
    ctx.sessions.with(|s| {
        *s = rows_of(
            (0..running)
                .map(|i| row(view(json!({ "id": format!("u{i}"), "threadId": format!("t{i}"), "status": "running" })), &format!("u{i}")))
                .collect(),
        )
    });
    let area = ctx.area();
    (ctx, area, record)
}

fn start_row(ctx: &FakeThreadsCtx, turn: &str, thread: &str) {
    ctx.sessions.with(|s| {
        s.insert(turn.to_string(), row(view(json!({ "id": turn, "threadId": thread, "status": "running", "harness": "codex" })), turn));
    });
}

fn cap_wait_of(running: i64, at_once: i64) -> ThreadCapWait {
    serde_json::from_value(json!({ "placeId": "here", "place": "here-name", "running": running, "atOnce": at_once })).expect("a wait")
}

fn at(turn: &str, thread: &str) -> CapAt {
    CapAt {
        workspace_id: "ws1".into(),
        thread_id: thread.into(),
        turn_id: turn.into(),
        session_id: format!("s-{turn}"),
        request_id: Some("req1".into()),
        prompt: "build the thing\nwhole".into(),
    }
}

fn end_of(turn: &str) -> Value {
    json!({ "type": "session.end", "workspaceId": "ws1", "sessionId": "x", "turnId": turn, "exitCode": null, "sawResult": false })
}

/// A turn's end: its row goes and the end is announced, as a turn that ended does.
fn turn_ends(ctx: &FakeThreadsCtx, turn: &str) {
    ctx.sessions.with(|s| s.shift_remove(turn));
    ctx.bus.emit_json(end_of(turn));
}

fn borrowed_of(area: &ThreadsArea, turn: &str) -> Option<String> {
    area.borrowed.borrow().get(turn).cloned()
}

// TS: thread-cap.test.ts:78
#[test]
fn a_leads_child_runs_in_the_leads_slot_and_a_thread_of_another_tree_waits_for_both() {
    let (ctx, area, record) = cap_host(1, 1);
    start_row(&ctx, "n1", "tn");
    area.cap_hold(&record, "n1", Some("u0"));
    assert_eq!(
        area.cap_full(&record, "n1"),
        None,
        "the child runs in the lead's slot, so the lead waiting on it is never stuck behind itself"
    );
    area.cap_left("n1");
    assert_eq!(borrowed_of(&area, "n1").as_deref(), Some("u0"));
    start_row(&ctx, "n2", "to");
    area.cap_hold(&record, "n2", None);
    assert_eq!(area.cap_full(&record, "n2"), Some(cap_wait_of(2, 1)), "the row counts both agents, so a thread of another tree waits");
    turn_ends(&ctx, "u0");
    assert_eq!(borrowed_of(&area, "n1"), None, "the child holds a slot of its own once its lender ended");
    assert_eq!(area.cap_full(&record, "n2"), Some(cap_wait_of(1, 1)));
    turn_ends(&ctx, "n1");
    assert_eq!(area.cap_full(&record, "n2"), None);
}

// TS: thread-cap.test.ts:251, 297
#[test]
fn children_started_detached_run_one_at_a_time_in_order_and_a_followed_grandchild_takes_its_parents_slot() {
    let (ctx, area, record) = cap_host(1, 0);
    for (turn, thread) in [("n1", "c1"), ("n2", "c2"), ("n3", "c3")] {
        start_row(&ctx, turn, thread);
        area.cap_hold(&record, turn, None);
    }
    assert_eq!(area.cap_full(&record, "n1"), None, "the first in line has the computer");
    assert_eq!(area.cap_full(&record, "n2"), Some(cap_wait_of(0, 1)), "the turn ahead of it counts, though it runs nowhere yet");
    assert_eq!(area.cap_full(&record, "n3"), Some(cap_wait_of(0, 1)));
    area.cap_left("n1");
    assert_eq!(area.cap_full(&record, "n2"), Some(cap_wait_of(1, 1)), "now the first runs, and the next waits on it");
    // A child following its own child gets it started in its slot rather than waiting on itself.
    start_row(&ctx, "g1", "g1t");
    area.cap_hold(&record, "g1", Some("n1"));
    assert_eq!(area.cap_full(&record, "g1"), None);
    area.cap_left("g1");
    assert_eq!(borrowed_of(&area, "g1").as_deref(), Some("n1"));
    // One it started detached takes a slot of its own, behind the turns already in line.
    start_row(&ctx, "g2", "g2t");
    area.cap_hold(&record, "g2", None);
    assert_eq!(
        area.cap_full(&record, "g2"),
        Some(cap_wait_of(2, 1)),
        "two agents run here, one of them in the other's slot, and two turns stand ahead"
    );
    turn_ends(&ctx, "g1");
    turn_ends(&ctx, "n1");
    assert_eq!(area.cap_full(&record, "n2"), None, "the line moves in the order it came");
    area.cap_left("n2");
    assert_eq!(area.cap_full(&record, "n3"), Some(cap_wait_of(1, 1)));
}

// TS: thread-cap.test.ts:346
#[test]
fn a_chain_of_blocking_starts_four_deep_runs_each_in_the_slot_of_the_thread_waiting_on_it() {
    let (ctx, area, record) = cap_host(1, 1);
    let mut lender = "u0".to_string();
    for d in 1..=4 {
        let turn = format!("n{d}");
        start_row(&ctx, &turn, &format!("tl{d}"));
        area.cap_hold(&record, &turn, Some(lender.as_str()));
        assert_eq!(area.cap_full(&record, &turn), None, "depth {d}");
        area.cap_left(&turn);
        assert_eq!(borrowed_of(&area, &turn).as_deref(), Some(lender.as_str()), "depth {d}");
        lender = turn;
    }
    start_row(&ctx, "nx", "tx");
    area.cap_hold(&record, "nx", None);
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(5, 1)), "five agents run on a computer set to one, and a sixth thread waits");
}

// TS: thread-cap.test.ts:371
#[test]
fn a_turn_in_a_slot_it_borrowed_holds_one_of_its_own_when_the_thread_it_borrowed_from_ends_first() {
    let (ctx, area, record) = cap_host(1, 1);
    start_row(&ctx, "n1", "child");
    area.cap_hold(&record, "n1", Some("u0"));
    assert_eq!(area.cap_full(&record, "n1"), None);
    area.cap_left("n1");
    start_row(&ctx, "n2", "grand");
    area.cap_hold(&record, "n2", Some("n1"));
    assert_eq!(area.cap_full(&record, "n2"), None);
    area.cap_left("n2");
    start_row(&ctx, "nx", "outside");
    area.cap_hold(&record, "nx", None);
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(3, 1)));
    // The child's follow gives up and its turn ends with the grandchild still running.
    turn_ends(&ctx, "n1");
    assert_eq!(borrowed_of(&area, "n2"), None, "the grandchild lent from the child holds a slot of its own now");
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(2, 1)), "the lead and the grandchild, one slot each");
    turn_ends(&ctx, "u0");
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(1, 1)), "the grandchild alone, still holding the computer");
    turn_ends(&ctx, "n2");
    assert_eq!(area.cap_full(&record, "nx"), None);
}

// TS: thread-cap.test.ts:788, 808, 828, 848
#[test]
fn a_follow_up_sent_into_a_held_thread_lends_it_the_senders_slot_and_the_transcript_says_lent() {
    local(async {
        let (ctx, area, record) = cap_host(1, 1);
        start_row(&ctx, "n1", "tb");
        area.cap_hold(&record, "n1", None);
        let waiting = area.cap_wait(at("n1", "tb"), cap_wait_of(1, 1)).shared();
        let watch = waiting.clone();
        tokio::task::spawn_local(async move {
            let _ = watch.await;
        });
        settle().await;
        assert!(waiting.peek().is_none(), "held detached, the builder waits for a slot");
        area.cap_lend("n1", "u0");
        settle().await;
        assert!(matches!(waiting.peek(), Some(Ok(()))), "the lead follows the builder now, so the builder looks again");
        assert_eq!(area.cap_full(&record, "n1"), None, "it runs in the sender's slot");
        assert_eq!(borrowed_of(&area, "n1").as_deref(), Some("u0"));
        assert_eq!(area.cap_held.borrow().get("n1").and_then(|h| h.through), Some((SessionUncappedEventWhy::Lent, 1)));
        area.cap_left("n1");
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(), vec![json!("session.capped"), json!("session.uncapped")]);
        for (k, v) in [
            ("workspaceId", json!("ws1")),
            ("threadId", json!("tb")),
            ("sessionId", json!("s-n1")),
            ("turnId", json!("n1")),
            ("why", json!("lent")),
            ("atOnce", json!(1)),
        ] {
            assert_eq!(said[1][k], v, "{k}");
        }
        assert_eq!(ctx.store.logged(), vec!["put held-starts/n1", "delete held-starts/n1"]);
    });
}

// TS: thread-cap.test.ts:445
#[test]
fn a_stop_that_lands_after_the_wake_is_read_by_the_next_look_and_the_turn_never_launches() {
    local(async {
        let (ctx, area, record) = cap_host(1, 1);
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        let waiting = area.cap_wait(at("n1", "tn"), cap_wait_of(1, 1)).shared();
        let watch = waiting.clone();
        tokio::task::spawn_local(async move {
            let _ = watch.await;
        });
        settle().await;
        ctx.bus.emit_json(end_of("other"));
        settle().await;
        assert!(matches!(waiting.peek(), Some(Ok(()))), "an end anywhere woke the wait");
        // The stop lands between the wake and the look the wake is for.
        assert!(area.cap_stop("n1", "stopped before it started"));
        let next = area.cap_wait(at("n1", "tn"), cap_wait_of(1, 1)).await;
        assert_eq!(next.expect_err("the look reads the stop").message, "stopped before it started");
        settle().await;
        assert_eq!(ctx.recorded_json().len(), 1, "only the first wait was said");
        assert_eq!(ctx.store.logged(), vec!["put held-starts/n1"], "no second record written");
        assert_eq!(ctx.called().iter().filter(|c| *c == "idle.hold ws1").count(), 1, "the refused look holds the machine awake no more");
        area.cap_left("n1");
        settle().await;
        assert_eq!(ctx.store.logged(), vec!["put held-starts/n1", "delete held-starts/n1"]);
        assert_eq!(ctx.recorded_json().len(), 1, "a turn that never got through says no why");
    });
}

// TS: thread-cap.test.ts:685
#[test]
fn a_held_send_of_three_thousand_characters_keeps_them_all_in_its_record_and_its_wait() {
    local(async {
        let (ctx, area, record) = cap_host(1, 1);
        let long: String = format!("two {}", "wait for a slot and keep every word. ".repeat(80)).chars().take(3000).collect();
        assert_eq!(long.chars().count(), 2964, "node gives 2964 for the 3000-slice of this string");
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        let _waiting = area.cap_wait(CapAt { prompt: long.clone(), ..at("n1", "tn") }, cap_wait_of(1, 1));
        settle().await;
        let held = ctx.store.get_now("held-starts", "n1").expect("written down");
        assert_eq!(held["prompt"], json!(long), "the record is the only copy of a message the agent never got");
        assert_eq!(held["turnId"], json!("n1"));
        let said = ctx.recorded_json();
        assert_eq!(
            (said[0]["type"].clone(), said[0]["turnId"].clone(), said[0]["prompt"].clone()),
            (json!("session.capped"), json!("n1"), json!(long))
        );
    });
}

// TS: thread-cap.test.ts:1003
#[test]
fn a_lenders_end_gives_the_turn_in_its_slot_a_slot_of_its_own_so_a_third_thread_waits() {
    let (ctx, area, record) = cap_host(2, 2);
    // u0 is the lead; u1 (thread t1) is another thread running beside it.
    start_row(&ctx, "nb", "tb");
    area.cap_hold(&record, "nb", None);
    start_row(&ctx, "nr", "tr");
    area.cap_hold(&record, "nr", Some("u0"));
    assert_eq!(area.cap_full(&record, "nr"), None, "the reviewer the lead follows runs in the lead's slot");
    area.cap_left("nr");
    // The reviewer asks the builder and follows: the builder runs in the reviewer's slot.
    area.cap_lend("nb", "nr");
    assert_eq!(area.cap_full(&record, "nb"), None);
    area.cap_left("nb");
    assert_eq!(borrowed_of(&area, "nb").as_deref(), Some("nr"));
    start_row(&ctx, "nx", "tx");
    area.cap_hold(&record, "nx", None);
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(4, 2)), "four agents, two of them lent: the third thread waits");
    // The reviewer is stopped while the builder runs in its slot.
    turn_ends(&ctx, "nr");
    assert_eq!(borrowed_of(&area, "nb"), None);
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(3, 2)), "lead, the other thread and the builder each hold a slot");
    turn_ends(&ctx, "u1");
    assert_eq!(area.cap_full(&record, "nx"), Some(cap_wait_of(2, 2)), "still full while the builder runs");
    turn_ends(&ctx, "nb");
    assert_eq!(area.cap_full(&record, "nx"), None);
}

// TS: thread-cap.test.ts:1098
#[test]
fn raising_the_threads_at_once_lets_the_waiting_turns_through_oldest_first_each_saying_raised() {
    local(async {
        let (ctx, area, record) = cap_host(6, 6);
        let mut waits = vec![];
        for (turn, thread, prompt) in [("n7", "t7", "seventh"), ("n8", "t8", "eighth")] {
            start_row(&ctx, turn, thread);
            area.cap_hold(&record, turn, None);
            let waiting = area.cap_wait(CapAt { prompt: prompt.into(), ..at(turn, thread) }, cap_wait_of(6, 6)).shared();
            let watch = waiting.clone();
            tokio::task::spawn_local(async move {
                let _ = watch.await;
            });
            waits.push(waiting);
        }
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(
            said.iter().map(|e| (e["turnId"].clone(), e["prompt"].clone())).collect::<Vec<_>>(),
            vec![(json!("n7"), json!("seventh")), (json!("n8"), json!("eighth"))]
        );
        // The person raises the count: every held turn looks again.
        ctx.threads_at.borrow_mut().insert("here".into(), 8);
        ctx.bus.emit_json(end_of("other"));
        settle().await;
        assert!(waits.iter().all(|w| matches!(w.peek(), Some(Ok(())))));
        assert_eq!(area.cap_full(&record, "n7"), None);
        assert_eq!(area.cap_full(&record, "n8"), None, "six running and one ahead make seven of eight");
        area.cap_left("n7");
        area.cap_left("n8");
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(
            said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(),
            vec![json!("session.capped"), json!("session.capped"), json!("session.uncapped"), json!("session.uncapped")]
        );
        for (i, turn) in [(2, "n7"), (3, "n8")] {
            assert_eq!(
                (said[i]["turnId"].clone(), said[i]["why"].clone(), said[i]["atOnce"].clone()),
                (json!(turn), json!("raised"), json!(8))
            );
        }
        assert_eq!(ctx.store.logged(), vec!["put held-starts/n7", "put held-starts/n8", "delete held-starts/n7", "delete held-starts/n8"]);
    });
}

// =====================================================================================================================
// thread-keeps-process, thread-warm-agent and box-keep: agent processes kept between turns
// =====================================================================================================================

struct Agent {
    closed: std::sync::Arc<std::sync::Mutex<Vec<bool>>>,
    working: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Agent {
    fn closes(&self) -> Vec<bool> {
        self.closed.lock().expect("closed").clone()
    }
}

fn launch(fixed: &str, picks: Value) -> KeptLaunch {
    KeptLaunch { fixed: fixed.into(), picks: serde_json::from_value(picks).expect("picks") }
}

fn kept_hand(workspace: &str, file: Option<SessionFile>, token: &str) -> (KeptHand, Agent) {
    let agent = FakeKeptAgent::new(file);
    let seen = Agent { closed: agent.closed.clone(), working: agent.working.clone() };
    (
        KeptHand {
            workspace_id: workspace.into(),
            agent: Box::new(agent),
            pid: None,
            launch: launch("claude|/work|v1", json!({ "model": "opus" })),
            session: Some("s1".into()),
            warm: None,
            turn_token: token.into(),
            scope_device_id: None,
            waiting: Default::default(),
        },
        seen,
    )
}

fn warm_kept_hand(workspace: &str) -> (KeptHand, Agent) {
    let (mut h, a) = kept_hand(workspace, None, "tok-warm");
    h.session = None;
    h.warm = Some(WarmKeep { harness: "claude".into(), claimed: false });
    (h, a)
}

/// A local workspace `ws1`, a workspace `wsb` on the joined computer `box1` (4 GB, linked), and records for `threads`.
fn kept_host(threads: &[&str]) -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>, std::sync::Arc<support::FakeMachine>) {
    let ctx = FakeThreadsCtx::new();
    let (local_ws, _) = entry(json!({ "id": "ws1", "kind": "local" }));
    ctx.add_entry(local_ws);
    let (boxed, machine) = entry(json!({ "id": "wsb", "kind": "local", "place": "box1" }));
    ctx.add_entry(boxed);
    ctx.mem_mb.borrow_mut().insert("box1".into(), 4096.0);
    ctx.linked.borrow_mut().push("box1".into());
    ctx.thread_records.with(|r| {
        for t in threads {
            r.insert(t.to_string(), record("ws1", json!({})));
        }
    });
    let area = ctx.area();
    (ctx, area, machine)
}

fn session_file() -> SessionFile {
    SessionFile { folder: "/home/u/.claude/projects".into(), name: "s1.jsonl".into(), depth: 1 }
}

fn kept_ids(area: &ThreadsArea) -> Vec<String> {
    area.kept_agents.borrow().keys().cloned().collect()
}

fn same_launch() -> KeptLaunch {
    launch("claude|/work|v1", json!({}))
}

fn wc_reads(machine: &support::FakeMachine) -> usize {
    machine.commands().iter().filter(|c| c.contains("wc -c")).count()
}

// TS: thread-keeps-process.test.ts:92, 106
#[test]
fn a_kept_process_holds_its_turn_token_between_turns_and_the_next_turn_runs_under_the_same_one() {
    local(async {
        let (_ctx, area, _) = kept_host(&["t1"]);
        let (h, _agent) = kept_hand("ws1", None, "tok-one");
        assert!(area.hold_kept("t1", h).is_none());
        assert_eq!(area.thread_of_token("tok-one").expect("a slate write between turns lands on its own thread"), "t1");
        let process = area.take_kept("t1", Some(&same_launch()), Some("s1"), None).expect("the second send runs on it");
        assert_eq!(process.borrow().turn_token, "tok-one", "the process goes on holding the token its first turn was launched with");
        let refused = area.thread_of_token("tok-one").expect_err("while its turn runs the row carries the token, not the keep");
        assert_eq!(refused.message, NO_SUCH_TURN);
        let (again, _agent) = kept_hand("ws1", None, "tok-one");
        assert!(area.hold_kept("t1", again).is_none());
        assert_eq!(area.thread_of_token("tok-one").expect("put back after the turn"), "t1");
    });
}

// TS: thread-keeps-process.test.ts:161
#[test]
fn the_hosts_own_title_write_into_the_session_file_does_not_end_the_kept_process() {
    local(async {
        let dir = tempfile::tempdir().expect("a folder");
        let path = dir.path().join("s1.jsonl");
        std::fs::write(&path, b"{}\n").expect("written");
        let on_disk = SessionFile { folder: dir.path().to_string_lossy().into_owned(), name: "s1.jsonl".into(), depth: 0 };
        let (_ctx, area, _) = kept_host(&["t1"]);
        let (h, agent) = kept_hand("ws1", Some(on_disk), "tok");
        assert!(area.hold_kept("t1", h).is_none());
        for (what, text) in [
            ("the title", "{}\n{\"type\":\"custom-title\"}\n"),
            ("a rename the person makes", "{}\n{\"type\":\"custom-title\"}\n{\"type\":\"custom-title\"}\n"),
        ] {
            let at = path.clone();
            let wrote = area
                .write_session(
                    "s1",
                    async move {
                        std::fs::write(&at, text.as_bytes()).expect("the host wrote the file");
                        Ok(SessionRenameWrite::Written)
                    }
                    .boxed_local(),
                )
                .await;
            assert_eq!(wrote.expect("written"), SessionRenameWrite::Written, "{what}");
            settle().await;
            assert_eq!(kept_ids(&area), vec!["t1"], "{what}");
        }
        let taken = area.take_kept("t1", Some(&same_launch()), Some("s1"), None);
        assert!(taken.is_some(), "the next send runs on the kept process, not read as the person resuming the session elsewhere");
        assert!(agent.closes().is_empty());
    });
}

// TS: thread-keeps-process.test.ts:196
#[test]
fn an_idle_kept_process_ends_at_the_end_of_its_keep_and_its_token_then_names_nobody() {
    local(async {
        let (ctx, area, _) = kept_host(&["t1"]);
        let (h, agent) = kept_hand("ws1", None, "tok-idle");
        assert!(area.hold_kept("t1", h).is_none());
        ctx.clock.advance(AGENT_KEEP_MS - 1_000);
        settle().await;
        assert_eq!(area.thread_of_token("tok-idle").expect("between turns the token still names its thread"), "t1");
        assert!(agent.closes().is_empty());
        ctx.clock.advance(1_000);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert_eq!(agent.closes(), vec![false]);
        assert_eq!(area.thread_of_token("tok-idle").expect_err("the keep ran out").message, NO_SUCH_TURN);
    });
}

// TS: thread-keeps-process.test.ts:270
#[test]
fn at_the_cap_the_idle_one_kept_longest_ends_and_never_a_process_still_working() {
    local(async {
        let names = ["t0", "t1", "t2", "t3", "t4", "t5", "t6"];
        let (ctx, area, _) = kept_host(&names);
        let mut agents = vec![];
        for (i, t) in names.iter().enumerate() {
            let (h, a) = kept_hand("ws1", None, &format!("tok-{i}"));
            if i == 0 {
                a.working.store(true, Ordering::SeqCst);
            }
            assert!(area.hold_kept(t, h).is_none());
            agents.push(a);
            ctx.clock.advance(1);
        }
        settle().await;
        assert_eq!(AGENTS_KEPT, 6);
        assert_eq!(
            kept_ids(&area),
            vec!["t0", "t2", "t3", "t4", "t5", "t6"],
            "the oldest is working, so the idle one kept longest after it goes"
        );
        assert_eq!(agents[1].closes(), vec![false]);
        assert!(agents[0].closes().is_empty());
    });
}

// TS: thread-warm-agent.test.ts:82
#[test]
fn a_claimed_warm_process_runs_the_send_that_opens_its_thread_and_is_kept_for_the_threads_next() {
    local(async {
        let (ctx, area, _) = kept_host(&[]);
        let (h, agent) = warm_kept_hand("ws1");
        area.hold_warm("w1", h);
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w1".to_string()), "the send that opens a thread takes its thread id");
        let process = area
            .take_kept("w1", None, None, Some(&launch("claude|/work|v1", json!({ "model": "opus" }))))
            .expect("launched exactly as the send would launch");
        assert_eq!(process.borrow().warm.as_ref().map(|w| w.claimed), Some(true));
        assert!(kept_ids(&area).is_empty(), "taken out of the keep for its turn");
        assert_eq!(ctx.clock.pending(), 0, "its idle window is cancelled");
        assert!(agent.closes().is_empty());
        assert_eq!(area.warm_on("ws1", "claude").map(|(t, _)| t), None);
        // The turn over, the thread's own process is kept for its next send.
        ctx.thread_records.with(|r| {
            r.insert("w1".into(), record("ws1", json!({})));
        });
        let (next, _a) = kept_hand("ws1", None, "tok-warm");
        assert!(area.hold_kept("w1", next).is_none());
        assert_eq!(kept_ids(&area), vec!["w1"]);
        assert_eq!(area.warm_on("ws1", "claude").map(|(t, _)| t), None, "kept as a thread's own, not as a warm one");
    });
}

// TS: thread-warm-agent.test.ts:100
#[test]
fn a_warm_process_never_outlives_its_three_idle_minutes() {
    local(async {
        let (ctx, area, _) = kept_host(&[]);
        let (h, agent) = warm_kept_hand("ws1");
        area.hold_warm("w1", h);
        ctx.clock.advance(AGENT_WARM_MS - 1_000);
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w1"]);
        assert!(agent.closes().is_empty());
        ctx.clock.advance(1_000);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert_eq!(agent.closes(), vec![false]);
        assert_eq!(area.claim_warm("ws1", "claude"), None, "a send after it boots a process of its own");
        let (again, _a) = warm_kept_hand("ws1");
        area.hold_warm("w1", again);
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w1".to_string()), "asked for again, a new one stands");
    });
}

// TS: thread-warm-agent.test.ts:124
#[test]
fn asked_for_a_minute_apart_for_five_minutes_the_send_runs_on_the_process_started_at_the_first() {
    local(async {
        let (ctx, area, _) = kept_host(&[]);
        let (h, agent) = warm_kept_hand("ws1");
        area.hold_warm("w1", h);
        for _ in 1..=5 {
            ctx.clock.advance(60_000);
            area.rewarm("w1");
            assert_eq!(ctx.clock.pending(), 1, "each ask replaces the window, never adds one");
        }
        ctx.clock.advance(AGENT_WARM_MS - 1_000);
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w1"]);
        assert!(agent.closes().is_empty());
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w1".to_string()));
        assert!(area.take_kept("w1", None, None, Some(&launch("claude|/work|v1", json!({ "model": "opus" })))).is_some());
    });
}

// TS: thread-warm-agent.test.ts:162, 174
#[test]
fn a_send_at_other_picks_than_the_warm_process_was_launched_with_ends_it_either_way_round() {
    local(async {
        for (kept_picks, sent_picks) in [
            (json!({ "model": "opus" }), json!({ "model": "sonnet" })),
            (json!({ "model": "opus", "fast": true }), json!({ "model": "opus" })),
        ] {
            let (_ctx, area, _) = kept_host(&[]);
            let (mut h, agent) = warm_kept_hand("ws1");
            h.launch = launch("claude|/work|v1", kept_picks.clone());
            area.hold_warm("w1", h);
            assert!(
                area.take_kept("w1", None, None, Some(&launch("claude|/work|v1", sent_picks.clone()))).is_none(),
                "{kept_picks} against {sent_picks}"
            );
            settle().await;
            assert!(kept_ids(&area).is_empty());
            assert_eq!(agent.closes(), vec![false], "it ends rather than run on the wrong model, or with a pick nobody asked for");
        }
    });
}

// TS: thread-warm-agent.test.ts:186
#[test]
fn a_send_that_claimed_the_warm_process_and_is_refused_ends_it_so_none_stands_claimed() {
    local(async {
        let (_ctx, area, _) = kept_host(&[]);
        let (h, agent) = warm_kept_hand("ws1");
        area.hold_warm("w1", h);
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w1".to_string()));
        area.reap_kept("w1", false);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert_eq!(agent.closes(), vec![false]);
        assert_eq!(area.warm_on("ws1", "claude").map(|(t, _)| t), None);
        let (fresh, _a) = warm_kept_hand("ws1");
        area.hold_warm("w2", fresh);
        assert_eq!(area.claim_warm("ws1", "claude"), Some("w2".to_string()), "the next warm one is claimable");
    });
}

// TS: thread-warm-agent.test.ts:252
#[test]
fn warming_a_third_ends_the_one_asked_for_longest_ago_and_asking_again_moves_one_up() {
    local(async {
        let (ctx, area, _) = kept_host(&[]);
        let mut agents = vec![];
        for t in ["w1", "w2", "w3"] {
            let (h, a) = warm_kept_hand("ws1");
            area.hold_warm(t, h);
            agents.push(a);
            ctx.clock.advance(10);
        }
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w2", "w3"]);
        assert_eq!(agents[0].closes(), vec![false]);
        // Asked for again, w2 is the more recent of the two, so the fourth ends w3.
        area.rewarm("w2");
        ctx.clock.advance(10);
        let (h, a) = warm_kept_hand("ws1");
        area.hold_warm("w4", h);
        agents.push(a);
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w2", "w4"]);
        assert_eq!(agents[2].closes(), vec![false]);
        assert!(agents[1].closes().is_empty() && agents[3].closes().is_empty());
    });
}

// TS: thread-warm-agent.test.ts:276
#[test]
fn five_warm_processes_asked_in_turn_leave_two_standing_and_those_count_against_the_cap() {
    local(async {
        let threads = ["k1", "k2", "k3", "k4", "k5"];
        let (ctx, area, _) = kept_host(&threads);
        let mut warm = vec![];
        for t in ["w1", "w2", "w3", "w4", "w5"] {
            let (h, a) = warm_kept_hand("ws1");
            area.hold_warm(t, h);
            warm.push(a);
            ctx.clock.advance(10);
        }
        settle().await;
        assert_eq!(kept_ids(&area), vec!["w4", "w5"]);
        for a in &warm[..3] {
            assert_eq!(a.closes(), vec![false]);
        }
        // Five threads' kept processes on top of the two: the seventh process ends the idle one kept longest, the older warm one.
        for t in threads {
            let (h, _a) = kept_hand("ws1", None, &format!("tok-{t}"));
            assert!(area.hold_kept(t, h).is_none());
            ctx.clock.advance(10);
        }
        settle().await;
        assert_eq!(warm[3].closes(), vec![false]);
        assert!(warm[4].closes().is_empty());
        assert_eq!(kept_ids(&area), vec!["w5", "k1", "k2", "k3", "k4", "k5"]);
    });
}

// TS: box-keep.test.ts:53
#[test]
fn a_joined_computers_second_send_runs_on_the_process_its_first_turn_left_up_with_no_new_launch() {
    local(async {
        let (_ctx, area, machine) = kept_host(&["t1"]);
        machine.answer("wc -c", 0, "100");
        let (h, agent) = kept_hand("wsb", Some(session_file()), "tok");
        assert!(area.hold_kept("t1", h).is_none());
        settle().await;
        area.kept_check("t1").await;
        assert_eq!(kept_ids(&area), vec!["t1"], "the file there stands as the turn left it");
        let taken = area.take_kept("t1", Some(&same_launch()), Some("s1"), None).expect("the second send runs on it");
        assert_eq!(taken.borrow().turn_token, "tok");
        assert_eq!(wc_reads(&machine), 2, "one read at the keep and one at the send's check: taking asks the computer nothing");
        assert!(agent.closes().is_empty());
    });
}

// TS: box-keep.test.ts:62
#[test]
fn a_joined_computer_keeps_at_most_what_its_memory_takes_and_counts_only_its_own() {
    local(async {
        let (ctx, area, _) = kept_host(&["t0", "t1", "t2", "t3"]);
        let mut agents = vec![];
        for t in ["t0", "t1", "t2"] {
            let (h, a) = kept_hand("wsb", None, &format!("tok-{t}"));
            assert!(area.hold_kept(t, h).is_none());
            agents.push(a);
            ctx.clock.advance(1);
        }
        settle().await;
        assert_eq!(kept_ids(&area), vec!["t1", "t2"], "4096 MB keeps 2 there, the one idle longest ended first");
        assert_eq!(agents[0].closes(), vec![false]);
        let (h, _a) = kept_hand("ws1", None, "tok-here");
        assert!(area.hold_kept("t3", h).is_none());
        settle().await;
        assert_eq!(kept_ids(&area), vec!["t1", "t2", "t3"], "a process on this computer is no part of the box's count");
    });
}

// TS: box-keep.test.ts:109
#[test]
fn a_keep_that_runs_out_while_the_computer_is_away_closes_the_agent_at_its_next_link_and_owes_nothing() {
    local(async {
        let (ctx, area, machine) = kept_host(&["t1"]);
        let (h, agent) = kept_hand("wsb", None, "tok");
        assert!(area.hold_kept("t1", h).is_none());
        ctx.linked.borrow_mut().clear();
        ctx.clock.advance(AGENT_KEEP_MS);
        settle().await;
        assert!(kept_ids(&area).is_empty());
        assert!(agent.closes().is_empty(), "nothing reaches a computer that is away");
        assert_eq!(ctx.place_listeners.borrow().len(), 1);
        assert!(ctx.store.logged().is_empty(), "no thread end is owed the computer");
        assert!(!machine.commands().iter().any(|c| c.contains("wsp_end")));
        ctx.place_move(wsp_threads::threads::threads::PlaceMove::Present("box1".into()));
        settle().await;
        assert_eq!(agent.closes(), vec![false], "closed at the next link, ending the run's own group as it would have then");
        assert!(ctx.place_listeners.borrow().is_empty());
        assert!(ctx.store.logged().is_empty());
    });
}

// TS: box-keep.test.ts:119
#[test]
fn a_send_while_the_computer_is_away_boots_cold_owes_no_thread_end_and_the_old_agent_closes_at_its_next_link() {
    local(async {
        let (ctx, area, machine) = kept_host(&["t1"]);
        let (h, agent) = kept_hand("wsb", None, "tok");
        assert!(area.hold_kept("t1", h).is_none());
        ctx.linked.borrow_mut().clear();
        area.kept_check("t1").await;
        assert!(kept_ids(&area).is_empty(), "its message would reach nothing, so the send boots cold");
        assert!(area.take_kept("t1", Some(&same_launch()), Some("s1"), None).is_none());
        assert!(agent.closes().is_empty());
        assert!(ctx.store.logged().is_empty());
        assert!(machine.commands().is_empty());
        ctx.place_move(wsp_threads::threads::threads::PlaceMove::Present("box1".into()));
        settle().await;
        assert_eq!(agent.closes(), vec![false]);
    });
}

// TS: box-keep.test.ts:128
#[test]
fn a_send_waits_out_the_hosts_own_write_into_the_session_file_before_reading_it() {
    local(async {
        let (_ctx, area, machine) = kept_host(&["t1"]);
        machine.answer("wc -c", 0, "100");
        let (h, agent) = kept_hand("wsb", Some(session_file()), "tok");
        assert!(area.hold_kept("t1", h).is_none());
        settle().await;
        assert_eq!(wc_reads(&machine), 1);
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        let write = area.write_session(
            "s1",
            async move {
                let _ = rx.await;
                Ok(SessionRenameWrite::Written)
            }
            .boxed_local(),
        );
        tokio::task::spawn_local(write);
        settle().await;
        // The title is in the file at once and its answer comes later, so a send meanwhile would read a grown file.
        machine.rules.borrow_mut().clear();
        machine.answer("wc -c", 0, "150");
        let check = tokio::task::spawn_local({
            let area = area.clone();
            async move { area.kept_check("t1").await }
        });
        settle().await;
        assert!(!check.is_finished(), "the send waits for the write");
        assert_eq!(wc_reads(&machine), 1, "it reads nothing before the write answered");
        let _ = tx.send(());
        settle().await;
        assert!(check.is_finished());
        assert_eq!(
            kept_ids(&area),
            vec!["t1"],
            "the send runs on the kept agent: the host's own write is not the person resuming the session"
        );
        assert_eq!(wc_reads(&machine), 3, "one at the keep, one restamping after the write, one at the check");
        assert!(agent.closes().is_empty());
    });
}

// =====================================================================================================================
// turn-changes.test.ts: what a turn changed
// =====================================================================================================================

fn te_host() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>, Entry) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "kind": "local", "name": "the-folder" }));
    ctx.add_entry(e.clone());
    let area = ctx.area();
    (ctx, area, e)
}

fn beside(ctx: &FakeThreadsCtx, rows: Vec<Value>) {
    ctx.sessions.with(|s| {
        for (i, v) in rows.into_iter().enumerate() {
            let id = format!("b{i}");
            let mut v = v;
            v["id"] = json!(id);
            s.insert(id.clone(), row(view(v), &format!("ub{i}")));
        }
    });
}

fn diff(files: Value, moved: Value) -> Value {
    json!({ "base": null, "files": files, "truncated": false, "moved": moved })
}

fn changed(path: &str) -> Value {
    json!({ "path": path, "kind": "added", "additions": 1, "deletions": 0, "patch": "@@" })
}

fn bare(path: &str) -> Value {
    json!({ "path": path, "kind": "added", "additions": 1, "deletions": 0 })
}

fn turn_ask(thread: &str, cwd: &str, wrote: Option<&[&str]>) -> TurnChangesAsk {
    TurnChangesAsk {
        session_id: "cs-1".into(),
        turn_id: format!("u-{thread}"),
        thread_id: thread.into(),
        cwd: cwd.into(),
        from: "base123".into(),
        started_at: 999_000,
        wrote: wrote.map(|w| w.iter().map(|p| p.to_string()).collect::<BTreeSet<_>>()),
    }
}

fn top_of_repo() -> (Value, TopRead) {
    let reads = Rc::new(Cell::new(0));
    let read: TopRead = Rc::new(move || {
        reads.set(reads.get() + 1);
        async { Some("/repo".to_string()) }.boxed_local()
    });
    (json!({ "worktrees": [{ "path": "/repo" }] }), read)
}

// TS: turn-changes.test.ts:186, 241
#[test]
fn a_snapshot_the_daemon_never_answers_ends_at_the_options_cap_and_the_turn_records_nothing() {
    local(async {
        let (ctx, area, e) = te_host();
        ctx.snapshot_ms.set(Some(30));
        ctx.daemon_hangs.borrow_mut().push("git.snapshot".into());
        let warned = wsp_core::console::capture();
        let snapshot = tokio::task::spawn_local({
            let (area, e) = (area.clone(), e.clone());
            async move { area.snapshot_of(&e, "/w").await }
        });
        settle().await;
        ctx.clock.advance(29);
        settle().await;
        assert!(!snapshot.is_finished(), "the cap is the option's, not the default 3000 ms");
        ctx.clock.advance(1);
        settle().await;
        assert_eq!(snapshot.await.expect("ran"), None);
        assert_eq!(
            warned.lines(),
            vec!["no snapshot of /w on the-folder inside 30ms; the turn runs without one and its reply lists no changes"]
        );
        let read = tokio::task::spawn_local({
            let (area, e) = (area.clone(), e.clone());
            async move { area.read_turn_changes(&e, turn_ask("mine", "/w", None)).await }
        });
        settle().await;
        ctx.clock.advance(30);
        settle().await;
        assert!(!read.await.expect("ran"), "no snapshot at the end either");
        assert!(!ctx.frames.borrow().iter().any(|f| f["op"] == "git.turn"), "the range is never asked");
        assert!(ctx.recorded.borrow().is_empty());
    });
}

// TS: turn-changes.test.ts:230
#[test]
fn a_turn_that_only_moved_head_records_no_files_of_its_own_and_the_move_named_on_the_card() {
    local(async {
        let (ctx, area, e) = te_host();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([]), json!(["Pulled"]))));
        assert!(area.read_turn_changes(&e, turn_ask("mine", "/repo/a", None)).await);
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        assert_eq!(
            (said[0]["type"].clone(), said[0]["files"].clone(), said[0]["moved"].clone()),
            (json!("session.changes"), json!([]), json!(["Pulled"]))
        );
        assert_eq!(said[0].get("shared"), None);
        assert_eq!(said[0].get("others"), None);
    });
}

// TS: turn-changes.test.ts:254, 333
#[test]
fn a_turn_shares_its_folder_with_another_threads_turn_and_keeps_the_whole_list_where_the_agent_reports_no_edits() {
    local(async {
        let (ctx, area, e) = te_host();
        beside(&ctx, vec![json!({ "threadId": "other", "cwd": "/repo/a", "startedAt": 999_500 })]);
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([changed("a.ts")]), json!([]))));
        assert!(area.read_turn_changes(&e, turn_ask("mine", "/repo/a", None)).await);
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        assert_eq!(said[0]["files"], json!([bare("a.ts")]), "the folder's list, not split");
        assert_eq!(said[0]["shared"], json!(true));
        assert_eq!(said[0].get("others"), None);
    });
}

// TS: turn-changes.test.ts:293
#[test]
fn a_turn_that_wrote_nothing_shows_none_of_its_own_while_the_thread_that_changed_the_folder_keeps_its_own() {
    local(async {
        let (ctx, area, e) = te_host();
        beside(
            &ctx,
            vec![
                json!({ "threadId": "quiet", "cwd": "/repo/a", "startedAt": 999_100 }),
                json!({ "threadId": "busy", "cwd": "/repo/a", "startedAt": 999_200 }),
            ],
        );
        let (worktrees, _) = top_of_repo();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([changed("a/b.ts")]), json!([]))));
        ctx.daemon_answers("git.worktrees", Ok(worktrees));
        assert!(area.read_turn_changes(&e, turn_ask("quiet", "/repo/a", Some(&[]))).await);
        assert!(area.read_turn_changes(&e, turn_ask("busy", "/repo/a", Some(&["/repo/a/b.ts"]))).await);
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 2);
        assert_eq!(
            (said[0]["files"].clone(), said[0]["others"].clone(), said[0]["shared"].clone()),
            (json!([]), json!([bare("a/b.ts")]), json!(true))
        );
        assert_eq!(
            (said[1]["files"].clone(), said[1]["others"].clone(), said[1]["shared"].clone()),
            (json!([bare("a/b.ts")]), json!([]), json!(true)),
            "an empty others is still said where the list was split"
        );
    });
}

// TS: turn-changes.test.ts:311
#[test]
fn two_threads_in_one_repo_at_different_subfolders_split_the_whole_checkouts_range_by_what_each_wrote() {
    local(async {
        let (ctx, area, e) = te_host();
        beside(
            &ctx,
            vec![
                json!({ "threadId": "ina", "cwd": "/repo/a", "startedAt": 999_100 }),
                json!({ "threadId": "inb", "cwd": "/repo/b", "startedAt": 999_200 }),
            ],
        );
        let (worktrees, _) = top_of_repo();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([changed("a/x.ts"), changed("b/y.ts")]), json!([]))));
        ctx.daemon_answers("git.worktrees", Ok(worktrees));
        assert!(area.read_turn_changes(&e, turn_ask("ina", "/repo/a", Some(&["/repo/a/x.ts"]))).await);
        assert!(area.read_turn_changes(&e, turn_ask("inb", "/repo/b", Some(&["/repo/b/y.ts"]))).await);
        let said = ctx.recorded_json();
        assert_eq!(
            (said[0]["files"].clone(), said[0]["others"].clone(), said[0]["shared"].clone()),
            (json!([bare("a/x.ts")]), json!([bare("b/y.ts")]), json!(true))
        );
        assert_eq!(
            (said[1]["files"].clone(), said[1]["others"].clone(), said[1]["shared"].clone()),
            (json!([bare("b/y.ts")]), json!([bare("a/x.ts")]), json!(true))
        );
    });
}

// TS: turn-changes.test.ts:349
#[test]
fn every_change_is_the_turns_own_where_no_other_thread_ran_in_its_folder_edits_it_never_reported_included() {
    local(async {
        let (ctx, area, e) = te_host();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([changed("a.ts"), changed("dist/out.js")]), json!([]))));
        assert!(area.read_turn_changes(&e, turn_ask("mine", "/repo/a", Some(&["/repo/a/a.ts"]))).await);
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        assert_eq!(
            said[0]["files"],
            json!([bare("a.ts"), bare("dist/out.js")]),
            "the build output the agent never reported stays the turn's own"
        );
        assert_eq!(said[0].get("others"), None);
        assert_eq!(said[0].get("shared"), None);
    });
}

// TS: turn-changes.test.ts:512
#[test]
fn a_folder_git_holds_no_repo_in_takes_no_snapshot_and_reads_no_range() {
    local(async {
        let (ctx, area, e) = te_host();
        ctx.projects.borrow_mut().insert("p1".into(), held_project("p1", None));
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        assert_eq!(area.snapshot_of(&e, "/plain").await, None);
        assert!(!area.read_turn_changes(&e, turn_ask("mine", "/plain", None)).await);
        assert!(ctx.frames.borrow().is_empty(), "no snapshot, no range and no checkpoint is asked of the daemon");
        assert!(ctx.recorded.borrow().is_empty());
        // A folder git does hold is asked.
        ctx.projects.borrow_mut().insert("p1".into(), held_project("p1", Some("/repo")));
        assert_eq!(area.snapshot_of(&e, "/repo").await.as_deref(), Some("snap1"));
        assert_eq!(ctx.frames.borrow().len(), 1);
    });
}

// =====================================================================================================================
// spawn-guard.test.ts, settle-tree.test.ts, waiting-behind.test.ts: the tree a thread's token reaches
// =====================================================================================================================

fn roots(ctx: &FakeThreadsCtx, pairs: &[(&str, &str)]) {
    for (thread, root) in pairs {
        ctx.roots.borrow_mut().insert(thread.to_string(), root.to_string());
    }
}

// TS: spawn-guard.test.ts:498
#[test]
fn a_thread_may_send_into_a_thread_of_its_own_tree_however_deep_it_sits() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("root", "root"), ("child", "root"), ("grand", "root")]);
    let area = ctx.area();
    let root = scoped("root", "ws1", "root");
    assert!(area.drives_thread(Some("root"), Some(&root)), "its own thread");
    assert!(area.drives_thread(Some("grand"), Some(&root)), "however deep");
    let deep = scoped("grand", "ws1", "root");
    assert!(area.drives_thread(Some("child"), Some(&deep)));
    assert!(area.drives_thread(Some("root"), Some(&deep)), "and up to the lead");
}

// TS: spawn-guard.test.ts:1246
#[test]
fn a_threads_own_token_drives_its_thread_and_the_tree_under_it_and_reads_nothing_else_on_its_workspace() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("mine", "mine"), ("other", "other"), ("root", "root"), ("child", "root"), ("grand", "root")]);
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "r-child", "threadId": "child", "parentThreadId": "root" })), "u1"),
            row(view(json!({ "id": "r-grand", "threadId": "grand", "parentThreadId": "child" })), "u2"),
        ])
    });
    let area = ctx.area();
    let lead = scoped("root", "ws1", "root");
    for thread in ["root", "child", "grand"] {
        assert!(area.drives_thread(Some(thread), Some(&lead)), "{thread}");
    }
    for thread in ["mine", "other"] {
        assert!(!area.drives_thread(Some(thread), Some(&lead)), "{thread} is another tree's, which reads as no thread at all");
    }
    for thread in ["mine", "other", "root", "child", "grand"] {
        assert!(area.drives_thread(Some(thread), None), "the person drives every thread");
    }
    assert_eq!(area.tree_under("root"), vec!["child", "grand"]);
}

// TS: spawn-guard.test.ts:1309
#[test]
fn a_notify_target_outside_the_callers_tree_reads_as_no_thread_even_when_the_callers_own_rows_are_gone() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("root", "root"), ("mine", "mine"), ("builder", "root")]);
    let area = ctx.area();
    let ghost = scoped("t_gone", "ws1", "root");
    assert!(
        !area.drives_thread(Some("mine"), Some(&ghost)),
        "a caller whose rows this host no longer holds still reads its root off the scope"
    );
    assert!(area.drives_thread(Some("root"), Some(&ghost)));
    assert!(area.drives_thread(Some("builder"), Some(&ghost)));
    assert!(area.drives_thread(Some("mine"), None), "a person names any thread, as today");
}

// TS: spawn-guard.test.ts:1351
#[test]
fn a_notify_target_may_be_the_parent_a_sibling_or_a_child_but_not_the_persons_own_thread() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("root", "root"), ("child", "root"), ("sibling", "root"), ("grand", "root"), ("mine", "mine")]);
    let area = ctx.area();
    let as_child = scoped("child", "ws1", "root");
    for target in ["root", "sibling", "grand"] {
        assert!(area.drives_thread(Some(target), Some(&as_child)), "{target}");
    }
    assert!(!area.drives_thread(Some("mine"), Some(&as_child)));
}

// TS: spawn-guard.test.ts:1438
#[test]
fn a_child_on_the_copy_its_lead_forked_reaches_the_lead_on_another_workspace_and_the_lead_reaches_it_back() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("lead", "lead"), ("child", "lead")]);
    ctx.sessions.with(|s| {
        *s =
            rows_of(vec![row(view(json!({ "id": "r-child", "threadId": "child", "parentThreadId": "lead", "workspaceId": "copy" })), "u1")])
    });
    let area = ctx.area();
    let as_child = scoped("child", "copy", "lead");
    let as_lead = scoped("lead", "ws1", "lead");
    assert!(
        area.drives_thread(Some("lead"), Some(&as_child)),
        "the scope's workspace is the copy, and the lead's thread is reached all the same"
    );
    assert!(area.drives_thread(Some("child"), Some(&as_lead)));
    assert!(area.settles_thread("child", Some(&as_lead)));
    assert!(!area.settles_thread("lead", Some(&as_child)), "settling is never the lead's");
}

// TS: spawn-guard.test.ts:1471
#[test]
fn the_childs_reach_is_the_thread_alone_the_persons_thread_another_leads_tree_and_a_threadless_row_read_as_absent() {
    let ctx = FakeThreadsCtx::new();
    roots(&ctx, &[("lead", "lead"), ("child", "lead"), ("mine", "mine"), ("other", "other")]);
    let area = ctx.area();
    let as_child = scoped("child", "copy", "lead");
    assert!(!area.drives_thread(Some("mine"), Some(&as_child)));
    assert!(!area.drives_thread(Some("other"), Some(&as_child)));
    assert!(!area.drives_thread(None, Some(&as_child)), "a row with no thread of its own is in nobody's tree");
    assert!(!area.drives_thread(Some("unknown"), Some(&as_child)), "a thread no root is held for");
}

// TS: spawn-guard.test.ts:1535
#[test]
fn a_child_that_stops_its_lead_stops_the_whole_tree_its_sibling_and_itself_included() {
    local(async {
        let ctx = FakeThreadsCtx::new();
        ctx.sessions.with(|s| {
            *s = rows_of(vec![
                row(view(json!({ "id": "r-child", "threadId": "child", "parentThreadId": "root", "status": "running" })), "u1"),
                row(view(json!({ "id": "r-sibling", "threadId": "sibling", "parentThreadId": "root", "status": "running" })), "u2"),
            ])
        });
        let area = ctx.area();
        let stopped = area.stop_under("root", Some(scoped("child", "ws1", "root"))).await;
        let mut named = stopped.clone();
        named.sort();
        assert_eq!(named, vec!["child", "sibling"], "the answer names them");
        assert_eq!(ctx.called(), vec!["interrupt r-sibling", "interrupt r-child"], "deepest first, each through its own row");
    });
}

// TS: spawn-guard.test.ts:1553
#[test]
fn stopping_a_root_ends_every_thread_its_agents_spawned_under_it_deepest_first() {
    local(async {
        let ctx = FakeThreadsCtx::new();
        ctx.sessions.with(|s| {
            *s = rows_of(vec![
                row(view(json!({ "id": "r-root", "threadId": "root", "status": "running" })), "u0"),
                row(view(json!({ "id": "r-child", "threadId": "child", "parentThreadId": "root", "status": "running" })), "u1"),
                row(view(json!({ "id": "r-grand", "threadId": "grand", "parentThreadId": "child", "status": "running" })), "u2"),
            ])
        });
        let area = ctx.area();
        let stopped = area.stop_under("root", None).await;
        assert_eq!(stopped, vec!["grand", "child"]);
        assert_eq!(
            ctx.called(),
            vec!["interrupt r-grand", "interrupt r-child"],
            "the root is the caller's own to stop, so only what spawned under it is asked here"
        );
    });
}

// TS: settle-tree.test.ts:156
#[test]
fn a_threads_own_token_settles_the_threads_under_it_and_is_refused_for_its_lead_and_a_thread_beside_it() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "r-l", "threadId": "L" })), "u0"),
            row(view(json!({ "id": "r-a", "threadId": "A", "parentThreadId": "L" })), "u1"),
            row(view(json!({ "id": "r-a1", "threadId": "A1", "parentThreadId": "A" })), "u2"),
            row(view(json!({ "id": "r-b", "threadId": "B", "parentThreadId": "L" })), "u3"),
            row(view(json!({ "id": "r-c", "threadId": "C", "parentThreadId": "L" })), "u4"),
        ])
    });
    let area = ctx.area();
    assert_eq!(area.tree_under("L"), vec!["A", "B", "C", "A1"], "level by level in the order the rows hold them");
    assert_eq!(area.tree_under("A"), vec!["A1"]);
    let as_a = scoped("A", "ws1", "L");
    assert!(area.settles_thread("A1", Some(&as_a)), "a thread under it");
    assert!(area.settles_thread("A", Some(&as_a)), "itself");
    assert!(!area.settles_thread("L", Some(&as_a)), "its lead's fold is the person's");
    assert!(!area.settles_thread("B", Some(&as_a)), "a thread beside it");
    for thread in ["L", "A", "A1", "B", "C"] {
        assert!(area.settles_thread(thread, None), "the person settles any thread: {thread}");
    }
}

fn prompt_of(ask_id: &str) -> ThreadPrompt {
    serde_json::from_value(json!({
        "askId": ask_id,
        "toolName": "Read",
        "toolUseId": "toolu_read",
        "input": "{\"file_path\":\"/root/hello.txt\"}",
        "detail": "hello.txt",
        "options": [{ "id": "allow", "label": "Allow", "effect": "allow" }, { "id": "deny", "label": "Deny", "effect": "deny" }],
    }))
    .expect("a prompt")
}

fn calls_of(calls: &[(&str, &str, Value)]) -> IndexMap<String, CallSeen> {
    calls.iter().map(|(id, tool, input)| (id.to_string(), CallSeen { tool_name: tool.to_string(), input: input.to_string() })).collect()
}

fn waiting_pair() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>) {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "r-caller", "threadId": "caller01", "status": "running", "prompt": "ask the other one" })), "u1"),
            row(view(json!({ "id": "r-target", "threadId": "target01", "status": "running", "prompt": "read the file" })), "u2"),
        ])
    });
    let area = ctx.area();
    (ctx, area)
}

// TS: waiting-behind.test.ts:109
#[test]
fn a_thread_inside_a_send_that_waits_for_another_thread_carries_that_threads_question() {
    let (_ctx, area) = waiting_pair();
    area.lead_asks.borrow_mut().insert("target01".into(), prompt_of("ask_1"));
    let caller_view = view(json!({ "id": "r-caller", "threadId": "caller01" }));
    let open = CallsOf {
        view: caller_view.clone(),
        calls: Some(calls_of(&[("toolu_send", "mcp__wsp__send", json!({ "thread": "target01", "message": "read hello.txt" }))])),
    };
    let behind = area.stopped_behind(&open).expect("the caller is stopped behind the question");
    assert_eq!(
        (behind.thread_id.as_str(), behind.workspace_id.as_str(), behind.session_id.as_str(), behind.title.as_str()),
        ("target01", "ws1", "r-target", "read the file")
    );
    assert_eq!((behind.prompt.ask_id.as_str(), behind.prompt.tool_name.as_str()), ("ask_1", "Read"));
    assert_eq!(behind.prompt.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>(), vec!["allow", "deny"]);
    // The question is answered off the other thread: once it is, nothing is open and the caller works again.
    area.lead_asks.borrow_mut().shift_remove("target01");
    assert!(area.stopped_behind(&open).is_none());
}

// TS: waiting-behind.test.ts:140
#[test]
fn a_thread_is_behind_another_only_through_a_call_that_waits_for_it_and_never_a_detached_send() {
    let (_ctx, area) = waiting_pair();
    area.lead_asks.borrow_mut().insert("target01".into(), prompt_of("ask_1"));
    let caller_view = view(json!({ "id": "r-caller", "threadId": "caller01" }));
    let detached = CallsOf {
        view: caller_view.clone(),
        calls: Some(calls_of(&[("toolu_send", "mcp__wsp__send", json!({ "thread": "target01", "message": "go", "detach": true }))])),
    };
    assert!(area.stopped_behind(&detached).is_none(), "a detached send answers at once");
    let own =
        CallsOf { view: caller_view.clone(), calls: Some(calls_of(&[("toolu_read", "Read", json!({ "file_path": "/root/notes.md" }))])) };
    assert!(area.stopped_behind(&own).is_none(), "a call of the agent's own, into no thread at all, is no wait");
}

// TS: waiting-behind.test.ts:156
#[test]
fn the_wait_is_dropped_when_the_call_the_thread_was_inside_answers_whatever_the_other_thread_is_doing() {
    let (_ctx, area) = waiting_pair();
    area.lead_asks.borrow_mut().insert("target01".into(), prompt_of("ask_1"));
    let caller_view = view(json!({ "id": "r-caller", "threadId": "caller01" }));
    let waiting = CallsOf {
        view: caller_view.clone(),
        calls: Some(calls_of(&[("toolu_wait", "mcp__wsp__threads_wait", json!({ "threads": ["target01"], "timeout": 60 }))])),
    };
    assert_eq!(area.stopped_behind(&waiting).map(|b| b.thread_id), Some("target01".to_string()));
    // The wait gave up on its deadline: the call answered, so it is off the row while the question still stands.
    let answered = CallsOf { view: caller_view, calls: Some(IndexMap::new()) };
    assert!(area.stopped_behind(&answered).is_none());
    assert!(area.lead_asks.borrow().contains_key("target01"), "the thread that was asked still is");
}
