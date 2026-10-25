// SPDX-License-Identifier: AGPL-3.0-only
//! The hard cases of unit claude-reading (spec/claude_reading.md): null and empty inputs, the caps, the orders. The TS
//! suite reaches these only through whole turns; each case here was checked against the TS functions at a3b870a6e.

use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::claude::fields::{harness_of, num, parse_line, rec, str_arr, string, Obj};
use wsp_adapters::claude::limits::{limit_of, note_rejected, with_limit, Rejected};
use wsp_adapters::claude::plans::{read_plan_call, PlanBook, PlanCall};
use wsp_adapters::claude::subagent_spend::{note_child_call, note_progress, subagent_spent, subagent_spent_at_end, SpendBook};
use wsp_adapters::claude::tokens::{plus_tokens, usage_of, usage_tokens};
use wsp_adapters::port::{PlanStep, SessionHarness, SubagentUsage, TurnResult, TurnStatus, TurnTokens};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::{HarnessLimit, LimitKind, LimitStatus, LimitWindow, TurnLimit, UsageTokens};
use wsp_wire::views::session_events::PlanStepState;

fn obj(v: Value) -> Obj {
    v.as_object().cloned().expect("an object")
}

fn tokens(input: f64, output: f64) -> TurnTokens {
    TurnTokens { input: JsNumber(input), output: JsNumber(output), cached: None, cache_write: None, reasoning: None, context: None, window: None }
}

fn failed() -> TurnResult {
    TurnResult { status: TurnStatus::Failed, duration_ms: None, waited_ms: None, cost_usd: None, tokens: None, model: None, models: None, text: None, error: Some("You've hit your limit".into()), unreached: None, refusal: None, limit: None }
}

#[test]
fn parse_line_reads_only_objects_with_a_string_type() {
    assert_eq!(parse_line("  {\"type\":\"result\",\"a\":1}  \n"), Some(obj(json!({"type": "result", "a": 1}))));
    assert_eq!(parse_line("{\"type\":1}"), None);
    assert_eq!(parse_line("{\"subtype\":\"init\"}"), None);
    assert_eq!(parse_line("[{\"type\":\"x\"}]"), None);
    assert_eq!(parse_line("warn: {\"type\":\"x\"}"), None);
    assert_eq!(parse_line("{\"type\":\"x\""), None);
    assert_eq!(parse_line(""), None);
    // Key order is the line's own.
    let line = parse_line("{\"z\":1,\"type\":\"t\",\"a\":2}").unwrap();
    assert_eq!(line.keys().cloned().collect::<Vec<_>>(), vec!["z", "type", "a"]);
}

#[test]
fn str_arr_keeps_the_strings_and_an_empty_array() {
    assert_eq!(str_arr(Some(&json!(["a", 1, null, "b", {"c": 1}]))), Some(vec!["a".to_string(), "b".to_string()]));
    assert_eq!(str_arr(Some(&json!([]))), Some(vec![]));
    assert_eq!(str_arr(Some(&json!("a"))), None);
    assert_eq!(str_arr(None), None);
    assert_eq!(string(Some(&json!(""))), Some(""));
    assert_eq!(string(Some(&json!(1))), None);
    assert_eq!(num(Some(&json!(0.5))), Some(0.5));
    assert_eq!(num(Some(&json!("1"))), None);
    assert!(rec(Some(&json!([]))).is_none());
    assert!(rec(Some(&Value::Null)).is_none());
    assert!(rec(Some(&json!({}))).is_some());
}

#[test]
fn harness_of_reads_each_field_alone() {
    assert_eq!(harness_of(&obj(json!({"type": "system", "subtype": "init"}))), None);
    assert_eq!(harness_of(&obj(json!({"slash_commands": []}))), Some(SessionHarness { slash_commands: Some(vec![]), permission_mode: None, agents: None }));
    assert_eq!(harness_of(&obj(json!({"permissionMode": "default", "agents": ["x", 2]}))), Some(SessionHarness { slash_commands: None, permission_mode: Some("default".into()), agents: Some(vec!["x".into()]) }));
}

#[test]
fn usage_tokens_names_only_the_counts_the_usage_names() {
    let plain = usage_tokens(&obj(json!({"input_tokens": 10, "output_tokens": 5})));
    assert_eq!(plain, tokens(10.0, 5.0));
    let cached = usage_tokens(&obj(json!({"input_tokens": 10, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 7, "output_tokens": 5})));
    assert_eq!(cached, TurnTokens { cached: Some(JsNumber(0.0)), cache_write: Some(JsNumber(7.0)), ..tokens(17.0, 5.0) });
    assert_eq!(usage_tokens(&obj(json!({}))), tokens(0.0, 0.0));
    assert_eq!(usage_of(&tokens(3.0, 4.0)), UsageTokens { input: 3, output: 4, cached: 0, cache_write: 0, reasoning: 0 });
}

#[test]
fn plus_tokens_drops_context_and_window_on_a_sum() {
    let b = TurnTokens { window: Some(JsNumber(200000.0)), context: Some(JsNumber(9.0)), ..tokens(1.0, 2.0) };
    assert_eq!(plus_tokens(None, &b), b, "nothing to add to: b stands whole");
    let a = TurnTokens { reasoning: Some(JsNumber(3.0)), ..tokens(10.0, 20.0) };
    let sum = plus_tokens(Some(&a), &TurnTokens { cached: Some(JsNumber(4.0)), ..b.clone() });
    assert_eq!(sum, TurnTokens { input: JsNumber(11.0), output: JsNumber(22.0), cached: Some(JsNumber(4.0)), cache_write: None, reasoning: Some(JsNumber(3.0)), context: None, window: None });
}

#[test]
fn limit_of_takes_unified_windows_first_and_one_window_per_kind() {
    let info = obj(json!({
        "status": "allowed_warning",
        "rateLimitType": "seven_day",
        "utilization": 0.9,
        "resetsAt": 1700000000,
        "unifiedWindows": {
            "five_hour": {"utilization": 0.12345, "resetsAt": 1690000000},
            "seven_day_overage_included": {"utilization": 1.5},
            "overage": {"utilization": 0.1},
            "seven_day": {"utilization": 0.2},
            "monthly": {"utilization": 0.3},
            "seven_day_opus": {"resetsAt": 1}
        }
    }));
    let limit = limit_of(Some(&info)).expect("a reading");
    assert_eq!(
        limit,
        HarnessLimit {
            windows: vec![
                LimitWindow { kind: LimitKind::Session, used_percent: JsNumber(12.3), resets_at: Some(JsNumber(1690000000000.0)) },
                LimitWindow { kind: LimitKind::Overage, used_percent: JsNumber(150.0), resets_at: None },
                LimitWindow { kind: LimitKind::Week, used_percent: JsNumber(20.0), resets_at: None },
            ],
            plan: None,
            status: Some(LimitStatus::Warning),
            account: None,
            keyed: None,
            credits: None,
        },
        "the top-level seven_day reading loses to the unified one of its kind; overage after its twin is dropped"
    );
}

#[test]
fn limit_of_reads_nothing_from_no_window() {
    assert_eq!(limit_of(None), None);
    assert_eq!(limit_of(Some(&obj(json!({"status": "allowed"})))), None);
    assert_eq!(limit_of(Some(&obj(json!({"status": "rejected", "rateLimitType": "five_hour"})))), None, "no utilization, no window");
    let odd_status = limit_of(Some(&obj(json!({"status": "surprise", "rateLimitType": "five_hour", "utilization": 0})))).unwrap();
    assert_eq!(odd_status.status, None);
    assert_eq!(odd_status.windows, vec![LimitWindow { kind: LimitKind::Session, used_percent: JsNumber(0.0), resets_at: None }]);
}

#[test]
fn note_rejected_moves_the_stop_by_each_reading() {
    let mut rejected: Rejected = IndexMap::new();
    note_rejected(None, &mut rejected);
    assert!(rejected.is_empty());
    note_rejected(Some(&obj(json!({"status": "rejected", "rateLimitType": "five_hour", "resetsAt": 100}))), &mut rejected);
    note_rejected(Some(&obj(json!({"status": "rejected", "rateLimitType": "seven_day"}))), &mut rejected);
    note_rejected(Some(&obj(json!({"status": "rejected", "rateLimitType": "five_hour", "resetsAt": 200}))), &mut rejected);
    assert_eq!(rejected.clone().into_iter().collect::<Vec<_>>(), vec![("five_hour".to_string(), Some(200000.0)), ("seven_day".to_string(), None)]);
    note_rejected(Some(&obj(json!({"status": "rejected", "rateLimitType": "seven_day", "overageStatus": "allowed_warning"}))), &mut rejected);
    note_rejected(Some(&obj(json!({"status": "allowed", "rateLimitType": "five_hour"}))), &mut rejected);
    assert!(rejected.is_empty(), "overage and a later allowed reading take each stop back");
    note_rejected(Some(&obj(json!({"status": "rejected"}))), &mut rejected);
    assert_eq!(rejected.get(""), Some(&None), "a rejected reading with no type is kept under the empty key");
}

#[test]
fn with_limit_stamps_only_a_failed_turn() {
    let mut rejected: Rejected = IndexMap::new();
    assert_eq!(with_limit(failed(), &rejected), failed());
    rejected.insert("five_hour".into(), Some(100.0));
    rejected.insert("seven_day".into(), Some(300.0));
    assert_eq!(with_limit(failed(), &rejected), TurnResult { limit: Some(TurnLimit { resets_at: Some(JsNumber(300.0)) }), ..failed() });
    let done = TurnResult { status: TurnStatus::Completed, ..failed() };
    assert_eq!(with_limit(done.clone(), &rejected), done);
    rejected.insert("overage".into(), None);
    assert_eq!(with_limit(failed(), &rejected), TurnResult { limit: Some(TurnLimit { resets_at: None }), ..failed() });
}

#[test]
fn todo_write_reads_its_list_whole() {
    let mut book = PlanBook::default();
    assert_eq!(read_plan_call(Some("TodoWrite"), Some(&json!({"todos": []})), Some("c1"), &mut book), PlanCall::Steps(vec![]));
    assert_eq!(read_plan_call(Some("TodoWrite"), Some(&json!({"todos": "x"})), Some("c1"), &mut book), PlanCall::NotPlan);
    let steps = read_plan_call(Some("TodoWrite"), Some(&json!({"todos": [{"content": "a", "status": "completed"}, 3, {"status": "in_progress"}, {"content": "c", "status": "weird"}]})), None, &mut book);
    assert_eq!(
        steps,
        PlanCall::Steps(vec![
            PlanStep { text: "a".into(), state: PlanStepState::Done },
            PlanStep { text: "".into(), state: PlanStepState::Working },
            PlanStep { text: "c".into(), state: PlanStepState::Pending },
        ])
    );
    assert_eq!(read_plan_call(Some("ExitPlanMode"), Some(&json!({"plan": "# Plan"})), None, &mut book), PlanCall::Text("# Plan".into()));
    assert_eq!(read_plan_call(Some("ExitPlanMode"), Some(&json!({})), None, &mut book), PlanCall::NotPlan);
    assert_eq!(read_plan_call(Some("TaskList"), None, None, &mut book), PlanCall::Nothing);
    assert_eq!(read_plan_call(Some("Bash"), Some(&json!({})), None, &mut book), PlanCall::NotPlan);
    assert_eq!(read_plan_call(None, None, None, &mut book), PlanCall::NotPlan);
    assert!(book.tasks.is_empty() && book.unnamed.is_empty(), "TodoWrite keeps nothing in the book");
}

#[test]
fn task_update_moves_deletes_and_ignores_unknown_ids() {
    let mut book = PlanBook::default();
    read_plan_call(Some("TaskCreate"), Some(&json!({"subject": "parse"})), Some("call-1"), &mut book);
    let two = read_plan_call(Some("TaskCreate"), Some(&json!({})), None, &mut book);
    assert_eq!(two, PlanCall::Steps(vec![PlanStep { text: "parse".into(), state: PlanStepState::Pending }, PlanStep { text: "".into(), state: PlanStepState::Pending }]));
    assert_eq!(book.unnamed.clone().into_iter().collect::<Vec<_>>(), vec![("call-1".to_string(), 0usize)], "a create with no call id is never named");
    assert_eq!(read_plan_call(Some("TaskUpdate"), Some(&json!({"taskId": "1", "status": "completed"})), None, &mut book), PlanCall::Nothing, "no task has an id yet");
    book.tasks[0].id = Some("1".into());
    assert_eq!(
        read_plan_call(Some("TaskUpdate"), Some(&json!({"taskId": "1", "status": "in_progress", "subject": "parse it"})), None, &mut book),
        PlanCall::Steps(vec![PlanStep { text: "parse it".into(), state: PlanStepState::Working }, PlanStep { text: "".into(), state: PlanStepState::Pending }])
    );
    assert_eq!(read_plan_call(Some("TaskUpdate"), Some(&json!({"taskId": "1", "status": "deleted"})), None, &mut book), PlanCall::Steps(vec![PlanStep { text: "".into(), state: PlanStepState::Pending }]));
    assert_eq!(book.tasks.len(), 1);
    assert_eq!(book.tasks[0].id, None);
}

#[test]
fn child_calls_count_once_at_the_largest_output() {
    let mut book = SpendBook::default();
    let line = |id: &str, out: u64| obj(json!({"type": "assistant", "message": {"id": id, "usage": {"input_tokens": 5, "cache_read_input_tokens": 100, "output_tokens": out}}}));
    note_child_call(&mut book, "t1", &line("m1", 3));
    note_child_call(&mut book, "t1", &line("m1", 40));
    note_child_call(&mut book, "t1", &line("m1", 12));
    note_child_call(&mut book, "t1", &line("m2", 1));
    note_child_call(&mut book, "t1", &obj(json!({"type": "assistant", "message": {"id": "m3"}})));
    assert_eq!(book.totals.get("t1"), Some(&UsageTokens { input: 210, output: 41, cached: 200, cache_write: 0, reasoning: 0 }));
    assert_eq!(book.calls["t1"].keys().cloned().collect::<Vec<_>>(), vec!["m1", "m2"]);
    assert_eq!(subagent_spent(&book, "t1"), Some(SubagentUsage { tokens: Some(UsageTokens { input: 210, output: 41, cached: 200, cache_write: 0, reasoning: 0 }), tool_uses: None, duration_ms: None }));
    assert_eq!(subagent_spent(&book, "t2"), None);
}

#[test]
fn spend_at_end_takes_the_run_time_from_the_end_line_alone() {
    let mut book = SpendBook::default();
    let subagents = |t: &str| t == "t1";
    let progress = obj(json!({"type": "system", "subtype": "task_progress", "task_id": "t1", "usage": {"tool_uses": 4, "duration_ms": 900, "total_tokens": 99999}}));
    assert_eq!(note_progress(&mut book, &progress, &subagents), Some("t1".to_string()));
    let other = obj(json!({"type": "system", "subtype": "task_progress", "task_id": "bash-1", "usage": {"tool_uses": 1}}));
    assert_eq!(note_progress(&mut book, &other, &subagents), None, "a background command is no subagent");
    assert_eq!(note_progress(&mut book, &obj(json!({"type": "system", "subtype": "task_progress", "task_id": "t1"})), &subagents), None, "no usage, nothing noted");
    assert_eq!(subagent_spent(&book, "t1"), Some(SubagentUsage { tokens: None, tool_uses: Some(4), duration_ms: Some(JsNumber(900.0)) }));
    let killed = obj(json!({"type": "system", "subtype": "task_updated", "task_id": "t1", "patch": {"status": "killed"}}));
    assert_eq!(subagent_spent_at_end(&book, "t1", &killed), Some(SubagentUsage { tokens: None, tool_uses: Some(4), duration_ms: None }));
    let ended = obj(json!({"type": "system", "subtype": "task_notification", "task_id": "t1", "usage": {"duration_ms": 1500}}));
    assert_eq!(subagent_spent_at_end(&book, "t1", &ended), Some(SubagentUsage { tokens: None, tool_uses: Some(4), duration_ms: Some(JsNumber(1500.0)) }));
    assert_eq!(subagent_spent_at_end(&book, "nobody", &ended), None);
}
