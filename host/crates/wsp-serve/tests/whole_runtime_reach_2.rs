// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-reach.test.ts at 65f6b89c7, second part: "a machine the provider answers it cannot
//! reach", "runtime port reach" and "runtime port probe", case for case (the first part is `whole_runtime_reach_1.rs`).
mod common;

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use common::gone::until_n;
use common::reach::{daemon_port, route_through, DaemonPort};
use common::*;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::status::{Reconcile, StatusListOptions, StatusReader, StatusWatchOptions, POLL_INTERVAL_MS};
use wsp_core::types::wiring::{GoldenRecipe, Local, OriginStatusApi, PORT_PROBE_BODY_CAP};
use wsp_engine::errors::EngineError;
use wsp_serve::AssembledRuntime;
use wsp_store::store::Store;
use wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH;
use wsp_words::exit::HostError;
use wsp_words::words::thread::{exec_failed_line, machine_unreachable_line, DAEMON_RESTARTING, DAEMON_RESTART_FAILED};

const TOKEN: &str = "deadbeefdeadbeefdeadbeef";

fn line() -> String {
    machine_unreachable_line("Sandbox is not reachable")
}

/// What `wsp workspaces --json` asks the runtime for.
fn table() -> StatusListOptions {
    StatusListOptions {
        reconcile: Some(Reconcile::OnFailure),
        zombie_probe: Some(false),
        reader: Some(StatusReader::Table),
        ..Default::default()
    }
}

async fn table_row(rt: &Rc<AssembledRuntime>) -> Value {
    let rows = OriginStatusApi::list(&*rt.status(), Some(table()), None).await.expect("a status list");
    j(&rows[0])
}

type Names = Rc<RefCell<HashSet<String>>>;

struct Fork {
    backend: Arc<StubBackend>,
    store: Rc<dyn Store>,
    fx: Fx,
    ws_id: String,
    fc: FakeClock,
    refusing: Names,
    hanging: Names,
    edge: DaemonPort,
    events: Events,
    _store: Rc<dyn Store>,
}

/// A fork whose daemon answers over its route while the provider refuses every command on the machines in `refusing`
/// with `refusal`, and answers none on those in `hanging`.
struct ForkOptions {
    up: Rc<dyn Fn() -> bool>,
    recipe: Option<GoldenRecipe>,
    exec_failed: bool,
    refusing: Names,
}

impl Default for ForkOptions {
    fn default() -> Self {
        ForkOptions { up: Rc::new(|| true), recipe: None, exec_failed: false, refusing: Rc::default() }
    }
}

async fn refused_fork(o: ForkOptions) -> Fork {
    let backend = StubBackend::new(None);
    let refusing = o.refusing.clone();
    let hanging: Names = Rc::default();
    let (refused, held, exec_failed) = (refusing.clone(), hanging.clone(), o.exec_failed);
    backend.set_exec_impl(exec_async(move |m, cmd| {
        let id = m.id().to_string();
        if refused.borrow().contains(&id) {
            let said = if exec_failed {
                EngineError::exec_failed(&id, "exec failed", 502)
            } else {
                EngineError::machine_unreachable(&id, "Sandbox is not reachable", 502, &line())
            };
            return Box::pin(async move { Err(said) });
        }
        if held.borrow().contains(&id) {
            return Box::pin(futures::future::pending());
        }
        Box::pin(async move { Ok(token_guest_answer(&cmd)) })
    }));
    let fc = FakeClock::at_now();
    let edge = daemon_port({
        let up = o.up.clone();
        move || up()
    })
    .await;
    let store = memory_state();
    let recipe = o.recipe;
    let fx = runtime(&backend)
        .store(&store)
        .clock(&fc)
        .with(move |opts| {
            opts.daemon_token = Some(TOKEN.into());
            if let Some(recipe) = recipe {
                opts.golden_recipe = Some(recipe);
            }
            opts.status =
                Some(StatusWatchOptions { cost_interval_ms: Some(24 * 3_600_000), reconcile_min_ms: Some(60_000), ..Default::default() });
        })
        .build();
    let ws = create_on(&fx.rt, CreateOn::golden("far"), None).await.expect("a workspace");
    route_through(&backend.machine(0), edge.port, Some(fc.clone()));
    // The route's token is read once and kept, as on a fork a host has dialled before the provider lost it.
    fx.rt.workspaces().daemon_reach(&ws.view.id, None).await.expect("a reach");
    let events = fx.events();
    Fork { backend, store: store.clone(), fx, ws_id: ws.view.id, fc, refusing, hanging, edge, events, _store: store }
}

impl Fork {
    fn pushed(&self) -> Vec<Value> {
        self.events.statuses()
    }
    /// One poll tick, waited out: the timer runs on the fake clock, the probe and the heal on real promises.
    async fn ticked(&self, tick: impl FnOnce()) {
        let before = self.edge.hits();
        tick();
        until_n(|| self.edge.hits() > before).await;
        sleep_ms(30).await;
    }
    async fn poll(&self) {
        self.ticked(|| self.fc.advance(POLL_INTERVAL_MS)).await;
    }
    /// The watch polls once at once; that tick is waited out too, so each poll after it is one tick.
    async fn watch(&self) -> Rc<dyn Fn()> {
        let mut stop: Option<Rc<dyn Fn()>> = None;
        self.ticked(|| stop = Some(self.fx.rt.status().watch(StatusWatchOptions::default()))).await;
        stop.expect("a watch")
    }
    fn probes(&self, m: &Arc<StubMachine>) -> usize {
        let _ = self;
        m.exec_log().iter().filter(|c| *c == "echo ok").count()
    }
    async fn exec_refused(&self) -> HostError {
        self.fx.rt.workspaces().exec(&self.ws_id, "true", None, None).await.expect_err("a refusal")
    }
    fn said(&self) -> Vec<String> {
        self.pushed()
            .iter()
            .map(|s| format!("{} {}", s["reach"]["state"].as_str().unwrap_or(""), s["reason"].as_str().unwrap_or("-")))
            .collect()
    }
}

// "a machine the provider answers it cannot reach" > "puts the provider's sentence on the row the moment an exec is refused, and the table reads the same"
#[test]
fn puts_the_providers_sentence_on_the_row_the_moment_an_exec_is_refused_and_the_table_reads_the_same() {
    drive(|| async {
        let f = refused_fork(ForkOptions::default()).await;
        f.refusing.borrow_mut().insert("m1".into());
        let refused = f.exec_refused().await;
        assert!(refused.message.contains(&line()), "{}", refused.message);
        assert_matches(
            f.pushed().last().expect("a status"),
            json!({ "id": f.ws_id, "machineState": "running", "reach": { "state": "unreachable" }, "reason": line() }),
        );
        let row = table_row(&f.fx.rt).await;
        assert_matches(&row, json!({ "id": f.ws_id, "machineState": "running", "reach": { "state": "unreachable" }, "reason": line() }));
        assert!(row["reach"]["url"].as_str().expect("a url").contains("127.0.0.1"));
        assert!(f.probes(&f.backend.machine(0)) <= 1);
        f.fx.rt.close().await.expect("a close");
    });
}

// "heals at the next tick when the tracker's own probe answers through the machine, and the row reads the probe again"
#[test]
fn heals_at_the_next_tick_when_the_trackers_own_probe_answers_through_the_machine_and_the_row_reads_the_probe_again() {
    drive(|| async {
        let f = refused_fork(ForkOptions::default()).await;
        f.refusing.borrow_mut().insert("m1".into());
        let refused = f.exec_refused().await;
        assert!(refused.message.contains(&line()), "{}", refused.message);
        f.refusing.borrow_mut().remove("m1");
        let before = f.pushed().len();
        let stop = f.watch().await;
        assert_eq!(f.probes(&f.backend.machine(0)), 1);
        f.poll().await;
        stop();
        // The answer pushes nothing and the tick it healed under drops the row it built marked: the next tick speaks once.
        assert_eq!(f.said()[before..], ["reachable -".to_string()]);
        assert_eq!(f.probes(&f.backend.machine(0)), 1);
        f.fx.rt.close().await.expect("a close");
    });
}

// "puts the second answer's sentence on the row when the provider cannot run commands on the machine, and the table reads the same"
#[test]
fn puts_the_second_answers_sentence_on_the_row_when_the_provider_cannot_run_commands_on_the_machine_and_the_table_reads_the_same() {
    drive(|| async {
        let exec_line = exec_failed_line("exec failed");
        let f = refused_fork(ForkOptions { exec_failed: true, ..Default::default() }).await;
        f.refusing.borrow_mut().insert("m1".into());
        let refused = f.exec_refused().await;
        assert!(refused.message.contains(&exec_line), "{}", refused.message);
        assert_matches(
            f.pushed().last().expect("a status"),
            json!({ "id": f.ws_id, "machineState": "running", "reach": { "state": "unreachable" }, "reason": exec_line }),
        );
        let row = table_row(&f.fx.rt).await;
        assert_matches(&row, json!({ "id": f.ws_id, "machineState": "running", "reach": { "state": "unreachable" }, "reason": exec_line }));
        assert!(row["reach"]["url"].as_str().expect("a url").contains("127.0.0.1"));
        f.fx.rt.close().await.expect("a close");
    });
}

// "heals the second answer at the next tick as it heals the first"
#[test]
fn heals_the_second_answer_at_the_next_tick_as_it_heals_the_first() {
    drive(|| async {
        let f = refused_fork(ForkOptions { exec_failed: true, ..Default::default() }).await;
        f.refusing.borrow_mut().insert("m1".into());
        let refused = f.exec_refused().await;
        assert!(refused.message.contains(&exec_failed_line("exec failed")), "{}", refused.message);
        f.refusing.borrow_mut().remove("m1");
        let before = f.pushed().len();
        let stop = f.watch().await;
        assert_eq!(f.probes(&f.backend.machine(0)), 1);
        f.poll().await;
        stop();
        assert_eq!(f.said()[before..], ["reachable -".to_string()]);
        assert_eq!(f.probes(&f.backend.machine(0)), 1);
        f.fx.rt.close().await.expect("a close");
    });
}

/// What the provider does while the tick's own reach probe is out: it starts refusing, after that tick read its records.
type Slot = Rc<RefCell<Option<(Rc<AssembledRuntime>, String)>>>;

fn refusing_on_the_first_probe(refusing: &Names, slot: &Slot, answer: bool) -> Rc<dyn Fn() -> bool> {
    let (refusing, slot, asked) = (refusing.clone(), slot.clone(), Rc::new(Cell::new(0u32)));
    Rc::new(move || {
        asked.set(asked.get() + 1);
        if asked.get() == 1 {
            refusing.borrow_mut().insert("m1".into());
            let held = slot.borrow().clone();
            if let Some((rt, id)) = held {
                fire(async move {
                    let _ = rt.workspaces().exec(&id, "true", None, None).await;
                });
            }
        }
        answer
    })
}

// "a refusal that lands while a tick is being built is not painted over by the row that tick built before it"
#[test]
fn a_refusal_that_lands_while_a_tick_is_being_built_is_not_painted_over_by_the_row_that_tick_built_before_it() {
    drive(|| async {
        let refusing: Names = Rc::default();
        let slot: Slot = Rc::default();
        // The provider starts refusing while the first tick's reach probe is out, after that tick read its records.
        let f = refused_fork(ForkOptions {
            up: refusing_on_the_first_probe(&refusing, &slot, true),
            refusing: refusing.clone(),
            ..Default::default()
        })
        .await;
        *slot.borrow_mut() = Some((f.fx.rt.clone(), f.ws_id.clone()));
        let stop = f.watch().await;
        assert_eq!(f.said(), [format!("unreachable {}", line())]);
        f.poll().await;
        stop();
        assert_eq!(f.said(), [format!("unreachable {}", line()), format!("unreachable {}", line())]);
        f.fx.rt.close().await.expect("a close");
    });
}

// "probes a machine the provider keeps refusing once per tick, and the row keeps its word"
#[test]
fn probes_a_machine_the_provider_keeps_refusing_once_per_tick_and_the_row_keeps_its_word() {
    drive(|| async {
        let f = refused_fork(ForkOptions::default()).await;
        f.refusing.borrow_mut().insert("m1".into());
        let refused = f.exec_refused().await;
        assert!(refused.message.contains(&line()), "{}", refused.message);
        let stop = f.watch().await;
        assert_eq!(f.probes(&f.backend.machine(0)), 1);
        for i in 2..=4 {
            f.poll().await;
            assert_eq!(f.probes(&f.backend.machine(0)), i);
        }
        stop();
        assert_matches(
            f.pushed().last().expect("a status"),
            json!({ "id": f.ws_id, "reach": { "state": "unreachable" }, "reason": line() }),
        );
        f.fx.rt.close().await.expect("a close");
    });
}

// "leaves a marked machine whose daemon reads dead alone: no deploy over the refused exec, no restart line, no flash"
#[test]
fn leaves_a_marked_machine_whose_daemon_reads_dead_alone_no_deploy_over_the_refused_exec_no_restart_line_no_flash() {
    drive(|| async {
        let deployed: Rc<RefCell<Vec<String>>> = Rc::default();
        let noted = deployed.clone();
        let golden = GoldenRecipe {
            setup: "true".into(),
            smoke: "true".into(),
            deploy_daemon: Some(Rc::new(move |machine, _| -> Local<'static, Option<String>> {
                noted.borrow_mut().push(machine.id().to_string());
                Box::pin(async { Ok(None) })
            })),
            ..Default::default()
        };
        let refusing: Names = Rc::default();
        let slot: Slot = Rc::default();
        // The provider starts refusing while the tick's own reach probe is out, so the probe's no-daemon reaches the revive
        // with the mark standing.
        let f = refused_fork(ForkOptions {
            up: refusing_on_the_first_probe(&refusing, &slot, false),
            recipe: Some(golden),
            refusing: refusing.clone(),
            ..Default::default()
        })
        .await;
        *slot.borrow_mut() = Some((f.fx.rt.clone(), f.ws_id.clone()));
        let warned = capture();
        let stop = f.watch().await;
        // Long enough for a revive's deploy, its roots write and its flash to have run on real promises.
        sleep_ms(150).await;
        assert!(deployed.borrow().is_empty());
        assert!(warned.lines().iter().all(|l| !l.contains("not restarted")), "{:?}", warned.lines());
        assert!(f.pushed().iter().all(|s| s["daemonNote"] != DAEMON_RESTART_FAILED && s["daemonNote"] != DAEMON_RESTARTING));
        f.poll().await;
        assert_matches(f.pushed().last().expect("a status"), json!({ "reach": { "state": "unreachable" }, "reason": line() }));
        stop();
        f.fx.rt.close().await.expect("a close");
    });
}

// "a rebuild that puts another machine under the record drops the mark"
#[test]
fn a_rebuild_that_puts_another_machine_under_the_record_drops_the_mark() {
    drive(|| async {
        let f = refused_fork(ForkOptions::default()).await;
        f.refusing.borrow_mut().insert("m1".into());
        let refused = f.exec_refused().await;
        assert!(refused.message.contains(&line()), "{}", refused.message);
        f.fx.rt.workspaces().rebuild(&f.ws_id, None).await.expect("a rebuild");
        route_through(&f.backend.machine(1), f.edge.port, Some(f.fc.clone()));
        let row = table_row(&f.fx.rt).await;
        assert_eq!(row["machineId"], "m2");
        assert_eq!(row["reach"]["state"], "reachable");
        assert!(row.get("reason").is_none(), "{row}");
        f.fx.rt.close().await.expect("a close");
    });
}

// "a workspace forgotten leaves no mark behind for the same machine should the sweep record it again"
#[test]
fn a_workspace_forgotten_leaves_no_mark_behind_for_the_same_machine_should_the_sweep_record_it_again() {
    drive(|| async {
        let f = refused_fork(ForkOptions::default()).await;
        let m1 = f.backend.machine(0);
        f.refusing.borrow_mut().insert("m1".into());
        let refused = f.exec_refused().await;
        assert!(refused.message.contains(&line()), "{}", refused.message);
        let stamp = f.store.get("workspace-names", &f.ws_id).await.expect("a read").expect("a stamp");
        m1.killed.set(true);
        f.fx.rt.workspaces().forget(&f.ws_id, None).await.expect("a forget");
        m1.killed.set(false);
        // The stamp the sweep names a machine's workspace from, as a store the machine outlived still holds it.
        f.store.put("workspace-names", &f.ws_id, stamp).await.expect("a write");
        // Neither a refusal nor an answer from here on: only what the forget left can put the sentence back.
        f.refusing.borrow_mut().remove("m1");
        f.hanging.borrow_mut().insert("m1".into());
        let mut labels = m1.labels_now().unwrap_or_default();
        labels.insert("createdAt".into(), iso(now_ms() - 2 * 60_000));
        m1.spec.borrow_mut().labels = Some(labels.clone());
        m1.set_labels(labels);
        let swept = f.fx.rt.reap(None, None).await.expect("a sweep");
        let adopted = swept.adopted.expect("an adopted list");
        assert_eq!(adopted.len(), 1);
        assert_eq!((adopted[0].id.as_str(), adopted[0].workspace_id.as_str()), ("m1", f.ws_id.as_str()));
        let row = table_row(&f.fx.rt).await;
        assert_matches(&row, json!({ "id": f.ws_id, "machineId": "m1" }));
        assert_eq!(row["reach"]["state"], "reachable");
        assert_ne!(row["reason"], line().as_str());
        // The TS never closes this runtime: its machine's exec is held for good, and a close would wait on it.
    });
}

fn iso(ms: i64) -> String {
    wsp_wire::views::activity::iso_time(ms)
}

// "runtime port reach" > "mints a guest port's route once while fresh, caches per port, and never reads or carries the daemon token"
#[test]
fn mints_a_guest_ports_route_once_while_fresh_caches_per_port_and_never_reads_or_carries_the_daemon_token() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let minted: Rc<RefCell<Vec<u16>>> = Rc::default();
        let noted = minted.clone();
        backend.machine(0).set_preview(Some(Rc::new(move |port| {
            noted.borrow_mut().push(port);
            let reach = wsp_engine::machine::PreviewReach {
                url: format!("https://m1-{port}.preview.example/?pt_token=edge"),
                token: "edge".into(),
                expires_at: wsp_wire::types::JsNumber((now_ms() + 3_600_000) as f64),
            };
            Box::pin(async move { Ok(reach) })
        })));

        // Nothing listens on 3000 in this fixture: a typed-in port still mints, the tab decides what to show.
        let reach = j(&fx.rt.workspaces().port_reach(&ws.view.id, 3000, None).await.expect("a reach"));
        assert_eq!(reach["url"], "https://m1-3000.preview.example/?pt_token=edge");
        assert!(reach["expiresAt"].is_number());
        assert_eq!(reach.as_object().expect("an object").len(), 2);
        fx.rt.workspaces().port_reach(&ws.view.id, 3000, None).await.expect("a reach");
        fx.rt.workspaces().port_reach(&ws.view.id, 5173, None).await.expect("a reach");
        assert_eq!(*minted.borrow(), [3000, 5173]);
        assert!(backend.machine(0).exec_containing(DAEMON_TOKEN_PATH).is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "rejects an unknown workspace"
#[test]
fn rejects_an_unknown_workspace() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let refused = fx.rt.workspaces().port_reach("ws_nobody", 3000, None).await.expect_err("a refusal");
        assert!(refused.message.contains("no such workspace"), "{}", refused.message);
        fx.rt.close().await.expect("a close");
    });
}

const VITE_BLOCKED: &str =
    "Blocked request. This host (m1-5173.preview.example) is not allowed. To allow this host, add it to server.allowedHosts";

/// One answer of a guest port: a status and headers, a body, and whether the connection is held open after it.
struct Reply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
    hold: bool,
}

impl Reply {
    fn of(status: u16, body: &str) -> Reply {
        Reply { status, headers: vec![("content-type", "text/plain".into())], body: body.as_bytes().to_vec(), hold: false }
    }
}

/// A guest port as the preview edge would relay it: `answer` is asked for the path of every request.
struct Guest {
    port: u16,
    task: JoinHandle<()>,
}

impl Drop for Guest {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Guest {
    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/?pt_token=edge", self.port)
    }
}

async fn guest_port(answer: impl Fn(&str) -> Reply + 'static) -> Guest {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
    let port = listener.local_addr().expect("an address").port();
    let answer = Rc::new(answer);
    let task = tokio::task::spawn_local(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let answer = answer.clone();
            tokio::task::spawn_local(async move {
                let mut head = vec![0u8; 8192];
                let n = stream.read(&mut head).await.unwrap_or(0);
                let text = String::from_utf8_lossy(&head[..n]).to_string();
                let path = text.split_whitespace().nth(1).unwrap_or("/").to_string();
                let reply = answer(&path);
                let mut out = format!("HTTP/1.1 {} X\r\n", reply.status);
                for (k, v) in &reply.headers {
                    out.push_str(&format!("{k}: {v}\r\n"));
                }
                if !reply.hold {
                    out.push_str(&format!("content-length: {}\r\nconnection: close\r\n", reply.body.len()));
                }
                out.push_str("\r\n");
                let _ = stream.write_all(out.as_bytes()).await;
                let _ = stream.write_all(&reply.body).await;
                if reply.hold {
                    futures::future::pending::<()>().await;
                }
                let _ = stream.shutdown().await;
            });
        }
    });
    Guest { port, task }
}

async fn probe_rig() -> (Arc<StubBackend>, Fx, String) {
    let backend = StubBackend::new(None);
    backend.set_exec_impl(token_guest());
    let fx = runtime(&backend).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
    (backend, fx, ws.view.id)
}

fn route_to(backend: &Arc<StubBackend>, url: String) {
    backend.machine(0).set_preview(Some(Rc::new(move |_| {
        let reach = wsp_engine::machine::PreviewReach {
            url: url.clone(),
            token: "edge".into(),
            expires_at: wsp_wire::types::JsNumber((now_ms() + 3_600_000) as f64),
        };
        Box::pin(async move { Ok(reach) })
    })));
}

// "runtime port probe" > "fetches the port's minted route once and reports the status and body a frame cannot read"
#[test]
fn fetches_the_ports_minted_route_once_and_reports_the_status_and_body_a_frame_cannot_read() {
    drive(|| async {
        let (backend, fx, id) = probe_rig().await;
        let guest = guest_port(|_| Reply::of(403, VITE_BLOCKED)).await;
        let minted: Rc<RefCell<Vec<u16>>> = Rc::default();
        let (noted, url) = (minted.clone(), guest.url());
        backend.machine(0).set_preview(Some(Rc::new(move |port| {
            noted.borrow_mut().push(port);
            let reach = wsp_engine::machine::PreviewReach {
                url: url.clone(),
                token: "edge".into(),
                expires_at: wsp_wire::types::JsNumber((now_ms() + 3_600_000) as f64),
            };
            Box::pin(async move { Ok(reach) })
        })));

        assert_eq!(
            j(&fx.rt.workspaces().port_probe(&id, 5173, None).await.expect("a probe")),
            json!({ "status": 403, "body": VITE_BLOCKED })
        );
        assert_eq!(*minted.borrow(), [5173]);
        fx.rt.close().await.expect("a close");
    });
}

// "cuts the body at the cap so a page never rides the reply whole"
#[test]
fn cuts_the_body_at_the_cap_so_a_page_never_rides_the_reply_whole() {
    drive(|| async {
        let (backend, fx, id) = probe_rig().await;
        let body = format!("{:x<width$}", "<html>", width = PORT_PROBE_BODY_CAP + 500);
        let guest = guest_port(move |_| Reply::of(200, &body)).await;
        route_to(&backend, guest.url());
        let probe = j(&fx.rt.workspaces().port_probe(&id, 3000, None).await.expect("a probe"));
        assert_eq!(probe["status"], 200);
        assert_eq!(probe["body"].as_str().expect("a body").len(), PORT_PROBE_BODY_CAP);
        fx.rt.close().await.expect("a close");
    });
}

// "never follows a redirect: a 302 on the route is the frame's business, not a refetch without the token"
#[test]
fn never_follows_a_redirect_a_302_on_the_route_is_the_frames_business_not_a_refetch_without_the_token() {
    drive(|| async {
        let (backend, fx, id) = probe_rig().await;
        let seen: Rc<RefCell<Vec<String>>> = Rc::default();
        let noted = seen.clone();
        let guest = guest_port(move |path| {
            noted.borrow_mut().push(path.to_string());
            if path.starts_with("/app") {
                Reply::of(401, "edge: no token")
            } else {
                Reply { status: 302, headers: vec![("location", "/app".into())], body: vec![], hold: false }
            }
        })
        .await;
        route_to(&backend, guest.url());
        assert_eq!(j(&fx.rt.workspaces().port_probe(&id, 3000, None).await.expect("a probe"))["status"], 302);
        assert_eq!(*seen.borrow(), ["/?pt_token=edge"]);
        fx.rt.close().await.expect("a close");
    });
}

// "reads the body only up to the cap: a response that never ends still answers with its first bytes"
#[test]
fn reads_the_body_only_up_to_the_cap_a_response_that_never_ends_still_answers_with_its_first_bytes() {
    drive(|| async {
        let (backend, fx, id) = probe_rig().await;
        let body = format!("{:x<width$}", "<html>", width = PORT_PROBE_BODY_CAP + 500);
        let guest = guest_port(move |_| Reply {
            status: 200,
            headers: vec![("content-type", "text/html".into())],
            body: body.as_bytes().to_vec(),
            hold: true,
        })
        .await;
        route_to(&backend, guest.url());
        let probe = j(&fx.rt.workspaces().port_probe(&id, 3000, None).await.expect("a probe"));
        assert_eq!(probe["status"], 200);
        let body = probe["body"].as_str().expect("a body");
        assert_eq!(body.len(), PORT_PROBE_BODY_CAP);
        assert!(body.starts_with("<html>"));
        fx.rt.close().await.expect("a close");
    });
}

// "a 401 drops the port's cached route and mints a fresh one, so the next portReach carries the new token"
#[test]
fn a_401_drops_the_ports_cached_route_and_mints_a_fresh_one_so_the_next_port_reach_carries_the_new_token() {
    drive(|| async {
        let (backend, fx, id) = probe_rig().await;
        let guest =
            guest_port(
                |path| if path.ends_with("pt_token=t1") { Reply::of(401, "token expired") } else { Reply::of(200, "<!doctype html>") },
            )
            .await;
        let mints = Rc::new(Cell::new(0u32));
        let (counted, port) = (mints.clone(), guest.port);
        backend.machine(0).set_preview(Some(Rc::new(move |_| {
            counted.set(counted.get() + 1);
            let n = counted.get();
            let reach = wsp_engine::machine::PreviewReach {
                url: format!("http://127.0.0.1:{port}/?pt_token=t{n}"),
                token: format!("t{n}"),
                expires_at: wsp_wire::types::JsNumber((now_ms() + 3_600_000) as f64),
            };
            Box::pin(async move { Ok(reach) })
        })));

        let first = j(&fx.rt.workspaces().port_reach(&id, 3000, None).await.expect("a reach"));
        assert!(first["url"].as_str().expect("a url").contains("pt_token=t1"));
        assert_eq!(j(&fx.rt.workspaces().port_probe(&id, 3000, None).await.expect("a probe"))["status"], 401);
        assert_eq!(mints.get(), 2);
        let next = j(&fx.rt.workspaces().port_reach(&id, 3000, None).await.expect("a reach"));
        assert!(next["url"].as_str().expect("a url").contains("pt_token=t2"));
        assert_eq!(mints.get(), 2);
        assert_eq!(j(&fx.rt.workspaces().port_probe(&id, 3000, None).await.expect("a probe"))["status"], 200);
        assert_eq!(mints.get(), 2);
        fx.rt.close().await.expect("a close");
    });
}

// "rejects when the route cannot be fetched at all, and rejects an unknown workspace"
#[test]
fn rejects_when_the_route_cannot_be_fetched_at_all_and_rejects_an_unknown_workspace() {
    drive(|| async {
        let (backend, fx, id) = probe_rig().await;
        let guest = guest_port(|_| Reply::of(200, "")).await;
        let url = guest.url();
        drop(guest);
        sleep_ms(20).await;
        route_to(&backend, url);
        assert!(fx.rt.workspaces().port_probe(&id, 3000, None).await.is_err());
        let unknown = fx.rt.workspaces().port_probe("ws_nobody", 3000, None).await.expect_err("a refusal");
        assert!(unknown.message.contains("no such workspace"), "{}", unknown.message);
        fx.rt.close().await.expect("a close");
    });
}
