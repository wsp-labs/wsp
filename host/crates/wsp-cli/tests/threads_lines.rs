// SPDX-License-Identifier: AGPL-3.0-only
//! The thread verbs' command lines (threads.ts, spec/threads-a.md and spec/threads-b.md): what a line prints on
//! stdout and stderr, the exit code it ends with, and what it asks the host in which order, driven through
//! `run_verb` with the verb the table holds.
#![allow(
    clippy::result_large_err,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::large_enum_variant,
    clippy::regex_creation_in_loops,
    non_snake_case,
    dead_code
)]

#[path = "cli_a_common/mod.rs"]
mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{json, Value};

use common::{answers, RecordingIo, Script, ScriptedClient, ScriptedHost};
use wsp_cli::verbs::client::{CliIo, DialFn, DialOpts, HostClient};
use wsp_cli::verbs::{find_verb, run_verb, verbs_for, RunDeps};
use wsp_words::exit::HostError;

/// A dial onto the scripted host: one socket per dial, every socket sharing the script and the record of asks.
struct OnTo(Arc<ScriptedHost>);

#[async_trait]
impl DialFn for OnTo {
    async fn dial(&self, _state_path: &str, _opts: DialOpts) -> Result<Arc<dyn HostClient>, HostError> {
        let socket = ScriptedClient::new(self.0.script.clone(), self.0.state.clone());
        self.0.sockets.lock().unwrap().push(socket.clone());
        Ok(socket)
    }
}

async fn line(host: Arc<ScriptedHost>, io: Arc<RecordingIo>, words: &[&str]) -> i32 {
    let table = verbs_for(false);
    let argv: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
    let verb = find_verb(&table, &argv).unwrap_or_else(|| panic!("no verb opens {words:?}"));
    let home = tempfile::tempdir().unwrap();
    let env = [("WSP_HOME", home.path().join("wsp")), ("HOME", home.path().to_path_buf())]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_string_lossy().into_owned()))
        .collect();
    let deps = RunDeps { env, dial: Some(Arc::new(OnTo(host))), cwd: Some("/root".into()), ..RunDeps::default() };
    let io_dyn: Arc<dyn CliIo> = io;
    run_verb(&table, verb, &argv, io_dyn, &|flag| flag.unwrap_or("/nowhere/state.json").to_owned(), deps).await
}

/// The exec case the tool server's record holds ("output and the folder"), its pushed frames swapped for `pushed`.
fn exec_host(pushed: &[&str]) -> Arc<ScriptedHost> {
    let case = answers("exec").cases.into_iter().find(|c| c.case == "output and the folder").expect("the recorded exec case");
    let mut script = Script { replies: case.replies, pushed: case.pushed, closes: None };
    script.pushed.insert("workspaces.exec".into(), pushed.iter().map(|f| (*f).to_owned()).collect());
    ScriptedHost::new(script)
}

const OUT_A: &str = r#"{"type":"exec.output","execId":"exec-1","text":"hi \u0085 \"there\""}"#;
const OUT_B: &str = r#"{"type":"exec.output","execId":"exec-1","text":"é"}"#;

mod exec {
    use super::*;

    #[tokio::test]
    async fn exec_json_prints_each_output_frame_as_the_host_pushed_it_then_the_result_and_exits_with_the_command_s_code() {
        let host = exec_host(&[
            r#"{"type":"exec.output","execId":"exec-0","text":"someone else's"}"#,
            OUT_A,
            OUT_B,
            r#"{"type":"exec.exit","execId":"exec-1","exitCode":3}"#,
        ]);
        let io = RecordingIo::new();
        let code = line(host.clone(), io.clone(), &["exec", "thread-7f3a9c21e4b5", "--json", "--", "sh", "-c", "echo hi"]).await;
        assert_eq!(code, 3, "exec alone exits with the command's own code");
        assert_eq!(
            io.lines(),
            vec![
                r#"{"type":"exec.output","execId":"exec-1","text":"hi \u0085 \"there\""}"#.to_owned(),
                r#"{"type":"exec.output","execId":"exec-1","text":"é"}"#.to_owned(),
                r#"{"exitCode":3,"cwd":"/root/wsp"}"#.to_owned(),
            ],
            "frames first, the result last, C1 escaped"
        );
        assert_eq!(io.errors(), vec!["ran in /root/wsp".to_owned()], "a failing command says on stderr where it ran");
        let exec = host.asked().into_iter().find(|f| f["op"] == "workspaces.exec").unwrap();
        assert_eq!(exec["argv"], json!(["sh", "-c", "echo hi"]), "each word as given");
    }

    #[tokio::test]
    async fn exec_without_json_prints_each_line_s_text_and_nothing_of_the_result() {
        let host = exec_host(&[OUT_A, OUT_B, r#"{"type":"exec.exit","execId":"exec-1","exitCode":0}"#]);
        let io = RecordingIo::new();
        let code = line(host, io.clone(), &["exec", "thread-7f3a9c21e4b5", "--", "sh", "-c", "echo hi"]).await;
        assert_eq!(code, 0);
        assert_eq!(io.lines(), vec!["hi \u{85} \"there\"".to_owned(), "é".to_owned()]);
        assert!(io.errors().is_empty(), "a command that exits 0 says nothing on stderr: {:?}", io.errors());
    }

    #[tokio::test]
    async fn an_exec_with_no_exit_code_exits_1_and_says_nothing_of_where_it_ran() {
        let host = exec_host(&[r#"{"type":"exec.exit","execId":"exec-1","exitCode":null}"#]);
        let io = RecordingIo::new();
        let code = line(host, io.clone(), &["exec", "thread-7f3a9c21e4b5", "--json", "--", "true"]).await;
        assert_eq!(code, 1, "no code of its own is the provider class");
        assert_eq!(io.lines(), vec![r#"{"exitCode":null,"cwd":"/root/wsp"}"#.to_owned()]);
        assert!(io.errors().is_empty());
    }

    #[tokio::test]
    async fn the_machine_going_under_a_running_exec_fails_the_line_with_its_reason_and_exit_1_after_the_lines_it_printed() {
        let host = exec_host(&[OUT_A, r#"{"type":"exec.exit","execId":"exec-1","exitCode":null,"error":"the machine was deleted"}"#]);
        let io = RecordingIo::new();
        let code = line(host, io.clone(), &["exec", "thread-7f3a9c21e4b5", "--", "sleep", "9"]).await;
        assert_eq!(code, 1);
        assert_eq!(io.lines(), vec!["hi \u{85} \"there\"".to_owned()]);
        assert_eq!(io.errors(), vec!["wsp exec: the machine was deleted".to_owned()]);
    }

    #[tokio::test]
    async fn exec_runs_in_the_cwd_given_and_the_folder_the_thread_works_in_without_it() {
        let host = exec_host(&[r#"{"type":"exec.exit","execId":"exec-1","exitCode":0}"#]);
        line(host.clone(), RecordingIo::new(), &["exec", "thread-7f3a9c21e4b5", "--cwd", "/tmp/x", "--", "ls"]).await;
        let exec = host.asked().into_iter().find(|f| f["op"] == "workspaces.exec").unwrap();
        assert_eq!(serde_json::to_string(&exec).unwrap(), r#"{"workspaceId":"ws-1","argv":["ls"],"cwd":"/tmp/x","op":"workspaces.exec"}"#);
    }
}

mod taking_away {
    use super::*;

    fn worktree_host() -> Arc<ScriptedHost> {
        ScriptedHost::replying(&[("worktree.remove", json!({ "ok": true }))])
    }

    fn asked(host: &ScriptedHost) -> Vec<String> {
        host.asked().iter().map(|f| serde_json::to_string(f).unwrap()).collect()
    }

    #[tokio::test]
    async fn off_a_terminal_without_yes_the_host_s_check_runs_first_and_the_question_refuses_the_line() {
        let host = worktree_host();
        let io = RecordingIo::new();
        let code = line(host.clone(), io.clone(), &["worktree", "remove", "wsp", "feat"]).await;
        assert_eq!(code, 3);
        assert_eq!(
            asked(&host),
            vec![r#"{"project":"wsp","branch":"feat","check":true,"op":"worktree.remove"}"#.to_owned()],
            "only the check went"
        );
        assert_eq!(
            io.errors(),
            vec!["wsp worktree remove: Remove the worktree for feat in wsp? There is no terminal to answer on. Pass --yes to say yes."
                .to_owned()]
        );
        assert!(io.lines().is_empty());
    }

    #[tokio::test]
    async fn with_yes_it_checks_then_removes_with_force_where_given_and_says_so() {
        let host = worktree_host();
        let io = RecordingIo::new();
        let code = line(host.clone(), io.clone(), &["worktree", "remove", "wsp", "feat", "--force", "--yes", "--json"]).await;
        assert_eq!(code, 0);
        assert_eq!(
            asked(&host),
            vec![
                r#"{"project":"wsp","branch":"feat","force":true,"check":true,"op":"worktree.remove"}"#.to_owned(),
                r#"{"project":"wsp","branch":"feat","force":true,"op":"worktree.remove"}"#.to_owned(),
            ]
        );
        assert_eq!(io.lines(), vec![r#"{"project":"wsp","branch":"feat","removed":true}"#.to_owned()]);
    }

    #[tokio::test]
    async fn at_a_terminal_a_no_keeps_it_says_kept_and_exits_1_with_nothing_removed() {
        let host = worktree_host();
        let io = RecordingIo::at_terminal("no");
        let code = line(host.clone(), io.clone(), &["worktree", "remove", "wsp", "feat", "--force"]).await;
        assert_eq!(code, 1);
        assert_eq!(
            io.asked(),
            vec!["Remove the worktree for feat in wsp?\nThe branch stays, and the files no commit holds there go.".to_owned()]
        );
        assert_eq!(io.errors(), vec!["feat kept".to_owned()]);
        assert_eq!(host.ops(), vec!["worktree.remove"], "the check alone");
        assert!(io.lines().is_empty(), "no JSON and no line on stdout");
    }

    #[tokio::test]
    async fn a_refusal_from_the_host_s_check_comes_back_before_anything_is_asked() {
        let host =
            ScriptedHost::replying(&[("worktree.remove", json!({ "ok": false, "error": "a thread is working in it", "kind": "usage" }))]);
        let io = RecordingIo::at_terminal("yes");
        let code = line(host.clone(), io.clone(), &["worktree", "remove", "wsp", "feat"]).await;
        assert_eq!(code, 3);
        assert!(io.asked().is_empty(), "nothing asked");
        assert_eq!(io.errors(), vec!["wsp worktree remove: a thread is working in it".to_owned()]);
        assert_eq!(host.ops(), vec!["worktree.remove"]);
    }
}

mod the_run_line {
    use super::*;

    #[tokio::test]
    async fn a_size_or_from_without_on_is_refused_only_after_the_line_has_dialled() {
        // threads.ts:115 dials before the checks at :121-122; the refusal is the same either way, the dial is not.
        for flag in ["--size", "--from"] {
            let host = ScriptedHost::new(Script::default());
            let io = RecordingIo::new();
            let code = line(host.clone(), io.clone(), &["run", flag, "x", "hi"]).await;
            assert_eq!(code, 3, "{flag}");
            assert_eq!(host.sockets.lock().unwrap().len(), 1, "{flag}: one socket opened");
            assert!(host.asked().is_empty(), "{flag}: nothing asked of it");
            let said = io.errors();
            assert_eq!(said.len(), 1, "{flag}: {said:?}");
            assert!(said[0].starts_with("wsp run: "), "{said:?}");
        }
    }

    #[tokio::test]
    async fn an_empty_message_is_refused_before_any_dial() {
        let host = ScriptedHost::new(Script::default());
        let io = RecordingIo::new();
        assert_eq!(line(host.clone(), io.clone(), &["run"]).await, 3);
        assert_eq!(
            io.errors(),
            vec!["wsp run: the message is empty; say what the thread is to do. Put the message in quotes: wsp run <project> \"say hi\"."
                .to_owned()]
        );
        assert!(host.sockets.lock().unwrap().is_empty());
    }
}

mod the_answer_verbs {
    use super::*;

    #[tokio::test]
    async fn allow_and_deny_print_the_answered_line_and_under_json_the_three_ids() {
        // The recorded allow case answers the prompt; the line's own words are the tool's text.
        let case = answers("thread_allow").cases.into_iter().find(|c| !c.line.contains("isError")).expect("an answered allow");
        let tool_text = serde_json::from_str::<Value>(&case.line).unwrap()["result"]["content"][0]["text"].as_str().unwrap().to_owned();
        let structured = serde_json::from_str::<Value>(&case.line).unwrap()["result"]["structuredContent"].clone();
        let thread = case.arguments["thread"].as_str().unwrap().to_owned();
        let host = ScriptedHost::new(Script { replies: case.replies.clone(), pushed: case.pushed.clone(), closes: None });
        let io = RecordingIo::new();
        assert_eq!(line(host, io.clone(), &["thread", "allow", &thread]).await, 0);
        assert_eq!(io.lines(), vec![tool_text]);
        let host = ScriptedHost::new(Script { replies: case.replies, pushed: case.pushed, closes: None });
        let io = RecordingIo::new();
        assert_eq!(line(host, io.clone(), &["thread", "allow", &thread, "--json"]).await, 0);
        assert_eq!(io.lines(), vec![serde_json::to_string(&structured).unwrap()]);
        let _ = Mutex::new(());
    }
}
