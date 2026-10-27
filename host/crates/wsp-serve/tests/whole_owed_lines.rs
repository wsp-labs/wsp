// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/owed-lines.test.ts at 65f6b89c7, case for case. A child's finished line that cannot start into its lead stays
//! owed until a turn of the lead takes it, and reaches the lead once: through a steer whose answer was lost or came late, a launch that
//! failed after its start answered, and a host restart, on a clock the case moves. A line that cannot reach the lead within an hour of
//! wall time, one the lead's door or its computer refuses, and one whose try the person stopped go to the person instead, on either
//! side of a restart. The runtime and the real Claude and Codex adapters, fed by hand. Each test names the TS test above it. The TS
//! `memoryStore()` is the sqlite store here.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::fed::{fed_runs, init, lifecycle, result, said, FedRuns, Loses, Run};
use common::*;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::codex::adapter::{create_codex_adapter, CodexAdapterDeps};
use wsp_adapters::port::{
    AttachRunOptions, Attached, ExecFailure, ExecFailureKind, ExecOptions, ExecStream, ExecStreamFactory, HarnessAdapter,
};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::internal::NOTIFY_OWED;
use wsp_core::types::wiring::SessionHandle;
use wsp_serve::AssembledRuntime;
use wsp_wire::views::session_events::NOTIFY_ME;
use wsp_words::exit::HostError;

const REPORT: &str = "Parsed 40 files; the suite passes.";
const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;

async fn tick() {
    sleep_ms(50).await;
}

/// What a provider's refusal of a launch comes to once the stream wraps it: the machine is gone.
const LAUNCH_REFUSED: &str = "remote launch failed on m1: gone";

fn launch_lost() -> ExecFailure {
    ExecFailure { kind: ExecFailureKind::Unreached, message: "machine.exec on lab-box was not answered in 35s".into() }
}

fn launch_refused() -> ExecFailure {
    ExecFailure { kind: ExecFailureKind::Other, message: LAUNCH_REFUSED.into() }
}

/// A Claude Code that announces no msg_lifecycle_v1, so its turns tell no unread message.
fn old_init() -> String {
    let mut doc: Value = serde_json::from_str(&init()).expect("an init");
    doc["claude_code_version"] = json!("2.1.200");
    doc["capabilities"] = json!(["interrupt_receipt_v1"]);
    doc.to_string()
}

// A Codex app server's lines, as 0.155.1 prints them, trimmed to the fields the adapter reads.
const THREAD_ID: &str = "01a0e2c1-5d10-7b42-9a6e-3f1c2d4b5a60";
const TURN_ID: &str = "01a0e2c1-5e02-7c11-8d3f-9b2a1c0d4e71";

fn opened() -> Vec<String> {
    vec![
        r#"{"id":"wsp-initialize","result":{"userAgent":"wsp/0.155.1","codexHome":"/root/.codex","platformFamily":"unix","platformOs":"linux"}}"#.into(),
        format!(
            r#"{{"id":"wsp-thread","result":{{"thread":{{"id":"{THREAD_ID}","model":"gpt-5.6-sol","cwd":"/root/app","turns":[]}},"model":"gpt-5.6-sol","cwd":"/root/app","approvalPolicy":"on-request","sandbox":{{"type":"workspaceWrite"}}}}}}"#
        ),
    ]
}

fn turn_started() -> String {
    format!(
        r#"{{"method":"turn/started","params":{{"threadId":"{THREAD_ID}","turn":{{"id":"{TURN_ID}","items":[],"status":"inProgress"}}}}}}"#
    )
}

fn agent_message() -> String {
    format!(
        r#"{{"method":"item/completed","params":{{"item":{{"type":"agentMessage","id":"m1","text":"waiting"}},"threadId":"{THREAD_ID}","turnId":"{TURN_ID}"}}}}"#
    )
}

fn completed() -> String {
    format!(
        r#"{{"method":"turn/completed","params":{{"threadId":"{THREAD_ID}","turn":{{"id":"{TURN_ID}","items":[],"status":"completed"}}}}}}"#
    )
}

/// The line a recorded app-server run printed at `line` (1-based), its thread named as this case's.
fn recorded(fixture: &str, line: usize) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/adapter-codex/test/fixtures").join(fixture);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let raw = text.split('\n').nth(line - 1).expect("the line").to_string();
    let named: Value = serde_json::from_str(&raw).expect("a json line");
    match named["params"]["threadId"].as_str() {
        None => raw,
        Some(id) => raw.split(id).collect::<Vec<_>>().join(THREAD_ID),
    }
}

// The warnings two recorded runs printed between the thread's answer and turn/started, and the one a server without bubblewrap prints
// right after initialize.
struct Warning {
    line: String,
    early: bool,
}

fn warnings() -> Vec<Warning> {
    vec![
        Warning { line: recorded("app-server-turn.jsonl", 5), early: false },
        Warning { line: recorded("app-server-subagent.jsonl", 5), early: false },
        Warning { line: recorded("catalog-probe-route.jsonl", 2), early: true },
    ]
}

/// Each run carries the workspace it was launched for, so a case with two leads tells their launches apart.
struct Tagged {
    inner: Arc<dyn ExecStreamFactory>,
    workspace: String,
}

impl ExecStreamFactory for Tagged {
    fn launch(&self, command: &str, mut o: ExecOptions) -> Arc<dyn ExecStream> {
        o.env.insert("LAB_WORKSPACE".into(), self.workspace.clone());
        self.inner.launch(command, o)
    }
    fn attaches(&self) -> bool {
        self.inner.attaches()
    }
    fn attach(&self, run: &str, options: AttachRunOptions) -> Option<LocalBoxFuture<'static, Result<Attached, ExecFailure>>> {
        self.inner.attach(run, options)
    }
}

/// An attach the case can make go unanswered, as a computer that is dark when the next host re-opens its runs, or hold until it lets
/// go, as a cloud machine still restoring answers nothing for a while.
struct Reopening {
    inner: Arc<dyn ExecStreamFactory>,
    dark: Rc<Cell<bool>>,
    gate: Rc<RefCell<Option<Rc<Gate>>>>,
}

impl ExecStreamFactory for Reopening {
    fn launch(&self, command: &str, o: ExecOptions) -> Arc<dyn ExecStream> {
        self.inner.launch(command, o)
    }
    fn attaches(&self) -> bool {
        true
    }
    fn attach(&self, run: &str, options: AttachRunOptions) -> Option<LocalBoxFuture<'static, Result<Attached, ExecFailure>>> {
        let (inner, dark, gate, run) = (self.inner.clone(), self.dark.clone(), self.gate.clone(), run.to_string());
        Some(
            async move {
                let held = gate.borrow().clone();
                if let Some(held) = held {
                    held.wait().await;
                }
                if dark.get() {
                    return Err(launch_lost());
                }
                inner.attach(&run, options).expect("an attach").await
            }
            .boxed_local(),
        )
    }
}

fn claude_over(exec: Arc<dyn ExecStreamFactory>, result_exit_ms: u64) -> Result<Arc<dyn HarnessAdapter>, HostError> {
    let mut deps = AdapterDeps::new(exec, "/root/.claude-cfg");
    deps.result_exit_ms = Some(result_exit_ms);
    let adapter = create_claude_adapter(deps).map_err(|e| HostError::new(e.0))?;
    Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
}

fn codex_over(exec: Arc<dyn ExecStreamFactory>) -> Result<Arc<dyn HarnessAdapter>, HostError> {
    let adapter = create_codex_adapter(CodexAdapterDeps::new(exec, "/root/.codex", "codex login")).map_err(|e| HostError::new(e.0))?;
    Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
}

fn lead_of(name: &str) -> CreateOn {
    CreateOn { agents: Some(serde_json::Map::new()), ..CreateOn::golden(name) }
}

async fn history(rt: &Rc<AssembledRuntime>, ws: &str) -> Vec<Value> {
    rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

async fn done_count(rt: &Rc<AssembledRuntime>, ws: &str) -> usize {
    history(rt, ws).await.iter().filter(|e| e["type"] == "session.done").count()
}

async fn has_type(rt: &Rc<AssembledRuntime>, ws: &str, kind: &'static str) -> bool {
    history(rt, ws).await.iter().any(|e| e["type"] == kind)
}

fn writes_of(run: &Rc<Run>, words: &str) -> bool {
    run.writes().iter().any(|w| w.contains(words))
}

// ===== a child's finished line its lead cannot take yet =====

#[derive(Default)]
struct Opts {
    lead_notify: Option<Vec<String>>,
    result_exit_ms: Option<u64>,
    lead_init: Option<String>,
}

/// The handles of the runtime a case holds now.
struct Rt {
    rt: Rc<AssembledRuntime>,
    store: Rc<dyn wsp_store::store::Store>,
}

struct Refusing {
    on: RefCell<Option<String>>,
    refused: Cell<u32>,
}

struct Setup {
    fed: Rc<FedRuns>,
    factory: Arc<dyn ExecStreamFactory>,
    backend: Arc<StubBackend>,
    state: Rc<State>,
    refusing: Rc<Refusing>,
    result_exit_ms: u64,
    rt: RefCell<Fx>,
    lead_run: Rc<Run>,
    lead_ws: String,
    lead: Rc<dyn SessionHandle>,
    lead_thread: String,
    kid_ws: String,
    kid_thread: RefCell<Option<String>>,
}

fn host(s: &SetupParts, on: &FakeClock) -> Fx {
    let (factory, refusing, exit_ms) = (s.factory.clone(), s.refusing.clone(), s.result_exit_ms);
    // Once a case says so, the agent refuses outright to start on the lead's folder, a plain refusal rather than a computer that did not
    // answer; each one it refused is counted.
    let claude: HarnessAdapterFactory = Arc::new(move |ctx| {
        if refusing.on.borrow().as_deref() == Some(ctx.workspace_id.as_str()) {
            refusing.refused.set(refusing.refused.get() + 1);
            return Err(HostError::new("claude will not start here"));
        }
        claude_over(Arc::new(Tagged { inner: factory.clone(), workspace: ctx.workspace_id.clone() }), exit_ms)
    });
    runtime(&s.backend).store(&s.state.store).clock(on).adapter("claude", claude).build()
}

struct SetupParts {
    factory: Arc<dyn ExecStreamFactory>,
    backend: Arc<StubBackend>,
    state: Rc<State>,
    refusing: Rc<Refusing>,
    result_exit_ms: u64,
}

/// `leadNotify` is who the lead's own ends tell, and `resultExitMs` how long a reply held for an open message waits for the CLI before
/// it stands.
async fn setup(clock: &FakeClock, o: Opts) -> Setup {
    let (factory, fed) = fed_runs();
    let fed = Rc::new(fed);
    let parts = SetupParts {
        factory,
        backend: StubBackend::new(None),
        state: Rc::new(sqlite_state()),
        refusing: Rc::new(Refusing { on: RefCell::new(None), refused: Cell::new(0) }),
        result_exit_ms: o.result_exit_ms.unwrap_or(60_000),
    };
    let rt = host(&parts, clock);
    let lead_ws = create_on(&rt.rt, lead_of("lead"), None).await.expect("a workspace").view.id;
    let start = StartOptions { prompt: "orchestrate".into(), notify: o.lead_notify.clone(), ..Default::default() };
    let lead = rt.rt.sessions().start(&lead_ws, start, None).await.expect("a start");
    until(|| async { fed.run_of("orchestrate").is_some() }).await;
    let lead_run = fed.run_of("orchestrate").expect("the run");
    lead_run.push(&[o.lead_init.unwrap_or_else(init)]);
    until(|| async { has_type(&rt.rt, &lead_ws, "session.start").await }).await;
    let lead_thread = lead.view().thread_id.expect("a thread");
    let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
    Setup {
        fed,
        factory: parts.factory,
        backend: parts.backend,
        state: parts.state,
        refusing: parts.refusing,
        result_exit_ms: parts.result_exit_ms,
        rt: RefCell::new(rt),
        lead_run,
        lead_ws,
        lead,
        lead_thread,
        kid_ws,
        kid_thread: RefCell::new(None),
    }
}

impl Setup {
    fn rt(&self) -> Rt {
        let held = self.rt.borrow();
        Rt { rt: held.rt.clone(), store: held.store.clone() }
    }

    fn parts(&self) -> SetupParts {
        SetupParts {
            factory: self.factory.clone(),
            backend: self.backend.clone(),
            state: self.state.clone(),
            refusing: self.refusing.clone(),
            result_exit_ms: self.result_exit_ms,
        }
    }

    async fn close(&self) {
        let rt = self.rt();
        rt.rt.close().await.expect("a close");
    }

    /// The lead replies and its process exits, as a coordinator's turn ends to wait on its children.
    async fn lead_waits(&self) {
        self.lead_run.push(&[said("msg_l", "waiting"), result("waiting", 1, None)]);
        let rt = self.rt();
        until(|| async { has_type(&rt.rt, &self.lead_ws, "session.done").await }).await;
        self.lead_run.end(Some(0));
        self.lead.finished().await;
    }

    /// The lead replies with a message still open in its turn, so its reply is held until its process exits.
    async fn lead_exits(&self) {
        self.lead_run.push(&[said("msg_l", "waiting"), result("waiting", 1, None)]);
        tick().await;
        self.lead_run.end(Some(0));
        self.lead.finished().await;
    }

    /// A child opened with the lead as its target, or the targets named, ends with REPORT.
    async fn kid_ends(&self, notify: Option<Vec<String>>, meanwhile: Option<&dyn Fn()>) {
        let rt = self.rt();
        let opts = StartOptions {
            prompt: "parse the files".into(),
            notify: Some(notify.unwrap_or_else(|| vec![self.lead_thread.clone()])),
            ..Default::default()
        };
        let kid = rt.rt.sessions().start(&self.kid_ws, opts, None).await.expect("a start");
        let thread = kid.view().thread_id.expect("a thread");
        *self.kid_thread.borrow_mut() = Some(thread.clone());
        until(|| async { self.fed.run_of("parse the files").is_some() }).await;
        let run = self.fed.run_of("parse the files").expect("the run");
        if let Some(meanwhile) = meanwhile {
            meanwhile();
        }
        run.push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
        let ws = kid.view().workspace_id.clone();
        until(|| async { history(&rt.rt, &ws).await.iter().any(|e| e["type"] == "session.done" && e["threadId"] == thread.as_str()) })
            .await;
        run.end(Some(0));
        kid.finished().await;
        tick().await;
    }

    fn kid_thread(&self) -> String {
        self.kid_thread.borrow().clone().expect("the child's thread")
    }

    /// The rows the person was given for a thread's lines: the child's by default.
    async fn to_person(&self, thread: Option<&str>) -> Vec<String> {
        let rt = self.rt();
        let thread = thread.map_or_else(|| self.kid_thread(), str::to_string);
        let mut all = history(&rt.rt, &self.kid_ws).await;
        all.extend(history(&rt.rt, &self.lead_ws).await);
        all.into_iter()
            .filter(|e| e["type"] == "session.notify" && e["notify"] == NOTIFY_ME && e["threadId"] == thread.as_str())
            .map(|e| e["text"].as_str().expect("a line").to_string())
            .collect()
    }

    /// The lead's runs opened with the child's line, in launch order.
    fn launches(&self) -> Vec<Rc<Run>> {
        self.fed.runs_of(REPORT)
    }

    /// Every write of the child's line into a running turn of the lead.
    fn steers(&self) -> Vec<String> {
        self.lead_run.writes().into_iter().filter(|w| w.contains(REPORT)).collect()
    }

    /// The clock moves in steps until the lead's nth launch of the line shows.
    async fn next_try(&self, on: &FakeClock, n: usize) {
        for _ in 0..40 {
            if self.launches().len() >= n {
                break;
            }
            on.advance(5_000);
            tick().await;
        }
        assert_eq!(self.launches().len(), n);
    }

    /// A launch the lead's agent took up: it announces itself, answers and exits.
    async fn taken(&self, n: usize) {
        let run = self.launches()[n].clone();
        run.push(&[init(), said(&format!("msg_t{n}"), "read it"), result("read it", 1, None)]);
        let rt = self.rt();
        until(|| async { done_count(&rt.rt, &self.lead_ws).await >= 2 }).await;
        run.end(Some(0));
        tick().await;
    }

    async fn owed(&self) -> usize {
        self.rt().store.list(NOTIFY_OWED).await.expect("a list").len()
    }

    /// The host goes down and a second one starts on the same store, on a clock that goes on from where the first stood.
    async fn restart(&self, next: &FakeClock) {
        self.close().await;
        let again = host(&self.parts(), next);
        *self.rt.borrow_mut() = again;
        self.rt().rt.sessions().list(Some(self.lead_ws.clone()), None).await.expect("a list");
    }

    /// Steps the clock a minute at a time for `minutes`, failing every launch of the line with `fails` as it shows.
    async fn keep_failing(&self, on: &FakeClock, minutes: usize, fails: fn() -> ExecFailure) {
        let mut failed: Vec<Rc<Run>> = vec![];
        for _ in 0..minutes {
            for run in self.launches() {
                if failed.iter().any(|f| Rc::ptr_eq(f, &run)) {
                    continue;
                }
                failed.push(run.clone());
                run.fail_with(fails());
            }
            tick().await;
            on.advance(MIN);
            tick().await;
        }
    }

    fn refuse_starts(&self) {
        *self.refusing.on.borrow_mut() = Some(self.lead_ws.clone());
    }

    fn refused_starts(&self) -> u32 {
        self.refusing.refused.get()
    }
}

// "a child's finished line its lead cannot take yet" > "a steer whose write landed but whose answer was lost reaches the lead once, and the person is told nothing"
#[test]
fn a_steer_whose_write_landed_but_whose_answer_was_lost_reaches_the_lead_once_and_the_person_is_told_nothing() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_run.loses.set(Some(Loses::Landed));
        s.kid_ends(None, None).await;
        assert_eq!(s.steers().len(), 1);
        let uuid = serde_json::from_str::<Value>(&s.steers()[0]).expect("a json line")["uuid"].as_str().expect("a uuid").to_string();
        s.lead_run.push(&[
            lifecycle(&uuid, "queued"),
            lifecycle(&uuid, "started"),
            said("msg_1", "read it"),
            result("read it", 1, None),
            lifecycle(&uuid, "completed"),
        ]);
        let rt = s.rt();
        until(|| async { has_type(&rt.rt, &s.lead_ws, "session.done").await }).await;
        s.lead_run.end(Some(0));
        tick().await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.steers().len(), 1);
        assert!(s.launches().is_empty());
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a steer whose write never landed reaches the lead once, as its next turn, once the turn it was steered into ends"
#[test]
fn a_steer_whose_write_never_landed_reaches_the_lead_once_as_its_next_turn_once_the_turn_it_was_steered_into_ends() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_run.loses.set(Some(Loses::Lost));
        s.kid_ends(None, None).await;
        assert!(s.steers().is_empty());
        // The lead's turn ends without its agent ever taking the message.
        s.lead_exits().await;
        until(|| async { s.launches().len() == 1 }).await;
        s.taken(0).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.steers().is_empty());
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a launch that fails after its start answered keeps the line owed, and the next try reaches the lead once"
#[test]
fn a_launch_that_fails_after_its_start_answered_keeps_the_line_owed_and_the_next_try_reaches_the_lead_once() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        assert_eq!(s.launches().len(), 1);
        // The box the lead runs on never answers the launch's posts: the stream fails with no word from the agent.
        s.launches()[0].fail_with(launch_lost());
        tick().await;
        assert_eq!(s.owed().await, 1);
        s.next_try(&at, 2).await;
        s.taken(1).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 2);
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a launch that fails after the lead's agent announced itself is not sent again: the agent had the line"
#[test]
fn a_launch_that_fails_after_the_leads_agent_announced_itself_is_not_sent_again_the_agent_had_the_line() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        s.launches()[0].push(&[init()]);
        let rt = s.rt();
        until(|| async { history(&rt.rt, &s.lead_ws).await.iter().filter(|e| e["type"] == "session.start").count() == 2 }).await;
        s.launches()[0].fail_with(launch_lost());
        tick().await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a steer whose write never landed is still sent back once when the host restarts before the turn it was steered into ends"
#[test]
fn a_steer_whose_write_never_landed_is_still_sent_back_once_when_the_host_restarts_before_the_turn_it_was_steered_into_ends() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_run.loses.set(Some(Loses::Lost));
        s.kid_ends(None, None).await;
        s.restart(&FakeClock::new(a.now())).await;
        // The second host reads the lead's run again; its turn ends without its agent ever taking the message.
        s.lead_run.push(&[said("msg_l", "waiting"), result("waiting", 1, None)]);
        tick().await;
        s.lead_run.end(Some(0));
        until(|| async { s.launches().len() == 1 }).await;
        s.taken(0).await;
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.steers().is_empty());
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a launch whose agent had not announced itself when the host restarted is read on the next host, and the line is not sent again"
#[test]
fn a_launch_whose_agent_had_not_announced_itself_when_the_host_restarted_is_read_on_the_next_host_and_the_line_is_not_sent_again() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        assert_eq!(s.launches().len(), 1);
        s.restart(&FakeClock::new(a.now())).await;
        tick().await;
        assert_eq!(s.launches().len(), 1);
        s.launches()[0].push(&[init(), said("msg_t0", "read it"), result("read it", 1, None)]);
        let rt = s.rt();
        until(|| async { done_count(&rt.rt, &s.lead_ws).await >= 2 }).await;
        s.launches()[0].end(Some(0));
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.to_person(None).await.is_empty());
        until(|| async { s.owed().await == 0 }).await;
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a line still owed when the host restarts reaches the lead from the next host, once"
#[test]
fn a_line_still_owed_when_the_host_restarts_reaches_the_lead_from_the_next_host_once() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        s.launches()[0].fail_with(launch_lost());
        tick().await;
        // The host stops before the line's next try.
        s.restart(&FakeClock::new(a.now() + 5 * MIN)).await;
        until(|| async { s.launches().len() == 2 }).await;
        s.taken(1).await;
        tick().await;
        assert_eq!(s.launches().len(), 2);
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a line its lead's computer keeps failing tells the person once, an hour of wall time after it became owed and not an hour of the next host, and is never sent after"
#[test]
fn a_line_its_leads_computer_keeps_failing_tells_the_person_once_an_hour_of_wall_time_after_it_became_owed_and_not_an_hour_of_the_next_host_and_is_never_sent_after(
) {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        // Each launch takes long minutes to fail, so the hour holds four tries, two on each host.
        a.advance(20 * MIN);
        s.launches()[0].fail_with(launch_lost());
        tick().await;
        s.next_try(&a, 2).await;
        a.advance(10 * MIN);
        s.launches()[1].fail_with(launch_lost());
        tick().await;
        assert!(s.to_person(None).await.is_empty());
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        s.next_try(&b, 3).await;
        b.advance(25 * MIN);
        s.launches()[2].fail_with(launch_lost());
        tick().await;
        assert!(s.to_person(None).await.is_empty());
        s.next_try(&b, 4).await;
        // The fourth try is still out at the hour the line has been owed, half an hour into the second host's own: it is stopped there and
        // the person is told.
        b.advance(10 * MIN);
        s.launches()[3].fail_with(launch_lost());
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        let told = s.to_person(None).await;
        let head = format!("thread {} finished (completed", &s.kid_thread()[..8]);
        assert!(told[0].starts_with(&head) && told[0].ends_with(&format!("): {REPORT}")), "{}", told[0]);
        b.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 4);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a line whose start the lead's agent refuses goes to the person at once and is never tried again"
#[test]
fn a_line_whose_start_the_leads_agent_refuses_goes_to_the_person_at_once_and_is_never_tried_again() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        // The agent refuses to start on the lead once the child works, so the line's own start is refused outright.
        s.kid_ends(None, Some(&|| s.refuse_starts())).await;
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        assert_eq!(s.refused_starts(), 1);
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.refused_starts(), 1);
        assert!(s.launches().is_empty());
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a steer whose write outlasts the lead's reply and the window its held reply waits reaches the lead once, as its next turn"
#[test]
fn a_steer_whose_write_outlasts_the_leads_reply_and_the_window_its_held_reply_waits_reaches_the_lead_once_as_its_next_turn() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts { result_exit_ms: Some(200), ..Default::default() }).await;
        let land = Gate::new();
        *s.lead_run.gate.borrow_mut() = Some(land.clone());
        s.kid_ends(None, None).await;
        // The lead's agent gives its last word while the write is on its way, and its held reply stands before the write answers: the turn
        // ends with the message unread.
        s.lead_run.push(&[said("msg_l", "waiting"), result("waiting", 1, None)]);
        sleep_ms(600).await;
        *s.lead_run.gate.borrow_mut() = None;
        land.open();
        s.lead.finished().await;
        until(|| async { !s.launches().is_empty() }).await;
        s.taken(0).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a steer whose write outlasts the lead's process reaches the lead once, as its next turn"
#[test]
fn a_steer_whose_write_outlasts_the_leads_process_reaches_the_lead_once_as_its_next_turn() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        let land = Gate::new();
        *s.lead_run.gate.borrow_mut() = Some(land.clone());
        s.kid_ends(None, None).await;
        // The lead replies and its process exits while the write is on its way.
        s.lead_exits().await;
        tick().await;
        *s.lead_run.gate.borrow_mut() = None;
        land.open();
        until(|| async { !s.launches().is_empty() }).await;
        s.taken(0).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a try whose launch the lead's computer never answers is stopped at the line's hour, and the person is told then"
#[test]
fn a_try_whose_launch_the_leads_computer_never_answers_is_stopped_at_the_lines_hour_and_the_person_is_told_then() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        assert_eq!(s.launches().len(), 1);
        // The launch's posts went out and nothing answers either way: the stream neither runs nor fails.
        at.advance(59 * MIN);
        tick().await;
        assert!(s.to_person(None).await.is_empty());
        at.advance(2 * MIN);
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a launch the lead's computer refuses goes to the person at once and is never tried again"
#[test]
fn a_launch_the_leads_computer_refuses_goes_to_the_person_at_once_and_is_never_tried_again() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        s.launches()[0].fail_with(launch_refused());
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "the person's stop of the lead's turn carrying the line holds: the line is not launched again, and the person is told it"
#[test]
fn the_persons_stop_of_the_leads_turn_carrying_the_line_holds_the_line_is_not_launched_again_and_the_person_is_told_it() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        assert_eq!(s.launches().len(), 1);
        let rows: Vec<Value> = s.rt().rt.sessions().list(Some(s.lead_ws.clone()), None).await.expect("a list").iter().map(j).collect();
        let row = rows.iter().find(|v| v["threadId"] == s.lead_thread.as_str() && v["status"] == "running").expect("the running row");
        s.rt().rt.sessions().interrupt(row["id"].as_str().expect("an id"), None, None).await.expect("a stop");
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a lead started with --notify me hears nothing of the line's failed tries, and the person gets the one row of the child's line at the hour"
#[test]
fn a_lead_started_with_notify_me_hears_nothing_of_the_lines_failed_tries_and_the_person_gets_the_one_row_of_the_childs_line_at_the_hour() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts { lead_notify: Some(vec![NOTIFY_ME.into()]), ..Default::default() }).await;
        s.lead_waits().await;
        let lead_thread = s.lead_thread.clone();
        let before = s.to_person(Some(&lead_thread)).await;
        assert_eq!(before.len(), 1);
        s.kid_ends(None, None).await;
        s.keep_failing(&at, 70, launch_lost).await;
        assert!(s.launches().len() > 5);
        assert_eq!(s.to_person(Some(&lead_thread)).await, before);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a restart keeps the spacing of the line's tries: the next host does not start it over at the first wait"
#[test]
fn a_restart_keeps_the_spacing_of_the_lines_tries_the_next_host_does_not_start_it_over_at_the_first_wait() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        s.launches()[0].fail_with(launch_lost());
        s.next_try(&a, 2).await;
        s.launches()[1].fail_with(launch_lost());
        s.next_try(&a, 3).await;
        s.launches()[2].fail_with(launch_lost());
        tick().await;
        // Three tries failed: the next waits two minutes, and a restart in the middle of that wait keeps it.
        a.advance(30_000);
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        assert_eq!(s.launches().len(), 3);
        b.advance(60_000);
        tick().await;
        assert_eq!(s.launches().len(), 3);
        s.next_try(&b, 4).await;
        s.launches()[3].fail_with(launch_lost());
        tick().await;
        // The fourth failure waits four minutes, on the second host as on the first.
        b.advance(200_000);
        tick().await;
        assert_eq!(s.launches().len(), 4);
        s.next_try(&b, 5).await;
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a child's lines into two leads that fall on two hosts tell the person once"
#[test]
fn a_childs_lines_into_two_leads_that_fall_on_two_hosts_tell_the_person_once() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        let rt = s.rt();
        let other_ws = create_on(&rt.rt, lead_of("lead-2"), None).await.expect("a workspace").view.id;
        let other = rt
            .rt
            .sessions()
            .start(&other_ws, StartOptions { prompt: "coordinate".into(), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { s.fed.run_of("coordinate").is_some() }).await;
        let other_run = s.fed.run_of("coordinate").expect("the run");
        other_run.push(&[init(), said("msg_o", "waiting"), result("waiting", 1, None)]);
        until(|| async { has_type(&rt.rt, &other_ws, "session.done").await }).await;
        other_run.end(Some(0));
        other.finished().await;
        s.kid_ends(Some(vec![s.lead_thread.clone(), other.view().thread_id.expect("a thread")]), None).await;
        until(|| async { s.launches().len() == 2 }).await;
        let into = |ws: &str| s.launches().into_iter().find(|r| r.env.get("LAB_WORKSPACE").is_some_and(|w| w == ws)).expect("the launch");
        // The first lead's computer refuses its launch: the person is told now. The second's never answers.
        into(&s.lead_ws).fail_with(launch_refused());
        into(&other_ws).fail_with(launch_lost());
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        tick().await;
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        s.keep_failing(&b, 70, launch_lost).await;
        assert!(s.launches().len() > 3);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// ===== a Codex lead =====

/// A host over a store, the real Claude Code and Codex adapters over runs fed by hand.
fn codex_host(backend: &Arc<StubBackend>, state: &State, exec: &Arc<dyn ExecStreamFactory>, on: &FakeClock) -> Fx {
    let (for_claude, for_codex) = (exec.clone(), exec.clone());
    let claude: HarnessAdapterFactory = Arc::new(move |_ctx| claude_over(for_claude.clone(), 60_000));
    let codex: HarnessAdapterFactory = Arc::new(move |_ctx| codex_over(for_codex.clone()));
    runtime(backend).store(&state.store).clock(on).adapter("claude", claude).adapter("codex", codex).build()
}

async fn person_rows(rt: &Rc<AssembledRuntime>, kid_ws: &str, lead_ws: &str, kid_thread: &str) -> usize {
    let mut all = history(rt, kid_ws).await;
    all.extend(history(rt, lead_ws).await);
    all.iter().filter(|e| e["type"] == "session.notify" && e["notify"] == NOTIFY_ME && e["threadId"] == kid_thread).count()
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's line whose steer never answered when the host restarted reaches the person when the lead's turn ends, never nobody"
#[test]
fn a_codex_leads_line_whose_steer_never_answered_when_the_host_restarted_reaches_the_person_when_the_leads_turn_ends_never_nobody() {
    drive(|| async {
        let (factory, fed) = fed_runs();
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let a = FakeClock::at_now();
        let mut rt = codex_host(&backend, &state, &factory, &a);
        let lead_ws = create_on(&rt.rt, lead_of("lead"), None).await.expect("a workspace").view.id;
        let lead = rt
            .rt
            .sessions()
            .start(&lead_ws, StartOptions { prompt: "orchestrate".into(), harness: Some("codex".into()), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { fed.runs.borrow().len() == 1 }).await;
        let lead_run = fed.runs.borrow()[0].clone();
        lead_run.push(&opened());
        until(|| async { writes_of(&lead_run, "turn/start") }).await;
        lead_run.push(&[turn_started()]);
        until(|| async { has_type(&rt.rt, &lead_ws, "session.start").await }).await;
        let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
        // The child's line steers into the lead's turn, and that write never lands and never answers.
        lead_run.loses.set(Some(Loses::Lost));
        let opts = StartOptions {
            prompt: "parse the files".into(),
            notify: Some(vec![lead.view().thread_id.expect("a thread")]),
            ..Default::default()
        };
        let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
        let kid_thread = kid.view().thread_id.expect("a thread");
        until(|| async { fed.run_of("parse the files").is_some() }).await;
        let kid_run = fed.run_of("parse the files").expect("the run");
        kid_run.push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
        until(|| async {
            history(&rt.rt, &kid_ws).await.iter().any(|e| e["type"] == "session.done" && e["threadId"] == kid_thread.as_str())
        })
        .await;
        kid_run.end(Some(0));
        kid.finished().await;
        tick().await;
        assert_eq!(rt.store.list(NOTIFY_OWED).await.expect("a list").len(), 1);
        let runs_before = fed.runs.borrow().len();
        rt.rt.close().await.expect("a close");
        rt = codex_host(&backend, &state, &factory, &FakeClock::new(a.now()));
        rt.rt.sessions().list(Some(lead_ws.clone()), None).await.expect("a list");
        tick().await;
        // The lead's turn goes on and ends on the second host; its agent never had the line.
        lead_run.push(&[agent_message(), completed()]);
        tick().await;
        lead_run.end(Some(0));
        until(|| async { person_rows(&rt.rt, &kid_ws, &lead_ws, &kid_thread).await == 1 }).await;
        let later: Vec<Rc<Run>> = fed.runs.borrow()[runs_before..].to_vec();
        for r in &later {
            r.push(&opened());
        }
        tick().await;
        assert!(fed.runs.borrow()[runs_before..].is_empty());
        assert_eq!(fed.runs.borrow().iter().map(|r| r.writes().iter().filter(|w| w.contains(REPORT)).count()).sum::<usize>(), 0);
        assert_eq!(person_rows(&rt.rt, &kid_ws, &lead_ws, &kid_thread).await, 1);
        assert!(rt.store.list(NOTIFY_OWED).await.expect("a list").is_empty());
        rt.rt.close().await.expect("a close");
    });
}

// ===== after a restart =====

// "a child's finished line its lead cannot take yet" > "after a restart, the person's stop of the re-opened turn carrying the line holds: it is not launched again, and the person is told it"
#[test]
fn after_a_restart_the_persons_stop_of_the_re_opened_turn_carrying_the_line_holds_it_is_not_launched_again_and_the_person_is_told_it() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        assert_eq!(s.launches().len(), 1);
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        let rows: Vec<Value> = s.rt().rt.sessions().list(Some(s.lead_ws.clone()), None).await.expect("a list").iter().map(j).collect();
        let row = rows.iter().find(|v| v["threadId"] == s.lead_thread.as_str() && v["status"] == "running").expect("the running row");
        s.rt().rt.sessions().interrupt(row["id"].as_str().expect("an id"), None, None).await.expect("a stop");
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        b.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "after a restart, a re-opened turn carrying the line that the lead's computer refuses goes to the person at once and is never launched again"
#[test]
fn after_a_restart_a_re_opened_turn_carrying_the_line_that_the_leads_computer_refuses_goes_to_the_person_at_once_and_is_never_launched_again(
) {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        s.launches()[0].fail_with(launch_refused());
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        b.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "after a restart, a re-opened turn carrying the line that the lead's computer never answers is stopped at the line's hour, and the person is told then"
#[test]
fn after_a_restart_a_re_opened_turn_carrying_the_line_that_the_leads_computer_never_answers_is_stopped_at_the_lines_hour_and_the_person_is_told_then(
) {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        b.advance(59 * MIN);
        tick().await;
        assert!(s.to_person(None).await.is_empty());
        b.advance(2 * MIN);
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        b.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "after a restart, a re-opened turn carrying the line that the lead's computer did not answer goes again on the clock"
#[test]
fn after_a_restart_a_re_opened_turn_carrying_the_line_that_the_leads_computer_did_not_answer_goes_again_on_the_clock() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        s.launches()[0].fail_with(launch_lost());
        s.next_try(&b, 2).await;
        s.taken(1).await;
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a Claude Code lead whose CLI tells no unread messages: a steer whose write never answered tells the person, as main did, and is never lost"
#[test]
fn a_claude_code_lead_whose_cli_tells_no_unread_messages_a_steer_whose_write_never_answered_tells_the_person_as_main_did_and_is_never_lost()
{
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts { lead_init: Some(old_init()), ..Default::default() }).await;
        s.lead_run.loses.set(Some(Loses::Lost));
        s.kid_ends(None, None).await;
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        s.lead_run.push(&[said("msg_l", "waiting"), result("waiting", 1, None)]);
        tick().await;
        s.lead_run.end(Some(0));
        for _ in 0..30 {
            b.advance(5 * MIN);
            tick().await;
        }
        assert!(s.launches().is_empty());
        assert!(s.steers().is_empty());
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a line whose try failed and whose lead then naps past its hour gets a try at the wake, as a line that never failed does"
#[test]
fn a_line_whose_try_failed_and_whose_lead_then_naps_past_its_hour_gets_a_try_at_the_wake_as_a_line_that_never_failed_does() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        s.launches()[0].fail_with(launch_lost());
        tick().await;
        s.rt().rt.workspaces().nap(&s.lead_ws, None).await.expect("a nap");
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        s.rt().rt.workspaces().wake(&s.lead_ws, None).await.expect("a wake");
        until(|| async { s.launches().len() == 2 }).await;
        s.taken(1).await;
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// ===== a Codex lead's line try =====

struct CodexLead {
    fed: FedRuns,
    rt: Fx,
    state: Rc<State>,
    kid_ws: String,
    lead_ws: String,
    kid_thread: String,
    at: FakeClock,
    try_run: Rc<Run>,
}

/// A Codex lead whose first turn has ended, and a child of it that ended with REPORT: the line's first try is the lead's third run.
async fn codex_lead() -> CodexLead {
    let (factory, fed) = fed_runs();
    let backend = StubBackend::new(None);
    let state = Rc::new(sqlite_state());
    let at = FakeClock::at_now();
    let rt = codex_host(&backend, &state, &factory, &at);
    let lead_ws = create_on(&rt.rt, lead_of("lead"), None).await.expect("a workspace").view.id;
    let lead = rt
        .rt
        .sessions()
        .start(&lead_ws, StartOptions { prompt: "orchestrate".into(), harness: Some("codex".into()), ..Default::default() }, None)
        .await
        .expect("a start");
    until(|| async { fed.runs.borrow().len() == 1 }).await;
    let lead_run = fed.runs.borrow()[0].clone();
    lead_run.push(&opened());
    until(|| async { writes_of(&lead_run, "turn/start") }).await;
    lead_run.push(&[turn_started(), agent_message(), completed()]);
    until(|| async { has_type(&rt.rt, &lead_ws, "session.done").await }).await;
    lead_run.end(Some(0));
    lead.finished().await;
    let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
    let opts = StartOptions {
        prompt: "parse the files".into(),
        notify: Some(vec![lead.view().thread_id.expect("a thread")]),
        ..Default::default()
    };
    let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
    let kid_thread = kid.view().thread_id.expect("a thread");
    until(|| async { fed.run_of("parse the files").is_some() }).await;
    let kid_run = fed.run_of("parse the files").expect("the run");
    kid_run.push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
    until(|| async { history(&rt.rt, &kid_ws).await.iter().any(|e| e["type"] == "session.done" && e["threadId"] == kid_thread.as_str()) })
        .await;
    kid_run.end(Some(0));
    kid.finished().await;
    tick().await;
    until(|| async { fed.runs.borrow().len() == 3 }).await;
    let try_run = fed.runs.borrow()[2].clone();
    CodexLead { fed, rt, state, kid_ws, lead_ws, kid_thread, at, try_run }
}

impl CodexLead {
    async fn to_person(&self) -> usize {
        person_rows(&self.rt.rt, &self.kid_ws, &self.lead_ws, &self.kid_thread).await
    }
    async fn steps(&self, n: usize, ms: i64) {
        for _ in 0..n {
            self.at.advance(ms);
            tick().await;
        }
    }
    fn runs(&self) -> usize {
        self.fed.runs.borrow().len()
    }
    async fn owed(&self) -> Vec<Value> {
        self.rt.store.list(NOTIFY_OWED).await.expect("a list").iter().map(|v| json!(format!("{v:?}"))).collect()
    }
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's server that opens the thread for the line's try and exits before the turn starts leaves the line to the person, never nobody"
#[test]
fn a_codex_leads_server_that_opens_the_thread_for_the_lines_try_and_exits_before_the_turn_starts_leaves_the_line_to_the_person_never_nobody(
) {
    drive(|| async {
        let c = codex_lead().await;
        c.try_run.push(&opened());
        until(|| async { writes_of(&c.try_run, "turn/start") }).await;
        tick().await;
        c.try_run.end(Some(1));
        until(|| async { c.to_person().await == 1 }).await;
        c.steps(30, 5 * MIN).await;
        assert_eq!(c.runs(), 3);
        assert_eq!(c.to_person().await, 1);
        assert!(c.owed().await.is_empty());
        c.rt.rt.close().await.expect("a close");
        let _ = &c.state;
    });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's server that opens the thread for the line's try and refuses its turn leaves the line to the person, never nobody"
#[test]
fn a_codex_leads_server_that_opens_the_thread_for_the_lines_try_and_refuses_its_turn_leaves_the_line_to_the_person_never_nobody() {
    drive(|| async {
        let c = codex_lead().await;
        c.try_run.push(&opened());
        until(|| async { writes_of(&c.try_run, "turn/start") }).await;
        c.try_run.push(&[r#"{"id":"wsp-turn","error":{"code":-32603,"message":"the model is not available on this plan"}}"#.into()]);
        tick().await;
        c.try_run.end(Some(0));
        until(|| async { c.to_person().await == 1 }).await;
        c.steps(30, 5 * MIN).await;
        assert_eq!(c.runs(), 3);
        assert_eq!(c.to_person().await, 1);
        assert!(c.owed().await.is_empty());
        c.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's launch of the line that its computer never answered is tried again, and the line is taken once the next try's turn starts"
#[test]
fn a_codex_leads_launch_of_the_line_that_its_computer_never_answered_is_tried_again_and_the_line_is_taken_once_the_next_trys_turn_starts() {
    drive(|| async {
        let c = codex_lead().await;
        c.try_run.fail_with(launch_lost());
        tick().await;
        c.steps(12, 10_000).await;
        assert_eq!(c.runs(), 4);
        let next = c.fed.runs.borrow()[3].clone();
        next.push(&opened());
        until(|| async { next.writes().iter().any(|w| w.contains("turn/start") && w.contains("Parsed 40 files")) }).await;
        assert_eq!(c.owed().await.len(), 1);
        next.push(&[turn_started()]);
        until(|| async { c.owed().await.is_empty() }).await;
        assert_eq!(c.to_person().await, 0);
        c.rt.rt.close().await.expect("a close");
    });
}

// ===== two hosts on one store =====

/// A host or two on one store, with the real Claude Code and Codex adapters over runs fed by hand, and an attach the case can make go
/// unanswered or hold until it lets go.
struct TwoHosts {
    fed: Rc<FedRuns>,
    factory: Arc<dyn ExecStreamFactory>,
    dark: Rc<Cell<bool>>,
    gate: Rc<RefCell<Option<Rc<Gate>>>>,
    backend: Arc<StubBackend>,
    state: Rc<State>,
}

fn two_hosts() -> TwoHosts {
    let (fed_factory, fed) = fed_runs();
    let (dark, gate): (Rc<Cell<bool>>, Rc<RefCell<Option<Rc<Gate>>>>) = (Rc::default(), Rc::default());
    let factory: Arc<dyn ExecStreamFactory> = Arc::new(Reopening { inner: fed_factory, dark: dark.clone(), gate: gate.clone() });
    TwoHosts { fed: Rc::new(fed), factory, dark, gate, backend: StubBackend::new(None), state: Rc::new(sqlite_state()) }
}

impl TwoHosts {
    fn host(&self, on: &FakeClock) -> Fx {
        codex_host(&self.backend, &self.state, &self.factory, on)
    }
    fn go_dark(&self) {
        self.dark.set(true);
    }
    fn hold(&self) {
        *self.gate.borrow_mut() = Some(Gate::new());
    }
    fn release(&self) {
        let held = self.gate.borrow_mut().take();
        if let Some(held) = held {
            held.open();
        }
    }
    async fn owed(&self) -> usize {
        self.state.store.list(NOTIFY_OWED).await.expect("a list").len()
    }
}

struct Dark {
    h: TwoHosts,
    b: FakeClock,
    lead_ws: String,
    kid_ws: String,
    kid_thread: String,
    rt: Fx,
}

/// A Claude Code lead, kept awake, whose child ended with REPORT; then the host goes down with the line's try out and unannounced, and
/// the next host comes up while the lead's computer answers nothing.
async fn dark_restart() -> Dark {
    let h = two_hosts();
    let a = FakeClock::at_now();
    let rt = h.host(&a);
    let lead_ws = create_on(&rt.rt, CreateOn { idle_window_ms: wsp_core::types::wiring::IdleWindow::Off, ..lead_of("lead") }, None)
        .await
        .expect("a workspace")
        .view
        .id;
    let lead =
        rt.rt.sessions().start(&lead_ws, StartOptions { prompt: "orchestrate".into(), ..Default::default() }, None).await.expect("a start");
    until(|| async { h.fed.run_of("orchestrate").is_some() }).await;
    h.fed.run_of("orchestrate").expect("the run").push(&[init(), said("msg_l", "waiting"), result("waiting", 1, None)]);
    until(|| async { has_type(&rt.rt, &lead_ws, "session.done").await }).await;
    h.fed.run_of("orchestrate").expect("the run").end(Some(0));
    lead.finished().await;
    let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
    let opts = StartOptions {
        prompt: "parse the files".into(),
        notify: Some(vec![lead.view().thread_id.expect("a thread")]),
        ..Default::default()
    };
    let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
    let kid_thread = kid.view().thread_id.expect("a thread");
    until(|| async { h.fed.run_of("parse the files").is_some() }).await;
    h.fed.run_of("parse the files").expect("the run").push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
    until(|| async { has_type(&rt.rt, &kid_ws, "session.done").await }).await;
    h.fed.run_of("parse the files").expect("the run").end(Some(0));
    kid.finished().await;
    tick().await;
    assert_eq!(h.fed.runs_of(REPORT).len(), 1);
    h.go_dark();
    rt.rt.close().await.expect("a close");
    let b = FakeClock::new(a.now());
    let rt = h.host(&b);
    rt.rt.sessions().list(Some(lead_ws.clone()), None).await.expect("a list");
    tick().await;
    Dark { h, b, lead_ws, kid_ws, kid_thread, rt }
}

impl Dark {
    async fn to_person(&self) -> usize {
        person_rows(&self.rt.rt, &self.kid_ws, &self.lead_ws, &self.kid_thread).await
    }
    async fn ends_of_lead(&self) -> usize {
        history(&self.rt.rt, &self.lead_ws).await.iter().filter(|e| e["type"] == "session.end").count()
    }
}

// "a child's finished line its lead cannot take yet" > "after a restart whose re-open of the line's try the lead's computer never answers, the person is told at the line's hour and the line is settled"
#[test]
fn after_a_restart_whose_re_open_of_the_lines_try_the_leads_computer_never_answers_the_person_is_told_at_the_lines_hour_and_the_line_is_settled(
) {
    drive(|| async {
        let d = dark_restart().await;
        let mut told_at: Option<i64> = None;
        let mut minute = 5;
        while minute <= 120 {
            d.b.advance(5 * MIN);
            tick().await;
            if told_at.is_none() && d.to_person().await > 0 {
                told_at = Some(minute);
            }
            minute += 5;
        }
        let told = told_at.expect("the person was told");
        assert!(told <= 65, "told at minute {told}");
        assert_eq!(d.to_person().await, 1);
        assert_eq!(d.h.fed.runs_of(REPORT).len(), 1);
        assert_eq!(d.h.owed().await, 0);
        // The try is out of reach: the host writes no end for it, and the lead's one end is its first turn's.
        assert_eq!(d.ends_of_lead().await, 1);
        d.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "after a restart whose re-open of the line's try the lead's computer never answers, the person's stop of that row settles the line, tells them, and writes no end for the turn"
#[test]
fn after_a_restart_whose_re_open_of_the_lines_try_the_leads_computer_never_answers_the_persons_stop_of_that_row_settles_the_line_tells_them_and_writes_no_end_for_the_turn(
) {
    drive(|| async {
        let d = dark_restart().await;
        let rows: Vec<Value> = d.rt.rt.sessions().list(Some(d.lead_ws.clone()), None).await.expect("a list").iter().map(j).collect();
        let row = rows.iter().find(|v| v["status"] == "running").expect("the running row");
        let stopped = d.rt.rt.sessions().interrupt(row["id"].as_str().expect("an id"), None, None).await.expect("a stop");
        assert_eq!(j(&stopped), json!({ "outcome": "not-running" }));
        until(|| async { d.to_person().await == 1 }).await;
        for _ in 0..20 {
            d.b.advance(MIN);
            tick().await;
        }
        assert_eq!(d.to_person().await, 1);
        assert_eq!(d.h.fed.runs_of(REPORT).len(), 1);
        assert_eq!(d.h.owed().await, 0);
        assert_eq!(d.ends_of_lead().await, 1);
        d.rt.rt.close().await.expect("a close");
    });
}

/// The rows a turn wrote, in order.
fn rows_of(history: &[Value], turn: &str) -> Vec<String> {
    history
        .iter()
        .filter(|e| e["turnId"] == turn)
        .map(|e| {
            if e["type"] == "session.delta" {
                format!("delta:{}", e["kind"].as_str().unwrap_or_default())
            } else {
                e["type"].as_str().unwrap_or_default().to_string()
            }
        })
        .collect()
}

// "a child's finished line its lead cannot take yet" > "a stop the person sends while the next host's re-open of a turn is still out writes no end of its own: the turn ends once, as its agent ends it"
#[test]
fn a_stop_the_person_sends_while_the_next_hosts_re_open_of_a_turn_is_still_out_writes_no_end_of_its_own_the_turn_ends_once_as_its_agent_ends_it(
) {
    drive(|| async {
        let h = two_hosts();
        let a = FakeClock::at_now();
        let mut rt = h.host(&a);
        let ws = create_on(&rt.rt, CreateOn { idle_window_ms: wsp_core::types::wiring::IdleWindow::Off, ..CreateOn::golden("app") }, None)
            .await
            .expect("a workspace")
            .view
            .id;
        let turn = rt
            .rt
            .sessions()
            .start(&ws, StartOptions { prompt: "build the parser".into(), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { h.fed.run_of("build the parser").is_some() }).await;
        let run = h.fed.run_of("build the parser").expect("the run");
        run.push(&[init(), said("msg_1", "Reading the grammar first.")]);
        until(|| async { history(&rt.rt, &ws).await.iter().any(|e| e["type"] == "session.delta") }).await;
        let rows: Vec<Value> = rt.rt.sessions().list(Some(ws.clone()), None).await.expect("a list").iter().map(j).collect();
        let row_id = rows.iter().find(|v| v["status"] == "running").expect("the running row")["id"].as_str().expect("an id").to_string();
        rt.rt.close().await.expect("a close");
        h.hold();
        rt = h.host(&FakeClock::new(a.now()));
        let stopping = {
            let (rt, id) = (rt.rt.clone(), row_id.clone());
            tokio::task::spawn_local(async move { rt.sessions().interrupt(&id, None, None).await })
        };
        tick().await;
        h.release();
        stopping.await.expect("the stop runs").expect("a stop");
        tick().await;
        run.push(&[said("msg_2", "Parser done: three files changed."), result("Parser done: three files changed.", 1, None)]);
        tick().await;
        run.end(Some(0));
        until(|| async { has_type(&rt.rt, &ws, "session.end").await }).await;
        for _ in 0..5 {
            tick().await;
        }
        let rows = rows_of(&history(&rt.rt, &ws).await, &turn.turn_id());
        assert_eq!(rows.iter().filter(|r| *r == "session.end").count(), 1);
        assert_eq!(rows.last().map(String::as_str), Some("session.end"));
        rt.rt.close().await.expect("a close");
    });
}

/// A lead whose child ended with REPORT; the host goes down with the line's try out, and its agent takes the line and answers it
/// meanwhile. The next host comes up `after` minutes later, and its re-open of the try answers only once a timer due at once has fired,
/// as a cloud machine still restoring does.
async fn up_later(after: i64) -> (Vec<String>, Vec<Value>, usize, usize) {
    let h = two_hosts();
    let a = FakeClock::at_now();
    let mut rt = h.host(&a);
    let lead_ws = create_on(&rt.rt, CreateOn { idle_window_ms: wsp_core::types::wiring::IdleWindow::Off, ..lead_of("lead") }, None)
        .await
        .expect("a workspace")
        .view
        .id;
    let lead =
        rt.rt.sessions().start(&lead_ws, StartOptions { prompt: "orchestrate".into(), ..Default::default() }, None).await.expect("a start");
    until(|| async { h.fed.run_of("orchestrate").is_some() }).await;
    h.fed.run_of("orchestrate").expect("the run").push(&[init(), said("msg_l", "waiting"), result("waiting", 1, None)]);
    until(|| async { has_type(&rt.rt, &lead_ws, "session.done").await }).await;
    h.fed.run_of("orchestrate").expect("the run").end(Some(0));
    lead.finished().await;
    let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
    let opts = StartOptions {
        prompt: "parse the files".into(),
        notify: Some(vec![lead.view().thread_id.expect("a thread")]),
        ..Default::default()
    };
    let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
    let kid_thread = kid.view().thread_id.expect("a thread");
    until(|| async { h.fed.run_of("parse the files").is_some() }).await;
    h.fed.run_of("parse the files").expect("the run").push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
    until(|| async { has_type(&rt.rt, &kid_ws, "session.done").await }).await;
    h.fed.run_of("parse the files").expect("the run").end(Some(0));
    kid.finished().await;
    tick().await;
    let try_run = h.fed.runs_of(REPORT)[0].clone();
    rt.rt.close().await.expect("a close");
    h.hold();
    let b = FakeClock::new(a.now() + after * MIN);
    rt = h.host(&b);
    let listing = {
        let (rt, ws) = (rt.rt.clone(), lead_ws.clone());
        tokio::task::spawn_local(async move { rt.sessions().list(Some(ws), None).await })
    };
    tick().await;
    b.advance(1);
    tick().await;
    h.release();
    tick().await;
    try_run.push(&[
        init(),
        said("msg_t", "Read the parser report; merging it next."),
        result("Read the parser report; merging it next.", 1, None),
    ]);
    tick().await;
    try_run.end(Some(0));
    for _ in 0..10 {
        tick().await;
        b.advance(10_000);
    }
    let _ = listing.await;
    let lead_history = history(&rt.rt, &lead_ws).await;
    let try_turn =
        lead_history.iter().rfind(|e| e["type"] == "session.end").expect("an end")["turnId"].as_str().expect("a turn").to_string();
    let to_person: Vec<Value> = history(&rt.rt, &kid_ws)
        .await
        .into_iter()
        .filter(|e| e["type"] == "session.notify" && e["notify"] == NOTIFY_ME && e["threadId"] == kid_thread.as_str())
        .collect();
    let rows = rows_of(&lead_history, &try_turn);
    let owed = h.owed().await;
    let launches = h.fed.runs_of(REPORT).len();
    rt.rt.close().await.expect("a close");
    (rows, to_person, launches, owed)
}

fn took_the_line(r: (Vec<String>, Vec<Value>, usize, usize)) {
    let (rows, to_person, launches, owed) = r;
    assert!(to_person.is_empty());
    assert_eq!(rows, ["session.start", "delta:text", "session.done", "session.end"]);
    assert_eq!(launches, 1);
    assert_eq!(owed, 0);
}

// "a child's finished line its lead cannot take yet" > "a next host that comes up past the line's hour reads the try only once its re-open answered: the lead took the line, one turn, nobody told it failed"
#[test]
fn a_next_host_that_comes_up_past_the_lines_hour_reads_the_try_only_once_its_re_open_answered_the_lead_took_the_line_one_turn_nobody_told_it_failed(
) {
    drive(|| async { took_the_line(up_later(61).await) });
}

// "a child's finished line its lead cannot take yet" > "a next host that comes up within the line's hour reads the try only once its re-open answered: the lead took the line, one turn, nobody told it failed"
#[test]
fn a_next_host_that_comes_up_within_the_lines_hour_reads_the_try_only_once_its_re_open_answered_the_lead_took_the_line_one_turn_nobody_told_it_failed(
) {
    drive(|| async { took_the_line(up_later(30).await) });
}

async fn warning_case(w: Warning) {
    let h = two_hosts();
    let rt = h.host(&FakeClock::at_now());
    let ws = create_on(&rt.rt, CreateOn::golden("lead"), None).await.expect("a workspace").view.id;
    let lead = rt
        .rt
        .sessions()
        .start(&ws, StartOptions { prompt: "orchestrate".into(), harness: Some("codex".into()), ..Default::default() }, None)
        .await
        .expect("a start");
    until(|| async { h.fed.runs.borrow().len() == 1 }).await;
    let run = h.fed.runs.borrow()[0].clone();
    let lines =
        if w.early { vec![opened()[0].clone(), w.line.clone(), opened()[1].clone()] } else { [opened(), vec![w.line.clone()]].concat() };
    run.push(&lines);
    until(|| async { writes_of(&run, "turn/start") }).await;
    run.push(&[turn_started(), agent_message(), completed()]);
    until(|| async { has_type(&rt.rt, &ws, "session.done").await }).await;
    run.end(Some(0));
    lead.finished().await;
    let rows: Vec<String> = history(&rt.rt, &ws)
        .await
        .iter()
        .filter(|e| e["type"] == "session.start" || e["type"] == "session.delta" || e["type"] == "session.done")
        .map(|e| {
            if e["type"] == "session.delta" {
                format!("delta:{}", e["kind"].as_str().unwrap_or_default())
            } else {
                e["type"].as_str().unwrap_or_default().to_string()
            }
        })
        .collect();
    assert_eq!(rows, ["session.start", "delta:note", "delta:text", "session.done"]);
    rt.rt.close().await.expect("a close");
}

// "a child's finished line its lead cannot take yet" > "a Codex turn whose server prints app-server-turn's warning after the thread answered writes its start row first, and the note after it"
#[test]
fn a_codex_turn_whose_server_prints_app_server_turns_warning_after_the_thread_answered_writes_its_start_row_first_and_the_note_after_it() {
    drive(|| async { warning_case(warnings().remove(0)).await });
}

// "a child's finished line its lead cannot take yet" > "a Codex turn whose server prints app-server-subagent's warning after the thread answered writes its start row first, and the note after it"
#[test]
fn a_codex_turn_whose_server_prints_app_server_subagents_warning_after_the_thread_answered_writes_its_start_row_first_and_the_note_after_it(
) {
    drive(|| async { warning_case(warnings().remove(1)).await });
}

// "a child's finished line its lead cannot take yet" > "a Codex turn whose server prints the startup configWarning before the thread answered writes its start row first, and the note after it"
#[test]
fn a_codex_turn_whose_server_prints_the_startup_config_warning_before_the_thread_answered_writes_its_start_row_first_and_the_note_after_it()
{
    drive(|| async { warning_case(warnings().remove(2)).await });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's line try whose server warned after the thread answered, and a host restart before turn/started: the line is taken once, by the turn that starts"
#[test]
fn a_codex_leads_line_try_whose_server_warned_after_the_thread_answered_and_a_host_restart_before_turn_started_the_line_is_taken_once_by_the_turn_that_starts(
) {
    drive(|| async {
        let h = two_hosts();
        let a = FakeClock::at_now();
        let mut rt = h.host(&a);
        let lead_ws = create_on(&rt.rt, lead_of("lead"), None).await.expect("a workspace").view.id;
        let lead = rt
            .rt
            .sessions()
            .start(&lead_ws, StartOptions { prompt: "orchestrate".into(), harness: Some("codex".into()), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { h.fed.runs.borrow().len() == 1 }).await;
        let lead_run = h.fed.runs.borrow()[0].clone();
        lead_run.push(&opened());
        until(|| async { writes_of(&lead_run, "turn/start") }).await;
        lead_run.push(&[turn_started(), agent_message(), completed()]);
        until(|| async { has_type(&rt.rt, &lead_ws, "session.done").await }).await;
        lead_run.end(Some(0));
        lead.finished().await;
        let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
        let opts = StartOptions {
            prompt: "parse the files".into(),
            notify: Some(vec![lead.view().thread_id.expect("a thread")]),
            ..Default::default()
        };
        let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
        let kid_thread = kid.view().thread_id.expect("a thread");
        until(|| async { h.fed.run_of("parse the files").is_some() }).await;
        h.fed.run_of("parse the files").expect("the run").push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
        until(|| async {
            history(&rt.rt, &kid_ws).await.iter().any(|e| e["type"] == "session.done" && e["threadId"] == kid_thread.as_str())
        })
        .await;
        h.fed.run_of("parse the files").expect("the run").end(Some(0));
        kid.finished().await;
        tick().await;
        until(|| async { h.fed.runs.borrow().len() == 3 }).await;
        let try_run = h.fed.runs.borrow()[2].clone();
        let mut first = opened();
        first.push(warnings().remove(0).line);
        try_run.push(&first);
        until(|| async { try_run.writes().iter().any(|w| w.contains("turn/start") && w.contains("Parsed 40 files")) }).await;
        tick().await;
        assert_eq!(h.owed().await, 1);
        rt.rt.close().await.expect("a close");
        let b = FakeClock::new(a.now());
        rt = h.host(&b);
        rt.rt.sessions().list(Some(lead_ws.clone()), None).await.expect("a list");
        tick().await;
        try_run.push(&[turn_started(), agent_message(), completed()]);
        until(|| async { h.owed().await == 0 }).await;
        try_run.end(Some(0));
        for _ in 0..20 {
            b.advance(5 * MIN);
            tick().await;
        }
        let lead_history = history(&rt.rt, &lead_ws).await;
        assert_eq!(lead_history.iter().filter(|e| e["type"] == "session.start").count(), 2);
        let mut all = history(&rt.rt, &kid_ws).await;
        all.extend(lead_history);
        assert!(all.iter().all(|e| !(e["type"] == "session.notify" && e["notify"] == NOTIFY_ME && e["threadId"] == kid_thread.as_str())));
        assert_eq!(h.fed.runs.borrow().len(), 3);
        rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's line try whose server opened the thread and whose launch then lost its computer is tried again under the same request, as a fresh turn"
#[test]
fn a_codex_leads_line_try_whose_server_opened_the_thread_and_whose_launch_then_lost_its_computer_is_tried_again_under_the_same_request_as_a_fresh_turn(
) {
    drive(|| async {
        let c = codex_lead().await;
        c.try_run.push(&opened());
        until(|| async { writes_of(&c.try_run, "turn/start") }).await;
        c.try_run.fail_with(launch_lost());
        tick().await;
        c.steps(12, 10_000).await;
        assert_eq!(c.runs(), 4);
        let next = c.fed.runs.borrow()[3].clone();
        next.push(&opened());
        until(|| async { next.writes().iter().any(|w| w.contains("turn/start") && w.contains("Parsed 40 files")) }).await;
        next.push(&[turn_started()]);
        until(|| async { c.owed().await.is_empty() }).await;
        assert_eq!(c.to_person().await, 0);
        c.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's line try that ended before turn/started still leaves the line untaken on the next host, whose retry launches it fresh"
#[test]
fn a_codex_leads_line_try_that_ended_before_turn_started_still_leaves_the_line_untaken_on_the_next_host_whose_retry_launches_it_fresh() {
    drive(|| async {
        let h = two_hosts();
        let a = FakeClock::at_now();
        let mut rt = h.host(&a);
        let lead_ws = create_on(&rt.rt, lead_of("lead"), None).await.expect("a workspace").view.id;
        let lead = rt
            .rt
            .sessions()
            .start(&lead_ws, StartOptions { prompt: "orchestrate".into(), harness: Some("codex".into()), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { h.fed.runs.borrow().len() == 1 }).await;
        let first = h.fed.runs.borrow()[0].clone();
        first.push(&opened());
        until(|| async { writes_of(&first, "turn/start") }).await;
        first.push(&[turn_started(), agent_message(), completed()]);
        until(|| async { has_type(&rt.rt, &lead_ws, "session.done").await }).await;
        first.end(Some(0));
        lead.finished().await;
        let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
        let opts = StartOptions {
            prompt: "parse the files".into(),
            notify: Some(vec![lead.view().thread_id.expect("a thread")]),
            ..Default::default()
        };
        let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
        until(|| async { h.fed.run_of("parse the files").is_some() }).await;
        h.fed.run_of("parse the files").expect("the run").push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
        until(|| async { has_type(&rt.rt, &kid_ws, "session.done").await }).await;
        h.fed.run_of("parse the files").expect("the run").end(Some(0));
        kid.finished().await;
        until(|| async { h.fed.runs.borrow().len() == 3 }).await;
        let tried = h.fed.runs.borrow()[2].clone();
        tried.push(&opened());
        until(|| async { writes_of(&tried, "turn/start") }).await;
        tried.fail_with(launch_lost());
        tick().await;
        rt.rt.close().await.expect("a close");
        let b = FakeClock::new(a.now());
        rt = h.host(&b);
        rt.rt.sessions().list(Some(lead_ws.clone()), None).await.expect("a list");
        for _ in 0..12 {
            if h.fed.runs.borrow().len() >= 4 {
                break;
            }
            b.advance(10_000);
            tick().await;
        }
        assert_eq!(h.fed.runs.borrow().len(), 4);
        let next = h.fed.runs.borrow()[3].clone();
        next.push(&opened());
        until(|| async { next.writes().iter().any(|w| w.contains("turn/start") && w.contains("Parsed 40 files")) }).await;
        next.push(&[turn_started()]);
        until(|| async { h.owed().await == 0 }).await;
        rt.rt.close().await.expect("a close");
    });
}
