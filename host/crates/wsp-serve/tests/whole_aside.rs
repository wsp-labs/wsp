// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/aside.test.ts at 65f6b89c7, case for case. A side question asked beside a thread, through the
//! runtime: answered by the adapter of the thread's harness off the thread's latest row, refused in one sentence where there is
//! no session to copy or the harness copies none, kept nowhere, and shut to a thread's own token and to a paired computer. The
//! harness is a fake that answers from memory, so the runtime's own reading is what is under test. Each test names the TS test
//! above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::adapter::{event, result};
use common::local::{local_wiring, ws_request};
use common::*;
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AsideAnswer, AsideQuestion, ExecFailure, ExecOptions, ExecStream, ExecStreamFactory, HarnessAdapter, HarnessSession,
    HarnessStartOptions, SharedFuture, TurnResult,
};
use wsp_core::devices::MintMore;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};
use wsp_core::types::wiring::{AgentsReach, LocalWiring};
use wsp_engine::copier::{fake_copier, Copier};
use wsp_serve::serve::{serve_runtime, ServeOptions};
use wsp_wire::types::env::{HOST_TOKEN_ENV, HOST_URL_ENV, NO_SLATE_MCP_ARG, SCOPED_MCP_ARG};
use wsp_wire::types::wsp_tools::MCP_SERVER_NAME;
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::exit::Kind;
use wsp_words::words::thread::{aside_unsupported_line, ASIDE_EMPTY_LINE, ASIDE_NO_SESSION_LINE, BLANK_ASIDE_LINE};

const SESSION: &str = "33333333-3333-4333-8333-333333333333";
const ANSWER: &str = "You are in /root/b and last asked for the second thing.";

/// What a side question was asked, as the fields these tests read.
#[derive(Clone, Debug, PartialEq)]
struct Asked {
    session: String,
    question: String,
    cwd: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    has_on_text: bool,
    mcp_servers: Option<Value>,
}

type Hook = Rc<dyn Fn(&AsideQuestion, &wsp_adapters::port::Env) -> LocalBoxFuture<'static, Result<AsideAnswer, String>>>;

#[derive(Clone)]
struct Options {
    announce: bool,
    aside: bool,
    compacts: Option<String>,
    terminal_resume: Option<String>,
    mcp_servers: bool,
    aside_servers: bool,
    hook: Option<Hook>,
    /// One session of its own per turn, in order, at a folder and model of its own.
    own_sessions: Option<Vec<String>>,
}

impl Default for Options {
    fn default() -> Options {
        Options { announce: true, aside: true, compacts: None, terminal_resume: None, mcp_servers: false, aside_servers: false, hook: None, own_sessions: None }
    }
}

struct Fake {
    o: Options,
    asked: Rc<RefCell<Vec<Asked>>>,
    turns: Rc<Cell<usize>>,
    env: wsp_adapters::port::Env,
}

/// A harness whose every turn announces the same session, each at a folder and model of its own, and replies at once.
/// `announce: false` dies before it announces any; `aside: false` has no side question.
fn answering(asked: &Rc<RefCell<Vec<Asked>>>, o: Options) -> HarnessAdapterFactory {
    let turns: Rc<Cell<usize>> = Rc::default();
    let asked = asked.clone();
    Arc::new(move |ctx: HarnessAdapterContext| {
        Ok(Arc::new(Fake { o: o.clone(), asked: asked.clone(), turns: turns.clone(), env: ctx.env.clone() }) as Arc<dyn HarnessAdapter>)
    })
}

impl HarnessAdapter for Fake {
    fn steers(&self) -> bool {
        false
    }
    fn asides(&self) -> bool {
        self.o.aside
    }
    fn aside_servers(&self) -> bool {
        self.o.aside_servers
    }
    fn mcp_servers(&self) -> bool {
        self.o.mcp_servers
    }
    fn compacts(&self) -> Option<String> {
        self.o.compacts.clone()
    }
    fn terminal_resume(&self) -> Option<String> {
        self.o.terminal_resume.clone()
    }
    fn aside(&self, q: AsideQuestion) -> Option<LocalBoxFuture<'static, Result<AsideAnswer, String>>> {
        if !self.o.aside {
            return None;
        }
        self.asked.borrow_mut().push(Asked {
            session: q.session.clone(),
            question: q.question.clone(),
            cwd: q.cwd.clone(),
            model: q.model.clone(),
            effort: q.effort.clone(),
            has_on_text: q.on_text.is_some(),
            mcp_servers: q.mcp_servers.as_ref().map(|m| serde_json::to_value(m).expect("servers")),
        });
        if let Some(hook) = &self.o.hook {
            return Some(hook(&q, &self.env));
        }
        if let Some(on_text) = &q.on_text {
            for piece in ["You are in ", "/root/b"] {
                on_text(piece.to_string());
            }
        }
        Some(async { Ok(AsideAnswer { text: ANSWER.into(), usage: None }) }.boxed_local())
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.turns.set(self.turns.get() + 1);
        let n = self.turns.get();
        let (session, at) = match &self.o.own_sessions {
            Some(sessions) => (sessions[n - 1].clone(), json!({ "cwd": "/root/w", "model": "claude-opus-5" })),
            None => (
                SESSION.to_string(),
                if n == 1 { json!({ "cwd": "/root/a", "model": "claude-opus-5" }) } else { json!({ "cwd": "/root/b", "model": "claude-sonnet-5" }) },
            ),
        };
        let announce = self.o.announce;
        let done = result(if announce {
            json!({ "status": "completed", "text": "done" })
        } else {
            json!({ "status": "failed", "error": "claude exited with code 1 before it answered" })
        });
        let on_event = o.on_event.clone().expect("an event sink");
        let (tx, rx) = futures::channel::oneshot::channel::<TurnResult>();
        let (id, finish) = (session.clone(), done.clone());
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            if announce {
                let mut start = json!({ "type": "session.start", "sessionId": id });
                start.as_object_mut().expect("an object").extend(at.as_object().expect("an object").clone());
                on_event(event(start));
            }
            on_event(event(json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&finish).expect("a result") })));
            on_event(event(json!({ "type": "session.end", "sessionId": id, "exitCode": if announce { 0 } else { 1 }, "sawResult": announce })));
            let _ = tx.send(finish);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(Done { local_id: session, finished: finished.shared() }))
    }
}

struct Done {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for Done {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

struct World {
    fx: Fx,
    asked: Rc<RefCell<Vec<Asked>>>,
    _root: tempfile::TempDir,
    _state: State,
}

fn local_in(root: &std::path::Path) -> LocalWiring {
    let copier: Rc<dyn Copier> = Rc::new(fake_copier(None, None));
    let mut local = local_wiring(&std::fs::canonicalize(root).expect("a real path"), copier);
    local.daemon_road = None;
    local
}

fn world(adapters: Vec<(&str, HarnessAdapterFactory)>, asked: Rc<RefCell<Vec<Asked>>>, agents: Option<AgentsReach>) -> World {
    let root = tempfile::tempdir().expect("a folder");
    let state = sqlite_state();
    let local = local_in(root.path());
    let backend = StubBackend::new(None);
    let mut wiring = runtime(&backend).store(&state.store).with(move |o| {
        o.local = Some(local);
        o.agents = agents;
    });
    for (name, factory) in adapters {
        wiring = wiring.adapter(name, factory);
    }
    World { fx: wiring.build(), asked, _root: root, _state: state }
}

fn plain(o: Options) -> World {
    let asked: Rc<RefCell<Vec<Asked>>> = Rc::default();
    world(vec![("claude", answering(&asked, o))], asked, None)
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

/// A thread of two finished turns on the one local workspace; answers the first turn's row id and the workspace.
async fn two_turns(w: &World) -> (String, String) {
    let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
    let first = w.fx.rt.sessions().start(&ws, start("the first thing"), None).await.expect("a start");
    first.finished().await;
    let thread = first.view().thread_id.expect("a thread");
    w.fx.rt.sessions().start(&ws, StartOptions { thread: Some(thread), ..start("the second thing") }, None).await.expect("a start").finished().await;
    (first.id(), ws)
}

async fn aside(w: &World, session: &str, question: &str) -> Result<Value, wsp_words::exit::HostError> {
    w.fx.rt.sessions().aside(session, question, None, None).await.map(|a| j(&a))
}

async fn claude_catalog(w: &World, ws: &str) -> Value {
    let list = w.fx.rt.harnesses().list(Some(ws.to_string()), None, None).await.expect("a list");
    list.iter().map(j).find(|c| c["harness"] == "claude").expect("claude's catalog")
}

fn wsp_mcp() -> McpServerSpec {
    McpServerSpec { command: "node".into(), args: vec!["/opt/wsp/dist/bin.js".into(), "mcp".into()], no_slate: None }
}

fn reach() -> AgentsReach {
    AgentsReach { here_url: Some("http://127.0.0.1:4801".into()), wsp_mcp: Some(wsp_mcp()) }
}

fn scoped_server(extra: &[&str]) -> Value {
    let mut args = vec!["/opt/wsp/dist/bin.js".to_string(), "mcp".to_string(), SCOPED_MCP_ARG.to_string()];
    args.extend(extra.iter().map(|a| a.to_string()));
    json!({ MCP_SERVER_NAME: { "command": "node", "args": args } })
}

async fn device_count(w: &World) -> usize {
    w.fx.rt.devices().list().await.expect("devices").len()
}

// "a side question beside a thread" > "is answered by the thread's harness off its latest row, and nothing of it is kept"
#[test]
fn is_answered_by_the_threads_harness_off_its_latest_row_and_nothing_of_it_is_kept() {
    drive(|| async {
        let w = plain(Options::default());
        let (first, ws) = two_turns(&w).await;
        let history: Vec<Value> = w.fx.rt.sessions().history(&ws, None).await.expect("a history").iter().map(j).collect();
        let rows: Vec<Value> = w.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list").iter().map(j).collect();
        assert_eq!(aside(&w, &first, "what did I last ask?").await.expect("an answer"), json!({ "text": ANSWER }));
        // The row named is the thread's first; the question goes at the latest one's folder, model and effort.
        let asked = w.asked.borrow().clone();
        assert_eq!(
            asked,
            [Asked {
                session: SESSION.into(),
                question: "what did I last ask?".into(),
                cwd: Some("/root/b".into()),
                model: Some("claude-sonnet-5".into()),
                effort: Some("high".into()),
                has_on_text: false,
                mcp_servers: None,
            }]
        );
        let after: Vec<Value> = w.fx.rt.sessions().history(&ws, None).await.expect("a history").iter().map(j).collect();
        assert_eq!(after, history);
        let rows_after: Vec<Value> = w.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list").iter().map(j).collect();
        assert_eq!(rows_after, rows);
        w.fx.rt.close().await.expect("a close");
    });
}

// "a side question beside a thread" > "passes each piece of the answer by under the question's id while it is written, and keeps none of them for a replay"
#[test]
fn passes_each_piece_of_the_answer_by_under_the_questions_id_while_it_is_written_and_keeps_none_of_them_for_a_replay() {
    drive(|| async {
        let w = plain(Options::default());
        let (first, ws) = two_turns(&w).await;
        let thread_id = j(&w.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list")[0])["threadId"].clone();
        let head = w.fx.rt.events().since(Some(0), None).head;
        let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
        let kept = seen.clone();
        let off = wsp_core::types::api::Runtime::events(&*w.fx.rt).on(
            "aside.text",
            Rc::new(move |e| {
                kept.borrow_mut().push(serde_json::to_value(e).expect("an event"));
                Ok(())
            }),
        );
        let said = w.fx.rt.sessions().aside(&first, "what did I last ask?", None, Some("ask-1".into())).await.expect("an answer");
        assert_eq!(j(&said), json!({ "text": ANSWER }));
        off();
        let want = |text: &str| json!({ "type": "aside.text", "workspaceId": ws, "threadId": thread_id, "askId": "ask-1", "text": text, "seq": head });
        assert_eq!(*seen.borrow(), [want("You are in "), want("/root/b")]);
        assert!(w.fx.rt.events().since(Some(head), None).events.is_empty());
        // Asked with no id, the pieces go nowhere.
        seen.borrow_mut().clear();
        let again = seen.clone();
        let _later = wsp_core::types::api::Runtime::events(&*w.fx.rt).on(
            "aside.text",
            Rc::new(move |e| {
                again.borrow_mut().push(serde_json::to_value(e).expect("an event"));
                Ok(())
            }),
        );
        aside(&w, &first, "again?").await.expect("an answer");
        assert!(seen.borrow().is_empty());
        w.fx.rt.close().await.expect("a close");
    });
}

// "a side question beside a thread" > "hands the side question the wsp server and the host pair a turn of the thread gets, and takes the token back once it is answered"
#[test]
fn hands_the_side_question_the_wsp_server_and_the_host_pair_a_turn_of_the_thread_gets_and_takes_the_token_back_once_it_is_answered() {
    drive(|| async {
        let asked: Rc<RefCell<Vec<Asked>>> = Rc::default();
        let seen: Rc<RefCell<Vec<(Value, wsp_adapters::port::Env, usize)>>> = Rc::default();
        let devices: Rc<RefCell<Option<Rc<dyn wsp_core::devices::DeviceDoor>>>> = Rc::default();
        let (seen_in, door) = (seen.clone(), devices.clone());
        let hook: Hook = Rc::new(move |q, env| {
            let (seen, door, env) = (seen_in.clone(), door.clone(), env.clone());
            let servers = serde_json::to_value(&q.mcp_servers).expect("servers");
            async move {
                let held = door.borrow().clone().expect("the door");
                let count = held.list().await.expect("devices").len();
                seen.borrow_mut().push((servers, env, count));
                Ok(AsideAnswer { text: ANSWER.into(), usage: None })
            }
            .boxed_local()
        });
        let o = Options { mcp_servers: true, aside_servers: true, hook: Some(hook), ..Default::default() };
        let w = world(vec![("claude", answering(&asked, o))], asked, Some(reach()));
        *devices.borrow_mut() = Some(w.fx.rt.devices());
        let (first, _ws) = two_turns(&w).await;
        // The turns' own tokens are taken back as their processes end, so what stands now is the side question's alone.
        until(|| async { device_count(&w).await == 0 }).await;
        assert_eq!(aside(&w, &first, "what did I last ask?").await.expect("an answer"), json!({ "text": ANSWER }));
        // The copy resumes a session that announced the wsp server's instructions; a fork without that server is told
        // it disconnected, and the answer opens on it.
        let (servers, env, count) = seen.borrow()[0].clone();
        assert_eq!(servers, scoped_server(&[]));
        // The scoped server refuses to start without the pair, which would read as the same disconnection.
        assert_eq!(env.get(HOST_URL_ENV).map(String::as_str), Some("http://127.0.0.1:4801"));
        assert!(env.get(HOST_TOKEN_ENV).is_some_and(|t| t.chars().any(|c| !c.is_whitespace())));
        assert_eq!(count, 1);
        assert_eq!(device_count(&w).await, 0);
        w.fx.rt.close().await.expect("a close");
    });
}

struct Silent {
    launched: Rc<RefCell<Vec<(IndexMap<String, String>, Rc<dyn Fn() -> LocalBoxFuture<'static, usize>>)>>>,
    door: Rc<dyn wsp_core::devices::DeviceDoor>,
}

/// An exec whose process prints nothing and exits 1: the side question's app server answers nothing.
struct Quiet;

impl ExecStream for Quiet {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        Box::pin(futures::stream::empty())
    }
    fn teardown(&self) {}
    fn kill(&self) {}
    fn write(&self, _input: String) -> LocalBoxFuture<'static, Result<wsp_adapters::port::WriteOutcome, ExecFailure>> {
        async { Ok(wsp_adapters::port::WriteOutcome::Written) }.boxed_local()
    }
    fn close_input(&self) {}
    fn exited(&self) -> SharedFuture<Option<i32>> {
        async { Some(1) }.boxed_local().shared()
    }
}

impl ExecStreamFactory for Silent {
    fn launch(&self, _command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        let door = self.door.clone();
        self.launched.borrow_mut().push((
            options.env.clone(),
            Rc::new(move || {
                let door = door.clone();
                async move { door.list().await.map(|d| d.len()).unwrap_or(usize::MAX) }.boxed_local()
            }),
        ));
        Arc::new(Quiet)
    }
}

// "a side question beside a thread" > "hands a Codex side question no token: its fork loads no server to dial with one, and it can run read-only commands"
#[test]
fn hands_a_codex_side_question_no_token_its_fork_loads_no_server_to_dial_with_one_and_it_can_run_read_only_commands() {
    drive(|| async {
        let asked: Rc<RefCell<Vec<Asked>>> = Rc::default();
        let launched: Rc<RefCell<Vec<(IndexMap<String, String>, Rc<dyn Fn() -> LocalBoxFuture<'static, usize>>)>>> = Rc::default();
        let door: Rc<RefCell<Option<Rc<dyn wsp_core::devices::DeviceDoor>>>> = Rc::default();
        // The real Codex adapter, its declarations and its side question as they ship; only the turn is a fake, and the side
        // question's app server answers nothing, so the ask rejects once its environment is read.
        let (launches, held_door, fake_asked) = (launched.clone(), door.clone(), asked.clone());
        let codex: HarnessAdapterFactory = Arc::new(move |ctx: HarnessAdapterContext| {
            let silent = Arc::new(Silent { launched: launches.clone(), door: held_door.borrow().clone().expect("the door") });
            let real = wsp_core::adapters::harness_adapters()["codex"].clone();
            let turn = answering(&fake_asked, Options::default())(dup(&ctx))?;
            let ctx = HarnessAdapterContext { exec_stream: silent, ..ctx };
            let adapter = real(ctx)?;
            Ok(Arc::new(WithTurn { adapter, turn }) as Arc<dyn HarnessAdapter>)
        });
        let root = tempfile::tempdir().expect("a folder");
        // The adapter's own probe and title question run codex on this computer: a stub that answers nothing.
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).expect("a folder");
        common::local::write_stub(&bin.join("codex"), "#!/bin/sh\nexit 0\n");
        let mut local = local_in(root.path());
        let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()));
        local.env = Rc::new(move || IndexMap::from([("PATH".to_string(), path.clone())]));
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let fx = runtime(&backend)
            .store(&state.store)
            .adapter("codex", codex)
            .with(move |o| {
                o.local = Some(local);
                o.agents = Some(reach());
            })
            .build();
        *door.borrow_mut() = Some(fx.rt.devices());
        let ws = create_on(&fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        let turn = fx
            .rt
            .sessions()
            .start(&ws, StartOptions { harness: Some("codex".into()), ..start("the first thing") }, None)
            .await
            .expect("a start");
        turn.finished().await;
        until(|| async { fx.rt.devices().list().await.expect("devices").is_empty() }).await;
        let refused = fx.rt.sessions().aside(&turn.id(), "what did I last ask?", None, None).await;
        assert!(refused.is_err());
        assert_eq!(launched.borrow().len(), 1);
        let (env, devices) = launched.borrow()[0].clone();
        assert!(!env.contains_key(HOST_TOKEN_ENV));
        assert!(!env.contains_key(HOST_URL_ENV));
        assert_eq!(devices().await, 0);
        fx.rt.close().await.expect("a close");
        drop(root);
    });
}

/// A second reading of the same context, for the fake turn beside the real adapter.
fn dup(ctx: &HarnessAdapterContext) -> HarnessAdapterContext {
    HarnessAdapterContext {
        machine: ctx.machine.clone(),
        workspace_id: ctx.workspace_id.clone(),
        exec_stream: ctx.exec_stream.clone(),
        home: ctx.home.clone(),
        env: ctx.env.clone(),
        project_key: ctx.project_key.clone(),
        sign_in_refusal: ctx.sign_in_refusal.clone(),
        vault: ctx.vault.clone(),
        brokered: ctx.brokered.clone(),
        codex_broker: ctx.codex_broker.clone(),
        login_stands: ctx.login_stands.clone(),
        launch: ctx.launch.clone(),
    }
}

/// `{ ...codex(ctx), start: answering(ctx).start }`: the real adapter in everything but the turn.
struct WithTurn {
    adapter: Arc<dyn HarnessAdapter>,
    turn: Arc<dyn HarnessAdapter>,
}

impl HarnessAdapter for WithTurn {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.turn.start(o)
    }
    fn steers(&self) -> bool {
        self.adapter.steers()
    }
    fn asides(&self) -> bool {
        self.adapter.asides()
    }
    fn aside(&self, q: AsideQuestion) -> Option<LocalBoxFuture<'static, Result<AsideAnswer, String>>> {
        self.adapter.aside(q)
    }
    fn aside_servers(&self) -> bool {
        self.adapter.aside_servers()
    }
    fn mcp_servers(&self) -> bool {
        self.adapter.mcp_servers()
    }
    fn denies_servers(&self) -> bool {
        self.adapter.denies_servers()
    }
    fn attachments(&self) -> Option<wsp_adapters::port::AttachmentRoad> {
        self.adapter.attachments()
    }
    fn compacts(&self) -> Option<String> {
        self.adapter.compacts()
    }
    fn terminal_resume(&self) -> Option<String> {
        self.adapter.terminal_resume()
    }
    fn env(&self) -> Option<wsp_adapters::port::Env> {
        self.adapter.env()
    }
}

// "a side question beside a thread" > "tells the composer the harness takes one, and not for a harness without it"
#[test]
fn tells_the_composer_the_harness_takes_one_and_not_for_a_harness_without_it() {
    drive(|| async {
        let w = plain(Options::default());
        let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        assert_eq!(claude_catalog(&w, &ws).await["asides"], true);
        let bare = plain(Options { aside: false, ..Default::default() });
        let other = create_on(&bare.fx.rt, CreateOn::named("mac-2").on("here"), None).await.expect("a workspace").view.id;
        assert_eq!(claude_catalog(&bare, &other).await["asides"], false);
        w.fx.rt.close().await.expect("a close");
        bare.fx.rt.close().await.expect("a close");
    });
}

// "a side question beside a thread" > "tells the composer the message that compacts the harness's thread, the adapter's own, and nothing where it has none"
#[test]
fn tells_the_composer_the_message_that_compacts_the_harnesss_thread_the_adapters_own_and_nothing_where_it_has_none() {
    drive(|| async {
        let w = plain(Options { compacts: Some("/compact".into()), ..Default::default() });
        let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        assert_eq!(claude_catalog(&w, &ws).await["compacts"], "/compact");
        let bare = plain(Options::default());
        let other = create_on(&bare.fx.rt, CreateOn::named("mac-2").on("here"), None).await.expect("a workspace").view.id;
        assert!(claude_catalog(&bare, &other).await.get("compacts").is_none());
        w.fx.rt.close().await.expect("a close");
        bare.fx.rt.close().await.expect("a close");
    });
}

// "a side question beside a thread" > "tells the app the command that opens the harness's session in the person's terminal, the adapter's own, and nothing where it has none"
#[test]
fn tells_the_app_the_command_that_opens_the_harnesss_session_in_the_persons_terminal_the_adapters_own_and_nothing_where_it_has_none() {
    drive(|| async {
        let w = plain(Options { terminal_resume: Some("claude --resume".into()), ..Default::default() });
        let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        assert_eq!(claude_catalog(&w, &ws).await["terminalResume"], "claude --resume");
        let bare = plain(Options::default());
        let other = create_on(&bare.fx.rt, CreateOn::named("mac-2").on("here"), None).await.expect("a workspace").view.id;
        assert!(claude_catalog(&bare, &other).await.get("terminalResume").is_none());
        w.fx.rt.close().await.expect("a close");
        bare.fx.rt.close().await.expect("a close");
    });
}

// "a side question beside a thread" > "refuses a thread whose harness never announced a session, a harness with no side question, and no words"
#[test]
fn refuses_a_thread_whose_harness_never_announced_a_session_a_harness_with_no_side_question_and_no_words() {
    drive(|| async {
        let w = plain(Options { announce: false, ..Default::default() });
        let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        let dead = w.fx.rt.sessions().start(&ws, start("hi"), None).await.expect("a start");
        dead.finished().await;
        let said = aside(&w, &dead.id(), "anyone there?").await.err().expect("a refusal");
        assert!(said.message.contains(ASIDE_NO_SESSION_LINE), "{}", said.message);
        w.fx.rt.close().await.expect("a close");

        let w = plain(Options { aside: false, ..Default::default() });
        let (first, _) = two_turns(&w).await;
        let said = aside(&w, &first, "anyone there?").await.err().expect("a refusal");
        assert!(said.message.contains(&aside_unsupported_line("claude")), "{}", said.message);
        w.fx.rt.close().await.expect("a close");

        let w = plain(Options::default());
        let (again, _) = two_turns(&w).await;
        let blank = aside(&w, &again, "   ").await.err().expect("a refusal");
        assert!(blank.message.contains(BLANK_ASIDE_LINE), "{}", blank.message);
        let none = aside(&w, "sess_nothing", "hi").await.err().expect("a refusal");
        assert_eq!(none.kind, Some(Kind::NotFound));
        assert!(w.asked.borrow().is_empty());
        w.fx.rt.close().await.expect("a close");
    });
}

// "a side question beside a thread" > "is refused under a thread's own token and to a paired computer, and answered for the person's own window"
#[test]
fn is_refused_under_a_threads_own_token_and_to_a_paired_computer_and_answered_for_the_persons_own_window() {
    drive(|| async {
        let w = plain(Options::default());
        let (first, _) = two_turns(&w).await;
        let served = serve_runtime(
            w.fx.rt.clone(),
            ServeOptions { port: Some(0), auth_token: "t".into(), devices: Some(w.fx.rt.devices()), ..Default::default() },
        )
        .await
        .expect("the runtime serves");
        let now = common::now_ms();
        let scope = serde_json::from_value(json!({ "kind": "thread", "threadId": "t1", "workspaceId": "w1", "rootThreadId": "t1" })).expect("a scope");
        let thread = w.fx.rt.devices().mint("thread t1", scope, now, MintMore::default()).await.expect("a token").device_token;
        let ask = json!({ "sessionId": first, "question": "what did I ask?" });
        let reply = ws_request(served.port(), &thread, "sessions.aside", ask.clone()).await;
        assert_matches(&reply, json!({ "ok": false, "error": wsp_words::wire::runtime::thread_op_refusal("sessions.aside", "t1", None) }));
        let paired = w.fx.rt.devices().admit("phone", now).await.expect("a device").device_token;
        let reply = ws_request(served.port(), &paired, "sessions.aside", ask.clone()).await;
        assert_matches(&reply, json!({ "ok": false, "error": wsp_words::words::computer::device_held_refusal("sessions.aside") }));
        assert!(w.asked.borrow().is_empty());

        let reply = ws_request(served.port(), "t", "sessions.aside", ask).await;
        assert_matches(&reply, json!({ "ok": true, "text": ANSWER }));
        assert_eq!(w.asked.borrow().len(), 1);
        served.close().await.expect("a close");
        w.fx.rt.close().await.expect("a close");
    });
}

// "a side question beside a thread" > "answers a question on thread A and then one on thread B of the same workspace each from its own thread's session"
#[test]
fn answers_a_question_on_thread_a_and_then_one_on_thread_b_of_the_same_workspace_each_from_its_own_threads_session() {
    drive(|| async {
        let sessions = vec!["44444444-4444-4444-8444-444444444444".to_string(), "55555555-5555-4555-8555-555555555555".to_string()];
        let w = plain(Options { own_sessions: Some(sessions.clone()), ..Default::default() });
        let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        let a = w.fx.rt.sessions().start(&ws, start("thread A"), None).await.expect("a start");
        a.finished().await;
        let b = w.fx.rt.sessions().start(&ws, start("thread B"), None).await.expect("a start");
        b.finished().await;
        assert_ne!(a.view().thread_id, b.view().thread_id);
        aside(&w, &a.id(), "what is this thread about?").await.expect("an answer");
        aside(&w, &b.id(), "what is this thread about?").await.expect("an answer");
        let got: Vec<String> = w.asked.borrow().iter().map(|q| q.session.clone()).collect();
        assert_eq!(got, sessions);
        w.fx.rt.close().await.expect("a close");
    });
}

// "a side question beside a thread" > "hands the copy the effort its thread's turns run at, so the request it sends is theirs and reads their cache"
#[test]
fn hands_the_copy_the_effort_its_threads_turns_run_at_so_the_request_it_sends_is_theirs_and_reads_their_cache() {
    drive(|| async {
        let w = plain(Options::default());
        let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        let turn = w.fx.rt.sessions().start(&ws, StartOptions { effort: Some("high".into()), ..start("the first thing") }, None).await.expect("a start");
        turn.finished().await;
        aside(&w, &turn.id(), "what did I ask?").await.expect("an answer");
        let asked = w.asked.borrow()[0].clone();
        assert_eq!((asked.session.as_str(), asked.effort.as_deref()), (SESSION, Some("high")));
        w.fx.rt.close().await.expect("a close");
    });
}

// "a side question beside a thread" > "hands a thread another thread started the server its turns get, with no slate, so its copy's tools are theirs"
#[test]
fn hands_a_thread_another_thread_started_the_server_its_turns_get_with_no_slate_so_its_copys_tools_are_theirs() {
    drive(|| async {
        let sessions = vec!["44444444-4444-4444-8444-444444444444".to_string(), "55555555-5555-4555-8555-555555555555".to_string()];
        let asked: Rc<RefCell<Vec<Asked>>> = Rc::default();
        let o = Options { own_sessions: Some(sessions), mcp_servers: true, aside_servers: true, ..Default::default() };
        let w = world(vec![("claude", answering(&asked, o))], asked, Some(reach()));
        let ws = create_on(&w.fx.rt, CreateOn::named("mac").on("here").agents(3, 2), None).await.expect("a workspace").view.id;
        let lead = w.fx.rt.sessions().start(&ws, start("lead"), None).await.expect("a start");
        lead.finished().await;
        let lead_thread = lead.view().thread_id.expect("a thread");
        let by = serde_json::from_value(json!({ "kind": "thread", "threadId": lead_thread, "workspaceId": ws, "rootThreadId": lead_thread })).expect("a scope");
        let caller = Caller::Scoped { origin: WorkspaceOrigin::Relayed, by };
        let child = w.fx.rt.sessions().start(&ws, start("builder"), Some(caller)).await.expect("a start");
        child.finished().await;
        aside(&w, &lead.id(), "how is it going?").await.expect("an answer");
        aside(&w, &child.id(), "how is it going?").await.expect("an answer");
        let asked = w.asked.borrow().clone();
        assert_eq!(asked[0].mcp_servers.as_ref().map(|m| m[MCP_SERVER_NAME].clone()), Some(scoped_server(&[])[MCP_SERVER_NAME].clone()));
        let mut quiet = scoped_server(&[NO_SLATE_MCP_ARG])[MCP_SERVER_NAME].clone();
        quiet["noSlate"] = json!(true);
        assert_eq!(asked[1].mcp_servers.as_ref().map(|m| m[MCP_SERVER_NAME].clone()), Some(quiet));
        w.fx.rt.close().await.expect("a close");
    });
}

// "a side question beside a thread" > "says in one line why where the harness answered with no words, rather than standing an empty answer under the question"
#[test]
fn says_in_one_line_why_where_the_harness_answered_with_no_words_rather_than_standing_an_empty_answer_under_the_question() {
    drive(|| async {
        let hook: Hook = Rc::new(|_, _| async { Ok(AsideAnswer { text: " \n".into(), usage: None }) }.boxed_local());
        let w = plain(Options { hook: Some(hook), ..Default::default() });
        let (first, _) = two_turns(&w).await;
        let said = aside(&w, &first, "what did I last ask?").await.err().expect("a refusal");
        assert!(said.message.contains(ASIDE_EMPTY_LINE), "{}", said.message);
        w.fx.rt.close().await.expect("a close");
    });
}
