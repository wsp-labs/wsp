// SPDX-License-Identifier: AGPL-3.0-only
//! The codex_items unit (spec/codex_items.md): no TS file tests these helpers alone, so each case here is built from
//! the expected values of the packages/adapter-codex/test/adapter.test.ts case it names, or from the spec rule it pins.

use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_adapters::codex::items::*;
use wsp_adapters::pending::{codex_key_refused_line, codex_missing_env_line, codex_not_signed_in_line};
use wsp_adapters::port::{AdapterEvent, DeltaKind, FilePatch, PlanStep, SubagentState, TurnDelta, TurnTokens};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::TurnRefusal;
use wsp_wire::views::session_events::{PatchHunk, PlanStepState};

const LOGIN: &str = "codex login --device-auth";
const S: &str = "thr-1";

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        other => panic!("not an object: {other}"),
    }
}

fn deltas(events: Vec<AdapterEvent>) -> Vec<TurnDelta> {
    events
        .into_iter()
        .map(|e| match e {
            AdapterEvent::TurnDelta(d) => d,
            other => panic!("not a delta: {other:?}"),
        })
        .collect()
}

fn delta(kind: DeltaKind, text: &str) -> TurnDelta {
    TurnDelta::new(S, kind, text)
}

fn n(v: f64) -> JsNumber {
    JsNumber(v)
}

// Rule 1 and 2: failure words. adapter.test.ts "a 401 on a turn that was handed the vault's key ...".

#[test]
fn a_401_with_a_key_handed_is_the_refused_key_line_with_the_providers_reason_and_no() {
    let words = failure_words("unexpected status 401 Unauthorized: token expired, url: https://api.openai.com/v1/responses, cf-ray: x", LOGIN, Some("OPENAI_API_KEY")).unwrap();
    assert_eq!(words, FailureWords { line: codex_key_refused_line("OPENAI_API_KEY", "token expired", LOGIN), cause: Some(TurnRefusal::SignIn) });
}

#[test]
fn a_401_with_no_key_handed_is_the_not_signed_in_line_claimed_as_sign_in() {
    let words = failure_words("unexpected status 401 Unauthorized: token expired", LOGIN, None).unwrap();
    assert_eq!(words, FailureWords { line: codex_not_signed_in_line(LOGIN), cause: Some(TurnRefusal::SignIn) });
}

#[test]
fn a_missing_env_key_names_the_variable_and_claims_no_cause() {
    let words = failure_words("Missing environment variable: `FAKE_API_KEY`.", LOGIN, None).unwrap();
    assert_eq!(words, FailureWords { line: codex_missing_env_line("FAKE_API_KEY"), cause: None });
}

#[test]
fn any_other_message_has_no_words_of_wsps() {
    assert_eq!(failure_words("stream disconnected before completion", LOGIN, None), None);
}

#[test]
fn refused_because_takes_the_clause_after_the_status_to_the_url_or_bracket() {
    assert_eq!(refused_because("unexpected status 401 Unauthorized: token expired, url: https://x, cf-ray: y"), "token expired");
    assert_eq!(refused_because("401 Unauthorized:   bad key (see docs)\nsecond line"), "bad key");
    assert_eq!(refused_because("401 Unauthorized"), "");
    assert_eq!(refused_because("no status here"), "");
}

// Rule 3: retries. adapter.test.ts "says each retry, which one of how many and why ...".

#[test]
fn a_reconnecting_error_with_a_count_is_a_retry_naming_the_status_where_codex_kept() {
    let info = json!({"responseStreamDisconnected": {"httpStatusCode": 401}});
    assert_eq!(retry_of("Reconnecting... 2/5", Some(&info), S), Some(AdapterEvent::TurnRetry { session_id: S.into(), attempt: 2.0, of: Some(5.0), why: Some("the provider answered 401".into()) }));
    let none = json!({"responseStreamDisconnected": {"httpStatusCode": null}});
    assert_eq!(retry_of("Reconnecting... 1/5", Some(&none), S), Some(AdapterEvent::TurnRetry { session_id: S.into(), attempt: 1.0, of: Some(5.0), why: Some("the connection dropped".into()) }));
}

#[test]
fn reconnecting_with_no_count_or_any_other_message_is_no_retry() {
    assert_eq!(retry_of("Reconnecting...", None, S), None);
    assert_eq!(retry_of("We're currently experiencing high demand", None, S), None);
    assert!(is_reconnecting("Reconnecting... 2/5"));
    assert!(!is_reconnecting("not Reconnecting... 2/5"));
}

// Rule 4 and 5.

#[test]
fn an_item_needs_a_string_id_and_type_the_empty_string_counting() {
    let item = item_of(&obj(json!({"item": {"type": "agentMessage", "id": "", "text": "hi"}}))).unwrap();
    assert_eq!(item.get("text"), Some(&json!("hi")));
    assert_eq!(item_of(&obj(json!({"item": {"type": "agentMessage"}}))), None);
    assert_eq!(item_of(&obj(json!({"item": [1]}))), None);
}

#[test]
fn changes_read_the_kinds_own_word_and_default_to_change() {
    let changes = json!([{"path": "README.md", "kind": {"type": "update", "move_path": null}}, {"path": "docs/new.md", "kind": "add"}, {"kind": 5}, 7]);
    assert_eq!(changes_of(Some(&changes)), vec![Change { path: "README.md".into(), kind: "update".into() }, Change { path: "docs/new.md".into(), kind: "add".into() }, Change { path: "".into(), kind: "change".into() }]);
    assert_eq!(changes_of(Some(&json!("x"))), vec![]);
    assert_eq!(change_lines(Some(&changes)), "update README.md\nadd docs/new.md\nchange");
}

// Rule 6: patches. adapter.test.ts "a finished command carries its exit code and how long it ran, and each file change its hunks".

#[test]
fn each_file_change_carries_its_hunks_and_a_rename_its_moved_to() {
    let changes = json!([
        {"path": "/root/lab/f.txt", "kind": {"type": "update", "move_path": null}, "diff": "@@ -1,3 +1,3 @@\n alpha\n-beta\n+BETA\n gamma\n"},
        {"path": "/root/lab/new.txt", "kind": {"type": "add"}, "diff": "hi\nthere\n"},
        {"path": "/root/lab/old.txt", "kind": {"type": "delete"}, "diff": "gone\n"},
        {"path": "/root/lab/a.txt", "kind": {"type": "update", "move_path": "/root/lab/b.txt"}, "diff": "@@ -2 +2 @@\n-x\n+y\n\n\nMoved to: /root/lab/b.txt"}
    ]);
    let hunk = |os: i64, ol: i64, ns: i64, nl: i64, lines: &[&str]| PatchHunk { old_start: os.into(), old_lines: ol.into(), new_start: ns.into(), new_lines: nl.into(), lines: lines.iter().map(|s| s.to_string()).collect() };
    assert_eq!(
        file_patches(Some(&changes)),
        Some(vec![
            FilePatch { path: "/root/lab/f.txt".into(), hunks: vec![hunk(1, 3, 1, 3, &[" alpha", "-beta", "+BETA", " gamma"])], moved_to: None },
            FilePatch { path: "/root/lab/new.txt".into(), hunks: vec![hunk(0, 0, 1, 2, &["+hi", "+there"])], moved_to: None },
            FilePatch { path: "/root/lab/old.txt".into(), hunks: vec![hunk(1, 1, 0, 0, &["-gone"])], moved_to: None },
            FilePatch { path: "/root/lab/a.txt".into(), hunks: vec![hunk(2, 1, 2, 1, &["-x", "+y"])], moved_to: Some("/root/lab/b.txt".into()) },
        ])
    );
}

#[test]
fn changes_whose_diffs_are_empty_carry_no_patch_at_all() {
    let changes = json!([{"path": "README.md", "kind": {"type": "update"}, "diff": ""}, {"path": "docs/new.md", "kind": {"type": "add"}, "diff": ""}, {"path": "", "kind": "add", "diff": "x\n"}]);
    assert_eq!(file_patches(Some(&changes)), None);
}

// Rule 7: item deltas.

#[test]
fn a_command_starts_as_its_script_and_completes_with_its_output_exit_code_and_durat() {
    let item = obj(json!({"type": "commandExecution", "id": "call_8", "command": "/bin/zsh -lc 'npm test'", "status": "failed", "aggregatedOutput": "1 failing\n", "exitCode": 1, "durationMs": 1534}));
    let start = deltas(item_deltas(false, &item, S));
    assert_eq!(start, vec![TurnDelta { tool_name: Some("command_execution".into()), tool_use_id: Some("call_8".into()), ..delta(DeltaKind::ToolUse, r#"{"command":"npm test"}"#) }]);
    let done = deltas(item_deltas(true, &item, S));
    assert_eq!(done, vec![TurnDelta { tool_use_id: Some("call_8".into()), is_error: Some(true), exit_code: Some(1), duration_ms: Some(1534.0), ..delta(DeltaKind::ToolResult, "1 failing\n") }]);
}

#[test]
fn a_declined_file_change_is_an_error_with_no_patch_and_a_completed_one_carries_it() {
    let changes = json!([{"path": "/root/lab/new.txt", "kind": {"type": "add"}, "diff": "hi\n"}]);
    let declined = obj(json!({"type": "fileChange", "id": "call_10", "changes": changes, "status": "declined"}));
    let [d] = deltas(item_deltas(true, &declined, S)).try_into().unwrap();
    assert_eq!(d, TurnDelta { tool_use_id: Some("call_10".into()), is_error: Some(true), ..delta(DeltaKind::ToolResult, "add /root/lab/new.txt") });
    let done = obj(json!({"type": "fileChange", "id": "call_9", "changes": changes, "status": "completed"}));
    let [c] = deltas(item_deltas(true, &done, S)).try_into().unwrap();
    assert_eq!(c.is_error, Some(false));
    assert_eq!(c.patch.map(|p| p.len()), Some(1));
    let [u] = deltas(item_deltas(false, &done, S)).try_into().unwrap();
    assert_eq!(u, TurnDelta { tool_name: Some("file_change".into()), tool_use_id: Some("call_9".into()), ..delta(DeltaKind::ToolUse, r#"{"changes":[{"path":"/root/lab/new.txt","kind":"add"}]}"#) });
}

#[test]
fn an_mcp_call_the_tool_failed_keeps_its_words_one_that_never_reached_it_the_error() {
    // adapter.test.ts "an MCP call the tool failed keeps the tool's words ...".
    let said = json!([{"type": "text", "text": "slate refused: 1 error: P100 bad-syntax"}]);
    let texts: Vec<(String, Option<bool>)> = [
        json!({"type": "mcpToolCall", "id": "call_4", "server": "wsp", "tool": "slate_write", "arguments": {}, "status": "failed", "result": {"content": said}, "error": null}),
        json!({"type": "mcpToolCall", "id": "call_5", "server": "wsp", "tool": "slate_write", "arguments": {}, "status": "failed", "result": null, "error": {"message": "server gone"}}),
        json!({"type": "mcpToolCall", "id": "call_6", "server": "wsp", "tool": "slate_write", "arguments": {}, "status": "failed", "result": null, "error": null}),
    ]
    .into_iter()
    .map(|i| {
        let [d] = deltas(item_deltas(true, &obj(i), S)).try_into().unwrap();
        (d.text, d.is_error)
    })
    .collect();
    assert_eq!(texts, vec![(serde_json::to_string(&said).unwrap(), Some(true)), ("server gone".into(), Some(true)), ("".into(), Some(true))]);
}

#[test]
fn an_mcp_call_to_the_slate_server_is_named_as_wsps_and_its_arguments_ride_as_json() {
    let item = obj(json!({"type": "mcpToolCall", "id": "call_7", "server": "wsp_slate", "tool": "slate_write", "arguments": {"text": "<slate/>"}, "status": "inProgress"}));
    let [d] = deltas(item_deltas(false, &item, S)).try_into().unwrap();
    assert_eq!(d, TurnDelta { tool_name: Some("mcp__wsp__slate_write".into()), tool_use_id: Some("call_7".into()), ..delta(DeltaKind::ToolUse, r#"{"text":"<slate/>"}"#) });
    let bare = obj(json!({"type": "mcpToolCall", "id": "c", "arguments": null, "status": "completed", "result": {"content": null}}));
    let [u] = deltas(item_deltas(false, &bare, S)).try_into().unwrap();
    assert_eq!((u.tool_name.as_deref(), u.text.as_str()), (Some("mcp__mcp__tool"), "{}"));
    let [r] = deltas(item_deltas(true, &bare, S)).try_into().unwrap();
    assert_eq!((r.text.as_str(), r.is_error), ("[]", Some(false)));
}

#[test]
fn reasoning_reads_its_summary_else_its_content_and_says_nothing_when_both_are_empt() {
    let summary = obj(json!({"type": "reasoning", "id": "rs_1", "summary": ["Listing the repository first."], "content": ["hidden"]}));
    assert_eq!(deltas(item_deltas(true, &summary, S)), vec![delta(DeltaKind::Thinking, "Listing the repository first.")]);
    let content = obj(json!({"type": "reasoning", "id": "rs_2", "summary": [], "content": ["a", 3, "b"]}));
    assert_eq!(deltas(item_deltas(true, &content, S)), vec![delta(DeltaKind::Thinking, "a\nb")]);
    assert_eq!(item_deltas(true, &obj(json!({"type": "reasoning", "id": "rs_3", "summary": [], "content": []})), S), vec![]);
    assert_eq!(item_deltas(false, &summary, S), vec![]);
}

#[test]
fn an_agent_message_is_text_on_completion_under_its_own_id_and_nothing_on_start() {
    let item = obj(json!({"type": "agentMessage", "id": "msg_1", "text": "done"}));
    assert_eq!(item_deltas(false, &item, S), vec![]);
    assert_eq!(deltas(item_deltas(true, &item, S)), vec![TurnDelta { message_id: Some("msg_1".into()), ..delta(DeltaKind::Text, "done") }]);
}

#[test]
fn a_web_search_draws_its_call_and_its_result_on_completion_alone() {
    let item = obj(json!({"type": "webSearch", "id": "ws_1", "query": "codex app-server"}));
    assert_eq!(item_deltas(false, &item, S), vec![]);
    assert_eq!(
        deltas(item_deltas(true, &item, S)),
        vec![
            TurnDelta { tool_name: Some("web_search".into()), tool_use_id: Some("ws_1".into()), ..delta(DeltaKind::ToolUse, r#"{"query":"codex app-server"}"#) },
            TurnDelta { tool_use_id: Some("ws_1".into()), is_error: Some(false), ..delta(DeltaKind::ToolResult, "codex app-server") },
        ]
    );
}

#[test]
fn a_spawn_draws_spawn_agent_with_its_prompt_and_other_collab_tools_draw_nothing() {
    let spawn = obj(json!({"type": "collabAgentToolCall", "id": "call_spawn_1", "tool": "spawnAgent", "status": "completed", "prompt": "Sleep 20 s and say done.\nEnd without waiting."}));
    assert_eq!(
        deltas(item_deltas(false, &spawn, S)),
        vec![TurnDelta { tool_name: Some("spawn_agent".into()), tool_use_id: Some("call_spawn_1".into()), ..delta(DeltaKind::ToolUse, r#"{"prompt":"Sleep 20 s and say done.\nEnd without waiting."}"#) }]
    );
    assert_eq!(deltas(item_deltas(true, &spawn, S)), vec![TurnDelta { tool_use_id: Some("call_spawn_1".into()), is_error: Some(false), ..delta(DeltaKind::ToolResult, "") }]);
    let close = obj(json!({"type": "collabAgentToolCall", "id": "c2", "tool": "closeAgent", "status": "completed"}));
    assert_eq!(item_deltas(true, &close, S), vec![]);
    assert_eq!(item_deltas(true, &obj(json!({"type": "contextCompaction", "id": "cc1"})), S), vec![]);
}

// Rule 8.

#[test]
fn a_tool_call_as_earlier_messages_show_it_is_the_items_tool_use() {
    let item = obj(json!({"type": "commandExecution", "id": "c1", "command": "bash -c 'ls -R'", "status": "completed"}));
    assert_eq!(tool_call_of(&item), Some(("command_execution".into(), r#"{"command":"ls -R"}"#.into())));
    let search = obj(json!({"type": "webSearch", "id": "ws", "query": "q"}));
    assert_eq!(tool_call_of(&search), Some(("web_search".into(), r#"{"query":"q"}"#.into())));
    assert_eq!(tool_call_of(&obj(json!({"type": "agentMessage", "id": "m", "text": "x"}))), None);
    assert_eq!(tool_call_of(&obj(json!({"type": "webSearch", "query": "q"}))), None);
}

// Rule 9: tokens. adapter.test.ts "counts a resumed thread's turn from where its running total stood".

fn usage(total: (f64, f64), last: (f64, f64)) -> (Map<String, Value>, Map<String, Value>) {
    let t = obj(json!({"inputTokens": total.0, "cachedInputTokens": 0, "cacheWriteInputTokens": 0, "outputTokens": total.1, "reasoningOutputTokens": 0, "totalTokens": total.0 + total.1}));
    let l = obj(json!({"inputTokens": last.0, "cachedInputTokens": 0, "cacheWriteInputTokens": 0, "outputTokens": last.1, "reasoningOutputTokens": 0, "totalTokens": last.0 + last.1}));
    (t, l)
}

#[test]
fn a_resumed_threads_turn_counts_from_where_its_running_total_stood() {
    let (t1, l1) = usage((50_000.0, 900.0), (20_000.0, 100.0));
    let before = total_before(&t1, &l1);
    assert_eq!(before, obj(json!({"inputTokens": 30_000.0, "cachedInputTokens": 0.0, "cacheWriteInputTokens": 0.0, "outputTokens": 800.0, "reasoningOutputTokens": 0.0})));
    let (t2, l2) = usage((72_000.0, 1_000.0), (22_000.0, 100.0));
    let tokens = turn_tokens_of(&UsageSeen { before, total: t2, last: l2, window: Some(258_400.0) });
    assert_eq!(tokens, TurnTokens { input: n(42_000.0), output: n(200.0), cached: Some(n(0.0)), cache_write: Some(n(0.0)), reasoning: Some(n(0.0)), context: Some(n(22_100.0)), window: Some(n(258_400.0)) });
}

#[test]
fn a_field_the_total_names_not_falls_back_to_the_latest_calls() {
    let seen = UsageSeen { before: Map::new(), total: Map::new(), last: obj(json!({"inputTokens": 900, "cachedInputTokens": 880, "outputTokens": 12, "reasoningOutputTokens": 0, "totalTokens": 912})), window: None };
    // adapter.test.ts "forks the thread ephemeral and read-only ...": the side question's usage.
    assert_eq!(turn_tokens_of(&seen), TurnTokens { input: n(900.0), output: n(12.0), cached: Some(n(880.0)), cache_write: None, reasoning: Some(n(0.0)), context: Some(n(912.0)), window: None });
}

#[test]
fn what_the_model_held_is_the_total_tokens_else_input_and_output() {
    assert_eq!(held_of(&obj(json!({"totalTokens": 4607, "inputTokens": 0, "outputTokens": 0}))), 4607.0);
    assert_eq!(held_of(&obj(json!({"inputTokens": 20, "outputTokens": 5}))), 25.0);
    assert_eq!(held_of(&obj(json!({"outputTokens": 5}))), 5.0);
    assert_eq!(held_of(&Map::new()), 0.0);
}

// Rule 10.

#[test]
fn a_childs_usage_is_its_whole_breakdown_each_missing_field_zero_and_none_without_a() {
    // adapter.test.ts "holds the lead's reply while its child runs": the child's own thread's running total.
    let u = child_usage_of(Some(&obj(json!({"inputTokens": 900, "outputTokens": "x"})))).unwrap();
    assert_eq!(u.tokens, Some(wsp_wire::types::usage::UsageTokens { input: 900.0.into(), output: 0.0.into(), cached: 0.0.into(), cache_write: 0.0.into(), reasoning: 0.0.into() }));
    assert_eq!((u.tool_uses, u.duration_ms), (None, None));
    assert_eq!(child_usage_of(Some(&obj(json!({"totalTokens": 5})))), None);
    assert_eq!(child_usage_of(None), None);
}

#[test]
fn the_child_tokens_add_to_the_leads_and_leave_its_context_and_window() {
    let lead = TurnTokens { input: n(36_576.0), output: n(10.0), cached: None, cache_write: None, reasoning: None, context: Some(n(18_315.0)), window: Some(n(258_400.0)) };
    let mut children = IndexMap::new();
    children.insert("input".to_string(), 20_756.0);
    children.insert("cached".to_string(), 3.0);
    assert_eq!(with_child_tokens(Some(lead.clone()), &children), Some(TurnTokens { input: n(57_332.0), cached: Some(n(3.0)), ..lead.clone() }));
    assert_eq!(with_child_tokens(Some(lead.clone()), &IndexMap::new()), Some(lead));
    assert_eq!(with_child_tokens(None, &children), Some(TurnTokens { input: n(20_756.0), output: n(0.0), cached: Some(n(3.0)), cache_write: None, reasoning: None, context: Some(n(0.0)), window: None }));
    assert_eq!(with_child_tokens(None, &IndexMap::new()), None);
}

// Rule 11.

#[test]
fn a_childs_end_depth_and_a_plan_steps_state_read_in_the_shared_words() {
    assert_eq!(child_end(Some("completed")), SubagentState::Done);
    assert_eq!(child_end(Some("interrupted")), SubagentState::Stopped);
    assert_eq!(child_end(None), SubagentState::Failed);
    assert_eq!(depth_of("/root/alpha"), 1);
    assert_eq!(depth_of("/root/alpha/beta"), 2);
    assert_eq!(depth_of("/root"), 1);
    assert_eq!(step_state(Some(&json!("completed"))), PlanStepState::Done);
    assert_eq!(step_state(Some(&json!("inProgress"))), PlanStepState::Working);
    assert_eq!(step_state(Some(&json!("pending"))), PlanStepState::Pending);
    assert_eq!(step_state(None), PlanStepState::Pending);
}

#[test]
fn the_plan_reads_each_step_and_its_state_skipping_what_is_not_an_object() {
    // adapter.test.ts "reads the plan the server updates as the turn's steps".
    let plan = json!([{"step": "read", "status": "completed"}, {"step": "write", "status": "inProgress"}, 3, {"status": "pending"}]);
    assert_eq!(
        plan_steps(Some(&plan)),
        vec![PlanStep { text: "read".into(), state: PlanStepState::Done }, PlanStep { text: "write".into(), state: PlanStepState::Working }, PlanStep { text: "".into(), state: PlanStepState::Pending }]
    );
    assert_eq!(plan_steps(Some(&json!("x"))), vec![]);
}

// Rules 12 and 13.

#[test]
fn compact_alone_is_the_servers_compaction_and_with_words_after_it_a_turn() {
    // adapter.test.ts "runs /compact as the server's own compaction" and "sends any other message as a turn".
    let compact: Value = serde_json::from_str(&turn_line_for("t1", "  /compact\n", &[], Some("high"))).unwrap();
    assert_eq!(compact, json!({"id": "wsp-turn", "method": "thread/compact/start", "params": {"threadId": "t1"}}));
    let turn: Value = serde_json::from_str(&turn_line_for("t1", "/compact the notes please", &["/i/1.png".into()], Some("high"))).unwrap();
    assert_eq!(turn, json!({"id": "wsp-turn", "method": "turn/start", "params": {"threadId": "t1", "input": [{"type": "text", "text": "/compact the notes please"}, {"type": "localImage", "path": "/i/1.png"}], "effort": "high"}}));
}

#[test]
fn a_line_already_starting_a_turn_is_told_apart_from_any_other() {
    assert!(is_turn_start(r#"{"id":"wsp-turn","method":"turn/start","params":{}}"#));
    assert!(is_turn_start(r#"{"id":"wsp-turn","method":"thread/compact/start","params":{}}"#));
    assert!(!is_turn_start(r#"{"id":"wsp-thread","method":"thread/resume"}"#));
    assert!(!is_turn_start("null"));
    assert!(!is_turn_start("not json"));
}

#[test]
fn a_threads_rollout_is_found_three_folders_under_sessions_by_its_name_end() {
    let file = rollout_in("/root/.codex", "01a0");
    assert_eq!((file.folder.as_str(), file.name.as_str(), file.depth), ("/root/.codex/sessions", "-01a0.jsonl", 3));
}

/// Rules CI1 to CI9 of spec/codex_items.md, "Rules added for the port", each value printed by the TS at 65f6b89c7 (the
/// module-private helpers run from a copy of adapter.ts with exports added).
mod rules_added {
    use super::*;

    const ROAD: &str = ". Put a working key in ~/.wsp/.env, or sign Codex in where this thread runs with codex login --device-auth.";

    fn words(line: &str) -> Option<FailureWords> {
        Some(FailureWords { line: line.into(), cause: Some(TurnRefusal::SignIn) })
    }

    #[test]
    fn a_refused_key_s_reason_is_cut_to_80_units_an_empty_key_env_is_one_and_401_beats_a_missing_variable() {
        // CI1; adapter.ts:288-295, words/places.ts:315.
        let long = "x".repeat(100);
        assert_eq!(failure_words(&format!("401 Unauthorized: {long}"), LOGIN, Some("OPENAI_API_KEY")), words(&format!("OpenAI refused your OPENAI_API_KEY: {}{ROAD}", "x".repeat(80))));
        assert_eq!(failure_words("401 Unauthorized:", LOGIN, Some("OPENAI_API_KEY")), words(&format!("OpenAI refused your OPENAI_API_KEY{ROAD}")));
        assert_eq!(failure_words("401 Unauthorized: bad", LOGIN, Some("")), words(&format!("OpenAI refused your : bad{ROAD}")));
        assert_eq!(failure_words("401 Unauthorized and Missing environment variable: `K`", LOGIN, None), words("Codex is not signed in where this thread runs; run codex login --device-auth there"));
        assert_eq!(failure_words("Missing environment variable: ``", LOGIN, None), None);
    }

    #[test]
    fn a_patch_takes_the_diff_road_for_any_kind_but_add_and_delete_and_a_move_alone_is_a_patch() {
        // CI2; adapter.ts:305-338. changes_of keeps an empty kind, file_patches has no "change" default.
        let hunk = |os: i64, ol: i64, ns: i64, nl: i64, lines: &[&str]| PatchHunk { old_start: os.into(), old_lines: ol.into(), new_start: ns.into(), new_lines: nl.into(), lines: lines.iter().map(|s| s.to_string()).collect() };
        assert_eq!(file_patches(Some(&json!([{"path": "a", "diff": "@@ -1 +1 @@\n-a\n+b\n"}]))), Some(vec![FilePatch { path: "a".into(), hunks: vec![hunk(1, 1, 1, 1, &["-a", "+b"])], moved_to: None }]));
        assert_eq!(file_patches(Some(&json!([{"path": "n", "kind": {"type": "add"}, "diff": "\n"}]))), Some(vec![FilePatch { path: "n".into(), hunks: vec![hunk(0, 0, 1, 1, &["+"])], moved_to: None }]));
        assert_eq!(file_patches(Some(&json!([{"path": "m", "kind": {"type": "update"}, "diff": "\n\nMoved to: b.ts"}]))), Some(vec![FilePatch { path: "m".into(), hunks: vec![], moved_to: Some("b.ts".into()) }]));
        assert_eq!(changes_of(Some(&json!([{"path": "p", "kind": {"type": ""}}, null, {}]))), vec![Change { path: "p".into(), kind: String::new() }, Change { path: String::new(), kind: "change".into() }]);
        assert_eq!(change_lines(Some(&json!([{"path": "p", "kind": {"type": ""}}]))), "p");
    }

    #[test]
    fn reasoning_reads_summary_whenever_it_holds_a_string_even_an_empty_one() {
        // CI3; adapter.ts:410-414.
        let reasoning = |summary: Value, content: Value| deltas(item_deltas(true, &obj(json!({"type": "reasoning", "id": "r", "summary": summary, "content": content})), "S"));
        assert_eq!(reasoning(json!([""]), json!(["c"])), vec![]);
        assert_eq!(reasoning(json!(["", ""]), json!([])), vec![TurnDelta::new("S", DeltaKind::Thinking, "\n")]);
        assert_eq!(reasoning(json!([5]), json!(["c"])), vec![TurnDelta::new("S", DeltaKind::Thinking, "c")]);
    }

    #[test]
    fn only_completed_is_success_and_a_failed_call_with_null_content_says_an_empty_list() {
        // CI4; adapter.ts:425-460.
        let mcp = deltas(item_deltas(true, &obj(json!({"type": "mcpToolCall", "id": "m", "server": "", "tool": "", "status": "failed", "result": {"content": null}})), "S"));
        assert_eq!(mcp, vec![TurnDelta { tool_use_id: Some("m".into()), is_error: Some(true), ..TurnDelta::new("S", DeltaKind::ToolResult, "[]") }]);
        let command = deltas(item_deltas(true, &obj(json!({"type": "commandExecution", "id": "c", "command": "ls"})), "S"));
        assert_eq!(command, vec![TurnDelta { tool_use_id: Some("c".into()), is_error: Some(true), ..TurnDelta::new("S", DeltaKind::ToolResult, "") }]);
    }

    #[test]
    fn a_path_s_depth_counts_its_non_empty_segments_less_one_never_below_one() {
        // CI5; adapter.ts:467.
        assert_eq!(["", "/", "x/y/z/w", "//root//a//"].map(depth_of), [1, 1, 3, 1]);
    }

    #[test]
    fn a_compaction_is_the_js_trimmed_prompt_alone_and_drops_images_and_effort() {
        // CI6; adapter.ts:495-499. JS trim takes U+2003 and U+FEFF, not U+200B; an empty effort is written.
        let compact = r#"{"id":"wsp-turn","method":"thread/compact/start","params":{"threadId":"T"}}"#;
        assert_eq!(turn_line_for("T", "/compact", &["/a.png".into()], Some("high")), compact);
        assert_eq!(turn_line_for("T", "\u{2003}/compact\u{FEFF}", &[], None), compact);
        assert_eq!(turn_line_for("T", "/compact\u{200B}", &[], None), "{\"id\":\"wsp-turn\",\"method\":\"turn/start\",\"params\":{\"threadId\":\"T\",\"input\":[{\"type\":\"text\",\"text\":\"/compact\u{200B}\"}]}}");
        assert_eq!(turn_line_for("T", "x", &[], Some("")), r#"{"id":"wsp-turn","method":"turn/start","params":{"threadId":"T","input":[{"type":"text","text":"x"}],"effort":""}}"#);
    }

    #[test]
    fn a_turn_start_is_an_object_line_whose_method_starts_a_turn_or_a_compaction() {
        // CI7; adapter.ts:501-509.
        let got = ["null", "5", "[]", r#""turn/start""#, r#"{"method":5}"#, "", r#" {"id":"wsp-turn","method":"turn/start"} "#, r#"{"method":"thread/compact/start"}"#].map(is_turn_start);
        assert_eq!(got, [false, false, false, false, false, false, true, true]);
    }

    #[test]
    fn a_call_s_arguments_are_written_as_js_writes_them_integer_keys_first_and_js_numbers() {
        // CI8; adapter.ts:397 (`JSON.stringify(item.arguments)`), the coordinator's ruling on a whole-value JS writer.
        let call = deltas(item_deltas(false, &obj(json!({"type": "mcpToolCall", "id": "m", "server": "s", "tool": "t", "arguments": {"b": 1, "2": "x", "a": 2, "1": "y", "f": 1.0, "g": 1e21}})), "S"));
        assert_eq!(
            call,
            vec![TurnDelta { tool_name: Some("mcp__s__t".into()), tool_use_id: Some("m".into()), ..TurnDelta::new("S", DeltaKind::ToolUse, r#"{"1":"y","2":"x","b":1,"a":2,"f":1,"g":1e+21}"#) }]
        );
    }

    #[test]
    fn child_tokens_over_no_lead_start_from_zero_and_only_breakdown_names_count() {
        // CI9; adapter.ts:476-493.
        let mut bogus = ChildTokens::new();
        bogus.insert("bogus".into(), 5.0);
        let n = |v: f64| Some(JsNumber(v));
        assert_eq!(with_child_tokens(None, &bogus), Some(TurnTokens { input: JsNumber(0.0), output: JsNumber(0.0), cached: None, cache_write: None, reasoning: None, context: n(0.0), window: None }));
        let mut kids = ChildTokens::new();
        kids.insert("reasoning".into(), 2.0);
        kids.insert("input".into(), 1.0);
        let lead = TurnTokens { input: JsNumber(6.0), output: JsNumber(2.0), cached: None, cache_write: None, reasoning: None, context: n(3.0), window: n(9.0) };
        assert_eq!(with_child_tokens(Some(lead), &kids), Some(TurnTokens { input: JsNumber(7.0), output: JsNumber(2.0), cached: None, cache_write: None, reasoning: n(2.0), context: n(3.0), window: n(9.0) }));
    }
}

/// Rule CI10 of spec/codex_items.md, printed by the TS at 65f6b89c7.
mod rules_added_retry {
    use super::*;

    fn retry(attempt: f64, of: f64, why: &str) -> Option<AdapterEvent> {
        Some(AdapterEvent::TurnRetry { session_id: "S".into(), attempt, of: Some(of), why: Some(why.into()) })
    }

    #[test]
    fn a_retry_is_a_prefix_match_with_a_count_and_only_a_numeric_status_names_the_provider() {
        // CI10; adapter.ts:262-273.
        let zero = json!({"responseStreamDisconnected": {"httpStatusCode": 0}});
        assert_eq!(retry_of("Reconnecting... 2/5 (extra)", Some(&zero), "S"), retry(2.0, 5.0, "the provider answered 0"));
        assert_eq!(retry_of("Reconnecting... 02/005", Some(&zero), "S"), retry(2.0, 5.0, "the provider answered 0"));
        assert_eq!(retry_of("Reconnecting... 5/3", Some(&zero), "S"), retry(5.0, 3.0, "the provider answered 0"));
        for none in ["Reconnecting...2/5", " Reconnecting... 2/5", "Reconnecting..."] {
            assert_eq!(retry_of(none, Some(&zero), "S"), None, "{none}");
        }
        assert_eq!(retry_of("Reconnecting... 1/5", None, "S"), retry(1.0, 5.0, "the connection dropped"));
        assert_eq!(retry_of("Reconnecting... 1/5", Some(&json!({"responseStreamDisconnected": {"httpStatusCode": "500"}})), "S"), retry(1.0, 5.0, "the connection dropped"));
        assert_eq!(retry_of("Reconnecting... 1/5", Some(&json!({"responseStreamDisconnected": {"httpStatusCode": 1.5}})), "S"), retry(1.0, 5.0, "the provider answered 1.5"));
        assert!(is_reconnecting("Reconnecting..."));
    }
}
