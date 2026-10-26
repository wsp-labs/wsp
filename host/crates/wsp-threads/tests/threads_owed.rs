// SPDX-License-Identifier: AGPL-3.0-only
//! threads_owed (spec/threads_owed.md): lines into threads, kept until a turn takes them, and the cut end. Expected
//! values read off threads.ts:24-62, 586-955 at 65f6b89c7 and owed-lines.test.ts, runtime-notify.test.ts,
//! steer-unread.test.ts.
#[path = "support_b/mod.rs"]
mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::future::{FutureExt, LocalBoxFuture, Shared};
use serde_json::{json, Value};
use support::threads_ctx::{entry, FakeThreadsCtx};
use support::Lend;
use support::{err, local, row, settle, silent, view};
use wsp_core::types::internal::{Steered, Taken, TakenOutcome, TurnAsked, TurnLive};
use wsp_core::types::wiring::{SessionHandle, SteerTaken};
use wsp_threads::threads::threads::tree::TellAs;
use wsp_threads::threads::threads::{read_owed, Owed, OwedImages, PersonRow, Row, ThreadsArea};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::exit::HostError;
use wsp_words::words::turn::{notify_line, unread_line, NotifyLength};

const FROM: &str = "child0001";
const PARENT: &str = "parent0001";
const HOUR: i64 = 3_600_000;

// ---- a handle a start answers --------------------------------------------------------------------------------------

struct FakeHandle {
    id: String,
    turn: String,
    outcome: SessionStartOutcome,
    finished: Shared<LocalBoxFuture<'static, TurnResult>>,
    tells_unread: Option<bool>,
}

/// A handle whose finish the test settles through the answered sender.
fn handle(
    id: &str,
    turn: &str,
    outcome: SessionStartOutcome,
    tells_unread: Option<bool>,
) -> (Rc<dyn SessionHandle>, Rc<RefCell<Option<futures::channel::oneshot::Sender<TurnResult>>>>) {
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
        self.outcome
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

fn finish(tx: &Rc<RefCell<Option<futures::channel::oneshot::Sender<TurnResult>>>>, r: TurnResult) {
    let _ = tx.borrow_mut().take().expect("finished once").send(r);
}

// ---- the host -----------------------------------------------------------------------------------------------------

/// A host whose parent thread has a settled row with an agent session on a running workspace `ws1`.
fn host() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>) {
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

fn scope(thread: &str) -> wsp_wire::views::workspace_more::ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": "ws1", "rootThreadId": thread })).expect("a scope")
}

fn here() -> wsp_wire::views::workspace_more::WorkspaceOrigin {
    serde_json::from_value(json!("here")).expect("an origin")
}

/// The person's line for an end, as tellPerson records it.
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

// ---- O1 -----------------------------------------------------------------------------------------------------------

#[test]
fn read_owed_needs_the_four_strings_and_drops_a_field_that_does_not_read() {
    assert!(read_owed(&json!({ "id": "a", "from": "b", "notify": "c" })).is_none(), "no text");
    assert!(read_owed(&json!({ "id": 1, "from": "b", "notify": "c", "text": "t" })).is_none());
    assert!(read_owed(&json!(null)).is_none());
    let read = read_owed(&json!({
        "id": "a", "from": "b", "notify": "c", "text": "t",
        "since": "yesterday", "tries": 2, "next": 9, "requestId": 7, "road": "paired",
        "toPerson": { "workspaceId": "w", "sessionId": "s", "turnId": "u" },
        "images": { "under": "req", "records": [] },
        "awaitsResult": "ws-c", "startedBy": "agent"
    }))
    .expect("the four strings read");
    assert_eq!((read.id.as_str(), read.from.as_str(), read.notify.as_str(), read.text.as_str()), ("a", "b", "c", "t"));
    assert_eq!(read.since, None, "a since that is no number is dropped");
    assert_eq!(read.tries, Some(2));
    assert_eq!(read.next, Some(9));
    assert_eq!(read.request_id, None, "a request id that is no string is dropped");
    assert_eq!(read.road, Some(serde_json::from_value(json!("paired")).expect("an origin")));
    assert_eq!(read.to_person, None, "a person row missing its text is dropped");
    assert_eq!(read.images, None, "an empty list of images is none");
    assert_eq!(read.awaits_result.as_deref(), Some("ws-c"));
    assert_eq!(read.started_by, Some(serde_json::from_value(json!("agent")).expect("an origin")));
}

// ---- O2, O11 ------------------------------------------------------------------------------------------------------

#[test]
fn deliver_writes_since_and_request_before_the_first_try_and_takes_the_line_once_started_on() {
    local(async {
        let (ctx, area) = host();
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver(line("L1"), false);
        settle().await;
        let kept = ctx.store.get_now("notify-owed", "L1").expect("kept before the first try");
        assert_eq!(kept["since"], json!(1_000_000));
        assert_eq!(kept["requestId"], json!("uuid-1"));
        assert_eq!(ctx.store.logged()[0], "put notify-owed/L1", "the write goes before the start");
        {
            let started = ctx.started.borrow();
            assert_eq!(started.len(), 1);
            let (ws, o, caller) = &started[0];
            assert_eq!(ws, "ws1");
            assert_eq!(o.prompt, "thread child000 finished (completed): the report");
            assert_eq!(o.harness.as_deref(), Some("claude"));
            assert_eq!(o.thread.as_deref(), Some(PARENT));
            assert_eq!(o.started_by, Some(serde_json::from_value(json!("agent")).expect("an origin")));
            assert_eq!(o.request_id.as_deref(), Some("uuid-1"));
            assert!(o.owed && o.asks_until_stopped && o.wakes_lead);
            assert!(o.attachments.is_none());
            assert!(caller.is_none(), "a line nobody but the person registered goes as theirs");
        }
        ctx.taken.borrow_mut().insert(
            ("ws1".into(), "uuid-1".into()),
            Taken { session_id: "r-try".into(), thread_id: PARENT.into(), turn_id: "u-try".into(), outcome: TakenOutcome::Started },
        );
        area.prompt_held("uuid-1");
        settle().await;
        assert!(ctx.store.get_now("notify-owed", "L1").is_none(), "taken: the line leaves the store");
        assert!(area.sending.borrow().is_empty());
        assert!(ctx.called().contains(&format!("mark {FROM} 1000000")), "a lead that took a child's line has read the child");
        assert!(told_person(&ctx.recorded_json()).is_empty(), "a taken line tells the person nothing");
    });
}

#[test]
fn deliver_to_a_thread_with_no_session_falls_to_the_person() {
    local(async {
        let (ctx, area) = host();
        ctx.sessions.with(|s| s.get("r-p").expect("row").borrow_mut().view.claude_session_id = None);
        let warned = wsp_core::console::capture();
        area.deliver(line("L1"), false);
        settle().await;
        assert_eq!(warned.lines(), vec!["thread child000 ended, but thread parent00 has no session to tell"]);
        assert!(ctx.started.borrow().is_empty());
        assert!(ctx.store.get_now("notify-owed", "L1").is_none(), "the line fell and left the store");
        let told = told_person(&ctx.recorded_json());
        assert_eq!(told.len(), 1);
        for (k, v) in [
            ("workspaceId", "ws-c"),
            ("sessionId", "cs-c"),
            ("turnId", "u-c"),
            ("threadId", FROM),
            ("text", "thread child000 finished (completed)"),
        ] {
            assert_eq!(told[0][k], json!(v), "{k}");
        }
    });
}

// ---- O4, O18 ------------------------------------------------------------------------------------------------------

#[test]
fn a_line_into_a_napping_workspace_is_held_and_goes_at_the_wake() {
    local(async {
        let (ctx, area) = host();
        set_phase(&ctx, "ws1", "napping");
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver(line("L1"), false);
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "held for the wake");
        assert_eq!(area.held_lines.borrow().get("ws1").map(|l| l.len()), Some(1));
        assert!(area.sending.borrow().contains_key("L1"));
        set_phase(&ctx, "ws1", "running");
        ctx.bus.emit_json(json!({ "type": "workspace.woken", "workspaceId": "ws1", "machineId": "m1" }));
        settle().await;
        assert_eq!(ctx.started.borrow().len(), 1, "the wake sends it");
        assert!(area.held_lines.borrow().get("ws1").is_none());
    });
}

#[test]
fn a_held_line_goes_at_a_rebuild_too() {
    local(async {
        let (ctx, area) = host();
        set_phase(&ctx, "ws1", "gone");
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver(line("L1"), false);
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "a gone workspace holds the line too");
        set_phase(&ctx, "ws1", "running");
        ctx.bus.emit_json(json!({ "type": "workspace.upgraded", "workspaceId": "ws9", "machineId": "m9" }));
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "another workspace's rebuild sends nothing");
        ctx.bus.emit_json(json!({ "type": "workspace.upgraded", "workspaceId": "ws1", "machineId": "m2" }));
        settle().await;
        assert_eq!(ctx.started.borrow().len(), 1);
    });
}

// ---- O5, O6 -------------------------------------------------------------------------------------------------------

#[test]
fn a_line_registered_from_a_paired_device_falls() {
    local(async {
        let (ctx, area) = host();
        let warned = wsp_core::console::capture();
        let mut l = line("L1");
        l.road = Some(serde_json::from_value(json!("paired")).expect("an origin"));
        area.deliver(l, false);
        settle().await;
        assert_eq!(
            warned.lines(),
            vec!["thread child000 ended, but its line was registered from a paired computer, which starts no turn on thread parent00"]
        );
        assert!(ctx.started.borrow().is_empty());
        assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
    });
}

#[test]
fn the_start_runs_as_the_thread_that_named_the_target_and_its_road() {
    local(async {
        let (ctx, area) = host();
        for _ in 0..2 {
            let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
            ctx.starts.borrow_mut().push_back(Ok(h));
        }
        let mut scoped = line("L1");
        scoped.by = Some(scope("lead"));
        area.deliver(scoped, false);
        let mut road_only = line("L2");
        road_only.road = Some(here());
        area.deliver(road_only, false);
        settle().await;
        let started = ctx.started.borrow();
        assert_eq!(started[0].2, Some(Caller::Scoped { origin: here(), by: scope("lead") }), "a scope with no road goes from here");
        assert_eq!(started[1].2, Some(Caller::Road(here())));
    });
}

// ---- O7.2 ---------------------------------------------------------------------------------------------------------

#[test]
fn a_sub_threads_line_waits_for_its_done_and_carries_its_tail() {
    local(async {
        let (ctx, area) = host();
        ctx.sub_lines.borrow_mut().insert("ws-c".into(), Some("pushed abc123".into()));
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let mut l = line("L1");
        l.awaits_result = Some("ws-c".into());
        area.deliver(l, false);
        settle().await;
        assert!(ctx.called().contains(&"subLine ws-c 1000000".to_string()));
        assert_eq!(ctx.started.borrow()[0].1.prompt, "thread child000 finished (completed): the report\npushed abc123");
        let kept = ctx.store.get_now("notify-owed", "L1").expect("kept");
        assert_eq!(kept["text"], json!("thread child000 finished (completed): the report\npushed abc123"));
        assert_eq!(kept.get("awaitsResult"), None, "the done read is on the line now");
    });
}

// ---- O7.3 ---------------------------------------------------------------------------------------------------------

#[test]
fn a_line_with_a_request_asks_the_turn_already_carrying_it() {
    local(async {
        let (ctx, area) = host();
        let (h, _tx) = handle("r-run", "u-run", SessionStartOutcome::Started, None);
        ctx.sessions.with(|s| {
            let r = row(
                view(json!({ "id": "r-run", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "running", "startedAt": 5 })),
                "u-run",
            );
            r.borrow_mut().asked = Some(TurnAsked {
                prompt: "x".into(),
                effort: None,
                typed: None,
                request_id: Some("req-c".into()),
                owed: Some(true),
                awaits_prompt: None,
            });
            r.borrow_mut().handle = Some(h);
            s.insert("r-run".into(), r);
        });
        let mut l = line("L1");
        l.since = Some(1_000_000 - 60_000);
        l.request_id = Some("req-c".into());
        area.deliver(l, false);
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "the try out is settled, not sent again");
        assert!(ctx.store.logged().is_empty(), "a line that had its request and since is not written again");
        ctx.taken.borrow_mut().insert(
            ("ws1".into(), "req-c".into()),
            Taken { session_id: "r-run".into(), thread_id: PARENT.into(), turn_id: "u-run".into(), outcome: TakenOutcome::Started },
        );
        area.prompt_held("req-c");
        settle().await;
        assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"]);
    });
}

#[test]
fn a_steered_line_whose_turn_ended_unread_and_cannot_say_falls() {
    for (tells, falls) in [(Some(false), true), (Some(true), false)] {
        local(async move {
            let (ctx, area) = host();
            let (h, tx) = handle("r-run", "u-run", SessionStartOutcome::Started, tells);
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
            assert!(ctx.started.borrow().is_empty());
            assert_eq!(ctx.store.logged(), vec!["delete notify-owed/L1"], "the line leaves the store either way");
            if falls {
                assert_eq!(warned.lines(), vec!["thread child000 ended, and nothing says whether thread parent00 read its line"]);
                assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
            } else {
                assert!(warned.lines().is_empty(), "the turn sends back what it never read, so the line is taken");
                assert!(told_person(&ctx.recorded_json()).is_empty());
            }
        });
    }
}

// ---- O7.4, O22 ----------------------------------------------------------------------------------------------------

#[test]
fn a_restarted_line_keeps_its_spacing_and_its_hour() {
    local(async {
        let (ctx, area) = host();
        let (h, _tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let mut spaced = line("L1");
        spaced.since = Some(1_000_000 - 600_000);
        spaced.request_id = Some("req-1".into());
        spaced.tries = Some(2);
        spaced.next = Some(1_005_000);
        area.deliver(spaced, false);
        settle().await;
        assert!(ctx.started.borrow().is_empty(), "the next try waits for its time");
        ctx.clock.advance(4_999);
        settle().await;
        assert!(ctx.started.borrow().is_empty());
        ctx.clock.advance(1);
        settle().await;
        assert_eq!(ctx.started.borrow().len(), 1, "at next, the same request goes again");
        assert_eq!(ctx.started.borrow()[0].1.request_id.as_deref(), Some("req-1"));

        let warned = wsp_core::console::capture();
        let mut late = line("L2");
        late.since = Some(wsp_core::clock::Clock::now(&*ctx.clock) - HOUR - 1);
        late.request_id = Some("req-2".into());
        late.tries = Some(1);
        area.deliver(late, false);
        settle().await;
        assert_eq!(
            warned.lines(),
            vec!["thread child000 ended, but its line did not reach thread parent00 within 60 minutes: its last try failed"]
        );
        assert!(ctx.store.logged().contains(&"delete notify-owed/L2".to_string()));
    });
}

// ---- O7.5 ---------------------------------------------------------------------------------------------------------

#[test]
fn a_steered_start_takes_the_line() {
    local(async {
        let (ctx, area) = host();
        let (h, _tx) = handle("r-p", "u-p2", SessionStartOutcome::Steered, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver(line("L1"), false);
        settle().await;
        assert_eq!(ctx.store.logged(), vec!["put notify-owed/L1", "delete notify-owed/L1"]);
        assert!(ctx.called().iter().any(|c| c.starts_with("mark child0001")));
    });
}

#[test]
fn a_refused_start_falls_with_its_words() {
    local(async {
        let (ctx, area) = host();
        ctx.starts.borrow_mut().push_back(Err(err("thread parent00's workspace takes no turn now")));
        let warned = wsp_core::console::capture();
        area.deliver(line("L1"), false);
        settle().await;
        assert_eq!(
            warned.lines(),
            vec!["thread child000 ended, but its line did not reach thread parent00: thread parent00's workspace takes no turn now"]
        );
        assert!(ctx.store.get_now("notify-owed", "L1").is_none());
        assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
    });
}

// ---- O8 -----------------------------------------------------------------------------------------------------------

#[test]
fn a_try_stopped_at_the_hour_gives_up() {
    local(async {
        let (ctx, area) = host();
        let (h, tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let mut l = line("L1");
        l.since = Some(1_000_000 - HOUR + 60_000);
        area.deliver(l, false);
        settle().await;
        assert!(ctx.started.borrow()[0].1.asks_until_stopped, "time is left, so the try asks until stopped");
        let warned = wsp_core::console::capture();
        ctx.clock.advance(60_000);
        settle().await;
        assert!(ctx.called().contains(&"stopTry r-try".to_string()), "the hour stops the try");
        finish(&tx, result("interrupted", None));
        settle().await;
        assert_eq!(warned.lines(), vec!["thread child000 ended, but its line did not reach thread parent00 within 60 minutes: the turn ended interrupted before its agent started"]);
        assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
    });
}

#[test]
fn a_dark_try_is_settled_at_the_hour_without_its_turn() {
    local(async {
        let (ctx, area) = host();
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
        l.since = Some(1_000_000 - HOUR + 30_000);
        l.request_id = Some("req-d".into());
        area.deliver(l, false);
        settle().await;
        let warned = wsp_core::console::capture();
        ctx.clock.advance(30_000);
        settle().await;
        assert!(!ctx.called().iter().any(|c| c.starts_with("stopTry")), "nothing reaches a dark turn to stop it");
        assert_eq!(warned.lines(), vec!["thread child000 ended, but its line did not reach thread parent00 within 60 minutes: nothing answered for the turn carrying it since the host restarted"]);
        assert!(ctx.started.borrow().is_empty());
        assert!(ctx.store.logged().contains(&"delete notify-owed/L1".to_string()));
    });
}

#[test]
fn a_try_the_person_stopped_falls() {
    local(async {
        let (ctx, area) = host();
        let (h, tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver(line("L1"), false);
        settle().await;
        let warned = wsp_core::console::capture();
        finish(&tx, result("interrupted", None));
        settle().await;
        assert_eq!(warned.lines(), vec!["thread child000 ended, but the turn carrying its line into thread parent00 was stopped"]);
        assert!(ctx.store.get_now("notify-owed", "L1").is_none());
        assert_eq!(told_person(&ctx.recorded_json()).len(), 1);
    });
}

#[test]
fn a_try_that_failed_silent_goes_again() {
    local(async {
        let (ctx, area) = host();
        let (h, tx) = handle("r-try", "u-try", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
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
        finish(&tx, result("failed", Some("the launch went unanswered")));
        settle().await;
        assert_eq!(
            warned.lines(),
            vec![
                "thread child000 ended, but its line did not reach thread parent00 yet, and goes again in 30 s: the launch went unanswered"
            ]
        );
        let kept = ctx.store.get_now("notify-owed", "L1").expect("still owed");
        assert_eq!((kept["tries"].clone(), kept["next"].clone()), (json!(1), json!(1_030_000)));
        assert!(told_person(&ctx.recorded_json()).is_empty());
    });
}

// ---- O9 -----------------------------------------------------------------------------------------------------------

#[test]
fn retries_double_from_thirty_seconds_to_ten_minutes_within_the_hour() {
    local(async {
        let (ctx, area) = host();
        for _ in 0..12 {
            ctx.starts.borrow_mut().push_back(Err(silent("the computer did not answer", "MachineUnreached")));
        }
        let warned = wsp_core::console::capture();
        area.deliver(line("L1"), false);
        settle().await;
        let waits = [30, 60, 120, 240, 480, 600, 600, 600, 600, 270];
        for w in waits {
            ctx.clock.advance(w * 1000);
            settle().await;
        }
        let lines = warned.lines();
        let said: Vec<String> = lines
            .iter()
            .filter(|l| l.contains("goes again in"))
            .map(|l| l.split("goes again in ").nth(1).and_then(|r| r.split(' ').next()).unwrap_or("").to_string())
            .collect();
        assert_eq!(said, waits.iter().map(|w| w.to_string()).collect::<Vec<_>>());
        assert_eq!(
            ctx.started.borrow().len(),
            10,
            "a try at 0, 30, 90, 210, 450, 930, 1530, 2130, 2730 and 3330 s; at 3600 s the hour is up"
        );
        assert_eq!(
            lines.last().map(String::as_str),
            Some("thread child000 ended, but its line did not reach thread parent00 within 60 minutes: its last try failed")
        );
        assert!(ctx.store.get_now("notify-owed", "L1").is_none());
    });
}

#[test]
fn the_retry_wait_prints_seconds_as_javascript_does() {
    local(async {
        let (ctx, area) = host();
        ctx.starts.borrow_mut().push_back(Err(silent("no answer", "DeadlineError")));
        let warned = wsp_core::console::capture();
        let mut l = line("L1");
        l.since = Some(1_000_000 - HOUR + 12_345);
        area.deliver(l, false);
        settle().await;
        assert_eq!(
            warned.lines(),
            vec!["thread child000 ended, but its line did not reach thread parent00 yet, and goes again in 12.345 s: no answer"]
        );
    });
}

// ---- O10, O12 -----------------------------------------------------------------------------------------------------

#[test]
fn a_fallen_line_tells_the_person_once_per_end() {
    local(async {
        let (ctx, area) = host();
        set_phase(&ctx, "ws1", "napping");
        area.deliver(line("L1"), false);
        let mut other = line("L2");
        other.notify = "gone0001".into();
        area.deliver(other, false);
        settle().await;
        let told = told_person(&ctx.recorded_json());
        assert_eq!(told.len(), 1, "L2 fell and told the person");
        let held = ctx.store.get_now("notify-owed", "L1").expect("L1 is still owed");
        assert_eq!(held.get("toPerson"), None, "and L1 no longer carries that row, so no host tells it twice");
        assert_eq!(area.sending.borrow().get("L1").map(|l| l.borrow().to_person.clone()), Some(None));
    });
}

// ---- O13 ----------------------------------------------------------------------------------------------------------

#[test]
fn took_request_reads_the_index_and_a_prompt_not_yet_held() {
    let (ctx, area) = host();
    assert!(!area.took_request("ws1", "req-x"));
    ctx.taken.borrow_mut().insert(
        ("ws1".into(), "req-s".into()),
        Taken { session_id: "r".into(), thread_id: PARENT.into(), turn_id: "u-s".into(), outcome: TakenOutcome::Steered },
    );
    assert!(area.took_request("ws1", "req-s"));
    ctx.taken.borrow_mut().insert(
        ("ws1".into(), "req-t".into()),
        Taken { session_id: "r".into(), thread_id: PARENT.into(), turn_id: "u-t".into(), outcome: TakenOutcome::Started },
    );
    assert!(area.took_request("ws1", "req-t"));
    ctx.sessions.with(|s| {
        let r = row(view(json!({ "id": "r-t", "threadId": PARENT })), "u-t");
        r.borrow_mut().asked = Some(TurnAsked {
            prompt: "x".into(),
            effort: None,
            typed: None,
            request_id: Some("req-t".into()),
            owed: None,
            awaits_prompt: Some(true),
        });
        s.insert("r-t".into(), r);
    });
    assert!(!area.took_request("ws1", "req-t"), "an agent that announced itself before it holds the prompt has not taken it");
}

#[test]
fn line_try_is_the_lines_own_turn_until_its_agent_holds_it() {
    let (ctx, area) = host();
    let asked = |owed: Option<bool>, awaits: Option<bool>| TurnAsked {
        prompt: "x".into(),
        effort: None,
        typed: None,
        request_id: Some("req-l".into()),
        owed,
        awaits_prompt: awaits,
    };
    ctx.sessions.with(|s| {
        let r = row(view(json!({ "id": "r-l", "threadId": PARENT, "workspaceId": "ws1" })), "u-l");
        r.borrow_mut().asked = Some(asked(Some(true), None));
        s.insert("r-l".into(), r);
        let plain = row(view(json!({ "id": "r-plain", "threadId": PARENT })), "u-plain");
        plain.borrow_mut().asked = Some(asked(None, None));
        s.insert("r-plain".into(), plain);
    });
    assert!(area.line_try("r-l"), "no turn took the request yet");
    assert!(!area.line_try("r-plain"), "a turn carrying no line");
    ctx.taken.borrow_mut().insert(
        ("ws1".into(), "req-l".into()),
        Taken { session_id: "r-l".into(), thread_id: PARENT.into(), turn_id: "u-l".into(), outcome: TakenOutcome::Started },
    );
    assert!(!area.line_try("r-l"), "its own turn took it");
    ctx.sessions.with(|s| s.get("r-l").expect("row").borrow_mut().asked = Some(asked(Some(true), Some(true))));
    assert!(area.line_try("r-l"), "until its agent holds the prompt");
}

// ---- O15 ----------------------------------------------------------------------------------------------------------

#[test]
fn images_go_back_with_the_line_and_a_missing_one_is_left_out() {
    local(async {
        let (ctx, area) = host();
        ctx.store.seed_blob("attachments", &format!("{PARENT}.req-s.0"), &[1, 2, 3]);
        let records = serde_json::from_value(
            json!([{ "mediaType": "image/png", "bytes": 3, "name": "a.png" }, { "mediaType": "image/png", "bytes": 9 }]),
        )
        .expect("records");
        let images = area.kept_images("ws1", PARENT, &OwedImages { under: "req-s".into(), records }).await;
        assert_eq!(support::json_of(&images), json!([{ "mediaType": "image/png", "bytes": "AQID", "name": "a.png" }]));
    });
}

// ---- O17 ----------------------------------------------------------------------------------------------------------

fn steered_row() -> Row {
    let steered: Steered = serde_json::from_value(json!({ "prompt": "\n  look at this  \nmore", "startedBy": "person", "requestId": "req-s", "attachments": [{ "mediaType": "image/png", "bytes": 3 }] })).expect("a steer");
    let r = row(view(json!({ "id": "r-t", "workspaceId": "ws1", "threadId": PARENT, "claudeSessionId": "cs-p" })), "u-t");
    r.borrow_mut().turn_live = Some(TurnLive { steered: Some([("st1".to_string(), steered)].into_iter().collect()), ..Default::default() });
    r
}

#[test]
fn send_back_redelivers_an_unread_steer_as_its_starter() {
    local(async {
        let (ctx, area) = host();
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.send_back(&steered_row(), &["st1".to_string(), "never-kept".to_string()], false);
        settle().await;
        let kept = ctx.store.get_now("notify-owed", "u-t:unread:st1").expect("kept as a line");
        assert_eq!(kept["from"], json!(PARENT));
        assert_eq!(kept["notify"], json!(PARENT));
        assert_eq!(kept["text"], json!("\n  look at this  \nmore"));
        assert_eq!(kept["startedBy"], json!("person"));
        assert_eq!(
            kept["toPerson"],
            json!({ "workspaceId": "ws1", "sessionId": "cs-p", "turnId": "u-t", "text": "the agent never read this message: look at this" })
        );
        assert_eq!(kept["images"], json!({ "under": "req-s", "records": [{ "mediaType": "image/png", "bytes": 3 }] }));
        let started = ctx.started.borrow();
        assert_eq!(started.len(), 1, "an id the row never kept goes nowhere");
        assert_eq!(started[0].1.started_by, Some(serde_json::from_value(json!("person")).expect("an origin")));
        assert!(!started[0].1.wakes_lead, "a steered message is no child's report");
    });
}

#[test]
fn send_back_of_a_stopped_turn_tells_the_person() {
    local(async {
        let (ctx, area) = host();
        area.send_back(&steered_row(), &["st1".to_string()], true);
        settle().await;
        assert!(ctx.started.borrow().is_empty());
        assert!(ctx.store.logged().is_empty());
        let told = ctx.recorded_json();
        assert_eq!(told.len(), 1);
        for (k, v) in [
            ("type", "session.notify"),
            ("workspaceId", "ws1"),
            ("sessionId", "cs-p"),
            ("turnId", "u-t"),
            ("threadId", PARENT),
            ("notify", "me"),
            ("text", "the agent never read this message: look at this"),
        ] {
            assert_eq!(told[0][k], json!(v), "{k}");
        }
    });
}

#[test]
fn send_back_leaves_a_steer_whose_write_is_out_to_that_road() {
    local(async {
        let (ctx, area) = host();
        area.steers_out.borrow_mut().insert("st1".into(), Default::default());
        area.send_back(&steered_row(), &["st1".to_string()], true);
        settle().await;
        assert!(ctx.recorded.borrow().is_empty() && ctx.started.borrow().is_empty());
        let out = area.steers_out.borrow();
        let ended = out.get("st1").and_then(|o| o.ended.clone()).expect("marked ended");
        assert_eq!((ended.0.borrow().turn_id.as_str(), ended.1), ("u-t", true));
    });
}

// ---- O19 ----------------------------------------------------------------------------------------------------------

fn child_row(claude: Option<&str>) -> Row {
    let mut v = json!({ "id": "r-c", "workspaceId": "ws-c", "threadId": FROM });
    if let Some(c) = claude {
        v["claudeSessionId"] = json!(c);
    }
    row(view(v), "u-c")
}

#[test]
fn notify_end_records_a_line_per_target_and_delivers_the_whole_report() {
    local(async {
        let (ctx, area) = host();
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let done = result("completed", None);
        area.notify_end(
            &child_row(Some("cs-c")),
            &["me".to_string(), PARENT.to_string()],
            &TellAs { by: Some(scope("lead")), road: None },
            &done,
            false,
        );
        settle().await;
        let tail = notify_line(FROM, &done, Some(NotifyLength::Tail));
        let whole = notify_line(FROM, &done, Some(NotifyLength::Whole));
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 2);
        assert_eq!((said[0]["notify"].clone(), said[0]["text"].clone()), (json!("me"), json!(tail)));
        assert_eq!((said[1]["notify"].clone(), said[1]["text"].clone()), (json!(PARENT), json!(whole)));
        for e in &said {
            assert_eq!(
                (e["workspaceId"].clone(), e["sessionId"].clone(), e["turnId"].clone(), e["threadId"].clone()),
                (json!("ws-c"), json!("cs-c"), json!("u-c"), json!(FROM))
            );
        }
        let kept = ctx.store.get_now("notify-owed", &format!("u-c:0:{PARENT}")).expect("the line is owed");
        assert_eq!(kept["text"], json!(whole));
        assert_eq!(kept["by"], support::json_of(&scope("lead")));
        assert_eq!(kept.get("toPerson"), None, "the person was told directly");
        assert_eq!(kept.get("awaitsResult"), None, "no sub-thread");
    });
}

#[test]
fn notify_end_adds_me_when_a_target_has_gone() {
    local(async {
        let (ctx, area) = host();
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        let done = result("completed", None);
        area.notify_end(&child_row(Some("cs-c")), &[PARENT.to_string(), "gone0001".to_string()], &TellAs::default(), &done, false);
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(
            said.iter().map(|e| e["notify"].clone()).collect::<Vec<_>>(),
            vec![json!(PARENT), json!("me")],
            "the reachable target, then the person once"
        );
        assert!(ctx.store.get_now("notify-owed", "u-c:0:gone0001").is_none(), "nothing goes to a thread that has gone");
        assert!(ctx.store.get_now("notify-owed", &format!("u-c:0:{PARENT}")).is_some());
    });
}

#[test]
fn notify_end_names_the_agents_session_else_the_row() {
    let (ctx, area) = host();
    area.notify_end(&child_row(None), &["me".to_string()], &TellAs::default(), &result("failed", Some("x")), false);
    assert_eq!(ctx.recorded_json()[0]["sessionId"], json!("r-c"));
}

// ---- O20 ----------------------------------------------------------------------------------------------------------

#[test]
fn deliver_owed_sends_every_readable_line_not_already_carried() {
    local(async {
        let (ctx, area) = host();
        set_phase(&ctx, "ws1", "napping");
        area.deliver(line("L-held"), false);
        settle().await;
        set_phase(&ctx, "ws1", "running");
        ctx.store.seed("notify-owed", "L-bad", json!({ "id": "L-bad", "from": FROM, "notify": PARENT }));
        ctx.store.seed("notify-owed", "L-new", serde_json::to_value(line("L-new")).expect("a line"));
        let (h, _tx) = handle("r-n", "u-n", SessionStartOutcome::Started, None);
        ctx.starts.borrow_mut().push_back(Ok(h));
        area.deliver_owed().await.expect("listed");
        settle().await;
        assert_eq!(ctx.started.borrow().len(), 1, "only the readable line no delivery carries");
        assert!(area.sending.borrow().contains_key("L-new"));
        ctx.store.fail_list.set(true);
        assert!(area.deliver_owed().await.is_err(), "a store that fails to list fails the call");
    });
}

// ---- O21 ----------------------------------------------------------------------------------------------------------

fn running_row(ctx: &FakeThreadsCtx, reply: Option<&str>, notify: Option<Vec<&str>>) {
    ctx.sessions.with(|s| {
        let r = row(view(json!({ "id": "r1", "workspaceId": "ws1", "threadId": "t1", "status": "running", "asking": "may I?" })), "u1");
        r.borrow_mut().snapshot = Some("snap".into());
        r.borrow_mut().notify = notify.map(|n| n.into_iter().map(String::from).collect());
        r.borrow_mut().turn_live =
            reply.map(|st| TurnLive { reply: Some(serde_json::from_value(json!(st)).expect("a status")), ..Default::default() });
        s.insert("r1".into(), r);
    });
}

#[test]
fn settle_cut_changes_the_row_it_is_handed() {
    let (ctx, area) = host();
    running_row(&ctx, None, None);
    area.lead_asks.borrow_mut().insert(
        "t1".into(),
        serde_json::from_value(json!({ "askId": "a", "toolName": "Bash", "input": "{}", "options": [] })).expect("a prompt"),
    );
    area.settle_cut("r1", "machine paused while the agent was working", &|at| format!("cut at {at}"), false, false);
    let (status, ended, asking, snapshot, last_line, failure) = ctx.sessions.with(|s| {
        let r = s.get("r1").expect("row").borrow();
        (
            support::json_of(&r.view.status),
            support::json_of(&r.view.ended_at),
            r.view.asking.clone(),
            r.snapshot.clone(),
            r.view.last_line.clone(),
            r.view.failure.clone(),
        )
    });
    assert_eq!(status, json!("failed"));
    assert_eq!(ended, json!(1_000_000));
    assert_eq!((asking, snapshot), (None, None));
    let lines = wsp_words::words::turn::turn_lines(&result("failed", None), Some("machine paused while the agent was working"));
    assert_eq!((last_line, failure), (lines.last_line, lines.failure));
    assert!(area.lead_asks.borrow().get("t1").is_none(), "the prompt goes with the process");
    assert_eq!(ctx.called(), vec!["portRootsMoved ws1", "endSnoozeFor r1"]);
    let said = ctx.recorded_json();
    assert_eq!(said.len(), 1);
    for (k, v) in [
        ("type", json!("session.end")),
        ("workspaceId", json!("ws1")),
        ("sessionId", json!("r1")),
        ("turnId", json!("u1")),
        ("threadId", json!("t1")),
        ("exitCode", Value::Null),
        ("sawResult", json!(false)),
        ("reason", json!("machine paused while the agent was working")),
    ] {
        assert_eq!(said[0][k], v, "{k}");
    }
    assert_eq!(said[0].get("promptless"), None);
}

#[test]
fn settle_cut_of_a_replied_turn_keeps_the_reply_and_sends_no_line() {
    let (ctx, area) = host();
    running_row(&ctx, Some("completed"), Some(vec!["me"]));
    area.settle_cut("r1", "host restarted while the agent was working", &|_| "never read".into(), false, false);
    assert_eq!(ctx.sessions.with(|s| support::json_of(&s.get("r1").expect("row").borrow().view.status)), json!("completed"));
    assert_eq!(ctx.sessions.with(|s| s.get("r1").expect("row").borrow().view.last_line.clone()), None, "a reply keeps its lines");
    let said = ctx.recorded_json();
    assert_eq!(said.len(), 1, "no notify line and no done: the reply's line already went");
    assert_eq!(said[0]["sawResult"], json!(true));
    assert!(!ctx.called().iter().any(|c| c.starts_with("endSnoozeFor")));
}

#[test]
fn settle_cut_of_a_stopped_turn_records_its_done_before_its_end() {
    let (ctx, area) = host();
    running_row(&ctx, None, Some(vec!["me"]));
    let at = Rc::new(Cell::new(0));
    let seen = at.clone();
    area.settle_cut(
        "r1",
        "stopped",
        &move |when| {
            seen.set(when);
            format!("stopped at {when}")
        },
        true,
        true,
    );
    assert_eq!(at.get(), 1_000_000, "the cut line gets the end's time");
    let said = ctx.recorded_json();
    assert_eq!(
        said.iter().map(|e| e["type"].clone()).collect::<Vec<_>>(),
        vec![json!("session.notify"), json!("session.done"), json!("session.end")]
    );
    assert_eq!(said[1]["result"], json!({ "status": "interrupted", "error": "stopped at 1000000", "unreached": true }));
    assert_eq!(said[2]["sawResult"], json!(true), "a stop the process never heard is still the turn's reply");
    assert_eq!(ctx.sessions.with(|s| support::json_of(&s.get("r1").expect("row").borrow().view.status)), json!("interrupted"));
    let cut = result("interrupted", Some("stopped at 1000000"));
    let mut cut_json = support::json_of(&cut);
    cut_json["unreached"] = json!(true);
    let cut: TurnResult = serde_json::from_value(cut_json).expect("a result");
    assert_eq!(said[0]["text"], json!(notify_line("t1", &cut, Some(NotifyLength::Tail))));
}

#[allow(dead_code)]
fn unread(prompt: &str) -> String {
    unread_line(prompt)
}

#[allow(dead_code)]
fn host_error(e: HostError) -> String {
    e.message
}
