// SPDX-License-Identifier: AGPL-3.0-only
//! Ports of packages/adapter-claude/test/permissions.test.ts, steers.test.ts, tool-results.test.ts and
//! shell-cwd.test.ts: the control channel's lines, the steers a re-opened turn waits on, the facts a tool result
//! carries, and where the agent's shell is.

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::collections::HashMap;

use serde_json::{json, Map, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::claude::landmines::user_message_line;
use wsp_adapters::claude::permissions::{control_answer_line, control_error_line, control_line, set_mode_line, ControlLine};
use wsp_adapters::claude::shell_cwd::shell_cwd_after;
use wsp_adapters::claude::steers::steered_ids;
use wsp_adapters::port::{
    DeltaKind, FilePatch, HarnessStartOptions, PermissionAsk, PermissionOption, TurnDelta, PERMISSION_ALLOW, PERMISSION_DENY,
};
use wsp_wire::views::session_events::PatchHunk;
use wsp_wire::wire::helpers::PermissionEffect;

use claude_exec::{collect, FakeExec, StreamOpts};

// ---------------------------------------------------------------------------------------------------------------
// permissions.test.ts
// ---------------------------------------------------------------------------------------------------------------

const REQUEST_ID: &str = "d9aa99d3-be4e-4a2b-8766-1b9494cde4f6";

fn obj(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

/// One can_use_tool request as the binary sent it; an override of `None` is a key the TS test set to `undefined`.
fn ask_line(overrides: &[(&str, Option<Value>)]) -> Map<String, Value> {
    let mut line = obj(json!({
        "type": "control_request",
        "request_id": REQUEST_ID,
        "request": {
            "subtype": "can_use_tool",
            "tool_name": "Write",
            "display_name": "Write",
            "input": { "file_path": "/tmp/probe/out.txt", "content": "hi" },
            "description": "out.txt",
            "permission_suggestions": [{ "type": "setMode", "mode": "acceptEdits", "destination": "session" }],
            "tool_use_id": "toolu_01WspFixAsk1",
        },
    }));
    let request = line.get_mut("request").and_then(Value::as_object_mut).unwrap();
    for (key, value) in overrides {
        match value {
            Some(v) => {
                request.insert(key.to_string(), v.clone());
            }
            None => {
                request.shift_remove(*key);
            }
        }
    }
    line
}

fn ask(overrides: &[(&str, Option<Value>)]) -> PermissionAsk {
    match control_line(&ask_line(overrides)) {
        Some(ControlLine::Ask { ask, .. }) => ask,
        other => panic!("expected an ask, got {other:?}"),
    }
}

fn option(id: &str, label: &str, effect: PermissionEffect, mode: Option<&str>) -> PermissionOption {
    PermissionOption { id: id.to_string(), label: label.to_string(), effect, mode: mode.map(String::from) }
}

fn option_ids(ask: &PermissionAsk) -> Vec<String> {
    ask.options.iter().map(|o| o.id.clone()).collect()
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

use wsp_adapters::pending::{other_option_id, picked_option_id};

fn answer_response(line: &str) -> Value {
    let parsed: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("not JSON ({e}): {line}"));
    parsed["response"]["response"].clone()
}

// TS: permissions.test.ts:34
#[test]
fn control_line_reads_a_can_use_tool_request_as_the_prompt_a_person_answers() {
    assert_eq!(
        ask(&[]),
        PermissionAsk {
            ask_id: REQUEST_ID.to_string(),
            tool_name: "Write".to_string(),
            tool_use_id: Some("toolu_01WspFixAsk1".to_string()),
            parent_tool_use_id: None,
            input: json!({ "file_path": "/tmp/probe/out.txt", "content": "hi" }).to_string(),
            detail: Some("out.txt".to_string()),
            options: vec![
                option(PERMISSION_ALLOW, "Allow", PermissionEffect::Allow, None),
                option(PERMISSION_DENY, "Deny", PermissionEffect::Deny, None),
                option("mode:acceptEdits", "acceptEdits", PermissionEffect::Mode, Some("acceptEdits")),
            ],
        }
    );
}

// TS: permissions.test.ts:49
#[test]
fn allow_and_deny_ride_with_no_suggestion_and_an_unusable_suggestion_is_dropped() {
    let bare = ids(&[PERMISSION_ALLOW, PERMISSION_DENY]);
    assert_eq!(option_ids(&ask(&[("permission_suggestions", Some(json!([])))])), bare);
    assert_eq!(option_ids(&ask(&[("permission_suggestions", None)])), bare);
    let odd = ask(&[(
        "permission_suggestions",
        Some(
            json!([{ "type": "addRules", "rules": [] }, { "type": "setMode" }, { "type": "setMode", "mode": "acceptEdits" }, { "type": "setMode", "mode": "acceptEdits" }]),
        ),
    )]);
    assert_eq!(option_ids(&odd), ids(&[PERMISSION_ALLOW, PERMISSION_DENY, "mode:acceptEdits"]));
}

// TS: permissions.test.ts:56
#[test]
fn a_detail_the_binary_named_as_nothing_is_left_out() {
    assert_eq!(ask(&[("description", Some(json!("")))]).detail, None);
    assert_eq!(ask(&[("description", None)]).detail, None);
}

// TS: permissions.test.ts:61
#[test]
fn a_withdrawal_is_read_and_an_ordinary_event_reads_as_nothing() {
    assert_eq!(
        control_line(&obj(json!({ "type": "control_cancel_request", "request_id": REQUEST_ID }))),
        Some(ControlLine::Cancel { request_id: REQUEST_ID.to_string() })
    );
    assert_eq!(control_line(&obj(json!({ "type": "assistant", "message": {} }))), None);
    assert_eq!(control_line(&obj(json!({ "type": "control_request" }))), None);
    assert_eq!(control_line(&obj(json!({ "type": "control_cancel_request" }))), None);
}

// TS: permissions.test.ts:68
#[test]
fn the_clis_answer_to_a_request_is_read_with_the_words_of_a_refusal() {
    assert_eq!(
        control_line(&obj(json!({ "type": "control_response", "response": { "subtype": "success", "request_id": "set_1" } }))),
        Some(ControlLine::Answer { request_id: "set_1".to_string(), error: None })
    );
    assert_eq!(
        control_line(&obj(
            json!({ "type": "control_response", "response": { "subtype": "error", "request_id": "set_1", "error": "no such mode" } })
        )),
        Some(ControlLine::Answer { request_id: "set_1".to_string(), error: Some("no such mode".to_string()) })
    );
    // A refusal with no words of its own is still a refusal, never a silent success.
    let bare = control_line(&obj(json!({ "type": "control_response", "response": { "subtype": "error", "request_id": "set_1" } })));
    assert!(matches!(&bare, Some(ControlLine::Answer { error: Some(_), .. })), "{bare:?}");
    assert_eq!(control_line(&obj(json!({ "type": "control_response", "response": {} }))), None);
}

// TS: permissions.test.ts:80
#[test]
fn a_request_of_another_subtype_is_one_to_refuse() {
    assert_eq!(
        control_line(&ask_line(&[("subtype", Some(json!("hook_callback")))])),
        Some(ControlLine::Unknown { request_id: REQUEST_ID.to_string(), subtype: "hook_callback".to_string() })
    );
    assert_eq!(
        control_line(&ask_line(&[("tool_name", None)])),
        Some(ControlLine::Unknown { request_id: REQUEST_ID.to_string(), subtype: "can_use_tool".to_string() })
    );
}

// TS: permissions.test.ts:87
#[test]
fn answer_allows_the_call_with_the_input_the_prompt_carried_unchanged() {
    let line = control_answer_line(&ask(&[]), PERMISSION_ALLOW, "unused").unwrap();
    let parsed: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        parsed,
        json!({
            "type": "control_response",
            "response": { "subtype": "success", "request_id": REQUEST_ID, "response": { "behavior": "allow", "updatedInput": { "file_path": "/tmp/probe/out.txt", "content": "hi" } } },
        })
    );
}

// TS: permissions.test.ts:94
#[test]
fn answer_refuses_the_call_with_the_words_the_agent_reads_as_its_result() {
    let line = control_answer_line(&ask(&[]), PERMISSION_DENY, "the person denied this in the chat").unwrap();
    let parsed: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        parsed,
        json!({
            "type": "control_response",
            "response": { "subtype": "success", "request_id": REQUEST_ID, "response": { "behavior": "deny", "message": "the person denied this in the chat" } },
        })
    );
}

// TS: permissions.test.ts:101
#[test]
fn a_mode_option_allows_the_call_and_carries_the_update_that_stops_the_asking() {
    let line = control_answer_line(&ask(&[]), "mode:acceptEdits", "unused").unwrap();
    let parsed: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        parsed,
        json!({
            "type": "control_response",
            "response": {
                "subtype": "success",
                "request_id": REQUEST_ID,
                "response": {
                    "behavior": "allow",
                    "updatedInput": { "file_path": "/tmp/probe/out.txt", "content": "hi" },
                    "updatedPermissions": [{ "type": "setMode", "mode": "acceptEdits", "destination": "session" }],
                },
            },
        })
    );
}

// TS: permissions.test.ts:116
#[test]
fn an_answer_typed_in_other_is_written_into_the_input_and_a_question_with_no_choice_asks_no_consent() {
    let questions = json!([
        { "question": "Which one?", "header": "Pick", "options": [{ "label": "A", "description": "" }, { "label": "B", "description": "" }], "multiSelect": false },
        { "question": "A name?", "header": "Name", "options": [], "multiSelect": false },
    ]);
    let asked = ask(&[("tool_name", Some(json!("AskUserQuestion"))), ("input", Some(json!({ "questions": questions })))]);
    let picked = picked_option_id(&[other_option_id(0, "neither, C"), other_option_id(1, "Webby")]);
    let line = control_answer_line(&asked, &picked, "unused").unwrap();
    assert_eq!(
        answer_response(&line),
        json!({ "behavior": "allow", "updatedInput": { "questions": questions, "answers": { "Which one?": "neither, C", "A name?": "Webby" } } })
    );
    let words_only = ask(&[("tool_name", Some(json!("AskUserQuestion"))), ("input", Some(json!({ "questions": [questions[1].clone()] })))]);
    assert_eq!(words_only.options, Vec::<PermissionOption>::new());
    let line = control_answer_line(&words_only, &other_option_id(0, "Webby"), "unused").unwrap();
    assert_eq!(answer_response(&line)["updatedInput"]["answers"], json!({ "A name?": "Webby" }));
}

// TS: permissions.test.ts:130
#[test]
fn answer_refuses_an_option_the_prompt_never_offered() {
    let refused = control_answer_line(&ask(&[]), "mode:bypassPermissions", "unused");
    match refused {
        Err(e) => assert!(e.0.contains("not an option"), "{}", e.0),
        Ok(line) => panic!("expected a refusal, got {line}"),
    }
}

// TS: permissions.test.ts:136
#[test]
fn error_line_tells_the_binary_this_host_answers_no_such_request() {
    let parsed: Value = serde_json::from_str(&control_error_line(REQUEST_ID, "hook_callback")).unwrap();
    assert_eq!(
        parsed,
        json!({
            "type": "control_response",
            "response": { "subtype": "error", "request_id": REQUEST_ID, "error": "wsp answers no hook_callback control request" },
        })
    );
}

// TS: permissions.test.ts:145
#[test]
fn set_mode_line_asks_for_the_mode_by_its_own_slug_under_the_id_its_answer_will_carry() {
    let parsed: Value = serde_json::from_str(&set_mode_line("set_1", "bypassPermissions")).unwrap();
    assert_eq!(
        parsed,
        json!({
            "type": "control_request",
            "request_id": "set_1",
            "request": { "subtype": "set_permission_mode", "mode": "bypassPermissions" },
        })
    );
}

// ---------------------------------------------------------------------------------------------------------------
// steers.test.ts
// ---------------------------------------------------------------------------------------------------------------

const SID: &str = "e16ed170-8257-4668-879e-fe836341633c";

// TS: steers.test.ts:9
#[test]
fn a_runs_channel_read_back_names_the_steers_of_its_last_turn_alone() {
    let a = "11111111-1111-4111-8111-111111111111";
    let b = "22222222-2222-4222-8222-222222222222";
    let taken = vec![
        user_message_line("open 1", SID, &[], None),
        user_message_line("x", SID, &[], Some(a)),
        user_message_line("open 2", SID, &[], None),
        user_message_line("y", SID, &[], Some(b)),
        "not json".to_string(),
        json!({ "type": "control_request" }).to_string(),
    ];
    assert_eq!(steered_ids(&taken), vec![b.to_string()]);
}

// TS: steers.test.ts:15
#[test]
fn a_line_written_again_under_a_steers_id_is_that_one_steer() {
    let a = "11111111-1111-4111-8111-111111111111";
    let taken = vec![
        user_message_line("open", SID, &[], None),
        user_message_line("x", SID, &[], Some(a)),
        user_message_line("push to main", SID, &[], Some(a)),
    ];
    assert_eq!(steered_ids(&taken), vec![a.to_string()]);
}

// ---------------------------------------------------------------------------------------------------------------
// shell-cwd.test.ts
// ---------------------------------------------------------------------------------------------------------------

const ROOT: &str = "/root";

fn bash(input: Value, current: &str) -> Option<String> {
    shell_cwd_after(Some("Bash"), Some(&input), current, ROOT)
}

fn bash_cmd(command: &str) -> Option<String> {
    bash(json!({ "command": command }), ROOT)
}

fn file_tool(tool: Option<&str>, path: &str, current: &str) -> Option<String> {
    shell_cwd_after(tool, Some(&json!({ "file_path": path })), current, ROOT)
}

// TS: shell-cwd.test.ts:10
#[test]
fn follows_a_bash_command_that_begins_with_cd_to_an_absolute_path() {
    assert_eq!(bash_cmd("cd /root/2048"), Some("/root/2048".to_string()));
    assert_eq!(bash_cmd("cd /root/2048 && npm test"), Some("/root/2048".to_string()));
    assert_eq!(bash_cmd("cd /root/2048; ls"), Some("/root/2048".to_string()));
    assert_eq!(bash_cmd("  cd '/root/my game' && ls"), Some("/root/my game".to_string()));
    assert_eq!(bash_cmd("cd \"/root/2048/\" \n ls"), Some("/root/2048".to_string()));
    assert_eq!(bash_cmd("cd /root/2048/../tetris"), Some("/root/tetris".to_string()));
    assert_eq!(bash_cmd("cd /tmp"), Some("/tmp".to_string()));
}

// TS: shell-cwd.test.ts:20
#[test]
fn ignores_a_cd_that_is_relative_not_first_or_not_a_cd() {
    assert_eq!(bash_cmd("cd 2048"), None);
    assert_eq!(bash_cmd("cd ~/2048"), None);
    assert_eq!(bash_cmd("cd -"), None);
    assert_eq!(bash_cmd("ls && cd /root/2048"), None);
    assert_eq!(bash_cmd("cdx /root/2048"), None);
    assert_eq!(bash_cmd("echo cd /root/2048"), None);
    assert_eq!(bash(json!({ "command": "cd /root/2048" }), "/root/2048"), None);
    assert_eq!(bash(json!({}), ROOT), None);
    assert_eq!(bash(Value::Null, ROOT), None);
}

// TS: shell-cwd.test.ts:32
#[test]
fn ignores_a_cd_whose_path_the_shell_would_still_expand() {
    for command in [
        "cd /root/$PROJ && ls",
        "cd /root/${PROJ}/src",
        "cd /root/*/src",
        "cd /root/204?",
        "cd /root/[ab]/src",
        "cd /root/{a,b}",
        "cd /root/`basename x`",
        "cd /root/$(ls | head -1)",
        "cd /root/~x",
        "cd \"$(pwd)/x\"",
        "cd \"/root/$PROJ\" && ls",
        "cd \"/root/`id -un`\"",
    ] {
        assert_eq!(bash_cmd(command), None, "{command}");
    }
    assert_eq!(bash_cmd("cd '/root/$literal'"), Some("/root/$literal".to_string()));
}

// TS: shell-cwd.test.ts:48
#[test]
fn takes_the_parent_of_a_write_or_edit_path_under_the_harness_folder_while_the_shell_is_still_there() {
    let write = json!({ "file_path": "/root/2048/index.html", "content": "" });
    assert_eq!(shell_cwd_after(Some("Write"), Some(&write), ROOT, ROOT), Some("/root/2048".to_string()));
    assert_eq!(file_tool(Some("Edit"), "/root/2048/src/game.js", ROOT), Some("/root/2048/src".to_string()));
    assert_eq!(file_tool(Some("Write"), "/root/notes.md", ROOT), None);
    assert_eq!(file_tool(Some("Write"), "/etc/hosts", ROOT), None);
    assert_eq!(file_tool(Some("Write"), "relative/x.ts", ROOT), None);
    assert_eq!(file_tool(Some("Read"), "/root/2048/index.html", ROOT), None);
    assert_eq!(file_tool(None, "/root/2048/index.html", ROOT), None);
}

// TS: shell-cwd.test.ts:58
#[test]
fn never_drills_deeper_into_the_folder_already_followed_but_picks_up_a_folder_beside_it() {
    assert_eq!(file_tool(Some("Write"), "/root/2048/src/game.js", "/root/2048"), None);
    assert_eq!(file_tool(Some("Edit"), "/root/2048/index.html", "/root/2048"), None);
    assert_eq!(file_tool(Some("Write"), "/root/tetris/index.html", "/root/2048"), Some("/root/tetris".to_string()));
    assert_eq!(file_tool(Some("Write"), "/root/2048-old/x", "/root/2048"), Some("/root/2048-old".to_string()));
    assert_eq!(file_tool(Some("Write"), "/tmp/x", "/root/2048"), None);
}

// ---------------------------------------------------------------------------------------------------------------
// tool-results.test.ts
// ---------------------------------------------------------------------------------------------------------------

const TOOL_SID: &str = "sess-tool-results";

fn line(o: Value) -> String {
    let mut all = obj(json!({ "session_id": TOOL_SID, "parent_tool_use_id": null }));
    all.extend(obj(o));
    Value::Object(all).to_string()
}

fn call(id: &str, name: &str, input: Value) -> String {
    line(
        json!({ "type": "assistant", "message": { "role": "assistant", "content": [{ "type": "tool_use", "id": id, "name": name, "input": input }] } }),
    )
}

fn answer(id: &str, content: &str, is_error: Option<bool>, result: Value) -> String {
    let mut block = obj(json!({ "tool_use_id": id, "type": "tool_result", "content": content }));
    if let Some(is_error) = is_error {
        block.insert("is_error".to_string(), json!(is_error));
    }
    line(json!({ "type": "user", "message": { "role": "user", "content": [Value::Object(block)] }, "tool_use_result": result }))
}

/// What `seq 1 n` prints.
fn seq(n: usize) -> String {
    (1..=n).map(|i| format!("{i}\n")).collect()
}

fn big() -> String {
    seq(40000)
}

fn lines() -> Vec<String> {
    let big = big();
    let preview = &big[..2000];
    let stdout_kept = &big[..30000];
    vec![
        line(json!({ "type": "system", "subtype": "init", "cwd": "/root/lab", "tools": ["Bash", "Edit", "Write"], "model": "claude-haiku-5-5" })),
        call("t_fail", "Bash", json!({ "command": "sleep 1; echo hello; echo oops >&2; exit 3" })),
        answer("t_fail", "Exit code 3\nhello\noops", Some(true), json!("Error: Exit code 3\nhello\noops")),
        call("t_seq", "Bash", json!({ "command": "seq 1 5" })),
        answer("t_seq", "1\n2\n3\n4\n5", Some(false), json!({ "stdout": "1\n2\n3\n4\n5", "stderr": "", "interrupted": false, "isImage": false, "noOutputExpected": false })),
        call("t_grep", "Bash", json!({ "command": "grep zzz f.txt" })),
        answer(
            "t_grep",
            "(Bash completed with no output)",
            Some(false),
            json!({ "stdout": "", "stderr": "", "interrupted": false, "isImage": false, "returnCodeInterpretation": "No matches found", "noOutputExpected": false }),
        ),
        call("t_big", "Bash", json!({ "command": "seq 1 40000" })),
        answer(
            "t_big",
            &format!("<persisted-output>\nOutput too large (223.5KB). Full output saved to: /root/.claude/projects/-root-lab/s/tool-results/b1.txt\n\nPreview (first 2KB):\n{preview}\n...\n</persisted-output>"),
            Some(false),
            json!({
                "stdout": stdout_kept,
                "stderr": "",
                "interrupted": false,
                "isImage": false,
                "noOutputExpected": false,
                "persistedOutputPath": "/root/.claude/projects/-root-lab/s/tool-results/b1.txt",
                "persistedOutputSize": 228894,
            }),
        ),
        call("t_edit", "Edit", json!({ "replace_all": false, "file_path": "/root/lab/f.txt", "old_string": "beta", "new_string": "BETA" })),
        answer(
            "t_edit",
            "The file /root/lab/f.txt has been updated successfully.",
            None,
            json!({
                "filePath": "/root/lab/f.txt",
                "oldString": "beta",
                "newString": "BETA",
                "originalFile": "alpha\nbeta\ngamma\n",
                "structuredPatch": [{ "oldStart": 1, "oldLines": 3, "newStart": 1, "newLines": 3, "lines": [" alpha", "-beta", "+BETA", " gamma"] }],
                "userModified": false,
                "replaceAll": false,
                "contentNotInModelContext": true,
            }),
        ),
        call("t_over", "Write", json!({ "file_path": "/root/lab/f.txt", "content": "one\ntwo\nthree\n" })),
        answer(
            "t_over",
            "The file /root/lab/f.txt has been updated successfully.",
            None,
            json!({
                "type": "update",
                "filePath": "/root/lab/f.txt",
                "content": "one\ntwo\nthree\n",
                "structuredPatch": [{ "oldStart": 1, "oldLines": 3, "newStart": 1, "newLines": 3, "lines": ["-alpha", "-BETA", "-gamma", "+one", "+two", "+three"] }],
                "originalFile": "alpha\nBETA\ngamma\n",
                "userModified": false,
            }),
        ),
        call("t_new", "Write", json!({ "file_path": "/root/lab/new.txt", "content": "hi\n" })),
        answer(
            "t_new",
            "File created successfully at: /root/lab/new.txt",
            None,
            json!({ "type": "create", "filePath": "/root/lab/new.txt", "content": "hi\n", "structuredPatch": [], "originalFile": null, "userModified": false }),
        ),
        line(json!({ "type": "result", "subtype": "success", "is_error": false, "duration_ms": 9000, "result": "Done.", "num_turns": 8 })),
    ]
}

/// A turn over `lines`, and the tool_result delta of each tool call, by id (the last wins, as a Map built from
/// entries does).
async fn results(lines: Vec<String>) -> HashMap<String, TurnDelta> {
    let (events, on_event) = collect();
    let exec = FakeExec::scripted(lines, StreamOpts::default());
    let adapter = create_claude_adapter(AdapterDeps::new(exec, "/root/.claude-cfg")).unwrap();
    let session = adapter.start(HarnessStartOptions { prompt: "go".to_string(), on_event: Some(on_event), ..Default::default() }).unwrap();
    session.finished().await;
    let events = events.lock().unwrap().clone();
    let mut by_id = HashMap::new();
    for delta in claude_exec::deltas(&events) {
        if delta.kind == DeltaKind::ToolResult {
            if let Some(id) = delta.tool_use_id.clone() {
                by_id.insert(id, delta);
            }
        }
    }
    by_id
}

fn hunk(old_start: i64, old_lines: i64, new_start: i64, new_lines: i64, lines: &[&str]) -> PatchHunk {
    PatchHunk {
        old_start: old_start.into(),
        old_lines: old_lines.into(),
        new_start: new_start.into(),
        new_lines: new_lines.into(),
        lines: lines.iter().map(|s| s.to_string()).collect(),
    }
}

fn patch(path: &str, hunks: Vec<PatchHunk>) -> Vec<FilePatch> {
    vec![FilePatch { path: path.to_string(), hunks, moved_to: None }]
}

fn delta<'a>(r: &'a HashMap<String, TurnDelta>, id: &str) -> &'a TurnDelta {
    r.get(id).unwrap_or_else(|| panic!("no tool_result delta for {id}"))
}

// TS: tool-results.test.ts:88
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failing_commands_exit_code_is_read_off_the_exit_code_line_its_error_opens_with() {
    tokio::task::LocalSet::new().run_until(a_failing_commands_exit_code_is_read_off_the_exit_code_line_its_error_opens_with_body()).await
}

async fn a_failing_commands_exit_code_is_read_off_the_exit_code_line_its_error_opens_with_body() {
    let r = results(lines()).await;
    let fail = delta(&r, "t_fail");
    assert_eq!(fail.is_error, Some(true));
    assert_eq!(fail.exit_code, Some(3));
    assert_eq!(fail.text, "Exit code 3\nhello\noops");
}

// TS: tool-results.test.ts:93
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_clean_command_names_no_code() {
    tokio::task::LocalSet::new().run_until(a_clean_command_names_no_code_body()).await
}

async fn a_clean_command_names_no_code_body() {
    let r = results(lines()).await;
    assert_eq!(delta(&r, "t_seq").exit_code, None);
    assert_eq!(delta(&r, "t_grep").exit_code, None);
    assert_eq!(delta(&r, "t_grep").text, "(Bash completed with no output)");
    assert_eq!(delta(&r, "t_seq").text, "1\n2\n3\n4\n5");
    assert_eq!(delta(&r, "t_seq").bytes, None);
}

// TS: tool-results.test.ts:101
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_output_the_cli_kept_of_one_too_large_for_the_model_comes_with_the_bytes_of_the_whole() {
    tokio::task::LocalSet::new()
        .run_until(the_output_the_cli_kept_of_one_too_large_for_the_model_comes_with_the_bytes_of_the_whole_body())
        .await
}

async fn the_output_the_cli_kept_of_one_too_large_for_the_model_comes_with_the_bytes_of_the_whole_body() {
    let r = results(lines()).await;
    let big_delta = delta(&r, "t_big");
    let big = big();
    assert_eq!(big_delta.text, &big[..30000]);
    assert_eq!(big_delta.bytes, Some(228894.0));
    assert_eq!(big_delta.bytes, Some(big.len() as f64));
}

// TS: tool-results.test.ts:108
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_edits_structured_patch_is_the_files_hunks() {
    tokio::task::LocalSet::new().run_until(an_edits_structured_patch_is_the_files_hunks_body()).await
}

async fn an_edits_structured_patch_is_the_files_hunks_body() {
    let r = results(lines()).await;
    assert_eq!(delta(&r, "t_edit").patch, Some(patch("/root/lab/f.txt", vec![hunk(1, 3, 1, 3, &[" alpha", "-beta", "+BETA", " gamma"])])));
    assert_eq!(
        delta(&r, "t_over").patch,
        Some(patch("/root/lab/f.txt", vec![hunk(1, 3, 1, 3, &["-alpha", "-BETA", "-gamma", "+one", "+two", "+three"])]))
    );
}

// TS: tool-results.test.ts:114
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_file_with_an_empty_structured_patch_is_one_hunk_of_every_line_added() {
    tokio::task::LocalSet::new().run_until(a_new_file_with_an_empty_structured_patch_is_one_hunk_of_every_line_added_body()).await
}

async fn a_new_file_with_an_empty_structured_patch_is_one_hunk_of_every_line_added_body() {
    let r = results(lines()).await;
    assert_eq!(delta(&r, "t_new").patch, Some(patch("/root/lab/new.txt", vec![hunk(0, 0, 1, 1, &["+hi"])])));
}

// TS: tool-results.test.ts:119
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_line_that_answers_two_calls_reads_no_tool_use_result() {
    tokio::task::LocalSet::new().run_until(a_line_that_answers_two_calls_reads_no_tool_use_result_body()).await
}

async fn a_line_that_answers_two_calls_reads_no_tool_use_result_body() {
    let two = line(json!({
        "type": "user",
        "message": { "role": "user", "content": [
            { "tool_use_id": "t_a", "type": "tool_result", "content": "Exit code 2", "is_error": true },
            { "tool_use_id": "t_b", "type": "tool_result", "content": "Exit code 2", "is_error": true },
        ] },
        "tool_use_result": "Error: Exit code 2",
    }));
    let all = lines();
    let r = results(vec![
        all[0].clone(),
        call("t_a", "Bash", json!({ "command": "a" })),
        call("t_b", "Bash", json!({ "command": "b" })),
        two,
        all.last().unwrap().clone(),
    ])
    .await;
    for id in ["t_a", "t_b"] {
        let d = delta(&r, id);
        assert_eq!(d.exit_code, None, "{id}");
        assert_eq!(d.is_error, Some(true), "{id}");
        assert_eq!(d.text, "Exit code 2", "{id}");
    }
}
