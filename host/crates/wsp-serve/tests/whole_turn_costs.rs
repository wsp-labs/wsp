// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/turn-costs.test.ts at 65f6b89c7, case for case. What a Claude turn files in the ledger, driven end to
//! end: the runtime, the Claude adapter and a real process on this computer, with stand-ins for the CLI that print the lines
//! Claude Code 2.1.280 printed in print mode (measured 2026-10-06 on a Linux machine): a turn its background subagent woke, which
//! prints a result per wake, and turns cut at the wall before their last result. Each test names the TS test above it. The TS
//! `memoryStore()` is the sqlite store here.
mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::local::{local_wiring_with, write_stub, LocalOpts};
use common::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::port::HarnessAdapter;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_engine::copier::{fake_copier, Copier};
use wsp_wire::types::usage::{UsageRange, UsageSplit};
use wsp_words::exit::HostError;

const HEAD: &str = "#!/bin/sh\nfor a; do [ \"$prev\" = --session-id ] && sid=$a; prev=$a; done\n[ -n \"$sid\" ] || exit 0\nread -r _\n";
const INIT: &str =
    r#"{"type":"system","subtype":"init","cwd":"$PWD","session_id":"$sid","tools":["Bash","Task"],"model":"claude-opus-5-5"}"#;

fn call(id: &str, usage: Value, parent: &str, model: &str) -> String {
    format!(
        r#"{{"type":"assistant","message":{{"model":"{model}","id":"{id}","type":"message","role":"assistant","content":[{{"type":"text","text":"working"}}],"usage":{usage}}},"parent_tool_use_id":{parent},"session_id":"$sid"}}"#
    )
}

fn call_o(id: &str, usage: Value) -> String {
    call(id, usage, "null", "claude-opus-5-5")
}

fn tokens(input: i64, write: i64, read: i64, output: i64) -> Value {
    json!({ "input_tokens": input, "cache_creation_input_tokens": write, "cache_read_input_tokens": read, "output_tokens": output })
}

#[derive(Clone, Copy)]
struct Use {
    input: i64,
    read: i64,
    write: i64,
    out: i64,
}

fn row(t: Use, cost: f64) -> Value {
    json!({ "inputTokens": t.input, "outputTokens": t.out, "cacheReadInputTokens": t.read, "cacheCreationInputTokens": t.write, "webSearchRequests": 0, "costUSD": cost, "contextWindow": 1_000_000 })
}

fn used(t: Use, cost: f64) -> Value {
    json!({ "claude-opus-5-5": row(t, cost) })
}

fn result(text: &str, total: f64, usage: Value, models: Value, woke: bool) -> String {
    let mut doc = json!({
        "type": "result", "subtype": "success", "is_error": false, "duration_ms": 900, "num_turns": 1, "result": text, "session_id": "$sid",
        "total_cost_usd": total, "usage": usage, "modelUsage": models,
    });
    if woke {
        doc["origin"] = json!({ "kind": "task-notification" });
    }
    doc.to_string()
}

fn printed(lines: &[String]) -> String {
    format!("cat <<EOF\n{}\nEOF\n", lines.join("\n"))
}

const TASK_SET: &str = r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"a1","task_type":"local_agent","description":"echo there"}],"session_id":"$sid"}"#;
const TASK_NONE: &str = r#"{"type":"system","subtype":"background_tasks_changed","tasks":[],"session_id":"$sid"}"#;
const TASK_DONE: &str = r#"{"type":"system","subtype":"task_notification","task_id":"a1","status":"completed","summary":"echo there completed","session_id":"$sid"}"#;

fn u1() -> Value {
    tokens(4, 10_000, 20_000, 3)
}

fn with_output(usage: &Value, out: i64) -> Value {
    let mut v = usage.clone();
    v["output_tokens"] = json!(out);
    v
}

/// The agent hands a job to a subagent in the background and replies; the subagent's end wakes it, and it replies again. Each
/// result's usage counts the agent's own calls since it last stopped; modelUsage counts all of them.
fn woken() -> String {
    format!(
        "{HEAD}{}sleep 0.3\n{}cat >/dev/null\n",
        printed(&[
            INIT.into(),
            call_o("msg_1", tokens(4, 10_000, 20_000, 3)),
            TASK_SET.replace("local_bash", "local_agent"),
            call("msg_2", tokens(4, 166, 24_218, 2), "\"toolu_1\"", "claude-opus-5-5"),
            result(
                "Waiting on the subagent.",
                0.08,
                tokens(4, 10_000, 20_000, 1_500),
                used(Use { input: 8, read: 44_218, write: 10_166, out: 1_681 }, 0.08),
                false
            ),
        ]),
        printed(&[
            TASK_NONE.into(),
            TASK_DONE.into(),
            INIT.into(),
            call_o("msg_3", tokens(10, 887, 27_898, 5)),
            result("done", 0.1, tokens(10, 887, 27_898, 144), used(Use { input: 18, read: 72_116, write: 11_053, out: 1_825 }, 0.1), true),
        ])
    )
}

/// Two calls, then work that outlasts the wall: the CLI never gets to print a result.
fn cut() -> String {
    format!(
        "{HEAD}{}sleep 30\n",
        printed(&[
            INIT.into(),
            call_o("msg_1", tokens(6, 40_000, 0, 7)),
            call_o("msg_1", tokens(6, 40_000, 0, 90)),
            call_o("msg_2", tokens(3, 500, 40_000, 12)),
        ])
    )
}

/// The woken stand-in whose woken agent prints one call and is then cut by the wall before it replies.
fn woken_cut() -> String {
    format!(
        "{HEAD}{}sleep 0.3\n{}sleep 30\n",
        printed(&[
            INIT.into(),
            call_o("msg_1", u1()),
            TASK_SET.into(),
            call("msg_2", tokens(4, 166, 24_218, 2), "\"toolu_1\"", "claude-opus-5-5"),
            result(
                "Waiting on the subagent.",
                0.08,
                with_output(&u1(), 1_500),
                used(Use { input: 8, read: 44_218, write: 10_166, out: 1_681 }, 0.08),
                false
            ),
        ]),
        printed(&[TASK_NONE.into(), TASK_DONE.into(), INIT.into(), call_o("msg_3", tokens(10, 887, 27_898, 5))])
    )
}

fn held_reply() -> String {
    result(
        "Waiting on the subagent.",
        0.084016,
        with_output(&u1(), 1_500),
        used(Use { input: 4, read: 20_000, write: 10_000, out: 1_500 }, 0.084016),
        false,
    )
}

fn after_reply() -> String {
    call("msg_2", tokens(4, 0, 500_000, 2), "\"toolu_1\"", "claude-opus-5-5")
}

/// The agent replies while its background subagent runs; the subagent prints a call after that reply, and the wall cuts the turn
/// before anything wakes the agent.
fn held_cut() -> String {
    format!(
        "{HEAD}{}sleep 0.3\n{}sleep 30\n",
        printed(&[INIT.into(), call_o("msg_1", u1()), TASK_SET.into(), held_reply()]),
        printed(&[after_reply()])
    )
}

struct Rates {
    input: f64,
    out: f64,
    read: f64,
    write: f64,
}

const OPUS: Rates = Rates { input: 4e-6, out: 2e-5, read: 2e-7, write: 5e-6 };
const SONNET: Rates = Rates { input: 3e-6, out: 1.5e-5, read: 3e-7, write: 3.75e-6 };

fn list_of(r: &Rates, t: Use) -> f64 {
    t.input as f64 * r.input + t.read as f64 * r.read + t.write as f64 * r.write + t.out as f64 * r.out
}

/// The session's totals at the second result of keptTwice, which reports them at exactly their list price.
const KEPT_TOTALS: Use = Use { input: 18, read: 550_000, write: 10_500, out: 1_522 };

fn kept_list() -> f64 {
    list_of(&OPUS, KEPT_TOTALS)
}

/// Two turns on one process: the first a held reply whose background subagent then prints a call and ends with nothing woken, the
/// second one call and a result whose totals run on from the session's start.
fn kept_twice() -> String {
    format!(
        "{HEAD}{}sleep 0.3\n{}read -r _\n{}cat >/dev/null\n",
        printed(&[INIT.into(), call_o("msg_1", u1()), TASK_SET.into(), held_reply()]),
        printed(&[after_reply(), TASK_NONE.into(), TASK_DONE.into()]),
        printed(&[
            INIT.into(),
            call_o("msg_3", tokens(10, 500, 30_000, 20)),
            result("two", kept_list(), tokens(10, 500, 30_000, 20), used(KEPT_TOTALS, kept_list()), false),
        ])
    )
}

const SONNET_AFTER: Use = Use { input: 5, read: 0, write: 2_000, out: 40 };
const OPUS_AT_TWO: Use = Use { input: 14, read: 50_000, write: 10_500, out: 1_520 };

fn new_model_list() -> f64 {
    list_of(&OPUS, OPUS_AT_TWO) + list_of(&SONNET, SONNET_AFTER)
}

/// keptTwice with the background subagent on Sonnet, a model the session had not used: its call after the reply names it bare,
/// and the next result keys it with its context window, as Claude Code keys a 1M model.
fn kept_new_model() -> String {
    let result_two = json!({
        "type": "result", "subtype": "success", "is_error": false, "duration_ms": 900, "num_turns": 1, "result": "two", "session_id": "$sid",
        "total_cost_usd": new_model_list(), "usage": tokens(10, 500, 30_000, 20),
        "modelUsage": {
            "claude-opus-5-5": row(OPUS_AT_TWO, list_of(&OPUS, OPUS_AT_TWO)),
            "claude-sonnet-5[1m]": row(SONNET_AFTER, list_of(&SONNET, SONNET_AFTER)),
        },
    })
    .to_string();
    format!(
        "{HEAD}{}sleep 0.3\n{}read -r _\n{}cat >/dev/null\n",
        printed(&[INIT.into(), call_o("msg_1", u1()), TASK_SET.into(), held_reply()]),
        printed(&[call("msg_2", tokens(5, 2_000, 0, 3), "\"toolu_1\"", "claude-sonnet-5"), TASK_NONE.into(), TASK_DONE.into()]),
        printed(&[INIT.into(), call_o("msg_3", tokens(10, 500, 30_000, 20)), result_two])
    )
}

/// The first launch answers at its own list price and ends; the second is cut at the wall before any result.
fn done_then_cut(counter: &str) -> String {
    format!(
        "{HEAD}n=$(cat {counter} 2>/dev/null || echo 0); echo $((n+1)) > {counter}\nif [ \"$n\" = 0 ]; then\n{}cat >/dev/null\nelse\n{}sleep 30\nfi\n",
        printed(&[
            INIT.into(),
            call_o("msg_1", u1()),
            result("done", 0.072016, with_output(&u1(), 900), used(Use { input: 4, read: 20_000, write: 10_000, out: 900 }, 0.072016), false)
        ]),
        printed(&[INIT.into(), call_o("msg_7", tokens(6, 40_000, 0, 90)), call_o("msg_8", tokens(3, 500, 40_000, 12))])
    )
}

fn prices() -> Value {
    let rates = |r: &Rates| json!({ "input_cost_per_token": r.input, "output_cost_per_token": r.out, "cache_read_input_token_cost": r.read, "cache_creation_input_token_cost": r.write });
    json!({ "claude-opus-5-5": rates(&OPUS), "claude-sonnet-5": rates(&SONNET) })
}

struct World {
    fx: Fx,
    root: tempfile::TempDir,
    _state: State,
}

/// A runtime whose claude is `script` on this computer's PATH, its turns' wall at `deadline_ms` where given.
fn runtime_of(script: &str, deadline_ms: Option<f64>) -> World {
    let root = tempfile::tempdir().expect("a folder");
    let path = std::fs::canonicalize(root.path()).expect("a real path");
    let bin = path.join("bin");
    std::fs::create_dir(&bin).expect("a folder");
    write_stub(&bin.join("claude"), script);
    let copier: Rc<dyn Copier> = Rc::new(fake_copier(None, None));
    let on = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()));
    let local = local_wiring_with(
        &path,
        copier,
        LocalOpts { deadline_ms, vars: Some(IndexMap::from([("PATH".to_string(), on)])), ..Default::default() },
    );
    let adapter: HarnessAdapterFactory = Arc::new(|ctx| {
        let mut deps = AdapterDeps::new(ctx.exec_stream.clone(), (ctx.home)("claude"));
        deps.base_env = Some(ctx.env.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect());
        deps.result_exit_ms = Some(250);
        let adapter = create_claude_adapter(deps).map_err(|e| HostError::new(e.0))?;
        Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
    });
    let state = sqlite_state();
    let backend = StubBackend::new(None);
    let fetch: Rc<dyn Fn() -> wsp_core::types::wiring::Local<'static, Value>> = Rc::new(|| Box::pin(async { Ok(prices()) }));
    let fx = runtime(&backend)
        .store(&state.store)
        .adapter("claude", adapter)
        .with(move |o| {
            o.local = Some(local);
            o.prices_fetch = Some(fetch);
        })
        .build();
    World { fx, root, _state: state }
}

async fn rows_of(w: &World) -> Vec<Value> {
    let answer = w.fx.rt.usage().used(UsageRange::Day, UsageSplit::Model, None).await.expect("usage");
    j(&answer)["rows"].as_array().expect("rows").clone()
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 5e-10, "{a} is not near {b}");
}

async fn turn(w: &World) -> (Value, Option<Value>) {
    let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
    let run = w.fx.rt.sessions().start(&ws, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.expect("a start");
    let result = j(&run.finished().await);
    let done = w.fx.rt.sessions().history(&ws, None).await.expect("a history").iter().map(j).find(|e| e["type"] == "session.done");
    (result, done.map(|d| d["result"].clone()))
}

fn pairs(rows: &[Value], f: impl Fn(&Value) -> Vec<Value>) -> Vec<Vec<Value>> {
    rows.iter().map(f).collect()
}

// "what a Claude turn files in the ledger" > "a turn its background subagent woke counts every token since it started: the turn row's tokens are its split by model"
#[test]
fn a_turn_its_background_subagent_woke_counts_every_token_since_it_started_the_turn_rows_tokens_are_its_split_by_model() {
    drive(|| async {
        let w = runtime_of(&woken(), None);
        let (result, filed) = turn(&w).await;
        assert_matches(&result, json!({ "status": "completed", "text": "done", "costUsd": 0.1 }));
        let filed = filed.expect("a filed result");
        let split = filed["models"].as_array().expect("a split");
        assert_eq!(split.iter().map(|m| m["model"].clone()).collect::<Vec<_>>(), [json!("claude-opus-5-5")]);
        let sum = json!({ "input": 83_187, "output": 1_825, "cached": 72_116, "cacheWrite": 11_053 });
        assert_matches(&split[0]["tokens"], sum.clone());
        assert_matches(&filed["tokens"], sum);
        let rows = rows_of(&w).await;
        assert_eq!(
            pairs(&rows, |r| vec![r["tokens"]["input"].clone(), r["tokens"]["output"].clone(), r["costReported"].clone()]),
            [vec![json!(83_187), json!(1_825), json!(0.1)]]
        );
        w.fx.rt.close().await.expect("a close");
        drop(w.root);
    });
}

// "what a Claude turn files in the ledger" > "a woken agent cut before its reply files the calls it printed under the turn's model, the turn counted once"
#[test]
fn a_woken_agent_cut_before_its_reply_files_the_calls_it_printed_under_the_turns_model_the_turn_counted_once() {
    drive(|| async {
        let w = runtime_of(&woken_cut(), Some(1_500.0));
        let (result, _) = turn(&w).await;
        assert_eq!(result["status"], "failed");
        let rows = rows_of(&w).await;
        assert_eq!(
            pairs(&rows, |r| vec![r["key"].clone(), r["tokens"]["input"].clone(), r["turns"].clone(), r["costReported"].clone()]),
            [vec![json!("claude:claude-opus-5-5"), json!(54_392 + 28_795), json!(1), json!(0.08)]]
        );
        w.fx.rt.close().await.expect("a close");
        drop(w.root);
    });
}

// "what a Claude turn files in the ledger" > "a held reply the wall cuts before anything wakes the agent files the calls its background subagent printed after it"
#[test]
fn a_held_reply_the_wall_cuts_before_anything_wakes_the_agent_files_the_calls_its_background_subagent_printed_after_it() {
    drive(|| async {
        let w = runtime_of(&held_cut(), Some(1_500.0));
        turn(&w).await;
        let rows = rows_of(&w).await;
        assert_eq!(
            pairs(&rows, |r| vec![r["key"].clone(), r["tokens"]["input"].clone(), r["turns"].clone(), r["costReported"].clone()]),
            [vec![json!("claude:claude-opus-5-5"), json!(30_004 + 500_004), json!(1), json!(0.084016)]]
        );
        // The reply's cost is its own tokens' list price, and the calls after it, which came with none, take theirs.
        near(num(&rows[0]["costList"]), 500_000.0 * 2e-7 + 4.0 * 4e-6 + 2.0 * 2e-5);
        near(num(&rows[0]["costReported"]) + num(&rows[0]["costList"]), num(&rows[0]["estimate"]));
        w.fx.rt.close().await.expect("a close");
        drop(w.root);
    });
}

// "what a Claude turn files in the ledger" > "a turn cut at the wall keeps its list price in a row beside a turn of the same hour that reported a cost"
#[test]
fn a_turn_cut_at_the_wall_keeps_its_list_price_in_a_row_beside_a_turn_of_the_same_hour_that_reported_a_cost() {
    drive(|| async {
        let counter = tempfile::tempdir().expect("a folder");
        let count = counter.path().join("count").display().to_string();
        let w = runtime_of(&done_then_cut(&count), Some(1_500.0));
        let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        let first = w.fx.rt.sessions().start(&ws, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.expect("a start");
        assert_eq!(j(&first.finished().await)["costUsd"], 0.072016);
        let second =
            w.fx.rt.sessions().start(&ws, StartOptions { prompt: "go again".into(), ..Default::default() }, None).await.expect("a start");
        assert_eq!(j(&second.finished().await)["status"], "failed");
        let rows = rows_of(&w).await;
        assert_eq!(
            pairs(&rows, |r| vec![r["key"].clone(), r["tokens"]["input"].clone(), r["turns"].clone(), r["costReported"].clone()]),
            [vec![json!("claude:claude-opus-5-5"), json!(30_004 + 80_509), json!(2), json!(0.072016)]]
        );
        near(num(&rows[0]["costList"]), 9.0 * 4e-6 + 40_500.0 * 5e-6 + 40_000.0 * 2e-7 + 102.0 * 2e-5);
        near(num(&rows[0]["costReported"]) + num(&rows[0]["costList"]), num(&rows[0]["estimate"]));
        w.fx.rt.close().await.expect("a close");
        drop(w.root);
    });
}

/// Two sends on one thread; the second's reply only comes from the process the first left up.
async fn two_on_one_process(w: &World) -> Value {
    let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
    let one = w.fx.rt.sessions().start(&ws, StartOptions { prompt: "one".into(), ..Default::default() }, None).await.expect("a start");
    one.finished().await;
    let opts = StartOptions { prompt: "two".into(), thread: one.view().thread_id, ..Default::default() };
    let two = j(&w.fx.rt.sessions().start(&ws, opts, None).await.expect("a start").finished().await);
    assert_eq!(two["text"], "two");
    two
}

async fn ledger_total(w: &World) -> (f64, f64, f64) {
    let rows = rows_of(w).await;
    let input = rows.iter().map(|r| num(&r["tokens"]["input"])).sum();
    let cost = rows.iter().map(|r| r["costReported"].as_f64().unwrap_or(0.0) + r["costList"].as_f64().unwrap_or(0.0)).sum();
    let turns = rows.iter().map(|r| r["turns"].as_f64().unwrap_or(0.0)).sum();
    (input, cost, turns)
}

// "what a Claude turn files in the ledger" > "two turns on a kept process, the first a held reply with calls after it, file the session's tokens and dollars once"
#[test]
fn two_turns_on_a_kept_process_the_first_a_held_reply_with_calls_after_it_file_the_sessions_tokens_and_dollars_once() {
    drive(|| async {
        let w = runtime_of(&kept_twice(), None);
        two_on_one_process(&w).await;
        // The CLI reports every figure at its list price, so a ledger that counts each token and dollar once equals it.
        let (input, cost, turns) = ledger_total(&w).await;
        assert_eq!(input, 560_518.0);
        near(cost, kept_list());
        assert_eq!(turns, 2.0);
        w.fx.rt.close().await.expect("a close");
        drop(w.root);
    });
}

// "what a Claude turn files in the ledger" > "the second of those turns, its one model withheld, reports no cost of its own rather than $0"
#[test]
fn the_second_of_those_turns_its_one_model_withheld_reports_no_cost_of_its_own_rather_than_zero() {
    drive(|| async {
        let w = runtime_of(&kept_twice(), None);
        let two = two_on_one_process(&w).await;
        assert!(two.get("costUsd").is_none());
        let models: Vec<(Value, Value, Value)> = two["models"]
            .as_array()
            .expect("models")
            .iter()
            .map(|m| (m["model"].clone(), m["tokens"]["input"].clone(), m.get("costUsd").cloned().unwrap_or(Value::Null)))
            .collect();
        assert_eq!(models, [(json!("claude-opus-5-5"), json!(30_510), Value::Null)]);
        w.fx.rt.close().await.expect("a close");
        drop(w.root);
    });
}

// "what a Claude turn files in the ledger" > "a model first seen after the held reply, keyed with its context window in the next result, is filed once"
#[test]
fn a_model_first_seen_after_the_held_reply_keyed_with_its_context_window_in_the_next_result_is_filed_once() {
    drive(|| async {
        let w = runtime_of(&kept_new_model(), None);
        two_on_one_process(&w).await;
        let session =
            (OPUS_AT_TWO.input + OPUS_AT_TWO.read + OPUS_AT_TWO.write + SONNET_AFTER.input + SONNET_AFTER.read + SONNET_AFTER.write) as f64;
        let (input, cost, turns) = ledger_total(&w).await;
        assert_eq!(input, session);
        near(cost, new_model_list());
        assert_eq!(turns, 2.0);
        w.fx.rt.close().await.expect("a close");
        drop(w.root);
    });
}

// "what a Claude turn files in the ledger" > "a turn cut at the wall before any result files a usage row of the calls it printed"
#[test]
fn a_turn_cut_at_the_wall_before_any_result_files_a_usage_row_of_the_calls_it_printed() {
    drive(|| async {
        let w = runtime_of(&cut(), Some(1_500.0));
        let (result, _) = turn(&w).await;
        assert_eq!(result["status"], "failed");
        assert!(result["error"].as_str().unwrap_or_default().contains("turn limit"), "{result}");
        assert_matches(&result["tokens"], json!({ "input": 80_509, "output": 102, "cached": 40_000, "cacheWrite": 40_500 }));
        let rows = rows_of(&w).await;
        assert_eq!(
            pairs(&rows, |r| vec![r["key"].clone(), r["tokens"]["input"].clone(), r["tokens"]["output"].clone(), r["turns"].clone()]),
            [vec![json!("claude:claude-opus-5-5"), json!(80_509), json!(102), json!(1)]]
        );
        w.fx.rt.close().await.expect("a close");
        drop(w.root);
    });
}
