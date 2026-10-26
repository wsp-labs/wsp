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

use wsp_slates::pending::slate::SlateDoc;
use wsp_slates::pending::wire::SlateEmpty;
use wsp_slates::slates::model::SlateRecord;
use wsp_slates::slates::reads::{bound_paths, waiting_of};

const SMALL: &str = r##"<slate title="Small"><value name="n" start={1} /><column><text id="t">{$n}</text></column></slate>"##;

fn record(doc: Option<Value>, values: Value) -> SlateRecord {
    SlateRecord {
        thread_id: THREAD.into(),
        workspace_id: WORKSPACE.into(),
        root_thread_id: THREAD.into(),
        schema: 2,
        version: 1,
        revision: Some(1),
        document: doc.map(|d| serde_json::from_value::<SlateDoc>(d).expect("a document")),
        values: serde_json::from_value(values).expect("values"),
        empty: None,
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

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn ask_without_key(ask: &SlateAsk) -> String {
    let mut j = ask.to_json();
    j.as_object_mut().unwrap().remove("key");
    serde_json::to_string(&j).unwrap()
}

// slates_reads Behaviour 4: every binding, every when and every derived expression, once each, less item and index
#[test]
fn bound_paths_lists_bindings_whens_and_derived_once_each_less_item_and_index() {
    let doc: SlateDoc = serde_json::from_value(json!({
        "schema": 2,
        "root": "column-1",
        "values": {},
        "derived": {"d": "$n"},
        "runs": {},
        "reactions": [],
        "pieces": {
            "column-1": {"type": "column", "children": ["a", "b", "c"]},
            "a": {"type": "text", "props": {"value": {"bind": "$n"}, "label": "x", "nested": {"k": [{"bind": "git.branch"}]}}, "when": "$flag"},
            "b": {"type": "text", "props": {"text": {"format": "hi ${$name} and ${time.now}"}}},
            "c": {"type": "text", "props": {"value": {"bind": "item.x"}, "label": {"bind": "index"}}}
        }
    }))
    .unwrap();
    assert_eq!(bound_paths(&doc), names(&["$n", "git.branch", "$flag", "$name", "time.now"]));
}

// slates_reads Behaviour 6: the runs held with no why or the approval why
#[test]
fn waiting_of_names_each_declared_run_held_with_no_why_or_the_approval_why() {
    let doc = json!({
        "schema": 2,
        "root": "column-1",
        "values": {},
        "derived": {},
        "runs": {
            "a": {"kind": "cmd", "cmd": "x"},
            "b": {"kind": "cmd", "cmd": "x"},
            "c": {"kind": "cmd", "cmd": "x"},
            "d": {"kind": "cmd", "cmd": "x"},
            "e": {"kind": "cmd", "cmd": "x"}
        },
        "reactions": [],
        "pieces": {"column-1": {"type": "column"}}
    });
    let values = json!({
        "a": {"state": "held", "runs": 0},
        "b": {"state": "held", "why": "needs your approval", "runs": 0},
        "c": {"state": "held", "why": "too many starts", "runs": 0},
        "d": {"state": "running", "runs": 1}
    });
    assert_eq!(waiting_of(&record(Some(doc), values)), names(&["$a", "$b"]));
}

// slates_reads Edge case: slates.test.ts:400
#[test]
fn a_read_carries_each_run_s_json_without_its_out_twice_cuts_long_output_and_onl() {
    support::local(async {
        let rig = rig("wsp-slates-read-size-");
        let s = &rig.slates;
        agent_write(
            s,
            r##"<slate><run name="big" cmd='python3 -c "import json; print(json.dumps({\"rows\": [\"x\" * 50] * 200}))"' /><run name="raw" cmd='python3 -c "print(\"y\" * 9000)"' /><column><button id="go" label="Go" onPress={[start($big), start($raw)]} /></column></slate>"##,
        )
        .await;
        press(s, "go", "r-size").await.unwrap();
        for ask in view(s).await.asks {
            approve(s, &ask.key(), ApprovalScope::Once).await.unwrap();
        }
        wait_view(s, 10_000, |v| state_is(v, "big", "done") && state_is(v, "raw", "done")).await;
        let whole = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert!(whole.runs["$big"].get("json").is_some());
        assert!(whole.runs["$big"].get("out").is_none());
        let out = whole.runs["$raw"]["out"].as_str().unwrap();
        assert!(out.ends_with("… (9001 characters; values [\"$raw.out\"] reads it whole)"), "{}", &out[out.len() - 80..]);
        let named = s.read(ReadParams { values: strs(&["$raw.out"]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert!(named.runs["$big"].get("json").is_none());
        assert_eq!(named.values["$raw.out"].as_str().unwrap().len(), 9001);
        assert!(named.text.contains("... (9004 characters)"), "the path's line is cut at 4,000 units");
    });
}

// slates_reads Edge case: slates.test.ts:506
#[test]
fn a_read_puts_the_paths_it_was_asked_for_in_its_text_not_only_its_structured_an() {
    support::local(async {
        let rig = rig("wsp-slates-read-text-");
        let s = &rig.slates;
        agent_write(s, TICKER).await;
        let read = s
            .read(ReadParams { values: strs(&["$ticks", "$tick.state"]), text: Some(false), ..Default::default() }, Some(&agent()))
            .await
            .unwrap();
        assert!(read.text.contains("read:\n  $ticks = 0\n  $tick.state = \"held\""), "{}", read.text);
        assert!(!read.text.contains("<slate"));
    });
}

// slates_reads Edge case: slates.test.ts:530
#[test]
fn a_thread_with_no_slate_yet_reads_as_an_empty_slate_not_a_refusal() {
    support::local(async {
        let rig = rig("wsp-slates-empty-read-");
        let s = &rig.slates;
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert_eq!(read.version, 0);
        assert_eq!(read.text, "slate v0, empty: write one with slate_write");
        assert!(read.problems.is_empty());
        assert!(read.document.is_none());
        let with_document = s.read(ReadParams { document: Some(true), ..Default::default() }, Some(&agent())).await.unwrap();
        assert_eq!(with_document.document, Some(None));
        assert!(rig.host.store.row("slates", THREAD).is_none(), "a read stores nothing");
    });
}

// slates_reads Edge case: slates.test.ts:543
#[test]
fn a_read_asked_for_a_run_or_a_value_without_its_dollar_says_where_it_is_and_one() {
    support::local(async {
        let rig = rig("wsp-slates-misread-");
        let s = &rig.slates;
        agent_write(s, TICKER).await;
        let read = s
            .read(
                ReadParams {
                    values: strs(&["tick", "ticks", "spot.json", "time.now", "$ticks", "len(git.changed)"]),
                    ..Default::default()
                },
                Some(&agent()),
            )
            .await
            .unwrap();
        let x401: Vec<Value> = read.problems.iter().filter(|p| p.code == "X401").map(|p| serde_json::to_value(p).unwrap()).collect();
        assert_eq!(
            x401,
            vec![
                json!({"code": "X401", "name": "path-unknown", "message": "tick is not a source: $tick is a run, read as $tick; its record is also under runs.$tick", "fix": "$tick"}),
                json!({"code": "X401", "name": "path-unknown", "message": "ticks is not a source: $ticks is a value, read as $ticks", "fix": "$ticks"}),
                json!({"code": "X401", "name": "path-unknown", "message": "spot is not a source and not this slate's; a slate's own names start with $"}),
            ]
        );
        assert!(read.text.contains("X401 slate: tick is not a source"), "{}", read.text);
    });
}

// slates_reads Edge case: slates.test.ts:555
#[test]
fn a_read_answers_the_jsx_like_form_by_default_and_the_json_document_only_when_a() {
    support::local(async {
        let rig = rig("wsp-slates-read-");
        let s = &rig.slates;
        agent_write(s, TICKER).await;
        let plain = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert!(plain.text.starts_with("slate v1"), "{}", plain.text);
        assert!(plain.text.contains(r#"<slate title="Ticker">"#));
        assert!(plain.text.contains(r#"<run name="tick""#));
        assert!(plain.document.is_none());
        let json = s.read(ReadParams { document: Some(true), text: Some(false), ..Default::default() }, Some(&agent())).await.unwrap();
        assert_has(&json.document.clone().flatten().expect("the document"), json!({"schema": 2, "title": "Ticker"}));
        assert!(!json.text.contains("<slate"), "{}", json.text);

        agent_write(s, r##"<slate><value name="at" start={0} /><text id="since">since {ago($at)}</text></slate>"##).await;
        let aged = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert!(like_in(&aged.text, "since \\d+d "), "{}", aged.text);
        assert!(aged.problems.is_empty());
    });
}

// slates_reads Edge case: slates.test.ts:393
#[test]
fn a_read_names_each_approval_by_the_run_it_is_for_not_by_its_key() {
    support::local(async {
        let rig = rig("wsp-slates-approval-names-");
        let s = &rig.slates;
        agent_write(s, TICKER).await;
        approve(s, &view(s).await.asks[0].key(), ApprovalScope::Thread).await.unwrap();
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert_eq!(serde_json::to_value(&read.approvals).unwrap(), json!({"$tick": "allowed"}));
        approve(s, "domain:example.com", ApprovalScope::Thread).await.unwrap();
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert_eq!(
            serde_json::to_value(&read.approvals).unwrap(),
            json!({"$tick": "allowed", "domain:example.com": "allowed"}),
            "a key no run declares stays its key"
        );
    });
}

// slates_reads Edge case: slates.test.ts:373
#[test]
fn a_timed_run_the_person_has_not_allowed_reads_held_in_a_read_and_is_no_problem() {
    support::local(async {
        let rig = rig("wsp-slates-read-held-");
        let s = &rig.slates;
        agent_write(s, TICKER).await;
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert_has(&json!(read.runs), json!({"$tick": {"state": "held"}}));
        assert!(read.problems.is_empty());
        assert_eq!(read.waiting, names(&["$tick"]));
    });
}

// slates_reads Edge case: slates.test.ts:826
#[test]
fn a_plotted_value_that_is_not_a_number_reads_r905_once_a_run_fills_it() {
    support::local(async {
        let rig = rig("wsp-slates-read-plot-");
        let s = &rig.slates;
        agent_write(
            s,
            r##"<slate title="Feed">
  <value name="go" start={0} />
  <run name="feed" cmd='printf %s "$J"' env={{ J: '[{"at":"10:01","n":7}]' }} timeout={20} />
  <when change={$go} do={start($feed)} />
  <column><chart id="fed" label="Fed" items={$feed.json} x={item.at} value={item} /></column>
</slate>"##,
        )
        .await;
        let early = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert!(!early.text.contains("R905"), "no data yet, so nothing read wrong yet");
        s.state(StateParams { target: target(), values: vals(&[("$go", json!(1))]), ..Default::default() }, None).await.unwrap();
        approve(s, &view(s).await.asks[0].key(), ApprovalScope::Thread).await.unwrap();
        wait_view(s, 10_000, |v| state_is(v, "feed", "done")).await;
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        let broken = r#"R905 fed.value: value={item} read an object on 1 of 1 rows, like {"at":"10:01","n":7}, so it draws no point for them; a plotted value is a number, like value={item.n}"#;
        assert!(read.text.contains(broken), "{}", read.text);
    });
}

// slates_reads Behaviour 10: `*` means every bound path, and a path asked for is read in the answer
#[test]
fn a_star_reads_every_path_the_document_binds_and_nothing_else() {
    support::local(async {
        let rig = rig("wsp-slates-read-star-");
        let s = &rig.slates;
        let none = s.read(ReadParams { values: strs(&["*", "$x"]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert_eq!(serde_json::to_value(&none.values).unwrap(), json!({"$x": null}), "no document: the asked list less *");
        agent_write(s, SMALL).await;
        let star = s.read(ReadParams { values: strs(&["*"]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert_eq!(serde_json::to_value(&star.values).unwrap(), json!({"$n": 1}));
        assert!(star.text.contains("read:\n  $n = 1"), "{}", star.text);
    });
}

// slates_reads Behaviour 10: the sketch alone, then the paths, then a blank line and the printed document
#[test]
fn a_read_s_text_is_the_sketch_then_the_paths_then_a_blank_line_and_the_document() {
    support::local(async {
        let rig = rig("wsp-slates-read-shape-");
        let s = &rig.slates;
        agent_write(s, SMALL).await;
        let bare = s
            .read(ReadParams { values: strs(&["$n"]), sketch: Some(false), text: Some(false), ..Default::default() }, Some(&agent()))
            .await
            .unwrap();
        assert_eq!(bare.text, "slate v1\nread:\n  $n = 1");
        let full = s.read(ReadParams { values: strs(&["$n"]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert!(full.text.contains("  $n = 1\n\n<slate title=\"Small\">"), "{}", full.text);
    });
}

// slates_reads Behaviour 10: state, derived and runs are answered by name with their dollar
#[test]
fn a_read_answers_values_derived_values_and_runs_each_by_its_dollar_name() {
    support::local(async {
        let rig = rig("wsp-slates-read-parts-");
        let s = &rig.slates;
        agent_write(s, r##"<slate title="D"><value name="n" start={2} /><derived name="d" value={$n + 1} /><run name="r" cmd="true" /><column><text id="t">{$d}</text></column></slate>"##).await;
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert_eq!(serde_json::to_value(&read.state).unwrap(), json!({"$n": 2}));
        assert_eq!(serde_json::to_value(&read.derived).unwrap(), json!({"$d": 3}));
        assert_eq!(serde_json::to_value(&read.runs).unwrap(), json!({"$r": {"state": "idle", "runs": 0}}));
        assert!(read.waiting.is_empty());
        assert!(read.comments.is_empty());
    });
}

// slates_reads Behaviour 10: a path's line is cut at 4,000 units, the structured answer is whole
#[test]
fn a_path_s_line_in_the_text_is_cut_at_4000_units_and_its_value_is_whole() {
    support::local(async {
        let rig = rig("wsp-slates-read-cut-");
        let s = &rig.slates;
        let big = "x".repeat(5_000);
        agent_write(
            s,
            &format!(r##"<slate title="Big"><value name="big" start="{big}" /><column><text id="t">hi</text></column></slate>"##),
        )
        .await;
        let read = s.read(ReadParams { values: strs(&["$big"]), text: Some(false), ..Default::default() }, Some(&agent())).await.unwrap();
        let line = format!("  $big = \"{}... (5002 characters)", "x".repeat(3_999));
        assert!(read.text.contains(&line), "the line is the first 4,000 units of the JSON, then the total");
        assert_eq!(read.values["$big"].as_str().unwrap().len(), 5_000);
    });
}

// slates_reads Behaviour 5: a bound path that resolves to nothing is R900
#[test]
fn a_bound_source_path_with_no_value_reads_r900_data_missing() {
    support::local(async {
        let rig = rig("wsp-slates-read-r900-");
        let s = &rig.slates;
        agent_write(s, r##"<slate title="Pr"><column><text id="n">{pr.number}</text></column></slate>"##).await;
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        let found: Vec<(String, String, String)> =
            read.problems.iter().map(|p| (p.code.clone(), p.name.clone(), p.message.clone())).collect();
        assert_eq!(found, vec![("R900".to_string(), "data-missing".to_string(), "pr.number has no value yet".to_string())]);
    });
}

// slates_reads Behaviour 10: every answer string goes through the thread's scrub
#[test]
fn a_secret_a_run_printed_comes_back_scrubbed_from_a_read_and_a_resolve() {
    support::local(async {
        let rig = rig("wsp-slates-read-scrub-");
        let s = &rig.slates;
        let token = "vcl_tok_9f8e7d6c5b4a3210";
        agent_write(
            s,
            r##"<slate title="Sec"><secret name="token" /><run name="env" cmd='echo "wrote $TOK"' env={{ TOK: $token }} timeout={20} /><column><input id="tok" label="Token" value={$token} kind="password" /><button id="go" label="Go" onPress={start($env)} /></column></slate>"##,
        )
        .await;
        s.state(StateParams { target: target(), values: vals(&[("$token", json!(token))]), ..Default::default() }, None).await.unwrap();
        let pressed = press(s, "go", "r-sec").await.unwrap();
        assert_eq!(pressed.outcome, SlateEventOutcome::Held);
        approve(s, &pressed.ask.expect("the sheet").key(), ApprovalScope::Once).await.unwrap();
        wait_view(s, 10_000, |v| state_is(v, "env", "done")).await;
        let read = s.read(ReadParams { values: strs(&["$env.out", "$token"]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert_eq!(read.values["$env.out"], json!("wrote [secret:token]\n"));
        assert_has(&read.values["$token"], json!({"secret": true, "set": true}));
        assert!(!format!("{read:?}").contains(token));
        let resolved = s.resolve(THREAD, &["$env.out".to_string(), "$token".to_string()]).await.unwrap();
        assert_eq!(resolved["$env.out"], json!("wrote [secret:token]\n"));
        assert!(!format!("{resolved:?}").contains(token));
    });
}

// slates_reads Behaviour 12: no record answers every path null, else each path evaluated
#[test]
fn a_resolve_answers_null_for_a_thread_with_no_slate_and_each_path_evaluated_for_one_with() {
    support::local(async {
        let rig = rig("wsp-slates-resolve-");
        let s = &rig.slates;
        let paths = vec!["$n".to_string(), "$nope".to_string()];
        let none = s.resolve(THREAD, &paths).await.unwrap();
        assert_eq!(serde_json::to_value(&none).unwrap(), json!({"$n": null, "$nope": null}));
        agent_write(s, SMALL).await;
        let some = s.resolve(THREAD, &paths).await.unwrap();
        assert_eq!(serde_json::to_value(&some).unwrap(), json!({"$n": 1, "$nope": null}));
    });
}

// slates_reads Behaviour 11: a sub-thread has no slate, so no catalog either
#[test]
fn a_sub_thread_s_catalog_refuses_z803() {
    support::local(async {
        let rig = rig("wsp-slates-catalog-sub-");
        let child = "22222222-aaaa-4bbb-8ccc-000000000002";
        rig.host.add_thread(child, THREAD, None);
        let e = refusal(rig.slates.catalog(CatalogParams::default(), Some(&as_thread(child, THREAD))).await);
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(code_of(&e).as_deref(), Some("Z803"));
        assert!(e.message.contains("Z803 sub-thread: a thread another thread started has no slate; tell the thread that started you what to show, and it puts it on its own"), "{}", e.message);
    });
}

// slates_reads Behaviour 11: a name the catalog lacks is looked up as an MCP server of the thread's agent
#[test]
fn a_catalog_name_that_is_not_in_the_catalog_asks_the_threads_mcp_servers_and_says_why_it_fails() {
    support::local(async {
        let rig = rig("wsp-slates-catalog-name-");
        let s = &rig.slates;
        agent_write(s, SMALL).await;
        let by_thread = s.catalog(CatalogParams { name: Some("zzz".into()), ..Default::default() }, Some(&agent())).await.unwrap();
        assert!(by_thread.contains("zzz is not in the catalog"), "{by_thread}");
        assert!(by_thread.contains("\nzzz as an MCP server of this thread's agent: "), "{by_thread}");
        let window = s.catalog(CatalogParams { name: Some("zzz".into()), ..Default::default() }, None).await.unwrap();
        assert!(window.contains("zzz is not in the catalog"), "{window}");
        assert!(!window.contains("as an MCP server"), "a window that names no thread gets the text alone");
        let all = s.catalog(CatalogParams::default(), Some(&agent())).await.unwrap();
        assert!(!all.is_empty());
        assert!(!all.contains("as an MCP server"));
    });
}

// slates_reads Behaviour 9: the view's keys in the TS's order, empty only when there is no document
#[test]
fn the_window_s_view_keeps_the_keys_of_the_ts_in_order_and_adds_empty_only_with_no_document() {
    support::local(async {
        let rig = rig("wsp-slates-view-keys-");
        let s = &rig.slates;
        agent_write(s, SMALL).await;
        let json = view(s).await.to_json();
        let keys: Vec<&str> = json.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        assert_eq!(
            keys,
            [
                "threadId",
                "workspaceId",
                "version",
                "revision",
                "document",
                "values",
                "comments",
                "approvals",
                "asks",
                "problems",
                "shownOnce",
                "canUndo",
                "updatedAt"
            ]
        );
        assert_has(
            &json,
            json!({"threadId": THREAD, "workspaceId": WORKSPACE, "version": 1, "revision": 1, "values": {"n": 1}, "comments": [], "approvals": {}, "asks": [], "problems": [], "shownOnce": false, "canUndo": false}),
        );
        agent_write(s, "<clear />").await;
        let cleared = view(s).await;
        assert_eq!(cleared.empty, Some(SlateEmpty::Cleared));
        let keys: Vec<String> = cleared.to_json().as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys[4..7], ["document", "values", "empty"]);
    });
}

// slates_reads Behaviour 9: a thread's permission to message the agent reads as a send approval, canUndo follows previous
#[test]
fn a_view_adds_the_send_approval_once_the_person_allowed_it_and_can_undo_follows_previous() {
    support::local(async {
        let rig = rig("wsp-slates-view-send-");
        let s = &rig.slates;
        agent_write(s, SMALL).await;
        assert!(!view(s).await.approvals.contains_key("send"));
        approve(s, "send", ApprovalScope::Thread).await.unwrap();
        let v = view(s).await;
        assert_eq!(v.approvals["send"].state, "allowed");
        assert!(v.approvals["send"].at > 0);
        assert!(!v.can_undo);
        agent_write(s, r#"<props id="t" tone="good" />"#).await;
        assert!(view(s).await.can_undo);
        approve(s, "send", ApprovalScope::Refuse).await.unwrap();
        assert!(!view(s).await.approvals.contains_key("send"));
    });
}

// slates_reads Behaviour 9: shown marks the slate shown once, and a thread with no slate stays without one
#[test]
fn shown_marks_the_slate_shown_once_and_makes_none_for_a_thread_without() {
    support::local(async {
        let rig = rig("wsp-slates-shown-");
        let s = &rig.slates;
        s.shown(THREAD).await.unwrap();
        assert!(s.get(THREAD).await.unwrap().is_none());
        agent_write(s, SMALL).await;
        assert!(!view(s).await.shown_once);
        s.shown(THREAD).await.unwrap();
        assert!(view(s).await.shown_once);
    });
}

// slates_reads Behaviour 8: a held command's ask, keys in the TS's order, the computer's own name
#[test]
fn a_held_command_asks_with_its_env_args_computer_folder_timeout_and_why_in_order() {
    support::local(async {
        let rig = rig("wsp-slates-ask-shape-");
        let s = &rig.slates;
        let folder = rig.host.folder().display().to_string();
        agent_write(s, r##"<slate title="Say"><run name="say" cmd="echo old" /><column><button id="go" label="Say" onPress={start($say)} /></column></slate>"##).await;
        let pressed = press(s, "go", "go-1").await.unwrap();
        let ask = pressed.ask.expect("the sheet");
        assert_eq!(
            ask_without_key(&ask),
            format!(
                r#"{{"run":"say","kind":"cmd","cmd":"echo old","env":{{}},"args":[],"computer":"this computer","folder":{},"timeoutS":60,"why":"needs your approval"}}"#,
                serde_json::to_string(&folder).unwrap()
            )
        );
        rig.host.threads.borrow_mut().get_mut(THREAD).unwrap().computer = Some("Pixel 9".into());
        agent_write(s, r##"<slate title="Say"><run name="say" cmd="echo new" confirm="Say it?" /><column><button id="go" label="Say" onPress={start($say)} /></column></slate>"##).await;
        let again = press(s, "go", "go-2").await.unwrap();
        let ask = again.ask.expect("the sheet");
        assert_eq!(
            ask_without_key(&ask),
            format!(
                r#"{{"run":"say","kind":"cmd","cmd":"echo new","env":{{}},"args":[],"computer":"Pixel 9","folder":{},"timeoutS":60,"confirm":"Say it?","why":"needs your approval"}}"#,
                serde_json::to_string(&folder).unwrap()
            )
        );
    });
}

// slates_reads Behaviour 7: a script edited since the person said Always names itself in the ask
#[test]
fn an_edited_script_names_itself_in_the_ask_that_asks_again() {
    support::local(async {
        let rig = rig("wsp-slates-script-changed-");
        let s = &rig.slates;
        std::fs::write(rig.host.folder().join("deploy.sh"), "echo safe\n").unwrap();
        agent_write(s, r##"<slate><run name="deploy" cmd="bash deploy.sh" /><column><button id="go" label="Deploy" onPress={start($deploy)} /></column></slate>"##).await;
        let first = press(s, "go", "p1").await.unwrap();
        approve(s, &first.ask.expect("the sheet").key(), ApprovalScope::Thread).await.unwrap();
        wait_view(s, 10_000, |v| subset(&val(v, "deploy"), &json!({"state": "done", "out": "safe\n"}))).await;
        std::fs::write(rig.host.folder().join("deploy.sh"), "echo changed\n").unwrap();
        press(s, "go", "p3").await.unwrap();
        let v = view(s).await;
        assert_has(&val(&v, "deploy"), json!({"state": "held"}));
        assert_eq!(v.asks.len(), 1);
        assert_has(&v.asks[0].to_json(), json!({"run": "deploy", "why": "deploy.sh changed since you allowed it, so it asks again"}));
    });
}
