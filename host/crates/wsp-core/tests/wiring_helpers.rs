// SPDX-License-Identifier: AGPL-3.0-only
//! The helpers wiring.ts shares: runtime/src/types/wiring.ts:799-1060 at 65f6b89c7, spec/wiring-helpers.md. Expected
//! values are the TS's own answers (tests/fixtures/core_types/wiring.json) where the oracle ran them.
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use common::{fixture, wire};
use futures::FutureExt;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use wsp_core::types::pending::Clock;
use wsp_core::types::wiring::*;
use wsp_engine::errors::{EngineError, Result as EngineResult};
use wsp_engine::machine::*;
use wsp_engine::no_provider_backend::NoProviderBackend;
use wsp_wire::views::project_bundle::{ProjectAgentOutcome, ProjectAgentResult};
use wsp_wire::views::workspace::ProjectSource;
use wsp_wire::wire::machine_link::{ExecResult, MachineShape};

// wiring.ts:849.
#[test]
fn last_line_of_is_the_last_trimmed_line() {
    for row in fixture("wiring")["lastLines"].as_array().unwrap() {
        assert_eq!(last_line_of(row[0].as_str().unwrap()), row[1].as_str().unwrap(), "{:?}", row[0]);
    }
}

// wiring.ts:851-864: moved counts rows and adds a note; transcript-only adds a note; carried and nothing take no
// note; failed says the error or that none was given.
#[test]
fn outcome_words_match_the_ts() {
    for row in fixture("wiring")["outcomes"].as_array().unwrap() {
        let a: ProjectAgentResult = wire(&row[0]);
        assert_eq!(outcome_words(&a), row[1].as_str().unwrap(), "{}", row[0]);
    }
}

// wiring.ts:903-907: the name, the sessions counted where given, the outcome, the rollouts skipped where any.
#[test]
fn home_outcome_matches_the_ts() {
    for row in fixture("wiring")["homes"].as_array().unwrap() {
        let name = row[0]["name"].as_str().unwrap().to_string();
        let mut raw = row[0].clone();
        raw.as_object_mut().unwrap().remove("name");
        let a = LandedAgent { result: wire(&raw), name };
        assert_eq!(home_outcome(&a), row[1].as_str().unwrap(), "{}", row[0]);
    }
}

// wiring.ts:956-964: lowercased, runs outside [a-z0-9-] become one hyphen, hyphens trimmed, cut to 63 and trimmed
// again; "wsp" where nothing is left. The lowercasing is JavaScript's: "İ" becomes "i" and a combining dot.
#[test]
fn hostname_for_matches_the_ts() {
    for row in fixture("wiring")["hostnames"].as_array().unwrap() {
        let got = hostname_for(row[0].as_str().unwrap());
        assert_eq!(got, row[1].as_str().unwrap(), "{:?}", row[0]);
        assert!(got.len() <= 63);
    }
}

// wiring.ts:936-944: cpu then memMb, each only where both sides name it; disk and createdAt never.
#[test]
fn shape_fault_names_cpu_and_memory_only() {
    for row in fixture("wiring")["shapes"].as_array().unwrap() {
        let (want, got): (MachineShape, MachineShape) = (wire(&row[0]), wire(&row[1]));
        assert_eq!(shape_fault(&want, &got), row[2].as_str().map(String::from), "{row}");
    }
}

// wiring.ts:842: two gits by url, two folders by path, as written; a git and a folder never.
#[test]
fn same_source_compares_the_url_or_the_path() {
    for row in fixture("wiring")["sources"].as_array().unwrap() {
        let (a, b): (ProjectSource, ProjectSource) = (wire(&row[0]), wire(&row[1]));
        assert_eq!(same_source(&a, &b), row[2].as_bool().unwrap(), "{row}");
    }
}

// wiring.ts:842: a github source is neither shape the rule names, so two equal ones are not the same place.
#[test]
fn same_source_never_matches_two_github_sources() {
    let gh: ProjectSource = wire(&json!({ "kind": "github", "repo": "o/r" }));
    assert!(!same_source(&gh, &gh.clone()));
}

struct Places(Vec<String>);

impl PlaceBackends for Places {
    fn wired(&self) -> String {
        self.0[0].clone()
    }
    fn backend(&self, _place: &str) -> Option<Arc<dyn MachineBackend>> {
        None
    }
    fn list(&self) -> Vec<String> {
        self.0.clone()
    }
}

// wiring.ts:953: a place is a provider when the list names it; none is not; the empty name is one when listed.
#[test]
fn at_provider_reads_the_list() {
    let solari = Places(vec!["solari".into()]);
    assert!(!at_provider(None, &solari));
    assert!(at_provider(Some("solari"), &solari));
    assert!(!at_provider(Some("mac"), &solari));
    assert!(at_provider(Some(""), &Places(vec!["".into()])));
}

// wiring.ts:811.
#[test]
fn settled_is_settled_or_moot() {
    assert!(settled(GoneOutcome::Settled));
    assert!(!settled(GoneOutcome::NotGone));
    assert!(!settled(GoneOutcome::Unchecked));
    assert!(settled(GoneOutcome::Moot));
}

// wiring.ts:522-531: the one row answers its backend for its own id alone, and a read id is read at each call.
#[test]
fn wired_place_reads_its_id_at_each_call() {
    let backend: Arc<dyn MachineBackend> = Arc::new(NoProviderBackend::new(None));
    let fixed = wired_place(WiredId::Fixed("solari".into()), backend.clone());
    assert_eq!(fixed.wired(), "solari");
    assert_eq!(fixed.list(), vec!["solari".to_string()]);
    assert!(Arc::ptr_eq(&fixed.backend("solari").unwrap(), &backend));
    assert!(fixed.backend("boat").is_none());
    let id = Rc::new(RefCell::new("solari".to_string()));
    let read = id.clone();
    let moving = wired_place(WiredId::Read(Rc::new(move || read.borrow().clone())), backend.clone());
    assert_eq!(moving.wired(), "solari");
    *id.borrow_mut() = "boat".into();
    assert_eq!(moving.wired(), "boat");
    assert_eq!(moving.list(), vec!["boat".to_string()]);
    assert!(Arc::ptr_eq(&moving.backend("boat").unwrap(), &backend));
    assert!(moving.backend("solari").is_none());
}

/// A machine whose run answers as scripted and whose exec fails a cleanup, logging each command as the oracle did.
struct Scripted {
    log: Arc<Mutex<Vec<String>>>,
    run: Box<dyn Fn() -> EngineResult<ExecResult> + Send + Sync>,
    exec: Box<dyn Fn(&str) -> EngineResult<ExecResult> + Send + Sync>,
}

fn other(message: &str) -> EngineError {
    EngineError::Other { message: message.into(), code: None }
}

fn exec_result(code: i64, stdout: &str, stderr: &str) -> ExecResult {
    wire(&json!({ "exitCode": code, "stdout": stdout, "stderr": stderr }))
}

#[async_trait]
impl Machine for Scripted {
    fn id(&self) -> &str {
        "m9"
    }
    fn kind(&self) -> MachineKind {
        wire(&json!("sandbox"))
    }
    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> EngineResult<ExecResult> {
        self.log.lock().unwrap().push(format!("exec {cmd}"));
        (self.exec)(cmd)
    }
    async fn run(&self, script: &str, opts: RunOptions) -> EngineResult<ExecResult> {
        self.log.lock().unwrap().push(format!("run {script} {}", json!({ "deadlineMs": opts.deadline_ms })));
        (self.run)()
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> EngineResult<String> {
        unreachable!()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> EngineResult<()> {
        unreachable!()
    }
    async fn resume(&self, _signal: Option<CancellationToken>) -> EngineResult<()> {
        unreachable!()
    }
    async fn kill(&self) -> EngineResult<()> {
        unreachable!()
    }
    async fn state(&self) -> EngineResult<MachineState> {
        unreachable!()
    }
    async fn download_url(&self, _path: &str) -> EngineResult<String> {
        unreachable!()
    }
    async fn upload_url(&self, _path: &str) -> EngineResult<String> {
        unreachable!()
    }
}

fn scripted(run: impl Fn() -> EngineResult<ExecResult> + Send + Sync + 'static) -> Scripted {
    Scripted {
        log: Arc::new(Mutex::new(Vec::new())),
        run: Box::new(run),
        exec: Box::new(|cmd| if cmd.starts_with("rm") { Err(other("cleanup refused")) } else { Ok(exec_result(0, "", "")) }),
    }
}

// wiring.ts:873-892: the merge's outcome off its exit and its last line, its script removed by an exec whose own
// failure is ignored, every case the oracle ran.
#[tokio::test(flavor = "current_thread")]
async fn merge_on_machine_matches_the_ts() {
    let f = fixture("wiring");
    let runs: Vec<(&str, Box<dyn Fn() -> EngineResult<ExecResult> + Send + Sync>)> = vec![
        ("exit2", Box::new(|| Ok(exec_result(2, "", "first\nlast line  \n")))),
        ("exit1Quiet", Box::new(|| Ok(exec_result(1, "{}", "")))),
        ("merged", Box::new(|| Ok(exec_result(0, "noise\n{\"merged\": 3, \"kept\": 1}\n", "")))),
        ("mergedNote", Box::new(|| Ok(exec_result(0, "{\"merged\": 0, \"kept\": 0, \"note\": \"store empty\"}", "")))),
        ("waiting", Box::new(|| Ok(exec_result(0, "{\"waiting\": \"no store yet\", \"merged\": 1, \"kept\": 1}", "")))),
        ("empty", Box::new(|| Ok(exec_result(0, "", "")))),
        ("notJson", Box::new(|| Ok(exec_result(0, "done", "")))),
        ("runThrows", Box::new(|| Err(other("machine gone")))),
    ];
    let agent: ProjectAgentResult = wire(&json!({ "agent": "claude", "files": 1, "bytes": 2, "outcome": "nothing", "note": "before" }));
    for (name, run) in runs {
        let want = f["merges"].as_array().unwrap().iter().find(|m| m["name"] == name).unwrap();
        let machine = scripted(run);
        let got = merge_on_machine(&machine, "/tmp/wsp-merge/x y/claude.py", agent.clone()).await;
        match got {
            Ok(result) => assert_eq!(serde_json::to_value(&result).unwrap(), want["res"]["ok"], "{name}"),
            Err(e) => assert_eq!(e.to_string(), want["res"]["err"].as_str().unwrap(), "{name}"),
        }
        let log: Vec<Value> = machine.log.lock().unwrap().iter().map(|l| json!(l)).collect();
        assert_eq!(Value::Array(log), want["log"], "{name}");
    }
}

// wiring.ts:968-977: the label named, set and written; a refusal or a failed exec answered with the line, never thrown.
#[tokio::test(flavor = "current_thread")]
async fn set_hostname_answers_the_refusal_rather_than_failing() {
    let f = fixture("wiring");
    let execs: Vec<(&str, Box<dyn Fn(&str) -> EngineResult<ExecResult> + Send + Sync>)> = vec![
        ("ok", Box::new(|_| Ok(exec_result(0, "", "")))),
        ("refused", Box::new(|_| Ok(exec_result(1, "", "  hostname: you must be root \n")))),
        ("throws", Box::new(|_| Err(other("socket closed")))),
    ];
    for (name, exec) in execs {
        let want = f["hostSet"].as_array().unwrap().iter().find(|m| m["name"] == name).unwrap();
        let machine = Scripted { log: Arc::new(Mutex::new(Vec::new())), run: Box::new(|| unreachable!()), exec };
        let got = set_hostname(&machine, "My Box!").await;
        assert_eq!(got.host, want["res"]["host"].as_str().unwrap(), "{name}");
        assert_eq!(got.refused.as_deref(), want["res"]["refused"].as_str(), "{name}");
        let log: Vec<String> = machine.log.lock().unwrap().iter().map(|l| l.trim_start_matches("exec ").to_string()).collect();
        assert_eq!(log, vec![want["log"][0].as_str().unwrap().to_string()], "{name}");
    }
}

/// A clock moved by hand that logs every schedule and cancel, as the oracle's did.
struct HandClock {
    now: RefCell<i64>,
    timers: Rc<RefCell<Vec<(i64, Option<Box<dyn FnOnce()>>)>>>,
    log: Rc<RefCell<Vec<String>>>,
}

impl HandClock {
    fn new() -> Rc<HandClock> {
        Rc::new(HandClock { now: RefCell::new(1000), timers: Rc::new(RefCell::new(Vec::new())), log: Rc::new(RefCell::new(Vec::new())) })
    }
    fn advance(&self, ms: i64) {
        *self.now.borrow_mut() += ms;
        let now = *self.now.borrow();
        let due: Vec<Box<dyn FnOnce()>> =
            self.timers.borrow_mut().iter_mut().filter(|(at, f)| *at <= now && f.is_some()).filter_map(|(_, f)| f.take()).collect();
        for run in due {
            run();
        }
    }
}

impl Clock for HandClock {
    fn now(&self) -> i64 {
        *self.now.borrow()
    }
    fn schedule(&self, run: Box<dyn FnOnce()>, ms: i64) -> wsp_core::clock::Cancel {
        self.log.borrow_mut().push(format!("schedule {ms}"));
        let at = *self.now.borrow() + ms;
        let slot = self.timers.borrow().len();
        self.timers.borrow_mut().push((at, Some(run)));
        let (timers, log) = (self.timers.clone(), self.log.clone());
        Rc::new(move || {
            log.borrow_mut().push("cancel".into());
            timers.borrow_mut()[slot].1 = None;
        })
    }
}

fn logged(c: &HandClock) -> Value {
    json!(c.log.borrow().clone())
}

// wiring.ts:921-930: the deadline is read on the clock given; past it the wait fails with the deadline's own error,
// naming the wait it was armed with, and the work is left to settle on its own.
#[tokio::test(flavor = "current_thread")]
async fn until_fails_on_the_clock_it_was_given() {
    let f = &fixture("wiring")["untils"];
    let clock = HandClock::new();
    let mut wait = Box::pin(until::<u32, String>(futures::future::pending().boxed_local(), 1500, "the probe", clock.clone()));
    assert!(futures::poll!(&mut wait).is_pending());
    clock.advance(600);
    match wait.await {
        Err(UntilError::Deadline(DeadlineError(m))) => assert_eq!(m, f["deadline"]["res"]["err"].as_str().unwrap()),
        other => panic!("not the deadline: {other:?}"),
    }
    assert_eq!(logged(&clock), f["deadline"]["log"]);
}

// wiring.ts:925-928: work that settles first answers as it settled and cancels the timer.
#[tokio::test(flavor = "current_thread")]
async fn until_answers_the_work_and_cancels_the_timer() {
    let f = &fixture("wiring")["untils"];
    let clock = HandClock::new();
    let got = until::<u32, String>(futures::future::ready(Ok(7)).boxed_local(), 1500, "x", clock.clone()).await;
    assert_eq!(got.ok(), Some(7));
    assert_eq!(logged(&clock), f["resolves"]["log"]);
    let clock = HandClock::new();
    match until::<u32, String>(futures::future::ready(Err("own".to_string())).boxed_local(), 1500, "x", clock.clone()).await {
        Err(UntilError::Failed(e)) => assert_eq!(e, f["rejects"]["res"]["err"].as_str().unwrap()),
        other => panic!("not the work's own failure: {other:?}"),
    }
    assert_eq!(logged(&clock), f["rejects"]["log"]);
}

// wiring.ts:923: a deadline already passed arms a wait of 0 ms, never a negative one.
#[tokio::test(flavor = "current_thread")]
async fn until_past_its_deadline_waits_zero() {
    let f = &fixture("wiring")["untils"];
    let clock = HandClock::new();
    let mut wait = Box::pin(until::<u32, String>(futures::future::pending().boxed_local(), 400, "late", clock.clone()));
    assert!(futures::poll!(&mut wait).is_pending());
    clock.advance(0);
    match wait.await {
        Err(UntilError::Deadline(DeadlineError(m))) => assert_eq!(m, f["past"]["res"]["err"].as_str().unwrap()),
        other => panic!("not the deadline: {other:?}"),
    }
    assert_eq!(logged(&clock), f["past"]["log"]);
}

fn real(p: &std::path::Path) -> String {
    std::fs::canonicalize(p).unwrap().to_string_lossy().into_owned()
}

// wiring.ts:829-838: "~" and "~/..." under the home, joined and normalized; any other relative word resolved under
// the folder the host runs in; then through the links, the joined path itself where that fails.
#[test]
fn folder_named_reads_home_cwd_and_links() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("h");
    let cwd = dir.path().join("c");
    let target = dir.path().join("target");
    for d in [&home, &cwd, &target] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::os::unix::fs::symlink(&target, cwd.join("link")).unwrap();
    let (h, c) = (home.to_str().unwrap(), cwd.to_str().unwrap());
    assert_eq!(folder_named("~", h, c), real(&home));
    assert_eq!(folder_named("~/a/../b", h, c), format!("{}/b", h));
    assert_eq!(folder_named("link", h, c), real(&target));
    assert_eq!(folder_named("link/../link", h, c), real(&target));
    assert_eq!(folder_named("~foo", h, c), format!("{}/~foo", c));
    assert_eq!(folder_named("", h, c), real(&cwd));
}

// wiring.ts:830: an absolute path is taken as written, unnormalized, where it does not resolve.
#[test]
fn folder_named_keeps_an_absolute_path_that_does_not_resolve_as_written() {
    assert_eq!(folder_named("/nonexistent-2448/../x", "/h", "/c"), "/nonexistent-2448/../x");
}
