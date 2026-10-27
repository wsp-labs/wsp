// SPDX-License-Identifier: AGPL-3.0-only
//! The whole-runtime cases of packages/runtime/test/channels.test.ts at 65f6b89c7. The tokens the host mints for a machine whose
//! credentials it keeps: one per op whose git or gh reaches the git host, taken away at its reply, and one per shell a pane opens,
//! taken away when the shell ends. The daemon is a fake that keeps every frame and asks the host's own token table whether each
//! token it is handed is live while it answers. Each test names the TS test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use common::adapter::{event, result};
use common::projects::project_with;
use common::*;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};
use wsp_adapters::claude::landmines::{build_env as claude_env, BrokerKind, BrokeredSignIn, ClaudeEnvOptions};
use wsp_adapters::codex::command::KEY_FILE_ENV;
use wsp_adapters::port::{
    AdapterAttachOptions, AdapterError, ExecFailure, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture, TurnResult,
};
use wsp_core::clock::FakeClock;
use wsp_core::daemon_channel::{DaemonChannel, Frame};
use wsp_core::devices::DeviceDoor;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::{BrokeredKind, HarnessAdapterContext, HarnessAdapterFactory};
use wsp_engine::exec_detached::INLINE_EXEC_MS;
use wsp_engine::golden_seam::PROFILE_PATH_FILE;
use wsp_machines::machines::brokered::{BROKERED_BIN_DIR, TERMINAL_ENV, TERMINAL_PROFILE};
use wsp_store::store::Store;
use wsp_wire::types::daemon_contract::TOOLS_PATH;
use wsp_wire::types::env::{HOST_TOKEN_ENV, THREAD_ENV};
use wsp_wire::wire::daemon_version::PIPE_DAEMON_VERSION;
use wsp_words::exit::Kind;

const DAEMON_TOKEN: &str = "cafef00dcafef00dcafef00d";
const SESSION: &str = "44444444-4444-4444-8444-444444444444";

/// What each adapter was made with, a turn's launch among them: the environment and the Claude Code sign-in on a brokered machine.
#[derive(Default)]
struct Made {
    envs: RefCell<Vec<IndexMap<String, String>>>,
    sign_ins: RefCell<Vec<Option<(String, BrokeredKind)>>>,
}

/// A turn that answers at once, so a thread stands on the machine with no turn running.
fn answers(made: &Rc<Made>) -> HarnessAdapterFactory {
    let made = made.clone();
    Arc::new(move |ctx: HarnessAdapterContext| {
        made.envs.borrow_mut().push(ctx.env.clone());
        made.sign_ins.borrow_mut().push(ctx.brokered.as_ref().map(|b| (b.token.clone(), b.kind)));
        Ok(Arc::new(Answering) as Arc<dyn HarnessAdapter>)
    })
}

struct Answering;

impl HarnessAdapter for Answering {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let on_event = o.on_event.clone().expect("an event sink");
        let done = result(json!({ "status": "completed", "text": "done" }));
        on_event(event(json!({ "type": "session.start", "sessionId": SESSION })));
        on_event(event(json!({ "type": "turn.done", "sessionId": SESSION, "result": serde_json::to_value(&done).expect("a result") })));
        on_event(event(json!({ "type": "session.end", "sessionId": SESSION, "exitCode": 0, "sawResult": true })));
        let finished: LocalBoxFuture<'static, TurnResult> = async move { done }.boxed_local();
        Ok(Arc::new(Finished { finished: finished.shared() }))
    }
}

struct Finished {
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for Finished {
    fn local_id(&self) -> String {
        SESSION.into()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

/// The daemon of channels.test.ts `fakeDaemon`: it says hello as each channel opens and answers what a machine would, and asks the
/// host's own token table about every token it is handed.
struct Daemon {
    rec: Rc<RecDaemon>,
    live_at_op: Rc<RefCell<Vec<bool>>>,
    names_at_op: Rc<RefCell<Vec<String>>>,
    ptys: Rc<RefCell<IndexSet<String>>>,
    refuses: Rc<RefCell<HashSet<String>>>,
    gate: Rc<RefCell<Option<Rc<Gate>>>>,
}

type Door = Rc<RefCell<Option<Rc<dyn DeviceDoor>>>>;

fn fake_daemon(door: &Door, version: i64) -> Daemon {
    let ptys: Rc<RefCell<IndexSet<String>>> = Rc::default();
    let refuses: Rc<RefCell<HashSet<String>>> = Rc::default();
    let made = Rc::new(Cell::new(0));
    let (ptys_in, refuses_in) = (ptys.clone(), refuses.clone());
    let rec = RecDaemon::new(move |frame: &Frame| {
        let op = frame["op"].as_str().unwrap_or_default().to_string();
        if refuses_in.borrow().contains(&op) {
            return json!({ "id": 1, "ok": false, "error": format!("{op} was refused") });
        }
        let mut reply = json!({ "id": 1, "ok": true });
        let extra = match op.as_str() {
            "git.push" => {
                json!({ "branch": "pricing-page", "base": frame.get("base").and_then(Value::as_str).unwrap_or_default(), "remote": "origin", "ahead": 1, "uncommitted": 0, "stat": [] })
            }
            "git.pr" => json!({
                "pr": { "number": 7, "url": "https://github.com/acme/lab/pull/7", "state": "open", "host": "github.com", "draft": false, "base": "main", "branch": "pricing-page", "headOid": "abc1234", "headSubject": "Work", "mergeable": "unknown", "mergeState": "unknown", "review": "none", "checks": [], "additions": 1, "deletions": 0, "changedFiles": 1, "commits": 1 },
                "created": true,
            }),
            "pty.create" => {
                made.set(made.get() + 1);
                let id = format!("p{}", made.get());
                ptys_in.borrow_mut().insert(id.clone());
                json!({ "ptyId": id, "pid": 4000 + made.get() })
            }
            "pty.kill" => {
                ptys_in.borrow_mut().shift_remove(frame["ptyId"].as_str().unwrap_or_default());
                json!({})
            }
            "pty.list" => json!({ "ptys": ptys_in.borrow().iter().map(|id| json!({ "id": id })).collect::<Vec<_>>() }),
            _ => json!({}),
        };
        reply.as_object_mut().expect("an object").extend(extra.as_object().cloned().unwrap_or_default());
        reply
    });
    rec.hello.set(Some(version));
    let live_at_op: Rc<RefCell<Vec<bool>>> = Rc::default();
    let names_at_op: Rc<RefCell<Vec<String>>> = Rc::default();
    let gate: Rc<RefCell<Option<Rc<Gate>>>> = Rc::default();
    let (live, names, gate_in, door_in) = (live_at_op.clone(), names_at_op.clone(), gate.clone(), door.clone());
    rec.before_send(move |frame, _dial| {
        let (live, names, gate, door) = (live.clone(), names.clone(), gate_in.clone(), door_in.clone());
        let frame = frame.clone();
        async move {
            if frame["op"] == "pty.create" {
                let held = gate.borrow().clone();
                if let Some(held) = held {
                    held.wait().await;
                }
            }
            let token = match frame.get("token").and_then(Value::as_str) {
                Some(t) => Some(t.to_string()),
                None => frame.get("env").and_then(|e| e.get(HOST_TOKEN_ENV)).and_then(Value::as_str).map(str::to_string),
            };
            if let Some(token) = token {
                let held = door.borrow().clone().expect("the door");
                let found = held.r#match(&token).await.expect("a match");
                live.borrow_mut().push(found.is_some());
                if let Some(device) = found {
                    names.borrow_mut().push(device.view.name.clone());
                }
            }
        }
        .boxed_local()
    });
    Daemon { rec, live_at_op, names_at_op, ptys, refuses, gate }
}

impl Daemon {
    fn frames(&self) -> Vec<Value> {
        self.rec.frames.borrow().iter().map(|f| Value::Object(f.clone())).collect()
    }
    fn ops_of(&self, ops: &[&str]) -> Vec<Value> {
        self.frames().into_iter().filter(|f| ops.contains(&f["op"].as_str().unwrap_or_default())).collect()
    }
    fn creates(&self) -> Vec<Value> {
        self.ops_of(&["pty.create"])
    }
}

/// The branch a brokered create would otherwise read with gh on this computer, which no runtime here wires.
fn of_record(name: &str, branch: &str) -> CreateOn {
    CreateOn::golden(name).branch_of_record(4242, "acme", "lab", branch)
}

struct Fork {
    fx: Fx,
    daemon: Daemon,
    id: String,
    thread_id: String,
    folder: String,
    backend: Arc<StubBackend>,
    project: String,
    store: Rc<dyn Store>,
    made: Rc<Made>,
    door: Door,
    state: State,
}

#[derive(Default)]
struct ForkOpts {
    ready: Option<Rc<dyn Fn() -> bool>>,
    version: Option<i64>,
    no_thread: bool,
    codex: Option<&'static str>,
    harness: Option<&'static str>,
    clock: Option<FakeClock>,
}

fn broker_of(o: &ForkOpts) -> TestBroker {
    let ready = o.ready.clone().unwrap_or_else(|| Rc::new(|| true));
    let broker = TestBroker { ready, claims: None, status: None };
    match o.codex {
        Some(kind) => broker.codex(kind, "acct-acme"),
        None => broker,
    }
}

fn runtime_over(backend: &Arc<StubBackend>, state: &State, daemon: &Daemon, made: &Rc<Made>, o: &ForkOpts) -> Fx {
    let open = daemon.rec.open();
    let broker = broker_of(o).rc();
    let mut wiring =
        runtime(backend).store(&state.store).adapter("claude", answers(made)).adapter("codex", answers(made)).with(move |opts| {
            opts.daemon_token = Some(DAEMON_TOKEN.into());
            opts.daemon_channel = Some(open);
            opts.broker = Some(broker);
        });
    if let Some(clock) = &o.clock {
        wiring = wiring.clock(clock);
    }
    wiring.build()
}

/// A runtime whose broker reads `ready`, with one fork at its own provider and a thread on it.
async fn brokered_fork(o: ForkOpts) -> Fork {
    let backend = StubBackend::new(None);
    backend.set_exec_impl(token_guest());
    let backend = backend.with_daemon_roads();
    let door: Door = Rc::default();
    let daemon = fake_daemon(&door, o.version.unwrap_or(PIPE_DAEMON_VERSION));
    let state = sqlite_state();
    let made: Rc<Made> = Rc::default();
    let fx = runtime_over(&backend, &state, &daemon, &made, &o);
    *door.borrow_mut() = Some(fx.rt.devices());
    let project = project_with(&fx.rt, None, None, None, Some("main")).await.expect("a project").id;
    let ws = create_on(&fx.rt, CreateOn { project: Some(project.clone()), ..of_record("pricing page", "wsp/pricing-page") }, None)
        .await
        .expect("a workspace");
    let store = state.store.clone();
    if o.no_thread {
        return Fork {
            fx,
            daemon,
            id: ws.view.id,
            thread_id: String::new(),
            folder: String::new(),
            backend,
            project,
            store,
            made,
            door,
            state,
        };
    }
    let opts = StartOptions { prompt: "build the pricing page".into(), harness: o.harness.map(str::to_string), ..Default::default() };
    let turn = fx.rt.sessions().start(&ws.view.id, opts, None).await.expect("a start");
    turn.finished().await;
    // Where the thread works, as its turn's own reads of the checkout named it.
    let folder =
        daemon.frames().iter().find_map(|f| f.get("cwd").and_then(Value::as_str).map(str::to_string)).unwrap_or_else(|| "undefined".into());
    Fork {
        fx,
        daemon,
        id: ws.view.id,
        thread_id: turn.view().thread_id.expect("a thread"),
        folder,
        backend,
        project,
        store,
        made,
        door,
        state,
    }
}

fn tokens_of(frames: &[Value]) -> Vec<Value> {
    frames.iter().map(|f| f.get("token").cloned().unwrap_or(Value::Null)).collect()
}

impl Fork {
    async fn bring_back(&self, title: Option<&str>) -> Result<Value, wsp_words::exit::HostError> {
        self.fx.rt.workspaces().bring_back(&self.id, title.map(str::to_string), None, None).await.map(|b| j(&b))
    }
    fn devices(&self) -> Rc<dyn DeviceDoor> {
        self.fx.rt.devices()
    }
    async fn alive(&self, token: &str) -> bool {
        self.devices().r#match(token).await.expect("a match").is_some()
    }
    async fn scoped_devices(&self) -> Vec<Value> {
        self.devices().list().await.expect("devices").iter().map(j).filter(|d| d.get("scope").is_some()).collect()
    }
    /// The pty.create frames the daemon got after a pane on the workspace sent each frame in `sent`, on one channel.
    async fn opened(&self, sent: &[Value]) -> (Vec<Value>, Rc<dyn DaemonChannel>) {
        let before = self.daemon.frames().len();
        let channel = self.fx.rt.workspaces().daemon_channel(&self.id, Rc::new(|_| {}), None).await.expect("a channel");
        for frame in sent {
            channel.send(pty_create(frame.clone())).await.expect("a reply");
        }
        let creates = self.daemon.frames()[before..].iter().filter(|f| f["op"] == "pty.create").cloned().collect();
        (creates, channel)
    }
}

fn pty_create(extra: Value) -> Frame {
    let mut frame = json!({ "op": "pty.create", "cols": 80, "rows": 24 });
    frame.as_object_mut().expect("an object").extend(extra.as_object().cloned().unwrap_or_default());
    frame.as_object().cloned().expect("an object")
}

fn token_in(frame: &Value) -> String {
    frame["env"][HOST_TOKEN_ENV].as_str().expect("a token").to_string()
}

// ===== an op token on a machine whose credentials the host keeps =====

// "an op token on a machine whose credentials the host keeps" > "rides each git op as a fresh token, live while the daemon answers it and taken away at its reply"
#[test]
fn rides_each_git_op_as_a_fresh_token_live_while_the_daemon_answers_it_and_taken_away_at_its_reply() {
    drive(|| async {
        let f = brokered_fork(ForkOpts::default()).await;
        f.bring_back(Some("the pricing page")).await.expect("a bring back");
        let ops = f.daemon.ops_of(&["git.push", "git.pr"]);
        assert_eq!(ops.iter().map(|o| o["op"].clone()).collect::<Vec<_>>(), [json!("git.push"), json!("git.pr")]);
        let toks = tokens_of(&ops);
        let (push, pr) = (toks[0].as_str().expect("a token").to_string(), toks[1].as_str().expect("a token").to_string());
        assert_ne!(push, pr);
        assert_eq!(*f.daemon.live_at_op.borrow(), [true, true]);
        assert_eq!(*f.daemon.names_at_op.borrow(), ["op git.push", "op git.pr"]);
        assert!(!f.alive(&push).await);
        assert!(!f.alive(&pr).await);
        // Scoped to this workspace's thread, which is what the broker matches against the link the pipe came on.
        assert!(f
            .scoped_devices()
            .await
            .iter()
            .all(|d| d["scope"]["workspaceId"] == f.id.as_str() && d["scope"]["threadId"] == f.thread_id.as_str()));
        f.fx.rt.close().await.expect("a close");
    });
}

// "an op token on a machine whose credentials the host keeps" > "is taken away when the daemon refuses the op too"
#[test]
fn is_taken_away_when_the_daemon_refuses_the_op_too() {
    drive(|| async {
        let f = brokered_fork(ForkOpts::default()).await;
        f.daemon.refuses.borrow_mut().insert("git.push".into());
        let refused = f.bring_back(None).await.expect_err("a refusal");
        assert!(refused.message.contains("git.push was refused"), "{}", refused.message);
        let toks = tokens_of(&f.daemon.ops_of(&["git.push"]));
        assert_eq!(*f.daemon.live_at_op.borrow(), [true]);
        assert!(!f.alive(toks[0].as_str().expect("a token")).await);
        f.fx.rt.close().await.expect("a close");
    });
}

// "an op token on a machine whose credentials the host keeps" > "is not minted while the broker is not ready, nor for a daemon from before op tokens"
#[test]
fn is_not_minted_while_the_broker_is_not_ready_nor_for_a_daemon_from_before_op_tokens() {
    drive(|| async {
        let off = brokered_fork(ForkOpts { ready: Some(Rc::new(|| false)), ..Default::default() }).await;
        off.bring_back(None).await.expect("a bring back");
        assert_eq!(tokens_of(&off.daemon.ops_of(&["git.push", "git.pr"])), [Value::Null, Value::Null]);
        off.fx.rt.close().await.expect("a close");
        let old = brokered_fork(ForkOpts { version: Some(PIPE_DAEMON_VERSION - 1), ..Default::default() }).await;
        old.bring_back(None).await.expect("a bring back");
        assert_eq!(tokens_of(&old.daemon.ops_of(&["git.push", "git.pr"])), [Value::Null, Value::Null]);
        assert!(old.scoped_devices().await.is_empty());
        old.fx.rt.close().await.expect("a close");
    });
}

// ===== an op on a machine whose credentials the host keeps, with no thread on it yet =====

// "an op on a machine whose credentials the host keeps, with no thread on it yet" > "is refused once the broker is ready, before anything reaches the daemon"
#[test]
fn is_refused_once_the_broker_is_ready_before_anything_reaches_the_daemon() {
    drive(|| async {
        let f = brokered_fork(ForkOpts { no_thread: true, ..Default::default() }).await;
        let refused = f.bring_back(None).await.expect_err("a refusal");
        assert_eq!(refused.message, "pricing page has no thread yet, so wsp cannot reach GitHub for it. Start a thread on it first.");
        assert_eq!(refused.kind, Some(Kind::Usage));
        assert!(f.daemon.ops_of(&["git.push", "git.pr"]).is_empty());
        f.fx.rt.close().await.expect("a close");
    });
}

// "an op on a machine whose credentials the host keeps, with no thread on it yet" > "runs as it always has while the broker is not ready"
#[test]
fn runs_as_it_always_has_while_the_broker_is_not_ready() {
    drive(|| async {
        let f = brokered_fork(ForkOpts { no_thread: true, ready: Some(Rc::new(|| false)), ..Default::default() }).await;
        f.bring_back(None).await.expect("a bring back");
        assert_eq!(tokens_of(&f.daemon.ops_of(&["git.push", "git.pr"])), [Value::Null, Value::Null]);
        f.fx.rt.close().await.expect("a close");
    });
}

// ===== a terminal token on a machine whose credentials the host keeps =====

// "a terminal token on a machine whose credentials the host keeps" > "opens the first tab in the thread's folder with a token of its own, and every later tab with another"
#[test]
fn opens_the_first_tab_in_the_threads_folder_with_a_token_of_its_own_and_every_later_tab_with_another() {
    drive(|| async {
        let f = brokered_fork(ForkOpts::default()).await;
        let (frames, channel) = f.opened(&[json!({ "thread": f.thread_id }), json!({})]).await;
        channel.close();
        assert_eq!(frames.iter().map(|x| x["cwd"].clone()).collect::<Vec<_>>(), [json!(f.folder), json!(f.folder)]);
        assert!(frames[0].get("thread").is_none());
        let (first, second) = (token_in(&frames[0]), token_in(&frames[1]));
        assert_ne!(first, second);
        assert_eq!(frames[0]["env"][THREAD_ENV], f.thread_id.as_str());
        assert_eq!(*f.daemon.live_at_op.borrow(), [true, true]);
        let device = f.devices().r#match(&first).await.expect("a match").expect("a device");
        assert_eq!(
            j(&device.view.scope),
            json!({ "kind": "thread", "threadId": f.thread_id, "workspaceId": f.id, "rootThreadId": f.thread_id })
        );
        // A pane that goes leaves its shells running on the machine, and their tokens with them.
        assert!(f.alive(&second).await);
        f.fx.rt.close().await.expect("a close");
    });
}

fn git_and_gh(env: &Value, token: &str) -> Value {
    let kept: serde_json::Map<String, Value> = env
        .as_object()
        .expect("an environment")
        .iter()
        .filter(|(name, _)| name.starts_with("GIT_") || name.starts_with("GH_"))
        .map(|(name, value)| (name.clone(), json!(value.as_str().unwrap_or_default().replace(token, "<token>"))))
        .collect();
    Value::Object(kept)
}

// "a terminal token on a machine whose credentials the host keeps" > "carries the git and gh variables a turn's launch does, on the shell's own token"
#[test]
fn carries_the_git_and_gh_variables_a_turns_launch_does_on_the_shells_own_token() {
    drive(|| async {
        let f = brokered_fork(ForkOpts::default()).await;
        let turn = f.made.envs.borrow().iter().rev().find(|e| e.contains_key(HOST_TOKEN_ENV)).cloned().expect("a launch with a token");
        let (frames, channel) = f.opened(&[json!({ "thread": f.thread_id })]).await;
        channel.close();
        let shell = frames[0]["env"].clone();
        let turn_env = serde_json::to_value(&turn).expect("an env");
        let turn_token = turn[HOST_TOKEN_ENV].clone();
        let mine = git_and_gh(&turn_env, &turn_token);
        assert_matches(&mine, json!({ "GIT_TERMINAL_PROMPT": "0", "GH_TOKEN": "<token>" }));
        assert_eq!(git_and_gh(&shell, shell[HOST_TOKEN_ENV].as_str().expect("a token")), mine);
        assert_eq!(shell["GH_TOKEN"], token_in(&frames[0]).as_str());
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "carries the sign-in a Claude Code turn is handed, on the shell's own token"
#[test]
fn carries_the_sign_in_a_claude_code_turn_is_handed_on_the_shells_own_token() {
    drive(|| async {
        let f = brokered_fork(ForkOpts::default()).await;
        let turn = f.made.sign_ins.borrow().iter().rev().find_map(Clone::clone).expect("a sign-in");
        let (frames, channel) = f.opened(&[json!({ "thread": f.thread_id })]).await;
        channel.close();
        let shell = frames[0]["env"].clone();
        // What Claude Code's launch puts on a brokered turn and on no other, as its adapter builds the turn's environment.
        let kind = match turn.1 {
            BrokeredKind::Token => BrokerKind::Token,
            BrokeredKind::Key => BrokerKind::Key,
            BrokeredKind::Gateway => BrokerKind::Gateway,
        };
        let brokered =
            claude_env(&ClaudeEnvOptions { brokered: Some(BrokeredSignIn { token: turn.0.clone(), kind }), ..Default::default() });
        let plain = claude_env(&ClaudeEnvOptions::default());
        let names: Vec<String> = brokered.keys().filter(|n| brokered.get(*n) != plain.get(*n)).cloned().collect();
        assert!(names.contains(&"ANTHROPIC_BASE_URL".to_string()) && names.contains(&"CLAUDE_CODE_OAUTH_TOKEN".to_string()), "{names:?}");
        let sign_in = |env: &dyn Fn(&str) -> Option<String>, token: &str| -> Vec<Option<String>> {
            names.iter().map(|n| env(n).map(|v| v.replace(token, "<token>"))).collect()
        };
        let in_shell = sign_in(&|n| shell.get(n).and_then(Value::as_str).map(str::to_string), &token_in(&frames[0]));
        let in_turn = sign_in(&|n| brokered.get(n).cloned(), &turn.0);
        assert_eq!(in_shell, in_turn);
        f.fx.rt.close().await.expect("a close");
    });
}

/// A fork whose host holds a Codex login of `codex`'s kind, its thread on `harness`, whose machine answers the line writing codex's
/// program for a shell with `answer` once `written` opens, and the lines it was handed.
struct CodexFork {
    fork: Fork,
    lines: Rc<RefCell<Vec<String>>>,
    guest: common::stub_backend::ExecImpl,
}

async fn codex_fork(
    codex: &'static str,
    harness: Option<&'static str>,
    written: Option<Rc<Gate>>,
    answer: Option<(i64, &'static str)>,
    clock: Option<FakeClock>,
) -> CodexFork {
    let fork = brokered_fork(ForkOpts { codex: Some(codex), harness, clock, ..Default::default() }).await;
    let lines: Rc<RefCell<Vec<String>>> = Rc::default();
    let guest = fork.backend.exec_impl_fn();
    let (kept, plain, gate) = (lines.clone(), guest.clone(), written);
    fork.backend.set_exec_impl(common::stub_backend::exec_async(move |m, cmd| {
        if cmd.contains(BROKERED_BIN_DIR) {
            kept.borrow_mut().push(cmd);
            let gate = gate.clone();
            return async move {
                if let Some(gate) = gate {
                    gate.wait().await;
                }
                let (code, said) = answer.unwrap_or((0, ""));
                Ok(common::stub_backend::exec_exit(code, "", said))
            }
            .boxed_local();
        }
        plain(m, cmd, None)
    }));
    CodexFork { fork, lines, guest }
}

// "a terminal token on a machine whose credentials the host keeps" > "opens a Codex thread's shell marked for the profile that finds codex's program first"
#[test]
fn opens_a_codex_threads_shell_marked_for_the_profile_that_finds_codexs_program_first() {
    drive(|| async {
        let c = codex_fork("chatgpt", Some("codex"), None, None, None).await;
        let f = &c.fork;
        let (frames, channel) = f.opened(&[json!({ "thread": f.thread_id })]).await;
        channel.close();
        let shell = frames[0]["env"].clone();
        assert_eq!(c.lines.borrow().len(), 1);
        assert!(c.lines.borrow()[0].contains("openai_base_url"));
        assert!(c.lines.borrow()[0].contains(&format!("> {TERMINAL_PROFILE}.$$")));
        // profile.d runs in name order, and the image's own file sets the PATH outright.
        let mut order = [TERMINAL_PROFILE, PROFILE_PATH_FILE];
        order.sort();
        assert_eq!(order, [PROFILE_PATH_FILE, TERMINAL_PROFILE]);
        assert_eq!(shell[TERMINAL_ENV], "codex");
        assert_eq!(shell["PATH"], TOOLS_PATH);
        let url = shell["CODEX_REFRESH_TOKEN_URL_OVERRIDE"].as_str().expect("a url");
        assert!(
            url.starts_with("http://127.0.0.1:")
                && url.ends_with("/oauth/token")
                && url["http://127.0.0.1:".len()..url.len() - "/oauth/token".len()].chars().all(|c| c.is_ascii_digit()),
            "{url}"
        );
        assert!(shell.get(KEY_FILE_ENV).is_none());
        // The broker sends this thread's calls to OpenAI alone, so Claude Code there gets no placeholder to send.
        assert!(shell.get("ANTHROPIC_BASE_URL").is_none());
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "opens a machine's first Codex shell once codex's program is written there, and a later one without waiting on a write"
#[test]
fn opens_a_machines_first_codex_shell_once_codexs_program_is_written_there_and_a_later_one_without_waiting_on_a_write() {
    drive(|| async {
        let write = Gate::new();
        let c = codex_fork("chatgpt", Some("codex"), Some(write.clone()), None, None).await;
        let f = &c.fork;
        let channel = f.fx.rt.workspaces().daemon_channel(&f.id, Rc::new(|_| {}), None).await.expect("a channel");
        let creates = || f.daemon.creates().len();
        let first = {
            let (channel, frame) = (channel.clone(), pty_create(json!({ "thread": f.thread_id })));
            tokio::task::spawn_local(async move { channel.send(frame).await })
        };
        until(|| async { c.lines.borrow().len() == 1 }).await;
        assert_eq!(creates(), 0);
        write.open();
        first.await.expect("the first open").expect("a reply");
        assert_eq!(creates(), 1);
        let guest = c.guest.clone();
        let kept = c.lines.clone();
        f.backend.set_exec_impl(common::stub_backend::exec_async(move |m, cmd| {
            if cmd.contains(BROKERED_BIN_DIR) {
                kept.borrow_mut().push(cmd);
                return futures::future::pending().boxed_local();
            }
            guest(m, cmd, None)
        }));
        channel.send(pty_create(json!({ "thread": f.thread_id }))).await.expect("a reply");
        channel.close();
        assert_eq!(c.lines.borrow().len(), 2);
        assert_eq!(creates(), 2);
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "opens a machine's first Codex shell after an inline exec's span where the write there never answers"
#[test]
fn opens_a_machines_first_codex_shell_after_an_inline_execs_span_where_the_write_there_never_answers() {
    drive(|| async {
        let clock = FakeClock::at_now();
        let never = Gate::new();
        let c = codex_fork("chatgpt", Some("codex"), Some(never), None, Some(clock.clone())).await;
        let f = &c.fork;
        let channel = f.fx.rt.workspaces().daemon_channel(&f.id, Rc::new(|_| {}), None).await.expect("a channel");
        let creates = || f.daemon.creates().len();
        let first = {
            let (channel, frame) = (channel.clone(), pty_create(json!({ "thread": f.thread_id })));
            tokio::task::spawn_local(async move { channel.send(frame).await })
        };
        until(|| async { c.lines.borrow().len() == 1 }).await;
        clock.advance(INLINE_EXEC_MS as i64 - 1_000);
        sleep_ms(50).await;
        assert_eq!(creates(), 0);
        clock.advance(1_000);
        first.await.expect("the first open").expect("a reply");
        assert_eq!(creates(), 1);
        channel.close();
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "opens the shell where the machine refuses the write, and says in the host's log why its codex is not signed in"
#[test]
fn opens_the_shell_where_the_machine_refuses_the_write_and_says_in_the_hosts_log_why_its_codex_is_not_signed_in() {
    drive(|| async {
        let warned = wsp_core::console::capture();
        let said = "mkdir: cannot create directory '/usr/local/libexec/wsp-terminal': Read-only file system\nsecond line\n";
        let c = codex_fork("chatgpt", Some("codex"), None, Some((1, said)), None).await;
        let f = &c.fork;
        let (frames, channel) = f.opened(&[json!({ "thread": f.thread_id })]).await;
        channel.close();
        assert_eq!(c.lines.borrow().len(), 1);
        assert_matches(&frames[0]["env"], json!({ TERMINAL_ENV: "codex" }));
        let lines: Vec<String> = warned.lines().into_iter().filter(|l| l.contains("terminal programs")).collect();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("for codex on"), "{}", lines[0]);
        assert!(
            lines[0].ends_with("exit 1, mkdir: cannot create directory '/usr/local/libexec/wsp-terminal': Read-only file system"),
            "{}",
            lines[0]
        );
        f.fx.rt.close().await.expect("a close");
    });
}

/// The line writing a terminal's programs, its paths moved into a folder of the test's own.
fn runnable(line: &str, dir: &std::path::Path) -> String {
    let at = line.find("mkdir -p").expect("a mkdir");
    line[at..]
        .replace(BROKERED_BIN_DIR, &dir.join("bin").display().to_string())
        .replace(TERMINAL_PROFILE, &dir.join("profile.sh").display().to_string())
}

fn sh(script: &str, vars: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c").arg(script).env_clear().env("PATH", "/usr/bin:/bin");
    for (k, v) in vars {
        cmd.env(k, v);
    }
    cmd.output().expect("sh runs")
}

// "a terminal token on a machine whose credentials the host keeps" > "writes a profile that puts codex's program first in a login shell carrying the mark, and in no other"
#[test]
fn writes_a_profile_that_puts_codexs_program_first_in_a_login_shell_carrying_the_mark_and_in_no_other() {
    drive(|| async {
        let c = codex_fork("chatgpt", Some("codex"), None, None, None).await;
        let f = &c.fork;
        let (_, channel) = f.opened(&[json!({ "thread": f.thread_id })]).await;
        channel.close();
        let dir = tempfile::tempdir().expect("a folder");
        let line = runnable(&c.lines.borrow()[0], dir.path());
        assert!(sh(&line, &[]).status.success());
        // As the image's profile leaves it, the PATH set outright before this one runs.
        let found = |vars: &[(&str, &str)]| -> String {
            let script = format!("PATH=/usr/bin:/bin; . \"{}/profile.sh\"; command -v codex || true", dir.path().display());
            String::from_utf8_lossy(&sh(&script, vars).stdout).trim().to_string()
        };
        assert_eq!(found(&[(TERMINAL_ENV, "codex")]), format!("{}/bin/codex/codex", dir.path().display()));
        // A codex of the box's own on /usr/bin shows through where the profile does not take over, so "no other" is read against it.
        let own = String::from_utf8_lossy(&sh("PATH=/usr/bin:/bin; command -v codex || true", &[]).stdout).trim().to_string();
        assert_eq!(found(&[(TERMINAL_ENV, "claude")]), own);
        assert_eq!(found(&[]), own);
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "hands codex its placeholder key on the shell's own token, where the host holds a Codex key"
#[test]
fn hands_codex_its_placeholder_key_on_the_shells_own_token_where_the_host_holds_a_codex_key() {
    drive(|| async {
        let c = codex_fork("key", Some("codex"), None, None, None).await;
        let f = &c.fork;
        let (frames, channel) = f.opened(&[json!({ "thread": f.thread_id })]).await;
        channel.close();
        assert_eq!(frames[0]["env"][KEY_FILE_ENV], format!("sk-wsp-{}", token_in(&frames[0])).as_str());
        f.fx.rt.close().await.expect("a close");
    });
}

/// `typed` in a login shell marked with `own`, on a machine where terminals' opens ran `lines` in turn, their paths moved into a
/// folder of their own: what it printed on each stream and how it exited.
fn typed_after(lines: &[String], typed: &str, own: &str) -> (String, String, Option<i32>) {
    let dir = tempfile::tempdir().expect("a folder");
    for line in lines {
        assert!(sh(&runnable(line, dir.path()), &[]).status.success());
    }
    let script = format!("PATH=/usr/bin:/bin; . \"{}/profile.sh\"; {typed}", dir.path().display());
    let ran = sh(&script, &[(TERMINAL_ENV, own)]);
    (
        String::from_utf8_lossy(&ran.stdout).replace(&dir.path().display().to_string(), "<dir>"),
        String::from_utf8_lossy(&ran.stderr).into_owned(),
        ran.status.code(),
    )
}

// "a terminal token on a machine whose credentials the host keeps" > "answers codex typed on a Claude Code thread with why it cannot sign in there, though the host holds a Codex login"
#[test]
fn answers_codex_typed_on_a_claude_code_thread_with_why_it_cannot_sign_in_there_though_the_host_holds_a_codex_login() {
    drive(|| async {
        let c = codex_fork("chatgpt", None, None, None, None).await;
        let f = &c.fork;
        let (frames, channel) = f.opened(&[json!({ "thread": f.thread_id })]).await;
        channel.close();
        let env = &frames[0]["env"];
        assert!(env["ANTHROPIC_BASE_URL"].is_string());
        assert_eq!(env[TERMINAL_ENV], "claude");
        assert!(env.get("CODEX_REFRESH_TOKEN_URL_OVERRIDE").is_none());
        assert_eq!(c.lines.borrow().len(), 1);
        assert!(!c.lines.borrow()[0].contains("openai_base_url"));
        let got = typed_after(&c.lines.borrow(), "codex exec \"say ok\"", "claude");
        assert_eq!(
            got,
            (
                String::new(),
                "wsp: Codex cannot sign in on this machine. This terminal's thread runs Claude Code, and your computer signs in only that agent here. Start a thread on Codex to use it.\n".to_string(),
                Some(1)
            )
        );
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "answers claude typed on a Codex thread with why it cannot sign in there"
#[test]
fn answers_claude_typed_on_a_codex_thread_with_why_it_cannot_sign_in_there() {
    drive(|| async {
        let c = codex_fork("chatgpt", Some("codex"), None, None, None).await;
        let f = &c.fork;
        let (frames, channel) = f.opened(&[json!({ "thread": f.thread_id })]).await;
        channel.close();
        assert!(frames[0]["env"].get("ANTHROPIC_BASE_URL").is_none());
        let got = typed_after(&c.lines.borrow(), "claude -p \"say ok\"", "codex");
        assert_eq!(
            got,
            (
                String::new(),
                "wsp: Claude Code cannot sign in on this machine. This terminal's thread runs Codex, and your computer signs in only that agent here. Start a thread on Claude Code to use it.\n".to_string(),
                Some(1)
            )
        );
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "signs in each thread's own agent in its own terminal where threads of both agents share the machine"
#[test]
fn signs_in_each_threads_own_agent_in_its_own_terminal_where_threads_of_both_agents_share_the_machine() {
    drive(|| async {
        let c = codex_fork("chatgpt", None, None, None, None).await;
        let f = &c.fork;
        let claude_thread = f.thread_id.clone();
        let opts = StartOptions { prompt: "review the pricing page".into(), harness: Some("codex".into()), ..Default::default() };
        let codex_turn = f.fx.rt.sessions().start(&f.id, opts, None).await.expect("a start");
        codex_turn.finished().await;
        let (codex_frames, codex_channel) = f.opened(&[json!({ "thread": codex_turn.view().thread_id })]).await;
        let (claude_frames, claude_channel) = f.opened(&[json!({ "thread": claude_thread })]).await;
        codex_channel.close();
        claude_channel.close();
        assert_eq!(codex_frames[0]["env"][TERMINAL_ENV], "codex");
        assert!(codex_frames[0]["env"]["CODEX_REFRESH_TOKEN_URL_OVERRIDE"].is_string());
        assert_eq!(claude_frames[0]["env"][TERMINAL_ENV], "claude");
        assert!(claude_frames[0]["env"]["ANTHROPIC_BASE_URL"].is_string());
        // The Claude Code thread's terminal opened last, and codex in the Codex thread's terminal is still its program.
        assert_eq!(typed_after(&c.lines.borrow(), "command -v codex", "codex").0, "<dir>/bin/codex/codex\n");
        let codex_in_claude = typed_after(&c.lines.borrow(), "codex exec \"say ok\"", "claude").1;
        assert!(
            codex_in_claude.starts_with("wsp: Codex cannot sign in on this machine. This terminal's thread runs Claude Code"),
            "{codex_in_claude}"
        );
        let claude_in_codex = typed_after(&c.lines.borrow(), "claude -p \"say ok\"", "codex").1;
        assert!(
            claude_in_codex.starts_with("wsp: Claude Code cannot sign in on this machine. This terminal's thread runs Codex"),
            "{claude_in_codex}"
        );
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "dies with its shell: at the exit the daemon pushes, at a kill, and at a list that no longer holds it"
#[test]
fn dies_with_its_shell_at_the_exit_the_daemon_pushes_at_a_kill_and_at_a_list_that_no_longer_holds_it() {
    drive(|| async {
        let f = brokered_fork(ForkOpts::default()).await;
        let (frames, channel) = f.opened(&[json!({}), json!({}), json!({})]).await;
        let toks: Vec<String> = frames.iter().map(token_in).collect();
        let (exits, killed, vanished) = (&toks[0], &toks[1], &toks[2]);
        f.daemon.rec.push(json!({ "type": "pty.exit", "ptyId": "p1", "code": 0 }));
        until(|| async { !f.alive(exits).await }).await;
        assert!(f.alive(killed).await);
        let kill = json!({ "op": "pty.kill", "ptyId": "p2" }).as_object().cloned().expect("an object");
        channel.send(kill).await.expect("a reply");
        assert!(!f.alive(killed).await);
        channel.close();
        // Gone while no pane listened: the next pane's list is what says so.
        f.daemon.ptys.borrow_mut().shift_remove("p3");
        let next = f.fx.rt.workspaces().daemon_channel(&f.id, Rc::new(|_| {}), None).await.expect("a channel");
        next.send(json!({ "op": "pty.list" }).as_object().cloned().expect("an object")).await.expect("a reply");
        next.close();
        assert!(!f.alive(vanished).await);
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "outlives a host restart while its shell stands, with no turn running, and dies with the shell after"
#[test]
fn outlives_a_host_restart_while_its_shell_stands_with_no_turn_running_and_dies_with_the_shell_after() {
    drive(|| async {
        let f = brokered_fork(ForkOpts::default()).await;
        let (frames, channel) = f.opened(&[json!({})]).await;
        channel.close();
        let token = token_in(&frames[0]);
        f.fx.rt.close().await.expect("a close");
        let rt = runtime_over(&f.backend, &f.state, &f.daemon, &f.made, &ForkOpts::default());
        *f.door.borrow_mut() = Some(rt.rt.devices());
        // The boot has run, and with it the sweep of every token whose thread has no turn running.
        rt.rt.workspaces().list(None).await.expect("a list");
        assert!(rt.rt.devices().r#match(&token).await.expect("a match").is_some());
        let pane = rt.rt.workspaces().daemon_channel(&f.id, Rc::new(|_| {}), None).await.expect("a channel");
        pane.send(json!({ "op": "pty.list" }).as_object().cloned().expect("an object")).await.expect("a reply");
        pane.close();
        assert!(rt.rt.devices().r#match(&token).await.expect("a match").is_some());
        f.daemon.ptys.borrow_mut().clear();
        let next = rt.rt.workspaces().daemon_channel(&f.id, Rc::new(|_| {}), None).await.expect("a channel");
        next.send(json!({ "op": "pty.list" }).as_object().cloned().expect("an object")).await.expect("a reply");
        next.close();
        assert!(rt.rt.devices().r#match(&token).await.expect("a match").is_none());
        rt.rt.close().await.expect("a close");
    });
}

/// A turn still running when the host stops, whose run the machine no longer holds when the next host asks.
struct Gone;

impl HarnessAdapter for Gone {
    fn steers(&self) -> bool {
        false
    }
    fn attaches(&self) -> bool {
        true
    }
    fn attach(
        &self,
        _options: AdapterAttachOptions,
    ) -> Option<LocalBoxFuture<'static, Result<Option<Arc<dyn HarnessSession>>, ExecFailure>>> {
        Some(async { Ok(None) }.boxed_local())
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let on_event = o.on_event.clone().expect("an event sink");
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            on_event(event(json!({ "type": "session.start", "sessionId": "55555555-5555-4555-8555-555555555555" })));
        });
        Ok(Arc::new(Hung))
    }
}

struct Hung;

impl HarnessSession for Hung {
    fn local_id(&self) -> String {
        "55555555-5555-4555-8555-555555555555".into()
    }
    fn run(&self) -> Option<String> {
        Some("/tmp/wsp-run/000000000001".into())
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        let never: LocalBoxFuture<'static, TurnResult> = futures::future::pending().boxed_local();
        never.shared()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

// "a terminal token on a machine whose credentials the host keeps" > "outlives a host restart that finds its thread's turn gone from the machine"
#[test]
fn outlives_a_host_restart_that_finds_its_threads_turn_gone_from_the_machine() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let backend = backend.with_daemon_roads();
        let door: Door = Rc::default();
        let daemon = fake_daemon(&door, PIPE_DAEMON_VERSION);
        let state = sqlite_state();
        let build = || {
            let open = daemon.rec.open();
            let gone: HarnessAdapterFactory = Arc::new(|_ctx| Ok(Arc::new(Gone) as Arc<dyn HarnessAdapter>));
            runtime(&backend)
                .store(&state.store)
                .adapter("claude", gone)
                .with(move |o| {
                    o.daemon_token = Some(DAEMON_TOKEN.into());
                    o.daemon_channel = Some(open);
                    o.broker = Some(TestBroker::ready().rc());
                })
                .build()
        };
        let rt = build();
        *door.borrow_mut() = Some(rt.rt.devices());
        let project = project_with(&rt.rt, None, None, None, Some("main")).await.expect("a project").id;
        let ws = create_on(&rt.rt, CreateOn { project: Some(project), ..of_record("pricing page", "wsp/pricing-page") }, None)
            .await
            .expect("a workspace")
            .view
            .id;
        let turn = rt
            .rt
            .sessions()
            .start(&ws, StartOptions { prompt: "build the pricing page".into(), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { rt.rt.sessions().history(&ws, None).await.expect("a history").iter().any(|e| j(e)["type"] == "session.start") })
            .await;
        let thread = turn.view().thread_id.expect("a thread");
        let before = daemon.rec.frames.borrow().len();
        let channel = rt.rt.workspaces().daemon_channel(&ws, Rc::new(|_| {}), None).await.expect("a channel");
        channel.send(pty_create(json!({ "thread": thread }))).await.expect("a reply");
        channel.close();
        let frames: Vec<Value> =
            daemon.rec.frames.borrow()[before..].iter().filter(|f| f["op"] == "pty.create").map(|f| Value::Object(f.clone())).collect();
        rt.rt.close().await.expect("a close");
        let rt = build();
        *door.borrow_mut() = Some(rt.rt.devices());
        rt.rt.workspaces().list(None).await.expect("a list");
        until(|| async {
            rt.rt.sessions().list(Some(ws.clone()), None).await.expect("a list").first().is_some_and(|r| j(r)["status"] == "failed")
        })
        .await;
        until(|| async { rt.rt.devices().list().await.expect("devices").iter().map(j).filter(|d| d.get("scope").is_some()).count() == 1 })
            .await;
        assert!(rt.rt.devices().r#match(&token_in(&frames[0])).await.expect("a match").is_some());
        rt.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "is not minted while the broker is not ready"
#[test]
fn is_not_minted_while_the_broker_is_not_ready() {
    drive(|| async {
        let f = brokered_fork(ForkOpts { ready: Some(Rc::new(|| false)), ..Default::default() }).await;
        let (frames, channel) = f.opened(&[json!({})]).await;
        channel.close();
        assert_eq!(frames.iter().map(|x| x["cwd"].clone()).collect::<Vec<_>>(), [json!(f.folder)]);
        assert!(frames[0].get("env").is_none());
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "is taken away at once when the shell's open answers after its workspace was deleted, and no record is kept"
#[test]
fn is_taken_away_at_once_when_the_shells_open_answers_after_its_workspace_was_deleted_and_no_record_is_kept() {
    drive(|| async {
        let f = brokered_fork(ForkOpts::default()).await;
        let answer = Gate::new();
        *f.daemon.gate.borrow_mut() = Some(answer.clone());
        let channel = f.fx.rt.workspaces().daemon_channel(&f.id, Rc::new(|_| {}), None).await.expect("a channel");
        let opening = {
            let (channel, frame) = (channel.clone(), pty_create(json!({})));
            tokio::task::spawn_local(async move { channel.send(frame).await })
        };
        until(|| async { !f.daemon.creates().is_empty() }).await;
        let token = token_in(&f.daemon.creates()[0]);
        f.fx.rt.workspaces().delete(&f.id, None).await.expect("a delete");
        answer.open();
        opening.await.expect("the open").expect("a reply");
        channel.close();
        assert!(!f.alive(&token).await);
        assert!(!f.store.keys("pty-tokens").await.expect("keys").contains(&f.id));
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "is scoped to this workspace's own thread when the pane names a thread of another workspace"
#[test]
fn is_scoped_to_this_workspaces_own_thread_when_the_pane_names_a_thread_of_another_workspace() {
    drive(|| async {
        let f = brokered_fork(ForkOpts::default()).await;
        let other =
            create_on(&f.fx.rt, CreateOn { project: Some(f.project.clone()), ..of_record("checkout page", "wsp/checkout-page") }, None)
                .await
                .expect("a workspace");
        let turn =
            f.fx.rt
                .sessions()
                .start(&other.view.id, StartOptions { prompt: "build the checkout page".into(), ..Default::default() }, None)
                .await
                .expect("a start");
        turn.finished().await;
        let theirs = turn.view().thread_id.expect("a thread");
        assert_ne!(theirs, f.thread_id);
        let (frames, channel) = f.opened(&[json!({ "thread": theirs })]).await;
        channel.close();
        let device = f.devices().r#match(&token_in(&frames[0])).await.expect("a match").expect("a device");
        assert_eq!(
            j(&device.view.scope),
            json!({ "kind": "thread", "threadId": f.thread_id, "workspaceId": f.id, "rootThreadId": f.thread_id })
        );
        assert_eq!(frames[0]["env"][THREAD_ENV], f.thread_id.as_str());
        f.fx.rt.close().await.expect("a close");
    });
}

// "a terminal token on a machine whose credentials the host keeps" > "goes with its workspace"
#[test]
fn goes_with_its_workspace() {
    drive(|| async {
        let f = brokered_fork(ForkOpts::default()).await;
        let (frames, channel) = f.opened(&[json!({})]).await;
        channel.close();
        let token = token_in(&frames[0]);
        f.fx.rt.workspaces().delete(&f.id, None).await.expect("a delete");
        until(|| async { !f.alive(&token).await }).await;
        f.fx.rt.close().await.expect("a close");
    });
}
