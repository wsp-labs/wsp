// SPDX-License-Identifier: AGPL-3.0-only
//! Ports of packages/adapter-claude/test/catalog.test.ts, session-title.test.ts and conversations.test.ts: what the
//! installed binary reports about itself, the titles Claude Code keeps and makes, and its stored conversations.

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::IndexSet;
use serde_json::{json, Map, Value};

use claude_exec::{collect, FakeExec, StreamOpts};
use wsp_adapters::claude::adapter::AdapterDeps;
use wsp_adapters::claude::catalog::{catalog_probe_command, parse_catalog_probe};
use wsp_adapters::claude::conversations::{keys_for, live_command, origin_of, parse_live, PROGRAMMATIC_ENTRYPOINTS};
use wsp_adapters::claude::landmines::{build_command, build_env, BaseEnv, BuildCommandOptions, ClaudeEnvOptions};
use wsp_adapters::claude::session_title::{
    draft_for_command, parse_draft_for, parse_rename, parse_session_title, parse_title_for, rename_command, session_title_command,
    title_for_command,
};
use wsp_adapters::claude::{create_claude_adapter, ClaudeAdapter};
use wsp_adapters::pending::title_prompt;
use wsp_adapters::port::{
    ConversationEarlier, ConversationLine, ConversationOrigin, ConversationRoad, DraftAsk, EarlierAsk, EarlierRead, Env, ExecFailure,
    ExecFailureKind, ExecOptions, ExecStream, ExecStreamFactory, HarnessCatalogModelProbe, HarnessExec, HarnessStartOptions,
    SessionRenameWrite, StoredConversation, TitleTurn,
};
use wsp_wire::types::shell_quote::ENV_FROM_INPUT;

const SEP: &str = "__WSP_CATALOG_SEP__";
const SESSION: &str = "5b3d3ddb-86d6-47ba-b216-0a510284d8b6";
const ID: &str = "7414323d-e71b-4957-8b56-eefdf6bfa350";
const NEW: &str = "77cae3d9-8a8b-46ee-8ce1-6dafa2a17db9";

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/claude/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn base(pairs: &[(&str, &str)]) -> BaseEnv {
    pairs.iter().map(|(k, v)| (k.to_string(), Some(v.to_string()))).collect()
}

/// An exec factory for a test whose turns must never launch.
struct NoTurns;

impl ExecStreamFactory for NoTurns {
    fn launch(&self, _command: &str, _options: ExecOptions) -> Arc<dyn ExecStream> {
        panic!("no turns here")
    }
}

fn adapter(exec: Arc<dyn ExecStreamFactory>, config_dir: &str, base_env: Option<BaseEnv>, project_dir_name: Option<&str>) -> ClaudeAdapter {
    let mut deps = AdapterDeps::new(exec, config_dir);
    deps.base_env = base_env;
    deps.project_dir_name = project_dir_name.map(String::from);
    create_claude_adapter(deps).unwrap()
}

/// `recorded()` of conversations.test.ts: a factory whose runs print nothing and end at once, keeping every command.
fn recorded() -> Arc<FakeExec> {
    FakeExec::scripted(vec![], StreamOpts::default())
}

type Asked = Arc<Mutex<Vec<(String, Option<Env>)>>>;

/// A HarnessExec answering every command with `stdout`, recording each command and the env it came with.
fn recording_exec(stdout: &str) -> (Asked, Arc<dyn HarnessExec>) {
    let asked: Asked = Default::default();
    let log = asked.clone();
    let stdout = stdout.to_string();
    let exec = move |command: &str, env: Option<&Env>| -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
        log.lock().unwrap().push((command.to_string(), env.cloned()));
        let out = stdout.clone();
        async move { Ok(out) }.boxed_local()
    };
    (asked, Arc::new(exec))
}

/// Writing a stub and spawning a process do not overlap, so a forked child never holds a stub's write fd open
/// when another test execs it.
static FORK: Mutex<()> = Mutex::new(());

fn fork_lock() -> std::sync::MutexGuard<'static, ()> {
    FORK.lock().unwrap_or_else(|e| e.into_inner())
}

/// One folder under the temp dir, unique to a test, removed when the test ends or panics.
struct Scratch(PathBuf);

impl Scratch {
    fn new(test: &str) -> Self {
        let path = std::env::temp_dir().join(format!("wsp-claude-store-{}-{}", std::process::id(), test));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Scratch(path)
    }
    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
    fn text(&self, rel: &str) -> String {
        self.path(rel).to_string_lossy().into_owned()
    }
    /// A config dir with one project folder holding the named session files, as the CLI lays them out.
    fn config_dir(&self, name: &str, files: &[(String, String)]) -> String {
        let root = self.path(name);
        let project = root.join("projects").join("-root-work-proj");
        std::fs::create_dir_all(&project).unwrap();
        for (file, content) in files {
            std::fs::write(project.join(file), content).unwrap();
        }
        root.to_string_lossy().into_owned()
    }
    /// A folder holding a `claude` that runs `script`, to be put first on PATH.
    fn fake_claude(&self, name: &str, script: &str) -> String {
        let dir = self.path(name);
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("claude");
        let _guard = fork_lock();
        std::fs::write(&bin, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        dir.to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One bash line as the guest runs it. With `env` the process holds nothing else; `input` is its stdin, which then closes.
fn bash(command: &str, env: Option<&[(String, String)]>, input: &str) -> String {
    let mut cmd = Command::new("bash");
    cmd.arg("-c").arg(command).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(env) = env {
        cmd.env_clear().envs(env.iter().cloned());
    }
    let mut child = {
        let _guard = fork_lock();
        cmd.spawn().unwrap()
    };
    let _ = child.stdin.take().unwrap().write_all(input.as_bytes());
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "bash exited {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A question as the exec runs it: PATH with `bin` first, nothing else, its variables on its input.
fn ask(command: &str, bin: &str, extra: &[(&str, &str)]) -> String {
    let mut env = vec![("PATH".to_string(), format!("{bin}:{}", std::env::var("PATH").unwrap_or_default()))];
    env.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    bash(command, Some(&env), "")
}

fn title_of(config_dir: &str, session_id: &str) -> Option<String> {
    parse_session_title(&bash(&session_title_command(config_dir, session_id), None, ""))
}

fn rename_to(config_dir: &str, session_id: &str, title: &str) -> SessionRenameWrite {
    parse_rename(&bash(&rename_command(config_dir, session_id, title), None, ""))
}

fn session_lines(config_dir: &str) -> Vec<String> {
    std::fs::read_to_string(format!("{config_dir}/projects/-root-work-proj/{SESSION}.jsonl"))
        .unwrap()
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

fn model<'a>(models: &'a [HarnessCatalogModelProbe], slug: &str) -> &'a HarnessCatalogModelProbe {
    models.iter().find(|m| m.slug == slug).unwrap_or_else(|| panic!("no model {slug}"))
}

// ---------------------------------------------------------------------------------------------------------------
// catalog.test.ts

// TS: catalog.test.ts:28
#[tokio::test]
async fn probe_reads_base_env_off_its_input_and_carries_no_value_in_its_words() {
    tokio::task::LocalSet::new().run_until(probe_reads_base_env_off_its_input_and_carries_no_value_in_its_words_body()).await
}

async fn probe_reads_base_env_off_its_input_and_carries_no_value_in_its_words_body() {
    let adapter = adapter(
        Arc::new(NoTurns),
        "/root/.claude-cfg",
        Some(base(&[("PATH", "/root/.local/bin:/usr/bin"), ("CLAUDECODE", "1"), ("GATEWAY_TOKEN", "tok-x")])),
        None,
    );
    let (asked, exec) = recording_exec("");
    adapter.probe_catalog(exec).await.unwrap();
    let asked = asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1);
    let (cmd, env) = &asked[0];
    let env = env.as_ref().expect("the base env rides the input");
    assert_eq!(env.get("PATH").map(String::as_str), Some("/root/.local/bin:/usr/bin"));
    assert_eq!(env.get("GATEWAY_TOKEN").map(String::as_str), Some("tok-x"));
    assert!(!env.contains_key("CLAUDECODE"));
    assert!(cmd.contains(&format!("unset ${{!CLAUDE_CODE_@}} CLAUDECODE FORCE_CODE_TERMINAL; {ENV_FROM_INPUT}")), "{cmd}");
    let words = cmd.replace(ENV_FROM_INPUT, "");
    for banned in ["PATH=", "tok-x", "export "] {
        assert!(!words.contains(banned), "{banned} in {words}");
    }
}

// TS: catalog.test.ts:40
#[test]
fn probe_asks_for_version_help_and_one_initialize_handshake_never_a_prompt() {
    let cmd = catalog_probe_command(None);
    assert!(cmd.contains("claude --version"), "{cmd}");
    assert!(cmd.contains("claude --help"));
    assert!(cmd.contains("--bare"));
    assert!(cmd.contains("--input-format stream-json"));
    assert!(cmd.contains(r#"\"subtype\":\"initialize\""#));
    assert!(cmd.starts_with("cd ~ && "));
    for banned in ["--permission-mode", "--dangerously-skip-permissions", "--session-id", "--resume"] {
        assert!(!cmd.contains(banned), "{banned} in {cmd}");
    }
}

// TS: catalog.test.ts:51
#[test]
fn probe_runs_under_the_config_dir_with_every_nesting_mark_gone() {
    let scratch = Scratch::new("probe_runs_under_config_dir");
    let bin = scratch.fake_claude("bin", "cat >/dev/null; echo CALL; env");
    let input: String =
        build_env(&ClaudeEnvOptions { base: Some(base(&[("CLAUDE_CONFIG_DIR", "/root/.claude-cfg")])), ..Default::default() })
            .iter()
            .map(|(k, v)| format!("{k}={v}\0"))
            .collect();
    let env = vec![
        ("PATH".to_string(), format!("{bin}:/usr/bin:/bin")),
        ("HOME".to_string(), scratch.text("")),
        ("CLAUDECODE".to_string(), "1".to_string()),
        ("CLAUDE_CODE_ENTRYPOINT".to_string(), "cli".to_string()),
        ("FORCE_CODE_TERMINAL".to_string(), "1".to_string()),
    ];
    let out = bash(&catalog_probe_command(None), Some(&env), &input);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.iter().filter(|l| **l == "CALL").count(), 3);
    assert_eq!(lines.iter().filter(|l| **l == "CLAUDE_CONFIG_DIR=/root/.claude-cfg").count(), 3);
    assert!(
        !lines
            .iter()
            .any(|l| l.starts_with("CLAUDECODE=") || l.starts_with("CLAUDE_CODE_ENTRYPOINT=") || l.starts_with("FORCE_CODE_TERMINAL=")),
        "{out}"
    );
    assert_eq!(lines.iter().filter(|l| **l == "CLAUDE_CODE_AUTO_CONNECT_IDE=0").count(), 3);
}

// TS: catalog.test.ts:72
#[test]
fn parse_catalog_reads_version_models_effort_levels_1m_variants_and_flag_choices() {
    let probe = parse_catalog_probe(&fixture("catalog-probe.txt")).expect("a probe");
    assert_eq!(probe.version.as_deref(), Some("2.1.257"));
    assert_eq!(
        probe.models.iter().map(|m| m.slug.as_str()).collect::<Vec<_>>(),
        ["claude-opus-5", "claude-fable-5-1", "claude-sonnet-5", "claude-haiku-4-5-20251001"]
    );
    assert_eq!(probe.models.iter().map(|m| m.label.as_str()).collect::<Vec<_>>(), ["Opus", "Fable", "Sonnet", "Haiku"]);
    let all = strings(&["low", "medium", "high", "xhigh", "max"]);
    let opus = model(&probe.models, "claude-opus-5");
    assert!(opus.is_default);
    assert_eq!(opus.context_windows, strings(&["200k", "1m"]));
    assert_eq!(opus.efforts, Some(all.clone()));
    let fable = model(&probe.models, "claude-fable-5-1");
    assert!(!fable.is_default);
    assert_eq!(fable.context_windows, strings(&["200k", "1m"]));
    assert_eq!(model(&probe.models, "claude-sonnet-5").context_windows, Vec::<String>::new());
    let haiku = model(&probe.models, "claude-haiku-4-5-20251001");
    assert_eq!(haiku.efforts, Some(vec![]));
    assert_eq!(haiku.context_windows, Vec::<String>::new());
    assert!(probe.models[1].description.as_deref().expect("a description").contains("Most capable"));
    assert_eq!(probe.efforts, all);
    assert_eq!(probe.permission_modes, strings(&["default", "acceptEdits", "auto", "bypassPermissions", "manual", "dontAsk"]));
}

// TS: catalog.test.ts:91
#[test]
fn parse_catalog_reads_the_pinned_versions_recording() {
    let probe = parse_catalog_probe(&fixture("catalog-probe-2.1.296.txt")).expect("a probe");
    assert_eq!(probe.version.as_deref(), Some("2.1.296"));
    assert_eq!(
        probe.models.iter().map(|m| m.slug.as_str()).collect::<Vec<_>>(),
        ["claude-opus-5-5", "claude-fable-5-1", "claude-sonnet-5-5", "claude-haiku-5-5"]
    );
    assert_eq!(probe.models.iter().filter(|m| m.is_default).map(|m| m.slug.as_str()).collect::<Vec<_>>(), ["claude-opus-5-5"]);
    assert_eq!(probe.models.iter().map(|m| m.context_windows.clone()).collect::<Vec<_>>(), vec![Vec::<String>::new(); 4]);
    let all = strings(&["low", "medium", "high", "xhigh", "max"]);
    assert_eq!(probe.models.iter().map(|m| m.efforts.clone()).collect::<Vec<_>>(), vec![Some(all.clone()); 4]);
    assert_eq!(probe.efforts, all);
}

// TS: catalog.test.ts:102
#[test]
fn parse_catalog_is_none_without_initialize_and_tolerates_missing_version_or_help() {
    assert_eq!(parse_catalog_probe(""), None);
    assert_eq!(parse_catalog_probe(&format!("2.1.257 (Claude Code)\n{SEP}\n\n{SEP}\nnot json\n")), None);
    let real = fixture("catalog-probe.txt");
    let init = real.split(SEP).nth(2).unwrap();
    let bare = parse_catalog_probe(&format!("\n{SEP}\n\n{SEP}{init}")).expect("a probe");
    assert_eq!(bare.version, None);
    assert_eq!(bare.efforts, Vec::<String>::new());
    assert_eq!(bare.permission_modes, strings(&["default"]));
    assert_eq!(bare.models.len(), 4);
    assert_eq!(
        bare.models.iter().map(|m| m.slug.as_str()).collect::<Vec<_>>(),
        ["claude-opus-5", "claude-fable-5-1", "claude-sonnet-5", "claude-haiku-4-5-20251001"]
    );
}

// TS: catalog.test.ts:114
#[test]
fn parse_catalog_ignores_hook_chatter_and_a_response_without_models() {
    let real = fixture("catalog-probe.txt");
    let parts: Vec<&str> = real.split(SEP).collect();
    let (version, help, init) = (parts[0], parts[1], parts[2]);
    let noisy = format!("{version}{SEP}{help}{SEP}\n{{\"type\":\"system\",\"subtype\":\"hook_started\"}}\n{}\n", init.trim());
    let probe = parse_catalog_probe(&noisy).expect("a probe");
    assert_eq!(probe.models.len(), 4);
    assert_eq!(
        probe.models.iter().map(|m| m.slug.as_str()).collect::<Vec<_>>(),
        ["claude-opus-5", "claude-fable-5-1", "claude-sonnet-5", "claude-haiku-4-5-20251001"]
    );
    let empty = format!("{version}{SEP}{help}{SEP}\n{{\"type\":\"control_response\",\"response\":{{\"subtype\":\"success\",\"request_id\":\"init\",\"response\":{{\"commands\":[]}}}}}}\n");
    assert_eq!(parse_catalog_probe(&empty), None);
}

// ---------------------------------------------------------------------------------------------------------------
// session-title.test.ts

// TS: session-title.test.ts:49
#[tokio::test]
async fn title_comes_out_of_the_session_file_the_persons_rename_beating_a_later_generated_title() {
    tokio::task::LocalSet::new()
        .run_until(title_comes_out_of_the_session_file_the_persons_rename_beating_a_later_generated_title_body())
        .await
}

async fn title_comes_out_of_the_session_file_the_persons_rename_beating_a_later_generated_title_body() {
    let scratch = Scratch::new("title_from_session_file");
    let dir = scratch.config_dir("cfg", &[(format!("{SESSION}.jsonl"), fixture("session-titles.jsonl"))]);
    assert_eq!(title_of(&dir, SESSION).as_deref(), Some("the sidebar's own name"));
}

// TS: session-title.test.ts:53
#[tokio::test]
async fn title_is_the_generated_one_until_the_person_renames() {
    tokio::task::LocalSet::new().run_until(title_is_the_generated_one_until_the_person_renames_body()).await
}

async fn title_is_the_generated_one_until_the_person_renames_body() {
    let scratch = Scratch::new("title_generated_until_renamed");
    let only = format!("{{\"type\":\"ai-title\",\"aiTitle\":\"Understanding the build\",\"sessionId\":\"{SESSION}\"}}\n");
    let dir = scratch.config_dir("cfg", &[(format!("{SESSION}.jsonl"), only)]);
    assert_eq!(title_of(&dir, SESSION).as_deref(), Some("Understanding the build"));
}

// TS: session-title.test.ts:58
#[tokio::test]
async fn title_is_nothing_without_a_title_record_and_without_the_session() {
    tokio::task::LocalSet::new().run_until(title_is_nothing_without_a_title_record_and_without_the_session_body()).await
}

async fn title_is_nothing_without_a_title_record_and_without_the_session_body() {
    let scratch = Scratch::new("title_nothing");
    let bare = format!("{{\"type\":\"user\",\"sessionId\":\"{SESSION}\",\"message\":{{\"role\":\"user\",\"content\":\"...\"}}}}\n");
    let with_file = scratch.config_dir("with", &[(format!("{SESSION}.jsonl"), bare)]);
    assert_eq!(title_of(&with_file, SESSION), None);
    let empty = scratch.config_dir("empty", &[]);
    assert_eq!(title_of(&empty, SESSION), None);
}

// TS: session-title.test.ts:64
#[test]
fn title_takes_the_last_record_of_each_kind() {
    let lines = [
        format!("{{\"type\":\"custom-title\",\"customTitle\":\"first name\",\"sessionId\":\"{SESSION}\"}}"),
        format!("{{\"type\":\"custom-title\",\"customTitle\":\"second name\",\"sessionId\":\"{SESSION}\"}}"),
    ]
    .join("\n");
    assert_eq!(parse_session_title(&lines).as_deref(), Some("second name"));
}

// TS: session-title.test.ts:72
#[test]
fn title_falls_back_to_generated_on_a_blank_rename_and_reads_nothing_from_non_records() {
    assert_eq!(parse_session_title("{\"type\":\"custom-title\",\"customTitle\":\"   \",\"sessionId\":\"x\"}\n{\"type\":\"ai-title\",\"aiTitle\":\"Understanding the build\",\"sessionId\":\"x\"}").as_deref(), Some("Understanding the build"));
    assert_eq!(parse_session_title("grep: no such file\nnot json at all\n[]\n"), None);
}

// TS: session-title.test.ts:77
#[test]
fn title_command_quotes_the_config_dir_and_session_id() {
    let command = session_title_command("/root/it's here", SESSION);
    assert!(command.contains(r#"'/root/it'\''s here/projects'/*/"#), "{command}");
    assert!(command.contains(&format!("'{SESSION}.jsonl'")), "{command}");
}

// TS: session-title.test.ts:95
#[tokio::test]
async fn title_for_sends_the_question_on_stdin_and_reads_the_print_mode_result() {
    tokio::task::LocalSet::new().run_until(title_for_sends_the_question_on_stdin_and_reads_the_print_mode_result_body()).await
}

async fn title_for_sends_the_question_on_stdin_and_reads_the_print_mode_result_body() {
    let scratch = Scratch::new("title_for_stdin");
    let seen = scratch.text("seen");
    let bin = scratch.fake_claude("bin", &format!("{{ printf '%s\\n' \"$*\"; cat; }} > {seen}\nprintf '{{\"type\":\"result\",\"is_error\":false,\"result\":\"Seed thread titles here\"}}\\n'"));
    let prompt = "Name it. It's a thread's own \"words\"; nothing else.";
    let command = title_for_command(prompt, Some("claude-sonnet-5"), None, None);
    let stdout = ask(&command, &bin, &[]);
    assert_eq!(parse_title_for(&stdout).as_deref(), Some("Seed thread titles here"));
    let text = std::fs::read_to_string(&seen).unwrap();
    let (argv, rest) = text.split_once('\n').unwrap();
    assert_eq!(argv, "-p --safe-mode --output-format json --tools  --no-session-persistence --model claude-sonnet-5");
    assert_eq!(rest, prompt);
}

// TS: session-title.test.ts:107
#[tokio::test]
async fn title_for_and_draft_for_run_under_the_logins_config_dir_after_the_nesting_marks_are_dropped() {
    tokio::task::LocalSet::new()
        .run_until(title_for_and_draft_for_run_under_the_logins_config_dir_after_the_nesting_marks_are_dropped_body())
        .await
}

async fn title_for_and_draft_for_run_under_the_logins_config_dir_after_the_nesting_marks_are_dropped_body() {
    let adapter = adapter(
        Arc::new(NoTurns),
        "/root/.claude-cfg",
        Some(base(&[("CLAUDE_CONFIG_DIR", "/root/it's here"), ("CLAUDE_CODE_ENTRYPOINT", "cli"), ("PATH", "/bin")])),
        None,
    );
    let (asked, exec) = recording_exec("");
    adapter.title_for(TitleTurn { opening: "name it".to_string(), ..Default::default() }, exec.clone()).await.unwrap();
    adapter.draft_for(DraftAsk { prompt_file: "/tmp/asked".to_string(), model: None }, exec).await.unwrap();
    let asked = asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 2);
    for (cmd, env) in asked {
        let env = env.expect("the environment rides the input");
        assert_eq!(env.get("CLAUDE_CONFIG_DIR").map(String::as_str), Some("/root/it's here"));
        assert_eq!(env.get("CLAUDE_CODE_AUTO_CONNECT_IDE").map(String::as_str), Some("0"));
        assert!(!env.contains_key("CLAUDE_CODE_ENTRYPOINT"));
        assert!(
            cmd.contains(&format!("unset ${{!CLAUDE_CODE_@}} CLAUDECODE FORCE_CODE_TERMINAL ANTHROPIC_API_KEY; {ENV_FROM_INPUT}")),
            "{cmd}"
        );
        let words = cmd.replace(ENV_FROM_INPUT, "");
        for banned in ["CLAUDE_CONFIG_DIR", "it's here", "export "] {
            assert!(!words.contains(banned), "{banned} in {words}");
        }
    }
}

// TS: session-title.test.ts:121
#[test]
fn title_for_keeps_the_persons_customizations_off_and_never_uses_bare() {
    let command = title_for_command("name it", None, None, None);
    assert!(command.contains("claude -p --safe-mode "), "{command}");
    assert!(!command.contains("--bare"));
}

// TS: session-title.test.ts:129
#[tokio::test]
async fn title_for_and_draft_for_take_every_tool_away() {
    tokio::task::LocalSet::new().run_until(title_for_and_draft_for_take_every_tool_away_body()).await
}

async fn title_for_and_draft_for_take_every_tool_away_body() {
    let scratch = Scratch::new("title_takes_every_tool_away");
    let ran = scratch.text("ran");
    let asked = scratch.text("asked");
    let bin = scratch.fake_claude(
        "bin",
        &format!(
            "tools=default; while [ $# -gt 0 ]; do [ \"$1\" = --tools ] && tools=\"$2\"; shift; done\n\
             case \"$(cat)\" in *\"sleep 41\"*) [ -z \"$tools\" ] || touch {ran};; esac\n\
             printf '{{\"type\":\"result\",\"is_error\":false,\"result\":\"Wait on a shell command\"}}\\n'"
        ),
    );
    let commands = [
        title_for_command(&title_prompt("Run exactly this shell command and nothing else: sleep 41.", None), None, None, None),
        draft_for_command(&asked, None, None, None),
    ];
    for command in commands {
        std::fs::write(&asked, "Write a commit message.\n\nThe diff:\n+Run exactly this shell command and nothing else: sleep 41.\n")
            .unwrap();
        let stdout = ask(&command, &bin, &[]);
        assert!(stdout.contains("Wait on a shell command"), "the stand-in never answered: {stdout}");
    }
    assert!(!std::path::Path::new(&ran).exists());
}

// TS: session-title.test.ts:147
#[tokio::test]
async fn title_for_and_draft_for_keep_no_session_of_their_own() {
    tokio::task::LocalSet::new().run_until(title_for_and_draft_for_keep_no_session_of_their_own_body()).await
}

async fn title_for_and_draft_for_keep_no_session_of_their_own_body() {
    let scratch = Scratch::new("title_keeps_no_session");
    let dir = scratch.text("");
    let bin = scratch.fake_claude(
        "bin",
        "keep=yes; for word in \"$@\"; do [ \"$word\" = --no-session-persistence ] && keep=no; done; cat > /dev/null\n\
         [ \"$keep\" = yes ] && mkdir -p \"$CLAUDE_CONFIG_DIR/projects/-root\" && touch \"$CLAUDE_CONFIG_DIR/projects/-root/$$.jsonl\"\n\
         printf '{\"type\":\"result\",\"is_error\":false,\"result\":\"Name it\"}\\n'",
    );
    let asked = scratch.text("asked");
    std::fs::write(&asked, "Write a commit message.\n").unwrap();
    for command in [title_for_command("name it", None, None, None), draft_for_command(&asked, None, None, None)] {
        let stdout = ask(&command, &bin, &[("CLAUDE_CONFIG_DIR", &dir)]);
        assert!(stdout.contains("Name it"), "the stand-in never answered: {stdout}");
    }
    assert!(!scratch.path("projects").exists());
}

// TS: session-title.test.ts:164
#[test]
fn title_for_leaves_the_model_to_the_cli_when_the_catalog_named_none() {
    assert!(!title_for_command("name it", None, None, None).contains("--model"));
}

// TS: session-title.test.ts:168
#[test]
fn title_for_reads_no_title_out_of_an_answer_that_is_not_one() {
    assert_eq!(
        parse_title_for(r#"{"type":"result","is_error":false,"result":"Seed thread titles from opening turn"}"#).as_deref(),
        Some("Seed thread titles from opening turn")
    );
    assert_eq!(parse_title_for(r#"{"type":"result","is_error":true,"result":"Credit balance is too low"}"#), None);
    assert_eq!(parse_title_for(r#"{"type":"result","is_error":false,"result":"Sure!\nHere it is"}"#), None);
    assert_eq!(parse_title_for("Invalid API key\n"), None);
    assert_eq!(parse_title_for(""), None);
}

// TS: session-title.test.ts:182
#[tokio::test]
async fn rename_appends_the_record_the_cli_appends_and_the_read_gives_it_back() {
    tokio::task::LocalSet::new().run_until(rename_appends_the_record_the_cli_appends_and_the_read_gives_it_back_body()).await
}

async fn rename_appends_the_record_the_cli_appends_and_the_read_gives_it_back_body() {
    let scratch = Scratch::new("rename_appends");
    let dir = scratch.config_dir(
        "cfg",
        &[(
            format!("{SESSION}.jsonl"),
            format!("{{\"type\":\"ai-title\",\"aiTitle\":\"Understanding the build\",\"sessionId\":\"{SESSION}\"}}\n"),
        )],
    );
    assert_eq!(rename_to(&dir, SESSION, "the name he typed in wsp"), SessionRenameWrite::Written);
    let lines = session_lines(&dir);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], format!("{{\"type\":\"ai-title\",\"aiTitle\":\"Understanding the build\",\"sessionId\":\"{SESSION}\"}}"));
    let record: Value = serde_json::from_str(&lines[1]).unwrap();
    assert_eq!(record, json!({"type": "custom-title", "customTitle": "the name he typed in wsp", "sessionId": SESSION}));
    assert_eq!(title_of(&dir, SESSION).as_deref(), Some("the name he typed in wsp"));
}

// TS: session-title.test.ts:191
#[tokio::test]
async fn rename_keeps_a_name_with_a_quote_a_newline_and_a_percent_sign_as_one_record() {
    tokio::task::LocalSet::new().run_until(rename_keeps_a_name_with_a_quote_a_newline_and_a_percent_sign_as_one_record_body()).await
}

async fn rename_keeps_a_name_with_a_quote_a_newline_and_a_percent_sign_as_one_record_body() {
    let scratch = Scratch::new("rename_quote_newline_percent");
    let dir = scratch.config_dir("cfg", &[(format!("{SESSION}.jsonl"), format!("{{\"type\":\"user\",\"sessionId\":\"{SESSION}\"}}\n"))]);
    assert_eq!(rename_to(&dir, SESSION, "it's 100%\nhere"), SessionRenameWrite::Written);
    let lines = session_lines(&dir);
    assert_eq!(lines.len(), 2);
    let record: Value = serde_json::from_str(&lines[1]).unwrap();
    assert_eq!(record, json!({"type": "custom-title", "customTitle": "it's 100%\nhere", "sessionId": SESSION}));
    assert_eq!(title_of(&dir, SESSION).as_deref(), Some("it's 100%\nhere"));
}

// TS: session-title.test.ts:198
#[tokio::test]
async fn rename_closes_a_half_written_last_line_rather_than_being_swallowed_by_it() {
    tokio::task::LocalSet::new().run_until(rename_closes_a_half_written_last_line_rather_than_being_swallowed_by_it_body()).await
}

async fn rename_closes_a_half_written_last_line_rather_than_being_swallowed_by_it_body() {
    let scratch = Scratch::new("rename_half_written_line");
    let content =
        format!("{{\"type\":\"ai-title\",\"aiTitle\":\"Understanding the build\",\"sessionId\":\"{SESSION}\"}}\n{{\"type\":\"assis");
    let dir = scratch.config_dir("cfg", &[(format!("{SESSION}.jsonl"), content)]);
    assert_eq!(rename_to(&dir, SESSION, "the name"), SessionRenameWrite::Written);
    let lines = session_lines(&dir);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1], "{\"type\":\"assis");
    assert_eq!(title_of(&dir, SESSION).as_deref(), Some("the name"));
}

// TS: session-title.test.ts:204
#[tokio::test]
async fn rename_says_no_session_when_the_glob_answered_and_no_file_of_that_id_is_there() {
    tokio::task::LocalSet::new().run_until(rename_says_no_session_when_the_glob_answered_and_no_file_of_that_id_is_there_body()).await
}

async fn rename_says_no_session_when_the_glob_answered_and_no_file_of_that_id_is_there_body() {
    let scratch = Scratch::new("rename_no_session");
    let dir = scratch.config_dir("cfg", &[]);
    assert_eq!(rename_to(&dir, SESSION, "the name"), SessionRenameWrite::NoSession);
}

// TS: session-title.test.ts:208
#[tokio::test]
async fn rename_says_failed_with_the_machines_own_line_when_the_append_cannot_run() {
    tokio::task::LocalSet::new().run_until(rename_says_failed_with_the_machines_own_line_when_the_append_cannot_run_body()).await
}

async fn rename_says_failed_with_the_machines_own_line_when_the_append_cannot_run_body() {
    let scratch = Scratch::new("rename_append_fails");
    let dir = scratch.config_dir("cfg", &[]);
    std::fs::create_dir_all(format!("{dir}/projects/-root-work-proj/{SESSION}.jsonl")).unwrap();
    match rename_to(&dir, SESSION, "the name") {
        SessionRenameWrite::Failed { error } => assert!(error.contains("Is a directory"), "{error}"),
        other => panic!("expected failed, got {other:?}"),
    }
}

// TS: session-title.test.ts:217
#[tokio::test]
async fn rename_quotes_the_dir_and_id_and_a_name_that_reads_as_shell_lands_as_the_bytes_it_is() {
    tokio::task::LocalSet::new()
        .run_until(rename_quotes_the_dir_and_id_and_a_name_that_reads_as_shell_lands_as_the_bytes_it_is_body())
        .await
}

async fn rename_quotes_the_dir_and_id_and_a_name_that_reads_as_shell_lands_as_the_bytes_it_is_body() {
    let command = rename_command("/root/it's here", SESSION, "x");
    assert!(command.contains(r#"'/root/it'\''s here/projects'/*/"#), "{command}");
    assert!(command.contains(&format!("'{SESSION}.jsonl'")), "{command}");
    let scratch = Scratch::new("rename_quotes_shell");
    let dir = scratch.config_dir("cfg", &[(format!("{SESSION}.jsonl"), format!("{{\"type\":\"user\",\"sessionId\":\"{SESSION}\"}}\n"))]);
    let _ = std::fs::remove_file("/tmp/wsp-rename-never");
    assert_eq!(rename_to(&dir, SESSION, "$(touch /tmp/wsp-rename-never) 'x'"), SessionRenameWrite::Written);
    assert_eq!(title_of(&dir, SESSION).as_deref(), Some("$(touch /tmp/wsp-rename-never) 'x'"));
    assert!(!std::path::Path::new("/tmp/wsp-rename-never").exists());
}

// TS: session-title.test.ts:227
#[test]
fn parse_rename_reads_an_unknown_stdout_as_a_failure_carrying_what_was_said() {
    assert_eq!(parse_rename(""), SessionRenameWrite::Failed { error: "the machine said nothing about the write".to_string() });
    assert_eq!(parse_rename("bash: no such file\n"), SessionRenameWrite::Failed { error: "bash: no such file".to_string() });
    assert_eq!(parse_rename("no-session\n"), SessionRenameWrite::NoSession);
    assert_eq!(parse_rename("wrote\n"), SessionRenameWrite::Written);
}

// TS: session-title.test.ts:236
#[tokio::test]
async fn draft_for_reads_the_question_from_the_file_on_stdin_with_no_tool_and_answers_the_result_whole() {
    tokio::task::LocalSet::new()
        .run_until(draft_for_reads_the_question_from_the_file_on_stdin_with_no_tool_and_answers_the_result_whole_body())
        .await
}

async fn draft_for_reads_the_question_from_the_file_on_stdin_with_no_tool_and_answers_the_result_whole_body() {
    let scratch = Scratch::new("draft_for_reads_file");
    let seen = scratch.text("seen");
    let asked = scratch.text("it's the question.txt");
    std::fs::write(&asked, "Write a commit message.\n\nThe diff:\n+one\n").unwrap();
    let bin = scratch.fake_claude(
        "bin",
        &format!("{{ printf '%s\\n' \"$*\"; cat; }} > {seen}\nprintf '{{\"type\":\"result\",\"is_error\":false,\"result\":\"Round the total once\\\\n\\\\nIt rounded per line.\"}}\\n'"),
    );
    let command = draft_for_command(&asked, Some("claude-haiku-4-5"), None, None);
    let stdout = ask(&command, &bin, &[]);
    assert_eq!(parse_draft_for(&stdout).as_deref(), Some("Round the total once\n\nIt rounded per line."));
    let text = std::fs::read_to_string(&seen).unwrap();
    let (argv, rest) = text.split_once('\n').unwrap();
    assert_eq!(argv, "-p --safe-mode --output-format json --tools  --no-session-persistence --model claude-haiku-4-5");
    assert_eq!(rest, "Write a commit message.\n\nThe diff:\n+one\n");
    assert!(!command.contains("--bare"));
}

// TS: session-title.test.ts:251
#[test]
fn draft_for_reads_no_message_out_of_an_error_or_out_of_nothing() {
    assert_eq!(parse_draft_for(r#"{"type":"result","is_error":true,"result":"Credit balance is too low"}"#), None);
    assert_eq!(parse_draft_for(r#"{"type":"result","is_error":false,"result":"  "}"#), None);
    assert_eq!(parse_draft_for("Invalid API key\n"), None);
}

// ---------------------------------------------------------------------------------------------------------------
// conversations.test.ts

struct Road {
    road: ConversationRoad,
    frames: Arc<Mutex<Vec<Map<String, Value>>>>,
    lines: Asked,
}

impl Road {
    fn frames(&self) -> Value {
        Value::Array(self.frames.lock().unwrap().iter().cloned().map(Value::Object).collect())
    }
}

/// A road whose daemon answers each op as given (a JSON object of op to reply) and whose shell answers every line with
/// `stdout`, recording both.
fn road(answers: Value, stdout: &str) -> Road {
    let frames: Arc<Mutex<Vec<Map<String, Value>>>> = Default::default();
    let (lines, exec) = recording_exec(stdout);
    let seen = frames.clone();
    let ask = move |frame: Map<String, Value>| -> LocalBoxFuture<'static, Result<Map<String, Value>, ExecFailure>> {
        let op = frame.get("op").and_then(Value::as_str).unwrap_or_default().to_string();
        seen.lock().unwrap().push(frame);
        let answer = answers.get(&op).and_then(Value::as_object).cloned();
        async move { answer.ok_or(ExecFailure { kind: ExecFailureKind::Other, message: format!("unknown op: {op}") }) }.boxed_local()
    };
    Road { road: ConversationRoad { exec, ask: Arc::new(ask) }, frames, lines }
}

fn id_set(ids: &[&str]) -> IndexSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

// TS: conversations.test.ts:53
#[tokio::test]
async fn list_comes_off_transcripts_list_under_the_turns_store_in_the_folders_keys_and_the_pinned_one() {
    tokio::task::LocalSet::new()
        .run_until(list_comes_off_transcripts_list_under_the_turns_store_in_the_folders_keys_and_the_pinned_one_body())
        .await
}

async fn list_comes_off_transcripts_list_under_the_turns_store_in_the_folders_keys_and_the_pinned_one_body() {
    let adapter = adapter(recorded(), "/home/dev/.config/claude/", None, Some("-work-acme-lab"));
    let r = road(
        json!({
            "transcripts.list": {
                "rows": [
                    {"id": ID, "cwd": "/work/acme/lab", "branch": "feat/x", "entrypoint": "cli", "title": "lab codewords", "firstPrompt": "Remember ALPHA.", "lastAt": 1_791_673_566_000_u64, "bytes": 209_753},
                    {"id": NEW, "cwd": "/work/acme/lab-wt", "entrypoint": "sdk-cli", "lastAt": 1_791_673_570_000_u64, "bytes": 1_024},
                ],
            },
        }),
        "[]",
    );
    let rows = adapter.conversations().list(strings(&["/work/acme/lab", "/work/acme/lab-wt"]), r.road.clone()).await.unwrap();
    assert_eq!(
        r.frames(),
        json!([{"op": "transcripts.list", "root": "/home/dev/.config/claude/projects", "dirs": ["-work-acme-lab", "-work-acme-lab-wt"], "cwds": ["/work/acme/lab", "/work/acme/lab-wt"]}])
    );
    assert_eq!(
        rows,
        vec![
            StoredConversation {
                id: ID.to_string(),
                title: "lab codewords".to_string(),
                first_prompt: Some("Remember ALPHA.".to_string()),
                branch: Some("feat/x".to_string()),
                cwd: "/work/acme/lab".to_string(),
                last_at: 1_791_673_566_000.0,
                bytes: Some(209_753.0),
                origin: ConversationOrigin::Terminal
            },
            StoredConversation {
                id: NEW.to_string(),
                title: NEW.to_string(),
                first_prompt: None,
                branch: None,
                cwd: "/work/acme/lab-wt".to_string(),
                last_at: 1_791_673_570_000.0,
                bytes: Some(1_024.0),
                origin: ConversationOrigin::Script
            },
        ]
    );
}

// TS: conversations.test.ts:71
#[test]
fn origin_is_a_scripts_where_the_first_entrypoint_is_one_the_picker_leaves_out() {
    assert_eq!(PROGRAMMATIC_ENTRYPOINTS, ["sdk-cli", "sdk-ts", "sdk-py"]);
    for e in PROGRAMMATIC_ENTRYPOINTS {
        assert_eq!(origin_of(Some(e)), ConversationOrigin::Script, "{e}");
    }
    for e in [Some("cli"), Some("claude-vscode"), Some("claude-desktop"), None] {
        assert_eq!(origin_of(e), ConversationOrigin::Terminal, "{e:?}");
    }
}

// TS: conversations.test.ts:77
#[test]
fn keys_sit_under_each_folders_own_key_with_the_pinned_key_once() {
    assert_eq!(keys_for(&strings(&["/work/acme/lab", "/work/acme/lab"]), Some("-work-acme-lab")), strings(&["-work-acme-lab"]));
    assert_eq!(keys_for(&strings(&["/work/acme/lab.v2"]), None), strings(&["-work-acme-lab-v2"]));
}

// TS: conversations.test.ts:82
#[tokio::test]
async fn live_is_open_where_claude_agents_json_names_the_session_under_the_turns_environment() {
    tokio::task::LocalSet::new().run_until(live_is_open_where_claude_agents_json_names_the_session_under_the_turns_environment_body()).await
}

async fn live_is_open_where_claude_agents_json_names_the_session_under_the_turns_environment_body() {
    let adapter =
        adapter(recorded(), "/home/dev/.claude", Some(base(&[("PATH", "/usr/bin"), ("CLAUDE_CONFIG_DIR", "/home/dev/.claude")])), None);
    let agents = json!([{"pid": 30886, "cwd": "/work/acme/lab", "kind": "interactive", "startedAt": 1, "sessionId": ID, "name": "lab codewords", "status": "idle"}]).to_string();
    let r = road(json!({}), &agents);
    let live = adapter.conversations().live(r.road.clone()).expect("live is offered").await;
    assert_eq!(live, Some(id_set(&[ID])));
    let lines = r.lines.lock().unwrap().clone();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, live_command(None));
    assert!(lines[0].0.ends_with("claude agents --json"), "{}", lines[0].0);
    let env = lines[0].1.as_ref().expect("the turn's environment rides the input");
    assert_eq!(env.get("CLAUDE_CONFIG_DIR").map(String::as_str), Some("/home/dev/.claude"));
}

// TS: conversations.test.ts:92
#[tokio::test]
async fn live_is_none_where_the_command_answers_no_list() {
    tokio::task::LocalSet::new().run_until(live_is_none_where_the_command_answers_no_list_body()).await
}

async fn live_is_none_where_the_command_answers_no_list_body() {
    assert_eq!(parse_live("error: unknown command 'agents'"), None);
    assert_eq!(parse_live(r#"{"not":"a list"}"#), None);
    assert_eq!(parse_live("[]"), Some(IndexSet::new()));
    let adapter = adapter(recorded(), "/home/dev/.claude", None, None);
    let exec = |_command: &str, _env: Option<&Env>| -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
        async { Err(ExecFailure { kind: ExecFailureKind::Other, message: "exit 127".to_string() }) }.boxed_local()
    };
    let ask = |_frame: Map<String, Value>| -> LocalBoxFuture<'static, Result<Map<String, Value>, ExecFailure>> {
        async { Ok(Map::new()) }.boxed_local()
    };
    let failing = ConversationRoad { exec: Arc::new(exec), ask: Arc::new(ask) };
    assert_eq!(adapter.conversations().live(failing).expect("live is offered").await, None);
}

// TS: conversations.test.ts:101
#[tokio::test]
async fn earlier_reads_one_conversation_off_transcripts_read_and_is_gone_where_no_folder_holds_it() {
    tokio::task::LocalSet::new()
        .run_until(earlier_reads_one_conversation_off_transcripts_read_and_is_gone_where_no_folder_holds_it_body())
        .await
}

async fn earlier_reads_one_conversation_off_transcripts_read_and_is_gone_where_no_folder_holds_it_body() {
    let adapter = adapter(recorded(), "/home/dev/.claude", None, None);
    let found = road(
        json!({"transcripts.read": {"found": true, "cwd": "/work/acme/lab", "title": "lab codewords", "messages": [{"who": "person", "text": "hi", "at": 1791848679454_u64}, {"who": "tool", "text": "{\"command\":\"ls\"}", "tool": "Bash", "at": 1791848681000_u64}, {"who": "agent", "text": "done"}], "earlier": 4}}),
        "[]",
    );
    let ask = EarlierAsk { cwds: strings(&["/work/acme/lab"]), last: 200, probe: true, tail: None, count: true };
    let read = adapter.conversations().earlier(ID.to_string(), ask.clone(), found.road.clone()).await.unwrap();
    assert_eq!(
        read,
        EarlierRead::Read(ConversationEarlier {
            cwd: Some("/work/acme/lab".to_string()),
            title: Some("lab codewords".to_string()),
            lines: vec![
                ConversationLine { who: "person".to_string(), text: "hi".to_string(), tool: None, at: Some(1791848679454.0) },
                ConversationLine {
                    who: "tool".to_string(),
                    text: "{\"command\":\"ls\"}".to_string(),
                    tool: Some("Bash".to_string()),
                    at: Some(1791848681000.0)
                },
                ConversationLine { who: "agent".to_string(), text: "done".to_string(), tool: None, at: None },
            ],
            earlier: 4.0,
        })
    );
    assert_eq!(
        found.frames(),
        json!([{"op": "transcripts.read", "root": "/home/dev/.claude/projects", "dirs": ["-work-acme-lab"], "session": ID, "last": 200}])
    );
    let missing = road(json!({"transcripts.read": {"found": false, "messages": [], "earlier": 0}}), "[]");
    assert_eq!(adapter.conversations().earlier(ID.to_string(), ask, missing.road.clone()).await.unwrap(), EarlierRead::Gone);
}

// TS: conversations.test.ts:114
#[tokio::test]
async fn earlier_asks_the_daemon_for_the_end_of_the_file_alone_where_the_read_names_a_tail() {
    tokio::task::LocalSet::new().run_until(earlier_asks_the_daemon_for_the_end_of_the_file_alone_where_the_read_names_a_tail_body()).await
}

async fn earlier_asks_the_daemon_for_the_end_of_the_file_alone_where_the_read_names_a_tail_body() {
    let adapter = adapter(recorded(), "/home/dev/.claude", None, None);
    let found = road(json!({"transcripts.read": {"found": true, "messages": [{"who": "agent", "text": "done"}], "earlier": 0}}), "[]");
    let ask = EarlierAsk { cwds: strings(&["/work/acme/lab"]), last: 4, probe: false, tail: Some(262144), count: true };
    let read = adapter.conversations().earlier(ID.to_string(), ask, found.road.clone()).await.unwrap();
    assert_eq!(
        read,
        EarlierRead::Read(ConversationEarlier {
            cwd: None,
            title: None,
            lines: vec![ConversationLine { who: "agent".to_string(), text: "done".to_string(), tool: None, at: None }],
            earlier: 0.0
        })
    );
    assert_eq!(
        found.frames(),
        json!([{"op": "transcripts.read", "root": "/home/dev/.claude/projects", "dirs": ["-work-acme-lab"], "session": ID, "last": 4, "tail": 262144}])
    );
}

// TS: conversations.test.ts:121
#[tokio::test]
async fn list_of_more_than_64_folders_asks_in_frames_of_at_most_64_and_keeps_the_newest_of_an_id() {
    tokio::task::LocalSet::new()
        .run_until(list_of_more_than_64_folders_asks_in_frames_of_at_most_64_and_keeps_the_newest_of_an_id_body())
        .await
}

async fn list_of_more_than_64_folders_asks_in_frames_of_at_most_64_and_keeps_the_newest_of_an_id_body() {
    let cwds: Vec<String> = (0..70).map(|i| format!("/work/acme/lab-t{i}")).collect();
    let adapter = adapter(recorded(), "/home/dev/.claude", None, Some("-work-acme-lab"));
    let frames: Arc<Mutex<Vec<Map<String, Value>>>> = Default::default();
    let seen = frames.clone();
    let ask = move |frame: Map<String, Value>| -> LocalBoxFuture<'static, Result<Map<String, Value>, ExecFailure>> {
        let has_t3 = frame.get("cwds").and_then(Value::as_array).is_some_and(|c| c.iter().any(|v| v == "/work/acme/lab-t3"));
        seen.lock().unwrap().push(frame);
        let last_at = if has_t3 { 9 } else { 5 };
        let reply = json!({"rows": [{"id": ID, "cwd": "/work/acme/lab-t3", "lastAt": last_at, "bytes": 1}]});
        async move { Ok(reply.as_object().cloned().unwrap()) }.boxed_local()
    };
    let exec = |command: &str, _env: Option<&Env>| -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
        assert!(command.contains("'-work-acme-lab-t69'"), "{command}");
        async { Ok("-work-acme-lab\n-work-acme-lab-t3\nnot-asked\n".to_string()) }.boxed_local()
    };
    let road = ConversationRoad { exec: Arc::new(exec), ask: Arc::new(ask) };
    let rows = adapter.conversations().list(cwds, road).await.unwrap();
    assert_eq!(rows.iter().map(|r| (r.id.as_str(), r.last_at)).collect::<Vec<_>>(), [(ID, 9.0)]);
    let shapes: Vec<Value> = frames.lock().unwrap().iter().map(|f| json!([f["dirs"], f["cwds"].as_array().unwrap().len()])).collect();
    assert_eq!(Value::Array(shapes), json!([[["-work-acme-lab-t3", "-work-acme-lab"], 64], [["-work-acme-lab-t3", "-work-acme-lab"], 6]]));
}

// TS: conversations.test.ts:145
#[test]
fn copy_is_a_new_session_the_cli_starts_off_the_original_with_fork_session() {
    let line =
        build_command(&BuildCommandOptions { session_id: Some(NEW.to_string()), copy_of: Some(ID.to_string()), ..Default::default() })
            .unwrap();
    assert!(line.contains(&format!("--session-id {NEW} --resume {ID} --fork-session")), "{line}");
    let both = build_command(&BuildCommandOptions { resume: Some(ID.to_string()), copy_of: Some(NEW.to_string()), ..Default::default() })
        .unwrap_err();
    assert!(both.0.contains("a copy opens a new session"), "{both}");
    let not_an_id =
        build_command(&BuildCommandOptions { session_id: Some(NEW.to_string()), copy_of: Some("x".to_string()), ..Default::default() })
            .unwrap_err();
    assert!(not_an_id.0.contains("a copy opens a new session"), "{not_an_id}");
}

// TS: conversations.test.ts:152
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_runs_under_an_id_of_wsps_with_no_name_and_no_saved_spend_read_off_the_original() {
    tokio::task::LocalSet::new().run_until(copy_runs_under_an_id_of_wsps_with_no_name_and_no_saved_spend_read_off_the_original_body()).await
}

async fn copy_runs_under_an_id_of_wsps_with_no_name_and_no_saved_spend_read_off_the_original_body() {
    let exec = recorded();
    let adapter = adapter(exec.clone(), "/home/dev/.claude", None, None);
    let (_events, on_event) = collect();
    let session = adapter
        .start(HarnessStartOptions {
            prompt: "go on".to_string(),
            resume: Some(ID.to_string()),
            copy: true,
            on_event: Some(on_event),
            ..Default::default()
        })
        .unwrap();
    session.finished().await;
    let command = exec.calls()[0].command.clone();
    let local_id = session.local_id();
    assert!(command.contains(&format!("--session-id {local_id} --resume {ID} --fork-session")), "{command}");
    assert_ne!(local_id, ID);
    assert!(!command.contains("--name"), "{command}");
    assert!(!command.contains(&format!("{ID}.jsonl")), "{command}");
}

// TS: conversations.test.ts:165
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_resume_resumes_the_original_by_its_id_and_never_forks() {
    tokio::task::LocalSet::new().run_until(plain_resume_resumes_the_original_by_its_id_and_never_forks_body()).await
}

async fn plain_resume_resumes_the_original_by_its_id_and_never_forks_body() {
    let exec = recorded();
    let adapter = adapter(exec.clone(), "/home/dev/.claude", None, None);
    let (_events, on_event) = collect();
    adapter
        .start(HarnessStartOptions {
            prompt: "go on".to_string(),
            resume: Some(ID.to_string()),
            on_event: Some(on_event),
            ..Default::default()
        })
        .unwrap()
        .finished()
        .await;
    let command = exec.calls()[0].command.clone();
    assert!(command.contains(&format!("--resume {ID}")), "{command}");
    assert!(!command.contains("--fork-session"), "{command}");
    assert!(!command.contains("--name"), "{command}");
}

// Rule S1: the rename script carries real line feeds where the TS template does, not `\n` escapes.
#[test]
fn the_rename_script_is_the_ts_bytes() {
    assert_eq!(
        rename_command("/c", "sid", "a \"q\"\nb"),
        "f=$(ls -1td '/c/projects'/*/'sid.jsonl' 2>/dev/null | head -n 1); [ -n \"$f\" ] || { echo no-session; exit 0; }; [ -z \"$(tail -c 1 \"$f\")\" ] || printf '\n' >> \"$f\" 2>/dev/null; e=$({ printf '%s\n' '{\"type\":\"custom-title\",\"customTitle\":\"a \\\"q\\\"\\nb\",\"sessionId\":\"sid\"}' >> \"$f\"; } 2>&1) && echo wrote || printf '%s %s\n' failed \"$e\""
    );
}

// Rule S2: each line is trimmed; `wrote` anywhere is written; the first line opening `failed ` gives the rest; else
// every non-blank line joined by `; `.
#[test]
fn a_rename_reads_its_answer_line_by_line() {
    let failed = |e: &str| SessionRenameWrite::Failed { error: e.into() };
    let cases = [
        ("wrote\nno-session", SessionRenameWrite::Written),
        ("no-session\nwrote", SessionRenameWrite::Written),
        ("  wrote  \r\n", SessionRenameWrite::Written),
        ("Wrote", failed("Wrote")),
        ("failed  two spaces", failed(" two spaces")),
        ("wrote extra\nno-session extra", failed("wrote extra; no-session extra")),
        ("a\n\nb\n", failed("a; b")),
        ("\n\n", failed("the machine said nothing about the write")),
        ("  failed trimmed lead\n", failed("trimmed lead")),
        ("failed \n", failed("failed")),
        ("failed x\nfailed y", failed("x")),
    ];
    for (stdout, want) in cases {
        assert_eq!(parse_rename(stdout), want, "{stdout:?}");
    }
}

// Rule S3: a title is blank by JS trim: U+FEFF alone is blank, so the generated title stands; U+0085 is a title.
#[test]
fn a_title_is_blank_by_javascript_trim() {
    let custom = |t: &str| {
        [json!({"type": "custom-title", "customTitle": t}).to_string(), json!({"type": "ai-title", "aiTitle": "gen"}).to_string()]
            .join("\n")
    };
    assert_eq!(parse_session_title(&custom("\u{FEFF}")), Some("gen".into()));
    assert_eq!(parse_session_title(&custom("\u{85}")), Some("\u{85}".into()));
}

// Rule S4: a folder key is each UTF-16 unit outside [A-Za-z0-9] as `-`, so an emoji is two; keys are deduplicated in
// first-seen order and a pinned "" is kept.
#[test]
fn folder_keys_count_utf16_units() {
    let cwds = ["/work/é", "/w/😀", "/", "", "/a b/c.d_e-f", "/a-b", "/a b", "/x/ü/日本"].map(String::from).to_vec();
    assert_eq!(keys_for(&cwds, Some("")), vec!["-work--", "-w---", "-", "", "-a-b-c-d-e-f", "-a-b", "-x-----"]);
}

// Rule S5: runs of at most 64, in order.
#[test]
fn runs_hold_at_most_64() {
    let sizes =
        |n: usize| wsp_adapters::claude::conversations::runs_of(&(0..n).collect::<Vec<_>>()).iter().map(Vec::len).collect::<Vec<_>>();
    assert_eq!([0, 1, 64, 65, 128, 129].map(sizes).to_vec(), vec![vec![], vec![1], vec![64], vec![64, 1], vec![64, 64], vec![64, 64, 1]]);
}

// Rule S6: the live list is a JSON array after a trim, its non-empty string sessionIds once each in first-seen order;
// anything else is None, an array of none an empty set.
#[test]
fn the_live_list_reads_only_a_whole_array() {
    let read = |s: &str| parse_live(s).map(|set| set.into_iter().collect::<Vec<_>>());
    assert_eq!(
        read(r#"[{"sessionId":"a"},{"sessionId":""},{"sessionId":1},null,[1],{"sessionId":"a"},"x",{"sessionId":"b"}]"#),
        Some(vec!["a".to_string(), "b".to_string()])
    );
    for none in ["", "null", "{}", "[{\"sessionId\":\"a\"}]\ntrailing"] {
        assert_eq!(read(none), None, "{none:?}");
    }
    assert_eq!(read("  []  "), Some(vec![]));
    assert_eq!(read("[1,2]"), Some(vec![]));
}

// Rule S7: the presence check writes `\n` as two characters for printf to read, unlike the rename script.
#[test]
fn the_presence_check_is_the_ts_bytes() {
    assert_eq!(
        wsp_adapters::claude::conversations::present_command("/r", &["a".to_string(), "b".to_string()], None),
        r#"cd '/r' 2>/dev/null && for d in 'a' 'b'; do [ -d "$d" ] && printf '%s\n' "$d"; done; true"#
    );
}

// Rule S8: the first result that is no error decides the title, even where it gives none.
#[test]
fn the_first_good_result_decides_the_title() {
    let lines = [
        json!({"type": "result", "is_error": false, "result": "Sure!\nHere"}).to_string(),
        json!({"type": "result", "is_error": false, "result": "Good title"}).to_string(),
    ]
    .join("\n");
    assert_eq!(parse_title_for(&lines), None);
}
