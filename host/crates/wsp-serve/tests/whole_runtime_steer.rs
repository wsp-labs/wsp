// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-steer.test.ts at 65f6b89c7, case for case: create idempotency keys, a session steer,
//! a start on a thread whose turn is running, and what a turn cost on its row. The TS `memoryStore()` is the sqlite
//! store here, which keeps transcript rows. Each test names the TS test above it.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use base64::Engine as _;
use common::stub_backend::{exec_async, exec_ok};
use common::*;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::port::HarnessAdapter;
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::machine_exec::factory::{machine_exec_stream, MachineExecOptions};
use wsp_core::status::StatusWatchOptions;
use wsp_core::types::api::{StartOptions, SteerAsk};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::wiring::GoldenBuildRequest;
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_engine::machine::MachineSpec;
use wsp_store::store::Store;
use wsp_words::exit::HostError;

type SpecLog = Rc<RefCell<Vec<MachineSpec>>>;

fn lost_answer() -> EngineError {
    fetch_failed()
}

/// Routes every create through `plan` first, so a test can lose an answer or hand back a replay.
fn intercept(
    backend: &Arc<StubBackend>,
    plan: impl Fn(
            MachineSpec,
            Rc<dyn Fn(MachineSpec) -> common::stub_backend::Fut<Arc<StubMachine>>>,
        ) -> common::stub_backend::Fut<Arc<StubMachine>>
        + 'static,
) -> SpecLog {
    let original = backend.create_fn();
    let real: Rc<dyn Fn(MachineSpec) -> common::stub_backend::Fut<Arc<StubMachine>>> = Rc::new(move |spec| original(spec, None, None));
    let specs: SpecLog = Rc::default();
    let kept = specs.clone();
    backend.set_create(Rc::new(move |spec, _waiting, _sent| {
        kept.borrow_mut().push(spec.clone());
        plan(spec, real.clone())
    }));
    specs
}

fn key_of(spec: &MachineSpec) -> String {
    spec.idempotency_key.clone().expect("a key")
}

fn is_hex(s: &str, n: usize) -> bool {
    s.len() == n && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

async fn build_golden(fx: &Fx, ask: GoldenBuildRequest) -> Result<(), HostError> {
    fx.rt.golden().build(ask).await.map(|_| ())
}

/// `rt.golden.build({ name, setup: "true", smoke: "true", cpu })`: `GoldenBuildRequest.options` is wsp-core's empty
/// `BuildGoldenOptions` marker (wsp-core/src/types/pending.rs:38), so the setup, the smoke and the size cannot be handed in.
fn golden_ask(name: Option<&str>) -> GoldenBuildRequest {
    GoldenBuildRequest { options: Default::default(), name: name.map(str::to_string) }
}

async fn keys_left(store: &Rc<dyn Store>) -> Vec<Value> {
    store.list("creates").await.expect("a list")
}

// "create idempotency keys" > "a workspace create carries a key made of the workspace id and a nonce, gone from the store once the machine exists"
#[test]
fn a_workspace_create_carries_a_key_made_of_the_workspace_id_and_a_nonce_gone_from_the_store_once_the_machine_exists() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace").view.id;
        let key = key_of(&backend.machine(0).spec_now());
        let (purpose, nonce) = key.rsplit_once(':').expect("a nonce");
        assert_eq!(purpose, format!("workspace/{ws}"));
        assert!(is_hex(nonce, 16), "{key}");
        assert!(keys_left(&state.store).await.is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "create idempotency keys" > "a retry of an attempt the provider never answered sends the same key and the same body"
#[test]
fn a_retry_of_an_attempt_the_provider_never_answered_sends_the_same_key_and_the_same_body() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace").view.id;
        let lose = Rc::new(Cell::new(true));
        let losing = lose.clone();
        let specs = intercept(&backend, move |spec, real| {
            if losing.replace(false) {
                return async { Err(lost_answer()) }.boxed_local();
            }
            real(spec)
        });
        let failed = fx.rt.workspaces().rebuild(&ws, None).await.err().expect("a failure");
        assert!(failed.message.contains("fetch failed"), "{}", failed.message);
        assert_eq!(keys_left(&state.store).await.len(), 1);
        let rebuilt = fx.rt.workspaces().rebuild(&ws, None).await.expect("a rebuild");
        assert_eq!(j(&rebuilt)["machineId"], "m2");
        let specs = specs.borrow();
        assert_eq!(specs.len(), 2);
        assert_eq!(key_of(&specs[1]), key_of(&specs[0]));
        assert_eq!(specs[1].labels.as_ref().and_then(|l| l.get("createdAt")), specs[0].labels.as_ref().and_then(|l| l.get("createdAt")));
        assert_ne!(key_of(&specs[1]), key_of(&backend.machine(0).spec_now()));
        assert!(keys_left(&state.store).await.is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "create idempotency keys" > "a new attempt after a kill, a refusal or a changed request mints a new key"
#[test]
fn a_new_attempt_after_a_kill_a_refusal_or_a_changed_request_mints_a_new_key() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace").view.id;
        fx.rt.workspaces().upgrade(&ws, None).await.expect("an upgrade");
        let (first, second) = (key_of(&backend.machine(0).spec_now()), key_of(&backend.machine(1).spec_now()));
        assert!(backend.machine(0).killed.get());
        assert_ne!(second, first);

        let refuse = Rc::new(Cell::new(true));
        let refusing = refuse.clone();
        let specs = intercept(&backend, move |spec, real| {
            if refusing.replace(false) {
                return async {
                    Err(EngineError::Provider(WspError {
                        kind: ErrorKind::Concurrency,
                        status: 429,
                        code: None,
                        message: "at cap".into(),
                        request_id: None,
                        window: None,
                        said: None,
                    }))
                }
                .boxed_local();
            }
            real(spec)
        });
        let capped = fx.rt.workspaces().rebuild(&ws, None).await.err().expect("a refusal");
        assert!(capped.message.contains("at cap"), "{}", capped.message);
        assert!(keys_left(&state.store).await.is_empty());
        fx.rt.workspaces().rebuild(&ws, None).await.expect("a rebuild");
        assert_ne!(key_of(&specs.borrow()[1]), key_of(&specs.borrow()[0]));

        let lose = Rc::new(Cell::new(true));
        let losing = lose.clone();
        let made = Rc::new(RefCell::new(Vec::<MachineSpec>::new()));
        let kept = made.clone();
        let fresh = StubBackend::new(None);
        backend.set_create(Rc::new(move |spec, waiting, sent| {
            kept.borrow_mut().push(spec.clone());
            if losing.replace(false) {
                return async { Err(lost_answer()) }.boxed_local();
            }
            let fresh = fresh.clone();
            async move { fresh.make(spec, waiting, sent).await }.boxed_local()
        }));
        let built = build_golden(&fx, golden_ask(None)).await.err().expect("a failure");
        assert!(built.message.contains("fetch failed"), "{}", built.message);
        assert_eq!(keys_left(&state.store).await.len(), 1);
        build_golden(&fx, golden_ask(None)).await.expect("a build");
        let made = made.borrow();
        let cpus: Vec<Option<f64>> = made.iter().take(2).map(|s| s.cpu.map(|c| c.0)).collect();
        assert_eq!(cpus, [Some(2.0), Some(4.0)]);
        assert_ne!(key_of(&made[1]), key_of(&made[0]));
        assert!(keys_left(&state.store).await.is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "create idempotency keys" > "a create the backend gave up on because nothing on the guest could run spends its key, so the next attempt never sends the deleted box's key again"
#[test]
fn a_create_the_backend_gave_up_on_because_nothing_on_the_guest_could_run_spends_its_key_so_the_next_attempt_never_sends_the_deleted_boxs_key_again(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace").view.id;
        let dead = Rc::new(Cell::new(true));
        let dying = dead.clone();
        let specs = intercept(&backend, move |spec, real| {
            if dying.replace(false) {
                // The backend deleted the box it could not use before throwing; a key that outlived it would name it again.
                return async {
                    Err(EngineError::guest_unusable(
                        "bx_dead",
                        "Boat",
                        "bash: error while loading shared libraries: libtinfo.so.6: cannot open shared object file: Error 24",
                        200,
                    ))
                }
                .boxed_local();
            }
            real(spec)
        });
        let refused = fx.rt.workspaces().rebuild(&ws, None).await.err().expect("a refusal");
        assert!(refused.message.contains("nothing on it can run"), "{}", refused.message);
        assert!(keys_left(&state.store).await.is_empty());
        fx.rt.workspaces().rebuild(&ws, None).await.expect("a rebuild");
        assert_ne!(key_of(&specs.borrow()[1]), key_of(&specs.borrow()[0]));
        fx.rt.close().await.expect("a close");
    });
}

// "create idempotency keys" > "a replayed create is logged, and a replay naming a dead machine is created anew under a fresh key"
#[test]
fn a_replayed_create_is_logged_and_a_replay_naming_a_dead_machine_is_created_anew_under_a_fresh_key() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let warn = capture();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace").view.id;
        let specs: Rc<RefCell<Vec<MachineSpec>>> = Rc::default();
        let kept = specs.clone();
        let original = backend.create_fn();
        backend.set_create(Rc::new(move |spec, waiting, sent| {
            kept.borrow_mut().push(spec.clone());
            let count = kept.borrow().len();
            let original = original.clone();
            async move {
                let m = original(spec, waiting, sent).await?;
                if count == 1 {
                    m.killed.set(true);
                }
                m.replayed.set(count <= 2);
                Ok(m)
            }
            .boxed_local()
        }));
        let rebuilt = fx.rt.workspaces().rebuild(&ws, None).await.expect("a rebuild");
        assert_eq!(j(&rebuilt)["machineId"], "m3");
        assert_eq!(specs.borrow().len(), 2);
        assert_ne!(key_of(&specs.borrow()[1]), key_of(&specs.borrow()[0]));
        let notes = warn.lines();
        assert!(notes.iter().any(|n| n.contains("m2") && n.contains("gone")), "{notes:?}");
        assert!(notes.iter().any(|n| n.contains("m3") && n.contains("replayed")), "{notes:?}");
        fx.rt.close().await.expect("a close");
    });
}

// "create idempotency keys" > "adopting a dead process's attempt takes the key and rewrites the entry to the adopter"
#[test]
fn adopting_a_dead_processs_attempt_takes_the_key_and_rewrites_the_entry_to_the_adopter() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace").view.id;
        let losses = Rc::new(Cell::new(2));
        let losing = losses.clone();
        let specs = intercept(&backend, move |spec, real| {
            if losing.get() > 0 {
                losing.set(losing.get() - 1);
                return async { Err(lost_answer()) }.boxed_local();
            }
            real(spec)
        });
        let failed = fx.rt.workspaces().rebuild(&ws, None).await.err().expect("a failure");
        assert!(failed.message.contains("fetch failed"), "{}", failed.message);
        let purpose = format!("workspace/{ws}");
        let left = state.store.get("creates", &purpose).await.expect("a read").expect("an entry");
        assert_eq!(left["pid"], std::process::id());
        let dead_pid = 99_999_999;
        let mut moved = left.clone();
        moved["pid"] = json!(dead_pid);
        state.store.put("creates", &purpose, moved).await.expect("a write");
        let failed = fx.rt.workspaces().rebuild(&ws, None).await.err().expect("a failure");
        assert!(failed.message.contains("fetch failed"), "{}", failed.message);
        let adopted = state.store.get("creates", &purpose).await.expect("a read").expect("an entry");
        assert_matches(&adopted, json!({ "key": left["key"], "pid": std::process::id() }));
        fx.rt.workspaces().rebuild(&ws, None).await.expect("a rebuild");
        let keys: Vec<String> = specs.borrow().iter().map(key_of).collect();
        let key = left["key"].as_str().expect("a key").to_string();
        assert_eq!(keys, [key.clone(), key.clone(), key]);
        assert!(keys_left(&state.store).await.is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "create idempotency keys" > "a purpose too long for the provider's 255-character key cap is hashed, the nonce kept"
#[test]
fn a_purpose_too_long_for_the_providers_255_character_key_cap_is_hashed_the_nonce_kept() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        build_golden(&fx, golden_ask(Some(&"g".repeat(300)))).await.expect("a build");
        let key = key_of(&backend.machine(0).spec_now());
        assert!(key.len() <= 255, "{}", key.len());
        let (hash, nonce) = key.rsplit_once(':').expect("a nonce");
        assert!(is_hex(hash, 64) && is_hex(nonce, 16), "{key}");
        fx.rt.close().await.expect("a close");
    });
}

// "create idempotency keys" > "a builder and its smoke fork carry keys for the golden they build"
#[test]
fn a_builder_and_its_smoke_fork_carry_keys_for_the_golden_they_build() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        build_golden(&fx, golden_ask(None)).await.expect("a build");
        let keys: Vec<String> = backend.machines().iter().map(|m| key_of(&m.spec_now())).collect();
        assert_eq!(keys.len(), 2);
        for key in &keys {
            let (purpose, nonce) = key.rsplit_once(':').expect("a nonce");
            assert_eq!(purpose, "golden/default");
            assert!(is_hex(nonce, 16), "{key}");
        }
        assert_ne!(keys[0], keys[1]);
        fx.rt.close().await.expect("a close");
    });
}

const SID: &str = "66666666-6666-4666-8666-666666666666";

/// A harness whose turn runs until the test ends it; steer records the prompt and answers as told, or is absent.
fn steerable(steers: bool) -> Rc<Live> {
    Live::new(LiveOptions { steers, announces: false, session: Some(SID.into()), ..Default::default() })
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn on_thread(prompt: &str, thread: &str, request_id: Option<&str>) -> StartOptions {
    StartOptions { thread: Some(thread.into()), request_id: request_id.map(str::to_string), ..start(prompt) }
}

fn steer(prompt: &str, request_id: Option<&str>) -> SteerAsk {
    SteerAsk { prompt: prompt.into(), request_id: request_id.map(str::to_string), attachments: None }
}

async fn steered(fx: &Fx, id: &str, ask: SteerAsk) -> Value {
    j(&fx.rt.sessions().steer(id, ask, None).await.expect("a steer"))
}

async fn rows(fx: &Fx, ws: &str) -> Vec<Value> {
    fx.rt.sessions().list(Some(ws.to_string()), None).await.expect("a list").iter().map(j).collect()
}

async fn types_of(fx: &Fx, ws: &str) -> Vec<String> {
    fx.rt
        .sessions()
        .history(ws, None)
        .await
        .expect("a history")
        .iter()
        .map(|e| j(e)["type"].as_str().unwrap_or_default().to_string())
        .collect()
}

async fn one_workspace(live: &Rc<Live>) -> (Fx, String, Arc<StubBackend>) {
    let backend = StubBackend::new(None);
    let fx = runtime(&backend).adapter("claude", live.factory()).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    (fx, ws, backend)
}

// "runtime session steer" > "accepted: the adapter takes the line and the runtime records session.steer under the turn's scope with the prompt and the request id; no session.start comes with it"
#[test]
fn accepted_the_adapter_takes_the_line_and_the_runtime_records_session_steer_under_the_turns_scope_with_the_prompt_and_the_request_id_no_session_start_comes_with_it(
) {
    drive(|| async {
        let h = steerable(true);
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).adapter("claude", h.factory()).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let handle =
            fx.rt.sessions().start(&ws, StartOptions { request_id: Some("req_1".into()), ..start("go") }, None).await.expect("a start");
        h.init(0);
        assert_eq!(steered(&fx, &handle.id(), steer("and say pineapple", Some("req_2"))).await, json!({ "outcome": "accepted" }));
        assert_eq!(h.steered.borrow().iter().map(|s| s.prompt.clone()).collect::<Vec<_>>(), ["and say pineapple"]);
        let start_event = events.of("session.start").remove(0);
        let steers = events.of("session.steer");
        assert_eq!(steers.len(), 1);
        assert_matches(
            &steers[0],
            json!({
                "type": "session.steer", "workspaceId": ws, "sessionId": SID, "turnId": start_event["turnId"],
                "threadId": start_event["threadId"], "prompt": "and say pineapple", "requestId": "req_2",
            }),
        );
        assert!(steers[0]["at"].is_number());
        assert!(!start_event["turnId"].is_null());
        assert!(!start_event["threadId"].is_null());
        assert_eq!(events.of("session.start").len(), 1);
        assert_eq!(rows(&fx, &ws).await[0]["status"], "running");
        h.end(0, "ok");
        handle.finished().await;
        assert_eq!(types_of(&fx, &ws).await, ["session.start", "session.steer", "session.done", "session.end"]);
        let history = fx.rt.sessions().history(&ws, None).await.expect("a history");
        assert_matches(&j(&history[1]), json!({ "type": "session.steer", "prompt": "and say pineapple" }));
        fx.rt.close().await.expect("a close");
    });
}

// "runtime session steer" > "marks the steer waiting when the turn it joined is stopped on a prompt nobody has answered, and leaves it off one that is working"
#[test]
fn marks_the_steer_waiting_when_the_turn_it_joined_is_stopped_on_a_prompt_nobody_has_answered_and_leaves_it_off_one_that_is_working() {
    drive(|| async {
        let h = steerable(true);
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).adapter("claude", h.factory()).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let handle = fx.rt.sessions().start(&ws, start("write it"), None).await.expect("a start");
        h.init(0);
        steered(&fx, &handle.id(), steer("hurry", None)).await;
        h.emit(
            0,
            json!({
                "type": "permission.ask", "sessionId": SID,
                "ask": { "askId": "a1", "toolName": "Write", "input": json!({ "file_path": "kai.txt" }).to_string(), "options": [{ "id": "allow", "label": "Yes", "effect": "allow" }] },
            }),
        );
        until(|| {
            let (fx, ws) = (fx.rt.clone(), ws.clone());
            async move { fx.sessions().list(Some(ws), None).await.expect("a list").first().is_some_and(|r| j(r).get("asking").is_some()) }
        })
        .await;
        steered(&fx, &handle.id(), steer("yes", None)).await;
        let steers: Vec<(Value, Value)> = events.of("session.steer").iter().map(|e| (e["prompt"].clone(), e["waiting"].clone())).collect();
        assert_eq!(steers, [(json!("hurry"), Value::Null), (json!("yes"), json!(true))]);
        h.end(0, "ok");
        handle.finished().await;
        fx.rt.close().await.expect("a close");
    });
}

// "runtime session steer" > "not-found for a session this runtime does not hold"
#[test]
fn not_found_for_a_session_this_runtime_does_not_hold() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).adapter("claude", steerable(true).factory()).build();
        assert_eq!(steered(&fx, "nope", steer("x", None)).await, json!({ "outcome": "not-found" }));
    });
}

// "runtime session steer" > "not-running once the turn ended, and when the adapter says the line missed the turn; nothing is recorded either way"
#[test]
fn not_running_once_the_turn_ended_and_when_the_adapter_says_the_line_missed_the_turn_nothing_is_recorded_either_way() {
    drive(|| async {
        let late = steerable(true);
        late.answer.set(false);
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).adapter("claude", late.factory()).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let handle = fx.rt.sessions().start(&ws, start("go"), None).await.expect("a start");
        late.init(0);
        assert_eq!(steered(&fx, &handle.id(), steer("racing", None)).await, json!({ "outcome": "not-running" }));
        assert_eq!(late.steered.borrow().iter().map(|s| s.prompt.clone()).collect::<Vec<_>>(), ["racing"]);
        late.end(0, "ok");
        handle.finished().await;
        assert_eq!(steered(&fx, &handle.id(), steer("too late", None)).await, json!({ "outcome": "not-running" }));
        assert_eq!(late.steered.borrow().iter().map(|s| s.prompt.clone()).collect::<Vec<_>>(), ["racing"]);
        assert!(!events.has("session.steer"));
        fx.rt.close().await.expect("a close");
    });
}

/// `{"type":"system","subtype":"init","session_id":"<SID>"}\n`
fn init_line() -> Vec<u8> {
    format!("{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"{SID}\"}}\n").into_bytes()
}

/// `cmd.match(/(__WSP_EOF_[a-z0-9]+__)/)?.[1]`
fn sentinel_of(cmd: &str) -> Option<String> {
    let at = cmd.find("__WSP_EOF_")?;
    let rest = &cmd[at + "__WSP_EOF_".len()..];
    let word: String = rest.chars().take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect();
    (!word.is_empty() && rest[word.len()..].starts_with("__")).then(|| format!("__WSP_EOF_{word}__"))
}

/// `Number(cmd.match(/tail -c \+(\d+)/)?.[1] ?? "1") - 1`
fn tail_from(cmd: &str) -> usize {
    let digits: String = cmd
        .find("tail -c +")
        .map(|at| cmd[at + "tail -c +".len()..].chars().take_while(char::is_ascii_digit).collect())
        .unwrap_or_default();
    digits.parse::<usize>().unwrap_or(1) - 1
}

/// The log a fake machine's run reads back, and the file that says the CLI ended.
#[derive(Default)]
struct FakeRun {
    log: RefCell<Vec<u8>>,
    exit_file: RefCell<String>,
    appends: RefCell<Vec<String>>,
}

/// What the machine answers a run's command with, in the order the TS closure tries them; `None` falls through to the stub.
fn run_answer(run: &FakeRun, cmd: &str, written: &'static str, gone: Option<&'static str>) -> Option<wsp_engine::machine::ExecResult> {
    if cmd.contains("WSP_LAUNCHED") {
        return Some(exec_ok("WSP_LAUNCHED\n"));
    }
    if let Some(sentinel) = sentinel_of(cmd) {
        let from = tail_from(cmd);
        let log = run.log.borrow();
        let tail = if from >= log.len() { &[][..] } else { &log[from..] };
        let exit = run.exit_file.borrow().clone();
        let up = if exit.is_empty() { "up" } else { "down" };
        return Some(exec_ok(&format!("{}\n{sentinel} {exit} {up}\n", base64::engine::general_purpose::STANDARD.encode(tail))));
    }
    if cmd.contains("base64 -d >> ") {
        run.appends.borrow_mut().push(cmd.to_string());
        let said = if run.exit_file.borrow().is_empty() { written } else { gone.unwrap_or(written) };
        return Some(exec_ok(&format!("{said}\n")));
    }
    None
}

/// `ctx => createClaudeAdapter({ exec: machineExecStream(ctx.machine, { pollMs }), configDir: "/root/.claude-cfg" })`
fn claude_over_machine(poll_ms: u64) -> HarnessAdapterFactory {
    Arc::new(move |ctx| {
        let exec = machine_exec_stream(ctx.machine.clone(), MachineExecOptions { poll_ms: Some(poll_ms), ..Default::default() }, None);
        let adapter = create_claude_adapter(AdapterDeps::new(exec, "/root/.claude-cfg")).map_err(|e| HostError::new(e.0))?;
        Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
    })
}

fn fake_run(backend: &Arc<StubBackend>, run: &Rc<FakeRun>, gone: Option<&'static str>) {
    let plain = backend.exec_impl_fn();
    let run = run.clone();
    backend.set_exec_impl(exec_async(move |m, cmd| {
        if let Some(answer) = run_answer(&run, &cmd, "WSP_OK", gone) {
            return async move { Ok(answer) }.boxed_local();
        }
        plain(m, cmd, None)
    }));
}

// "runtime session steer" > "over the claude adapter on a fake machine: a steer after the CLI exited, before the poll saw it, answers not-running, appends nothing and records nothing"
#[test]
fn over_the_claude_adapter_on_a_fake_machine_a_steer_after_the_cli_exited_before_the_poll_saw_it_answers_not_running_appends_nothing_and_records_nothing(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let run = Rc::new(FakeRun::default());
        *run.log.borrow_mut() = init_line();
        fake_run(&backend, &run, Some("WSP_GONE"));
        let fx = runtime(&backend).adapter("claude", claude_over_machine(200)).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let handle = fx.rt.sessions().start(&ws, start("go"), None).await.expect("a start");
        until(|| {
            let events = events.clone();
            async move { events.has("session.start") }
        })
        .await;
        *run.exit_file.borrow_mut() = "1".into();
        assert_eq!(steered(&fx, &handle.id(), steer("after exit", None)).await, json!({ "outcome": "not-running" }));
        assert_eq!(run.appends.borrow().len(), 1);
        assert!(!events.has("session.steer"));
        assert_eq!(j(&handle.finished().await)["status"], "failed");
        assert!(!types_of(&fx, &ws).await.iter().any(|t| t == "session.steer"));
        fx.rt.close().await.expect("a close");
    });
}

// "runtime session steer" > "over the claude adapter on a fake machine: a send while the CLI has not announced itself yet joins the turn once it does, and is written after the init"
#[test]
fn over_the_claude_adapter_on_a_fake_machine_a_send_while_the_cli_has_not_announced_itself_yet_joins_the_turn_once_it_does_and_is_written_after_the_init(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let run = Rc::new(FakeRun::default());
        fake_run(&backend, &run, None);
        let fx = runtime(&backend).adapter("claude", claude_over_machine(20)).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let first = fx.rt.sessions().start(&ws, start("go"), None).await.expect("a start");
        let thread_id = first.view().thread_id.expect("a thread");
        let joined: Rc<RefCell<Option<Rc<dyn wsp_core::types::wiring::SessionHandle>>>> = Rc::default();
        let slot = joined.clone();
        let rt = fx.rt.clone();
        let (w, t) = (ws.clone(), thread_id.clone());
        tokio::task::spawn_local(async move {
            let s = rt.sessions().start(&w, on_thread("also this", &t, Some("req_2")), None).await.expect("a start");
            *slot.borrow_mut() = Some(s);
        });
        settle().await;
        assert!(joined.borrow().is_none());
        assert_eq!(run.appends.borrow().len(), 0);
        *run.log.borrow_mut() = init_line();
        until_for(3000, &mut || {
            let joined = joined.clone();
            async move { joined.borrow().as_ref().is_some_and(|s| j(&s.outcome()) == "steered") }
        })
        .await;
        let joined = joined.borrow().clone().expect("the joined start");
        assert_eq!(joined.turn_id(), first.turn_id());
        let steers = events.of("session.steer");
        assert_eq!(steers.len(), 1);
        assert_matches(
            &steers[0],
            json!({ "turnId": first.turn_id(), "threadId": thread_id, "prompt": "also this", "requestId": "req_2" }),
        );
        assert_eq!(run.appends.borrow().len(), 1);
        *run.exit_file.borrow_mut() = "0".into();
        first.finished().await;
        fx.rt.close().await.expect("a close");
    });
}

// "runtime session steer" > "over the claude adapter on a fake machine: a send while the turn is still on its way to the machine joins it once the CLI announces itself, and says no wait behind it"
#[test]
fn over_the_claude_adapter_on_a_fake_machine_a_send_while_the_turn_is_still_on_its_way_to_the_machine_joins_it_once_the_cli_announces_itself_and_says_no_wait_behind_it(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let plain = backend.exec_impl_fn();
        let run = Rc::new(FakeRun::default());
        let hold: Rc<RefCell<Option<Rc<Gate>>>> = Rc::default();
        {
            let (run, hold) = (run.clone(), hold.clone());
            backend.set_exec_impl(exec_async(move |m, cmd| {
                if let Some(answer) = run_answer(&run, &cmd, "WSP_OK", None) {
                    return async move { Ok(answer) }.boxed_local();
                }
                // The first start's look at its folder is held, so the second send meets a turn still on its way to the machine.
                let gate = if cmd.contains("rev-parse") { hold.borrow_mut().take() } else { None };
                let plain = plain.clone();
                async move {
                    if let Some(gate) = gate {
                        gate.wait().await;
                    }
                    plain(m, cmd, None).await
                }
                .boxed_local()
            }));
        }
        let fx = runtime(&backend).adapter("claude", claude_over_machine(20)).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let release = Gate::new();
        *hold.borrow_mut() = Some(release.clone());
        let launching = {
            let (rt, ws) = (fx.rt.clone(), ws.clone());
            tokio::task::spawn_local(async move { rt.sessions().start(&ws, start("go"), None).await.expect("a start") })
        };
        settle().await;
        let thread_id = rows(&fx, &ws).await.into_iter().find(|r| r["status"] == "running").expect("a running row")["threadId"]
            .as_str()
            .expect("a thread")
            .to_string();
        let joined: Rc<RefCell<Option<Rc<dyn wsp_core::types::wiring::SessionHandle>>>> = Rc::default();
        {
            let (slot, rt, w, t) = (joined.clone(), fx.rt.clone(), ws.clone(), thread_id.clone());
            tokio::task::spawn_local(async move {
                let s = rt.sessions().start(&w, on_thread("also this", &t, Some("req_2")), None).await.expect("a start");
                *slot.borrow_mut() = Some(s);
            });
        }
        settle().await;
        release.open();
        let first = launching.await.expect("the first start");
        settle().await;
        assert!(joined.borrow().is_none());
        *run.log.borrow_mut() = init_line();
        until_for(3000, &mut || {
            let joined = joined.clone();
            async move { joined.borrow().as_ref().is_some_and(|s| j(&s.outcome()) == "steered") }
        })
        .await;
        let joined = joined.borrow().clone().expect("the joined start");
        assert_eq!(joined.turn_id(), first.turn_id());
        assert!(!events.has("session.queued"));
        *run.exit_file.borrow_mut() = "0".into();
        first.finished().await;
        fx.rt.close().await.expect("a close");
    });
}

// "runtime session steer" > "unsupported when the session's harness takes no message mid-turn; the catalog says so before the turn runs"
#[test]
fn unsupported_when_the_sessions_harness_takes_no_message_mid_turn_the_catalog_says_so_before_the_turn_runs() {
    drive(|| async {
        let plain = steerable(false);
        let (fx, ws, _backend) = one_workspace(&plain).await;
        let catalog = fx.rt.harnesses().list(Some(ws.clone()), None, None).await.expect("a list");
        assert_eq!(catalog.iter().map(j).find(|c| c["harness"] == "claude").expect("claude")["steers"], false);
        let handle = fx.rt.sessions().start(&ws, start("go"), None).await.expect("a start");
        plain.init(0);
        assert_eq!(steered(&fx, &handle.id(), steer("x", None)).await, json!({ "outcome": "unsupported" }));
        assert!(!types_of(&fx, &ws).await.iter().any(|t| t == "session.steer"));
        plain.end(0, "ok");
        handle.finished().await;
        fx.rt.close().await.expect("a close");
    });
}

// "runtime session steer" > "harnesses.list carries steers from the adapter on a workspace; the table alone, with no machine to ask, says false"
#[test]
fn harnesses_list_carries_steers_from_the_adapter_on_a_workspace_the_table_alone_with_no_machine_to_ask_says_false() {
    drive(|| async {
        let (fx, ws, _backend) = one_workspace(&steerable(true)).await;
        let on = fx.rt.harnesses().list(Some(ws), None, None).await.expect("a list");
        assert_eq!(on.iter().map(j).find(|c| c["harness"] == "claude").expect("claude")["steers"], true);
        let table = fx.rt.harnesses().list(None, None, None).await.expect("a list");
        assert_eq!(table.iter().map(j).find(|c| c["harness"] == "claude").expect("claude")["steers"], false);
        fx.rt.close().await.expect("a close");
    });
}

// "runtime session steer" > "a steer inside a turn keeps the idle hold: the workspace stays awake through the window and naps after the turn ends"
#[test]
fn a_steer_inside_a_turn_keeps_the_idle_hold_the_workspace_stays_awake_through_the_window_and_naps_after_the_turn_ends() {
    drive(|| async {
        const WINDOW: i64 = 5 * 60_000;
        let h = steerable(true);
        let fc = FakeClock::new(1_788_851_404_444);
        let backend = StubBackend::new(None);
        let fx = runtime(&backend)
            .clock(&fc)
            .adapter("claude", h.factory())
            .with(|o| {
                o.idle_default_window_ms = Some(WINDOW as u64);
                o.status =
                    Some(StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(60_000), ..Default::default() });
            })
            .build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let handle = fx.rt.sessions().start(&ws, start("go"), None).await.expect("a start");
        h.init(0);
        fc.advance(WINDOW * 2);
        assert_eq!(steered(&fx, &handle.id(), steer("more", None)).await, json!({ "outcome": "accepted" }));
        fc.advance(WINDOW * 2);
        assert_eq!(j(&fx.rt.workspaces().get(&ws, None, None).await.expect("a workspace"))["phase"], "running");
        h.end(0, "ok");
        handle.finished().await;
        sleep_ms(20).await;
        fc.advance(WINDOW);
        until(|| {
            let (rt, ws) = (fx.rt.clone(), ws.clone());
            async move { j(&rt.workspaces().get(&ws, None, None).await.expect("a workspace"))["phase"] == "napping" }
        })
        .await;
        fx.rt.close().await.expect("a close");
    });
}

// "runtime session steer" > "refuses while the workspace is pausing and once it is paused, with the composer's sentence, as start does"
#[test]
fn refuses_while_the_workspace_is_pausing_and_once_it_is_paused_with_the_composers_sentence_as_start_does() {
    drive(|| async {
        let h = steerable(true);
        let (fx, ws, backend) = one_workspace(&h).await;
        let handle = fx.rt.sessions().start(&ws, start("go"), None).await.expect("a start");
        h.init(0);
        let m = backend.machine(0);
        let release = Gate::new();
        let pause = m.pause_fn();
        let gate = release.clone();
        m.set_pause(Rc::new(move || {
            let (gate, pause) = (gate.clone(), pause.clone());
            async move {
                gate.wait().await;
                pause().await
            }
            .boxed_local()
        }));
        let napping = {
            let (rt, ws) = (fx.rt.clone(), ws.clone());
            tokio::task::spawn_local(async move { rt.workspaces().nap(&ws, None).await })
        };
        until(|| {
            let (rt, ws) = (fx.rt.clone(), ws.clone());
            async move { j(&rt.workspaces().get(&ws, None, None).await.expect("a workspace"))["phase"] == "pausing" }
        })
        .await;
        let pausing = fx.rt.sessions().steer(&handle.id(), steer("x", None), None).await.err().expect("a refusal");
        assert!(pausing.message.contains("Workspace is pausing; wake it to send"), "{}", pausing.message);
        assert!(h.steered.borrow().is_empty());
        release.open();
        napping.await.expect("the nap").expect("a nap");
        let paused = fx.rt.sessions().steer(&handle.id(), steer("x", None), None).await.err().expect("a refusal");
        assert!(paused.message.contains("Workspace is paused; wake it to send"), "{}", paused.message);
        fx.rt.close().await.expect("a close");
    });
}

async fn held_workspace(steers: bool) -> (Fx, String, Rc<Held>) {
    let h = Held::new(steers);
    let backend = StubBackend::new(None);
    let fx = runtime(&backend).adapter("claude", h.factory()).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    (fx, ws, h)
}

fn held_prompts(h: &Held) -> Vec<String> {
    h.starts.borrow().iter().map(|s| s.prompt.clone()).collect()
}

type Handle = Rc<dyn wsp_core::types::wiring::SessionHandle>;

/// `rt.sessions.start(...).then(s => outcomes.push(...))`: a start a test leaves pending, its handle kept when it answers.
fn pending_start(fx: &Fx, ws: &str, opts: StartOptions) -> Rc<RefCell<Option<Handle>>> {
    let slot: Rc<RefCell<Option<Handle>>> = Rc::default();
    let (kept, rt, ws) = (slot.clone(), fx.rt.clone(), ws.to_string());
    tokio::task::spawn_local(async move {
        let s = rt.sessions().start(&ws, opts, None).await.expect("a start");
        *kept.borrow_mut() = Some(s);
    });
    slot
}

async fn answered(slot: &Rc<RefCell<Option<Handle>>>) -> Handle {
    until(|| {
        let slot = slot.clone();
        async move { slot.borrow().is_some() }
    })
    .await;
    let handle = slot.borrow().clone().expect("a handle");
    handle
}

// "a start on a thread whose turn is running" > "on a harness that steers, the start becomes a steer: session.steer is recorded with the request id, no second session.start, and the caller holds the running turn"
#[test]
fn on_a_harness_that_steers_the_start_becomes_a_steer_session_steer_is_recorded_with_the_request_id_no_second_session_start_and_the_caller_holds_the_running_turn(
) {
    drive(|| async {
        let (fx, ws, h) = held_workspace(true).await;
        let events = fx.events();
        let first = fx
            .rt
            .sessions()
            .start(&ws, StartOptions { request_id: Some("req_1".into()), ..start("loop for a minute") }, None)
            .await
            .expect("a start");
        assert_eq!(j(&first.outcome()), "started");
        let view = j(&first.view());
        let sid = view["claudeSessionId"].as_str().expect("a session").to_string();
        let thread_id = view["threadId"].as_str().expect("a thread").to_string();
        let opts = StartOptions {
            started_by: Some(wsp_wire::views::session::SessionOrigin::Cli),
            ..on_thread("end with STEERED", &thread_id, Some("req_2"))
        };
        let joined = fx.rt.sessions().start(&ws, opts, None).await.expect("a start");
        assert_eq!(j(&joined.outcome()), "steered");
        assert_eq!(joined.id(), first.id());
        assert_eq!(joined.turn_id(), first.turn_id());
        assert_matches(&j(&joined.view()), json!({ "threadId": thread_id, "status": "running", "prompt": "loop for a minute" }));
        assert_eq!(h.starts.borrow().len(), 1);
        assert_eq!(*h.steered.borrow(), ["end with STEERED"]);
        assert_eq!(events.of("session.start").len(), 1);
        assert!(!events.has("session.queued"));
        let steers = events.of("session.steer");
        assert_eq!(steers.len(), 1);
        assert_matches(
            &steers[0],
            json!({ "sessionId": sid, "turnId": first.turn_id(), "threadId": thread_id, "prompt": "end with STEERED", "requestId": "req_2" }),
        );
        assert_eq!(rows(&fx, &ws).await.len(), 1);
        h.end(0, "done STEERED", json!({}));
        assert_eq!(j(&joined.finished().await), json!({ "status": "completed", "text": "done STEERED" }));
        assert_eq!(types_of(&fx, &ws).await, ["session.start", "session.steer", "session.done", "session.end"]);
        fx.rt.close().await.expect("a close");
    });
}

// "a start on a thread whose turn is running" > "on a harness that cannot steer, the start waits for the running turn and follows its done: one turn at a time on the session"
#[test]
fn on_a_harness_that_cannot_steer_the_start_waits_for_the_running_turn_and_follows_its_done_one_turn_at_a_time_on_the_session() {
    drive(|| async {
        let (fx, ws, h) = held_workspace(false).await;
        let events = fx.events();
        let first = fx.rt.sessions().start(&ws, start("one"), None).await.expect("a start");
        let view = j(&first.view());
        let sid = view["claudeSessionId"].as_str().expect("a session").to_string();
        let thread_id = view["threadId"].as_str().expect("a thread").to_string();
        let second = pending_start(&fx, &ws, on_thread("two", &thread_id, Some("req_2")));
        settle().await;
        assert_eq!(held_prompts(&h), ["one"]);
        assert!(second.borrow().is_none());
        // The wait is announced at once, so a caller can say it is waiting before the reply comes.
        let queued = events.of("session.queued");
        assert_eq!(queued.len(), 1);
        assert!(queued[0]["seq"].is_number());
        let mut said = queued[0].clone();
        said.as_object_mut().expect("an object").remove("seq");
        assert_eq!(
            said,
            json!({ "type": "session.queued", "workspaceId": ws, "threadId": thread_id, "harness": "claude", "prompt": "two", "requestId": "req_2" })
        );
        h.end(0, "one done", json!({}));
        let second = answered(&second).await;
        assert_eq!(j(&second.outcome()), "queued");
        assert_ne!(second.turn_id(), first.turn_id());
        let starts: Vec<(String, Option<String>)> = h.starts.borrow().iter().map(|s| (s.prompt.clone(), s.resume.clone())).collect();
        assert_eq!(starts, [("one".to_string(), None), ("two".to_string(), Some(sid))]);
        assert_matches(&j(&second.view()), json!({ "status": "running", "prompt": "one", "threadId": thread_id }));
        h.end(1, "two done", json!({}));
        assert_eq!(j(&second.finished().await), json!({ "status": "completed", "text": "two done" }));
        assert_eq!(
            types_of(&fx, &ws).await,
            ["session.start", "session.done", "session.end", "session.start", "session.done", "session.end"]
        );
        let history: Vec<Value> = fx.rt.sessions().history(&ws, None).await.expect("a history").iter().map(j).collect();
        assert_matches(
            &history[3],
            json!({ "type": "session.start", "prompt": "two", "requestId": "req_2", "threadId": thread_id, "turnId": second.turn_id() }),
        );
        assert!(!history.iter().any(|e| e["type"] == "session.steer"));
        assert_eq!(events.of("session.queued").len(), 1);
        fx.rt.close().await.expect("a close");
    });
}

// "a start on a thread whose turn is running" > "two sends queued behind one turn run in order, each after the one before it ended"
#[test]
fn two_sends_queued_behind_one_turn_run_in_order_each_after_the_one_before_it_ended() {
    drive(|| async {
        let (fx, ws, h) = held_workspace(false).await;
        let events = fx.events();
        let thread = fx.rt.sessions().start(&ws, start("one"), None).await.expect("a start").view().thread_id.expect("a thread");
        let outcomes: Rc<RefCell<Vec<String>>> = Rc::default();
        let mut waits = Vec::new();
        for prompt in ["two", "three"] {
            let (rt, ws, thread, outcomes) = (fx.rt.clone(), ws.clone(), thread.clone(), outcomes.clone());
            waits.push(tokio::task::spawn_local(async move {
                let s = rt.sessions().start(&ws, on_thread(prompt, &thread, None), None).await.expect("a start");
                outcomes.borrow_mut().push(format!("{prompt}:{}", j(&s.outcome()).as_str().unwrap_or_default()));
            }));
        }
        settle().await;
        assert_eq!(held_prompts(&h), ["one"]);
        let queued: Vec<String> =
            events.of("session.queued").iter().map(|e| e["prompt"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(queued, ["two", "three"]);
        h.end(0, "one done", json!({}));
        waits.remove(0).await.expect("the second send");
        settle().await;
        assert_eq!(held_prompts(&h), ["one", "two"]);
        assert_eq!(*outcomes.borrow(), ["two:queued"]);
        h.end(1, "two done", json!({}));
        waits.remove(0).await.expect("the third send");
        assert_eq!(held_prompts(&h), ["one", "two", "three"]);
        assert_eq!(*outcomes.borrow(), ["two:queued", "three:queued"]);
        h.end(2, "three done", json!({}));
        // Each waiter says so once, when it first waits; the second wait of the third send is not announced again.
        assert_eq!(events.of("session.queued").len(), 2);
        let starts: Vec<String> = fx
            .rt
            .sessions()
            .history(&ws, None)
            .await
            .expect("a history")
            .iter()
            .map(j)
            .filter(|e| e["type"] == "session.start")
            .map(|e| e["prompt"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(starts, ["one", "two", "three"]);
        fx.rt.close().await.expect("a close");
    });
}

// "a start on a thread whose turn is running" > "a send that meets the reply tail queues instead of being refused, and two of them keep their order"
#[test]
fn a_send_that_meets_the_reply_tail_queues_instead_of_being_refused_and_two_of_them_keep_their_order() {
    drive(|| async {
        let (fx, ws, h) = held_workspace(false).await;
        let first = fx.rt.sessions().start(&ws, start("one"), None).await.expect("a start");
        let thread_id = first.view().thread_id.expect("a thread");
        // The turn has answered and its process has not exited: the row still reads running.
        h.reply(0, "answered", json!({}));
        assert_eq!(j(&first.view())["status"], "running");
        let outcomes: Rc<RefCell<Vec<String>>> = Rc::default();
        let mut waits = Vec::new();
        for prompt in ["two", "three"] {
            let (rt, ws, thread, outcomes) = (fx.rt.clone(), ws.clone(), thread_id.clone(), outcomes.clone());
            waits.push(tokio::task::spawn_local(async move {
                let s = rt.sessions().start(&ws, on_thread(prompt, &thread, None), None).await.expect("a start");
                outcomes.borrow_mut().push(format!("{prompt}:{}", j(&s.outcome()).as_str().unwrap_or_default()));
            }));
        }
        settle().await;
        // Neither was refused, and neither opened a second agent on the thread while the first process lived.
        assert_eq!(held_prompts(&h), ["one"]);
        h.exit(0);
        waits.remove(0).await.expect("the second send");
        h.end(1, "two done", json!({}));
        waits.remove(0).await.expect("the third send");
        assert_eq!(held_prompts(&h), ["one", "two", "three"]);
        assert_eq!(*outcomes.borrow(), ["two:queued", "three:queued"]);
        h.end(2, "three done", json!({}));
        fx.rt.close().await.expect("a close");
    });
}

// "a start on a thread whose turn is running" > "a harness that takes a message mid-turn takes none once its turn has answered: that send queues for the next turn too"
#[test]
fn a_harness_that_takes_a_message_mid_turn_takes_none_once_its_turn_has_answered_that_send_queues_for_the_next_turn_too() {
    drive(|| async {
        let (fx, ws, h) = held_workspace(true).await;
        let first = fx.rt.sessions().start(&ws, start("one"), None).await.expect("a start");
        let thread_id = first.view().thread_id.expect("a thread");
        h.reply(0, "answered", json!({}));
        let later = pending_start(&fx, &ws, on_thread("two", &thread_id, None));
        settle().await;
        // Steering it would put the words behind a reply the caller has already read.
        assert!(h.steered.borrow().is_empty());
        assert_eq!(held_prompts(&h), ["one"]);
        h.exit(0);
        assert_eq!(j(&answered(&later).await.outcome()), "queued");
        assert_eq!(held_prompts(&h), ["one", "two"]);
        h.end(1, "two done", json!({}));
        fx.rt.close().await.expect("a close");
    });
}

// "a start on a thread whose turn is running" > "a start on another thread of the same workspace is not held back by the running turn"
#[test]
fn a_start_on_another_thread_of_the_same_workspace_is_not_held_back_by_the_running_turn() {
    drive(|| async {
        let (fx, ws, h) = held_workspace(false).await;
        fx.rt.sessions().start(&ws, start("one"), None).await.expect("a start");
        let other = fx.rt.sessions().start(&ws, start("elsewhere"), None).await.expect("a start");
        assert_eq!(j(&other.outcome()), "started");
        assert_eq!(held_prompts(&h), ["one", "elsewhere"]);
        fx.rt.close().await.expect("a close");
    });
}

// "what a turn cost, on the row it ran on" > "is kept on the row the listing answers with, and adds up over the turns that ran there, so a reader of the list needs no transcript to say what a thread spent"
#[test]
fn is_kept_on_the_row_the_listing_answers_with_and_adds_up_over_the_turns_that_ran_there_so_a_reader_of_the_list_needs_no_transcript_to_say_what_a_thread_spent(
) {
    drive(|| async {
        let (fx, ws, h) = held_workspace(false).await;
        let one = fx.rt.sessions().start(&ws, start("orchestrate"), None).await.expect("a start");
        h.end(0, "done", json!({ "durationMs": 1_000, "costUsd": 0.75 }));
        one.finished().await;
        let cost = j(&one.view())["costUsd"].as_f64().expect("a cost");
        assert!((cost - 0.75).abs() < 5e-11, "{cost}");
        // A second turn on the same row: what the row says is what the turns on it have cost together.
        let thread_id = one.view().thread_id.expect("a thread");
        let two = fx.rt.sessions().start(&ws, on_thread("and again", &thread_id, None), None).await.expect("a start");
        h.end(1, "done again", json!({ "durationMs": 1_000, "costUsd": 0.39 }));
        two.finished().await;
        let total: f64 = rows(&fx, &ws).await.iter().filter_map(|r| r["costUsd"].as_f64()).sum();
        assert!((total - 1.14).abs() < 5e-11, "{total}");
        fx.rt.close().await.expect("a close");
    });
}

// "what a turn cost, on the row it ran on" > "is absent on a row whose harness reported no figure, so nothing reads a missing number as nothing spent"
#[test]
fn is_absent_on_a_row_whose_harness_reported_no_figure_so_nothing_reads_a_missing_number_as_nothing_spent() {
    drive(|| async {
        let (fx, ws, h) = held_workspace(false).await;
        let turn = fx.rt.sessions().start(&ws, start("orchestrate"), None).await.expect("a start");
        h.end(0, "done", json!({ "durationMs": 1_000 }));
        turn.finished().await;
        assert!(rows(&fx, &ws).await[0].get("costUsd").is_none());
        fx.rt.close().await.expect("a close");
    });
}
