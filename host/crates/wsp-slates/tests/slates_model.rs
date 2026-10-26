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
    EventParams {
        thread_id: THREAD.into(),
        version,
        piece: piece.into(),
        event: "press".into(),
        request_id: request_id.into(),
        scope: None,
        row_action: None,
    }
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

use wsp_slates::pending::slate::{SlateProblem, SlateRunDecl};
use wsp_slates::pending::wire::SlateEmpty;
use wsp_slates::slates::model::{
    command_of, own_path, problem, refused, said, size_of, sources_named, trim_snapshots, usage, SlateRecord, SlateSnap, TurnMark,
};
use wsp_slates::slates::DeliveryOutcome;

const SETUP: &str = r##"<slate title="Vercel setup">
  <secret name="token" />
  <value name="project" start="" />
  <value name="step" start={1} />
  <value name="fired" start={0} />
  <run name="check" cmd='printf "{\"name\":\"%s\"}" "$PROJECT"' env={{ PROJECT: $project }} timeout={20} />
  <run name="env" cmd='printf "VERCEL_TOKEN=%s\n" "$VERCEL_TOKEN" >> .env; echo "wrote $VERCEL_TOKEN"' env={{ VERCEL_TOKEN: $token }} />
  <run name="slow" cmd="sleep 30" timeout={60} />
  <derived name="ok" value={$check.state == 'done' and $check.json.name == $project} />
  <when change={$project} do={start($check)} />
  <when done={$check} do={set($step, 2)} />
  <when done={$slow} do={set($fired, $fired + 1)} />
  <column>
    <input id="tok" label="Vercel token" value={$token} kind="password" />
    <input id="proj" label="Project" value={$project} />
    <text id="found" when={$ok} tone="good">Found {$check.json.name}</text>
    <button id="next" label="Next" variant="primary" when={$ok} onPress={set($step, 3)} />
    <button id="write" label="Write .env" onPress={start($env)} />
    <button id="wait" label="Wait" onPress={start($slow)} />
    <button id="tell" label="Tell the agent" onPress={send("The Vercel setup is done.", $project, $token, $env.out)} />
  </column>
</slate>"##;

const SMALL: &str = r##"<slate title="Small"><value name="n" start={1} /><column><text id="t">{$n}</text></column></slate>"##;

fn record() -> SlateRecord {
    SlateRecord {
        thread_id: THREAD.into(),
        workspace_id: WORKSPACE.into(),
        root_thread_id: THREAD.into(),
        schema: 2,
        version: 1,
        revision: Some(1),
        document: None,
        values: IndexMap::new(),
        empty: Some(SlateEmpty::None),
        turns: IndexMap::new(),
        kept: IndexMap::new(),
        comments: vec![],
        approvals: IndexMap::new(),
        problems: vec![],
        shown_once: false,
        updated_at: 0,
        previous: None,
        sends_allowed: None,
    }
}

fn snap(bytes: usize) -> SlateSnap {
    SlateSnap { document: None, values: IndexMap::from([("x".to_string(), json!("a".repeat(bytes)))]), empty: None }
}

fn turn(r: &mut SlateRecord, i: usize, revision: i64, bytes: usize) {
    r.turns.insert(format!("turn-{i:03}"), TurnMark { at: i as i64, version: 1, revision });
    r.kept.entry(revision.to_string()).or_insert_with(|| snap(bytes));
}

fn turn_ids(r: &SlateRecord) -> Vec<String> {
    r.turns.keys().cloned().collect()
}

fn kept_ids(r: &SlateRecord) -> Vec<String> {
    r.kept.keys().cloned().collect()
}

fn decl(v: Value) -> SlateRunDecl {
    serde_json::from_value(v).expect("a run declaration")
}

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// slates_model Behaviour 13 / Edge case: the 101st turn drops the first turn's pointer and the snapshot only it used
#[test]
fn the_101st_turn_drops_the_first_turn_and_the_snapshot_only_it_used() {
    let mut r = record();
    for i in 0..101 {
        turn(&mut r, i, i as i64, 0);
    }
    trim_snapshots(&mut r);
    let expected_turns: Vec<String> = (1..101).map(|i| format!("turn-{i:03}")).collect();
    let expected_kept: Vec<String> = (1..101).map(|i| i.to_string()).collect();
    assert_eq!(turn_ids(&r), expected_turns);
    assert_eq!(kept_ids(&r), expected_kept);
}

// slates_model Behaviour 13: a turn where nothing changed shares its snapshot
#[test]
fn turns_that_share_a_revision_share_one_snapshot_and_the_trim_keeps_it() {
    let mut r = record();
    for i in 0..101 {
        turn(&mut r, i, 7, 10);
    }
    trim_snapshots(&mut r);
    assert_eq!(r.turns.len(), 100);
    assert!(!r.turns.contains_key("turn-000"));
    assert_eq!(kept_ids(&r), names(&["7"]));
}

// slates_model Behaviour 13: every kept snapshot no turn points at goes
#[test]
fn a_kept_snapshot_no_turn_points_at_is_dropped() {
    let mut r = record();
    turn(&mut r, 1, 1, 0);
    turn(&mut r, 2, 2, 0);
    r.kept.insert("9".into(), snap(0));
    trim_snapshots(&mut r);
    assert_eq!(turn_ids(&r), names(&["turn-001", "turn-002"]));
    assert_eq!(kept_ids(&r), names(&["1", "2"]));
}

// slates_model Behaviour 13: past 8 MiB the oldest turn and its snapshot go
#[test]
fn past_8_mib_of_snapshots_the_oldest_turn_and_the_snapshot_only_it_used_go() {
    let mut r = record();
    for i in 1..=3 {
        turn(&mut r, i, i as i64, 3_000_000);
    }
    trim_snapshots(&mut r);
    assert_eq!(turn_ids(&r), names(&["turn-002", "turn-003"]));
    assert_eq!(kept_ids(&r), names(&["2", "3"]));
}

// slates_model Behaviour 13: the byte cap is a cap, not a count
#[test]
fn snapshots_under_8_mib_are_all_kept() {
    let mut r = record();
    for i in 1..=2 {
        turn(&mut r, i, i as i64, 4_000_000);
    }
    trim_snapshots(&mut r);
    assert_eq!(turn_ids(&r), names(&["turn-001", "turn-002"]));
    assert_eq!(kept_ids(&r), names(&["1", "2"]));
}

// slates_model Behaviour 13: the loop drops turn after turn until the snapshots fit
#[test]
fn the_byte_cap_drops_as_many_old_turns_as_it_takes() {
    let mut r = record();
    for i in 1..=4 {
        turn(&mut r, i, i as i64, 3_000_000);
    }
    trim_snapshots(&mut r);
    assert_eq!(turn_ids(&r), names(&["turn-003", "turn-004"]));
    assert_eq!(kept_ids(&r), names(&["3", "4"]));
}

// slates_model Edge case: an 8 MiB cap with one turn left keeps that turn whatever its size
#[test]
fn one_turn_is_kept_whatever_its_size() {
    let mut one = record();
    turn(&mut one, 1, 1, 9_000_000);
    trim_snapshots(&mut one);
    assert_eq!(turn_ids(&one), names(&["turn-001"]));
    assert_eq!(kept_ids(&one), names(&["1"]));
    let mut two = record();
    turn(&mut two, 1, 1, 9_000_000);
    turn(&mut two, 2, 2, 9_000_000);
    trim_snapshots(&mut two);
    assert_eq!(turn_ids(&two), names(&["turn-002"]));
    assert_eq!(kept_ids(&two), names(&["2"]));
}

// slates_model Behaviour 6: the declaration less every, always, timeout and stream
#[test]
fn command_of_leaves_out_every_always_timeout_and_stream() {
    let cmd = decl(
        json!({"kind": "cmd", "cmd": "echo hi", "env": {"A": "1"}, "timeout": 20, "stream": true, "confirm": "Sure?", "every": 60, "once": true, "always": true}),
    );
    assert_eq!(command_of(&cmd), json!({"kind": "cmd", "cmd": "echo hi", "env": {"A": "1"}, "confirm": "Sure?", "once": true}));
    let tool = decl(json!({"kind": "tool", "server": "s", "tool": "t", "every": 30, "always": true}));
    assert_eq!(command_of(&tool), json!({"kind": "tool", "server": "s", "tool": "t"}));
}

// slates_model Edge case: a command written again with only timeout changed keeps its result fresh
#[test]
fn command_of_is_equal_when_only_how_often_or_how_long_changed() {
    let a = decl(json!({"kind": "cmd", "cmd": "echo hi", "timeout": 20}));
    let b = decl(json!({"kind": "cmd", "cmd": "echo hi", "timeout": 30, "every": 10, "always": true, "stream": true}));
    let c = decl(json!({"kind": "cmd", "cmd": "echo bye", "timeout": 20}));
    assert_eq!(command_of(&a), command_of(&b));
    assert_ne!(command_of(&a), command_of(&c));
}

// slates_model Behaviour 5
#[test]
fn own_path_puts_a_dollar_before_a_path_that_lacks_one() {
    assert_eq!(own_path("i"), "$i");
    assert_eq!(own_path("$i"), "$i");
    assert_eq!(own_path("x.y[0]"), "$x.y[0]");
    assert_eq!(own_path(""), "$");
}

// slates_model Behaviour 5
#[test]
fn sources_named_answers_the_host_sources_the_text_names_in_their_own_order() {
    assert_eq!(sources_named("time.now and git.changed"), names(&["time", "git"]));
    assert_eq!(sources_named("$cost.total, pr.checks, thread.id, usage.week"), names(&["thread", "usage", "cost", "pr"]));
    assert_eq!(sources_named("git.a git.b"), names(&["git"]));
}

// slates_model Behaviour 5: the source name must start a word and be followed by a dot
#[test]
fn sources_named_needs_a_word_start_and_a_dot() {
    assert!(sources_named("mygit.x timex.y pr gitx. cost 5").is_empty());
    assert_eq!(sources_named("{$a}{git.branch}"), names(&["git"]));
}

// slates_model Behaviour 4
#[test]
fn problem_is_code_name_message_and_a_fix_when_given() {
    let with = problem("V752", "nothing-to-undo", "there is no earlier slate", Some("write one"));
    assert_eq!(
        serde_json::to_string(&with).unwrap(),
        r#"{"code":"V752","name":"nothing-to-undo","message":"there is no earlier slate","fix":"write one"}"#
    );
    let without = problem("Z802", "no-slate", "none", None);
    assert_eq!(serde_json::to_string(&without).unwrap(), r#"{"code":"Z802","name":"no-slate","message":"none"}"#);
}

// slates_model Behaviour 4
#[test]
fn refused_and_usage_say_code_name_message_and_carry_the_kind_and_the_code() {
    let p: SlateProblem = problem("V754", "event-rate", "too many presses; try again in a moment", None);
    let r = refused(&p, Kind::Conflict);
    assert_eq!(r.message, "V754 event-rate: too many presses; try again in a moment");
    assert_eq!(r.kind, Some(Kind::Conflict));
    assert_eq!(code_of(&r).as_deref(), Some("V754"));
    let u = usage(&p);
    assert_eq!(u.message, "V754 event-rate: too many presses; try again in a moment");
    assert_eq!(u.kind, Some(Kind::Usage));
    assert_eq!(code_of(&u).as_deref(), Some("V754"));
}

// slates_model Behaviour 7
#[test]
fn said_reads_each_outcome_under_a_pressed_piece_by_the_send_s_kind() {
    use DeliveryOutcome::*;
    assert_eq!(said("send", Started), "Sent");
    assert_eq!(said("send", Steered), "Sent into the running turn");
    assert_eq!(said("send", Queued), "Waiting for the turn to end");
    assert_eq!(said("steer", Started), "Nothing was running, so it was sent as the next message");
    assert_eq!(said("steer", Steered), "Sent into the running turn");
    assert_eq!(said("steer", Queued), "Waiting for the turn to end");
    assert_eq!(said("queue", Started), "Sent");
    assert_eq!(said("queue", Steered), "Sent into the running turn; this agent takes messages mid-turn");
    assert_eq!(said("queue", Queued), "Waiting for the turn to end");
}

// slates_model Behaviour 6
#[test]
fn size_of_is_the_utf8_bytes_of_the_json_text() {
    assert_eq!(size_of(&json!({"a": "é"})), 10);
    assert_eq!(size_of(&json!([1, "x"])), 7);
    assert_eq!(size_of(&json!("😀")), 6);
    assert_eq!(size_of(&json!(null)), 4);
}

// slates_model Behaviour 1: the stored record's keys in fresh_record's order, previous and sendsAllowed last
#[test]
fn a_stored_record_keeps_the_keys_of_a_fresh_one_in_order_and_adds_previous_and_sends_allowed_last() {
    support::local(async {
        let rig = rig("wsp-slates-record-keys-");
        let s = &rig.slates;
        agent_write(s, SMALL).await;
        let row = rig.host.store.row("slates", THREAD).expect("the stored record");
        let keys = |row: &Value| row.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        assert_eq!(
            keys(&row),
            names(&[
                "threadId",
                "workspaceId",
                "rootThreadId",
                "schema",
                "version",
                "revision",
                "document",
                "values",
                "turns",
                "kept",
                "comments",
                "approvals",
                "problems",
                "shownOnce",
                "updatedAt"
            ])
        );
        assert_has(
            &row,
            json!({"threadId": THREAD, "workspaceId": WORKSPACE, "rootThreadId": THREAD, "schema": 2, "version": 1, "revision": 1, "values": {"n": 1}, "turns": {}, "kept": {}, "comments": [], "approvals": {}, "problems": [], "shownOnce": false}),
        );
        agent_write(s, r#"<props id="t" tone="good" />"#).await;
        approve(s, "send", ApprovalScope::Thread).await.unwrap();
        s.settled().await;
        let row = rig.host.store.row("slates", THREAD).unwrap();
        let all = keys(&row);
        assert_eq!(all[all.len() - 2..], names(&["previous", "sendsAllowed"])[..]);
        assert_has(&row, json!({"version": 2, "previous": {"version": 1}}));
        assert_eq!(keys(&row["previous"]), names(&["document", "values", "version"]));
    });
}

// slates_model Behaviour 2: turns by turn id in the order they ended, a snapshot by revision
#[test]
fn a_turn_s_end_writes_a_pointer_and_one_snapshot_and_a_turn_that_ends_twice_moves_to_the_end() {
    support::local(async {
        let rig = rig("wsp-slates-turns-");
        let s = &rig.slates;
        s.turn_ended(THREAD, "turn-0").await.unwrap();
        assert!(rig.host.store.row("slates", THREAD).is_none(), "a thread with no slate keeps nothing");
        agent_write(s, SMALL).await;
        s.turn_ended(THREAD, "turn-1").await.unwrap();
        s.turn_ended(THREAD, "turn-2").await.unwrap();
        s.turn_ended(THREAD, "turn-1").await.unwrap();
        let row = rig.host.store.row("slates", THREAD).unwrap();
        let turns = row["turns"].as_object().unwrap();
        assert_eq!(turns.keys().cloned().collect::<Vec<_>>(), names(&["turn-2", "turn-1"]));
        assert_has(&row["turns"]["turn-1"], json!({"version": 1, "revision": 1}));
        assert!(row["turns"]["turn-1"]["at"].is_number());
        let kept = row["kept"].as_object().unwrap();
        assert_eq!(kept.keys().cloned().collect::<Vec<_>>(), names(&["1"]), "turns that changed nothing share one snapshot");
        assert_eq!(kept["1"]["values"], json!({"n": 1}));
        assert_eq!(kept["1"].as_object().unwrap().keys().cloned().collect::<Vec<_>>(), names(&["document", "values"]));
    });
}

// slates_model Behaviour 8 / Edge case: a value whose declaration moved from value to run (or back) starts fresh
#[test]
fn a_value_that_moved_to_a_run_starts_idle_and_a_run_that_moved_to_a_value_starts_at_its_start() {
    support::local(async {
        let rig = rig("wsp-slates-values-moved-");
        let s = &rig.slates;
        agent_write(s, SMALL).await;
        s.state(StateParams { values: vals(&[("n", json!(9))]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert!(num_is(view(s).await.values.get("n"), 9.0));
        agent_write(s, r##"<slate title="Small"><run name="n" cmd="true" /><column><text id="t">hi</text></column></slate>"##).await;
        assert_eq!(val(&view(s).await, "n"), json!({"state": "idle", "runs": 0}));
        agent_write(s, r##"<slate title="Small"><value name="n" start={7} /><column><text id="t">hi</text></column></slate>"##).await;
        assert!(num_is(view(s).await.values.get("n"), 7.0));
    });
}

// slates_model Behaviour 8: names both documents declare keep their live value, new names start, old names go
#[test]
fn a_new_document_keeps_live_values_starts_new_names_and_drops_old_ones() {
    support::local(async {
        let rig = rig("wsp-slates-values-keep-");
        let s = &rig.slates;
        agent_write(s, r##"<slate title="Two"><value name="a" start={1} /><value name="b" start={2} /><column><text id="t">{$a}{$b}</text></column></slate>"##).await;
        s.state(StateParams { values: vals(&[("a", json!(41))]), ..Default::default() }, Some(&agent())).await.unwrap();
        agent_write(s, r##"<slate title="Two"><value name="a" start={100} /><value name="c" start={3} /><column><text id="t">{$a}{$c}</text></column></slate>"##).await;
        let v = view(s).await;
        assert!(num_is(v.values.get("a"), 41.0), "a name both declare keeps its live value");
        assert!(num_is(v.values.get("c"), 3.0), "a name only the new one declares starts");
        assert!(!v.values.contains_key("b"), "a name only the old one declared goes");
    });
}

// slates_model Behaviour 8: each value in the new document's order, then each run
#[test]
fn the_live_values_of_a_new_document_list_its_values_first_then_its_runs() {
    support::local(async {
        let rig = rig("wsp-slates-values-order-");
        let s = &rig.slates;
        agent_write(s, r##"<slate title="Order"><run name="r" cmd="true" /><value name="v" start={1} /><column><text id="t">{$v}</text></column></slate>"##).await;
        let v = view(s).await;
        assert_eq!(v.values.keys().cloned().collect::<Vec<_>>(), names(&["v", "r"]));
        assert_eq!(val(&v, "r"), json!({"state": "idle", "runs": 0}));
    });
}

// slates_model Behaviour 8, 9: a secret keeps its handle through a new document and a dropped one is forgotten
#[test]
fn a_secret_keeps_its_handle_through_a_new_document_and_a_dropped_one_is_forgotten() {
    support::local(async {
        let rig = rig("wsp-slates-values-secret-");
        let s = &rig.slates;
        let with = r##"<slate title="S"><secret name="token" /><column><input id="tok" label="Token" value={$token} kind="password" /></column></slate>"##;
        let without = r##"<slate title="S"><value name="n" start={1} /><column><text id="t">{$n}</text></column></slate>"##;
        agent_write(s, with).await;
        s.state(StateParams { target: target(), values: vals(&[("$token", json!("hunter2"))]), ..Default::default() }, None).await.unwrap();
        assert_has(&val(&view(s).await, "token"), json!({"secret": true, "set": true, "len": 7}));
        agent_write(s, &with.replace("title=\"S\"", "title=\"S two\"")).await;
        assert_has(&val(&view(s).await, "token"), json!({"secret": true, "set": true, "len": 7}));
        agent_write(s, without).await;
        assert!(!view(s).await.values.contains_key("token"));
        agent_write(s, with).await;
        assert_has(&val(&view(s).await, "token"), json!({"secret": true, "set": false}));
    });
}

// slates_model Behaviour 8 / Edge case: a changed command marks a done result stale and keeps out
#[test]
fn a_changed_command_marks_a_done_result_stale_last_in_the_record_and_keeps_its_out() {
    support::local(async {
        let rig = rig("wsp-slates-stale-");
        let s = &rig.slates;
        let doc = |cmd: &str, timeout: u32| {
            format!(
                r##"<slate title="Run"><run name="go" cmd="{cmd}" timeout={{{timeout}}} /><column><button id="b" label="Go" onPress={{start($go)}} /></column></slate>"##
            )
        };
        agent_write(s, &doc("echo one", 20)).await;
        let pressed = s.event(press_params(1, "b", "p1")).await.unwrap();
        approve(s, &pressed.ask.expect("the sheet").key(), ApprovalScope::Once).await.unwrap();
        wait_view(s, 10_000, |v| subset(&val(v, "go"), &json!({"state": "done", "out": "one\n"}))).await;
        agent_write(s, &doc("echo one", 30)).await;
        assert!(val(&view(s).await, "go").get("stale").is_none(), "only timeout changed");
        agent_write(s, &doc("echo two", 30)).await;
        let stale = val(&view(s).await, "go");
        assert_has(&stale, json!({"state": "done", "out": "one\n", "stale": true}));
        assert!(serde_json::to_string(&stale).unwrap().ends_with(r#","stale":true}"#), "stale is added last: {stale}");
    });
}

// slates_model Behaviour 9: a run the new document does not declare is cancelled
#[test]
fn a_run_a_new_document_drops_is_cancelled_and_leaves_nothing_of_its_record() {
    support::local(async {
        let rig = rig("wsp-slates-drop-run-");
        let s = &rig.slates;
        let marker = rig.host.folder().join("finished");
        let doc = |with_run: bool| {
            let run = if with_run {
                format!(r##"<run name="long" cmd='sleep 1; touch "{}"' timeout={{20}} />"##, marker.display())
            } else {
                String::new()
            };
            let press = if with_run { "start($long)" } else { "set($n, 2)" };
            format!(
                r##"<slate title="Long">{run}<value name="n" start={{1}} /><column><button id="b" label="Go" onPress={{{press}}} /></column></slate>"##
            )
        };
        agent_write(s, &doc(true)).await;
        let pressed = s.event(press_params(1, "b", "p1")).await.unwrap();
        approve(s, &pressed.ask.expect("the sheet").key(), ApprovalScope::Once).await.unwrap();
        wait_view(s, 10_000, |v| state_is(v, "long", "running")).await;
        agent_write(s, &doc(false)).await;
        assert!(!view(s).await.values.contains_key("long"));
        support::sleep_ms(1_800).await;
        assert!(!marker.exists(), "the dropped run was killed before it finished");
        agent_write(s, &doc(true)).await;
        assert_eq!(val(&view(s).await, "long"), json!({"state": "idle", "runs": 0}));
    });
}

// slates_model Behaviour 11: a timed run is armed off the document, on its period
#[test]
fn a_document_with_an_always_timer_arms_it_on_its_period_and_a_new_every_arms_again() {
    support::local(async {
        let clock = ManualClock::new(1_000_000);
        let rig = rig_on("wsp-slates-arm-", &clock);
        let s = &rig.slates;
        let doc = |every: u32| {
            format!(
                r##"<slate title="Feed"><run name="feed" cmd="echo hi" every={{{every}}} always timeout={{20}} /><column><output run={{$feed}} /></column></slate>"##
            )
        };
        agent_write(s, &doc(30)).await;
        clock.advance(1);
        s.settled().await;
        assert!(clock.asked.borrow().contains(&30_000), "{:?}", clock.asked.borrow());
        agent_write(s, &doc(60)).await;
        clock.advance(1);
        s.settled().await;
        assert!(clock.asked.borrow().contains(&60_000), "{:?}", clock.asked.borrow());
    });
}

// slates_model Behaviour 12 / Edge case: slates.test.ts:219
#[test]
fn a_rewind_restores_the_turn_s_snapshot_cancels_a_run_in_flight_with_no_reaction() {
    support::local(async {
        let rig = rig("wsp-slates-rewind-");
        let s = &rig.slates;
        s.turn_ended(THREAD, "turn-1").await.unwrap();
        agent_write(s, SETUP).await;
        let wait = s.event(press_params(1, "wait", "w1")).await.unwrap();
        approve(s, &wait.ask.expect("the sheet").key(), ApprovalScope::Thread).await.unwrap();
        wait_view(s, 10_000, |v| state_is(v, "slow", "running")).await;
        s.turn_ended(THREAD, "turn-2").await.unwrap();
        s.state(StateParams { target: target(), values: vals(&[("$project", json!("later"))]), ..Default::default() }, None).await.unwrap();
        s.turn_ended(THREAD, "turn-3").await.unwrap();
        s.settled().await;
        let before = view(s).await;

        s.rewound(THREAD, "turn-2", &["turn-3".to_string()]).await.unwrap();
        s.settled().await;
        let rewound = view(s).await;
        assert_eq!(val(&rewound, "project"), json!(""));
        assert_has(&val(&rewound, "slow"), json!({"state": "cancelled", "why": "the thread was rewound"}));
        assert!(num_is(rewound.values.get("fired"), 0.0));
        assert_eq!(rewound.version, before.version + 1);
        assert_eq!(rewound.revision, before.revision + 1);
        assert_has(
            &serde_json::to_value(rig.host.recorded.borrow().last().unwrap()).unwrap(),
            json!({"cause": "restore", "by": "host", "pieces": []}),
        );
        support::sleep_ms(300).await;
        s.settled().await;
        assert!(num_is(view(s).await.values.get("fired"), 0.0), "the killed command's end is dropped");

        s.rewound(THREAD, "turn-1", &["turn-2".to_string()]).await.unwrap();
        s.settled().await;
        let empty = view(s).await;
        assert!(empty.document.is_none());
        assert_eq!(empty.empty, Some(SlateEmpty::RewoundBefore));
        assert_eq!(empty.approvals.values().map(|a| a.state.clone()).collect::<Vec<_>>(), names(&["allowed"]));
        assert_eq!(val(&empty, "project"), json!(""), "a rewind to before the slate keeps the values");
    });
}

// slates_model Behaviour 12 / Behaviour nobody wrote down: a rewind neither fills nor empties a secret
#[test]
fn a_rewind_reads_a_secret_off_the_runs_table_and_never_fills_or_empties_it() {
    support::local(async {
        let rig = rig("wsp-slates-rewind-secret-");
        let s = &rig.slates;
        agent_write(s, SETUP).await;
        s.turn_ended(THREAD, "turn-1").await.unwrap();
        let typed = |t: &str| StateParams { target: target(), values: vals(&[("$token", json!(t))]), ..Default::default() };
        s.state(typed("hunter2"), None).await.unwrap();
        s.rewound(THREAD, "turn-1", &[]).await.unwrap();
        assert_has(&val(&view(s).await, "token"), json!({"secret": true, "set": true, "len": 7}));
        s.turn_ended(THREAD, "turn-2").await.unwrap();
        s.state(typed(""), None).await.unwrap();
        s.rewound(THREAD, "turn-2", &[]).await.unwrap();
        assert_has(&val(&view(s).await, "token"), json!({"secret": true, "set": false}));
    });
}

// slates_model Behaviour 12: a rewind to a turn with no pointer empties the slate and a read says so
#[test]
fn a_rewind_to_a_turn_with_no_pointer_reads_rewound_before_in_a_read() {
    support::local(async {
        let rig = rig("wsp-slates-rewound-before-");
        let s = &rig.slates;
        agent_write(s, SMALL).await;
        s.rewound(THREAD, "turn-0", &[]).await.unwrap();
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert_eq!(read.problems.iter().map(|p| p.code.as_str()).collect::<Vec<_>>(), vec!["Z804"]);
        assert_eq!(read.problems[0].message, "the thread was rewound to before this slate existed; write one with slate_write");
        assert_eq!(read.version, 2);
    });
}
