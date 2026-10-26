// SPDX-License-Identifier: AGPL-3.0-only
//! `machines/brokered.rs` against the TS oracle's `brokered.json`, and the `sh` cases of channels.test.ts that run
//! the written terminal line. Spec: spec/brokered.md.

mod common;

use std::cell::RefCell;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_adapters::claude::landmines::{BrokerKind, BrokeredSignIn};
use wsp_adapters::codex::command::{AccountModels, BrokerMode, CodexBroker};
use wsp_adapters::pending::CodexClaims;
use wsp_adapters::port::{AttachRunOptions, Attached, Env, ExecFailure, ExecOptions, ExecStream, ExecStreamFactory, SharedFuture, WriteOutcome};
use wsp_machines::machines::brokered::*;
use wsp_wire::wire::runtime::BrokerStatus;
use wsp_words::exit::Kind;

fn rows(name: &str) -> Vec<Value> {
    common::fixture("brokered")[name].as_array().unwrap_or_else(|| panic!("fixture member {name} is not an array")).clone()
}

fn pairs_of(v: &Value) -> Vec<(String, String)> {
    v.as_array().expect("pairs").iter().map(|p| (p[0].as_str().expect("key").to_string(), p[1].as_str().expect("value").to_string())).collect()
}

fn object_pairs(v: &Value) -> Vec<(String, String)> {
    v.as_object().expect("object").iter().map(|(k, x)| (k.clone(), x.as_str().expect("value").to_string())).collect()
}

fn ordered(m: IndexMap<String, String>) -> Vec<(String, String)> {
    m.into_iter().collect()
}

fn fixture_program(other: &str, own: &str) -> String {
    rows("otherAgentProgram")
        .iter()
        .find(|r| r["other"] == other && r["own"] == own)
        .unwrap_or_else(|| panic!("no fixture row for {other} on {own}"))["out"]
        .as_str()
        .expect("program")
        .to_string()
}

/// The constants, then the folder they name: a constant alone passes against the stub and tests nothing (rule 13.12).
#[test]
fn constants_match_the_ts() {
    let c = &common::fixture("brokered")["constants"];
    assert_eq!(BROKERED_GH_DIR, c["BROKERED_GH_DIR"].as_str().unwrap());
    assert_eq!(BROKERED_BIN_DIR, c["BROKERED_BIN_DIR"].as_str().unwrap());
    assert_eq!(TERMINAL_ENV, c["TERMINAL_ENV"].as_str().unwrap());
    assert_eq!(TERMINAL_PROFILE, c["TERMINAL_PROFILE"].as_str().unwrap());
    assert_eq!(terminal_bin_of("codex"), format!("{BROKERED_BIN_DIR}/codex"), "the folder of an agent is under the bin folder");
}

#[test]
fn terminal_bin_of_matches_the_ts() {
    for row in rows("terminalBinOf") {
        let own = row["own"].as_str().unwrap();
        assert_eq!(terminal_bin_of(own), row["out"].as_str().unwrap(), "own {own:?}");
    }
}

#[test]
fn other_agent_program_matches_the_ts_for_every_pair() {
    for row in rows("otherAgentProgram") {
        let (other, own) = (row["other"].as_str().unwrap(), row["own"].as_str().unwrap());
        assert_eq!(other_agent_program(other, own), row["out"].as_str().unwrap(), "{other:?} on {own:?}");
    }
}

#[test]
fn terminal_line_matches_the_ts_byte_for_byte() {
    for row in rows("terminalLine") {
        let own = row["own"].as_str().unwrap();
        let program = row["program"].as_str();
        assert_eq!(terminal_line(own, program), row["out"].as_str().unwrap(), "own {own:?} program {program:?}");
    }
}

#[test]
fn brokered_env_matches_the_ts_in_key_order() {
    for row in rows("brokeredEnv") {
        let repo = row["repo"].as_object().map(|r| RepoOfRecord {
            owner: r["owner"].as_str().unwrap().to_string(),
            name: r["name"].as_str().unwrap().to_string(),
            public: r["public"].as_bool().unwrap(),
        });
        let direct: Vec<String> = row["direct"].as_array().unwrap().iter().map(|d| d.as_str().unwrap().to_string()).collect();
        let got = ordered(brokered_env(row["token"].as_str().unwrap(), repo.as_ref(), &direct));
        assert_eq!(got, pairs_of(&row["out"]), "repo {:?} direct {direct:?}", row["repo"]);
    }
}

#[test]
fn placeholders_of_matches_the_ts_for_every_status() {
    let mut checked = 0;
    for row in rows("placeholdersOf") {
        let status = if row["status"].is_null() {
            None
        } else {
            // A status whose words BrokerStatus does not carry ({"claude": "nonsense"}) cannot be built in Rust: the
            // wire type refuses it before this function is reached, so that row has no Rust counterpart.
            match serde_json::from_value::<BrokerStatus>(row["status"].clone()) {
                Ok(s) => Some(s),
                Err(_) => continue,
            }
        };
        let got = placeholders_of(status.as_ref(), "tok");
        let want = &row["out"];
        assert_eq!(got.claude_token, want["claude"]["token"].as_str().unwrap(), "status {}", row["status"]);
        let kind = match want["claude"]["kind"].as_str().unwrap() {
            "token" => ClaudePlaceholderKind::Token,
            "key" => ClaudePlaceholderKind::Key,
            "gateway" => ClaudePlaceholderKind::Gateway,
            other => panic!("fixture kind {other}"),
        };
        assert_eq!(got.claude_kind, kind, "status {}", row["status"]);
        assert_eq!(ordered(got.vault), object_pairs(&want["vault"]), "status {}", row["status"]);
        checked += 1;
    }
    assert!(checked > 0, "no status of the fixture was built");
}

#[test]
fn codex_broker_of_matches_the_ts() {
    for row in rows("codexBrokerOf") {
        // A status the wire's enum cannot hold (a claude kind of "nonsense") never reaches the port; the TS answers none for it.
        let status = if row["status"].is_null() {
            None
        } else {
            match serde_json::from_value::<BrokerStatus>(row["status"].clone()) {
                Ok(status) => Some(status),
                Err(_) => continue,
            }
        };
        let claims = row["claims"].as_object().map(|c| CodexClaims { account_id: c["accountId"].as_str().unwrap().to_string(), ..Default::default() });
        let models_value = row["models"].clone();
        let models: Option<AccountModels> = if models_value.is_null() {
            None
        } else {
            Some(Arc::new(move |_account: String| {
                let answer = models_value.clone();
                async move { Ok(Some(answer)) }.boxed()
            }))
        };
        let got = codex_broker_of(status.as_ref(), claims.as_ref(), models);
        let label = format!("status {} claims {} models {}", row["status"], row["claims"], row["models"]);
        let want = &row["out"];
        match (got, want.is_null()) {
            (None, true) => {}
            (None, false) => panic!("{label}: wanted {want}, got none"),
            (Some(_), true) => panic!("{label}: wanted none, got a broker"),
            (Some(broker), false) => match (broker.mode, want["mode"].as_str().unwrap()) {
                (BrokerMode::Apikey, "apikey") => {}
                (BrokerMode::Chatgpt { claims, models }, "chatgpt") => {
                    assert_eq!(claims.account_id, want["claims"]["accountId"].as_str().unwrap(), "{label}");
                    let wants_models = want.get("models").is_some_and(|m| !m.is_null());
                    assert_eq!(models.is_some(), wants_models, "{label}: models");
                }
                (_, mode) => panic!("{label}: wanted mode {mode}"),
            },
        }
    }
}

#[test]
fn unbrokered_refusal_matches_the_ts_words_and_kind() {
    for row in rows("unbrokeredRefusal") {
        let harness = row["harness"].as_str().unwrap();
        match (unbrokered_refusal(harness), row["out"].as_str()) {
            (None, None) => assert!(row["kind"].is_null(), "harness {harness:?}"),
            (Some(err), Some(message)) => {
                assert_eq!(err.message, message, "harness {harness:?}");
                assert_eq!(row["kind"], "usage", "fixture kind for {harness:?}");
                assert_eq!(err.kind, Some(Kind::Usage), "harness {harness:?}");
            }
            (got, want) => panic!("harness {harness:?}: got {:?}, wanted {want:?}", got.map(|e| e.message)),
        }
    }
}

type Read = Box<dyn Fn(Option<&str>) -> Option<LaunchRead>>;

fn launch(vault: IndexMap<String, String>, claude: Option<BrokeredSignIn>, codex: Option<CodexBroker>) -> LaunchRead {
    LaunchRead { vault, claude, codex }
}

/// The oracle's `reads` object, by its names.
fn read_named(name: &str) -> Read {
    match name {
        "none" => Box::new(|_| None),
        "claude" => Box::new(|token| token.map(|t| launch(IndexMap::new(), Some(BrokeredSignIn { token: t.to_string(), kind: BrokerKind::Token }), None))),
        "claudeKey" => Box::new(|token| token.map(|t| launch(IndexMap::new(), Some(BrokeredSignIn { token: t.to_string(), kind: BrokerKind::Key }), None))),
        "codexChatgpt" => Box::new(|_| {
            let mode = BrokerMode::Chatgpt { claims: CodexClaims { account_id: "acct-acme".into(), ..Default::default() }, models: None };
            Some(launch(IndexMap::new(), None, Some(CodexBroker { mode, providers: None })))
        }),
        "codexKey" => Box::new(|token| {
            let vault = match token {
                None => IndexMap::new(),
                Some(t) => IndexMap::from([("OPENAI_API_KEY".to_string(), format!("sk-wsp-{t}"))]),
            };
            Some(launch(vault, None, Some(CodexBroker { mode: BrokerMode::Apikey, providers: None })))
        }),
        "codexOnlyWithToken" => Box::new(|token| token.map(|_| launch(IndexMap::new(), None, Some(CodexBroker { mode: BrokerMode::Apikey, providers: None })))),
        other => panic!("no read named {other}"),
    }
}

#[test]
fn terminal_sign_in_matches_the_ts_for_every_read() {
    for row in rows("terminalSignIn") {
        let (own, read_name) = (row["own"].as_str().unwrap(), row["read"].as_str().unwrap());
        let label = format!("own {own} read {read_name}");
        let read = read_named(read_name);
        let got = terminal_sign_in(own, &*read, "tok", common::T0).unwrap_or_else(|e| panic!("{label}: refused: {e}"));
        let want = &row["out"];
        if want.is_null() {
            assert_eq!(got, None, "{label}");
            continue;
        }
        let got = got.unwrap_or_else(|| panic!("{label}: wanted a sign-in, got none"));
        if own == "codex" {
            // The program text comes from wsp-adapters' codex_for_shell, which has its own tests.
            assert!(got.env.contains_key("CODEX_REFRESH_TOKEN_URL_OVERRIDE"), "{label}: env {:?}", got.env);
            for (k, v) in want["env"].as_object().unwrap() {
                assert_eq!(got.env.get(k).map(String::as_str), v.as_str(), "{label}: env {k}");
            }
            assert!(got.program.is_some(), "{label}: program");
        } else {
            assert_eq!(ordered(got.env), object_pairs(&want["env"]), "{label}");
            assert_eq!(got.program, None, "{label}: program");
        }
    }
}

#[test]
fn terminal_sign_in_reads_codex_without_a_token_first() {
    let calls_of = |own: &str, read_name: &str| -> (Vec<Option<String>>, Option<TerminalSignIn>) {
        let calls = RefCell::new(Vec::new());
        let inner = read_named(read_name);
        let read = |token: Option<&str>| {
            calls.borrow_mut().push(token.map(str::to_string));
            inner(token)
        };
        let got = terminal_sign_in(own, &read, "tok", common::T0).expect("no refusal");
        (calls.into_inner(), got)
    };
    let (calls, got) = calls_of("codex", "codexKey");
    assert_eq!(calls, vec![None, Some("tok".to_string())]);
    assert!(got.is_some());
    // A host holding no Codex login it has read answers nothing before the token is ever offered.
    let (calls, got) = calls_of("codex", "codexOnlyWithToken");
    assert_eq!(calls, vec![None]);
    assert_eq!(got, None);
    let (calls, _) = calls_of("claude", "claude");
    assert_eq!(calls, vec![Some("tok".to_string())]);
    let (calls, got) = calls_of("cursor", "codexKey");
    assert_eq!(calls, Vec::<Option<String>>::new());
    assert_eq!(got, None);
}

struct Quiet;

impl ExecStream for Quiet {
    fn lines(&self) -> futures::stream::LocalBoxStream<'static, Result<String, ExecFailure>> {
        futures::StreamExt::boxed_local(futures::stream::empty())
    }
    fn teardown(&self) {}
    fn kill(&self) {}
    fn write(&self, _line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        async { Ok(WriteOutcome::Written) }.boxed_local()
    }
    fn close_input(&self) {}
    fn exited(&self) -> SharedFuture<Option<i32>> {
        async { Some(0) }.boxed_local().shared()
    }
}

/// A road that records what it was asked to launch; with `extras` it has `attach` and `sweep`, as the oracle's does.
struct Road {
    launched: RefCell<Vec<(String, Env)>>,
    extras: bool,
}

impl Road {
    fn new(extras: bool) -> Arc<Road> {
        Arc::new(Road { launched: RefCell::new(Vec::new()), extras })
    }
}

impl ExecStreamFactory for Road {
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        self.launched.borrow_mut().push((command.to_string(), options.env));
        Arc::new(Quiet)
    }
    fn attach(&self, _run: &str, _options: AttachRunOptions) -> Option<LocalBoxFuture<'static, Result<Attached, ExecFailure>>> {
        self.extras.then(|| async { Ok(Attached::Gone) }.boxed_local())
    }
    fn sweep(&self, _keep: &[String]) -> Option<LocalBoxFuture<'static, Result<Vec<String>, ExecFailure>>> {
        self.extras.then(|| async { Ok(vec!["swept".to_string()]) }.boxed_local())
    }
}

fn with_env(pairs: &[(&str, &str)]) -> ExecOptions {
    ExecOptions { env: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), ..Default::default() }
}

#[test]
fn with_gh_door_prefixes_only_the_exact_gh_dir() {
    common::run_local(async {
        let fixture = common::fixture("brokered");
        let launched = fixture["withGhDoor"]["launched"].as_array().unwrap();
        let road = Road::new(true);
        let wrapped = with_gh_door(road.clone());
        let gh_dir = launched[0]["env"]["GH_CONFIG_DIR"].as_str().unwrap();
        let trailing = launched[1]["env"]["GH_CONFIG_DIR"].as_str().unwrap();
        wrapped.launch("echo hi", with_env(&[("GH_CONFIG_DIR", gh_dir)]));
        wrapped.launch("echo hi", with_env(&[("GH_CONFIG_DIR", trailing)]));
        wrapped.launch("echo hi", with_env(&[]));
        let got = road.launched.borrow().clone();
        assert_eq!(got.len(), launched.len());
        for (i, (command, env)) in got.iter().enumerate() {
            assert_eq!(command, launched[i]["command"].as_str().unwrap(), "launch {i}");
            assert_eq!(env.clone().into_iter().collect::<Vec<_>>(), object_pairs(&launched[i]["env"]), "launch {i}: options pass through untouched");
        }
    })
}

#[test]
fn with_gh_door_keeps_attach_and_sweep_only_where_the_road_has_them() {
    common::run_local(async {
        let fixture = common::fixture("brokered");
        let door = &fixture["withGhDoor"];
        assert_eq!((door["attach"].as_str(), door["sweep"].as_str(), door["bareAttach"].as_bool(), door["bareSweep"].as_bool()), (Some("function"), Some("function"), Some(false), Some(false)));
        let both = with_gh_door(Road::new(true));
        let attach = both.attach("run1", AttachRunOptions { input: false, started_at: 0.0, from: None }).expect("attach kept");
        assert!(matches!(attach.await, Ok(Attached::Gone)), "attach is the inner road's");
        let sweep = both.sweep(&[]).expect("sweep kept");
        assert_eq!(sweep.await.ok(), Some(vec!["swept".to_string()]), "sweep is the inner road's");
        let bare = with_gh_door(Road::new(false));
        assert!(bare.attach("run1", AttachRunOptions { input: false, started_at: 0.0, from: None }).is_none(), "no attach where the road has none");
        assert!(bare.sweep(&[]).is_none(), "no sweep where the road has none");
    })
}

fn run_line(dir: &Path, line: &str) {
    let rewritten = line.replace(BROKERED_BIN_DIR, dir.join("bin").to_str().unwrap()).replace(TERMINAL_PROFILE, dir.join("profile.sh").to_str().unwrap());
    let ran = Command::new("sh").arg("-c").arg(&rewritten).output().expect("sh runs");
    assert!(ran.status.success(), "the line failed: {}", String::from_utf8_lossy(&ran.stderr));
}

/// `typed` in a login shell marked `own` after the profile ran with the PATH set outright, as the image's profile
/// leaves it: stdout, stderr and the exit code.
fn typed(dir: &Path, typed: &str, own: Option<&str>) -> (String, String, Option<i32>) {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(format!("PATH=/usr/bin:/bin; . \"{}/profile.sh\"; {typed}", dir.display())).env_clear().stdin(Stdio::null());
    if let Some(own) = own {
        cmd.env(TERMINAL_ENV, own);
    }
    let ran = cmd.output().expect("sh runs");
    (String::from_utf8_lossy(&ran.stdout).replace(dir.to_str().unwrap(), "<dir>"), String::from_utf8_lossy(&ran.stderr).into_owned(), ran.status.code())
}

const CODEX_PROGRAM: &str = "#!/bin/sh\necho codex-program\n";

#[test]
fn the_profile_puts_codex_first_only_in_a_shell_marked_codex() {
    let dir = tempfile::TempDir::new().unwrap();
    run_line(dir.path(), &terminal_line("codex", Some(CODEX_PROGRAM)));
    let found = |own: Option<&str>| typed(dir.path(), "command -v codex || true", own).0.trim().to_string();
    assert_eq!(found(Some("codex")), "<dir>/bin/codex/codex", "typed() writes the folder as <dir>");
    // The profile adds nothing for any other shell: they find what the box itself holds (a codex in /usr/bin where one is installed).
    let boxs = typed(dir.path(), "PATH=/usr/bin:/bin command -v codex || true", None).0.trim().to_string();
    assert_eq!(found(Some("claude")), boxs);
    assert_eq!(found(None), boxs);
}

#[test]
fn codex_typed_on_a_claude_thread_says_why_it_cannot_sign_in() {
    let dir = tempfile::TempDir::new().unwrap();
    let line = terminal_line("claude", None);
    assert!(!line.contains("openai_base_url"));
    run_line(dir.path(), &line);
    assert_eq!(
        typed(dir.path(), "codex exec \"say ok\"", Some("claude")),
        (
            String::new(),
            "wsp: Codex cannot sign in on this machine. This terminal's thread runs Claude Code, and your computer signs in only that agent here. Start a thread on Codex to use it.\n".to_string(),
            Some(1)
        )
    );
}

#[test]
fn a_second_line_for_another_agent_keeps_both_folders() {
    let dir = tempfile::TempDir::new().unwrap();
    run_line(dir.path(), &terminal_line("claude", None));
    run_line(dir.path(), &terminal_line("codex", Some(CODEX_PROGRAM)));
    let read = |rel: &str| std::fs::read_to_string(dir.path().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    assert_eq!(read("bin/claude/codex"), fixture_program("codex", "claude"));
    assert!(!dir.path().join("bin/claude/claude").exists(), "claude's own file is removed where it has no program");
    assert_eq!(read("bin/codex/codex"), CODEX_PROGRAM);
    assert_eq!(read("bin/codex/claude"), fixture_program("claude", "codex"));
    assert_eq!(read("profile.sh").lines().count(), 1);
    // The Codex thread's terminal opened last, and codex in the Codex thread's terminal is still its program.
    assert_eq!(typed(dir.path(), "command -v codex", Some("codex")).0, "<dir>/bin/codex/codex\n");
    let (out, said, status) = typed(dir.path(), "codex exec \"say ok\"", Some("claude"));
    assert_eq!((out.as_str(), status), ("", Some(1)));
    assert!(said.starts_with("wsp: Codex cannot sign in on this machine. This terminal's thread runs Claude Code"), "said {said:?}");
    let (out, said, status) = typed(dir.path(), "claude -p \"say ok\"", Some("codex"));
    assert_eq!((out.as_str(), status), ("", Some(1)));
    assert!(said.starts_with("wsp: Claude Code cannot sign in on this machine. This terminal's thread runs Codex"), "said {said:?}");
}
