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

const PLOT: &str = r##"<slate title="Feed">
  <value name="go" start={0} />
  <run name="feed" cmd='printf %s "$J"' env={{ J: '[{"at":"10:01","n":7}]' }} timeout={20} />
  <when change={$go} do={start($feed)} />
  <column><chart id="fed" label="Fed" items={$feed.json} x={item.at} value={item} /></column>
</slate>"##;

const SMALL: &str = r##"<slate title="Small"><value name="n" start={1} /><column><text id="t">{$n}</text></column></slate>"##;

// slates_writes Behaviour 1, 3 / Edge case: slates.test.ts:316
#[test]
fn changing_how_often_a_command_runs_or_letting_it_run_hidden_asks_again() {
    support::local(async {
        let rig = rig("wsp-slates-key-");
        let s = &rig.slates;
        agent_write(s, TICKER).await;
        let release = s.subscribe(THREAD, &[]);
        let mut asked = String::new();
        async fn held(s: &Rc<dyn Slates>, asked: &mut String) -> String {
            let v = wait_view(s, 5_000, |v| v.asks.len() == 1 && v.asks[0].key() != *asked).await;
            *asked = v.asks[0].key();
            asked.clone()
        }
        async fn ran(s: &Rc<dyn Slates>, n: f64) {
            wait_view(s, 10_000, |v| num_is(v.values.get("ticks"), n)).await;
        }
        let first = held(s, &mut asked).await;
        approve(s, &first, ApprovalScope::Thread).await.unwrap();
        ran(s, 1.0).await;

        agent_write(s, TICKER).await;
        assert!(view(s).await.asks.is_empty(), "the same declaration written again asks nothing");

        agent_write(s, &TICKER.replace("every={60}", "every={120}")).await;
        let slower = held(s, &mut asked).await;
        assert_ne!(slower, first);
        approve(s, &slower, ApprovalScope::Thread).await.unwrap();
        ran(s, 2.0).await;

        agent_write(s, &TICKER.replace("every={60}", "every={120} always")).await;
        let hidden = held(s, &mut asked).await;
        assert!(hidden != first && hidden != slower);

        approve(s, &hidden, ApprovalScope::Thread).await.unwrap();
        agent_write(s, &TICKER.replace("every={60}", "every={120} always").replace("timeout={20}", "timeout={600}")).await;
        let longer = held(s, &mut asked).await;
        assert!(![&first, &slower, &hidden].contains(&&longer));
        approve(s, &longer, ApprovalScope::Thread).await.unwrap();
        wait_view(s, 10_000, |v| state_is(v, "tick", "done")).await;
        agent_write(s, &TICKER.replace("every={60}", "every={120} always").replace("timeout={20}", "timeout={600} once")).await;
        let once = held(s, &mut asked).await;
        assert!(![&first, &slower, &hidden, &longer].contains(&&once));
        release();
    });
}

// slates_writes Edge case: slates.test.ts:373
#[test]
fn a_timed_run_the_person_has_not_allowed_reads_held_from_the_write_and_the_writ() {
    support::local(async {
        let rig = rig("wsp-slates-timed-held-");
        let s = &rig.slates;
        let wrote = agent_write(s, TICKER).await;
        assert!(wrote.problems.is_empty());
        assert_eq!(wrote.waiting, strs(&["$tick"]));
        assert!(like_at(&wrote.text, "slate v1 \"Ticker\", \\d+ pieces, \\d+ bound, 0 problems"), "{}", wrote.text);
        assert!(wrote.text.contains("waiting for the person's approval: $tick; each starts once they allow it on the slate. Until then nothing they feed has data: ask the person to allow them, and never call the panel live or ready"));
        assert!(wrote.text.contains("$tick: held needs your approval"));
        let v = view(s).await;
        assert_has(&val(&v, "tick"), json!({"state": "held"}));
        assert_eq!(v.asks.iter().map(|a| a.run()).collect::<Vec<_>>(), vec!["tick"]);
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert_has(&json!(read.runs), json!({"$tick": {"state": "held"}}));
        assert!(read.problems.is_empty());
        assert_eq!(read.waiting, vec!["$tick".to_string()]);
        let again = s.state(StateParams { start: strs(&["tick"]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert!(again.problems.is_empty());
        assert_eq!(again.waiting, strs(&["$tick"]));
        assert_eq!(again.not_started, strs(&["$tick: it already waits for the person to allow it on the slate, and starts once they do"]));
        assert!(again.text.contains("not started:\n  $tick: it already waits"));
    });
}

// slates_writes Behaviour 5 / Edge case: slates.test.ts:498
#[test]
fn a_check_previews_what_the_panel_shows_for_a_run_s_result_setting_run_json_on() {
    support::local(async {
        let rig = rig("wsp-slates-preview-run-");
        let s = &rig.slates;
        let text = r##"<slate><run name="market" cmd="echo '{}'" every={60} /><column><text when={$market.exit == 0}>{$market.json.open ? "Open" : "Closed"}</text></column></slate>"##;
        let preview = s
            .write(
                WriteParams {
                    text: Some(text.into()),
                    check: Some(true),
                    values: vals(&[("$market.json", json!({"open": true})), ("$market.exit", json!(0))]),
                    ..Default::default()
                },
                Some(&agent()),
            )
            .await
            .unwrap();
        assert!(preview.text.contains("Open  [text-1 text]"), "{}", preview.text);
        assert!(s.get(THREAD).await.unwrap().is_none());
    });
}

// slates_writes Edge case: slates.test.ts:514
#[test]
fn a_run_that_nothing_can_start_warns_in_the_write_s_text_and_the_slate_is_stil() {
    support::local(async {
        let rig = rig("wsp-slates-dead-run-");
        let wrote = agent_write(&rig.slates, r##"<slate title="Error log"><run name="tail" cmd="tail -n 100 -F logs/error.log" stream once /><column><output run={$tail} lines={40} /></column></slate>"##).await;
        assert_eq!(wrote.version, 1);
        assert_eq!(wrote.warnings.iter().map(|w| w.code.as_str()).collect::<Vec<_>>(), vec!["W019"]);
        assert!(wrote.text.contains("W019 slate: nothing starts $tail: give it a button, a when, or every."));
        assert_eq!(view(&rig.slates).await.version, 1);
    });
}

// slates_writes Behaviour 3 / Edge case: slates.test.ts:522
#[test]
fn a_write_s_warnings_are_in_its_text_with_their_fixes_not_only_in_the_structure() {
    support::local(async {
        let rig = rig("wsp-slates-warnings-text-");
        let wrote = agent_write(&rig.slates, r##"<slate title="Live Gold Prices"><column><text>a · b</text></column></slate>"##).await;
        assert_eq!(wrote.warnings.iter().map(|w| w.code.as_str()).collect::<Vec<_>>(), vec!["W018", "W012"]);
        assert!(wrote.text.starts_with("slate v1 \"Live Gold Prices\", 2 pieces, 0 bound, 0 problems, 2 warnings\n"), "{}", wrote.text);
        assert!(wrote.text.contains(
            r#"W018 slate: the slate's title "Live Gold Prices" is in Title Case; heads take sentence case. Fix: title="Live gold prices""#
        ));
    });
}

// slates_writes Edge case: slates.test.ts:536
#[test]
fn a_refused_write_names_every_error_on_its_one_line_numbered_not_the_first_alon() {
    support::local(async {
        let rig = rig("wsp-slates-every-error-");
        let s = &rig.slates;
        let two = refusal(
            s.write(
                text_write(r##"<slate><column><text value={$nope} /><meter label="x" value={1} tone="loud" /></column></slate>"##),
                Some(&agent()),
            )
            .await,
        );
        let m = &two.message;
        let at =
            |from: usize, needle: &str| m[from..].find(needle).map(|i| from + i).unwrap_or_else(|| panic!("{needle} missing from {m}"));
        assert!(m.starts_with("slate refused: 2 errors: 1) "), "{m}");
        let s501 = at(0, "S501 ");
        let second = at(s501, " 2) ");
        at(second, "T306 ");
        assert_eq!(two.kind, Some(Kind::Invalid));
        let one = refusal(s.write(text_write(r##"<slate><text value={$nope} /></slate>"##), Some(&agent())).await);
        let rest = one.message.strip_prefix("slate refused: 1 error: ").unwrap_or_else(|| panic!("{}", one.message));
        assert!(!rest.starts_with('1'), "{rest}");
        assert!(rest.contains("S501"), "{rest}");
    });
}

// slates_writes Edge case: slates.test.ts:1158
#[test]
fn refuses_a_file_that_names_a_secret() {
    support::local(async {
        let rig = rig("wsp-slates-filesecret-");
        let e = refusal(
            rig.slates
                .write(
                    text_write(
                        r##"<slate>
  <secret name="token" />
  <run name="use" cmd='sh "$SLATE_DIR/use.sh"' />
  <column><input id="tok" label="Token" value={$token} kind="password" /></column>
  <file name="use.sh">{`curl -H "Authorization: $token" example.com`}</file>
</slate>"##,
                    ),
                    Some(&agent()),
                )
                .await,
        );
        assert_eq!(e.kind, Some(Kind::Invalid));
        let errors = e.errors.expect("the validator's errors");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["code"], json!("S520"));
    });
}

// slates_writes Edge case: slates.test.ts:1010
#[test]
fn slate_read_shows_each_file_s_text_in_the_printed_slate_and_its_size_in_the_sk() {
    support::local(async {
        let rig = rig("wsp-slates-files-");
        let s = &rig.slates;
        let wrote = agent_write(s, "<slate>\n  <run name=\"hits\" cmd='python3 \"$SLATE_DIR/hits.py\"' />\n  <column />\n  <file name=\"hits.py\">{`print(1)`}</file>\n</slate>").await;
        assert!(wrote.text.contains("files in $SLATE_DIR: hits.py (8 B)"), "{}", wrote.text);
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert!(read.text.contains("<file name=\"hits.py\">{`\n    print(1)\n  `}</file>"), "{}", read.text);
        agent_write(s, "<file name=\"hits.py\">{`print(22)`}</file>").await;
        let again = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert!(again.text.contains("    print(22)\n"), "{}", again.text);
    });
}

// slates_writes Behaviour 6 / Edge case: slates.test.ts:965
#[test]
fn the_agent_starts_a_run_the_person_said_always_to_and_an_unapproved_or_ask_eve() {
    support::local(async {
        let rig = rig("wsp-slates-agent-start-");
        let s = &rig.slates;
        agent_write(s, PROBE).await;

        let before = s.state(StateParams { start: strs(&["probe"]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert_eq!(
            before.not_started,
            strs(&[
                r#"$probe: you start only a run the person allowed "Always in this thread"; a press, a <when> or every= starts it and the slate asks them"#
            ])
        );
        assert!(before.problems.is_empty());
        let untouched = view(s).await;
        assert_has(&val(&untouched, "probe"), json!({"state": "idle"}));
        assert!(untouched.asks.is_empty());
        let near = refusal(s.state(StateParams { start: strs(&["prob"]), ..Default::default() }, Some(&agent())).await);
        assert!(near.message.contains("K702 run-name") && near.message.contains("Did you mean $probe"), "{}", near.message);
        let neither = refusal(s.state(StateParams::default(), Some(&agent())).await);
        assert!(neither.message.contains("names neither"), "{}", neither.message);

        s.state(StateParams { target: target(), values: vals(&[("$n", json!(1))]), ..Default::default() }, None).await.unwrap();
        let asked = view(s).await;
        approve(s, &asked.asks[0].key(), ApprovalScope::Thread).await.unwrap();
        wait_view(s, 10_000, |v| num_is(v.values.get("done"), 1.0)).await;
        let delivered = rig.host.delivered.borrow().len();

        let started = s.state(StateParams { start: strs(&["$probe"]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert!(started.not_started.is_none());
        assert!(like_in(&started.text, "$probe: running") || like_in(&started.text, "$probe: done"), "{}", started.text);
        wait_view(s, 10_000, |v| num_is(v.values.get("done"), 2.0)).await;
        assert_has(&val(&view(s).await, "probe"), json!({"state": "done", "out": "run 1\n", "runs": 2}));
        assert_eq!(rig.host.delivered.borrow().len(), delivered, "no turn starts");

        let ask = s.state(StateParams { start: strs(&["ask"]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert_eq!(
            ask.not_started,
            strs(&["$ask: it has confirm, so it asks the person every start and only a press or a <when> starts it"])
        );
        assert_has(&val(&view(s).await, "ask"), json!({"state": "idle"}));
        assert!(ask.text.contains("[ More ]  [more button]"), "{}", ask.text);
        assert!(!ask.text.contains("held"), "{}", ask.text);
    });
}

// slates_writes Behaviour 6 / Edge case: slates.test.ts:723
#[test]
fn a_paired_computer_writes_values_and_starts_nothing_no_reaction_fires_off_its() {
    support::local(async {
        let rig = rig("wsp-slates-paired-");
        let s = &rig.slates;
        let set_n = |n: i64| StateParams { target: target(), values: vals(&[("$n", json!(n))]), ..Default::default() };
        agent_write(s, PROBE).await;
        s.state(set_n(1), Some(&here())).await.unwrap();
        let asked = view(s).await;
        approve(s, &asked.asks[0].key(), ApprovalScope::Thread).await.unwrap();
        wait_view(s, 10_000, |v| subset(&val(v, "probe"), &json!({"state": "done", "out": "run 1\n", "runs": 1}))).await;

        s.state(set_n(5), Some(&paired())).await.unwrap();
        let after = view(s).await;
        assert!(num_is(after.values.get("n"), 5.0));
        assert_has(&val(&after, "probe"), json!({"state": "done", "out": "run 1\n", "runs": 1}));
        let refused =
            refusal(s.state(StateParams { target: target(), start: strs(&["$probe"]), ..Default::default() }, Some(&paired())).await);
        assert!(refused.message.contains("a paired computer writes a slate's values and starts no run"), "{}", refused.message);
        assert_eq!(refused.kind, Some(Kind::Usage));
        assert_has(&val(&view(s).await, "probe"), json!({"state": "done", "runs": 1}));

        s.state(set_n(1), Some(&here())).await.unwrap();
        wait_view(s, 10_000, |v| subset(&val(v, "probe"), &json!({"state": "done", "out": "run 1\n", "runs": 2}))).await;
    });
}

// slates_writes Edge case: slates.test.ts:826
#[test]
fn a_plotted_value_that_is_not_a_number_is_a_problem_in_a_read_once_a_run_fills_() {
    support::local(async {
        let rig = rig("wsp-slates-plot-");
        let s = &rig.slates;
        let wrote = agent_write(s, PLOT).await;
        assert!(!wrote.text.contains("R905"), "{}", wrote.text);
        s.state(StateParams { target: target(), values: vals(&[("$go", json!(1))]), ..Default::default() }, None).await.unwrap();
        approve(s, &view(s).await.asks[0].key(), ApprovalScope::Thread).await.unwrap();
        wait_view(s, 10_000, |v| state_is(v, "feed", "done")).await;
        let broken = r#"R905 fed.value: value={item} read an object on 1 of 1 rows, like {"at":"10:01","n":7}, so it draws no point for them; a plotted value is a number, like value={item.n}"#;
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert!(read.text.contains(broken), "{}", read.text);
        let state = s.state(StateParams { values: vals(&[("$go", json!(1))]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert!(state.text.contains(broken), "{}", state.text);
        let fixed = agent_write(s, r#"<props id="fed" value={item.n} />"#).await;
        assert!(!fixed.text.contains("R905"), "{}", fixed.text);
        assert!(fixed.text.contains("over 1 point"), "{}", fixed.text);
        let back = agent_write(s, r#"<props id="fed" value={item} />"#).await;
        assert!(back.text.contains(broken), "{}", back.text);
    });
}

// slates_writes Behaviour 1, 5, 6 / Edge case: slates.test.ts:588
#[test]
fn a_check_rehearses_values_and_a_press_on_a_copy_a_bare_name_writes_name_and_a_() {
    support::local(async {
        let rig = rig("wsp-slates-rehearse-");
        let s = &rig.slates;
        agent_write(s, QUIZ).await;
        let live = view(s).await;

        let tried = s
            .write(
                WriteParams {
                    check: Some(true),
                    values: vals(&[("i", json!(2))]),
                    press: Some(Press { piece: "pick".into(), index: Some(1), action: Some(0) }),
                    ..Default::default()
                },
                Some(&agent()),
            )
            .await
            .unwrap();
        assert!(tried.text.contains("question 2"), "{}", tried.text);
        assert!(tried.text.contains("picked b"), "{}", tried.text);
        assert!(tried.text.contains("would start $grade (a press on pick)"), "{}", tried.text);
        assert!(tried.text.contains(r#"would send "Graded.""#), "{}", tried.text);
        let after = view(s).await;
        assert_eq!(after.values, live.values);
        assert_eq!((after.version, after.revision), (live.version, live.revision));
        assert!(after.asks.is_empty());

        let next = s
            .write(
                WriteParams {
                    text: Some(QUIZ.replace("question {$i}", "Q{$i}")),
                    check: Some(true),
                    values: vals(&[("$i", json!(5))]),
                    ..Default::default()
                },
                Some(&agent()),
            )
            .await
            .unwrap();
        assert!(next.text.contains("Q5"), "{}", next.text);
        assert_eq!(view(s).await.version, live.version);
        let no_check = refusal(s.write(WriteParams { values: vals(&[("i", json!(1))]), ..Default::default() }, Some(&agent())).await);
        assert!(no_check.message.contains("only a check"), "{}", no_check.message);
        let missing = refusal(
            s.write(
                WriteParams {
                    check: Some(true),
                    press: Some(Press { piece: "pik".into(), index: None, action: None }),
                    ..Default::default()
                },
                Some(&agent()),
            )
            .await,
        );
        assert!(missing.message.contains("D203") && missing.message.contains("Did you mean pick"), "{}", missing.message);

        s.state(StateParams { values: vals(&[("i", json!(3))]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert!(num_is(view(s).await.values.get("i"), 3.0));

        let pressed = s.event(press_params(1, "grade-it", "g1")).await.unwrap();
        approve(s, &pressed.ask.expect("the sheet").key(), ApprovalScope::Once).await.unwrap();
        wait_view(s, 10_000, |v| subset(&val(v, "grade"), &json!({"state": "done", "out": "graded paris\n"}))).await;
        agent_write(s, &QUIZ.replace("paris", "rome")).await;
        let stale = view(s).await;
        assert_has(&val(&stale, "grade"), json!({"state": "done", "out": "graded paris\n", "stale": true}));
        let read = s.read(ReadParams::default(), Some(&agent())).await.unwrap();
        assert!(like_in(&read.text, "$grade: done (exit 0, \\d+ ms), stale: the command changed since it ran"), "{}", read.text);
        let again = s.event(press_params(3, "grade-it", "g2")).await.unwrap();
        approve(s, &again.ask.expect("the sheet").key(), ApprovalScope::Once).await.unwrap();
        let fresh = wait_view(s, 10_000, |v| subset(&val(v, "grade"), &json!({"state": "done", "out": "graded rome\n"}))).await;
        assert!(val(&fresh, "grade").get("stale").is_none());
        agent_write(s, &QUIZ.replace("paris", "rome").replace("timeout={20}", "timeout={30}")).await;
        assert!(val(&view(s).await, "grade").get("stale").is_none(), "how long it may run is not what it runs");
    });
}

// slates_writes Behaviour 1 / Edge case: values and press rehearse only under a check
#[test]
fn values_or_a_press_without_check_refuse_with_the_fix_to_add_it() {
    support::local(async {
        let rig = rig("wsp-slates-nocheck-");
        let e = refusal(
            rig.slates
                .write(WriteParams { text: Some(SMALL.into()), values: vals(&[("n", json!(2))]), ..Default::default() }, Some(&agent()))
                .await,
        );
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains("values and press rehearse a slate, which only a check does."), "{}", e.message);
        assert_eq!(e.fix.as_deref(), Some("Add check, or set the live values with slate_state."));
        assert!(rig.slates.get(THREAD).await.unwrap().is_none());
    });
}

// slates_writes Behaviour 1: exactly one of text or document
#[test]
fn a_write_of_neither_text_nor_document_or_of_both_is_refused_a601() {
    support::local(async {
        let rig = rig("wsp-slates-a601-");
        let s = &rig.slates;
        let none = refusal(s.write(WriteParams::default(), Some(&agent())).await);
        assert_eq!(none.kind, Some(Kind::Usage));
        assert_eq!(code_of(&none).as_deref(), Some("A601"));
        assert!(
            none.message.contains("A601 step-arg: a slate write takes exactly one of text or document, and this one has none."),
            "{}",
            none.message
        );
        assert_eq!(none.fix.as_deref(), Some("Send text alone."));
        let both = refusal(
            s.write(WriteParams { text: Some(SMALL.into()), document: Some(json!({"schema": 2})), ..Default::default() }, Some(&agent()))
                .await,
        );
        assert_eq!(code_of(&both).as_deref(), Some("A601"));
        assert!(both.message.contains("and this one has both."), "{}", both.message);
    });
}

// slates_writes Behaviour 1: a rehearsal with no document to rehearse
#[test]
fn a_rehearsal_of_the_live_slate_with_no_slate_refuses_z802() {
    support::local(async {
        let rig = rig("wsp-slates-norehearse-");
        let e = refusal(
            rig.slates
                .write(WriteParams { check: Some(true), values: vals(&[("n", json!(1))]), ..Default::default() }, Some(&agent()))
                .await,
        );
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(code_of(&e).as_deref(), Some("Z802"));
        assert!(e.message.contains("Z802 no-slate: this thread has no slate to rehearse; send the <slate> with check"), "{}", e.message);
    });
}

// slates_writes Behaviour 1: a patch needs a slate to patch
#[test]
fn a_patch_with_no_slate_refuses_z802_and_asks_for_a_whole_slate() {
    support::local(async {
        let rig = rig("wsp-slates-nopatch-");
        let e = refusal(rig.slates.write(text_write(r#"<props id="t" tone="good" />"#), Some(&agent())).await);
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(code_of(&e).as_deref(), Some("Z802"));
        assert!(e.message.contains("Z802 no-slate: this thread has no slate to patch; write a whole <slate> first"), "{}", e.message);
    });
}

// slates_writes Behaviour 1: a check stores nothing and leaves the version
#[test]
fn a_check_of_a_whole_slate_answers_its_sketch_at_the_version_read_and_stores_no() {
    support::local(async {
        let rig = rig("wsp-slates-check-whole-");
        let s = &rig.slates;
        let checked =
            s.write(WriteParams { text: Some(SMALL.into()), check: Some(true), ..Default::default() }, Some(&agent())).await.unwrap();
        assert_eq!(checked.version, 0);
        assert!(checked.problems.is_empty());
        assert!(checked.text.contains("Small"), "{}", checked.text);
        assert!(s.get(THREAD).await.unwrap().is_none());
        assert!(rig.host.recorded.borrow().is_empty());
        agent_write(s, SMALL).await;
        let patched = s
            .write(
                WriteParams { text: Some(r#"<props id="t" tone="good" />"#.into()), check: Some(true), ..Default::default() },
                Some(&agent()),
            )
            .await
            .unwrap();
        assert_eq!(patched.version, 1);
        assert_eq!(view(s).await.version, 1);
    });
}

// slates_writes Behaviour 1 / Edge case: a write at a stale version
#[test]
fn a_write_at_a_version_that_is_not_the_slate_s_refuses_v750() {
    support::local(async {
        let rig = rig("wsp-slates-v750-");
        let s = &rig.slates;
        agent_write(s, SMALL).await;
        let e = refusal(s.write(WriteParams { text: Some(SMALL.into()), if_version: Some(5), ..Default::default() }, Some(&agent())).await);
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(code_of(&e).as_deref(), Some("V750"));
        assert!(e.message.contains("V750 version-behind: the slate is at version 1, not 5; read it and write again"), "{}", e.message);
        let ok = s
            .write(
                WriteParams { text: Some(r#"<props id="t" tone="good" />"#.into()), if_version: Some(1), ..Default::default() },
                Some(&agent()),
            )
            .await
            .unwrap();
        assert_eq!(ok.version, 2);
    });
}

// slates_writes Behaviour 2: the agent's budget, the person's none
#[test]
fn the_agent_s_twenty_first_write_at_once_refuses_v751_and_the_person_s_never() {
    support::local(async {
        let clock = ManualClock::new(1_000_000);
        let rig = rig_on("wsp-slates-budget-", &clock);
        let s = &rig.slates;
        for _ in 0..20 {
            agent_write(s, SMALL).await;
        }
        let e = refusal(s.write(text_write(SMALL), Some(&agent())).await);
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(code_of(&e).as_deref(), Some("V751"));
        assert!(e.message.contains("V751 write-rate: wait 1 s; a slate that rewrites itself constantly is a bug"), "{}", e.message);
        assert_eq!(view(s).await.version, 20);
        for _ in 0..25 {
            s.write(window_write(SMALL), None).await.expect("the person's writes spend no budget");
        }
        assert_eq!(view(s).await.version, 45);
    });
}

// slates_writes Behaviour 3: the record, the announcement and the pushes of a write
#[test]
fn a_write_announces_every_piece_id_and_pushes_every_value() {
    support::local(async {
        let rig = rig("wsp-slates-announce-");
        let s = &rig.slates;
        let wrote = agent_write(s, QUIZ).await;
        assert_eq!(wrote.version, 1);
        let recorded = rig.host.recorded.borrow().clone();
        assert_eq!(recorded.len(), 1);
        let event = serde_json::to_value(&recorded[0]).unwrap();
        assert_has(&event, json!({"threadId": THREAD, "cause": "write", "version": 1, "by": "agent"}));
        let pieces = event["pieces"].as_array().unwrap();
        for id in ["q", "p", "pick", "grade-it"] {
            assert!(pieces.contains(&json!(id)), "{id} missing from {pieces:?}");
        }
        let pushes = rig.host.pushes();
        let last = pushes.last().expect("the values were pushed");
        let sent = serde_json::to_value(last).unwrap();
        let keys: Vec<&str> = sent["values"].as_object().unwrap().keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, vec!["$i", "$picked", "$grade"]);
        assert_eq!(sent["version"], json!(1));
        let person = s.write(window_write(QUIZ), None).await.unwrap();
        assert_eq!(person.version, 2);
        assert_has(&serde_json::to_value(rig.host.recorded.borrow().last().unwrap()).unwrap(), json!({"by": "person", "version": 2}));
    });
}

// slates_writes Behaviour 3: pr.checks makes the host watch the pull request
#[test]
fn a_document_that_names_pr_checks_makes_the_host_watch_the_pull_request() {
    support::local(async {
        let rig = rig("wsp-slates-watchpr-");
        let s = &rig.slates;
        agent_write(s, SMALL).await;
        assert!(rig.host.watched.borrow().is_empty());
        assert!(!s.watches_pr(WORKSPACE));
        agent_write(s, r##"<slate title="Pull request"><column><text id="n">{len(pr.checks)}</text></column></slate>"##).await;
        assert_eq!(rig.host.watched.borrow().clone(), vec![WORKSPACE.to_string()]);
        assert!(s.watches_pr(WORKSPACE));
    });
}

// slates_writes Behaviour 3, 4: a patch keeps one step back, undo puts it back and keeps one step too
#[test]
fn a_patch_keeps_the_slate_before_it_and_undo_puts_it_back() {
    support::local(async {
        let rig = rig("wsp-slates-undo-");
        let s = &rig.slates;
        let nothing = refusal(s.write(text_write("<undo />"), Some(&agent())).await);
        assert!(nothing.message.contains("Z802"), "{}", nothing.message);
        agent_write(s, QUIZ).await;
        assert!(!view(s).await.can_undo);
        let none = refusal(s.write(text_write("<undo />"), Some(&agent())).await);
        assert_eq!(none.kind, Some(Kind::Conflict));
        assert_eq!(code_of(&none).as_deref(), Some("V752"));
        assert!(
            none.message.contains("V752 nothing-to-undo: there is no earlier slate to go back to; the undo keeps one step"),
            "{}",
            none.message
        );
        let patched = agent_write(s, r#"<props id="q" tone="good" />"#).await;
        assert_eq!(patched.version, 2);
        let v2 = view(s).await;
        assert!(v2.can_undo);
        let checked =
            s.write(WriteParams { text: Some("<undo />".into()), check: Some(true), ..Default::default() }, Some(&agent())).await.unwrap();
        assert_eq!(checked.version, 2);
        assert_eq!(view(s).await.version, 2);
        let undone = agent_write(s, "<undo />").await;
        assert_eq!(undone.version, 3);
        assert!(undone.warnings.is_empty() && undone.problems.is_empty());
        let v3 = view(s).await;
        assert!(v3.can_undo, "the undo keeps one step: the slate it undid");
        assert_ne!(v3.document, v2.document);
        assert_has(
            &serde_json::to_value(rig.host.recorded.borrow().last().unwrap()).unwrap(),
            json!({"cause": "undo", "by": "agent", "version": 3, "pieces": []}),
        );
    });
}

// slates_writes Behaviour 1, 3: a clear keeps the values and reads cleared
#[test]
fn a_clear_empties_the_slate_keeps_its_values_and_reads_cleared() {
    support::local(async {
        let rig = rig("wsp-slates-clear-");
        let s = &rig.slates;
        agent_write(s, QUIZ).await;
        s.state(StateParams { values: vals(&[("i", json!(4))]), ..Default::default() }, Some(&agent())).await.unwrap();
        let cleared = agent_write(s, "<clear />").await;
        assert_eq!(cleared.version, 2);
        let v = view(s).await;
        assert!(v.document.is_none());
        assert_eq!(v.empty, Some(wsp_slates::pending::wire::SlateEmpty::Cleared));
        assert!(num_is(v.values.get("i"), 4.0), "a clear keeps the values");
        assert_has(&serde_json::to_value(rig.host.recorded.borrow().last().unwrap()).unwrap(), json!({"cause": "clear", "version": 2}));
        let e = refusal(s.state(StateParams { values: vals(&[("i", json!(1))]), ..Default::default() }, Some(&agent())).await);
        assert_eq!(code_of(&e).as_deref(), Some("Z802"));
        assert!(e.message.contains("Z802 no-slate: this slate is empty; write one with slate_write"), "{}", e.message);
    });
}

// slates_writes Behaviour 6: a state with no slate, a stale version, and names that are not values
#[test]
fn a_state_with_no_slate_or_at_a_version_behind_is_refused() {
    support::local(async {
        let rig = rig("wsp-slates-state-refused-");
        let s = &rig.slates;
        let none = refusal(s.state(StateParams { values: vals(&[("n", json!(1))]), ..Default::default() }, Some(&agent())).await);
        assert_eq!(none.kind, Some(Kind::Usage));
        assert!(none.message.contains("Z802 no-slate: this thread has no slate yet; write one with slate_write"), "{}", none.message);
        agent_write(s, SMALL).await;
        let behind = refusal(
            s.state(StateParams { values: vals(&[("n", json!(2))]), if_version: Some(9), ..Default::default() }, Some(&agent())).await,
        );
        assert_eq!(behind.kind, Some(Kind::Conflict));
        assert!(behind.message.contains("V750 version-behind: the slate is at version 1, not 9"), "{}", behind.message);
        let neither = refusal(s.state(StateParams::default(), Some(&agent())).await);
        assert_eq!(neither.kind, Some(Kind::Usage));
        assert!(neither.message.contains("slate state sets values or starts runs, and this one names neither."), "{}", neither.message);
        assert_eq!(neither.fix.as_deref(), Some("Give values, start, or both."));
    });
}

// slates_writes Behaviour 6: a path that is not a value, a derived value, a run, an undeclared name
#[test]
fn a_state_names_that_are_not_value_paths_refuse_with_the_nearest_name() {
    support::local(async {
        let rig = rig("wsp-slates-state-names-");
        let s = &rig.slates;
        agent_write(
            s,
            r##"<slate><value name="counter" start={1} /><derived name="d" value={$counter + 1} /><run name="r" cmd="true" /><column><text id="t">{$d}</text></column></slate>"##,
        )
        .await;
        let set = |k: &str| StateParams { values: vals(&[(k, json!(2))]), ..Default::default() };
        let bad = refusal(s.state(set("!!"), Some(&agent())).await);
        assert_eq!(bad.kind, Some(Kind::Invalid));
        assert!(
            bad.message.contains("S501 name-undeclared !! is not a value path; a value path reads $name, with .field and [index] below it"),
            "{}",
            bad.message
        );
        let derived = refusal(s.state(set("$d"), Some(&agent())).await);
        assert!(
            derived
                .message
                .contains("A607 set-not-value $d is a derived value and cannot be written. Did you mean write the value it reads?"),
            "{}",
            derived.message
        );
        let run = refusal(s.state(set("r"), Some(&agent())).await);
        assert!(run.message.contains("A607 set-not-value $r is a run and cannot be written"), "{}", run.message);
        let near = refusal(s.state(set("$countr"), Some(&agent())).await);
        assert!(near.message.contains("S501 name-undeclared $countr is not declared. Did you mean $counter?"), "{}", near.message);
        assert!(num_is(view(s).await.values.get("counter"), 1.0));
    });
}

// slates_writes Behaviour 6: a secret takes only the person's typed text at its root
#[test]
fn a_secret_takes_only_the_person_s_typed_string_and_an_empty_one_clears_it() {
    support::local(async {
        let rig = rig("wsp-slates-secret-state-");
        let s = &rig.slates;
        agent_write(
            s,
            r##"<slate><secret name="token" /><column><input id="tok" label="Token" value={$token} kind="password" /></column></slate>"##,
        )
        .await;
        let by_agent =
            refusal(s.state(StateParams { values: vals(&[("$token", json!("x"))]), ..Default::default() }, Some(&agent())).await);
        assert!(
            by_agent.message.contains("S520 secret-exposed $token is a secret: only the person types it, into its input"),
            "{}",
            by_agent.message
        );
        let window = |v: Value| StateParams { target: target(), values: vals(&[("$token", v)]), ..Default::default() };
        let not_text = refusal(s.state(window(json!(5)), None).await);
        assert!(not_text.message.contains("S520 secret-exposed $token takes the typed text"), "{}", not_text.message);
        let below =
            refusal(s.state(StateParams { target: target(), values: vals(&[("$token.x", json!("t"))]), ..Default::default() }, None).await);
        assert!(below.message.contains("S520"), "{}", below.message);
        s.state(window(json!("hunter2")), None).await.unwrap();
        let typed = val(&view(s).await, "token");
        assert_has(&typed, json!({"secret": true, "set": true, "len": 7}));
        assert!(!serde_json::to_string(&typed).unwrap().contains("hunter2"));
        s.state(window(json!("")), None).await.unwrap();
        assert_has(&val(&view(s).await, "token"), json!({"secret": true, "set": false}));
    });
}

// slates_writes Behaviour 6: the agent's value write is announced, and a person's is not
#[test]
fn the_agent_s_state_is_announced_as_a_state_and_the_person_s_is_not() {
    support::local(async {
        let rig = rig("wsp-slates-state-announce-");
        let s = &rig.slates;
        agent_write(s, SMALL).await;
        assert_eq!(rig.host.recorded.borrow().len(), 1);
        s.state(StateParams { target: target(), values: vals(&[("$n", json!(2))]), ..Default::default() }, None).await.unwrap();
        assert_eq!(rig.host.recorded.borrow().len(), 1, "a person's state is not a transcript event");
        s.state(StateParams { values: vals(&[("$n", json!(3))]), ..Default::default() }, Some(&agent())).await.unwrap();
        assert_eq!(rig.host.recorded.borrow().len(), 2);
        assert_has(
            &serde_json::to_value(rig.host.recorded.borrow().last().unwrap()).unwrap(),
            json!({"cause": "state", "by": "agent", "pieces": []}),
        );
        let v = view(s).await;
        assert!(num_is(v.values.get("n"), 3.0));
        assert_eq!(v.version, 1);
    });
}
