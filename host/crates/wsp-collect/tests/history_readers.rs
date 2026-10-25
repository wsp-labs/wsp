// SPDX-License-Identifier: AGPL-3.0-only
//! The session readers over fixtures shaped like each agent's own store. Port of the `Claude Code reader`, `Codex
//! reader`, `Hermes reader` and `weighing by project` blocks of packages/collect/test/history.test.ts.
mod common;

use common::fake_host::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_catalog::catalog::HistoryFormat;
use wsp_collect::history::claude::claude_session;
use wsp_collect::history::codex::CodexReader;
use wsp_collect::history::hermes::HermesReader;
use wsp_collect::history::tally::Count;
use wsp_collect::history::{history_reader, read_store, StoreOptions};

fn line(o: Value) -> String {
    o.to_string()
}

fn claude_line(session_id: &str, tools: &[(&str, Value)]) -> String {
    let content: Vec<Value> =
        tools.iter().map(|(name, input)| json!({ "type": "tool_use", "id": "toolu_1", "name": name, "input": input })).collect();
    line(
        json!({ "type": "assistant", "cwd": "/Users/dev/proj", "sessionId": session_id, "message": { "role": "assistant", "content": content } }),
    )
}

fn claude_line_no_cwd(session_id: &str, command: &str) -> String {
    line(
        json!({ "type": "assistant", "sessionId": session_id, "message": { "role": "assistant", "content": [{ "type": "tool_use", "id": "toolu_1", "name": "Bash", "input": { "command": command } }] } }),
    )
}

fn user_line(text: &str) -> String {
    line(
        json!({ "type": "user", "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "toolu_1", "content": text }] } }),
    )
}

/// `[...map]`: the entries in order.
fn entries(m: &IndexMap<String, Count>) -> Vec<(String, Count)> {
    m.iter().map(|(k, v)| (k.clone(), *v)).collect()
}

/// `(name, sessions, calls)` rows as the entries a map holds.
fn rows(r: &[(&str, u64, u64)]) -> Vec<(String, Count)> {
    r.iter().map(|(k, sessions, calls)| (k.to_string(), Count { sessions: *sessions, calls: *calls })).collect()
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn folders(f: &[&str]) -> Vec<String> {
    strings(f)
}

mod claude_code_reader {
    use super::*;

    #[test]
    fn keys_a_transcript_to_its_top_level_session_nested_and_cleared_ones_included() {
        assert_eq!(claude_session("-Users-dev-proj/abc.jsonl"), Some("abc".to_string()));
        assert_eq!(claude_session("-Users-dev-proj/abc/subagents/agent-1.jsonl"), Some("abc".to_string()));
        assert_eq!(claude_session("-Users-dev-proj/_cleared_sessions/old.jsonl"), Some("old".to_string()));
        assert_eq!(claude_session("-Users-dev-proj"), None);
    }

    #[test]
    fn counts_a_session_once_across_its_sub_agent_transcripts_and_keeps_names_and_count() {
        let host = fake_host(
            FakeLaptop::new()
                .file(
                    "~/.claude/projects/-Users-dev-proj/s1.jsonl",
                    [
                        user_line("export TOKEN=sk-ant-x-secret-value"),
                        claude_line(
                            "s1",
                            &[("Bash", json!({ "command": "gh pr list | head -3" })), ("Read", json!({ "file_path": "/x" }))],
                        ),
                        claude_line("s1", &[("Skill", json!({ "skill": "unslop" })), ("mcp__zoho-mail__zoho_list_emails", json!({}))]),
                        "not json at all".to_string(),
                    ]
                    .join("\n"),
                )
                .file(
                    "~/.claude/projects/-Users-dev-proj/s1/subagents/agent-a.jsonl",
                    claude_line("s1", &[("Bash", json!({ "command": "gh api repos/x && npm install -g agent-browser" }))]),
                )
                .file("~/.claude/projects/-Users-dev-other/s2.jsonl", claude_line("s2", &[("Bash", json!({ "command": "git status" }))]))
                .file("~/.claude/projects/-Users-dev-other/memory/MEMORY.md", "# notes"),
        );
        let usage =
            block_on(read_store(&host, history_reader(HistoryFormat::ClaudeJsonl), "/Users/dev/.claude/projects", StoreOptions::default()))
                .unwrap();
        assert_eq!(usage.sessions, 2);
        assert_eq!(usage.calls, 6);
        assert_eq!(entries(&usage.commands), rows(&[("gh", 1, 2), ("git", 1, 1), ("head", 1, 1), ("npm", 1, 1)]));
        assert_eq!(entries(&usage.installs), rows(&[("npm agent-browser", 1, 1)]));
        // gh ran twice in one session and is one tool; npm is Node's; the install names agent-browser.
        assert_eq!(entries(&usage.tools), rows(&[("gh", 1, 2), ("agent-browser", 1, 1), ("git", 1, 1), ("node", 1, 1)]));
        let kept = format!("{:?}", [entries(&usage.commands), entries(&usage.installs), entries(&usage.tools)].concat());
        assert!(!kept.contains("sk-ant-x"), "a secret from a tool result was kept: {kept}");
    }

    #[test]
    fn counts_a_session_once_when_two_project_folders_hold_its_files_by_the_folder_whos() {
        let at = |m: u32| format!("2026-09-06T10:{m:02}:00.000Z");
        let call = |session: &str, m: u32, cwd: &str, command: &str| {
            line(
                json!({ "type": "assistant", "cwd": cwd, "sessionId": session, "timestamp": at(m), "message": { "role": "assistant", "content": [{ "type": "tool_use", "id": "toolu_1", "name": "Bash", "input": { "command": command } }] } }),
            )
        };
        let home = "/Users/dev/wsp";
        let copy = "/private/tmp/wsp-test/export-dev/wsp";
        let host = fake_host(
            FakeLaptop::new()
                .file(
                    "~/.claude/projects/-Users-dev-wsp/s1.jsonl",
                    [call("s1", 1, home, "gh pr view"), call("s1", 2, home, "gh pr list"), call("s1", 50, home, "cargo build")].join("\n"),
                )
                .file("~/.claude/projects/-Users-dev-wsp/s1/subagents/agent-a.jsonl", call("s1", 3, home, "jq ."))
                .file(
                    "~/.claude/projects/-private-tmp-wsp-test-export-dev-wsp/s1.jsonl",
                    [call("s1", 1, copy, "gh pr view"), call("s1", 2, copy, "gh pr list")].join("\n"),
                )
                .file("~/.claude/projects/-private-tmp-wsp-test-export-dev-wsp/s1/subagents/agent-a.jsonl", call("s1", 3, copy, "jq ."))
                .file("~/.claude/projects/-private-tmp-wsp-test-export-dev-wsp/s2.jsonl", call("s2", 5, copy, "gh auth status")),
        );
        let usage =
            block_on(read_store(&host, history_reader(HistoryFormat::ClaudeJsonl), "/Users/dev/.claude/projects", StoreOptions::default()))
                .unwrap();
        assert_eq!(usage.sessions, 2);
        assert_eq!(usage.calls, 5);
        assert_eq!(entries(&usage.commands), rows(&[("gh", 2, 3), ("cargo", 1, 1), ("jq", 1, 1)]));
        let only = folders(&[home]);
        let in_home = block_on(read_store(
            &host,
            history_reader(HistoryFormat::ClaudeJsonl),
            "/Users/dev/.claude/projects",
            StoreOptions { folders: Some(&only), ..Default::default() },
        ))
        .unwrap();
        assert_eq!(in_home.sessions, 1);
        assert_eq!(in_home.calls, 4);
    }
}

mod codex_reader {
    use super::*;

    #[test]
    fn reads_exec_command_and_the_older_shell_array_out_of_a_rollouts_function_calls() {
        let rollout = [
            line(json!({ "type": "session_meta", "payload": { "id": "t1", "cwd": "/Users/dev/proj" } })),
            line(json!({ "type": "response_item", "payload": { "type": "function_call", "name": "exec_command", "arguments": json!({ "cmd": "cargo build && cargo test", "workdir": "/x" }).to_string() } })),
            line(json!({ "type": "response_item", "payload": { "type": "function_call", "name": "shell", "arguments": json!({ "command": ["bash", "-lc", "ls"] }).to_string() } })),
            line(json!({ "type": "response_item", "payload": { "type": "function_call", "name": "update_plan", "arguments": "{\"plan\":[]}" } })),
            line(json!({ "type": "response_item", "payload": { "type": "function_call_output", "output": "secret output" } })),
            line(json!({ "type": "event_msg", "payload": { "type": "token_count" } })),
        ]
        .join("\n");
        let host = fake_host(FakeLaptop::new().file("~/.codex/sessions/2026/06/01/rollout-2026-06-01T10-00-00-t1.jsonl", rollout));
        let usage = block_on(read_store(&host, &CodexReader, "/Users/dev/.codex/sessions", StoreOptions::default())).unwrap();
        assert_eq!(usage.sessions, 1);
        assert_eq!(usage.calls, 3);
        assert_eq!(entries(&usage.commands), rows(&[("cargo", 1, 2), ("bash", 1, 1)]));
        assert_eq!(entries(&usage.tools), rows(&[("rust", 1, 2)]));
    }
}

mod hermes_reader {
    use super::*;

    const DB: &str = "/Users/dev/.hermes/state.db";
    const QUERY: &str = "select session_id, tool_calls from messages where tool_calls is not null";
    const JOINED: &str = "select m.session_id, m.tool_calls, s.cwd from messages m left join sessions s on s.id = m.session_id where m.tool_calls is not null";

    fn call(session: &str, command: &str) -> Value {
        json!({ "session_id": session, "tool_calls": json!([{ "type": "function", "function": { "name": "terminal", "arguments": json!({ "command": command }).to_string() } }]).to_string() })
    }

    fn rows_json() -> String {
        json!([
            { "session_id": "h1", "tool_calls": json!([{ "type": "function", "function": { "name": "terminal", "arguments": json!({ "command": "docker ps", "timeout": 30 }).to_string() } }]).to_string() },
            { "session_id": "h1", "tool_calls": json!([{ "type": "function", "function": { "name": "skill_view", "arguments": { "name": "github" } } }, { "type": "function", "function": { "name": "browser_exec", "arguments": "{}" } }]).to_string() },
            { "session_id": "h2", "tool_calls": json!([{ "type": "function", "function": { "name": "terminal", "arguments": json!({ "command": "docker compose up" }).to_string() } }]).to_string() },
        ])
        .to_string()
    }

    #[test]
    fn asks_sqlite3_read_only_for_its_columns_and_the_folder_each_session_ran_in_and_re() {
        let host = fake_host(
            FakeLaptop::new().file("~/.hermes/state.db", 4096).exec(&format!("sqlite3 -readonly -json {DB} {JOINED}"), &rows_json()),
        );
        let usage = block_on(read_store(&host, &HermesReader, DB, StoreOptions::default())).unwrap();
        assert_eq!(usage.sessions, 2);
        assert_eq!(usage.calls, 4);
        assert_eq!(entries(&usage.commands), rows(&[("docker", 2, 2)]));
        assert_eq!(host.calls(), vec![format!("run sqlite3 -readonly -json {DB} {JOINED}")]);
    }

    #[test]
    fn falls_back_to_the_columns_alone_on_a_database_with_no_sessions_table_and_counts() {
        let host = fake_host(
            FakeLaptop::new().file("~/.hermes/state.db", 4096).exec(&format!("sqlite3 -readonly -json {DB} {QUERY}"), &rows_json()),
        );
        let usage = block_on(read_store(&host, &HermesReader, DB, StoreOptions::default())).unwrap();
        assert_eq!(usage.sessions, 2);
        assert_eq!(usage.calls, 4);
        assert_eq!(
            host.calls(),
            vec![format!("run sqlite3 -readonly -json {DB} {JOINED}"), format!("run sqlite3 -readonly -json {DB} {QUERY}")]
        );
        let with_cwd = |session: &str, command: &str, cwd: &str| {
            let mut row = call(session, command);
            row["cwd"] = json!(cwd);
            row
        };
        let known = fake_host(FakeLaptop::new().file("~/.hermes/state.db", 4096).exec(
            &format!("sqlite3 -readonly -json {DB} {JOINED}"),
            &json!([with_cwd("h1", "docker ps", "/Users/dev/proj"), with_cwd("h2", "go build", "/Users/dev/other")]).to_string(),
        ));
        let only = folders(&["/Users/dev/proj"]);
        let usage = block_on(read_store(&known, &HermesReader, DB, StoreOptions { folders: Some(&only), ..Default::default() })).unwrap();
        assert_eq!(entries(&usage.commands), rows(&[("docker", 1, 1)]));
    }

    #[test]
    fn reads_nothing_when_there_is_no_database_and_throws_when_the_database_is_there_bu() {
        let usage = block_on(read_store(&empty_host(), &HermesReader, DB, StoreOptions::default())).unwrap();
        assert_eq!(usage.sessions, 0);
        assert_eq!(usage.calls, 0);
        let there = fake_host(FakeLaptop::new().file("~/.hermes/state.db", 4096));
        let err = block_on(read_store(&there, &HermesReader, DB, StoreOptions::default())).expect_err("sqlite3 does not answer");
        assert!(err.message.contains("could not be read"), "{}", err.message);
    }
}

mod weighing_by_project {
    use super::*;

    const CLAUDE_ROOT: &str = "/Users/dev/.claude/projects";

    fn bash_line(cwd: &str, session: &str, command: &str) -> String {
        line(
            json!({ "type": "assistant", "cwd": cwd, "sessionId": session, "message": { "role": "assistant", "content": [{ "type": "tool_use", "id": "toolu_1", "name": "Bash", "input": { "command": command } }] } }),
        )
    }

    fn sorted_names(usage: &wsp_collect::history::tally::Usage) -> Vec<String> {
        let mut names: Vec<String> = usage.commands.keys().cloned().collect();
        names.sort();
        names
    }

    #[test]
    fn counts_only_the_sessions_that_ran_at_a_named_folder_or_under_it_whatever_the_sto() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.claude/projects/-Users-dev-proj/s1.jsonl", claude_line("s1", &[("Bash", json!({ "command": "pnpm test" }))]))
                .file("~/.claude/projects/-Users-dev-proj-web/s2.jsonl", bash_line("/Users/dev/proj/web", "s2", "gh pr view"))
                .file("~/.claude/projects/-Users-dev-other/s3.jsonl", bash_line("/Users/dev/other", "s3", "go build")),
        );
        let reader = history_reader(HistoryFormat::ClaudeJsonl);
        let all = block_on(read_store(&host, reader, CLAUDE_ROOT, StoreOptions::default())).unwrap();
        assert_eq!(sorted_names(&all), strings(&["gh", "go", "pnpm"]));
        let proj = folders(&["/Users/dev/proj"]);
        let one = block_on(read_store(&host, reader, CLAUDE_ROOT, StoreOptions { folders: Some(&proj), ..Default::default() })).unwrap();
        assert_eq!(sorted_names(&one), strings(&["gh", "pnpm"]));
        assert_eq!(one.sessions, 2);
        // A sibling folder that shares the prefix is not inside it.
        let pro = folders(&["/Users/dev/pro"]);
        let none = block_on(read_store(&host, reader, CLAUDE_ROOT, StoreOptions { folders: Some(&pro), ..Default::default() })).unwrap();
        assert_eq!(none.sessions, 0);
    }

    #[test]
    fn takes_a_codex_sessions_folder_from_the_session_meta_the_rollout_opens_with() {
        let rollout = |id: &str, cwd: &str, cmd: &str| {
            [
                line(json!({ "type": "session_meta", "payload": { "id": id, "cwd": cwd } })),
                line(json!({ "type": "response_item", "payload": { "type": "function_call", "name": "exec_command", "arguments": json!({ "cmd": cmd }).to_string() } })),
            ]
            .join("\n")
        };
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.codex/sessions/2026/06/01/rollout-a-t1.jsonl", rollout("t1", "/Users/dev/proj", "cargo build"))
                .file("~/.codex/sessions/2026/06/01/rollout-b-t2.jsonl", rollout("t2", "/Users/dev/other", "go build")),
        );
        let proj = folders(&["/Users/dev/proj"]);
        let usage = block_on(read_store(
            &host,
            &CodexReader,
            "/Users/dev/.codex/sessions",
            StoreOptions { folders: Some(&proj), ..Default::default() },
        ))
        .unwrap();
        assert_eq!(entries(&usage.commands), rows(&[("cargo", 1, 1)]));
    }

    #[test]
    fn counts_nothing_from_a_session_whose_store_never_recorded_a_folder() {
        let host = fake_host(FakeLaptop::new().file("~/.claude/projects/-Users-dev-proj/s1.jsonl", claude_line_no_cwd("s1", "pnpm test")));
        let reader = history_reader(HistoryFormat::ClaudeJsonl);
        let proj = folders(&["/Users/dev/proj"]);
        let weighed =
            block_on(read_store(&host, reader, CLAUDE_ROOT, StoreOptions { folders: Some(&proj), ..Default::default() })).unwrap();
        assert_eq!(weighed.sessions, 0);
        let all = block_on(read_store(&host, reader, CLAUDE_ROOT, StoreOptions::default())).unwrap();
        assert_eq!(all.sessions, 1);
    }
}
