// SPDX-License-Identifier: AGPL-3.0-only
#![allow(unused_imports, dead_code)]
mod support;

use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::{json, Value};
use support::{as_thread, here, paired, temp_root, FakeHost, ManualClock, MemStore, THREAD, WORKSPACE};
use wsp_slates::pending::code_of;
use wsp_slates::pending::wire::{SlateAsk, SlateEventAnswer, SlateEventOutcome, SlateView};
use wsp_slates::slates::{
    ApprovalScope, ApproveParams, CatalogParams, EventParams, EventScope, Press, ReadParams, SlateTarget, Slates, StateParams, WriteParams,
};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::{HostError, Kind};

struct Rig {
    _root: tempfile::TempDir,
    host: Rc<FakeHost>,
    slates: Rc<dyn Slates>,
}

fn rig(prefix: &str) -> Rig {
    support::own_group();
    let root = temp_root(prefix);
    let host = FakeHost::new(root.path(), MemStore::new());
    let slates = host.slates();
    Rig { _root: root, host, slates }
}

fn rig_on(prefix: &str, clock: &ManualClock) -> Rig {
    support::own_group();
    let root = temp_root(prefix);
    let host = FakeHost::with_clock(root.path(), MemStore::new(), clock.clock());
    let slates = host.slates();
    Rig { _root: root, host, slates }
}

fn agent() -> Caller {
    as_thread(THREAD, THREAD)
}

fn target() -> SlateTarget {
    SlateTarget { thread_id: Some(THREAD.into()), turn_token: None }
}

fn vals(pairs: &[(&str, Value)]) -> Option<IndexMap<String, Value>> {
    Some(pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

fn strs(items: &[&str]) -> Option<Vec<String>> {
    Some(items.iter().map(|s| s.to_string()).collect())
}

fn text_write(text: &str) -> WriteParams {
    WriteParams { text: Some(text.into()), ..Default::default() }
}

fn window_write(text: &str) -> WriteParams {
    WriteParams { target: target(), text: Some(text.into()), ..Default::default() }
}

async fn agent_write(s: &Rc<dyn Slates>, text: &str) -> wsp_slates::pending::wire::SlateWriteAnswer {
    s.write(text_write(text), Some(&agent())).await.expect("the agent's write")
}

async fn view(s: &Rc<dyn Slates>) -> SlateView {
    s.settled().await;
    s.get(THREAD).await.expect("get").expect("a slate")
}

async fn wait_view(s: &Rc<dyn Slates>, ms: u64, mut check: impl FnMut(&SlateView) -> bool) -> SlateView {
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    loop {
        let v = view(s).await;
        if check(&v) {
            return v;
        }
        if std::time::Instant::now() >= end {
            panic!("the slate never read as waited for: {v:?}");
        }
        support::sleep_ms(20).await;
    }
}

fn num_is(v: Option<&Value>, n: f64) -> bool {
    v.and_then(|v| v.as_f64()) == Some(n)
}

fn val(v: &SlateView, name: &str) -> Value {
    v.values.get(name).cloned().unwrap_or(Value::Null)
}

fn subset(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => e.iter().all(|(k, v)| a.get(k).is_some_and(|x| subset(x, v))),
        (Value::Array(a), Value::Array(e)) => a.len() == e.len() && a.iter().zip(e).all(|(x, y)| subset(x, y)),
        (Value::Number(a), Value::Number(e)) => a.as_f64() == e.as_f64(),
        _ => actual == expected,
    }
}

fn assert_has(actual: &Value, expected: Value) {
    assert!(subset(actual, &expected), "expected {expected} inside {actual}");
}

fn state_is(v: &SlateView, run: &str, state: &str) -> bool {
    val(v, run).get("state").and_then(|s| s.as_str()) == Some(state)
}

fn refusal<T>(r: Result<T, HostError>) -> HostError {
    match r {
        Ok(_) => panic!("expected a refusal"),
        Err(e) => e,
    }
}

fn like_at(text: &str, pat: &str) -> bool {
    let mut rest = text;
    for (i, lit) in pat.split("\\d+").enumerate() {
        if i > 0 {
            let n = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            if n == 0 {
                return false;
            }
            rest = &rest[n..];
        }
        match rest.strip_prefix(lit) {
            Some(r) => rest = r,
            None => return false,
        }
    }
    true
}

fn like_in(text: &str, pat: &str) -> bool {
    text.char_indices().any(|(i, _)| like_at(&text[i..], pat))
}

async fn approve(s: &Rc<dyn Slates>, key: &str, scope: ApprovalScope) -> Result<(), HostError> {
    s.approve(ApproveParams { thread_id: THREAD.into(), key: key.into(), scope }).await
}

fn press_params(version: i64, piece: &str, request_id: &str) -> EventParams {
    EventParams { thread_id: THREAD.into(), version, piece: piece.into(), event: "press".into(), request_id: request_id.into(), scope: None, row_action: None }
}

async fn press(s: &Rc<dyn Slates>, piece: &str, request_id: &str) -> Result<SlateEventAnswer, HostError> {
    let version = view(s).await.version;
    s.event(press_params(version, piece, request_id)).await
}

const QUIZ: &str = r##"<slate title="Quiz">
  <value name="i" start={0} />
  <value name="picked" start="" />
  <run name="grade" cmd="echo graded paris" timeout={20} />
  <column>
    <text id="q">question {$i}</text>
    <text id="p">picked {$picked}</text>
    <table id="pick" items={['a', 'b', 'c']}>
      <col title="Answer" value={item} />
      <action label="Pick" onPress={[set($picked, item), start($grade), send("Graded.", $picked)]} />
    </table>
    <button id="grade-it" label="Grade" onPress={start($grade)} />
  </column>
</slate>"##;

const TICKER: &str = r##"<slate title="Ticker">
  <value name="ticks" start={0} />
  <value name="at" start={0} />
  <run name="tick" cmd="sleep 1; echo tick" every={60} timeout={20} />
  <when done={$tick} do={set($ticks, $ticks + 1)} />
  <column>
    <text id="clock">clock {time.now}</text>
    <text id="since">since {ago($at)}</text>
  </column>
</slate>"##;

const PROBE: &str = r##"<slate title="Probe">
  <value name="n" start={0} />
  <value name="done" start={0} />
  <run name="probe" cmd='if [ "$N" -ge 2 ]; then sleep 1; fi; echo "run $N"' env={{ N: $n }} timeout={20} />
  <run name="ask" cmd="echo asked" confirm="Run it?" />
  <when change={$n} do={start($probe)} />
  <when done={$probe} do={set($done, $done + 1)} />
  <column>
    <text id="out">{$probe.out}</text>
    <button id="more" label="More" held={$n > 100 and 'too many'} onPress={set($n, $n + 1)} />
    <input id="num" label="N" value={$n} held={$n > 100 and 'too many'} />
  </column>
</slate>"##;

use wsp_slates::slates::{Delivered, DeliveryOutcome};

const PRESS: &str = r##"<slate><column><button id="go" label="Go on" onPress={send("Go on.")} /></column></slate>"##;

const COUNTER: &str = r##"<slate><value name="n" start={0} /><column><button id="up" label="Up" onPress={set($n, $n + 1)} /></column></slate>"##;

const DEPLOY: &str = r##"<slate title="Deploy">
  <run name="deploy" cmd='sleep 2; echo deployed' confirm="Deploy now?" timeout={20} />
  <column><button id="go" label="Deploy" onPress={start($deploy)} /></column>
</slate>"##;

const PNG_HEX: &str = "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d4944415478da63f8cfc0f01f0005000201a3b0d8f60000000049454e44ae426082";
const PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP4z8DwHwAFAAIBo7DY9gAAAABJRU5ErkJggg==";

fn slate_line(prompt: &str) -> Value {
    let (_, line) = prompt.split_once("\n\nslate: ").expect("a slate: line");
    serde_json::from_str(line).expect("the slate: line is JSON")
}

async fn n_of(s: &Rc<dyn Slates>) -> f64 {
    view(s).await.values.get("n").and_then(|v| v.as_f64()).expect("a number")
}

async fn count_press(s: &Rc<dyn Slates>, id: &str) -> Result<SlateEventAnswer, HostError> {
    s.event(press_params(1, "up", id)).await
}

// slates_acts Edge case: slates.test.ts:484
#[test]
fn takes_five_of_the_person_s_events_a_second_answers_a_resend_of_one_it_took_an() {
    support::local(async {
        let rig = rig("wsp-slates-event-rate-");
        let s = &rig.slates;
        agent_write(s, COUNTER).await;
        let ev = |id: &str| s.event(press_params(1, "up", id));
        let results = futures::future::join_all(vec![ev("e0"), ev("e1"), ev("e2"), ev("e3"), ev("e4"), ev("e5"), ev("e0")]).await;
        for (i, r) in results.iter().enumerate().take(5) {
            assert!(r.is_ok(), "press e{i}: {r:?}");
        }
        let sixth = results[5].as_ref().err().expect("the sixth is refused");
        assert_eq!(sixth.kind, Some(Kind::Conflict));
        assert_eq!(code_of(sixth).as_deref(), Some("V754"));
        assert!(sixth.message.starts_with("V754 event-rate: too many presses; try again in a moment"), "{}", sixth.message);
        assert_eq!(results[6], results[0], "a resend answers the first answer");
        assert_eq!(n_of(s).await, 5.0);
    });
}

// slates_acts Behaviour 1: the bucket refills five a second, and a refusal is not remembered
#[test]
fn the_event_bucket_refills_five_a_second_and_a_refused_press_is_new_when_pressed_again() {
    support::local(async {
        let clock = ManualClock::new(1_000_000);
        let rig = rig_on("wsp-slates-event-refill-", &clock);
        let s = &rig.slates;
        agent_write(s, COUNTER).await;
        for i in 0..5 {
            count_press(s, &format!("e{i}")).await.unwrap();
        }
        let refused = refusal(count_press(s, "e5").await);
        assert_eq!(code_of(&refused).as_deref(), Some("V754"));
        assert_eq!(n_of(s).await, 5.0);
        clock.advance(200);
        count_press(s, "e5").await.expect("a token is back after 200 ms, and the refused id was forgotten");
        assert_eq!(n_of(s).await, 6.0);
        let again = refusal(count_press(s, "e6").await);
        assert_eq!(code_of(&again).as_deref(), Some("V754"), "the one token went to e5");
    });
}

// slates_acts Behaviour 1: request ids older than 600,000 ms are forgotten
#[test]
fn a_request_id_is_remembered_for_600000_ms_and_then_the_press_is_new() {
    support::local(async {
        let clock = ManualClock::new(1_000_000);
        let rig = rig_on("wsp-slates-event-ids-", &clock);
        let s = &rig.slates;
        agent_write(s, COUNTER).await;
        count_press(s, "x").await.unwrap();
        assert_eq!(n_of(s).await, 1.0);
        count_press(s, "x").await.unwrap();
        assert_eq!(n_of(s).await, 1.0, "a resend is the same press");
        clock.advance(600_000);
        count_press(s, "x").await.unwrap();
        assert_eq!(n_of(s).await, 1.0, "600,000 ms is not older than 600,000 ms");
        clock.advance(1);
        count_press(s, "x").await.unwrap();
        assert_eq!(n_of(s).await, 2.0);
    });
}

// slates_acts Behaviour 1: at 500 held ids the oldest is dropped
#[test]
fn at_500_held_request_ids_the_oldest_is_dropped_and_pressed_again_is_new() {
    support::local(async {
        let clock = ManualClock::new(1_000_000);
        let rig = rig_on("wsp-slates-event-500-", &clock);
        let s = &rig.slates;
        agent_write(s, COUNTER).await;
        for i in 0..501 {
            count_press(s, &format!("p{i}")).await.unwrap();
            clock.advance(250);
        }
        assert_eq!(n_of(s).await, 501.0);
        count_press(s, "p2").await.unwrap();
        assert_eq!(n_of(s).await, 501.0, "p2 is still held: a resend");
        count_press(s, "p0").await.unwrap();
        assert_eq!(n_of(s).await, 502.0, "p0 was dropped to make room: a new press");
    });
}

// slates_acts Behaviour 1: a piece the document lacks is gone, and the refusal is forgotten
#[test]
fn a_press_on_a_piece_the_host_s_document_lacks_refuses_and_a_press_again_is_new() {
    support::local(async {
        let clock = ManualClock::new(1_000_000);
        let rig = rig_on("wsp-slates-event-gone-", &clock);
        let s = &rig.slates;
        agent_write(s, COUNTER).await;
        let gone = refusal(s.event(press_params(1, "nope", "same")).await);
        assert_eq!(gone.kind, Some(Kind::Usage));
        assert!(gone.message.contains("That part of the slate is gone (piece nope, version 1; the slate is at 1)."), "{}", gone.message);
        assert_eq!(gone.fix.as_deref(), Some("Press it again once the slate has redrawn."));
        agent_write(s, r##"<slate><value name="n" start={0} /><column><button id="nope" label="Now" onPress={set($n, 9)} /></column></slate>"##).await;
        clock.advance(250);
        s.event(press_params(2, "nope", "same")).await.expect("the refusal was not remembered");
        assert_eq!(n_of(s).await, 9.0);
    });
}

// slates_acts Behaviour 1: a row action the piece lacks
#[test]
fn a_row_action_the_piece_lacks_is_refused_with_the_redraw_fix() {
    support::local(async {
        let rig = rig("wsp-slates-event-noaction-");
        let s = &rig.slates;
        agent_write(s, COUNTER).await;
        let mut p = press_params(1, "up", "ra");
        p.row_action = Some(3);
        let e = refusal(s.event(p).await);
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains("up has no row action 3."), "{}", e.message);
        assert_eq!(e.fix.as_deref(), Some("Press it again once the slate has redrawn."));
        assert_eq!(n_of(s).await, 0.0);
    });
}

// slates_acts Behaviour 1: a scoped event's item is the host's own row of the piece's items
#[test]
fn a_row_event_reads_the_item_off_the_host_s_list_not_the_window_s_copy() {
    support::local(async {
        let rig = rig("wsp-slates-event-row-");
        let s = &rig.slates;
        agent_write(s, QUIZ).await;
        let mut p = press_params(1, "pick", "row");
        p.scope = Some(EventScope { item: Some(json!("zzz")), index: 1 });
        p.row_action = Some(0);
        s.event(p).await.unwrap();
        assert_eq!(val(&view(s).await, "picked"), json!("b"));
        let delivered = rig.host.delivered.borrow().clone();
        assert_eq!(delivered.len(), 1);
        assert!(delivered[0].prompt.starts_with("Graded.\n\nslate: "), "{}", delivered[0].prompt);
        assert_has(&slate_line(&delivered[0].prompt), json!({"piece": "pick", "event": "press", "index": 1, "with": {"$picked": "b"}}));
    });
}

// slates_acts Behaviour 1 / Edge case: slates.test.ts:705
#[test]
fn a_press_that_sends_starts_the_agent_s_turn_with_the_text_and_the_slate_line() {
    support::local(async {
        let rig = rig("wsp-slates-press-send-");
        let s = &rig.slates;
        agent_write(s, PRESS).await;
        let told = s.event(press_params(1, "go", "go-1")).await.unwrap();
        assert_eq!(told.outcome, SlateEventOutcome::Started);
        assert_eq!(told.said, "Sent");
        assert_eq!(told.turn_id.as_deref(), Some("turn-2"));
        assert!(told.ask.is_none());
        let delivered = rig.host.delivered.borrow().clone();
        assert_eq!(delivered.len(), 1);
        assert_eq!((delivered[0].thread_id.as_str(), delivered[0].workspace_id.as_str(), delivered[0].request_id.as_str()), (THREAD, WORKSPACE, "go-1"));
        assert!(delivered[0].prompt.starts_with("Go on.\n\nslate: "), "{}", delivered[0].prompt);
        assert_has(&slate_line(&delivered[0].prompt), json!({"v": 2, "kind": "action", "piece": "go", "label": "Go on", "event": "press", "by": "person"}));
    });
}

// slates_acts Behaviour 1: the outcome a delivery came to is what the press answers
#[test]
fn a_press_answers_steered_and_queued_with_what_each_reads_as() {
    support::local(async {
        let clock = ManualClock::new(1_000_000);
        let rig = rig_on("wsp-slates-press-outcomes-", &clock);
        let s = &rig.slates;
        agent_write(s, PRESS).await;
        *rig.host.deliver_answer.borrow_mut() = Ok(Delivered { outcome: DeliveryOutcome::Steered, turn_id: None });
        let steered = s.event(press_params(1, "go", "g-steer")).await.unwrap();
        assert_eq!((steered.outcome, steered.said.as_str(), steered.turn_id.clone()), (SlateEventOutcome::Steered, "Sent into the running turn", None));
        clock.advance(2_001);
        *rig.host.deliver_answer.borrow_mut() = Ok(Delivered { outcome: DeliveryOutcome::Queued, turn_id: None });
        let queued = s.event(press_params(1, "go", "g-queue")).await.unwrap();
        assert_eq!((queued.outcome, queued.said.as_str(), queued.turn_id.clone()), (SlateEventOutcome::Queued, "Waiting for the turn to end", None));
    });
}

// slates_acts Behaviour 1: a send that cannot be delivered refuses with its kind, or conflict, and is forgotten
#[test]
fn a_press_whose_send_cannot_be_delivered_refuses_with_its_kind_or_conflict() {
    support::local(async {
        let clock = ManualClock::new(1_000_000);
        let rig = rig_on("wsp-slates-press-undelivered-", &clock);
        let s = &rig.slates;
        agent_write(s, PRESS).await;
        *rig.host.deliver_answer.borrow_mut() = Err(HostError::new("boom"));
        let plain = refusal(s.event(press_params(1, "go", "same")).await);
        assert_eq!(plain.message, "This thread cannot take a message now: boom");
        assert_eq!(plain.kind, Some(Kind::Conflict));
        clock.advance(2_001);
        *rig.host.deliver_answer.borrow_mut() = Err(HostError::with_kind("no turn", Kind::Usage));
        let kinded = refusal(s.event(press_params(1, "go", "other")).await);
        assert_eq!(kinded.message, "This thread cannot take a message now: no turn");
        assert_eq!(kinded.kind, Some(Kind::Usage));
        clock.advance(2_001);
        *rig.host.deliver_answer.borrow_mut() = Ok(Delivered { outcome: DeliveryOutcome::Started, turn_id: Some("turn-9".into()) });
        let again = s.event(press_params(1, "go", "same")).await.expect("a refused answer is forgotten, so the press is new");
        assert_eq!(again.turn_id.as_deref(), Some("turn-9"));
    });
}

// slates_acts Behaviour 1: a press that starts an unapproved command answers held with the sheet
#[test]
fn a_press_that_starts_an_unapproved_command_answers_held_with_the_ask() {
    support::local(async {
        let rig = rig("wsp-slates-press-held-");
        let s = &rig.slates;
        agent_write(s, QUIZ).await;
        let pressed = s.event(press_params(1, "grade-it", "g1")).await.unwrap();
        assert_eq!(pressed.outcome, SlateEventOutcome::Held);
        assert_eq!(pressed.said, "Needs your approval");
        let ask = pressed.ask.expect("the sheet");
        assert_eq!(ask.run(), "grade");
        assert!(rig.host.delivered.borrow().is_empty());
        assert_eq!(view(s).await.asks.len(), 1);
    });
}

// slates_acts Behaviour 1: a press that changes a value and holds nothing is done with nothing said
#[test]
fn a_press_that_only_sets_a_value_answers_done_with_nothing_said() {
    support::local(async {
        let rig = rig("wsp-slates-press-done-");
        let s = &rig.slates;
        agent_write(s, COUNTER).await;
        let done = s.event(press_params(1, "up", "u1")).await.unwrap();
        assert_eq!((done.outcome, done.said.as_str()), (SlateEventOutcome::Done, ""));
        assert!(done.turn_id.is_none() && done.ask.is_none());
    });
}

// slates_acts Behaviour 1: an event needs a slate
#[test]
fn an_event_cancel_image_or_approval_on_a_thread_with_no_slate_refuses_z802() {
    support::local(async {
        let rig = rig("wsp-slates-acts-noslate-");
        let s = &rig.slates;
        let said = "Z802 no-slate: this thread has no slate yet; write one with slate_write";
        for e in [
            refusal(s.event(press_params(1, "go", "x")).await),
            refusal(approve(s, "anything", ApprovalScope::Thread).await),
            refusal(s.revoke(THREAD, "anything").await),
            refusal(s.cancel(THREAD, "tick").await),
            refusal(s.image(THREAD, "a.png", None).await),
        ] {
            assert_eq!(e.kind, Some(Kind::Usage));
            assert_eq!(code_of(&e).as_deref(), Some("Z802"));
            assert!(e.message.contains(said), "{}", e.message);
        }
    });
}

// slates_acts Behaviour 2: a key that names no declared command
#[test]
fn approving_a_key_no_command_declares_refuses_and_names_the_fix() {
    support::local(async {
        let rig = rig("wsp-slates-approve-nokey-");
        let s = &rig.slates;
        agent_write(s, QUIZ).await;
        let e = refusal(approve(s, "0123456789abcdef", ApprovalScope::Once).await);
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains("this slate declares no command with approval key 0123456789abcdef."), "{}", e.message);
        assert_eq!(e.fix.as_deref(), Some("Read the slate again and approve what it asks now."));
    });
}

// slates_acts Behaviour 2: Not now keeps the refusal, and the run is denied
#[test]
fn not_now_keeps_the_refusal_by_run_and_command_and_nothing_runs() {
    support::local(async {
        let rig = rig("wsp-slates-approve-refuse-");
        let s = &rig.slates;
        agent_write(s, QUIZ).await;
        let pressed = s.event(press_params(1, "grade-it", "g1")).await.unwrap();
        let key = pressed.ask.expect("the sheet").key();
        approve(s, &key, ApprovalScope::Refuse).await.unwrap();
        let v = view(s).await;
        assert_has(&json!(v.approvals[&key]), json!({"state": "refused", "run": "grade", "cmd": "echo graded paris"}));
        assert!(v.approvals[&key].at > 0);
        assert!(!state_is(&v, "grade", "done") && !state_is(&v, "grade", "running"));
        support::sleep_ms(300).await;
        assert!(!state_is(&view(s).await, "grade", "done"));
    });
}

// slates_acts Behaviour 2: Run once keeps nothing, Always in this thread keeps the approval with its run and command
#[test]
fn run_once_keeps_no_approval_and_always_keeps_one_with_its_run_and_command() {
    support::local(async {
        let rig = rig("wsp-slates-approve-kept-");
        let s = &rig.slates;
        agent_write(s, QUIZ).await;
        let first = s.event(press_params(1, "grade-it", "g1")).await.unwrap();
        let key = first.ask.expect("the sheet").key();
        approve(s, &key, ApprovalScope::Once).await.unwrap();
        wait_view(s, 10_000, |v| state_is(v, "grade", "done")).await;
        assert!(view(s).await.approvals.is_empty(), "a run once is never kept");
        let second = s.event(press_params(1, "grade-it", "g2")).await.unwrap();
        assert_eq!(second.ask.expect("asks again").key(), key);
        approve(s, &key, ApprovalScope::Thread).await.unwrap();
        wait_view(s, 10_000, |v| num_is(v.values.get("grade").and_then(|g| g.get("runs")), 2.0)).await;
        let v = view(s).await;
        assert_has(&json!(v.approvals[&key]), json!({"state": "allowed", "run": "grade", "cmd": "echo graded paris"}));
    });
}

// slates_acts Behaviour 2: the slate's permission to message the agent
#[test]
fn the_send_key_allows_or_refuses_the_slate_messaging_the_agent_and_a_revoke_drops_it() {
    support::local(async {
        let rig = rig("wsp-slates-send-key-");
        let s = &rig.slates;
        agent_write(s, COUNTER).await;
        let none = refusal(s.revoke(THREAD, "send").await);
        assert_eq!(none.kind, Some(Kind::Usage));
        assert!(none.message.contains("this thread's reactions were not allowed to message the agent."), "{}", none.message);
        assert_eq!(none.fix.as_deref(), Some("Read its approvals again."));
        approve(s, "send", ApprovalScope::Thread).await.unwrap();
        assert_eq!(view(s).await.approvals["send"].state, "allowed");
        s.revoke(THREAD, "send").await.unwrap();
        assert!(!view(s).await.approvals.contains_key("send"));
        approve(s, "send", ApprovalScope::Once).await.unwrap();
        assert!(view(s).await.approvals.contains_key("send"), "any scope but refuse stamps it");
        approve(s, "send", ApprovalScope::Refuse).await.unwrap();
        assert!(!view(s).await.approvals.contains_key("send"));
    });
}

// slates_acts Edge case: slates.test.ts:476
#[test]
fn keeps_a_link_s_domain_the_person_allowed_for_the_thread_and_refuses_a_key_tha() {
    support::local(async {
        let rig = rig("wsp-slates-domain-");
        let s = &rig.slates;
        agent_write(s, r##"<slate><column><button id="go" label="Go" onPress={open("https://example.com")} /></column></slate>"##).await;
        approve(s, "domain:example.com", ApprovalScope::Thread).await.unwrap();
        assert_has(&json!(view(s).await.approvals["domain:example.com"]), json!({"state": "allowed", "cmd": "links to example.com"}));
        let e = refusal(approve(s, "domain:../x y", ApprovalScope::Thread).await);
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains("domain:../x y names no domain."), "{}", e.message);
        assert_eq!(e.fix.as_deref(), Some("Approve a link as domain:example.com."));
    });
}

// slates_acts Behaviour 2: once keeps nothing of a domain, refuse deletes it
#[test]
fn a_domain_allowed_once_keeps_nothing_and_a_refused_one_is_deleted() {
    support::local(async {
        let rig = rig("wsp-slates-domain-scopes-");
        let s = &rig.slates;
        agent_write(s, COUNTER).await;
        approve(s, "domain:example.com", ApprovalScope::Once).await.unwrap();
        assert!(!view(s).await.approvals.contains_key("domain:example.com"));
        approve(s, "domain:example.com", ApprovalScope::Thread).await.unwrap();
        assert!(view(s).await.approvals.contains_key("domain:example.com"));
        approve(s, "domain:example.com", ApprovalScope::Refuse).await.unwrap();
        assert!(!view(s).await.approvals.contains_key("domain:example.com"));
    });
}

// slates_acts Edge case: slates.test.ts:447
#[test]
fn the_person_revokes_an_approval_its_run_stops_and_leaves_its_timer_and_the_nex() {
    support::local(async {
        let rig = rig("wsp-slates-revoke-");
        let s = &rig.slates;
        let slow = TICKER.replace(r#"cmd="sleep 1; echo tick""#, r#"cmd="sleep 5; echo tick""#);
        agent_write(s, &slow).await;
        let release = s.subscribe(THREAD, &[]);
        let key = view(s).await.asks[0].key();
        approve(s, &key, ApprovalScope::Thread).await.unwrap();
        wait_view(s, 5_000, |v| state_is(v, "tick", "running")).await;
        s.revoke(THREAD, &key).await.unwrap();
        let v = wait_view(s, 5_000, |v| state_is(v, "tick", "cancelled")).await;
        assert!(!v.approvals.contains_key(&key));
        agent_write(s, &slow).await;
        wait_view(s, 5_000, |v| state_is(v, "tick", "held") && v.asks.iter().map(|a| a.key()).collect::<Vec<_>>() == vec![key.clone()]).await;
        approve(s, "domain:example.com", ApprovalScope::Thread).await.unwrap();
        s.revoke(THREAD, "domain:example.com").await.unwrap();
        assert!(!view(s).await.approvals.contains_key("domain:example.com"));
        let nope = refusal(s.revoke(THREAD, "nope").await);
        assert_eq!(nope.kind, Some(Kind::Usage));
        assert!(nope.message.contains("holds no approval nope"), "{}", nope.message);
        assert_eq!(nope.fix.as_deref(), Some("Read its approvals again."));
        release();
    });
}

// slates_acts Behaviour 4: a cancel stops the run
#[test]
fn a_cancel_stops_a_running_command_and_reads_cancelled() {
    support::local(async {
        let rig = rig("wsp-slates-cancel-");
        let s = &rig.slates;
        agent_write(s, r##"<slate><run name="slow" cmd="sleep 30" timeout={60} /><column><button id="go" label="Go" onPress={start($slow)} /></column></slate>"##).await;
        let pressed = s.event(press_params(1, "go", "go")).await.unwrap();
        approve(s, &pressed.ask.expect("the sheet").key(), ApprovalScope::Once).await.unwrap();
        wait_view(s, 10_000, |v| state_is(v, "slow", "running")).await;
        s.cancel(THREAD, "slow").await.unwrap();
        wait_view(s, 10_000, |v| state_is(v, "slow", "cancelled")).await;
    });
}

// slates_acts Edge case: slates.test.ts:1175
#[test]
fn a_held_run_whose_command_the_agent_rewrote_asks_again_for_the_new_one_and_run() {
    support::local(async {
        let rig = rig("wsp-slates-held-rewrite-");
        let s = &rig.slates;
        let slate = |cmd: &str| format!(r##"<slate title="Say"><run name="say" cmd="{cmd}" /><column><button id="go" label="Say" onPress={{start($say)}} /></column></slate>"##);
        agent_write(s, &slate("echo old")).await;
        let pressed = press(s, "go", "go").await.unwrap();
        let first = pressed.ask.expect("the sheet");
        assert_has(&first.to_json(), json!({"cmd": "echo old"}));
        agent_write(s, &slate("echo new")).await;
        let asks = view(s).await.asks;
        assert_eq!(asks.len(), 1);
        assert_has(&asks[0].to_json(), json!({"run": "say", "cmd": "echo new"}));
        assert_ne!(asks[0].key(), first.key());
        approve(s, &asks[0].key(), ApprovalScope::Once).await.unwrap();
        wait_view(s, 10_000, |v| subset(&val(v, "say"), &json!({"state": "done", "out": "new\n"}))).await;
    });
}

// slates_acts Edge case: slates.test.ts:1210
#[test]
fn a_second_press_on_a_confirmed_run_still_running_leaves_it_running_and_don_t_d() {
    support::local(async {
        let rig = rig("wsp-slates-confirm-again-");
        let s = &rig.slates;
        agent_write(s, DEPLOY).await;
        let first = press(s, "go", "first").await.unwrap();
        let key = first.ask.expect("the sheet").key();
        approve(s, &key, ApprovalScope::Once).await.unwrap();
        wait_view(s, 10_000, |v| state_is(v, "deploy", "running")).await;
        let again = press(s, "go", "again").await.unwrap();
        assert_eq!(again.ask.expect("asks again").key(), key);
        assert_has(&val(&view(s).await, "deploy"), json!({"state": "running", "runs": 1}));
        approve(s, &key, ApprovalScope::Refuse).await.unwrap();
        assert_has(&val(&view(s).await, "deploy"), json!({"state": "running", "runs": 1}));
        wait_view(s, 10_000, |v| subset(&val(v, "deploy"), &json!({"state": "done", "out": "deployed\n", "runs": 1}))).await;
    });
}

// slates_acts Edge case: slates.test.ts:1020
#[test]
fn an_image_piece_reads_a_file_from_any_path_and_under_the_thread_s_folder_and_f() {
    support::local(async {
        let rig = rig("wsp-slates-image-");
        let s = &rig.slates;
        let png = hex::decode(PNG_HEX).unwrap();
        let folder = rig.host.folder();
        std::fs::create_dir(folder.join("shots")).unwrap();
        std::fs::write(folder.join("shots").join("home.png"), &png).unwrap();
        std::fs::write(rig.host.root.join("elsewhere.png"), &png).unwrap();
        agent_write(s, r##"<slate><column><image src="shots/home.png" /></column></slate>"##).await;
        async fn code(s: &Rc<dyn Slates>, src: &str) -> String {
            let a = s.image(THREAD, src, None).await.unwrap();
            if let Some(p) = a.get("problem") {
                format!("{} {}", p["code"].as_str().unwrap(), p["message"].as_str().unwrap())
            } else if let Some(ask) = a.get("ask") {
                format!("ask {}", ask["domain"].as_str().unwrap())
            } else if a.get("unchanged").is_some() {
                "unchanged".to_string()
            } else {
                a["mediaType"].as_str().unwrap().to_string()
            }
        }
        let whole = s.image(THREAD, "shots/home.png", None).await.unwrap();
        assert_has(&whole, json!({"mediaType": "image/png", "bytes": PNG_BASE64}));
        assert_eq!(code(s, &rig.host.root.join("elsewhere.png").display().to_string()).await, "image/png");
        assert_eq!(code(s, "../elsewhere.png").await, "image/png");
        assert_eq!(code(s, "shots/gone.png").await, "R900 no file at shots/gone.png");
        assert_eq!(code(s, "http://localhost:9/a.png").await, "ask localhost");
        approve(s, "domain:localhost", ApprovalScope::Thread).await.unwrap();
        let fetched = code(s, "http://localhost:9/a.png").await;
        assert!(fetched.starts_with("R917 http://localhost:9/a.png resolves to "), "{fetched}");
        assert!(fetched.contains(", which is this computer or its network"), "{fetched}");
        assert_eq!(code(s, "http://other.localhost:9/a.png").await, "ask other.localhost");
    });
}
