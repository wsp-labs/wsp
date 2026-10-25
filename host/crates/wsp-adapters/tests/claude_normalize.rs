// SPDX-License-Identifier: AGPL-3.0-only
//! The hard cases of unit claude-normalize (spec/claude_normalize.md). The TS functions are module-private, so each
//! expected value here is what a copy of adapter.ts:1-625 at a3b870a6e, with those functions exported, printed.

use serde_json::{json, Value};
use wsp_adapters::claude::fields::Obj;
use wsp_adapters::claude::handback::HandbackBook;
use wsp_adapters::claude::normalize::*;
use wsp_adapters::claude::plans::{PlanBook, PlanTask};
use wsp_adapters::pending::{anthropic_retry_why, fmt_duration, Refusal};
use wsp_adapters::port::{AdapterEvent, DeltaKind, HarnessLimit, PlanStep, SessionHarness, TurnDelta, TurnResult, TurnStatus, TurnTokens};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::UsageTokens;
use wsp_wire::views::session::TurnRefusal;
use wsp_wire::views::session_events::PlanStepState;

fn obj(v: Value) -> Obj {
    v.as_object().cloned().expect("an object")
}

fn n(x: f64) -> JsNumber {
    JsNumber(x)
}

fn result(status: TurnStatus) -> TurnResult {
    TurnResult { status, duration_ms: None, waited_ms: None, cost_usd: None, tokens: None, model: None, models: None, text: None, error: None, unreached: None, refusal: None, limit: None }
}

fn done() -> TurnResult {
    result(TurnStatus::Completed)
}

fn mu(model: &str, input: u64, output: u64, cost: Option<f64>) -> ModelUse {
    ModelUse { model: model.into(), tokens: UsageTokens { input, output, cached: 0, cache_write: 0, reasoning: 0 }, cost_usd: cost.map(JsNumber) }
}

fn tok(input: f64, output: f64) -> TurnTokens {
    TurnTokens { input: n(input), output: n(output), cached: None, cache_write: None, reasoning: None, context: None, window: None }
}

fn full(input: f64, output: f64) -> TurnTokens {
    TurnTokens { cached: Some(n(0.0)), cache_write: Some(n(0.0)), reasoning: Some(n(0.0)), ..tok(input, output) }
}

#[test]
fn result_status_reads_the_abort_first() {
    let st = |v: Value, errors: &str| result_status(&obj(v), errors);
    assert_eq!(st(json!({"type": "result", "subtype": "success", "is_error": true, "terminal_reason": "aborted_tools"}), ""), TurnStatus::Interrupted);
    assert_eq!(st(json!({"type": "result", "subtype": "success", "is_error": true}), ""), TurnStatus::Failed);
    assert_eq!(st(json!({"type": "result", "subtype": "success", "is_error": "yes"}), ""), TurnStatus::Completed, "only the boolean true is the flag");
    assert_eq!(st(json!({"type": "result", "subtype": "error_during_execution", "is_error": false}), "the request was aborted"), TurnStatus::Interrupted);
    assert_eq!(st(json!({"type": "result", "subtype": "error_during_execution", "is_error": true}), "request was aborted"), TurnStatus::Failed);
    assert_eq!(st(json!({"type": "result", "subtype": "error_max_turns"}), "cancelled by user"), TurnStatus::Interrupted);
}

#[test]
fn own_use_is_the_gain_over_the_saved_totals() {
    let turn = TurnResult {
        duration_ms: Some(n(5.0)),
        cost_usd: Some(n(0.5)),
        tokens: Some(TurnTokens { window: Some(n(200000.0)), ..tok(100.0, 10.0) }),
        models: Some(vec![mu("claude-opus-5-5[1m]", 900, 90, Some(0.4)), mu("claude-haiku-5-5", 10, 1, Some(0.1))]),
        text: Some("hi".into()),
        ..done()
    };
    let saved = SavedUse { cost_usd: 0.3, models: vec![mu("claude-opus-5-5", 500, 50, Some(0.25))], unpriced: None };
    assert_eq!(
        own_use(turn, Some(&saved), false, None),
        TurnResult {
            duration_ms: Some(n(5.0)),
            tokens: Some(TurnTokens { window: Some(n(200000.0)), ..full(410.0, 41.0) }),
            text: Some("hi".into()),
            cost_usd: Some(n(0.2)),
            models: Some(vec![mu("claude-opus-5-5[1m]", 400, 40, Some(0.15000000000000002)), mu("claude-haiku-5-5", 10, 1, Some(0.1))]),
            model: Some("claude-opus-5-5[1m]".into()),
            ..done()
        },
        "the windowed name finds its bare saved totals; the turn's tokens are its split by model"
    );
}

#[test]
fn own_use_takes_a_total_under_the_saved_one_whole() {
    let turn = TurnResult { cost_usd: Some(n(0.1)), models: Some(vec![mu("m", 10, 1, Some(0.1))]), ..done() };
    let saved = SavedUse { cost_usd: 0.3, models: vec![mu("m", 500, 50, Some(0.3))], unpriced: Some(vec!["m".into()]) };
    assert_eq!(own_use(turn, Some(&saved), false, None), TurnResult { tokens: Some(full(10.0, 1.0)), cost_usd: Some(n(0.1)), models: Some(vec![mu("m", 10, 1, Some(0.1))]), model: Some("m".into()), ..done() }, "a restart forgets what was unpriced");
}

#[test]
fn own_use_leaves_cost_out_with_nothing_saved() {
    let turn = TurnResult { cost_usd: Some(n(0.1)), tokens: Some(tok(1.0, 2.0)), models: Some(vec![mu("m", 10, 1, Some(0.1))]), ..done() };
    assert_eq!(own_use(turn, None, false, Some(&tok(7.0, 8.0))), TurnResult { tokens: Some(tok(7.0, 8.0)), ..done() });
}

#[test]
fn own_use_drops_the_split_where_a_model_fell() {
    let turn = TurnResult { cost_usd: Some(n(0.6)), models: Some(vec![mu("m", 400, 60, Some(0.6))]), ..done() };
    let saved = SavedUse { cost_usd: 0.3, models: vec![mu("m", 500, 50, Some(0.3))], unpriced: None };
    assert_eq!(own_use(turn, Some(&saved), false, None), TurnResult { cost_usd: Some(n(0.3)), ..done() }, "never a model's whole total");
}

#[test]
fn own_use_withholds_unpriced_models() {
    let turn = TurnResult { cost_usd: Some(n(0.9)), models: Some(vec![mu("a", 700, 70, Some(0.5)), mu("b", 300, 30, Some(0.4))]), ..done() };
    let saved = SavedUse { cost_usd: 0.3, models: vec![mu("a", 500, 50, Some(0.2)), mu("b", 100, 10, Some(0.1))], unpriced: Some(vec!["b[1m]".into()]) };
    assert_eq!(
        own_use(turn, Some(&saved), false, None),
        TurnResult { tokens: Some(full(400.0, 40.0)), cost_usd: Some(n(0.30000000000000004)), models: Some(vec![mu("a", 200, 20, Some(0.3)), mu("b", 200, 20, None)]), model: Some("a".into()), ..done() }
    );
}

#[test]
fn own_use_drops_models_that_spent_nothing() {
    let turn = TurnResult { cost_usd: Some(n(0.2)), models: Some(vec![mu("a", 0, 0, Some(0.0)), mu("b", 3, 4, Some(0.2))]), ..done() };
    assert_eq!(own_use(turn, None, true, None), TurnResult { tokens: Some(full(3.0, 4.0)), cost_usd: Some(n(0.2)), models: Some(vec![mu("b", 3, 4, Some(0.2))]), model: Some("b".into()), ..done() });
}

#[test]
fn gained_floors_the_cost_at_zero() {
    assert_eq!(gained(&mu("m", 10, 1, Some(0.1)), Some(&mu("m", 5, 1, Some(0.3)))), Some(mu("m", 5, 0, Some(0.0))));
    assert_eq!(gained(&mu("m", 10, 1, None), Some(&mu("m", 5, 1, Some(0.3)))), Some(mu("m", 5, 0, None)));
    assert_eq!(gained(&mu("m", 4, 1, Some(0.1)), Some(&mu("m", 5, 1, None))), None);
    assert_eq!(gained(&mu("m", 4, 1, Some(0.1)), None), Some(mu("m", 4, 1, Some(0.1))));
}

#[test]
fn a_refused_result_carries_the_sentence_and_the_road() {
    let line = obj(json!({"type": "result", "subtype": "success", "is_error": true, "result": "Not logged in · Please run /login", "duration_ms": 12, "total_cost_usd": 0, "usage": {"input_tokens": 0, "output_tokens": 0}, "errors": ["[ede_diagnostic] x"]}));
    let refusal = Refusal { road: Some("sign in on the Computers page, then send again".into()), cause: Some(TurnRefusal::SignIn) };
    assert_eq!(
        normalize_result(&line, Some(&refusal)),
        TurnResult { duration_ms: Some(n(12.0)), cost_usd: Some(n(0.0)), tokens: Some(tok(0.0, 0.0)), error: Some("Not logged in · Please run /login; sign in on the Computers page, then send again".into()), refusal: Some(TurnRefusal::SignIn), ..result(TurnStatus::Failed) }
    );
}

#[test]
fn a_failed_result_reads_its_models_and_largest_window() {
    let line = obj(json!({"type": "result", "subtype": "error_during_execution", "is_error": false, "errors": ["[ede_diagnostic] a", "Real error"], "modelUsage": {"m[1m]": {"inputTokens": 1, "outputTokens": 2, "cacheReadInputTokens": 3, "cacheCreationInputTokens": 4, "costUSD": 0.5, "contextWindow": 1000000}, "x": 5, "n": {"contextWindow": 200000}}, "usage": {"input_tokens": 9, "output_tokens": 1}}));
    assert_eq!(
        normalize_result(&line, None),
        TurnResult {
            tokens: Some(TurnTokens { window: Some(n(1000000.0)), ..tok(9.0, 1.0) }),
            models: Some(vec![ModelUse { model: "m[1m]".into(), tokens: UsageTokens { input: 8, output: 2, cached: 3, cache_write: 4, reasoning: 0 }, cost_usd: Some(n(0.5)) }, mu("n", 0, 0, None)]),
            error: Some("Real error".into()),
            ..result(TurnStatus::Failed)
        }
    );
    assert!(answered_nothing(&TurnResult { text: Some("  ".into()), tokens: Some(tok(0.0, 0.0)), ..done() }));
    assert!(answered_nothing(&done()), "no tokens at all counts as none");
    assert!(!answered_nothing(&TurnResult { tokens: Some(tok(0.0, 1.0)), ..done() }));
    assert_eq!(with_context(TurnResult { tokens: Some(tok(1.0, 2.0)), ..done() }, Some(50.0), Some("init-model")), TurnResult { tokens: Some(TurnTokens { context: Some(n(50.0)), ..tok(1.0, 2.0) }), model: Some("init-model".into()), ..done() });
    assert_eq!(with_context(TurnResult { model: Some("own".into()), ..done() }, Some(50.0), Some("init-model")), TurnResult { model: Some("own".into()), ..done() });
    assert_eq!(no_output_error(&TurnResult { duration_ms: Some(n(61000.0)), ..done() }, &["a".into(), "b".into()]), format!("claude answered with no output and no usage after {}: a\nb", fmt_duration(61000.0)));
    assert_eq!(no_output_error(&done(), &[]), format!("claude answered with no output and no usage after {}", fmt_duration(0.0)));
}

#[test]
fn an_init_says_the_session_and_a_key_has_no_plan() {
    let (mut plans, mut book) = (PlanBook::default(), HandbackBook::default());
    let init = obj(json!({"type": "system", "subtype": "init", "session_id": "S2", "model": "m", "cwd": "/w", "tools": ["Bash"], "apiKeySource": "ANTHROPIC_API_KEY", "permissionMode": "default"}));
    assert_eq!(
        normalize_event(&init, "S1", None, &mut plans, &mut book),
        vec![
            AdapterEvent::SessionStart { session_id: "S2".into(), model: Some("m".into()), cwd: Some("/w".into()), tools: Some(vec!["Bash".into()]), harness: Some(SessionHarness { slash_commands: None, permission_mode: Some("default".into()), agents: None }), prompted: None },
            AdapterEvent::Limit { session_id: "S2".into(), limit: HarnessLimit { windows: vec![], plan: None, status: None, account: None, keyed: Some(true), credits: None } },
        ]
    );
    assert_eq!(normalize_event(&obj(json!({"type": "system", "subtype": "init", "apiKeySource": "none"})), "S1", None, &mut plans, &mut book), vec![AdapterEvent::SessionStart { session_id: "S1".into(), model: None, cwd: None, tools: None, harness: None, prompted: None }]);
}

#[test]
fn assistant_blocks_become_deltas_and_plans() {
    let (mut plans, mut book) = (PlanBook::default(), HandbackBook::default());
    let line = obj(json!({"type": "assistant", "parent_tool_use_id": "p1", "message": {"id": "msg1", "content": [{"type": "text", "text": "a"}, {"type": "thinking", "thinking": "t"}, {"type": "tool_use", "id": "t1", "name": "TodoWrite", "input": {"todos": []}}, {"type": "tool_use", "id": "t2", "name": "Read"}, 5]}}));
    let d = |kind, text: &str| TurnDelta { parent_tool_use_id: Some("p1".into()), ..TurnDelta::new("S1", kind, text) };
    assert_eq!(
        normalize_event(&line, "S1", None, &mut plans, &mut book),
        vec![
            AdapterEvent::TurnDelta(TurnDelta { message_id: Some("msg1".into()), ..d(DeltaKind::Text, "a") }),
            AdapterEvent::TurnDelta(d(DeltaKind::Thinking, "t")),
            AdapterEvent::TurnDelta(TurnDelta { tool_name: Some("TodoWrite".into()), tool_use_id: Some("t1".into()), ..d(DeltaKind::ToolUse, r#"{"todos":[]}"#) }),
            AdapterEvent::TurnDelta(TurnDelta { tool_name: Some("Read".into()), tool_use_id: Some("t2".into()), ..d(DeltaKind::ToolUse, "null") }),
        ],
        "a subagent's todo list is a tool row, not the turn's plan; a call with no input reads null"
    );
    assert!(plans.calls.is_empty());
    let own = obj(json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t3", "name": "TaskCreate", "input": {"subject": "s"}}]}}));
    assert_eq!(normalize_event(&own, "S1", None, &mut plans, &mut book), vec![AdapterEvent::TurnPlan { session_id: "S1".into(), steps: Some(vec![PlanStep { text: "s".into(), state: PlanStepState::Pending }]), text: None }]);
    assert!(plans.calls.contains("t3"));
    assert_eq!(normalize_event(&obj(json!({"type": "assistant", "message": {"content": "text"}})), "S1", None, &mut plans, &mut book), vec![]);
}

#[test]
fn a_task_create_result_names_the_task_and_draws_nothing() {
    let (mut plans, mut book) = (PlanBook::default(), HandbackBook::default());
    normalize_event(&obj(json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t3", "name": "TaskCreate", "input": {"subject": "s"}}]}})), "S1", None, &mut plans, &mut book);
    let answers = obj(json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "t3", "content": "Task #7 created"}, {"type": "tool_result", "tool_use_id": "t2", "content": [{"type": "text", "text": "x"}], "is_error": true}, {"type": "text", "text": "no"}]}}));
    assert_eq!(
        normalize_event(&answers, "S1", None, &mut plans, &mut book),
        vec![AdapterEvent::TurnDelta(TurnDelta { tool_use_id: Some("t2".into()), is_error: Some(true), ..TurnDelta::new("S1", DeltaKind::ToolResult, "x") })]
    );
    assert_eq!(plans, PlanBook { calls: Default::default(), tasks: vec![PlanTask { id: Some("7".into()), text: "s".into(), state: PlanStepState::Pending }], unnamed: Default::default() });
}

#[test]
fn retries_tasks_and_drained_notices() {
    assert_eq!(
        retry_of(&obj(json!({"type": "system", "subtype": "api_retry", "attempt": 2, "max_retries": 10, "error": "overloaded", "error_status": 529})), "s"),
        Some(AdapterEvent::TurnRetry { session_id: "s".into(), attempt: 2.0, of: Some(10.0), why: Some(anthropic_retry_why(Some("overloaded"), Some(529.0))) })
    );
    assert_eq!(retry_of(&obj(json!({"type": "system", "subtype": "api_retry", "attempt": 1, "error": "unknown"})), "s"), Some(AdapterEvent::TurnRetry { session_id: "s".into(), attempt: 1.0, of: None, why: Some(anthropic_retry_why(Some("unknown"), None)) }));
    assert_eq!(retry_of(&obj(json!({"type": "system", "subtype": "api_retry"})), "s"), None);
    let (mut plans, mut book) = (PlanBook::default(), HandbackBook::default());
    assert_eq!(
        normalize_event(&obj(json!({"type": "tool_progress", "subagent_retry": {"attempt": 1, "error_category": "server_error", "error_status": 500}})), "S1", None, &mut plans, &mut book),
        vec![AdapterEvent::TurnRetry { session_id: "S1".into(), attempt: 1.0, of: None, why: Some(anthropic_retry_why(Some("server_error"), Some(500.0))) }]
    );
    assert_eq!(normalize_event(&obj(json!({"type": "rate_limit_event", "rate_limit_info": {"status": "allowed"}})), "S1", None, &mut plans, &mut book), vec![]);
    assert_eq!(normalize_event(&obj(json!({"type": "stream_event"})), "S1", None, &mut plans, &mut book), vec![]);
    assert_eq!(subagent_prompts(&obj(json!({"type": "assistant", "parent_tool_use_id": null, "message": {"content": [{"type": "tool_use", "id": "c1", "name": "Agent", "input": {"prompt": "do x"}}, {"type": "tool_use", "id": "c2", "name": "Bash", "input": {"prompt": "no"}}, {"type": "tool_use", "id": "c3", "name": "Task", "input": {}}]}}))), vec![("c1".to_string(), "do x".to_string())]);
    assert_eq!(background_tasks_of(&obj(json!({"type": "system", "subtype": "background_tasks_changed", "tasks": "nope"}))), Some(vec![]));
    assert_eq!(
        background_tasks_of(&obj(json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [{"task_id": "b1", "description": "npm test"}, {"task_id": "b2"}, {"description": "no id"}]}))),
        Some(vec![BackgroundTask { id: "b1".into(), description: "npm test".into() }, BackgroundTask { id: "b2".into(), description: "b2".into() }])
    );
    let notice = |text: &str| obj(json!({"type": "result", "subtype": "success", "origin": {"kind": "task-notification"}, "num_turns": 0, "result": text, "usage": {"input_tokens": 0, "output_tokens": 0}}));
    assert!(drained_notice(&notice("")));
    assert!(!drained_notice(&notice("the server stopped")), "a report's answer with words is a reply");
    assert_eq!(compacted_to(&obj(json!({"type": "system", "subtype": "compact_boundary", "compact_metadata": {"pre_tokens": 1000}}))), None, "no after, no compaction figure");
    assert_eq!(compacted_to(&obj(json!({"type": "system", "subtype": "compact_boundary", "compact_metadata": {"post_tokens": 40}}))), Some(Compacted { before: None, after: 40.0 }));
}

// Rule N1: a result answered nothing where its text is blank by JS trim, which strips U+FEFF and leaves U+0085,
// U+200B, U+180E and U+001C.
#[test]
fn answered_nothing_trims_as_javascript_does() {
    let blank = |t: &str| answered_nothing(&TurnResult { text: Some(t.into()), ..done() });
    let got = ["\u{85}", "\u{FEFF}", "\u{A0}", "\u{2028}", "\u{3000}", "\u{B}", "\u{200B}", "\u{180E}", "\u{1C}"].map(blank);
    assert_eq!(got, [false, true, true, true, true, true, false, false, false]);
}

// Rule N2: modelUsage present as null is a turn that named no model (`[]`); usage `{}` is zero tokens with no cache
// counts; usage that is no object is nothing.
#[test]
fn result_tokens_read_null_model_usage_as_no_model() {
    assert_eq!(result_tokens(&obj(json!({"modelUsage": null, "usage": {"input_tokens": 1, "output_tokens": 2}}))), ResultTokens { tokens: Some(tok(1.0, 2.0)), models: Some(vec![]) });
    assert_eq!(result_tokens(&obj(json!({"usage": {}}))), ResultTokens { tokens: Some(tok(0.0, 0.0)), models: None });
    assert_eq!(result_tokens(&obj(json!({"usage": [1]}))), ResultTokens::default());
}

// Rule N3: a call joining an entry of its base model adds tokens only, its cost dropped; an exact name beats a base
// match wherever it sits; "" finds nothing.
#[test]
fn a_joined_call_adds_tokens_only_and_an_exact_name_beats_its_base() {
    let joined = with_calls(&[mu("m[1m]", 10, 1, Some(0.5))], &[mu("m", 2, 3, Some(0.7)), mu("n", 1, 1, Some(0.2)), mu("n[1m]", 4, 4, Some(0.3))]);
    assert_eq!(joined, vec![mu("m[1m]", 12, 4, Some(0.5)), mu("n", 5, 5, Some(0.2))]);
    let two = [mu("m[1m]", 1, 1, None), mu("m", 2, 2, None)];
    assert_eq!(start_of(&two, "m"), Some(&two[1]));
    let windows = [mu("m[1m]", 1, 1, None), mu("m[200k]", 2, 2, None)];
    assert_eq!(start_of(&windows, "m"), Some(&windows[0]));
    assert_eq!(start_of(&[mu("m", 1, 1, None)], ""), None);
}

// Rule N4: a resume whose result names no model's totals costs the gain over the saved total and names no models.
#[test]
fn own_use_with_no_running_models_costs_the_gain_alone() {
    let saved = SavedUse { cost_usd: 0.1, models: vec![mu("a", 1, 1, Some(0.1))], unpriced: None };
    let tokens = TurnTokens { cached: Some(n(3.0)), ..tok(1.0, 2.0) };
    let result = TurnResult { tokens: Some(tokens.clone()), cost_usd: Some(n(0.2)), ..done() };
    assert_eq!(own_use(result, Some(&saved), false, None), TurnResult { tokens: Some(tokens), cost_usd: Some(n(0.1)), ..done() });
}

// Rule N5: a tool_use block with no name and no id still draws, its input text "null", no name or id keys.
#[test]
fn a_bare_tool_use_block_draws_null() {
    let event = obj(json!({"type": "assistant", "session_id": "S", "message": {"content": [{"type": "tool_use"}]}}));
    let drawn = normalize_event(&event, "FB", None, &mut PlanBook::default(), &mut HandbackBook::default());
    assert_eq!(drawn, vec![AdapterEvent::TurnDelta(TurnDelta::new("S", DeltaKind::ToolUse, "null"))]);
}

// Rule N6: a tool result with no is_error says isError false, always present.
#[test]
fn a_tool_result_always_says_whether_it_failed() {
    let event = obj(json!({"type": "user", "session_id": "S", "message": {"content": [{"type": "tool_result", "tool_use_id": "t1", "content": "ok"}]}}));
    let drawn = normalize_event(&event, "FB", None, &mut PlanBook::default(), &mut HandbackBook::default());
    assert_eq!(drawn, vec![AdapterEvent::TurnDelta(TurnDelta { tool_use_id: Some("t1".into()), is_error: Some(false), ..TurnDelta::new("S", DeltaKind::ToolResult, "ok") })]);
}

// Rule N7: a retry with attempt 0 and nothing else is still a retry, with the default words.
#[test]
fn a_first_retry_with_no_words_says_anthropic_did_not_answer() {
    assert_eq!(retry_of(&obj(json!({"type": "system", "subtype": "api_retry", "attempt": 0})), "s"), Some(AdapterEvent::TurnRetry { session_id: "s".into(), attempt: 0.0, of: None, why: Some("Anthropic did not answer".into()) }));
}

// Rule N8: the empty-answer line times the result through fmt_duration, a negative duration 0ms, none 1ms, and keeps
// the tail's blank entries.
#[test]
fn the_empty_answer_line_times_any_duration() {
    let line = |d: Option<f64>| no_output_error(&TurnResult { duration_ms: d.map(n), ..done() }, &["a".to_string(), "".to_string(), "b".to_string()]);
    let got: Vec<String> = [Some(1500.0), Some(999.6), Some(59999.0), Some(61000.0), Some(-5.0), None].into_iter().map(line).collect();
    let said = |t: &str| format!("claude answered with no output and no usage after {t}: a\n\nb");
    assert_eq!(got, ["1.5s", "1000ms", "60s", "1m 1s", "0ms", "1ms"].map(said).to_vec());
}
