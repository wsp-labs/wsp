// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-codex/test/rpc.test.ts and shell-script.test.ts at a3b870a6e, plus the edges of spec rule 12 and 13
//! the TS does not cover.

use serde_json::{json, Map, Number, Value};
use wsp_adapters::codex::command::{AccessParams, ApprovalPolicy, Sandbox};
use wsp_adapters::codex::fields;
use wsp_adapters::codex::rpc::{
    decision_line, initialize_line, rate_limits_read_line, read_message, refuse_request_line, request as REQUEST, thread_fork_line,
    thread_resume_line, thread_revert_line, thread_start_line, turn_interrupt_line, turn_start_line, turn_steer_line, Decision, RequestId,
    RpcMessage, ThreadOptions, INITIALIZED_LINE,
};
use wsp_adapters::codex::shell_script::shell_script_of;

const THREAD: &str = "01a0e2a6-3a1e-7461-9bdd-a92cd79ee986";
const TURN: &str = "01a0e2a6-3a54-78c1-86d2-6a970789a480";

fn parsed(line: &str) -> Value {
    assert!(!line.contains('\n'), "{line}");
    serde_json::from_str(line).unwrap_or_else(|e| panic!("{line}: {e}"))
}

fn options(access: AccessParams) -> ThreadOptions {
    ThreadOptions { cwd: None, model: None, service_tier: None, access, config: None }
}

fn params(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

mod the_lines_wsp_writes_to_codex_app_server {
    use super::*;

    #[test]
    fn reads_the_plans_limits_with_the_banked_resets_count_alone_on_a_turn_and_with_eac() {
        assert_eq!(
            parsed(&rate_limits_read_line(false)),
            json!({ "id": REQUEST::RATE_LIMITS, "method": "account/rateLimits/read", "params": { "excludeResetCreditDetails": true } })
        );
        assert_eq!(parsed(&rate_limits_read_line(true)), json!({ "id": REQUEST::RATE_LIMITS, "method": "account/rateLimits/read", "params": {} }));
    }

    #[test]
    fn opens_with_initialize_under_wsps_own_name_then_the_initialized_notification_neit() {
        assert_eq!(
            parsed(&initialize_line()),
            json!({ "id": REQUEST::INITIALIZE, "method": "initialize", "params": { "clientInfo": { "name": "wsp", "title": "wsp", "version": "1" } } })
        );
        assert_eq!(parsed(INITIALIZED_LINE), json!({ "method": "initialized" }));
    }

    #[test]
    fn starts_a_thread_in_the_folder_on_the_model_in_the_sandbox_and_approval_policy_th() {
        let line = thread_start_line(&ThreadOptions {
            cwd: Some("/root/w".into()),
            model: Some("gpt-5.5".into()),
            ..options(AccessParams { sandbox: Sandbox::WorkspaceWrite, approval_policy: ApprovalPolicy::OnRequest })
        });
        assert_eq!(
            parsed(&line),
            json!({ "id": REQUEST::THREAD, "method": "thread/start", "params": { "cwd": "/root/w", "model": "gpt-5.5", "sandbox": "workspace-write", "approvalPolicy": "on-request" } })
        );
    }

    #[test]
    fn leaves_out_what_the_turn_did_not_name_so_the_servers_own_default_stands() {
        let line = thread_start_line(&options(AccessParams { sandbox: Sandbox::DangerFullAccess, approval_policy: ApprovalPolicy::Never }));
        assert_eq!(
            parsed(&line),
            json!({ "id": REQUEST::THREAD, "method": "thread/start", "params": { "sandbox": "danger-full-access", "approvalPolicy": "never" } })
        );
    }

    #[test]
    fn resumes_a_thread_by_the_id_the_server_announced_with_the_same_picks() {
        let line = thread_resume_line(&options(AccessParams { sandbox: Sandbox::ReadOnly, approval_policy: ApprovalPolicy::OnRequest }), THREAD);
        assert_eq!(
            parsed(&line),
            json!({ "id": REQUEST::THREAD, "method": "thread/resume", "params": { "threadId": THREAD, "excludeTurns": true, "sandbox": "read-only", "approvalPolicy": "on-request" } })
        );
    }

    #[test]
    fn forks_a_thread_ephemeral_and_read_only_asking_nobody_with_the_instructions_a_sid() {
        let line = thread_fork_line(THREAD, Some("/root/w"), None, "answer only");
        assert_eq!(
            parsed(&line),
            json!({
                "id": REQUEST::THREAD,
                "method": "thread/fork",
                "params": { "threadId": THREAD, "ephemeral": true, "excludeTurns": true, "sandbox": "read-only", "approvalPolicy": "never", "cwd": "/root/w", "developerInstructions": "answer only" }
            })
        );
    }

    #[test]
    fn starts_a_turn_with_the_text_first_and_each_image_as_a_local_path_and_the_effort() {
        let images = vec!["/root/.wsp/threads/thr_1/images/1.png".to_string()];
        let line = turn_start_line(THREAD, "what is this?", &images, Some("high"));
        assert_eq!(
            parsed(&line),
            json!({
                "id": REQUEST::TURN,
                "method": "turn/start",
                "params": { "threadId": THREAD, "input": [{ "type": "text", "text": "what is this?" }, { "type": "localImage", "path": "/root/.wsp/threads/thr_1/images/1.png" }], "effort": "high" }
            })
        );
    }

    #[test]
    fn a_prompt_with_quotes_newlines_and_a_heredoc_marker_travels_as_written_since_it() {
        let text = "Say \"hi\" and 'bye'.\nWSP_PROMPT_END\n$(date)";
        let line = turn_start_line(THREAD, text, &[], None);
        assert_eq!(parsed(&line)["params"]["input"][0]["text"], json!(text));
    }

    #[test]
    fn steers_the_running_turn_by_its_id_and_interrupts_it_by_the_same() {
        assert_eq!(
            parsed(&turn_steer_line("wsp-steer-1", THREAD, TURN, "stop at 12")),
            json!({ "id": "wsp-steer-1", "method": "turn/steer", "params": { "threadId": THREAD, "expectedTurnId": TURN, "input": [{ "type": "text", "text": "stop at 12" }] } })
        );
        assert_eq!(
            parsed(&turn_interrupt_line("wsp-interrupt-1", THREAD, TURN)),
            json!({ "id": "wsp-interrupt-1", "method": "turn/interrupt", "params": { "threadId": THREAD, "turnId": TURN } })
        );
    }

    #[test]
    fn answers_an_approval_with_accept_or_decline_alone_under_the_id_the_server_asked() {
        assert_eq!(parsed(&decision_line(&RequestId::Num(Number::from(0u64)), Decision::Accept)), json!({ "id": 0, "result": { "decision": "accept" } }));
        assert_eq!(parsed(&decision_line(&RequestId::Str("req-7".into()), Decision::Decline)), json!({ "id": "req-7", "result": { "decision": "decline" } }));
    }

    #[test]
    fn refuses_a_server_request_it_does_not_serve_with_an_error_so_the_server_stops() {
        assert_eq!(
            parsed(&refuse_request_line(&RequestId::Num(Number::from(4u64)), "item/tool/requestUserInput")),
            json!({ "id": 4, "error": { "code": -32601, "message": "wsp does not answer item/tool/requestUserInput" } })
        );
    }
}

mod read_message_cases {
    use super::*;

    #[test]
    fn tells_the_four_kinds_apart_a_response_an_error_a_notification_and_a_request_from() {
        assert_eq!(
            read_message(r#"{"id":"wsp-thread","result":{"thread":{"id":"t"}}}"#),
            Some(RpcMessage::Response { id: RequestId::Str("wsp-thread".into()), result: json!({ "thread": { "id": "t" } }) })
        );
        assert_eq!(
            read_message(r#"{"error":{"code":-32600,"message":"invalid thread id"},"id":"wsp-turn"}"#),
            Some(RpcMessage::Error { id: RequestId::Str("wsp-turn".into()), message: "invalid thread id".into() })
        );
        assert_eq!(
            read_message(r#"{"method":"turn/started","params":{"threadId":"t","turn":{"id":"u"}}}"#),
            Some(RpcMessage::Notification {
                method: "turn/started".into(),
                params: params(json!({ "threadId": "t", "turn": { "id": "u" } })),
                emitted_at_ms: None,
            })
        );
        assert_eq!(
            read_message(r#"{"id":0,"method":"item/fileChange/requestApproval","params":{"itemId":"i"}}"#),
            Some(RpcMessage::Request {
                id: RequestId::Num(Number::from(0u64)),
                method: "item/fileChange/requestApproval".into(),
                params: params(json!({ "itemId": "i" })),
            })
        );
    }

    // codex's stderr shares the log with its stdout
    #[test]
    fn reads_anything_else_as_no_message_codexs_stderr_shares_the_log_with_its_stdout() {
        assert_eq!(read_message("\u{1b}[2m2026-09-27T11:35:40Z\u{1b}[0m ERROR failed to connect"), None);
        assert_eq!(read_message("{not json"), None);
        assert_eq!(read_message(r#"{"neither":true}"#), None);
        assert_eq!(read_message(""), None);
    }

    #[test]
    fn a_response_whose_result_is_null_is_still_a_response_carrying_null() {
        assert_eq!(
            read_message(r#"{"id":"x","result":null}"#),
            Some(RpcMessage::Response { id: RequestId::Str("x".into()), result: Value::Null })
        );
    }
}

mod the_revert_line {
    use super::*;

    #[test]
    fn cuts_a_thread_before_one_turn_by_the_ids_the_server_gave_as_its_schema_spells_th() {
        assert_eq!(
            parsed(&thread_revert_line(THREAD, TURN)),
            json!({ "id": REQUEST::REVERT, "method": "thread/revert", "params": { "threadId": THREAD, "beforeTurnId": TURN } })
        );
    }
}

mod fields_readers {
    use super::*;

    #[test]
    fn str_accepts_the_empty_string() {
        assert_eq!(fields::str(Some(&json!(""))), Some(""));
    }

    #[test]
    fn count_refuses_a_non_number() {
        assert_eq!(fields::count(Some(&json!("5"))), None);
        assert_eq!(fields::count(Some(&Value::Null)), None);
        assert_eq!(fields::count(Some(&json!(true))), None);
        assert_eq!(fields::count(None), None);
    }
}

// Each wire string is what shlex 1.3.0's try_join printed for ["/bin/zsh", "-lc", <script>], run through cargo.
fn joined() -> Vec<(&'static str, &'static str)> {
    vec![
        ("/bin/zsh -lc 'touch hi.txt'", "touch hi.txt"),
        (r#"/bin/zsh -lc "echo \"it's done\" | wc -c""#, r#"echo "it's done" | wc -c"#),
        ("/bin/zsh -lc \"cat <<'EOF'\nline one\nEOF\"", "cat <<'EOF'\nline one\nEOF"),
        (r#"/bin/zsh -lc "printf '%s\\n' a b""#, r"printf '%s\n' a b"),
        ("/bin/zsh -lc 'echo $HOME && ls'", "echo $HOME && ls"),
        (r#"/bin/zsh -lc "grep -rn 'a\\b' src""#, r"grep -rn 'a\b' src"),
        (r#"/bin/zsh -lc "echo \"it's "'$HOME"'"#, r#"echo "it's $HOME""#),
    ]
}

mod shell_script_of_cases {
    use super::*;

    #[test]
    fn reads_the_script_out_of_every_quoting_shlex_writes() {
        for (wire, script) in joined() {
            assert_eq!(shell_script_of(wire), script, "{wire}");
        }
    }

    #[test]
    fn takes_c_and_any_shell_by_its_name_and_leaves_every_other_command_as_it_came() {
        assert_eq!(shell_script_of("bash -c 'ls -R'"), "ls -R");
        assert_eq!(shell_script_of("/usr/bin/sh -c ls"), "ls");
        assert_eq!(shell_script_of("python3 -c 'print(1)'"), "python3 -c 'print(1)'");
        assert_eq!(shell_script_of("ls -R src"), "ls -R src");
        assert_eq!(shell_script_of("/bin/zsh -lc 'unclosed"), "/bin/zsh -lc 'unclosed");
        assert_eq!(shell_script_of("/bin/zsh -lc 'a' extra"), "/bin/zsh -lc 'a' extra");
    }
}

/// Rule 2 and 4 of spec/codex_rpc.md: the lines are compared as text, since a parsed value's equality ignores key order
/// and the TS writes each object's keys in its literal's order (rule 4.26).
mod key_order_as_written {
    use super::*;
    use indexmap::IndexMap;
    use wsp_adapters::codex::rpc::{decision_line, initialize_line, thread_copy_line, thread_fork_line, thread_resume_line, thread_turns_list_line, Decision, RequestId, ServiceTier, ThreadOptions};

    fn full() -> ThreadOptions {
        let mut config = IndexMap::new();
        config.insert("mcp_servers.docs.env.TOKEN".to_string(), "x".to_string());
        ThreadOptions { cwd: Some("/w".into()), model: Some("gpt-5.5".into()), service_tier: Some(ServiceTier::Fast), access: AccessParams { sandbox: Sandbox::WorkspaceWrite, approval_policy: ApprovalPolicy::OnRequest }, config: Some(config) }
    }

    #[test]
    fn a_resume_line_writes_its_keys_in_the_order_the_ts_literal_does() {
        assert_eq!(
            thread_resume_line(&full(), "t1"),
            r#"{"id":"wsp-thread","method":"thread/resume","params":{"threadId":"t1","excludeTurns":true,"cwd":"/w","model":"gpt-5.5","serviceTier":"fast","sandbox":"workspace-write","approvalPolicy":"on-request","config":{"mcp_servers.docs.env.TOKEN":"x"}}}"#
        );
    }

    #[test]
    fn a_copy_line_puts_last_turn_id_after_the_thread_and_an_empty_config_nowhere() {
        let o = ThreadOptions { config: Some(IndexMap::new()), ..full() };
        assert_eq!(
            thread_copy_line(&o, "t1", Some("u9")),
            r#"{"id":"wsp-thread","method":"thread/fork","params":{"threadId":"t1","lastTurnId":"u9","ephemeral":false,"excludeTurns":true,"cwd":"/w","model":"gpt-5.5","serviceTier":"fast","sandbox":"workspace-write","approvalPolicy":"on-request"}}"#
        );
    }

    #[test]
    fn a_side_questions_fork_line_carries_its_instructions_last() {
        assert_eq!(
            thread_fork_line("t1", Some("/w"), Some("gpt-5.5"), "answer only"),
            r#"{"id":"wsp-thread","method":"thread/fork","params":{"threadId":"t1","ephemeral":true,"excludeTurns":true,"sandbox":"read-only","approvalPolicy":"never","cwd":"/w","model":"gpt-5.5","developerInstructions":"answer only"}}"#
        );
    }

    #[test]
    fn the_turns_page_writes_a_null_cursor_and_initialize_names_wsp_three_ways() {
        assert_eq!(thread_turns_list_line("t1", None, 3), r#"{"id":"wsp-turns","method":"thread/turns/list","params":{"threadId":"t1","cursor":null,"limit":3,"sortDirection":"desc","itemsView":"summary"}}"#);
        assert_eq!(initialize_line(), r#"{"id":"wsp-initialize","method":"initialize","params":{"clientInfo":{"name":"wsp","title":"wsp","version":"1"}}}"#);
    }

    #[test]
    fn a_decision_echoes_a_numeric_id_as_a_number_and_a_string_id_as_a_string() {
        assert_eq!(decision_line(&RequestId::Num(Number::from(0)), Decision::Accept), r#"{"id":0,"result":{"decision":"accept"}}"#);
        assert_eq!(decision_line(&RequestId::Str("req-7".into()), Decision::Decline), r#"{"id":"req-7","result":{"decision":"decline"}}"#);
        assert_eq!(RequestId::Num(Number::from(11)).js_string(), "11");
    }
}
