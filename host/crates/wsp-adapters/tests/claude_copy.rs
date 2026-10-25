// SPDX-License-Identifier: AGPL-3.0-only
//! Rust form of packages/adapter-claude/test/chain.test.ts and aside.test.ts at a3b870a6e: a session copied through
//! one message (the walk, the read and the write run by a shell on real files) and the side question asked on that
//! copy.

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

use futures::future::{LocalBoxFuture, FutureExt, Shared};
use futures::stream::{self, LocalBoxStream, StreamExt};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps, ClaudeAdapter};
use wsp_adapters::claude::aside::{aside_command, aside_hooks_line, aside_prompt, AsideCommandOptions, ASIDE_HOOKS_ID};
use wsp_adapters::claude::chain::{chain_read_command, chain_through, chain_write_command, copy_cleanup_command, copy_session, line_ranges, live_end, no_conversation_line, read_chain, ChainLine, CopyOptions, CopyThrough, LiveEnd};
use wsp_adapters::claude::landmines::{build_command, user_message_line, BaseEnv, BuildCommandOptions};
use wsp_adapters::pending::{aside_wall_line, McpServerSpec, ASIDE_TOOL_LINE};
use wsp_adapters::port::{agent_word, AgentLaunch, AsideAnswer, AsideQuestion, Env, ExecFailure, ExecOptions, ExecSecret, ExecStream, ExecStreamFactory, ServerValues, WriteOutcome};

use claude_exec::{fixture_lines, json, Exit};

const SESSION: &str = "e16ed170-8257-4668-879e-fe836341633c";
const FORK: &str = "0b7f3a52-6c1d-4e8a-9f2b-3d4c5e6f7a8b";
const CONFIG: &str = "/root/.claude-cfg";

fn id(n: u32) -> String {
    format!("1a2b3c4d-0000-4aaa-8bbb-{n:012}")
}

fn msg(kind: &str, n: u32, parent: Option<u32>, text: &str) -> String {
    let mut m = Map::new();
    m.insert("parentUuid".into(), parent.map_or(Value::Null, |p| Value::String(id(p))));
    m.insert("isSidechain".into(), json!(false));
    m.insert("type".into(), json!(kind));
    if kind == "attachment" {
        m.insert("attachment".into(), json!({"type": "todo", "note": text}));
    } else {
        let mut message = Map::new();
        message.insert("role".into(), json!(kind));
        if kind == "assistant" {
            message.insert("id".into(), json!(format!("msg_{n}")));
        }
        message.insert("content".into(), json!([{"type": "text", "text": text}]));
        m.insert("message".into(), Value::Object(message));
    }
    m.insert("uuid".into(), json!(id(n)));
    m.insert("sessionId".into(), json!(SESSION));
    Value::Object(m).to_string()
}

fn note(kind: &str, leaf: Option<u32>) -> String {
    let mut m = Map::new();
    m.insert("type".into(), json!(kind));
    if let Some(l) = leaf {
        m.insert("leafUuid".into(), json!(id(l)));
    }
    m.insert("sessionId".into(), json!(SESSION));
    Value::Object(m).to_string()
}

fn boundary(n: u32, logical: u32) -> String {
    json!({"parentUuid": null, "logicalParentUuid": id(logical), "type": "system", "subtype": "compact_boundary", "content": "Conversation compacted", "uuid": id(n), "sessionId": SESSION}).to_string()
}

/// Three turns teaching ALPHA, BETA and GAMMA, then a compaction, then a turn teaching DELTA.
fn compacted() -> Vec<String> {
    vec![
        note("queue-operation", None),
        msg("user", 1, None, "Remember ALPHA."),
        msg("attachment", 2, Some(1), "skills"),
        note("last-prompt", Some(2)),
        msg("assistant", 3, Some(2), "OK"),
        msg("attachment", 4, Some(3), "todo"),
        note("cost-state", None),
        msg("user", 5, Some(4), "Remember BETA."),
        msg("assistant", 6, Some(5), "OK"),
        note("last-prompt", Some(6)),
        msg("user", 7, Some(6), "Remember GAMMA."),
        msg("assistant", 8, Some(7), "OK"),
        note("mode", None),
        boundary(9, 8),
        msg("user", 10, Some(9), "This session is being continued from a previous conversation."),
        msg("attachment", 11, Some(10), "skills"),
        msg("user", 12, Some(11), "Remember DELTA."),
        msg("assistant", 13, Some(12), "OK"),
        note("last-prompt", Some(13)),
    ]
}

/// ALPHA and BETA, then a rewind to ALPHA's reply and a turn teaching OMEGA hung off it: BETA's turn stays in the file.
fn rewound() -> Vec<String> {
    vec![
        msg("user", 1, None, "Remember ALPHA."),
        msg("assistant", 2, Some(1), "OK"),
        msg("attachment", 3, Some(2), "todo"),
        msg("user", 4, Some(3), "Remember BETA."),
        msg("assistant", 5, Some(4), "OK"),
        note("last-prompt", Some(5)),
        msg("user", 6, Some(2), "Remember OMEGA."),
        msg("assistant", 7, Some(6), "OK"),
        note("last-prompt", Some(7)),
    ]
}

/// The read as the read command prints it, run over the file by hand: the count, then each message line by number.
fn read_of(file: &[String]) -> Vec<String> {
    let mut out = vec![file.len().to_string()];
    for (i, l) in file.iter().enumerate() {
        if l.contains("\"uuid\":\"") {
            out.push(format!("{}:{}", i + 1, l));
        }
    }
    out
}

fn texts_of(file: &[String], kept: &[u64]) -> Vec<String> {
    kept.iter()
        .map(|n| {
            let l: Value = serde_json::from_str(&file[*n as usize - 1]).unwrap();
            l["message"]["content"][0]["text"].as_str().or_else(|| l["attachment"]["note"].as_str()).or_else(|| l["subtype"].as_str()).unwrap_or("").to_string()
        })
        .collect()
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// A folder under the temp dir unique to the test, removed when it drops.
struct Root(PathBuf);

impl Root {
    fn new(name: &str) -> Root {
        let path = std::env::temp_dir().join(format!("wsp-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Root(path)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    out.sort();
    out
}

/// A config dir holding the thread's session file under a project folder: (config, project, the file's text).
fn set_up(root: &Root, project_name: &str, file: &[String]) -> (String, PathBuf, String) {
    let config = root.0.join("it's config");
    let project = config.join("projects").join(project_name);
    std::fs::create_dir_all(&project).unwrap();
    let source = format!("{}\n", file.join("\n"));
    std::fs::write(project.join(format!("{SESSION}.jsonl")), &source).unwrap();
    (config.to_str().unwrap().to_string(), project, source)
}

type Done = Shared<LocalBoxFuture<'static, (Vec<String>, Option<i32>)>>;

/// Runs each command in /bin/sh with its input lines on stdin, stdout and stderr read as lines, as a computer's exec
/// road hands them. The launches are noted in `commands` and `inputs`.
struct ShellExec {
    env: Vec<(String, String)>,
    commands: Mutex<Vec<String>>,
    inputs: Mutex<Vec<Option<Vec<String>>>>,
}

impl ShellExec {
    fn new(env: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(ShellExec { env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), commands: Default::default(), inputs: Default::default() })
    }
    fn commands(&self) -> Vec<String> {
        self.commands.lock().unwrap().clone()
    }
    fn inputs(&self) -> Vec<Option<Vec<String>>> {
        self.inputs.lock().unwrap().clone()
    }
}

struct ShellStream {
    stdin: Mutex<Option<ChildStdin>>,
    done: Done,
    pid: u32,
}

impl ExecStreamFactory for ShellExec {
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        self.commands.lock().unwrap().push(command.to_string());
        self.inputs.lock().unwrap().push(options.input.clone());
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .env_clear()
            .envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .envs(options.env.iter())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take();
        match &options.input {
            Some(input) => {
                for line in input {
                    if let Some(s) = stdin.as_mut() {
                        let _ = writeln!(s, "{line}");
                    }
                }
            }
            None => stdin = None,
        }
        let pid = child.id();
        let (tx, rx) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let out = child.wait_with_output().unwrap();
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            let lines = text.split('\n').filter(|l| !l.is_empty()).map(String::from).collect();
            let _ = tx.send((lines, out.status.code()));
        });
        let done: Done = rx.map(|r| r.unwrap_or_default()).boxed_local().shared();
        Arc::new(ShellStream { stdin: Mutex::new(stdin), done, pid })
    }
}

impl ExecStream for ShellStream {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        let done = self.done.clone();
        stream::once(async move { done.await.0 }).flat_map(|lines| stream::iter(lines.into_iter().map(Ok))).boxed_local()
    }
    fn teardown(&self) {
        let _ = Command::new("kill").arg("-TERM").arg(self.pid.to_string()).status();
    }
    fn kill(&self) {
        let _ = Command::new("kill").arg("-KILL").arg(self.pid.to_string()).status();
    }
    fn write(&self, _line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        async { Ok(WriteOutcome::Gone) }.boxed_local()
    }
    fn close_input(&self) {
        self.stdin.lock().unwrap().take();
    }
    fn exited(&self) -> wsp_adapters::port::SharedFuture<Option<i32>> {
        let done = self.done.clone();
        async move { done.await.1 }.boxed_local().shared()
    }
}

async fn collect_lines(stream: &Arc<dyn ExecStream>) -> Vec<String> {
    stream.lines().map(|l| l.unwrap()).collect().await
}

fn into_exec(exec: &Arc<ShellExec>) -> Arc<dyn ExecStreamFactory> {
    exec.clone()
}

fn copy_options(exec: &Arc<ShellExec>, config: &str, through: CopyThrough) -> CopyOptions {
    CopyOptions { exec: into_exec(exec), env: Env::new(), config_dir: config.to_string(), session: SESSION.to_string(), through, grace_ms: 5, wall_ms: None }
}

fn lines_of(file: &[String]) -> Vec<ChainLine> {
    read_chain(&read_of(file)).unwrap().lines
}

// ---------------------------------------------------------------------------------------------------------------
// chain.test.ts: the walk
// ---------------------------------------------------------------------------------------------------------------

// TS: chain.test.ts:68
#[test]
fn walk_fork_before_last_compaction_copies_back_to_start_and_nothing_after() {
    let file = compacted();
    let read = read_chain(&read_of(&file)).unwrap();
    assert_eq!(read.total, file.len() as u64);
    let kept = chain_through(&read.lines, &id(6)).unwrap();
    assert_eq!(texts_of(&file, &kept), strs(&["Remember ALPHA.", "skills", "OK", "todo", "Remember BETA.", "OK"]));
    assert_eq!(kept, vec![2, 3, 5, 6, 8, 9]);
}

// TS: chain.test.ts:76
#[test]
fn walk_fork_after_compaction_starts_at_the_boundary() {
    let file = compacted();
    let kept = chain_through(&lines_of(&file), &id(13)).unwrap();
    assert_eq!(texts_of(&file, &kept), strs(&["compact_boundary", "This session is being continued from a previous conversation.", "skills", "Remember DELTA.", "OK"]));
}

// TS: chain.test.ts:81
#[test]
fn walk_copies_the_live_branch_of_a_rewound_session() {
    let file = rewound();
    let lines = lines_of(&file);
    assert_eq!(texts_of(&file, &chain_through(&lines, &id(7)).unwrap()), strs(&["Remember ALPHA.", "OK", "Remember OMEGA.", "OK"]));
    assert_eq!(live_end(&lines), Some(LiveEnd { anchor: id(7), running: vec![] }));
    // A fork at the abandoned turn is still that turn's own chain.
    assert_eq!(texts_of(&file, &chain_through(&lines, &id(5)).unwrap()), strs(&["Remember ALPHA.", "OK", "todo", "Remember BETA.", "OK"]));
}

// TS: chain.test.ts:89
#[test]
fn walk_refuses_a_missing_anchor_and_a_broken_chain() {
    let file = rewound();
    let lines = lines_of(&file);
    assert_eq!(chain_through(&lines, &id(99)).unwrap_err().0, format!("the session holds no message {}", id(99)));
    let torn_file: Vec<String> = file.iter().enumerate().filter(|(i, _)| *i != 0).map(|(_, l)| l.clone()).collect();
    let torn = lines_of(&torn_file);
    let broken = chain_through(&torn, &id(7)).unwrap_err().0;
    assert_eq!(broken, format!("the session's chain to message {} is broken at {}", id(7), id(1)));
}

// TS: chain.test.ts:96
#[test]
fn walk_ends_a_live_copy_before_the_message_whose_call_has_no_result() {
    let mid_turn = fixture_lines("aside-mid-turn.jsonl");
    let lines = lines_of(&mid_turn);
    let end = live_end(&lines).unwrap();
    assert_eq!(end.running.len(), 1);
    assert!(end.running[0].starts_with("Bash {\"command\":\"./check.sh\""), "{}", end.running[0]);
    let kept = chain_through(&lines, &end.anchor).unwrap();
    // The last message's three lines (its thinking, its words, its Bash call) go; the result before them stays last.
    let last = *kept.last().unwrap();
    assert_eq!(last, mid_turn.len() as u64 - 3);
    let line: Value = serde_json::from_str(&mid_turn[last as usize - 1]).unwrap();
    assert_eq!(line["message"]["content"][0]["type"], json!("tool_result"));
}

// TS: chain.test.ts:108
#[test]
fn walk_writes_line_numbers_as_ranges() {
    assert_eq!(line_ranges(&[2, 3, 5, 6, 7, 9]), "2-3,5-7,9");
    assert_eq!(line_ranges(&[4]), "4");
}

// ---------------------------------------------------------------------------------------------------------------
// chain.test.ts: run by a shell on a session's files
// ---------------------------------------------------------------------------------------------------------------

// TS: chain.test.ts:161
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_writes_exactly_the_anchors_chain_beside_the_source() {
    tokio::task::LocalSet::new().run_until(shell_writes_exactly_the_anchors_chain_beside_the_source_body()).await
}

async fn shell_writes_exactly_the_anchors_chain_beside_the_source_body() {
    let root = Root::new("chain-write");
    let file = compacted();
    let (config, project, source) = set_up(&root, "-root-acme", &file);
    let exec = ShellExec::new(&[("PATH", "/usr/bin:/bin")]);
    let copied = copy_session(copy_options(&exec, &config, CopyThrough::Anchor(id(6)))).await.unwrap();
    let commands = exec.commands();
    assert_eq!(commands[0], chain_read_command(SESSION, &config).unwrap());
    assert_eq!(commands[1], chain_write_command(SESSION, &copied.session, &config, 6).unwrap());
    assert!(commands[1].contains(&format!("tmp=\"${{src%/*}}/.{}.jsonl.part\"", copied.session)), "{}", commands[1]);
    assert!(commands[1].contains(&format!("mv -f \"$tmp\" \"${{src%/*}}/{}.jsonl\"", copied.session)), "{}", commands[1]);
    assert_eq!(exec.inputs()[1], Some(vec!["2-3,5-6,8-9".to_string()]));
    let want: Vec<&str> = [2usize, 3, 5, 6, 8, 9].iter().map(|n| file[n - 1].as_str()).collect();
    assert_eq!(std::fs::read_to_string(project.join(format!("{}.jsonl", copied.session))).unwrap(), format!("{}\n", want.join("\n")));
    assert_eq!(std::fs::read_to_string(project.join(format!("{SESSION}.jsonl"))).unwrap(), source);
    let mut both = vec![format!("{}.jsonl", copied.session), format!("{SESSION}.jsonl")];
    both.sort();
    assert_eq!(names(&project), both);
}

// TS: chain.test.ts:175
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_reads_a_long_session_as_kilobytes_and_walks_it_as_the_whole_file_would() {
    tokio::task::LocalSet::new().run_until(shell_reads_a_long_session_as_kilobytes_and_walks_it_as_the_whole_file_would_body()).await
}

async fn shell_reads_a_long_session_as_kilobytes_and_walks_it_as_the_whole_file_would_body() {
    let root = Root::new("chain-long");
    let content = format!("say \\\"hi\\\" {}", "x".repeat(1 << 20));
    let big = json!({"parentUuid": id(2), "type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": content}]}, "uuid": id(50), "sessionId": SESSION}).to_string();
    let second = big.replacen(&id(50), &id(51), 1).replacen(&format!("\"parentUuid\":\"{}\"", id(2)), &format!("\"parentUuid\":\"{}\"", id(50)), 1);
    let mut file: Vec<String> = rewound()[..2].to_vec();
    file.push(big);
    file.push(second);
    let (config, _project, _source) = set_up(&root, "-root-acme", &file);
    let exec = ShellExec::new(&[("PATH", "/usr/bin:/bin")]);
    let out = exec.launch(&chain_read_command(SESSION, &config).unwrap(), ExecOptions::default());
    let read = collect_lines(&out).await;
    assert!(read.join("\n").len() < (8 << 10), "the read was {} bytes", read.join("\n").len());
    assert_eq!(chain_through(&read_chain(&read).unwrap().lines, &id(51)).unwrap(), vec![1, 2, 3, 4]);
}

// TS: chain.test.ts:187
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_refuses_an_anchor_the_file_does_not_hold_before_anything_is_written() {
    tokio::task::LocalSet::new().run_until(shell_refuses_an_anchor_the_file_does_not_hold_before_anything_is_written_body()).await
}

async fn shell_refuses_an_anchor_the_file_does_not_hold_before_anything_is_written_body() {
    let root = Root::new("chain-refuse");
    let (config, project, _source) = set_up(&root, "-root-acme", &rewound());
    let exec = ShellExec::new(&[("PATH", "/usr/bin:/bin")]);
    let err = copy_session(copy_options(&exec, &config, CopyThrough::Anchor(id(99)))).await.unwrap_err();
    assert_eq!(err.0, format!("the session holds no message {}", id(99)));
    assert_eq!(exec.commands().len(), 1);
    assert_eq!(names(&project), vec![format!("{SESSION}.jsonl")]);
}

// TS: chain.test.ts:195
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_says_the_clis_own_words_for_a_session_its_store_does_not_hold() {
    tokio::task::LocalSet::new().run_until(shell_says_the_clis_own_words_for_a_session_its_store_does_not_hold_body()).await
}

async fn shell_says_the_clis_own_words_for_a_session_its_store_does_not_hold_body() {
    let root = Root::new("chain-nostore");
    let exec = ShellExec::new(&[("PATH", "/usr/bin:/bin")]);
    let config = root.0.to_str().unwrap().to_string();
    let err = copy_session(copy_options(&exec, &config, CopyThrough::Live)).await.unwrap_err();
    assert_eq!(err.0, no_conversation_line(SESSION));
}

// TS: chain.test.ts:201
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_leaves_no_copy_and_no_part_file_where_the_write_comes_out_short() {
    tokio::task::LocalSet::new().run_until(shell_leaves_no_copy_and_no_part_file_where_the_write_comes_out_short_body()).await
}

async fn shell_leaves_no_copy_and_no_part_file_where_the_write_comes_out_short_body() {
    let root = Root::new("chain-short");
    let (config, project, _source) = set_up(&root, "-root-acme", &rewound());
    let exec = ShellExec::new(&[("PATH", "/usr/bin:/bin")]);
    let fork = "0b7f3a52-6c1d-4e8a-9f2b-3d4c5e6f7a8b";
    let write = exec.launch(&chain_write_command(SESSION, fork, &config, 5).unwrap(), ExecOptions { input: Some(vec!["1-2".to_string()]), ..Default::default() });
    write.close_input();
    let said = collect_lines(&write).await;
    assert_eq!(write.exited().await, Some(1));
    assert_eq!(said, vec![format!("the copy of session {SESSION} came out short")]);
    assert_eq!(names(&project), vec![format!("{SESSION}.jsonl")]);
}

// TS: chain.test.ts:214
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_removes_only_the_copy() {
    tokio::task::LocalSet::new().run_until(shell_removes_only_the_copy_body()).await
}

async fn shell_removes_only_the_copy_body() {
    let root = Root::new("chain-remove");
    let (config, project, _source) = set_up(&root, "-root-acme", &rewound());
    let exec = ShellExec::new(&[("PATH", "/usr/bin:/bin")]);
    let copied = copy_session(copy_options(&exec, &config, CopyThrough::Live)).await.unwrap();
    assert!(project.join(format!("{}.jsonl", copied.session)).exists());
    let gone = exec.launch(&copy_cleanup_command(&copied.session, &config).unwrap(), ExecOptions::default());
    gone.exited().await;
    assert_eq!(names(&project), vec![format!("{SESSION}.jsonl")]);
    let refused = copy_cleanup_command("*", &config).unwrap_err();
    assert!(refused.0.contains("UUID"), "{}", refused.0);
}

// ---------------------------------------------------------------------------------------------------------------
// aside.test.ts
// ---------------------------------------------------------------------------------------------------------------

const RESULT_TEXT: &str = "You are in /root/spoo and last asked why the short links 302 twice.";

fn result_event() -> Value {
    json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "duration_ms": 2310,
        "result": RESULT_TEXT,
        "session_id": FORK,
        "total_cost_usd": 0.004,
        "usage": {"input_tokens": 12, "cache_read_input_tokens": 18435, "output_tokens": 31},
    })
}

fn result_line() -> String {
    result_event().to_string()
}

fn result_usage() -> IndexMap<String, Value> {
    result_event()["usage"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// The result event with `set` applied over it, in place, and `drop` keys removed.
fn result_with(set: &[(&str, Value)], drop: &[&str]) -> String {
    let mut v = result_event();
    for (k, val) in set {
        v[*k] = val.clone();
    }
    for k in drop {
        v.as_object_mut().unwrap().remove(*k);
    }
    v.to_string()
}

fn finished() -> Vec<String> {
    let u1 = id(1);
    let u2 = id(2);
    vec![
        "2".to_string(),
        format!("1:{}", json!({"parentUuid": null, "type": "user", "message": {"role": "user", "content": "hi"}, "uuid": u1})),
        format!("2:{}", json!({"parentUuid": u1, "type": "assistant", "message": {"id": "m1", "content": [{"type": "text", "text": "hello"}]}, "uuid": u2})),
    ]
}

/// A session file as the copy's read prints it: every line, by number.
fn read_all_of(file: &[String]) -> Vec<String> {
    let mut out = vec![file.len().to_string()];
    out.extend(file.iter().enumerate().map(|(i, l)| format!("{}:{}", i + 1, l)));
    out
}

fn fork_of(command: &str) -> String {
    regex::Regex::new(r"--resume ([0-9a-f-]+)$").unwrap().captures(command).map(|c| c[1].to_string()).unwrap_or_default()
}

fn servers() -> IndexMap<String, McpServerSpec> {
    IndexMap::from([("wsp".to_string(), McpServerSpec { command: "wsp".into(), args: strs(&["mcp", "--scoped"]), no_slate: None })])
}

#[derive(Clone, Debug)]
struct Rec {
    command: String,
    env: Env,
    input: Option<Vec<String>>,
    secret: Option<ExecSecret>,
}

#[derive(Clone, Default)]
struct ScriptOpts {
    exit_code: Option<i32>,
    hang: bool,
    tail: Option<Vec<String>>,
    tail_code: Option<i32>,
}

/// The copy's read prints `tail` and ends with `tail_code`, and its write ends at once; the CLI's run prints `lines`,
/// then ends with `exit_code`, or in hang mode only once it is torn down; any later run (the copy's removal) prints
/// nothing and ends at once. `order` notes each launch and each end.
struct Scripted {
    lines: Vec<String>,
    opts: ScriptOpts,
    calls: Mutex<Vec<Rec>>,
    order: Arc<Mutex<Vec<String>>>,
    writes: Arc<Mutex<Vec<String>>>,
}

fn scripted(lines: Vec<String>, opts: ScriptOpts) -> Arc<Scripted> {
    Arc::new(Scripted { lines, opts, calls: Default::default(), order: Default::default(), writes: Default::default() })
}

impl Scripted {
    fn calls(&self) -> Vec<Rec> {
        self.calls.lock().unwrap().clone()
    }
    fn order(&self) -> Vec<String> {
        self.order.lock().unwrap().clone()
    }
    fn writes(&self) -> Vec<String> {
        self.writes.lock().unwrap().clone()
    }
    fn at(&self, what: &str) -> usize {
        self.order().iter().position(|o| o == what).unwrap_or_else(|| panic!("{what} never happened: {:?}", self.order()))
    }
}

#[derive(Clone)]
struct Ender {
    n: usize,
    exit: Exit,
    order: Arc<Mutex<Vec<String>>>,
}

impl Ender {
    fn end(&self, code: Option<i32>) {
        if self.exit.settled().is_none() {
            self.order.lock().unwrap().push(format!("exited {}", self.n));
            self.exit.resolve(code);
        }
    }
}

struct ScriptStream {
    lines: Mutex<Option<LocalBoxStream<'static, Result<String, ExecFailure>>>>,
    ender: Ender,
    writes: Arc<Mutex<Vec<String>>>,
}

impl ExecStreamFactory for Scripted {
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        let n = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(Rec { command: command.to_string(), env: options.env, input: options.input, secret: options.secret });
            calls.len()
        };
        self.order.lock().unwrap().push(format!("launch {n}"));
        let ender = Ender { n, exit: Exit::new(), order: self.order.clone() };
        let (body, code, hang) = match n {
            1 => (self.opts.tail.clone().unwrap_or_else(finished), self.opts.tail_code.unwrap_or(0), false),
            3 => (self.lines.clone(), self.opts.exit_code.unwrap_or(0), self.opts.hang),
            _ => (vec![], 0, false),
        };
        let tail_ender = ender.clone();
        let tail = stream::once(async move {
            if hang {
                tail_ender.exit.future().await;
            } else {
                tail_ender.end(Some(code));
            }
            None::<Result<String, ExecFailure>>
        })
        .filter_map(|x| async move { x });
        let lines = stream::iter(body.into_iter().map(Ok)).chain(tail).boxed_local();
        Arc::new(ScriptStream { lines: Mutex::new(Some(lines)), ender, writes: self.writes.clone() })
    }
}

impl ExecStream for ScriptStream {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        self.lines.lock().unwrap().take().unwrap_or_else(|| stream::empty().boxed_local())
    }
    fn teardown(&self) {
        self.ender.order.lock().unwrap().push("teardown".to_string());
        self.ender.end(Some(143));
    }
    fn kill(&self) {
        self.ender.order.lock().unwrap().push("kill".to_string());
        self.ender.end(None);
    }
    fn write(&self, line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        self.writes.lock().unwrap().push(line);
        async { Ok(WriteOutcome::Written) }.boxed_local()
    }
    fn close_input(&self) {
        self.ender.order.lock().unwrap().push("closeInput".to_string());
    }
    fn exited(&self) -> wsp_adapters::port::SharedFuture<Option<i32>> {
        self.ender.exit.future()
    }
}

fn adapter(exec: &Arc<Scripted>, aside_wall_ms: Option<u64>) -> ClaudeAdapter {
    let factory: Arc<dyn ExecStreamFactory> = exec.clone();
    let mut deps = AdapterDeps::new(factory, CONFIG);
    deps.base_env = Some(BaseEnv::from([("PATH".to_string(), Some("/bin".to_string())), ("CLAUDE_CODE_ENTRYPOINT".to_string(), Some("cli".to_string()))]));
    deps.oauth_token = Some("sk-ant-oat-x".to_string());
    deps.project_dir_name = Some("-root-spoo".to_string());
    deps.result_exit_ms = Some(5);
    deps.interrupt_grace_ms = Some(5);
    deps.aside_wall_ms = aside_wall_ms;
    create_claude_adapter(deps).unwrap()
}

fn ask(question: &str) -> AsideQuestion {
    AsideQuestion { session: SESSION.to_string(), question: question.to_string(), ..Default::default() }
}

fn env_is(env: &Env, key: &str, value: &str) {
    assert_eq!(env.get(key).map(String::as_str), Some(value), "{key} in {env:?}");
}

// ---- asideCommand

// TS: aside.test.ts:103
#[test]
fn aside_command_launches_the_copy_as_the_threads_turns_launch_with_its_hooks_off() {
    let memory_dir = format!("{CONFIG}/projects/-root-spoo/memory");
    let turn = build_command(&BuildCommandOptions {
        resume: Some(SESSION.to_string()),
        cwd: Some("/root/spoo".into()),
        model: Some("claude-opus-5".into()),
        effort: Some("high".into()),
        context_window: Some("1m".into()),
        mcp_servers: Some(servers()),
        memory_dir: Some(memory_dir.clone()),
        ..Default::default()
    })
    .unwrap();
    let line = aside_command(&AsideCommandOptions {
        fork: FORK.to_string(),
        cwd: Some("/root/spoo".into()),
        model: Some("claude-opus-5".into()),
        effort: Some("high".into()),
        context_window: Some("1m".into()),
        mcp_servers: Some(servers()),
        memory_dir: Some(memory_dir.clone()),
        ..Default::default()
    })
    .unwrap();
    assert!(line.starts_with(&format!("cd '/root/spoo' && {} -p ", agent_word("claude", None))), "{line}");
    // Every flag that shapes the request the model is sent: the model and its window, the effort, the servers whose
    // tools head the prompt, and the slate's brief at the end of the system prompt.
    for flag in [r"--model '[^']*'", r"--effort '[^']*'", r"--mcp-config '[^']*'", r"--append-system-prompt '[^']*'"] {
        let part = regex::Regex::new(flag).unwrap().find(&turn).unwrap_or_else(|| panic!("{flag} not in the turn: {turn}")).as_str().to_string();
        assert!(line.contains(&part), "{flag}: {part} not in {line}");
    }
    for part in [
        "--input-format stream-json".to_string(),
        "--output-format stream-json".to_string(),
        "--verbose".to_string(),
        "--include-partial-messages".to_string(),
        "--permission-mode 'default' --permission-prompt-tool stdio".to_string(),
        format!("--settings '{{\"autoMemoryDirectory\":\"{memory_dir}\",\"disableAllHooks\":true}}'"),
    ] {
        assert!(line.contains(&part), "{part} not in {line}");
    }
    assert!(line.ends_with(&format!("--max-turns 2 --resume {FORK}")), "{line}");
    // A tool list the turn did not have is a prompt the cache has never seen, and the copy pays the whole thread again.
    for absent in ["--tools", "--disallowedTools", "--allowed-tools", "--dangerously-skip-permissions", "--fork-session", "--session-id", "--strict-mcp-config", "--safe-mode", "--bare", "--no-session-persistence", "--resume-session-at"] {
        assert!(!line.contains(absent), "{absent} in {line}");
    }
}

// TS: aside.test.ts:130
#[test]
fn aside_command_keeps_hooks_off_where_the_launch_words_name_a_settings_file() {
    let launch = AgentLaunch { program: None, args: Some(strs(&["--debug", "--settings", "/root/my-settings.json"])) };
    let line = aside_command(&AsideCommandOptions { fork: FORK.to_string(), launch: Some(launch.clone()), ..Default::default() }).unwrap();
    assert!(line.contains("--settings '{\"disableAllHooks\":true}'"), "{line}");
    assert!(!line.contains("my-settings.json"), "{line}");
    assert!(line.contains("claude -p '--debug' --input-format"), "{line}");
    // A turn keeps the person's file, as the CLI reads it.
    let turn = build_command(&BuildCommandOptions { resume: Some(SESSION.to_string()), launch: Some(launch), ..Default::default() }).unwrap();
    assert!(turn.contains("'--settings' '/root/my-settings.json'"), "{turn}");
    let equals = aside_command(&AsideCommandOptions { fork: FORK.to_string(), launch: Some(AgentLaunch { program: None, args: Some(strs(&["--settings=/root/my-settings.json"])) }), ..Default::default() }).unwrap();
    assert!(!equals.contains("my-settings.json"), "{equals}");
}

// TS: aside.test.ts:141
#[test]
fn aside_command_hands_the_copy_the_servers_on_the_flag_a_turn_takes_them_on() {
    let line = aside_command(&AsideCommandOptions { fork: FORK.to_string(), mcp_servers: Some(servers()), ..Default::default() }).unwrap();
    let turn = build_command(&BuildCommandOptions { resume: Some(SESSION.to_string()), mcp_servers: Some(servers()), ..Default::default() }).unwrap();
    let flag = regex::Regex::new(r"--mcp-config '[^']*'").unwrap().find(&turn).unwrap_or_else(|| panic!("no --mcp-config in {turn}")).as_str().to_string();
    assert!(line.contains(&flag), "{flag} not in {line}");
}

// TS: aside.test.ts:150
#[test]
fn aside_command_leaves_the_model_out_and_refuses_a_copy_id_of_the_wrong_shape() {
    let bare = aside_command(&AsideCommandOptions { fork: FORK.to_string(), ..Default::default() }).unwrap();
    assert!(!bare.contains("--model"), "{bare}");
    assert!(bare.starts_with("cd ~ && "), "{bare}");
    let refused = aside_command(&AsideCommandOptions { fork: "*".to_string(), ..Default::default() }).unwrap_err();
    assert!(refused.0.contains("UUID"), "{}", refused.0);
}

// ---- run by a shell against a stand-in CLI

/// A config dir holding the thread's session file, and a stand-in claude that answers with the words "Remember" opens
/// in the copy it was resumed on, which it finds by the id after its last flag.
fn stand_in(root: &Root, file: &[String]) -> (PathBuf, String, ClaudeAdapter) {
    let (config, project, thread) = set_up(root, "-root-spoo", file);
    let bin = root.0.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let copy = format!("{}/\"$last\".jsonl", wsp_frames::shell_quote(project.to_str().unwrap()));
    let script = "#!/bin/sh\nfor a; do last=\"$a\"; done\nsaid=$(grep -o 'Remember [A-Z]*' COPY | tr '\\n' ' ')\nlines=$(wc -l < COPY)\nprintf '{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"%s| %s lines\"}\\n' \"$said\" $lines\n".replace("COPY", &copy);
    let stub = bin.join("claude");
    std::fs::write(&stub, script).unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:/usr/bin:/bin", bin.to_str().unwrap());
    let exec: Arc<dyn ExecStreamFactory> = ShellExec::new(&[("PATH", path.as_str())]);
    let mut deps = AdapterDeps::new(exec, config);
    deps.base_env = Some(BaseEnv::from([("PATH".to_string(), Some(path.clone()))]));
    deps.result_exit_ms = Some(5);
    deps.interrupt_grace_ms = Some(5);
    (project, thread, create_claude_adapter(deps).unwrap())
}

// TS: aside.test.ts:208
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_asks_the_mid_turn_thread_on_a_copy_cut_before_its_running_call_and_removes_only_the_copy() {
    tokio::task::LocalSet::new().run_until(shell_asks_the_mid_turn_thread_on_a_copy_cut_before_its_running_call_and_removes_only_the_copy_body()).await
}

async fn shell_asks_the_mid_turn_thread_on_a_copy_cut_before_its_running_call_and_removes_only_the_copy_body() {
    let root = Root::new("aside-mid-turn");
    let mid_turn = fixture_lines("aside-mid-turn.jsonl");
    let (project, thread, adapter) = stand_in(&root, &mid_turn);
    let answer = adapter.aside(AsideQuestion { cwd: Some(root.0.to_str().unwrap().to_string()), ..ask("btw what's the status?") }).await.unwrap();
    // The last message's three lines (its thinking, its words, its Bash call) stay out of the copy.
    assert!(answer.text.ends_with(&format!("| {} lines", mid_turn.len() - 3)), "{}", answer.text);
    assert_eq!(names(&project), vec![format!("{SESSION}.jsonl")]);
    assert_eq!(std::fs::read_to_string(project.join(format!("{SESSION}.jsonl"))).unwrap(), thread);
}

// TS: aside.test.ts:217
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_answers_from_the_live_conversation_of_a_rewound_thread() {
    tokio::task::LocalSet::new().run_until(shell_answers_from_the_live_conversation_of_a_rewound_thread_body()).await
}

async fn shell_answers_from_the_live_conversation_of_a_rewound_thread_body() {
    let root = Root::new("aside-rewound");
    let line = |n: u32, parent: Option<u32>, kind: &str, text: &str| {
        let mut message = Map::new();
        message.insert("role".into(), json!(kind));
        if kind == "assistant" {
            message.insert("id".into(), json!(format!("msg_{n}")));
        }
        message.insert("content".into(), json!([{"type": "text", "text": text}]));
        json!({"parentUuid": parent.map(id), "type": kind, "message": Value::Object(message), "uuid": id(n)}).to_string()
    };
    // ALPHA, BETA, then a rewind to ALPHA's reply and OMEGA hung off it: BETA's turn is still in the file, after which
    // the last-prompt line points at the live end as 2.1.296 writes it.
    let rewound = vec![
        line(1, None, "user", "Remember ALPHA."),
        line(2, Some(1), "assistant", "OK"),
        line(3, Some(2), "user", "Remember BETA."),
        line(4, Some(3), "assistant", "OK"),
        line(5, Some(2), "user", "Remember OMEGA."),
        line(6, Some(5), "assistant", "OK"),
        json!({"type": "last-prompt", "leafUuid": id(6)}).to_string(),
    ];
    let (_project, _thread, adapter) = stand_in(&root, &rewound);
    let answer = adapter.aside(AsideQuestion { cwd: Some(root.0.to_str().unwrap().to_string()), ..ask("which code words?") }).await.unwrap();
    assert_eq!(answer.text, "Remember ALPHA Remember OMEGA | 4 lines");
}

// ---- the adapter's aside

// TS: aside.test.ts:233
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_copies_through_the_live_end_and_runs_the_copy_on_the_turns_road() {
    tokio::task::LocalSet::new().run_until(aside_copies_through_the_live_end_and_runs_the_copy_on_the_turns_road_body()).await
}

async fn aside_copies_through_the_live_end_and_runs_the_copy_on_the_turns_road_body() {
    let exec = scripted(
        vec![
            json!({"type": "system", "subtype": "init", "session_id": FORK, "cwd": "/root/spoo", "tools": []}).to_string(),
            json!({"type": "assistant", "message": {"id": "m1", "content": [{"type": "text", "text": "You are in /root/spoo"}]}, "session_id": FORK}).to_string(),
            result_line(),
        ],
        ScriptOpts::default(),
    );
    let question = "which folder, and what did I last ask?";
    let answer = adapter(&exec, None).aside(AsideQuestion { cwd: Some("/root/spoo".into()), model: Some("claude-opus-5".into()), mcp_servers: Some(servers()), ..ask(question) }).await.unwrap();
    assert_eq!(answer, AsideAnswer { text: RESULT_TEXT.to_string(), usage: Some(result_usage()) });
    let calls = exec.calls();
    // The thread's session file, read first on the same road, says where the copy ends, and the write takes its lines.
    assert_eq!(calls[0].command, chain_read_command(SESSION, CONFIG).unwrap());
    assert_eq!(calls[0].input, None);
    let call = &calls[2];
    let fork = fork_of(&call.command);
    assert!(fork.len() == 36 && fork.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase() || c == '-'), "{fork}");
    assert_ne!(fork, SESSION);
    assert_eq!(calls[1].command, chain_write_command(SESSION, &fork, CONFIG, 2).unwrap());
    assert_eq!(calls[1].input, Some(vec!["1-2".to_string()]));
    let want = aside_command(&AsideCommandOptions {
        fork: fork.clone(),
        cwd: Some("/root/spoo".into()),
        model: Some("claude-opus-5".into()),
        mcp_servers: Some(servers()),
        memory_dir: Some(format!("{CONFIG}/projects/-root-spoo/memory")),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(call.command, want);
    // The copy reads the memory the thread's turns read, in the one settings flag that turns its hooks off.
    assert!(call.command.contains(&format!("--settings '{{\"autoMemoryDirectory\":\"{CONFIG}/projects/-root-spoo/memory\",\"disableAllHooks\":true}}'")), "{}", call.command);
    // The turn's own environment: the login it signs in with and the folder key the thread's session is stored under.
    env_is(&call.env, "CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat-x");
    env_is(&call.env, "CLAUDE_CODE_PROJECT_DIR_NAME", "-root-spoo");
    env_is(&call.env, "PATH", "/bin");
    assert_eq!(call.env.get("CLAUDE_CODE_ENTRYPOINT"), None);
    // The hook that refuses every tool goes first, so it stands before the question is read.
    let input = call.input.clone().unwrap();
    assert_eq!(input, vec![aside_hooks_line(), user_message_line(&aside_prompt(question, &[]), &fork, &[], None)]);
    // No line carries the uuid a steer goes out under: the copy holds no reply for the CLI's word on a message.
    for l in &input {
        assert!(json(l).get("uuid").is_none(), "{l}");
    }
    assert!(exec.order().contains(&"closeInput".to_string()));
    // The copy's file goes by a last run on the same road and environment, once the CLI's own run has ended.
    assert_eq!(calls[3].command, copy_cleanup_command(&fork, CONFIG).unwrap());
    assert_eq!(calls[3].input, None);
    env_is(&calls[3].env, "PATH", "/bin");
    assert!(exec.at("exited 1") < exec.at("launch 2"));
    assert!(exec.at("exited 2") < exec.at("launch 3"));
    assert!(exec.at("exited 3") < exec.at("launch 4"));
    assert_eq!(calls.len(), 4);
}

// TS: aside.test.ts:269
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_hands_the_copy_each_servers_value_in_a_file_of_the_runs() {
    tokio::task::LocalSet::new().run_until(aside_hands_the_copy_each_servers_value_in_a_file_of_the_runs_body()).await
}

async fn aside_hands_the_copy_each_servers_value_in_a_file_of_the_runs_body() {
    let exec = scripted(vec![result_line()], ScriptOpts::default());
    let entry: IndexMap<String, Value> = IndexMap::from([
        ("type".to_string(), json!("http")),
        ("url".to_string(), json!("https://mcp.linear.app/mcp")),
        ("headers".to_string(), json!({"Authorization": "Bearer lin_TESTONLY"})),
    ]);
    let entries = IndexMap::from([("tracker".to_string(), entry)]);
    let values = ServerValues { entries: Some(entries), config: None };
    adapter(&exec, None).aside(AsideQuestion { cwd: Some("/root/spoo".into()), server_values: Some(values), ..ask("what did I last ask?") }).await.unwrap();
    let calls = exec.calls();
    let call = &calls[2];
    let file = json!({"mcpServers": {"tracker": {"type": "http", "url": "https://mcp.linear.app/mcp", "headers": {"Authorization": "Bearer lin_TESTONLY"}}}}).to_string();
    assert_eq!(call.secret, Some(ExecSecret { files: Some(Env::from([("WSP_MCP_VALUES".to_string(), file)])), input: false }));
    assert!(call.command.contains("--mcp-config \"$WSP_MCP_VALUES\""), "{}", call.command);
    assert!(!call.command.contains("lin_TESTONLY"), "{}", call.command);
    assert!(!serde_json::to_string(&call.env).unwrap().contains("lin_TESTONLY"));
    // The copy's read, its write and its removal carry none.
    assert_eq!([&calls[0], &calls[1], &calls[3]].map(|c| c.secret.clone()), [None, None, None]);
}

// TS: aside.test.ts:282
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_hands_each_piece_of_the_answer_on_and_still_answers_the_whole_result() {
    tokio::task::LocalSet::new().run_until(aside_hands_each_piece_of_the_answer_on_and_still_answers_the_whole_result_body()).await
}

async fn aside_hands_each_piece_of_the_answer_on_and_still_answers_the_whole_result_body() {
    // The partial message lines 2.1.289 prints under --include-partial-messages, a thinking piece among them.
    let delta = |index: u32, delta: Value| json!({"type": "stream_event", "event": {"type": "content_block_delta", "index": index, "delta": delta}, "session_id": FORK, "parent_tool_use_id": null}).to_string();
    let exec = scripted(
        vec![
            json!({"type": "system", "subtype": "init", "session_id": FORK, "tools": []}).to_string(),
            json!({"type": "stream_event", "event": {"type": "message_start", "message": {"id": "m1"}}, "session_id": FORK}).to_string(),
            delta(0, json!({"type": "thinking_delta", "thinking": "the folder"})),
            delta(1, json!({"type": "text_delta", "text": "You are in "})),
            delta(1, json!({"type": "text_delta", "text": "/root/spoo"})),
            result_line(),
        ],
        ScriptOpts::default(),
    );
    let pieces: Arc<Mutex<Vec<String>>> = Default::default();
    let sink = pieces.clone();
    let on_text: Arc<dyn Fn(String)> = Arc::new(move |t| sink.lock().unwrap().push(t));
    let answer = adapter(&exec, None).aside(AsideQuestion { on_text: Some(on_text), ..ask("which folder?") }).await.unwrap();
    assert_eq!(*pieces.lock().unwrap(), strs(&["You are in ", "/root/spoo"]));
    assert_eq!(answer.text, RESULT_TEXT);
}

// TS: aside.test.ts:299
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_answers_past_the_empty_result_a_resume_gives_a_background_command() {
    tokio::task::LocalSet::new().run_until(aside_answers_past_the_empty_result_a_resume_gives_a_background_command_body()).await
}

async fn aside_answers_past_the_empty_result_a_resume_gives_a_background_command_body() {
    // As 2.1.289 printed it, forking a thread whose last turn ended with a command still running in the background.
    let exec = scripted(
        vec![
            json!({"type": "system", "subtype": "task_notification", "task_id": "b1we0t1su", "status": "stopped", "summary": "Background shell command didn't finish before the previous session ended", "session_id": FORK}).to_string(),
            json!({"type": "system", "subtype": "init", "session_id": FORK, "tools": []}).to_string(),
            json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 0, "duration_ms": 174, "result": "", "origin": {"kind": "task-notification"}, "session_id": FORK, "usage": {"input_tokens": 0, "output_tokens": 0}}).to_string(),
            json!({"type": "system", "subtype": "init", "session_id": FORK, "tools": []}).to_string(),
            result_with(&[("num_turns", json!(1))], &[]),
        ],
        ScriptOpts::default(),
    );
    let answer = adapter(&exec, None).aside(ask("what is going on?")).await.unwrap();
    assert_eq!(answer.text, RESULT_TEXT);
    assert!(exec.order().contains(&"closeInput".to_string()));
}

// TS: aside.test.ts:313
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_rejects_with_the_clis_own_words_when_its_result_is_an_error() {
    tokio::task::LocalSet::new().run_until(aside_rejects_with_the_clis_own_words_when_its_result_is_an_error_body()).await
}

async fn aside_rejects_with_the_clis_own_words_when_its_result_is_an_error_body() {
    let exec = scripted(vec![result_with(&[("subtype", json!("success")), ("is_error", json!(true)), ("result", json!("Not logged in · Please run /login"))], &[])], ScriptOpts::default());
    let err = adapter(&exec, None).aside(ask("hi")).await.unwrap_err();
    assert_eq!(err, "Not logged in · Please run /login");
}

// TS: aside.test.ts:318
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_rejects_with_what_the_cli_printed_on_an_exit_with_no_result_and_removes_the_copy() {
    tokio::task::LocalSet::new().run_until(aside_rejects_with_what_the_cli_printed_on_an_exit_with_no_result_and_removes_the_copy_body()).await
}

async fn aside_rejects_with_what_the_cli_printed_on_an_exit_with_no_result_and_removes_the_copy_body() {
    let exec = scripted(vec![no_conversation_line(SESSION)], ScriptOpts { exit_code: Some(1), ..Default::default() });
    let err = adapter(&exec, None).aside(ask("hi")).await.unwrap_err();
    assert_eq!(err, no_conversation_line(SESSION));
    let calls = exec.calls();
    assert_eq!(calls[3].command, copy_cleanup_command(&fork_of(&calls[2].command), CONFIG).unwrap());
}

// TS: aside.test.ts:324
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_rejects_with_the_reads_own_line_where_the_session_file_is_not_there() {
    tokio::task::LocalSet::new().run_until(aside_rejects_with_the_reads_own_line_where_the_session_file_is_not_there_body()).await
}

async fn aside_rejects_with_the_reads_own_line_where_the_session_file_is_not_there_body() {
    let exec = scripted(vec![result_line()], ScriptOpts { tail: Some(vec![no_conversation_line(SESSION)]), tail_code: Some(1), ..Default::default() });
    let err = adapter(&exec, None).aside(ask("hi")).await.unwrap_err();
    assert_eq!(err, no_conversation_line(SESSION));
    assert_eq!(exec.calls().len(), 1);
}

// TS: aside.test.ts:330
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_ends_a_cli_that_says_nothing_for_the_whole_wall_and_says_so() {
    tokio::task::LocalSet::new().run_until(aside_ends_a_cli_that_says_nothing_for_the_whole_wall_and_says_so_body()).await
}

async fn aside_ends_a_cli_that_says_nothing_for_the_whole_wall_and_says_so_body() {
    let exec = scripted(vec![], ScriptOpts { hang: true, ..Default::default() });
    let err = adapter(&exec, Some(20)).aside(ask("hi")).await.unwrap_err();
    assert_eq!(err, aside_wall_line(20));
    assert!(exec.at("launch 3") < exec.at("teardown"));
    // The kill took the CLI's shell with it, so the copy's file goes by a run of its own once that one has ended.
    let calls = exec.calls();
    let fork = fork_of(&calls[2].command);
    assert_eq!(calls[3].command, copy_cleanup_command(&fork, CONFIG).unwrap());
    assert!(exec.at("exited 3") < exec.at("launch 4"));
}

// TS: aside.test.ts:340
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_registers_a_pre_tool_use_hook_and_refuses_every_call_through_it() {
    tokio::task::LocalSet::new().run_until(aside_registers_a_pre_tool_use_hook_and_refuses_every_call_through_it_body()).await
}

async fn aside_registers_a_pre_tool_use_hook_and_refuses_every_call_through_it_body() {
    let init = json(&aside_hooks_line());
    assert_eq!(init["type"], json!("control_request"));
    assert_eq!(init["request_id"], json!(ASIDE_HOOKS_ID));
    assert_eq!(init["request"]["subtype"], json!("initialize"));
    let callbacks = init["request"]["hooks"]["PreToolUse"].as_array().unwrap().clone();
    assert_eq!(callbacks.len(), 1);
    assert_eq!(callbacks[0].as_object().unwrap().keys().cloned().collect::<Vec<_>>(), strs(&["matcher", "hookCallbackIds"]));
    assert_eq!(callbacks[0]["matcher"], Value::Null);
    let ids = callbacks[0]["hookCallbackIds"].as_array().unwrap().clone();
    assert_eq!(ids.len(), 1);
    assert!(ids[0].is_string());
    // As 2.1.280 printed them: the answer to the hook's registration, a call to the hook for a Bash call the model
    // tried, and a prompt for a call that reached the permission check anyway.
    let exec = scripted(
        vec![
            json!({"type": "control_response", "response": {"subtype": "success", "request_id": ASIDE_HOOKS_ID, "response": {"commands": []}}}).to_string(),
            json!({"type": "control_request", "request_id": "h1", "request": {"subtype": "hook_callback", "callback_id": ids[0], "input": {"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "touch x"}}}}).to_string(),
            json!({"type": "control_request", "request_id": "p1", "request": {"subtype": "can_use_tool", "tool_name": "Edit", "input": {"file_path": "a"}, "tool_use_id": "toolu_1"}}).to_string(),
            result_line(),
        ],
        ScriptOpts::default(),
    );
    adapter(&exec, None).aside(ask("touch x for me?")).await.unwrap();
    let writes: Vec<Value> = exec.writes().iter().map(|l| json(l)).collect();
    assert_eq!(writes.len(), 2, "{writes:?}");
    let reason = writes[0]["response"]["response"]["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or_default().to_string();
    assert!(reason.contains("side question"), "{reason}");
    let message = writes[1]["response"]["response"]["message"].as_str().unwrap_or_default().to_string();
    assert!(message.contains("side question"), "{message}");
    assert_eq!(writes[0], json!({"type": "control_response", "response": {"subtype": "success", "request_id": "h1", "response": {"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": reason}}}}));
    assert_eq!(writes[1], json!({"type": "control_response", "response": {"subtype": "success", "request_id": "p1", "response": {"behavior": "deny", "message": message}}}));
}

// TS: aside.test.ts:360
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_asks_the_mid_turn_thread_on_a_copy_cut_before_its_running_call_and_says_it_is_running() {
    tokio::task::LocalSet::new().run_until(aside_asks_the_mid_turn_thread_on_a_copy_cut_before_its_running_call_and_says_it_is_running_body()).await
}

async fn aside_asks_the_mid_turn_thread_on_a_copy_cut_before_its_running_call_and_says_it_is_running_body() {
    let mid_turn = fixture_lines("aside-mid-turn.jsonl");
    let exec = scripted(vec![result_line()], ScriptOpts { tail: Some(read_all_of(&mid_turn)), ..Default::default() });
    adapter(&exec, None).aside(ask("btw what's the status?")).await.unwrap();
    let lines = read_chain(&read_all_of(&mid_turn)).unwrap().lines;
    let end = live_end(&lines).unwrap();
    let calls = exec.calls();
    assert_eq!(calls[1].input, Some(vec![line_ranges(&chain_through(&lines, &end.anchor).unwrap())]));
    let line = json(&calls[2].input.clone().unwrap()[1]);
    let asked: String = line["message"]["content"].as_array().unwrap().iter().map(|b| b["text"].as_str().unwrap_or("")).collect();
    assert_eq!(asked, aside_prompt("btw what's the status?", &end.running));
    assert!(asked.ends_with("btw what's the status?"));
    // The words the CLI adds on opening the copy, named so the model reads them as that and not as the person.
    for part in [
        "not interrupted",
        "\"Continue from where you left off.\"",
        "\"No response requested.\"",
        "didn't finish before the previous session ended",
        // A background agent's notice reads status failed, "was running when the previous Claude Code process exited"
        // and its state lost; named so it reads as the copy opening and not the agent failing.
        "a background command or a background agent",
        "(stopped, failed or another)",
        "was running when the previous Claude Code process exited",
        "its state was lost",
        "did not fail, stop or get cut off",
        "do not mention these lines, a notification or this copy unless the person asks about them by name",
        "the thread is running right now, whose result has not come back yet: Bash {\"command\":\"./check.sh\"",
    ] {
        assert!(asked.contains(part), "{part} not in {asked}");
    }
}

// TS: aside.test.ts:386
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_says_in_one_line_that_the_answer_reached_for_a_tool_at_the_turn_cap() {
    tokio::task::LocalSet::new().run_until(aside_says_in_one_line_that_the_answer_reached_for_a_tool_at_the_turn_cap_body()).await
}

async fn aside_says_in_one_line_that_the_answer_reached_for_a_tool_at_the_turn_cap_body() {
    let exec = scripted(vec![result_with(&[("subtype", json!("error_max_turns")), ("is_error", json!(true)), ("num_turns", json!(3))], &["result"])], ScriptOpts::default());
    let err = adapter(&exec, None).aside(ask("run the tests?")).await.unwrap_err();
    assert_eq!(err, ASIDE_TOOL_LINE);
}

// TS: aside.test.ts:391
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_rejects_with_the_last_line_the_cli_printed_when_it_exits_with_no_result() {
    tokio::task::LocalSet::new().run_until(aside_rejects_with_the_last_line_the_cli_printed_when_it_exits_with_no_result_body()).await
}

async fn aside_rejects_with_the_last_line_the_cli_printed_when_it_exits_with_no_result_body() {
    let exec = scripted(strs(&["Warning: something about the shell", "Error: Invalid API key · Please run /login"]), ScriptOpts { exit_code: Some(1), ..Default::default() });
    let err = adapter(&exec, None).aside(ask("hi")).await.unwrap_err();
    assert_eq!(err, "Error: Invalid API key · Please run /login");
}

// TS: aside.test.ts:397
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aside_ends_the_run_and_says_why_where_the_cli_refuses_the_hook() {
    tokio::task::LocalSet::new().run_until(aside_ends_the_run_and_says_why_where_the_cli_refuses_the_hook_body()).await
}

async fn aside_ends_the_run_and_says_why_where_the_cli_refuses_the_hook_body() {
    let exec = scripted(
        vec![json!({"type": "control_response", "response": {"subtype": "error", "request_id": ASIDE_HOOKS_ID, "error": "Unsupported control request subtype: initialize"}}).to_string()],
        ScriptOpts { hang: true, ..Default::default() },
    );
    let err = adapter(&exec, None).aside(ask("hi")).await.unwrap_err();
    assert_eq!(err, "Unsupported control request subtype: initialize");
    assert!(exec.order().contains(&"teardown".to_string()));
}

fn chain_line(at: u64, uuid: &str, parent: Option<&str>, kind: &str, message_id: Option<&str>, calls: Vec<wsp_adapters::claude::chain::ChainCall>) -> ChainLine {
    ChainLine { at, uuid: uuid.into(), parent: parent.map(String::from), r#type: kind.into(), boundary: false, message_id: message_id.map(String::from), calls, answers: vec![] }
}

// Rule K1 (a forced difference): a running call is named by its first 300 UTF-16 units of input; the TS cut can split a
// pair and leave half of it, which a Rust string cannot hold, so the half goes.
#[test]
fn a_running_call_is_cut_at_300_utf16_units() {
    let s = format!("a{}", "😀".repeat(200));
    let call = wsp_adapters::claude::chain::ChainCall { id: "c1".into(), name: "Write".into(), input: json!({"s": s}) };
    let lines = [chain_line(1, "u0", None, "user", None, vec![]), chain_line(2, "a1", Some("u0"), "assistant", Some("m1"), vec![call])];
    let end = live_end(&lines).unwrap();
    assert_eq!(end, LiveEnd { anchor: "u0".into(), running: vec![format!("Write {{\"s\":\"a{}", "😀".repeat(146))] }, "7 units, 146 pairs, and the half of the 147th dropped");
}

// Rule K2: the write's refusals come in the TS order: the fork's id, then the count, then the session's id.
#[test]
fn the_write_refuses_in_the_ts_order() {
    let u = "e16ed170-8257-4668-879e-fe836341633c";
    let refused = |session: &str, fork: &str, count: u64| chain_write_command(session, fork, "/c", count).expect_err("refused").0;
    assert_eq!(refused(u, "x", 0), r#"session identifier must be a UUID, got "x""#);
    assert_eq!(refused("bad", u, 0), "a copy keeps a whole number of lines, got 0");
    assert_eq!(refused("bad", u, 3), r#"session identifier must be a UUID, got "bad""#);
}

// Rule K3: the copy's wall says its time through fmt_duration.
#[test]
fn the_copy_wall_says_its_time() {
    assert_eq!(wsp_adapters::claude::chain::copy_wall_line(90_000), "the copy of the session had not finished after 1m 30s and was stopped");
}

// Rule K4: a side question's answer: a turn cap with no error is the tool line; a failure with a blank first error
// names its subtype, with none says "no result"; a success keeps its text whole, a failure's text is trimmed.
#[test]
fn a_side_answer_reads_each_ending() {
    let answer = |v: Value| wsp_adapters::claude::aside::aside_answer(v.as_object().unwrap());
    assert_eq!(answer(json!({"type": "result", "subtype": "error_max_turns", "is_error": false, "result": "hello"})), Err(ASIDE_TOOL_LINE.to_string()));
    assert_eq!(answer(json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "errors": ["", "real"]})), Err("claude ended the side question with error_during_execution".to_string()));
    assert_eq!(answer(json!({"type": "result", "is_error": true})), Err("claude ended the side question with no result".to_string()));
    assert_eq!(answer(json!({"type": "result", "subtype": "success", "is_error": false, "result": "  hi \n"})), Ok(AsideAnswer { text: "  hi \n".into(), usage: None }));
    assert_eq!(answer(json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "result": "  boom \n"})), Err("boom".to_string()));
}
