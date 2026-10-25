// SPDX-License-Identifier: AGPL-3.0-only
//! A verb's run from the parsed line to its exit code (verbs.ts:694-764, spec/verbs-table.md "The run"): what is
//! read once, what is dialled once, what is printed where, and what is closed, driven through verbs of the test's
//! own so the run is the only thing under test.

#[path = "cli_a_common/mod.rs"]
mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;

use common::{RecordingIo, Script, ScriptedClient, ScriptedHost};
use wsp_cli::hosts::HostAim;
use wsp_cli::verbs::client::{
    Again, BoxFuture, CliOnlyVerb, CliVerb, DialFn, DialOpts, FlagKind, FlagRow, FlagTable, Frame, HostClient, LineVerb, Page, Tool, Verb,
    VerbContext,
};
use wsp_cli::verbs::{run_verb, RunDeps};
use wsp_words::exit::{HostError, Kind};

/// What one dial was handed.
#[derive(Clone, Debug)]
struct Dialled {
    state: String,
    any_release: bool,
    deadline_ms: Option<u64>,
    started: bool,
    aim: Option<HostAim>,
}

/// A dial that opens a scripted socket and records what it was handed.
struct Recording {
    host: Arc<ScriptedHost>,
    dials: Mutex<Vec<Dialled>>,
    sockets: Mutex<Vec<Arc<ScriptedClient>>>,
}

impl Recording {
    fn new() -> Arc<Recording> {
        Arc::new(Recording { host: ScriptedHost::new(Script::default()), dials: Mutex::new(Vec::new()), sockets: Mutex::new(Vec::new()) })
    }
    fn dials(&self) -> Vec<Dialled> {
        self.dials.lock().unwrap().clone()
    }
    fn closes(&self) -> usize {
        self.host.state.closes_called.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl DialFn for Recording {
    async fn dial(&self, state_path: &str, opts: DialOpts) -> Result<Arc<dyn HostClient>, HostError> {
        if let Some(say) = &opts.say {
            say("starting the host");
        }
        self.dials.lock().unwrap().push(Dialled {
            state: state_path.to_owned(),
            any_release: opts.any_release,
            deadline_ms: opts.deadline_ms,
            started: opts.start.is_some(),
            aim: opts.aim.clone(),
        });
        let socket = ScriptedClient::new(self.host.script.clone(), self.host.state.clone());
        self.sockets.lock().unwrap().push(socket.clone());
        Ok(socket)
    }
}

struct NoStart;

#[async_trait]
impl wsp_cli::host_lock::HostStarter for NoStart {
    async fn start(
        &self,
        _state_path: &str,
        _say: &(dyn Fn(&str) + Send + Sync),
    ) -> Result<Option<wsp_cli::host_lock::HostLock>, HostError> {
        Ok(None)
    }
}

fn no_tool(_: Frame, _: &wsp_cli::verbs::client::VerbDeps) -> BoxFuture<'_, Result<wsp_cli::verbs::client::CallToolResult, HostError>> {
    Box::pin(async { unreachable!("the tool door is not under test") })
}

fn verb(name: &'static str, run: wsp_cli::verbs::client::RunFn) -> CliVerb {
    CliVerb {
        name,
        usage: format!("wsp {name} [--flag <v>]"),
        about: "does what the test says".to_owned(),
        page: Page::Agent,
        options: FlagTable(vec![("flag", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run,
        tool: Tool { description: String::new(), input: vec![], output: vec![], stream: vec![], call: no_tool },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }
}

static RUNS: AtomicUsize = AtomicUsize::new(0);

/// Dials twice plainly and once again after a stop, then answers what it was handed.
fn dials_three_ways(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        RUNS.fetch_add(1, Ordering::SeqCst);
        let a = ctx.client(None).await?;
        let b = ctx.client(None).await?;
        assert!(Arc::ptr_eq(&a, &b), "a line dials once and hands the same socket back");
        let _c = ctx.client(Some(Again { within_ms: 1_234 })).await?;
        ctx.out.emit(&json!({ "args": ctx.args, "state": ctx.state_path }), Some("ran"));
        Ok(0)
    })
}

fn dials_once(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        ctx.client(None).await?;
        Ok(0)
    })
}

fn never_dials(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        ctx.out.emit(&json!({ "ok": true }), Some("nothing dialled"));
        Ok(7)
    })
}

fn fails_after_dialling(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        ctx.client(None).await?;
        Err(HostError::with_kind("no thread nope", Kind::NotFound))
    })
}

fn fails_naming_itself(_ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move { Err(HostError::with_kind("wsp mine takes one thread.", Kind::Usage)) })
}

fn host_validator_refused(_ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move { Err(HostError::new(r#"the request did not match: [{"path":["thread"],"message":"Required"}]"#)) })
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
}

struct Run {
    code: i32,
    io: Arc<RecordingIo>,
    picked: Vec<Option<String>>,
}

async fn run(verb: LineVerb<'_>, words: &[&str], dial: Arc<Recording>, env: &[(&str, &str)], start: bool) -> Run {
    let home = tempfile::tempdir().unwrap();
    let io = RecordingIo::new();
    let picked = Arc::new(Mutex::new(Vec::new()));
    let noted = picked.clone();
    let pick = move |flag: Option<&str>| {
        noted.lock().unwrap().push(flag.map(str::to_owned));
        flag.unwrap_or("/picked/state.json").to_owned()
    };
    let mut env: indexmap::IndexMap<String, String> = env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    env.entry("WSP_HOME".into()).or_insert_with(|| home.path().join("wsp").to_string_lossy().into_owned());
    env.entry("HOME".into()).or_insert_with(|| home.path().to_string_lossy().into_owned());
    let deps = RunDeps {
        env,
        dial: Some(dial),
        start: if start { Some(Arc::new(NoStart)) } else { None },
        cwd: Some("/root".into()),
        ..RunDeps::default()
    };
    let code = run_verb(&[], verb, &argv(words), io.clone(), &pick, deps).await;
    let picked = picked.lock().unwrap().clone();
    Run { code, io, picked }
}

#[tokio::test]
async fn a_line_dials_once_lazily_hands_the_socket_back_and_dials_again_only_when_asked_after_a_stop() {
    let v = verb("mine", dials_three_ways);
    let dial = Recording::new();
    let r = run(LineVerb::Cli(&v), &["mine", "a", "b"], dial.clone(), &[], true).await;
    assert_eq!(r.code, 0, "{:?}", r.io.errors());
    let dials = dial.dials();
    assert_eq!(dials.len(), 2, "one plain dial for the line, one again: {dials:?}");
    assert_eq!(dials[0].state, "/picked/state.json");
    assert_eq!(dials[0].aim, Some(HostAim::Here), "the aim read once is the aim dialled");
    assert!(dials[0].started && dials[0].deadline_ms.is_none(), "a plain dial here may start a host");
    assert!(!dials[1].started, "a dial again starts nothing: a host it started would hold the lock the returning one needs");
    assert_eq!(dials[1].deadline_ms, Some(1_234), "the dial again is held to what the wait has left");
    assert_eq!(r.io.lines(), vec!["ran".to_owned()]);
    assert_eq!(r.picked, vec![None], "the state is picked once, from the --state flag or nothing");
}

#[tokio::test]
async fn the_starter_s_line_goes_to_stderr_beside_what_the_verb_prints() {
    let v = verb("mine", dials_once);
    let r = run(LineVerb::Cli(&v), &["mine"], Recording::new(), &[], true).await;
    assert_eq!(r.io.errors(), vec!["starting the host".to_owned()]);
    assert!(r.io.lines().is_empty());
}

#[tokio::test]
async fn the_socket_is_closed_once_the_run_returns_whether_it_answered_or_failed_and_never_opened_when_unused() {
    let ok = verb("mine", dials_once);
    let dial = Recording::new();
    assert_eq!(run(LineVerb::Cli(&ok), &["mine"], dial.clone(), &[], false).await.code, 0);
    assert_eq!(dial.closes(), 1, "closed after an answer");
    let failing = verb("mine", fails_after_dialling);
    let dial = Recording::new();
    let r = run(LineVerb::Cli(&failing), &["mine"], dial.clone(), &[], false).await;
    assert_eq!(r.code, 3, "not-found is the usage class");
    assert_eq!(dial.closes(), 1, "closed after a failure");
    let idle = verb("mine", never_dials);
    let dial = Recording::new();
    let r = run(LineVerb::Cli(&idle), &["mine"], dial.clone(), &[], false).await;
    assert_eq!(r.code, 7, "the run's own exit code stands");
    assert!(dial.dials().is_empty() && dial.closes() == 0);
}

#[tokio::test]
async fn a_failure_is_one_stderr_line_behind_the_verb_s_name_or_its_failure_object_under_json() {
    let v = verb("mine", fails_after_dialling);
    let r = run(LineVerb::Cli(&v), &["mine"], Recording::new(), &[], false).await;
    assert_eq!(r.io.errors(), vec!["wsp mine: no thread nope".to_owned()]);
    assert!(r.io.lines().is_empty());
    let r = run(LineVerb::Cli(&v), &["mine", "--json"], Recording::new(), &[], false).await;
    assert_eq!(r.io.errors(), vec![r#"{"error":"no thread nope","class":"usage","exit":3}"#.to_owned()]);
    assert!(r.io.lines().is_empty(), "stdout carries nothing on a failure");
}

#[tokio::test]
async fn a_sentence_that_opens_with_the_verb_s_name_is_printed_once() {
    let v = verb("mine", fails_naming_itself);
    let r = run(LineVerb::Cli(&v), &["mine"], Recording::new(), &[], false).await;
    assert_eq!(r.io.errors(), vec!["wsp mine takes one thread.".to_owned()]);
    assert_eq!(r.code, 3);
}

#[tokio::test]
async fn a_refusal_the_host_s_validator_wrote_reads_as_the_line_it_would_not_take_and_the_verb_s_form() {
    let v = verb("mine", host_validator_refused);
    let r = run(LineVerb::Cli(&v), &["mine"], Recording::new(), &[], false).await;
    assert_eq!(r.code, 3, "a usage refusal");
    let said = r.io.errors();
    assert_eq!(said.len(), 1);
    assert!(said[0].starts_with("wsp mine: "), "{said:?}");
    assert!(said[0].ends_with(" usage: wsp mine [--flag <v>]"), "the verb's usage is the fix: {said:?}");
    assert!(!said[0].contains("\"path\""), "the wire's issue list is no person's to read: {said:?}");
}

#[tokio::test]
async fn help_prints_the_verb_s_page_and_returns_without_picking_a_state_or_dialling() {
    let v = verb("mine", dials_once);
    let dial = Recording::new();
    let r = run(LineVerb::Cli(&v), &["mine", "--help"], dial.clone(), &[], true).await;
    assert_eq!(r.code, 0);
    assert!(r.picked.is_empty(), "no state is read for help");
    assert!(dial.dials().is_empty());
    let page = r.io.lines().join("\n");
    assert!(page.starts_with("usage: wsp mine [--flag <v>]\n  does what the test says\n\n  --flag "), "{page}");
    assert!(page.contains("  --host     run the line against a host on your account"), "{page}");
}

#[tokio::test]
async fn help_on_a_line_that_runs_at_its_own_host_says_what_host_means_there() {
    let base = verb("mine", dials_once);
    let v = CliOnlyVerb {
        name: base.name,
        usage: base.usage.clone(),
        about: base.about.clone(),
        page: Page::Agent,
        options: base.options.clone(),
        run: dials_once,
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
        cli_only: "a person's line",
        host_side: Some("Keys stay at their host."),
    };
    let r = run(LineVerb::CliOnly(&v), &["mine", "-h"], Recording::new(), &[], true).await;
    assert!(
        r.io.lines().join("\n").contains("--host     read to say this line runs at its own host's terminal; it dials no other"),
        "{:?}",
        r.io.lines()
    );
}

#[tokio::test]
async fn a_line_that_runs_at_its_own_host_refuses_any_other_aim_naming_it_and_dials_nothing() {
    let v = CliOnlyVerb {
        name: "mine",
        usage: "wsp mine".to_owned(),
        about: String::new(),
        page: Page::Agent,
        options: FlagTable::default(),
        run: dials_once,
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
        cli_only: "a person's line",
        host_side: Some("Keys stay at their host."),
    };
    let dial = Recording::new();
    let r =
        run(LineVerb::CliOnly(&v), &["mine"], dial.clone(), &[("WSP_HOST_URL", "http://127.0.0.1:7777"), ("WSP_HOST_TOKEN", "t")], true)
            .await;
    assert_eq!(r.code, 3);
    assert_eq!(r.io.errors(), vec!["wsp mine runs on the computer the host runs on, and this line is aimed at http://127.0.0.1:7777. Run it in a terminal over there. Keys stay at their host.".to_owned()]);
    assert!(dial.dials().is_empty());
}

#[tokio::test]
async fn an_empty_host_is_refused_behind_the_verb_s_name_before_anything_is_dialled() {
    let v = verb("mine", dials_once);
    let dial = Recording::new();
    let r = run(LineVerb::Cli(&v), &["mine", "--host", ""], dial.clone(), &[], true).await;
    assert_eq!(r.code, 3);
    assert_eq!(
        r.io.errors(),
        vec!["wsp mine: --host was given an empty word, which names no host. Name one as wsp hosts lists it, or leave the flag off."
            .to_owned()]
    );
    assert!(dial.dials().is_empty());
}

#[tokio::test]
async fn a_state_named_for_a_line_aimed_elsewhere_is_noted_once_and_only_when_the_line_dials() {
    let env = [("WSP_HOST_URL", "http://127.0.0.1:7777"), ("WSP_HOST_TOKEN", "t")];
    let v = verb("mine", dials_three_ways);
    let r = run(LineVerb::Cli(&v), &["mine", "--state", "/mine/state.json"], Recording::new(), &env, false).await;
    assert_eq!(
        r.io.errors(),
        vec!["--state names a file on this computer and this line runs against http://127.0.0.1:7777, which serves its own, so it is not read.".to_owned(), "starting the host".to_owned(), "starting the host".to_owned()]
    );
    let idle = verb("mine", never_dials);
    let r = run(LineVerb::Cli(&idle), &["mine", "--state", "/mine/state.json"], Recording::new(), &env, false).await;
    assert!(r.io.errors().is_empty(), "a line that never dials says nothing about a state it did not read: {:?}", r.io.errors());
    assert_eq!(r.picked, vec![Some("/mine/state.json".to_owned())]);
}

#[tokio::test]
async fn a_line_aimed_elsewhere_dials_that_aim_and_starts_nothing_here() {
    let env = [("WSP_HOST_URL", "http://127.0.0.1:7777"), ("WSP_HOST_TOKEN", "t")];
    let v = verb("mine", dials_once);
    let dial = Recording::new();
    run(LineVerb::Cli(&v), &["mine"], dial.clone(), &env, true).await;
    let dials = dial.dials();
    assert_eq!(dials[0].aim, Some(HostAim::Url { url: "http://127.0.0.1:7777".into(), token: Some("t".into()), host_key: None }));
}

#[tokio::test]
async fn a_line_that_starts_no_host_dials_without_the_starter_and_any_release_dials_past_the_release() {
    let mut v = verb("mine", dials_once);
    v.starts_no_host = Some("a proxy for one connection");
    v.any_release = Some("the restart is the fix");
    let dial = Recording::new();
    run(LineVerb::Cli(&v), &["mine"], dial.clone(), &[], true).await;
    let dials = dial.dials();
    assert!(!dials[0].started);
    assert!(dials[0].any_release);
    let plain = verb("mine", dials_once);
    let dial = Recording::new();
    run(LineVerb::Cli(&plain), &["mine"], dial.clone(), &[], true).await;
    assert!(!dial.dials()[0].any_release);
}

#[tokio::test]
async fn the_shared_flags_bind_wherever_they_sit_and_the_verb_reads_its_positionals_in_order() {
    let v = verb("mine", dials_three_ways);
    let r =
        run(LineVerb::Cli(&v), &["mine", "a", "--json", "b", "--flag", "x", "--state", "/s/state.json", "c"], Recording::new(), &[], false)
            .await;
    assert_eq!(r.io.lines(), vec![r#"{"args":["a","b","c"],"state":"/s/state.json"}"#.to_owned()]);
    assert_eq!(r.picked, vec![Some("/s/state.json".to_owned())]);
}

#[tokio::test]
async fn words_after_the_double_dash_are_positionals_never_flags() {
    let v = verb("mine", dials_three_ways);
    let r = run(LineVerb::Cli(&v), &["mine", "--json", "--", "--help", "-h", "--nope"], Recording::new(), &[], false).await;
    assert_eq!(r.code, 0, "{:?}", r.io.errors());
    assert_eq!(r.io.lines(), vec![r#"{"args":["--help","-h","--nope"],"state":"/picked/state.json"}"#.to_owned()]);
    let _ = RUNS.load(Ordering::SeqCst);
    let _ = Verb::Cli(v.clone());
}
