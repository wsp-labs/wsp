// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-reach.test.ts at 65f6b89c7, first part: "runtime daemon reach", case for case (the
//! second part, `whole_runtime_reach_2.rs`, holds the machine the provider answers it cannot reach, the port reach and the
//! port probe).
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::gone::until_n;
use common::reach::{daemon_port, preview_example, route_through, token_written};
use common::*;
use futures::{SinkExt as _, StreamExt as _};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use wsp_adapters::port::{AdapterError, HarnessAdapter, HarnessSession, HarnessStartOptions};
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::daemon_roots::write_daemon_roots_script;
use wsp_core::daemon_token::{daemon_token_for, rotate_daemon_token_script, DAEMON_TOKEN_NONE, DAEMON_TOKEN_SET};
use wsp_core::status::{StatusWatchOptions, POLL_INTERVAL_MS};
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};
use wsp_core::types::wiring::{Broker, GoldenRecipe, Local, RuntimeOptions, DAEMON_REVIVE_AGAIN_MS};
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_serve::AssembledRuntime;
use wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH;
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_words::exit::HostError;
use wsp_words::words::thread::{DAEMON_RESTARTING, DAEMON_RESTART_FAILED, DAEMON_UPDATE_FAILED, DAEMON_UPDATING};

const TOKEN: &str = "deadbeefdeadbeefdeadbeef";

/// A host whose broker keeps the credentials of the stub's machines, whose turns reach their model through the daemon.
struct Ready;

#[async_trait::async_trait(?Send)]
impl Broker for Ready {
    fn ready(&self) -> bool {
        true
    }
    async fn codex_claims(&self) -> Result<Option<wsp_adapters::pending::CodexClaims>, HostError> {
        Ok(None)
    }
}

type Deploy = Rc<dyn Fn(Arc<dyn Machine>) -> Fut2>;
type Fut2 = futures::future::LocalBoxFuture<'static, Result<Option<String>, HostError>>;

fn recipe(deploy: Deploy) -> GoldenRecipe {
    GoldenRecipe {
        setup: "true".into(),
        smoke: "true".into(),
        deploy_daemon: Some(Rc::new(move |machine, _| -> Local<'static, Option<String>> { deploy(machine) })),
        ..Default::default()
    }
}

/// A deploy that notes the machine it ran on and then does `then`.
fn noting(deployed: &Rc<RefCell<Vec<String>>>, then: impl Fn() + 'static) -> GoldenRecipe {
    let noted = deployed.clone();
    recipe(Rc::new(move |machine| {
        noted.borrow_mut().push(machine.id().to_string());
        then();
        Box::pin(async { Ok(None) })
    }))
}

fn status_options(poll_ms: i64, reconcile_ms: i64) -> StatusWatchOptions {
    StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(poll_ms), reconcile_min_ms: Some(reconcile_ms), ..Default::default() }
}

async fn get(rt: &Rc<AssembledRuntime>, id: &str) -> Value {
    j(&rt.workspaces().get(id, None, None).await.expect("the workspace"))
}

async fn reach_of(rt: &Rc<AssembledRuntime>, id: &str) -> Value {
    j(&rt.workspaces().daemon_reach(id, None).await.expect("a reach"))
}

fn exec_with_token(m: &Arc<StubMachine>) -> Vec<String> {
    m.exec_containing(DAEMON_TOKEN_PATH)
}

/// `helloOf(port)`: the version the daemon on that port announces after the auth reply.
async fn hello_of(port: u16) -> i64 {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/")).await.expect("a socket");
    ws.send(Message::text(json!({ "id": 1, "op": "auth", "token": "x" }).to_string())).await.expect("auth sent");
    loop {
        let Some(Ok(Message::Text(raw))) = ws.next().await else { panic!("the daemon closed before it said hello") };
        let frame: Value = serde_json::from_str(raw.as_str()).expect("a frame");
        if frame["type"] == "daemon.hello" {
            return frame["version"].as_i64().expect("a version");
        }
    }
}

/// `until(...).catch(() => {})`: waits for the condition up to two seconds and goes on either way.
async fn until_soft(mut cond: impl FnMut() -> bool) {
    for _ in 0..200 {
        if cond() {
            return;
        }
        sleep_ms(10).await;
    }
}

// "runtime daemon reach" > "hands back the preview route plus the token it minted, written to the guest once and never in the url"
#[test]
fn hands_back_the_preview_route_plus_the_token_it_minted_written_to_the_guest_once_and_never_in_the_url() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let fx = runtime(&backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let minted = Rc::new(Cell::new(0u32));
        preview_example(&backend.machine(0), "edge", Some(minted.clone()));

        let reach = reach_of(&fx.rt, &ws.view.id).await;
        assert_eq!(reach["url"], "https://m1-7070.preview.example/?pt_token=edge");
        assert!(reach["expiresAt"].is_number());
        let token = reach["daemonToken"].as_str().expect("a token").to_string();
        assert!(!token.is_empty() && token.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')), "{token}");
        assert_eq!(reach.as_object().expect("an object").len(), 3);
        reach_of(&fx.rt, &ws.view.id).await;
        assert_eq!(minted.get(), 1);
        assert_eq!(exec_with_token(&backend.machine(0)), [rotate_daemon_token_script(&token, None).expect("a script")]);
        fx.rt.close().await.expect("a close");
    });
}

// "mints a token per machine, hex so the write needs no quoting, unless a seed is given"
#[test]
fn mints_a_token_per_machine_hex_so_the_write_needs_no_quoting_unless_a_seed_is_given() {
    drive(|| async {
        let written: Rc<RefCell<Vec<String>>> = Rc::default();
        let mut tokens: Vec<String> = vec![];
        for _ in 0..2 {
            let backend = StubBackend::new(None);
            let noted = written.clone();
            backend.set_exec_impl(exec_fn(move |_, cmd| {
                if let Some(token) = token_written(cmd) {
                    noted.borrow_mut().push(token);
                }
                Ok(exec_ok(DAEMON_TOKEN_SET))
            }));
            let fx = runtime(&backend).build();
            let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
            preview_example(&backend.machine(0), "e", None);
            tokens.push(reach_of(&fx.rt, &ws.view.id).await["daemonToken"].as_str().expect("a token").to_string());
        }
        for token in &tokens {
            assert!(token.len() == 48 && token.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')), "{token}");
        }
        assert_ne!(tokens[0], tokens[1]);
        assert_eq!(*written.borrow(), tokens);
        let refused = wsp_serve::create_runtime(RuntimeOptions {
            backend: Some(StubBackend::new(None)),
            store: Some(memory_state()),
            daemon_token: Some("it's not hex".into()),
            ..Default::default()
        });
        let refused = refused.err().expect("a refusal");
        assert!(refused.message.contains("hex"), "{}", refused.message);
    });
}

// "gives two machines of one runtime two tokens, each its own on its own reach view, and writes a machine no second one"
#[test]
fn gives_two_machines_of_one_runtime_two_tokens_each_its_own_on_its_own_reach_view_and_writes_a_machine_no_second_one() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let written: Rc<RefCell<Vec<String>>> = Rc::default();
        let noted = written.clone();
        backend.set_exec_impl(exec_fn(move |_, cmd| {
            if let Some(token) = token_written(cmd) {
                noted.borrow_mut().push(token);
            }
            Ok(exec_ok(DAEMON_TOKEN_SET))
        }));
        let fx = runtime(&backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let one = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let two = create_on(&fx.rt, CreateOn::golden("b"), None).await.expect("a workspace");
        for m in backend.machines() {
            preview_example(&m, "e", None);
        }

        let first = reach_of(&fx.rt, &one.view.id).await["daemonToken"].as_str().expect("a token").to_string();
        let second = reach_of(&fx.rt, &two.view.id).await["daemonToken"].as_str().expect("a token").to_string();
        // The token read off one box opens no other machine this host rotated onto.
        assert_ne!(first, second);
        assert_eq!(*written.borrow(), [first.clone(), second.clone()]);
        // A machine keeps the token it was written; a second ask writes nothing and hands back the same one.
        assert_eq!(reach_of(&fx.rt, &one.view.id).await["daemonToken"], first.as_str());
        assert_eq!(*written.borrow(), [first.clone(), second.clone()]);
        assert_eq!(exec_with_token(&backend.machine(0)), [rotate_daemon_token_script(&first, None).expect("a script")]);
        assert_eq!(exec_with_token(&backend.machine(1)), [rotate_daemon_token_script(&second, None).expect("a script")]);
        fx.rt.close().await.expect("a close");
    });
}

// "derives each machine's token from the seed a caller pinned, so two runtimes on one seed write one machine one token"
#[test]
fn derives_each_machines_token_from_the_seed_a_caller_pinned_so_two_runtimes_on_one_seed_write_one_machine_one_token() {
    drive(|| async {
        async fn token_of() -> String {
            let backend = StubBackend::new(None);
            backend.set_exec_impl(token_guest());
            let fx = runtime(&backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
            let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
            preview_example(&backend.machine(0), "e", None);
            let token = reach_of(&fx.rt, &ws.view.id).await["daemonToken"].as_str().expect("a token").to_string();
            fx.rt.close().await.expect("a close");
            token
        }
        assert_eq!(token_of().await, daemon_token_for(TOKEN, "m1").expect("a token"));
        assert_eq!(token_of().await, daemon_token_for(TOKEN, "m1").expect("a token"));
    });
}

// "omits the daemon token when the guest has none and refuses backends without preview urls"
#[test]
fn omits_the_daemon_token_when_the_guest_has_none_and_refuses_backends_without_preview_urls() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(exec_fn(|_, cmd| {
            Ok(if cmd.contains(DAEMON_TOKEN_PATH) { exec_ok(&format!("{DAEMON_TOKEN_NONE}\n")) } else { exec_ok("") })
        }));
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let refused = fx.rt.workspaces().daemon_reach(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("without preview URLs"), "{}", refused.message);

        backend.machine(0).set_preview_url("https://m1-7070.preview.example/?pt_token=e");
        let reach = reach_of(&fx.rt, &ws.view.id).await;
        assert!(reach.get("daemonToken").is_none(), "{reach}");
        fx.rt.close().await.expect("a close");
    });
}

// "updateDaemon runs the recipe's deploy on the running machine, then writes this runtime's token again so the next reach opens the new daemon"
#[test]
fn update_daemon_runs_the_recipes_deploy_on_the_running_machine_then_writes_this_runtimes_token_again_so_the_next_reach_opens_the_new_daemon() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let deployed: Rc<RefCell<Vec<String>>> = Rc::default();
        let noted = deployed.clone();
        let golden = recipe(Rc::new(move |machine| {
            noted.borrow_mut().push(machine.id().to_string());
            Box::pin(async move {
                machine.exec("DEPLOY_DAEMON", ExecOptions::default()).await.map_err(|e| HostError::new(e.to_string()))?;
                Ok(Some("daemon on node v22".to_string()))
            })
        }));
        let fx = runtime(&backend)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.daemon_hello_timeout_ms = Some(50);
            })
            .build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let m = backend.machine(0);
        preview_example(&m, "e", None);
        let token = reach_of(&fx.rt, &ws.view.id).await["daemonToken"].as_str().expect("a token").to_string();
        let before = m.exec_log().len();

        fx.rt.workspaces().update_daemon(&ws.view.id, None).await.expect("an update");
        assert_eq!(*deployed.borrow(), ["m1"]);
        // The deploy started the daemon on its own token; the rotation after it is what makes this machine's token open it.
        assert_eq!(m.exec_log()[before..], ["DEPLOY_DAEMON".to_string(), rotate_daemon_token_script(&token, None).expect("a script")]);
        assert_eq!(reach_of(&fx.rt, &ws.view.id).await["daemonToken"], token.as_str());
        assert_eq!(m.exec_log().len(), before + 2);

        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let refused = fx.rt.workspaces().update_daemon(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("wake a before updating its daemon"), "{}", refused.message);
        assert_eq!(*deployed.borrow(), ["m1"]);
        fx.rt.close().await.expect("a close");
    });
}

// "replaces a daemon older than this wsp by itself: the runtime connects to a machine it adopts, reads the hello and deploys, with nothing asking it to"
#[test]
fn replaces_a_daemon_older_than_this_wsp_by_itself_the_runtime_connects_to_a_machine_it_adopts_reads_the_hello_and_deploys_with_nothing_asking_it_to() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let state = sqlite_state();
        let daemon = Rc::new(helloing_daemon(1, false, None).await);
        let before = runtime(&backend).store(&state.store).build();
        let ws = create_on(&before.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let m = backend.machine(0);
        m.set_preview_url(&format!("ws://127.0.0.1:{}", daemon.port));

        let deployed: Rc<RefCell<Vec<String>>> = Rc::default();
        let held = Gate::new();
        let (noted, waiting, daemon_of_deploy) = (deployed.clone(), held.clone(), daemon.clone());
        let golden = recipe(Rc::new(move |machine| {
            noted.borrow_mut().push(machine.id().to_string());
            let (waiting, daemon) = (waiting.clone(), daemon_of_deploy.clone());
            Box::pin(async move {
                waiting.wait().await;
                daemon.announce(DAEMON_VERSION);
                Ok(None)
            })
        }));
        // The host starting again over the same store, on a machine that is already running: the one case a person hits
        // after an upgrade, and the one nothing else in the runtime reaches.
        let fx = runtime(&backend)
            .store(&state.store)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.daemon_hello_timeout_ms = Some(2_000);
            })
            .build();
        let rt = fx.rt.clone();
        let events = fx.events();
        rt.workspaces().list(None).await.expect("a list");
        until_n(|| deployed.borrow().len() == 1).await;
        assert_eq!(*deployed.borrow(), ["m1"]);

        // The row says what is being done while it is being done, and the status carrying it still carries the nap
        // countdown: a client replaces the whole status, so a line that dropped it would blank the row.
        until(|| {
            let (rt, id) = (rt.clone(), ws.view.id.clone());
            async move { get(&rt, &id).await["daemonNote"] == DAEMON_UPDATING }
        })
        .await;
        let updating: Vec<Value> = events.statuses().into_iter().filter(|s| s["daemonNote"] == DAEMON_UPDATING).collect();
        assert_eq!(updating.len(), 1);
        assert!(updating[0]["idleAt"].as_f64().expect("an idle deadline") > now_ms() as f64);

        held.open();
        until(|| {
            let (rt, id) = (rt.clone(), ws.view.id.clone());
            async move { get(&rt, &id).await.get("daemonNote").is_none() }
        })
        .await;
        let last = events.statuses().last().cloned().expect("a status");
        assert!(last.get("daemonNote").is_none());
        assert!(last["idleAt"].as_f64().expect("an idle deadline") > now_ms() as f64);
        rt.close().await.expect("a close");
        before.rt.close().await.expect("a close");
    });
}

// "a deploy that fails leaves the old daemon serving, says so on the row once and no longer, and logs the reason"
#[test]
fn a_deploy_that_fails_leaves_the_old_daemon_serving_says_so_on_the_row_once_and_no_longer_and_logs_the_reason() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let state = sqlite_state();
        let daemon = helloing_daemon(1, false, None).await;
        let before = runtime(&backend).store(&state.store).build();
        let ws = create_on(&before.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let m = backend.machine(0);
        m.set_preview_url(&format!("ws://127.0.0.1:{}", daemon.port));

        let at_deploy = Rc::new(Cell::new(usize::MAX));
        let (noted, machine) = (at_deploy.clone(), m.clone());
        let golden = recipe(Rc::new(move |_| {
            noted.set(machine.exec_log().len());
            Box::pin(async { Err(HostError::new("daemon deploy failed: NPM_FAIL")) })
        }));
        let fx = runtime(&backend)
            .store(&state.store)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.daemon_hello_timeout_ms = Some(2_000);
            })
            .build();
        let events = fx.events();
        let warned = capture();
        fx.rt.workspaces().list(None).await.expect("a list");
        until_n(|| events.statuses().iter().any(|s| s["daemonNote"] == DAEMON_UPDATE_FAILED)).await;
        // The row said it once. It is not on the workspace any more, so the next poll shows the rate and the countdown
        // again rather than a failure nobody here can act on.
        assert!(get(&fx.rt, &ws.view.id).await.get("daemonNote").is_none());
        assert_eq!(events.statuses().iter().filter(|s| s["daemonNote"] == DAEMON_UPDATE_FAILED).count(), 1);
        // The reason npm gave is in this host's log and nowhere a person reads.
        assert!(warned.lines().iter().any(|l| l.contains("daemon deploy failed: NPM_FAIL") && l.contains("m1")), "{:?}", warned.lines());
        assert!(events.statuses().iter().all(|s| s["daemonNote"].as_str().is_none_or(|n| !n.contains("NPM_FAIL"))));
        // Nothing reached the machine after the deploy threw, the token was not rotated away from the daemon that holds
        // it, and that daemon still answers with the version it always did.
        assert!(m.exec_log()[at_deploy.get()..].is_empty());
        assert_eq!(hello_of(daemon.port).await, 1);
        fx.rt.close().await.expect("a close");
        before.rt.close().await.expect("a close");
    });
}

// "a turn opened while the runtime still waits on the machine's hello launches once the update landed, so the deploy ends nothing it runs under"
#[test]
fn a_turn_opened_while_the_runtime_still_waits_on_the_machines_hello_launches_once_the_update_landed_so_the_deploy_ends_nothing_it_runs_under() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let state = sqlite_state();
        let daemon = helloing_daemon(1, true, None).await;
        let h = Held::new(false);
        let before = runtime(&backend).store(&state.store).build();
        let ws = create_on(&before.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        backend.machine(0).set_preview_url(&format!("ws://127.0.0.1:{}", daemon.port));

        let deployed: Rc<RefCell<Vec<String>>> = Rc::default();
        let (noted, turns) = (deployed.clone(), h.clone());
        let golden = recipe(Rc::new(move |machine| {
            noted.borrow_mut().push(format!("{} with {} turns", machine.id(), turns.turns()));
            Box::pin(async { Ok(None) })
        }));
        let fx = runtime(&backend)
            .store(&state.store)
            .adapter("claude", h.factory())
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.daemon_hello_timeout_ms = Some(5_000);
                o.broker = Some(Rc::new(Ready));
            })
            .build();
        let rt = fx.rt.clone();
        rt.workspaces().list(None).await.expect("a list");
        let starting = tokio::task::spawn_local({
            let (rt, id) = (rt.clone(), ws.view.id.clone());
            async move { rt.sessions().start(&id, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.map(|_| ()) }
        });
        sleep_ms(200).await;
        assert_eq!((deployed.borrow().clone(), h.turns()), (vec![], 0));

        daemon.release();
        starting.await.expect("a task").expect("a start");
        until_n(|| h.turns() == 1).await;
        assert_eq!(*deployed.borrow(), ["m1 with 0 turns"]);
        daemon.release();
        for turn in 0..h.turns() {
            h.end(turn, "done", json!({}));
        }
        rt.close().await.expect("a close");
        before.rt.close().await.expect("a close");
    });
}

/// An adapter that is `inner` in everything and notes, in `order`, the daemon version serving each time a turn starts.
struct Ordered {
    inner: Arc<dyn HarnessAdapter>,
    order: Rc<RefCell<Vec<String>>>,
    serving: Rc<Cell<i64>>,
}

impl HarnessAdapter for Ordered {
    fn start(&self, options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.order.borrow_mut().push(format!("turn on {}", self.serving.get()));
        self.inner.start(options)
    }
    fn steers(&self) -> bool {
        self.inner.steers()
    }
}

// "a send to a machine woken on an older daemon starts its turn once the update landed, so the agent runs on the daemon this host ships"
#[test]
fn a_send_to_a_machine_woken_on_an_older_daemon_starts_its_turn_once_the_update_landed_so_the_agent_runs_on_the_daemon_this_host_ships() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let state = sqlite_state();
        let serving = Rc::new(Cell::new(DAEMON_VERSION - 1));
        let daemon = Rc::new(helloing_daemon(serving.get(), false, None).await);
        let h = Held::new(false);
        let order: Rc<RefCell<Vec<String>>> = Rc::default();
        let base = h.factory();
        let (seen_order, seen_serving) = (order.clone(), serving.clone());
        let adapter: HarnessAdapterFactory = Arc::new(move |ctx: HarnessAdapterContext| {
            let inner = base(ctx)?;
            Ok(Arc::new(Ordered { inner, order: seen_order.clone(), serving: seen_serving.clone() }) as Arc<dyn HarnessAdapter>)
        });
        let before = runtime(&backend).store(&state.store).build();
        let ws = create_on(&before.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        before.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        before.rt.close().await.expect("a close");
        backend.machine(0).set_preview_url(&format!("ws://127.0.0.1:{}", daemon.port));

        let (noted, now_serving, announcing) = (order.clone(), serving.clone(), daemon.clone());
        let golden = recipe(Rc::new(move |_| {
            noted.borrow_mut().push("deploy".into());
            now_serving.set(DAEMON_VERSION);
            announcing.announce(DAEMON_VERSION);
            Box::pin(async { Ok(None) })
        }));
        let fx = runtime(&backend)
            .store(&state.store)
            .adapter("claude", adapter)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.daemon_hello_timeout_ms = Some(2_000);
                o.broker = Some(Rc::new(Ready));
            })
            .build();
        // The app's send to a napping workspace: a wake, then the start the moment the wake answers.
        assert_eq!(j(&fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake"))["phase"], "running");
        fx.rt.sessions().start(&ws.view.id, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.expect("a start");
        until_soft(|| order.borrow().len() >= 2).await;
        assert_eq!(*order.borrow(), ["deploy".to_string(), format!("turn on {DAEMON_VERSION}")]);
        for turn in 0..h.turns() {
            h.end(turn, "done", json!({}));
        }
        fx.rt.close().await.expect("a close");
    });
}

/// A fork a host before this one made, whose daemon is the one on this port, and the host starting again over the same
/// store on a fake clock with a deploy that brings the daemon to this wsp's version.
struct Restarted {
    rt: Rc<AssembledRuntime>,
    ws_id: String,
    fc: FakeClock,
    deployed: Rc<RefCell<Vec<String>>>,
    unread: String,
    _fx: Fx,
    _first: Fx,
    _state: State,
}

impl Restarted {
    /// One poll tick, waited out: the timer runs on the fake clock, the probe and the sync on real promises.
    async fn poll(&self, daemon: &HelloingDaemon) {
        let before = daemon.hits();
        self.fc.advance(POLL_INTERVAL_MS);
        until_n(|| daemon.hits() > before).await;
        sleep_ms(150).await;
    }
}

async fn restart_onto(daemon: &HelloingDaemon, deploys: bool, served: bool, adapters: Vec<(&str, HarnessAdapterFactory)>, broker: bool) -> Restarted {
    let backend = StubBackend::new(None);
    backend.set_exec_impl(token_guest());
    let state = sqlite_state();
    let fc = FakeClock::at_now();
    let first = runtime(&backend).store(&state.store).build();
    let ws = create_on(&first.rt, CreateOn::golden("a"), None).await.expect("a workspace");
    backend.machine(0).set_preview_url(&format!("http://127.0.0.1:{}/", daemon.port));
    if served {
        backend.machine(0).set_served(true);
    }
    let deployed: Rc<RefCell<Vec<String>>> = Rc::default();
    let (noted, version) = (deployed.clone(), DAEMON_VERSION);
    let golden = {
        let shared = daemon_announcer(daemon);
        recipe(Rc::new(move |machine| {
            noted.borrow_mut().push(machine.id().to_string());
            shared(version);
            Box::pin(async { Ok(None) })
        }))
    };
    let mut wiring = runtime(&backend).store(&state.store).clock(&fc);
    for (name, factory) in adapters {
        wiring = wiring.adapter(name, factory);
    }
    let fx = wiring
        .with(move |o| {
            o.daemon_token = Some(TOKEN.into());
            if broker {
                o.broker = Some(Rc::new(Ready));
            }
            if deploys {
                o.golden_recipe = Some(golden);
            }
            o.daemon_hello_timeout_ms = Some(100);
            o.status = Some(StatusWatchOptions { cost_interval_ms: Some(24 * 3_600_000), reconcile_min_ms: Some(60_000), ..Default::default() });
        })
        .build();
    let unread = format!("daemon on m1 (workspace {}): version not read within 0.1 s; asking again at the next reach probe", ws.view.id);
    Restarted { rt: fx.rt.clone(), ws_id: ws.view.id, fc, deployed, unread, _fx: fx, _first: first, _state: state }
}

/// The daemon's `announce`, held by a deploy that outlives the borrow of the daemon the test holds.
fn daemon_announcer(daemon: &HelloingDaemon) -> Rc<dyn Fn(i64)> {
    daemon.announcer()
}

fn warned_daemon(warned: &wsp_core::console::Captured) -> Vec<String> {
    warned.lines().into_iter().filter(|l| l.starts_with("daemon on")).collect()
}

// "a start that gives up before its turn reached the machine lets the daemon update through: its end says the turn stopped running"
#[test]
fn a_start_that_gives_up_before_its_turn_reached_the_machine_lets_the_daemon_update_through_its_end_says_the_turn_stopped_running() {
    struct Refusing {
        probed: Rc<Gate>,
    }
    impl HarnessAdapter for Refusing {
        fn start(&self, _: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
            Err(AdapterError("the harness would not launch".into()))
        }
        fn steers(&self) -> bool {
            false
        }
        fn probe_catalog(
            &self,
            _exec: Arc<dyn wsp_adapters::port::HarnessExec>,
        ) -> Option<futures::future::LocalBoxFuture<'static, Result<Option<wsp_adapters::port::HarnessCatalogAnswer>, wsp_adapters::port::ExecFailure>>> {
            let probed = self.probed.clone();
            Some(Box::pin(async move {
                probed.wait().await;
                Ok(None)
            }))
        }
    }
    drive(|| async {
        let daemon = helloing_daemon(1, true, None).await;
        let warned = capture();
        let probed = Gate::new();
        let refusing: HarnessAdapterFactory = {
            let probed = probed.clone();
            Arc::new(move |_| Ok(Arc::new(Refusing { probed: probed.clone() }) as Arc<dyn HarnessAdapter>))
        };
        let r = restart_onto(&daemon, true, false, vec![("claude", refusing)], true).await;
        r.rt.workspaces().list(None).await.expect("a list");
        until_n(|| warned.lines().contains(&r.unread)).await;
        sleep_ms(50).await;
        // The brokered start waits for a sync in flight before its launch; none is, so it holds its row and goes on.
        let giving = tokio::task::spawn_local({
            let (rt, id) = (r.rt.clone(), r.ws_id.clone());
            async move { rt.sessions().start(&id, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.map(|_| ()) }
        });
        until(|| {
            let rt = r.rt.clone();
            async move { rt.sessions().list(None, None).await.expect("a list").first().is_some_and(|s| j(s)["status"] == "running") }
        })
        .await;
        // The probe past the window reads the version again, and the update waits out the turn this start is opening.
        daemon.release();
        let stop = r.rt.status().watch(StatusWatchOptions::default());
        r.fc.advance(DAEMON_REVIVE_AGAIN_MS as i64);
        r.poll(&daemon).await;
        let waiting = format!("daemon on m1 (workspace {}): update waits for the running turn", r.ws_id);
        until_n(|| warned.lines().contains(&waiting)).await;
        assert!(r.deployed.borrow().is_empty());

        probed.open();
        let refused = giving.await.expect("a task").expect_err("a refusal");
        assert!(refused.message.contains("the harness would not launch"), "{}", refused.message);
        until_n(|| r.deployed.borrow().len() == 1).await;
        assert!(r.rt.sessions().list(None, None).await.expect("a list").is_empty());
        stop();
        r.rt.close().await.expect("a close");
    });
}

// "writes the folder the record's project names on every connect, and again after the update, so it is browsable without a second import"
#[test]
fn writes_the_folder_the_records_project_names_on_every_connect_and_again_after_the_update_so_it_is_browsable_without_a_second_import() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let state = sqlite_state();
        let daemon = helloing_daemon(1, false, None).await;
        let before = runtime(&backend).store(&state.store).build();
        let ws = create_on(&before.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let path = before
            .rt
            .projects()
            .list(None)
            .await
            .expect("a list")
            .iter()
            .map(|p| j(p))
            .find(|p| p["id"] == ws.view.project.id.as_str())
            .expect("the project")["path"]
            .as_str()
            .expect("a path")
            .to_string();
        let roots = write_daemon_roots_script(&[path], None);
        let m = backend.machine(0);
        m.set_preview_url(&format!("ws://127.0.0.1:{}", daemon.port));

        let at_deploy = Rc::new(Cell::new(usize::MAX));
        let (noted, machine, announce) = (at_deploy.clone(), m.clone(), daemon.announcer());
        let golden = recipe(Rc::new(move |_| {
            noted.set(machine.exec_log().len());
            announce(DAEMON_VERSION);
            Box::pin(async { Ok(None) })
        }));
        let from = m.exec_log().len();
        let fx = runtime(&backend)
            .store(&state.store)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.daemon_hello_timeout_ms = Some(2_000);
            })
            .build();
        fx.rt.workspaces().list(None).await.expect("a list");
        until_n(|| m.exec_log()[from..].iter().filter(|c| **c == roots).count() == 2).await;
        // Once before the daemon is asked anything, so the first files op lands; once after the daemon was replaced.
        let log = m.exec_log();
        assert_eq!(log[from..at_deploy.get()].iter().filter(|c| **c == roots).count(), 1);
        assert_eq!(log[at_deploy.get()..].iter().filter(|c| **c == roots).count(), 1);
        fx.rt.close().await.expect("a close");
        before.rt.close().await.expect("a close");
    });
}

// "puts the daemon back on a running machine whose port answers nothing, with nothing asking it to"
#[test]
fn puts_the_daemon_back_on_a_running_machine_whose_port_answers_nothing_with_nothing_asking_it_to() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        // Answers once, then dies: the first silence is a window, and only the second is a daemon that is gone.
        let probes = Rc::new(Cell::new(0u32));
        let back = Rc::new(Cell::new(false));
        let edge = daemon_port({
            let (probes, back) = (probes.clone(), back.clone());
            move || {
                probes.set(probes.get() + 1);
                probes.get() == 1 || back.get()
            }
        })
        .await;
        let deployed: Rc<RefCell<Vec<String>>> = Rc::default();
        let golden = noting(&deployed, {
            let back = back.clone();
            move || back.set(true)
        });
        let fx = runtime(&backend)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.status = Some(status_options(5, 60_000));
            })
            .build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        route_through(&backend.machine(0), edge.port, None);
        let events = fx.events();

        let stop = fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| deployed.borrow().len() == 1).await;
        // One silence is a window a restart sits inside; the deploy waits for the second.
        assert!(edge.hits() >= 3, "{}", edge.hits());
        until_n(|| events.statuses().last().is_some_and(|s| s["reach"]["state"] == "reachable")).await;
        stop();
        assert_eq!(*deployed.borrow(), ["m1"]);
        // The row said what was being done while it was being done, and says nothing once the daemon answers again.
        assert!(events.statuses().iter().filter(|s| s["daemonNote"] == DAEMON_RESTARTING).count() >= 1);
        assert!(get(&fx.rt, &ws.view.id).await.get("daemonNote").is_none());
        // The redeploy went through the same road the verb takes, so this machine's own token opens the new daemon.
        let rotated = rotate_daemon_token_script(&daemon_token_for(TOKEN, "m1").expect("a token"), None).expect("a script");
        assert!(backend.machine(0).exec_log().contains(&rotated));
        fx.rt.close().await.expect("a close");
    });
}

// "leaves a machine alone whose daemon answers again on the next poll: one silence is not a dead daemon"
#[test]
fn leaves_a_machine_alone_whose_daemon_answers_again_on_the_next_poll_one_silence_is_not_a_dead_daemon() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let probes = Rc::new(Cell::new(0u32));
        // Every other probe answers; the run of silences never reaches two.
        let edge = daemon_port({
            let probes = probes.clone();
            move || {
                probes.set(probes.get() + 1);
                probes.get() % 2 == 1
            }
        })
        .await;
        let deployed: Rc<RefCell<Vec<String>>> = Rc::default();
        let golden = noting(&deployed, || {});
        let fx = runtime(&backend)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.status = Some(status_options(5, 60_000));
            })
            .build();
        create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        route_through(&backend.machine(0), edge.port, None);
        let stop = fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.hits() >= 10).await;
        stop();
        assert!(deployed.borrow().is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "a redeploy that fails says so on the row once, logs the reason, holds the row at no-daemon, and is tried again a cooldown later"
#[test]
fn a_redeploy_that_fails_says_so_on_the_row_once_logs_the_reason_holds_the_row_at_no_daemon_and_is_tried_again_a_cooldown_later() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let fc = FakeClock::at_now();
        let edge = daemon_port(|| false).await;
        let deployed: Rc<RefCell<Vec<i64>>> = Rc::default();
        let (noted, clock) = (deployed.clone(), fc.clone());
        let golden = recipe(Rc::new(move |_| {
            noted.borrow_mut().push(clock.now());
            Box::pin(async { Err(HostError::new("daemon deploy failed: NPM_FAIL")) })
        }));
        let fx = runtime(&backend)
            .clock(&fc)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.status = Some(StatusWatchOptions { cost_interval_ms: Some(24 * 3_600_000), reconcile_min_ms: Some(60_000), ..Default::default() });
            })
            .build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        route_through(&backend.machine(0), edge.port, Some(fc.clone()));
        let events = fx.events();
        let warned = capture();
        // One poll tick, waited out: the timer runs on the fake clock, the probe and the deploy on real promises.
        let poll = || async {
            let before = edge.hits();
            fc.advance(POLL_INTERVAL_MS);
            until_n(|| edge.hits() > before).await;
            sleep_ms(30).await;
        };
        let stop = fx.rt.status().watch(StatusWatchOptions::default());
        poll().await;
        poll().await;
        until_n(|| deployed.borrow().len() == 1).await;
        until_n(|| events.statuses().iter().any(|s| s["daemonNote"] == DAEMON_RESTART_FAILED)).await;
        // Ten more polls, three minutes of them, all inside the cooldown: the machine is left alone.
        for _ in 0..10 {
            poll().await;
        }
        assert_eq!(deployed.borrow().len(), 1);
        // The row a client is left looking at says the machine's daemon is dead, not that the machine answers: a push made
        // while the deploy ran must not paint over what the poll measured, or the poll's own word after it reads as a
        // repeat and never reaches the bus.
        let last = events.statuses().last().cloned().expect("a status");
        assert_eq!(last["reach"]["state"], "no-daemon");
        assert!(last.get("daemonNote").is_none());
        // Past the cooldown, it tries again on its own.
        fc.advance(DAEMON_REVIVE_AGAIN_MS as i64);
        poll().await;
        until_n(|| deployed.borrow().len() == 2).await;
        stop();
        let at = deployed.borrow().clone();
        assert!(at[1] - at[0] >= DAEMON_REVIVE_AGAIN_MS as i64, "{at:?}");
        // Neither line about the daemon claims the machine answers. The runtime has no probe of its own between polls, so
        // a push carries what the poll measured rather than what a running machine's kind would claim.
        let about: Vec<Value> =
            events.statuses().into_iter().filter(|s| s["daemonNote"] == DAEMON_RESTARTING || s["daemonNote"] == DAEMON_RESTART_FAILED).collect();
        assert!(about.len() >= 3, "{}", about.len());
        assert!(about.iter().all(|s| s["reach"]["state"] == "no-daemon"));
        // The row said it and stopped saying it; the npm log is in this host's log and nowhere a person reads.
        assert!(get(&fx.rt, &ws.view.id).await.get("daemonNote").is_none());
        assert!(warned.lines().iter().any(|l| l.contains("not restarted") && l.contains("NPM_FAIL") && l.contains("m1")), "{:?}", warned.lines());
        assert!(events.statuses().iter().all(|s| s["daemonNote"].as_str().is_none_or(|n| !n.contains("NPM_FAIL"))));
        fx.rt.close().await.expect("a close");
    });
}

// "a sync whose version read answers nothing says so in the log, and asks again at the first probe past the revive window that finds the machine answering"
#[test]
fn a_sync_whose_version_read_answers_nothing_says_so_in_the_log_and_asks_again_at_the_first_probe_past_the_revive_window_that_finds_the_machine_answering() {
    drive(|| async {
        let daemon = helloing_daemon(DAEMON_VERSION - 1, true, None).await;
        let warned = capture();
        let said = || warned_daemon(&warned);
        let r = restart_onto(&daemon, true, false, vec![], false).await;
        r.rt.workspaces().list(None).await.expect("a list");
        until_n(|| said().len() == 1).await;
        assert_eq!(said(), [r.unread.clone()]);
        assert_eq!((r.deployed.borrow().clone(), daemon.dials()), (vec![], 1));

        // Probes inside the window find the machine answering and ask it nothing.
        let stop = r.rt.status().watch(StatusWatchOptions::default());
        r.poll(&daemon).await;
        r.poll(&daemon).await;
        assert_eq!((r.deployed.borrow().clone(), daemon.dials(), said()), (vec![], 1, vec![r.unread.clone()]));

        // Past it, the probe that finds the machine answering runs the sync again, and this time the hello lands.
        daemon.release();
        r.fc.advance(DAEMON_REVIVE_AGAIN_MS as i64);
        r.poll(&daemon).await;
        until_n(|| r.deployed.borrow().len() == 1).await;
        assert_eq!(*r.deployed.borrow(), ["m1"]);
        until(|| {
            let (rt, id) = (r.rt.clone(), r.ws_id.clone());
            async move { get(&rt, &id).await.get("daemonNote").is_none() }
        })
        .await;

        // The read that answered took the mark off: a later window asks nothing more.
        let dialled = daemon.dials();
        r.fc.advance(DAEMON_REVIVE_AGAIN_MS as i64);
        r.poll(&daemon).await;
        r.poll(&daemon).await;
        assert_eq!((r.deployed.borrow().clone(), daemon.dials(), said()), (vec!["m1".to_string()], dialled, vec![r.unread.clone()]));
        stop();
        r.rt.close().await.expect("a close");
    });
}

// "a version read that answers nothing again says so again, one that answers current deploys nothing and asks no more, and a host with no deploy road or a workspace its computer serves neither dials nor warns"
#[test]
fn a_version_read_that_answers_nothing_again_says_so_again_one_that_answers_current_deploys_nothing_and_asks_no_more_and_a_host_with_no_deploy_road_or_a_workspace_its_computer_serves_neither_dials_nor_warns() {
    drive(|| async {
        let warned = capture();
        let said = || warned_daemon(&warned);
        let mut stops: Vec<Rc<dyn Fn()>> = vec![];
        let daemon = helloing_daemon(DAEMON_VERSION, true, None).await;
        let bare = helloing_daemon(DAEMON_VERSION - 1, true, None).await;
        let boxed = helloing_daemon(DAEMON_VERSION - 1, true, None).await;
        let r = restart_onto(&daemon, true, false, vec![], false).await;
        r.rt.workspaces().list(None).await.expect("a list");
        until_n(|| said().len() == 1).await;
        stops.push(r.rt.status().watch(StatusWatchOptions::default()));
        r.fc.advance(DAEMON_REVIVE_AGAIN_MS as i64);
        r.poll(&daemon).await;
        until_n(|| said().len() == 2).await;
        assert_eq!((said(), daemon.dials()), (vec![r.unread.clone(), r.unread.clone()], 2));

        daemon.release();
        r.fc.advance(DAEMON_REVIVE_AGAIN_MS as i64);
        r.poll(&daemon).await;
        until_n(|| daemon.dials() == 3).await;
        r.fc.advance(DAEMON_REVIVE_AGAIN_MS as i64);
        r.poll(&daemon).await;
        r.poll(&daemon).await;
        assert_eq!((r.deployed.borrow().clone(), daemon.dials(), said()), (vec![], 3, vec![r.unread.clone(), r.unread.clone()]));

        // A host wired without the bundle has nothing to ask a version for.
        let without = restart_onto(&bare, false, false, vec![], false).await;
        without.rt.workspaces().list(None).await.expect("a list");
        stops.push(without.rt.status().watch(StatusWatchOptions::default()));
        without.fc.advance(DAEMON_REVIVE_AGAIN_MS as i64);
        without.poll(&bare).await;
        without.poll(&bare).await;
        assert_eq!((bare.dials(), said().len()), (0, 2));

        // Nor is anything asked inside a workspace whose computer serves its daemon.
        let boxed_in = restart_onto(&boxed, true, true, vec![], false).await;
        boxed_in.rt.workspaces().list(None).await.expect("a list");
        stops.push(boxed_in.rt.status().watch(StatusWatchOptions::default()));
        boxed_in.fc.advance(DAEMON_REVIVE_AGAIN_MS as i64 + POLL_INTERVAL_MS);
        sleep_ms(300).await;
        assert_eq!((boxed.dials(), said().len(), boxed_in.deployed.borrow().clone()), (0, 2, vec![]));
        for stop in &stops {
            stop();
        }
        r.rt.close().await.expect("a close");
    });
}

// "puts this wsp's daemon on a fresh fork inside its own create, where the image carries an older one, and says nothing about a workspace that is not there yet"
#[test]
fn puts_this_wsps_daemon_on_a_fresh_fork_inside_its_own_create_where_the_image_carries_an_older_one_and_says_nothing_about_a_workspace_that_is_not_there_yet() {
    drive(|| async {
        let daemon = helloing_daemon(DAEMON_VERSION - 1, false, None).await;
        let deployed: Rc<RefCell<Vec<String>>> = Rc::default();
        // The clone the create runs is held until the deploy has landed, and the deploy waits for the clone to be asked, so
        // the deploy provably runs while the record is still creating rather than early by timing.
        let clone_asked = Gate::new();
        let deploy_done = Gate::new();
        let backend = StubBackend::new(None);
        let (asked, done) = (clone_asked.clone(), deploy_done.clone());
        backend.set_exec_impl(exec_async(move |_, cmd| {
            let (asked, done) = (asked.clone(), done.clone());
            Box::pin(async move {
                if cmd.contains("clone") {
                    asked.open();
                    done.wait().await;
                }
                Ok(token_guest_answer(&cmd))
            })
        }));
        let create = backend.create_fn();
        let port = daemon.port;
        backend.set_create(Rc::new(move |spec, waiting, sent| {
            let made = create(spec, waiting, sent);
            Box::pin(async move {
                let m = made.await?;
                m.set_preview_url(&format!("http://127.0.0.1:{port}/"));
                Ok(m)
            })
        }));
        let warned = capture();
        let (noted, waiting_clone, finished, announce) = (deployed.clone(), clone_asked.clone(), deploy_done.clone(), daemon.announcer());
        let golden = recipe(Rc::new(move |machine| {
            let (noted, waiting_clone, finished, announce) = (noted.clone(), waiting_clone.clone(), finished.clone(), announce.clone());
            Box::pin(async move {
                waiting_clone.wait().await;
                noted.borrow_mut().push(machine.id().to_string());
                announce(DAEMON_VERSION);
                finished.open();
                Ok(None)
            })
        }));
        let fx = runtime(&backend)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.daemon_hello_timeout_ms = Some(2_000);
            })
            .build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        assert_eq!(*deployed.borrow(), ["m1"]);
        // Nothing about the daemon reached the log: no refusal for a workspace the verb's door cannot see yet.
        assert!(warned_daemon(&warned).is_empty(), "{:?}", warned.lines());
        assert!(get(&fx.rt, &ws.view.id).await.get("daemonNote").is_none());
        fx.rt.close().await.expect("a close");
    });
}

// "a machine replaced under the record starts its own attempt instead of inheriting the old machine's cooldown"
#[test]
fn a_machine_replaced_under_the_record_starts_its_own_attempt_instead_of_inheriting_the_old_machines_cooldown() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let edge = daemon_port(|| false).await;
        let deployed: Rc<RefCell<Vec<String>>> = Rc::default();
        let noted = deployed.clone();
        let golden = recipe(Rc::new(move |machine| {
            noted.borrow_mut().push(machine.id().to_string());
            Box::pin(async { Err(HostError::new("daemon deploy failed: NPM_FAIL")) })
        }));
        let fx = runtime(&backend)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.daemon_hello_timeout_ms = Some(100);
                o.status = Some(status_options(5, 60_000));
            })
            .build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        route_through(&backend.machine(0), edge.port, None);
        let _warn = capture();
        let stop = fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| deployed.borrow().len() == 1).await;
        fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        route_through(&backend.machine(1), edge.port, None);
        // Well inside the cooldown the first machine earned: the entry is that machine's, and this is another one.
        until_for(5_000, &mut || {
            let deployed = deployed.clone();
            async move { deployed.borrow().len() == 2 }
        })
        .await;
        stop();
        fx.rt.close().await.expect("a close");
        assert_eq!(*deployed.borrow(), ["m1", "m2"]);
    });
}

// "leaves a dead daemon alone on a backend that mints no upload URL, since the deploy has no road to the machine"
#[test]
fn leaves_a_dead_daemon_alone_on_a_backend_that_mints_no_upload_url_since_the_deploy_has_no_road_to_the_machine() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        backend.set_capabilities(|c| c.signed_urls = false);
        let edge = daemon_port(|| false).await;
        let deployed: Rc<RefCell<Vec<String>>> = Rc::default();
        let golden = noting(&deployed, || {});
        let fx = runtime(&backend)
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(golden);
                o.status = Some(status_options(5, 60_000));
            })
            .build();
        create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        route_through(&backend.machine(0), edge.port, None);
        let stop = fx.rt.status().watch(StatusWatchOptions::default());
        until_n(|| edge.hits() >= 10).await;
        stop();
        assert!(deployed.borrow().is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "updateDaemon refuses on a runtime whose recipe carries no deploy"
#[test]
fn update_daemon_refuses_on_a_runtime_whose_recipe_carries_no_deploy() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let refused = fx.rt.workspaces().update_daemon(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("cannot deploy a daemon"), "{}", refused.message);
        let unknown = fx.rt.workspaces().update_daemon("ws_nobody", None).await.expect_err("a refusal");
        assert!(unknown.message.contains("no such workspace"), "{}", unknown.message);
        fx.rt.close().await.expect("a close");
    });
}

// "writes a token again after a rebuild replaces the machine, the new machine's own"
#[test]
fn writes_a_token_again_after_a_rebuild_replaces_the_machine_the_new_machines_own() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let fx = runtime(&backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        preview_example(&backend.machine(0), "e", None);
        let gone = reach_of(&fx.rt, &ws.view.id).await["daemonToken"].as_str().expect("a token").to_string();

        fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        preview_example(&backend.machine(1), "e", None);
        let fresh = reach_of(&fx.rt, &ws.view.id).await["daemonToken"].as_str().expect("a token").to_string();
        assert_ne!(fresh, gone);
        assert_eq!(exec_with_token(&backend.machine(1)), [rotate_daemon_token_script(&fresh, None).expect("a script")]);
        fx.rt.close().await.expect("a close");
    });
}
