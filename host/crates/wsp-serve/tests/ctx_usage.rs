// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 15 (#2476): the context's bodies for `UsageCtx` (52 methods, usage.ts:27-344),
//! `UsageResetDeps` (10, usage-reset.ts:53-68), `Resets` (5, adapters.ts:85) and `StatusCtx` (40, status.ts:9-182), read
//! against the TS at 65f6b89c7: runtime.ts:480 and :484, the two area files' reads of `ctx`, and the five TS tests the
//! plan names (usage, usage-boxes, usage-limit-resume, status, sweep-places).
//!
//! Those five build a whole runtime, make workspaces with `createOn` and run turns on a fake adapter. A Rust test cannot
//! replace an area, and the create, turn and session bodies they run through belong to packets 6, 8, 9, 11, 12 and 13, so
//! each method is read at its own seam on the runtime `create_runtime` builds over `FakeBackend` and the sqlite store,
//! seeded through the handles the traits hand out. The area crates' own functions run over the context where the TS test
//! drives them through `rt.usage` and the sweep (`UsageArea::read_logs`, `StatusArea::adopt_lost`). A test that needs
//! another packet's bodies says which in its comment ("waits on packet N") and is red until that packet lands.
//!
//! The usage-limit-resume file is all turns and threads (arming Resume at reset, the thread's row): packets 11 to 13.
//! Its one line in this packet is `UsageResetDeps::limits`, which boot's `account_key_on` reads (tested below).
mod support;

use std::cell::RefCell;
use std::future::Future;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_account::account::status::{ListedRow, StatusArea, StatusCtx as S};
use wsp_account::account::usage::{LimitReading, NameOf, PlanRoad, UsageArea, UsageCtx as U};
use wsp_account::usage_reset::{ResetPlace, ResetPlaceKind, Resets as R, UsageResetDeps as D};
use wsp_adapters::port::ResetRoad;
use wsp_core::clock::FakeClock;
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, DaemonChannelOptions, Frame};
use wsp_core::types::api::Runtime as _;
use wsp_core::types::wiring::{LiveWorkspace, LocalWiring, Platform, ReadingsTarget, RuntimeOptions, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_machines::machines::boot::BootRuntime;
use wsp_machines::machines::machines::Entry;
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::Store;
use wsp_wire::types::usage::{UsageRange, UsageSplit};
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::workspace_state::absent_computer;

// Fixtures.

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

/// The sqlite store of a throwaway state folder, as the host opens it.
fn sqlite_in(dir: &Path) -> Rc<dyn Store> {
    Rc::new(wsp_store::sqlite_store::sqlite_store(
        &dir.join("state.json"),
        wsp_store::store::StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() },
    ))
}

fn options(dir: &Path) -> RuntimeOptions {
    RuntimeOptions { backend: Some(backend()), store: Some(sqlite_in(dir)), ..Default::default() }
}

fn wiring() -> PlaceWiring {
    PlaceWiring {
        host_key: new_place_key_pair(),
        provider: None,
        here: Rc::new(|| HerePlace { name: "zingzys-mac".into(), ..Default::default() }),
        host_name: Rc::new(|| "zingzys-mac".to_string()),
        install: None,
        dial: None,
        log: None,
        update: None,
        leave: None,
        forget_key: None,
        sudo_over: None,
        provision: None,
        github_token: None,
        undo: None,
        run_over: None,
        back: None,
    }
}

fn with_door(dir: &Path) -> RuntimeOptions {
    RuntimeOptions { place_links: Some(Rc::new(wiring())), ..options(dir) }
}

/// This computer's wiring: the daemon road is the one asked for, the exec stream refuses (no turn runs here).
fn local_wiring(daemon: bool) -> LocalWiring {
    LocalWiring {
        backend: backend(),
        folder: "/tmp/wsp-local".into(),
        exec_stream: Rc::new(|_, _| unimplemented!("no turn runs in these tests")),
        home: Rc::new(|agent| format!("/home/me/.{agent}")),
        home_dir: "/home/me".into(),
        roots_path: "/home/me/.wsp/roots".into(),
        env: Rc::new(|| IndexMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string()), ("HOME".to_string(), "/home/me".to_string())])),
        daemon_road: daemon.then(|| {
            let held: Rc<dyn Fn() -> wsp_core::types::wiring::Local<'static, wsp_wire::views::workspace::DaemonReachView>> =
                Rc::new(|| {
                    Box::pin(async {
                        Ok(wsp_wire::views::workspace::DaemonReachView {
                            url: "ws://127.0.0.1:1".into(),
                            expires_at: wsp_wire::types::js_number::JsNumber(9.0e15),
                            daemon_token: Some("tok".into()),
                        })
                    })
                });
            held
        }),
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: None,
        platform: Platform::Linux,
        close: None,
    }
}

/// A daemon channel that answers each frame with the reply its op is given, keeping every frame asked.
struct Answering {
    replies: IndexMap<String, Value>,
    asked: Rc<RefCell<Vec<Value>>>,
}

#[async_trait(?Send)]
impl DaemonChannel for Answering {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        let op = frame.get("op").and_then(Value::as_str).unwrap_or_default().to_string();
        self.asked.borrow_mut().push(Value::Object(frame));
        let mut reply = match self.replies.get(&op) {
            Some(Value::Object(reply)) => reply.clone(),
            _ => serde_json::Map::from_iter([("ok".to_string(), json!(false)), ("error".to_string(), json!(format!("no {op} here")))]),
        };
        reply.insert("id".to_string(), json!(1));
        Ok(reply)
    }
    fn close(&self) {}
    async fn closed(&self) -> ChannelEnd {
        futures::future::pending().await
    }
}

fn answering(
    replies: Value,
    asked: Rc<RefCell<Vec<Value>>>,
) -> Rc<dyn Fn(DaemonChannelOptions) -> wsp_core::types::wiring::Local<'static, Rc<dyn DaemonChannel>>> {
    let replies: IndexMap<String, Value> = replies.as_object().expect("an object").iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    Rc::new(move |_| {
        let channel: Rc<dyn DaemonChannel> = Rc::new(Answering { replies: replies.clone(), asked: asked.clone() });
        Box::pin(async move { Ok(channel) })
    })
}

/// The runtime on a throwaway state, inside the task set the owner thread runs, and its context.
fn run<Fut: Future<Output = ()>>(opts: impl FnOnce(&Path) -> RuntimeOptions, f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>) -> Fut) {
    let dir = tempfile::tempdir().expect("a folder");
    let opts = opts(dir.path());
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let runtime = create_runtime(opts).expect("a runtime");
        let ctx = runtime.context();
        f(runtime, ctx).await
    });
    drop(dir);
}

fn plain(dir: &Path) -> RuntimeOptions {
    options(dir)
}

fn record(id: &str, extra: Value) -> WorkspaceRecord {
    let mut base = json!({
        "id": id, "name": "alpha", "machineId": "m_1", "phase": "running", "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z",
        "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false, "kind": "cloud",
    });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a workspace record")
}

fn project(id: &str, name: &str, path: &str, computer: &str) -> ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "path": path, "computer": computer, "source": { "kind": "folder", "path": path }, "remote": "",
        "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a project")
}

/// A live entry over a fake machine, as `attach` leaves one, put in the live map.
async fn put_live(c: &Rc<Ctx>, record: WorkspaceRecord) -> Entry {
    use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
    let machine = FakeBackend::new(FakeBackendOptions::default()).get_fake(&record.machine_id).await.expect("a fake machine");
    let hooks = WorkspaceHooks {
        golden_snapshot: String::new(),
        wake_attempts: 1,
        resurrect: None,
        vault_export: None,
        vault_import: None,
        stash_vault: None,
        restore_vault: None,
        wake_check: None,
        move_hook: None,
        retire: None,
    };
    let ws = Rc::new(Workspace::new(machine.clone(), hooks, WorkspaceInitial::default()));
    let id = record.id.clone();
    let entry: Entry = Rc::new(RefCell::new(LiveWorkspace {
        record: Rc::new(RefCell::new(record)),
        opens_thread: None,
        of_record_held: false,
        ws,
        machine,
        generation: 0,
        waking: None,
        unchecked: false,
        napping: None,
        adopting: None,
        deleting: None,
        late_read: None,
        wake_stop: None,
        resize_to: None,
        nap_wait_ms: None,
        wake_said: None,
        delete_said: None,
        wake_ask: None,
        creating: false,
        checkout: None,
        checkout_reading: None,
        tree: None,
        tree_reading: None,
        pr: None,
        pr_reading: None,
        pr_poll: None,
        pr_poll_ms: None,
        pr_none_at: None,
    }));
    BootRuntime::live(&**c).borrow_mut().insert(id, entry.clone());
    entry
}

fn listen(runtime: &AssembledRuntime, kind: &str) -> (Rc<RefCell<Vec<Value>>>, wsp_core::types::events::Unlisten) {
    let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
    let sink = seen.clone();
    let listener: wsp_core::types::events::EventListener = Rc::new(move |e: &EventUnion| {
        sink.borrow_mut().push(serde_json::to_value(e).expect("an event"));
        Ok(())
    });
    (seen, runtime.events().on(kind, listener))
}

fn reset_place(id: &str, name: &str, kind: ResetPlaceKind, connected: bool, signed_in: bool) -> ResetPlace {
    ResetPlace { id: id.into(), name: name.into(), kind, connected, signed_in: Rc::new(move |_| signed_in) }
}

/// The system clock the runtime reads (the day range counts back from it).
fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("time").as_millis() as i64
}

fn limit_json(key: &str, agent: &str, label: &str, road: &str, computers: Value) -> Value {
    json!({
        "key": key, "agent": agent, "label": label, "road": road, "windows": [{ "kind": "session", "usedPercent": 40 }],
        "status": "ok", "readAt": 1_000, "computers": computers,
    })
}

fn window_limit() -> wsp_wire::types::usage::HarnessLimit {
    serde_json::from_value(
        json!({ "windows": [{ "kind": "session", "usedPercent": 40 }, { "kind": "week", "usedPercent": 12 }], "status": "ok" }),
    )
    .expect("a limit")
}

fn reading(key: &str, agent: &str, label: &str, road: &str, computer: &str, at: Option<f64>) -> LimitReading {
    LimitReading {
        key: key.into(),
        agent: agent.into(),
        label: label.into(),
        road: road.into(),
        computer: computer.into(),
        limit: window_limit(),
        at,
    }
}

fn usage_entry(at: f64, account: &str) -> wsp_account::account::usage::UsageEntry {
    wsp_account::account::usage::UsageEntry {
        at,
        agent: "claude".into(),
        account: account.into(),
        account_label: "Claude Code signed in on zingzys-mac".into(),
        computer: "here".into(),
        project: String::new(),
        model: "claude-opus-5".into(),
        tokens: json!({ "input": 40, "output": 1, "cached": 0, "cacheWrite": 0, "reasoning": 0 }),
        cost_usd: None,
        session: "s-term".into(),
        source: "log".into(),
    }
}

// UsageCtx: the clock, the door, this computer and the core's maps (usage.ts:27-31, 36-44, 96-139, 208-255).

#[test]
fn the_clock_is_the_runtimes_and_a_scheduled_call_fires_on_it_until_cancelled() {
    let clock = FakeClock::new(5_000);
    let fake = clock.clone();
    run(
        move |dir| RuntimeOptions { clock: Some(fake.clock()), ..plain(dir) },
        |_, c| async move {
            assert_eq!(U::now(&*c), 5_000);
            let fired = Rc::new(RefCell::new(0));
            let hits = fired.clone();
            let cancel = U::schedule(&*c, 1_000, Box::new(move || *hits.borrow_mut() += 1));
            clock.advance(999);
            assert_eq!(*fired.borrow(), 0, "not yet");
            clock.advance(1);
            assert_eq!(*fired.borrow(), 1, "at its time");
            let hits = fired.clone();
            let cancel_second = U::schedule(&*c, 1_000, Box::new(move || *hits.borrow_mut() += 1));
            cancel_second();
            clock.advance(5_000);
            assert_eq!(*fired.borrow(), 1, "a cancelled call never fires");
            cancel();
        },
    );
}

#[test]
fn the_agent_logs_are_read_unless_the_preference_is_turned_off() {
    // waits on packet 16: `PreferencesApi::set` writes through `PreferencesCtx::set_held` and `store`.
    run(plain, |rt, c| async move {
        assert!(U::usage_logs_on(&*c).await, "usageLogs defaults on (DEFAULT_PREFERENCES)");
        let patch = serde_json::from_value(json!({ "usageLogs": false })).expect("a patch");
        rt.preferences().set(patch).await.expect("set");
        assert!(!U::usage_logs_on(&*c).await, "the read follows the stored preference");
    });
}

#[test]
fn with_no_door_there_are_no_places_and_nothing_is_linked_or_signed_in() {
    run(plain, |_, c| async move {
        assert!(!U::has_door(&*c));
        assert!(U::places(&*c, 0).await.is_empty(), "placeDoor?.list(now) ?? []");
        assert!(!U::linked(&*c, "here"));
        assert!(!U::signed_in_at(&*c, "srv_1", "claude"));
        assert_eq!(U::place_for(&*c, "srv").await.expect("no door, no refusal"), None);
        assert_eq!(U::box_login_path(&*c, "srv_1").await, None);
        assert!(!U::place_away(&*c, "srv_1"));
        assert_eq!(U::login_home(&*c, "srv_1", "claude").await, None);
    });
}

#[test]
fn a_door_lists_this_computer_and_names_a_place_by_its_own_name() {
    // waits on the join line in ctx/build.rs, `ctx.status_tracker.fill(ctx.usage_status_tracker())`: the door lists a place
    // through its spend, which reads the status tracker (runtime.ts:1056).
    run(with_door, |_, c| async move {
        assert!(U::has_door(&*c));
        let places = U::places(&*c, 0).await;
        assert!(places.iter().any(|p| p.id == "here"), "this computer is the row every door lists: {places:?}");
        assert_eq!(U::name_of(&*c, "here"), "zingzys-mac");
        assert!(!U::linked(&*c, "srv_1"), "no link to a computer never joined");
        // places/link.ts:518 throws for a word naming no computer, and usage.ts:307 lets it through.
        let refused = U::place_for(&*c, "no-such-computer").await.expect_err("no place of that name");
        assert!(refused.message.starts_with("no place named no-such-computer; you have zingzys-mac"), "{}", refused.message);
        assert_eq!(U::computer_name(&*c, &places, "here", None), "zingzys-mac");
    });
}

#[test]
fn this_computers_wiring_answers_the_home_the_env_and_the_daemon() {
    run(
        |dir| RuntimeOptions { local: Some(local_wiring(true)), ..plain(dir) },
        |_, c| async move {
            assert!(U::local_wired(&*c));
            assert!(U::local_has_daemon(&*c));
            assert_eq!(U::local_home(&*c, "claude"), "/home/me/.claude");
            let env = U::local_env(&*c);
            assert_eq!(env.get("PATH").map(String::as_str), Some("/usr/bin:/bin"));
            assert_eq!(env.get("HOME").map(String::as_str), Some("/home/me"));
        },
    );
    run(plain, |_, c| async move {
        assert!(!U::local_wired(&*c));
        assert!(!U::local_has_daemon(&*c));
    });
    run(
        |dir| RuntimeOptions { local: Some(local_wiring(false)), ..plain(dir) },
        |_, c| async move {
            assert!(U::local_wired(&*c));
            assert!(!U::local_has_daemon(&*c), "local.daemonRoad !== undefined");
        },
    );
}

#[test]
fn a_wsp_thread_session_and_a_project_are_read_off_the_cores_maps() {
    run(plain, |_, c| async move {
        assert!(U::wsp_sessions(&*c).is_empty());
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web", "here"));
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_2".into(), project("p_2", "api", "/work/api", "srv_1"));
        assert_eq!(U::projects_on(&*c, "here"), vec![("p_1".to_string(), "/work/web".to_string())], "projects whose computer is the place");
        assert_eq!(U::projects_on(&*c, "srv_1"), vec![("p_2".to_string(), "/work/api".to_string())]);
        assert!(U::projects_on(&*c, "srv_9").is_empty());
        assert_eq!(U::project_name(&*c, "p_1").as_deref(), Some("web"));
        assert_eq!(U::project_name(&*c, "p_9"), None);
    });
}

#[test]
fn the_agent_setup_of_a_computer_answers_its_env_config_dir_and_program() {
    run(plain, |_, c| async move {
        let (env, config_dir, program) = U::launch_of(&*c, "here", "claude");
        assert!(env.is_empty(), "a computer nobody set up carries no variables: {env:?}");
        assert_eq!((config_dir, program), (None, None));
        assert_eq!(U::vaulted_for(&*c, "claude"), None, "nothing in the vault, no word");
    });
}

#[test]
fn the_catalogs_words_label_an_agent_a_model_and_the_plan_tables_name_codex_alone() {
    run(plain, |_, c| async move {
        assert_eq!(U::agent_label(&*c, "claude"), "Claude Code");
        assert_eq!(U::agent_label(&*c, "no-such-agent"), "no-such-agent", "harnessCatalog(agent)?.label ?? agent");
        assert_eq!(U::model_label(&*c, "claude-opus-5"), wsp_core::harness_catalog::model_label("claude-opus-5"));
        assert_eq!(U::plan_agents(&*c), vec!["codex".to_string()], "Object.entries(PLAN_RESETS) agents, in order");
        let snapshot = json!({ "primary": { "usedPercent": 40, "windowDurationMins": 300, "resetsAt": 1_900_000_000 }, "secondary": { "usedPercent": 12, "windowDurationMins": 10_080 } });
        let limit = U::log_limit(&*c, "codex", &snapshot).expect("codex's windows read off its store");
        assert!(!limit.windows.is_empty());
        assert!(U::log_limit(&*c, "claude", &snapshot).is_none(), "LOG_LIMITS holds codex alone");
        assert!(U::log_limit(&*c, "codex", &json!("not an object")).is_none(), "a snapshot that is no object reads nothing");
    });
}

#[test]
fn the_plan_read_of_codex_is_its_own_script_and_its_own_parse() {
    run(plain, |_, c| async move {
        let road = PlanRoad { home: "/home/me/.codex".into(), env: IndexMap::new(), launch_program: None };
        let script = U::plan_read_command(&*c, "codex", &road);
        assert!(script.contains("/home/me/.codex"), "the script runs under the login's home: {script}");
        assert_eq!(U::plan_parse(&*c, "codex", ""), None, "an answer with no reading is none");
    });
}

#[test]
fn a_relative_home_is_the_throw_the_ts_catches_and_the_script_is_then_nothing() {
    // usage.ts:284: `readCommand` throws on a home that is not absolute and the plan read warns `limits ... were not
    // read`; the trait answers a plain string, so the refusal is an empty script: nothing runs, nothing is filed.
    run(plain, |_, c| async move {
        let road = PlanRoad { home: "relative/.codex".into(), env: IndexMap::new(), launch_program: None };
        assert_eq!(U::plan_read_command(&*c, "codex", &road), "");
        assert_eq!(U::plan_read_command(&*c, "no-such-agent", &road), "", "an agent with no plan resets has no script");
    });
}

// UsageCtx: the places and the daemons (usage.ts:96-139, 323-336); usage-boxes.test.ts.

#[test]
fn this_computers_agent_logs_are_read_by_its_own_daemon_over_the_stores_the_catalog_names() {
    let asked: Rc<RefCell<Vec<Value>>> = Rc::default();
    let replies = json!({ "usage.logs": { "ok": true, "rows": [
        { "agent": "claude", "session": "s-term", "at": now_ms(), "model": "claude-opus-5", "folder": "/work/web/src",
          "tokens": { "input": 40, "output": 1, "cached": 0, "cacheWrite": 0, "reasoning": 0 } },
    ], "limits": [] } });
    let channel = answering(replies, asked.clone());
    run(
        move |dir| RuntimeOptions { local: Some(local_wiring(true)), daemon_channel: Some(channel), ..plain(dir) },
        |_, c| async move {
            let stores = UsageArea::usage_stores();
            let reply = U::read_logs_on(&*c, "here", &stores).await.expect("the daemon's reply");
            assert_eq!(reply.rows.len(), 1);
            assert_eq!((reply.rows[0].agent.as_str(), reply.rows[0].session.as_str()), ("claude", "s-term"));
            let asked = asked.borrow();
            assert_eq!(asked.len(), 1);
            assert_eq!(asked[0]["op"], json!("usage.logs"));
            assert_eq!(asked[0]["stores"], serde_json::to_value(&stores).expect("stores"), "the frame carries the catalog's stores");
        },
    );
}

#[test]
fn this_computer_without_a_daemon_reads_no_logs_and_a_joined_one_with_no_link_is_absent() {
    run(plain, |_, c| async move {
        let said = U::read_logs_on(&*c, "here", &UsageArea::usage_stores()).await.expect_err("no daemon here");
        assert_eq!(said.message(), "this host runs no daemon on this computer");
    });
    run(with_door, |_, c| async move {
        let said = U::read_logs_on(&*c, "srv_1", &UsageArea::usage_stores()).await.expect_err("no link to it");
        let name = U::name_of(&*c, "srv_1");
        assert_eq!(said.message(), absent_computer(&name, None).sentence, "usage.ts:96-97");
    });
}

#[test]
fn a_usage_read_files_this_computers_log_rows_as_outside_wsp_under_its_own_login_and_the_project_a_folder_is_in() {
    // waits on packet 16: `usage_logs_on` reads `PreferencesArea::get`, which reads the held preferences.
    // usage.test.ts "files the logs' rows as outside wsp, a folder inside a project under it and any other under no
    // project", and usage-boxes "filed under that computer, a folder in its project under that project".
    let asked: Rc<RefCell<Vec<Value>>> = Rc::default();
    let replies = json!({ "usage.logs": { "ok": true, "rows": [
        { "agent": "claude", "session": "s-term", "at": now_ms(), "model": "claude-opus-5", "folder": "/work/web/src",
          "tokens": { "input": 40, "output": 1, "cached": 0, "cacheWrite": 0, "reasoning": 0 } },
        { "agent": "claude", "session": "s-else", "at": now_ms(), "model": "claude-opus-5", "folder": "/tmp/else",
          "tokens": { "input": 5, "output": 1, "cached": 0, "cacheWrite": 0, "reasoning": 0 } },
    ], "limits": [] } });
    let channel = answering(replies, asked.clone());
    run(
        move |dir| RuntimeOptions { local: Some(local_wiring(true)), daemon_channel: Some(channel), ..plain(dir) },
        |rt, c| async move {
            BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web", "here"));
            let area = UsageArea::new(c.clone());
            area.read_logs().await.expect("the logs are read");
            let used = rt.usage().used(UsageRange::Day, UsageSplit::Project, Some(true)).await.expect("used");
            let labels: Vec<String> = used.rows.iter().map(|r| r.label.clone()).collect();
            assert!(labels.contains(&"web".to_string()), "the folder inside the project is the project's: {labels:?}");
            assert!(labels.contains(&"No project".to_string()), "any other folder is under no project: {labels:?}");
            let by_account = rt.usage().used(UsageRange::Day, UsageSplit::Account, Some(true)).await.expect("used");
            assert_eq!(
                by_account.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
                vec!["Claude Code signed in on this computer"],
                "a terminal's work is under this computer's own login of the agent"
            );
        },
    );
}

#[test]
fn a_thread_wsp_ran_is_never_filed_from_the_logs() {
    // waits on packet 16: `usage_logs_on` reads `PreferencesArea::get`, which reads the held preferences.
    // usage.test.ts "a wsp thread's own transcript is never filed as outside wsp", read through `wsp_sessions`.
    let asked: Rc<RefCell<Vec<Value>>> = Rc::default();
    let replies = json!({ "usage.logs": { "ok": true, "rows": [
        { "agent": "claude", "session": "s-wsp", "at": now_ms(), "model": "claude-opus-5", "folder": "/tmp/x",
          "tokens": { "input": 40, "output": 1, "cached": 0, "cacheWrite": 0, "reasoning": 0 } },
    ], "limits": [] } });
    let channel = answering(replies, asked);
    run(
        move |dir| RuntimeOptions { local: Some(local_wiring(true)), daemon_channel: Some(channel), ..plain(dir) },
        |rt, c| async move {
            let view: wsp_wire::views::session::SessionView = serde_json::from_value(json!({
                "id": "sess_1", "workspaceId": "w_1", "harness": "claude", "status": "completed", "claudeSessionId": "s-wsp",
            }))
            .expect("a session view");
            let entry = wsp_core::types::internal::SessionEntry {
                turn_id: "turn-1".into(),
                view,
                replaces: None,
                notify: None,
                notify_by: None,
                notify_road: None,
                turn_token: None,
                scope_device_id: None,
                handle: None,
                end: None,
                turn_live: None,
                run: None,
                from: None,
                asked: None,
                snapshot: None,
                pid: None,
                launch: None,
                calls: None,
                unanswered: false,
                queued: false,
            };
            BootRuntime::sessions(&*c).borrow_mut().insert("sess_1".into(), Rc::new(RefCell::new(entry)));
            assert_eq!(U::wsp_sessions(&*c), vec!["s-wsp".to_string()]);
            UsageArea::new(c.clone()).read_logs().await.expect("read");
            let used = rt.usage().used(UsageRange::Day, UsageSplit::Source, None).await.expect("used");
            assert!(used.rows.iter().all(|r| r.label != "Outside wsp"), "the thread's own transcript is not outside wsp: {:?}", used.rows);
        },
    );
}

#[test]
fn a_readings_ask_goes_to_this_computers_daemon_and_hands_the_reply_on_as_it_said_it() {
    // usage.test.ts "asks the machine's daemon for its kept minutes over the range" (here, the one daemon this test has).
    let asked: Rc<RefCell<Vec<Value>>> = Rc::default();
    let replies = json!({ "sys.history": { "ok": true, "points": [ { "at": 1, "cpu": 0.5 } ], "stepMs": 15_000 } });
    let channel = answering(replies, asked.clone());
    run(
        move |dir| RuntimeOptions { local: Some(local_wiring(true)), daemon_channel: Some(channel), ..plain(dir) },
        |_, c| async move {
            let mut frame = serde_json::Map::new();
            frame.insert("op".into(), json!("sys.history"));
            frame.insert("from".into(), json!(0));
            frame.insert("to".into(), json!(900_000));
            frame.insert("stepMs".into(), json!(15_000));
            let reply = U::sys_history(&*c, &ReadingsTarget::Place("here".into()), frame, None).await.expect("a reply");
            assert_eq!(reply["stepMs"], json!(15_000));
            assert_eq!(asked.borrow()[0]["op"], json!("sys.history"));
        },
    );
    run(with_door, |_, c| async move {
        let mut frame = serde_json::Map::new();
        frame.insert("op".into(), json!("sys.history"));
        let said = U::sys_history(&*c, &ReadingsTarget::Place("srv_1".into()), frame, None).await.expect_err("no link");
        let name = U::name_of(&*c, "srv_1");
        assert_eq!(said.message(), absent_computer(&name, None).sentence);
    });
}

#[test]
fn boot_is_awaited_before_the_threads_read_off_the_sessions_map() {
    // waits on packet 5: `BootArea::ready`.
    run(plain, |_, c| async move {
        U::ready(&*c).await;
        S::ready(&*c).await;
    });
}

#[test]
fn this_computers_sign_ins_are_none_where_the_host_wired_no_agents_reader() {
    // usage.ts:166-171: `opts.agentsReader?.read(..) ?? Promise.resolve(undefined)`. The reader is a host port with no
    // Rust body yet (ruling 3 of the plan's section 5), so no test can wire one: the answer is none, and the area
    // reads it as no sign-ins (`here_sign_ins` holds `{}`).
    run(plain, |_, c| async move {
        assert_eq!(U::read_sign_ins_here(&*c).await.expect("no reader is no read"), None);
        let area = UsageArea::new(c.clone());
        assert!(area.here_sign_ins().await.is_empty());
    });
}

// UsageCtx: the ledger, the alerts and the account words (usage.ts:46-50, 106-139, 183-206); usage.test.ts.

#[test]
fn the_ledger_keeps_a_limit_a_turn_printed_and_hands_back_what_it_replaced() {
    run(plain, |_, c| async move {
        let (before, after) =
            U::file_limit(&*c, reading("claude:vault-token", "claude", "Claude Code with your sign-in", "vault", "here", None))
                .await
                .expect("filed");
        assert_eq!(before, Value::Null, "the ledger held none: null where the TS has undefined");
        assert_eq!(after["key"], json!("claude:vault-token"));
        assert_eq!(after["windows"][0]["usedPercent"], json!(40));
        let limits = U::limits(&*c).await.expect("limits");
        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0]["key"], json!("claude:vault-token"));
        let (again, _) =
            U::file_limit(&*c, reading("claude:vault-token", "claude", "Claude Code with your sign-in", "vault", "here", None))
                .await
                .expect("filed");
        assert_eq!(again["key"], json!("claude:vault-token"), "the second filing replaces the first and hands it back");
    });
}

#[test]
fn a_reading_older_than_the_one_held_leaves_the_held_one_standing() {
    // usage.ts:412-437 (`at` against `readAt`), through the context's ledger.
    run(plain, |_, c| async move {
        let first = reading("codex@here", "codex", "Codex signed in on zingzys-mac", "own", "here", Some(f64::MAX / 4.0));
        U::file_limit(&*c, first).await.expect("filed");
        let (before, after) =
            U::file_limit(&*c, reading("codex@here", "codex", "Codex signed in on zingzys-mac", "own", "here", Some(1.0)))
                .await
                .expect("filed");
        assert_eq!(before, after, "an older reading changes nothing");
    });
}

#[test]
fn the_ledger_answers_what_was_used_split_by_account_and_names_every_account_once() {
    run(plain, |_, c| async move {
        let now = U::now(&*c) as f64;
        U::file_logs(&*c, vec![usage_entry(now, "claude@here")], vec!["here".into()]).await.expect("filed");
        let labels = U::account_labels(&*c).await.expect("labels");
        assert_eq!(labels.get("claude@here").map(String::as_str), Some("Claude Code signed in on zingzys-mac"));
        let by_label: wsp_account::account::usage::LabelOf = Rc::new(|_split, value| format!("<{value}>"));
        let used = U::used(&*c, UsageRange::Day, UsageSplit::Account, true, by_label).await.expect("used");
        assert_eq!(
            used.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
            vec!["<claude@here>"],
            "the label comes from the function the area hands it"
        );
    });
}

#[test]
fn a_reading_the_ledger_now_holds_is_what_the_alerts_compare_against() {
    // usage.test.ts "tells every client once a reading crosses 70% of a window, and not again for a reading no worse".
    run(plain, |rt, c| async move {
        let (seen, _off) = listen(&rt, "usage.alert");
        let hot: wsp_wire::types::usage::HarnessLimit = serde_json::from_value(
            json!({ "windows": [{ "kind": "session", "usedPercent": 75, "resetsAt": 1_900_000_000_000_i64 }], "status": "ok" }),
        )
        .expect("a limit");
        let mut r = reading("claude:vault-token", "claude", "Claude Code with your sign-in", "vault", "here", None);
        let (before_cool, after_cool) = U::file_limit(&*c, r.clone()).await.expect("filed");
        U::alerts_read(&*c, before_cool, after_cool).await.expect("read");
        assert!(seen.borrow().is_empty(), "40% is under the line");
        r.limit = hot;
        let (before, after) = U::file_limit(&*c, r.clone()).await.expect("filed");
        U::alerts_read(&*c, before.clone(), after.clone()).await.expect("read");
        assert_eq!(seen.borrow().len(), 1, "crossing 70% says so once: {:?}", seen.borrow());
        U::alerts_read(&*c, after.clone(), after).await.expect("read");
        assert_eq!(seen.borrow().len(), 1, "a reading no worse says nothing more");
    });
}

#[test]
fn an_account_is_keyed_and_labelled_as_the_ts_does() {
    run(plain, |_, c| async move {
        let own = U::account_of(&*c, "opencode", None, "here", "zingzy's MacBook Pro");
        assert_eq!(
            (own.key.as_str(), own.label.as_str(), own.road.as_str()),
            ("opencode@here", "OpenCode signed in on zingzy's MacBook Pro", "own")
        );
        let named: wsp_wire::types::usage::HarnessLimitAccount =
            serde_json::from_value(json!({ "id": "acct_9", "label": "Maya's team" })).expect("an account");
        let account = U::account_of(&*c, "codex", Some(&named), "here", "Mac");
        assert_eq!((account.key.as_str(), account.label.as_str(), account.road.as_str()), ("codex:acct_9", "Maya's team", "named"));
    });
}

#[test]
fn a_computers_account_is_the_one_its_turns_named_else_the_login() {
    run(plain, |_, c| async move {
        let named = limit_json("claude:maya", "claude", "maya@example.com", "named", json!(["here"]));
        let on = U::account_on_computer(&*c, "claude", "here", "Mac", &[named]);
        assert_eq!((on.key.as_str(), on.label.as_str()), ("claude:maya", "maya@example.com"));
        let none = U::account_on_computer(&*c, "claude", "here", "Mac", &[]);
        assert_eq!((none.key.as_str(), none.label.as_str(), none.road.as_str()), ("claude@here", "Claude Code signed in on Mac", "own"));
    });
}

#[test]
fn the_account_rows_list_each_agent_signed_in_here_before_any_turn_has_reported_a_limit() {
    // usage.test.ts "lists each agent signed in here off this computer's own read".
    run(plain, |_, c| async move {
        let name_of: NameOf = Rc::new(|id: &str| if id == "here" { "Mac".to_string() } else { id.to_string() });
        let places = vec![(
            "here".to_string(),
            "Mac".to_string(),
            Some(json!({ "claude": "signed-in", "codex": "signed-in", "opencode": "signed-in" })),
        )];
        let answer = U::account_rows(&*c, vec![], places, name_of).await.expect("rows");
        let rows: Vec<(String, Option<String>, usize)> = answer
            .accounts
            .iter()
            .map(|a| (a.key.clone(), a.note.map(|n| serde_json::to_value(n).unwrap().as_str().unwrap().to_string()), a.computers.len()))
            .collect();
        assert_eq!(
            rows,
            vec![
                ("claude@here".to_string(), Some("not read yet: shows after its next turn".to_string()), 1),
                ("codex@here".to_string(), Some("not read yet: shows after its next turn".to_string()), 1),
                ("opencode@here".to_string(), Some("reports no plan limit".to_string()), 1),
            ]
        );
    });
}

#[test]
fn an_account_a_turn_read_limits_for_is_named_off_that_reading() {
    // usage.test.ts "names an account the harness named by its plan, in the limits and in what was used alike".
    run(plain, |_, c| async move {
        let mut limit = limit_json("claude:acct-1", "claude", "dev@example.com", "named", json!(["here"]));
        limit["plan"] = json!("max");
        let name_of: NameOf = Rc::new(|id: &str| id.to_string());
        let names = U::account_names(&*c, vec![limit], name_of).await.expect("names");
        assert_eq!(names.get("claude:acct-1").map(String::as_str), Some("Claude Code with Claude Max"));
    });
}

#[test]
fn a_computers_name_is_kept_once_it_was_listed_and_read_back_when_it_is_gone() {
    // waits on the join line in ctx/build.rs, `ctx.status_tracker.fill(ctx.usage_status_tracker())`: the door lists a place
    // through its spend, which reads the status tracker (runtime.ts:1056).
    run(with_door, |_, c| async move {
        let places = U::places(&*c, 0).await;
        let kept = U::kept_computer_names(&*c, &places).await.expect("kept");
        assert!(!kept.contains_key("here"), "this computer is never kept: it is always listed");
        let removed = U::computer_name(&*c, &places, "srv_gone", Some(&kept));
        assert_eq!(removed, "a removed computer", "a computer neither listed nor kept");
        let mut named = IndexMap::new();
        named.insert("srv_gone".to_string(), "the old box".to_string());
        assert_eq!(U::computer_name(&*c, &places, "srv_gone", Some(&named)), "the old box");
        assert_eq!(U::computer_name(&*c, &places, "srv_gone", None), "a removed computer", "no kept names is an empty map");
    });
}

#[test]
fn spending_a_reset_on_an_account_nobody_has_is_refused_in_the_words_of_the_ts() {
    run(plain, |_, c| async move {
        let said = U::spend_reset(&*c, "codex:nobody", None, None).await.expect_err("no such account");
        assert_eq!(said.message, wsp_words::usage::no_such_account_line("codex:nobody"));
        assert_eq!(said.kind, Some(wsp_words::exit::Kind::NotFound));
    });
}

#[test]
fn a_spend_is_the_ledgers_to_read_so_a_limit_filed_through_the_context_is_the_one_a_spend_sees() {
    // The one ledger, shared: the reading filed through `UsageCtx` is what `UsageResetDeps::limits` and `row` read.
    run(plain, |_, c| async move {
        U::file_limit(&*c, reading("claude:vault-key", "claude", "Claude Code with an API key", "vault", "here", None))
            .await
            .expect("filed");
        let held = D::limits(&*c).await.expect("limits");
        assert_eq!(held.iter().map(|l| l.key.as_str()).collect::<Vec<_>>(), vec!["claude:vault-key"]);
        let row = D::row(&*c, "claude:vault-key").await.expect("row").expect("the account's row");
        assert_eq!(row.key, "claude:vault-key");
        let said = U::spend_reset(&*c, "claude:vault-key", None, None).await.expect_err("an agent with no plan resets");
        assert_eq!(said.message, wsp_words::usage::reset_none_line("Claude Code"), "usage-reset.ts:106: PLAN_RESETS holds codex alone");
    });
}

#[test]
fn a_reason_for_a_row_is_whatever_the_reach_area_holds_against_the_machine() {
    run(plain, |_, c| async move {
        let entry = put_live(&c, record("w_1", json!({}))).await;
        assert_eq!(U::nap_refused_reason(&*c, &entry), None);
        assert_eq!(U::stop_refused_reason(&*c, &entry), None);
        assert_eq!(U::disk_reason(&*c, &entry), None);
    });
}

#[test]
fn spawn_runs_the_future_now_and_warn_is_one_line_to_stderr() {
    run(plain, |_, c| async move {
        let ran = Rc::new(RefCell::new(false));
        let flag = ran.clone();
        U::spawn(&*c, Box::pin(async move { *flag.borrow_mut() = true }));
        tokio::task::yield_now().await;
        assert!(*ran.borrow(), "the TS void promise starts on its own");
        U::warn(&*c, "a line");
    });
}

// The one built state: ledger, prices, alerts, burn and resets are built once and shared (usage.ts:46-50, 292-303).

#[test]
fn the_ledger_prices_burn_alerts_and_resets_are_built_once_per_runtime() {
    run(plain, |_, c| async move {
        let first = c.usage_parts();
        let second = c.usage_parts();
        assert!(Rc::ptr_eq(&first, &second), "one set of parts for the context");
        assert!(Rc::ptr_eq(&first.ledger, &c.usage_parts().ledger));
    });
    let mut parts = Vec::new();
    for _ in 0..2 {
        run(plain, |_, c| {
            parts.push(Rc::as_ptr(&c.usage_parts()) as usize);
            async {}
        });
    }
    assert_ne!(parts[0], parts[1], "a second runtime has parts of its own");
}

#[test]
fn a_runtime_whose_state_holds_an_armed_alert_arms_it_again_at_construction() {
    // usage.ts:49: `alerts.resume()` at construction; the packet's `usage_arm` is the line build.rs calls.
    run(plain, |_, c| async move {
        c.usage_arm();
        assert_eq!(c.usage_parts().alerts.armed(), 0, "nothing was armed in a fresh state");
    });
}

#[test]
fn the_burn_the_context_holds_is_the_one_an_account_row_reads() {
    // usage.test.ts "reads off the calls its running turns report, on the account each turn runs on".
    run(plain, |_, c| async move {
        c.usage_parts().burn.add(wsp_machines::usage::accounts::BurnCall {
            account: "claude@here".into(),
            thread_id: "t_1".into(),
            tokens: 150.0,
            replayed: None,
        });
        let name_of: NameOf = Rc::new(|id: &str| id.to_string());
        let places = vec![("here".to_string(), "Mac".to_string(), Some(json!({ "claude": "signed-in" })))];
        let answer = U::account_rows(&*c, vec![], places, name_of).await.expect("rows");
        let burn = answer.accounts[0].burn.clone().expect("the row carries the burn");
        assert_eq!(burn.tokens_per_minute.0, 10.0, "150 tokens over the 15 minute window");
        assert_eq!(burn.threads.0 .0, 1.0);
    });
}

// UsageResetDeps and Resets (usage-reset.ts:53-68, usage.ts:208-263, adapters.ts:85).

#[test]
fn the_reset_deps_read_the_cores_store_the_catalogs_names_and_a_fresh_id_each_time() {
    run(plain, |_, c| async move {
        let store = D::store(&*c);
        store.put("probe", "k", json!({ "v": 1 })).await.expect("put");
        assert_eq!(
            wsp_core::types::pending::Store::get(&*D::store(&*c), "probe", "k").await.expect("get"),
            Some(json!({ "v": 1 })),
            "the one store"
        );
        assert_eq!(D::agent_name(&*c, "codex"), "Codex");
        assert_eq!(D::agent_name(&*c, "no-such-agent"), "no-such-agent");
        let (a, b) = (D::uuid(&*c), D::uuid(&*c));
        assert_ne!(a, b);
        assert_eq!(a.len(), 36, "a uuid: {a}");
        assert!(D::limits(&*c).await.expect("limits").is_empty());
    });
}

#[test]
fn only_codex_has_plan_resets_and_its_five_words_are_the_adapters() {
    run(plain, |_, c| async move {
        assert!(D::resets(&*c, "claude").is_none(), "PLAN_RESETS[agent] is undefined");
        let resets = D::resets(&*c, "codex").expect("codex has resets");
        let codex = wsp_adapters::codex::reset::CodexPlanResets;
        use wsp_adapters::port::PlanResets as _;
        assert_eq!(resets.too_old(), codex.too_old());
        let road = ResetRoad { home: "/home/me/.codex".into(), env: None, launch: None };
        assert_eq!(resets.read_command(&road), codex.read_command(&road));
        assert_eq!(resets.spend_command(&road, "key-1", None), codex.spend_command(&road, "key-1", None));
        let relative = ResetRoad { home: "relative".into(), env: None, launch: None };
        assert_eq!(resets.read_command(&relative), codex.read_command(&relative), "the throw keeps its message");
        assert!(resets.read_command(&relative).is_err());
        assert_eq!(resets.parse_read(""), codex.parse_read(""));
        assert_eq!(format!("{:?}", resets.parse_spend("")), format!("{:?}", codex.parse_spend("")));
    });
}

#[test]
fn the_context_itself_answers_the_resets_words_as_the_one_agent_that_has_them() {
    // The skeleton's `impl Resets for Ctx` (every_trait_answers calls `too_old`): the same five words, codex's.
    run(plain, |_, c| async move {
        use wsp_adapters::port::PlanResets as _;
        let codex = wsp_adapters::codex::reset::CodexPlanResets;
        assert_eq!(R::too_old(&*c), codex.too_old());
        let road = ResetRoad { home: "/home/me/.codex".into(), env: None, launch: None };
        assert_eq!(R::read_command(&*c, &road), codex.read_command(&road));
        assert_eq!(R::spend_command(&*c, &road, "key-1", Some("credit-1")), codex.spend_command(&road, "key-1", Some("credit-1")));
        assert_eq!(R::parse_read(&*c, ""), codex.parse_read(""));
        assert_eq!(format!("{:?}", R::parse_spend(&*c, "")), format!("{:?}", codex.parse_spend("")));
    });
}

#[test]
fn each_computer_a_plan_read_may_run_on_is_this_one_a_joined_box_or_a_provider() {
    // waits on the join line in ctx/build.rs, `ctx.status_tracker.fill(ctx.usage_status_tracker())`: the door lists a place
    // through its spend, which reads the status tracker (runtime.ts:1056).
    run(with_door, |_, c| async move {
        let place_of = D::places(&*c).await.expect("the places");
        let here = place_of("here");
        assert_eq!(
            (here.kind, here.name.as_str(), here.connected),
            (wsp_account::usage_reset::ResetPlaceKind::Here, "zingzys-mac", false),
            "no local wiring: not connected"
        );
        let unknown = place_of("srv_9");
        assert_eq!(unknown.kind, wsp_account::usage_reset::ResetPlaceKind::Provider, "an id the host does not know still answers a place");
        assert!(!unknown.connected);
        assert!(!(unknown.signed_in)("codex"));
    });
    run(
        |dir| RuntimeOptions { local: Some(local_wiring(true)), ..with_door(dir) },
        |_, c| async move {
            let place_of = D::places(&*c).await.expect("the places");
            assert!(place_of("here").connected, "connected: local !== undefined");
        },
    );
}

#[test]
fn the_road_of_an_agents_login_is_its_config_dir_or_this_computers_own_home() {
    // waits on the join line in ctx/build.rs, `ctx.status_tracker.fill(ctx.usage_status_tracker())`: the door lists a place
    // through its spend, which reads the status tracker (runtime.ts:1056).
    run(
        |dir| RuntimeOptions { local: Some(local_wiring(true)), ..with_door(dir) },
        |_, c| async move {
            let place_of = D::places(&*c).await.expect("the places");
            let here = place_of("here");
            let road = D::road(&*c, "codex", &here).await.expect("a road");
            assert_eq!(road.home, "/home/me/.codex", "local.home(agent) where the setup names no folder");
            assert_eq!(road.env.as_ref().map(|e| e.len()), Some(0), "the script exports nothing of the setup's");
        },
    );
    run(plain, |_, c| async move {
        let here = reset_place("here", "zingzys-mac", ResetPlaceKind::Here, false, false);
        let said = D::road(&*c, "codex", &here).await.expect_err("no local wiring");
        assert_eq!(said.message, absent_computer("zingzys-mac", None).sentence);
        let box_place = reset_place("srv_1", "srv", ResetPlaceKind::Box, true, true);
        let said = D::road(&*c, "codex", &box_place).await.expect_err("a box that lists no logins");
        assert_eq!(said.message, wsp_words::usage::reset_no_logins_line("srv", "Codex"));
    });
}

#[test]
fn one_script_runs_on_a_box_over_the_door_and_here_as_a_child_of_this_host() {
    run(plain, |_, c| async move {
        let box_place = reset_place("srv_1", "srv", ResetPlaceKind::Box, true, true);
        let said = D::run(&*c, &box_place, "echo hi", "codex").await.expect_err("no door");
        assert_eq!(said.message, wsp_machines::places::types::NO_PLACE_DOOR);
        let here = reset_place("here", "zingzys-mac", ResetPlaceKind::Here, false, false);
        let said = D::run(&*c, &here, "echo hi", "codex").await.expect_err("no local wiring");
        assert_eq!(said.message, absent_computer("zingzys-mac", None).sentence);
    });
}

#[test]
fn a_reading_a_plans_own_read_gave_is_filed_the_way_a_turns_is_under_the_key_it_answers() {
    run(plain, |_, c| async move {
        let here = reset_place("here", "zingzys-mac", ResetPlaceKind::Here, true, true);
        let limit = window_limit();
        let key = D::file(&*c, wsp_account::usage_reset::FileAsk { agent: "codex", place: &here, limit: &limit }).await.expect("filed");
        assert_eq!(key, "codex@here");
        assert_eq!(D::limits(&*c).await.expect("limits").iter().map(|l| l.key.as_str()).collect::<Vec<_>>(), vec!["codex@here"]);
        let named: wsp_wire::types::usage::HarnessLimit =
            serde_json::from_value(json!({ "windows": [], "account": { "id": "acct_1", "label": "dev@example.com" } })).expect("a limit");
        let key = D::file(&*c, wsp_account::usage_reset::FileAsk { agent: "codex", place: &here, limit: &named }).await.expect("filed");
        assert_eq!(key, "codex:acct_1", "the account the harness named wins");
    });
}

#[test]
fn an_accounts_row_is_found_by_its_key_in_the_accounts_the_door_lists() {
    run(plain, |_, c| async move {
        assert!(D::row(&*c, "claude@here").await.expect("row").is_none(), "no limit, no sign-in: no row");
        U::file_limit(&*c, reading("claude@here", "claude", "Claude Code signed in on zingzys-mac", "own", "here", None))
            .await
            .expect("filed");
        let row = D::row(&*c, "claude@here").await.expect("row").expect("a row");
        assert_eq!(row.agent, "claude");
    });
}

// StatusCtx (status.ts:9-182).

#[test]
fn the_status_ctx_reads_the_clock_the_wall_the_owner_and_the_places_word() {
    let clock = FakeClock::new(7_000);
    let fake = clock.clone();
    run(
        move |dir| RuntimeOptions { clock: Some(fake.clock()), ..plain(dir) },
        |rt, c| async move {
            assert_eq!(S::now(&*c), 7_000, "clock.now()");
            let wall = S::wall_now(&*c);
            let system = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("time").as_millis() as i64;
            assert!((system - wall).abs() < 5_000, "Date.now() is the system's, apart from the clock: {wall} against {system}");
            let owner = rt.owner().await.expect("an owner");
            assert_eq!(S::owner(&*c), owner, "state.owner after boot");
            assert_eq!(S::places_wired(&*c), "default", "ctx.places.wired");
        },
    );
}

#[test]
fn the_store_the_status_ctx_reads_and_writes_is_the_cores_store() {
    run(plain, |_, c| async move {
        assert_eq!(S::store_get(&*c, "probe", "a").await.expect("get"), None);
        S::store_put(&*c, "probe", "a", json!({ "n": 1 })).await.expect("put");
        S::store_put(&*c, "probe", "b", json!({ "n": 2 })).await.expect("put");
        assert_eq!(S::store_get(&*c, "probe", "a").await.expect("get"), Some(json!({ "n": 1 })));
        assert_eq!(S::store_list(&*c, "probe").await.expect("list").len(), 2);
        S::store_delete(&*c, "probe", "a").await.expect("delete");
        assert_eq!(S::store_get(&*c, "probe", "a").await.expect("get"), None);
        assert_eq!(D::store(&*c).get("probe", "b").await.expect("get"), Some(json!({ "n": 2 })), "UsageResetDeps reads the same one");
    });
}

#[test]
fn a_live_entry_is_found_by_its_id_and_the_projects_held_are_the_cores() {
    run(plain, |_, c| async move {
        assert!(S::live(&*c, "w_1").is_none());
        assert!(!S::boot_work_has(&*c, "w_1"));
        let entry = put_live(&c, record("w_1", json!({}))).await;
        assert!(Rc::ptr_eq(&S::live(&*c, "w_1").expect("held"), &entry));
        assert_eq!(S::held(&*c).len(), 1, "RulesArea::held");
        assert!(!S::project_held(&*c, "p_1"));
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web", "here"));
        assert!(S::project_held(&*c, "p_1"));
    });
}

#[test]
fn a_view_of_a_record_is_the_workspace_view_as_a_json_object() {
    run(plain, |_, c| async move {
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web", "here"));
        let view = S::view(&*c, &record("w_1", json!({}))).expect("a view");
        assert_eq!(view.get("id"), Some(&json!("w_1")));
        assert_eq!(view.get("name"), Some(&json!("alpha")));
        assert_eq!(view.get("phase"), Some(&json!("running")));
        assert_eq!(view["project"]["name"], json!("web"));
        assert_eq!(
            S::view(&*c, &record("w_2", json!({ "project": "p_gone" }))).expect_err("refused").message,
            "no project \"p_gone\"; this host holds web",
            "a project this host does not hold is no view: records.ts:88 throws, and so does the trait now"
        );
    });
}

#[test]
fn the_rate_follows_the_backend_of_the_records_kind_and_zero_where_the_backend_prices_nothing() {
    run(plain, |_, c| async move {
        // FakeBackend: cpu 2 * 0.02 + 4 GB * 0.01 (fake-backend.ts:57).
        assert_eq!(S::rate_usd_per_hour(&*c, &record("w_1", json!({}))), 0.08);
    });
}

#[test]
fn a_rows_words_come_from_the_usage_area_and_the_idle_policy() {
    run(plain, |_, c| async move {
        let entry = put_live(&c, record("w_1", json!({}))).await;
        assert_eq!(S::row_reason(&*c, &entry), None);
        entry.borrow_mut().wake_said = Some("the wake's own line".into());
        assert_eq!(S::row_reason(&*c, &entry).as_deref(), Some("the wake's own line"), "usage.ts:44");
        assert_eq!(S::idle_at(&*c, "w_1"), None, "no idle window running");
        assert_eq!(S::away_line(&*c, &record("w_1", json!({}))), None);
        assert_eq!(S::unreached_of(&*c, &entry), None);
    });
}

#[test]
fn a_record_at_a_place_the_door_holds_no_backend_or_link_to_reads_away() {
    // usage.ts:36-38: `placeDoor !== undefined && placeAway(at)` is the door wired and neither a backend nor a link to
    // the place (runtime.ts:852); the line is `absentComputer(placeDoor.nameOf(at)).sentence`.
    run(with_door, |_, c| async move {
        let away = S::away_line(&*c, &record("w_1", json!({ "place": "srv_1" })));
        assert_eq!(away, Some(absent_computer(&U::name_of(&*c, "srv_1"), None).sentence));
        assert_eq!(S::away_line(&*c, &record("w_1", json!({}))), None, "a record with no place is never away");
    });
    run(plain, |_, c| async move {
        assert_eq!(S::away_line(&*c, &record("w_1", json!({ "place": "srv_1" }))), None, "no door, nothing is away");
    });
}

#[test]
fn the_reach_the_machines_daemon_and_the_optional_reads_of_an_entry_come_off_its_kind_and_machine() {
    // waits on packet 2 only for `KindsArea::module_of`, which is merged: a cloud machine has a daemon road where it
    // offers a preview URL and no computer serves it (table.rs, kinds.ts), a folder here where this host wired one.
    run(plain, |_, c| async move {
        let entry = put_live(&c, record("w_1", json!({}))).await;
        let offers = entry.borrow().machine.offers();
        assert_eq!(S::has_daemon(&*c, &entry), offers.preview_url, "a cloud machine's daemon road is its preview URL");
        let (answers, prompt_ms, metrics, facts) = S::machine_reads(&*c, &entry);
        assert_eq!(
            (answers, metrics, facts),
            (offers.daemon_answers, offers.metrics, offers.facts),
            "which optional members the machine offers"
        );
        assert_eq!(prompt_ms, entry.borrow().machine.daemon_answers_prompt_ms().map(|ms| ms as f64));
        let expect = if offers.preview_url || offers.daemon_answers { "reachable" } else { "unsupported" };
        assert_eq!(S::reach_of(&*c, &entry), expect, "an entry the poll has not marked reads off what its machine can answer");
    });
    run(
        |dir| RuntimeOptions { local: Some(local_wiring(true)), ..plain(dir) },
        |_, c| async move {
            let entry = put_live(&c, record("w_1", json!({ "kind": "local" }))).await;
            assert!(S::has_daemon(&*c, &entry), "every folder here shares this computer's daemon where the host wired a road");
            assert_eq!(S::reach_of(&*c, &entry), "reachable");
        },
    );
    run(
        |dir| RuntimeOptions { local: Some(local_wiring(false)), ..plain(dir) },
        |_, c| async move {
            let entry = put_live(&c, record("w_1", json!({ "kind": "local" }))).await;
            assert!(!S::has_daemon(&*c, &entry), "no daemon road wired here, none to reach");
        },
    );
}

#[test]
fn the_polled_reach_is_set_per_workspace_and_deleted_with_it() {
    run(plain, |_, c| async move {
        S::polled_reach_set(&*c, "w_1", "m_1", "reachable");
        let entry = put_live(&c, record("w_1", json!({}))).await;
        // `reach_of` reads the polled reach while the machine id still matches (reach.ts:373).
        assert_eq!(S::reach_of(&*c, &entry), "reachable");
        S::polled_reach_set(&*c, "w_1", "m_1", "slow");
        assert_eq!(S::reach_of(&*c, &entry), "slow", "the poll's latest word");
        S::polled_reach_delete(&*c, "w_1");
        assert_ne!(S::reach_of(&*c, &entry), "slow");
    });
}

#[test]
fn a_gone_reading_is_settled_by_the_machines_area_against_what_the_provider_still_holds() {
    // machines.ts adoptGone: a record already gone is left (the early return), and a gone reading over a machine the
    // provider still holds as running leaves the record running.
    run(plain, |_, c| async move {
        let gone = put_live(&c, record("w_1", json!({ "phase": "gone" }))).await;
        S::adopt_gone(&*c, &gone, "the provider says the machine is gone").await.expect("adopted");
        assert_eq!(gone.borrow().record.borrow().phase, wsp_wire::views::workspace::WorkspacePhase::Gone);
        let alive = put_live(&c, record("w_2", json!({ "machineId": "m_2" }))).await;
        S::adopt_gone(&*c, &alive, "the provider says the machine is gone").await.expect("adopted");
        assert_eq!(
            alive.borrow().record.borrow().phase,
            wsp_wire::views::workspace::WorkspacePhase::Running,
            "the machine answers: the verdict falls"
        );
    });
}

#[test]
fn the_daemon_callbacks_take_the_reach_and_the_status_as_words() {
    // `revive_daemon`, `offer_daemon_again` and `read_version_again` parse the word and the status the poll hands them
    // and hand them to the daemon area, which does nothing for a machine with no daemon to offer (daemon.ts:767-862).
    run(plain, |_, c| async move {
        let entry = put_live(&c, record("w_1", json!({}))).await;
        let status = json!({ "id": "w_1", "machineId": "m_1", "phase": "running", "reach": { "state": "reachable" } });
        S::revive_daemon(&*c, &entry, "reachable");
        S::offer_daemon_again(&*c, &entry, &status);
        S::read_version_again(&*c, &entry, &status);
    });
}

#[test]
fn a_name_the_create_would_refuse_is_refused_in_its_words() {
    run(plain, |_, c| async move {
        assert_eq!(S::name_refusal(&*c, "alpha"), None);
        assert!(S::name_refusal(&*c, "").is_some(), "an empty name is no name");
    });
}

#[test]
fn the_golden_head_is_none_before_any_golden_was_made() {
    // waits on packet 14: `GoldenArea::get` reads through `ready` and the images' roads.
    run(plain, |_, c| async move {
        assert_eq!(S::golden_head(&*c).await, None);
    });
}

#[test]
fn a_provider_row_is_lost_by_the_engines_reading_and_this_hosts_owner() {
    run(plain, |_, c| async move {
        let owner = S::owner(&*c);
        let labels = |extra: &[(&str, &str)]| {
            let mut l: IndexMap<String, String> =
                IndexMap::from([("wsp".to_string(), "1".to_string()), ("wsp-owner".to_string(), owner.clone())]);
            for (k, v) in extra {
                l.insert(k.to_string(), v.to_string());
            }
            l
        };
        let now = S::wall_now(&*c);
        let own = ListedRow { id: "m_1".into(), state: "running".into(), labels: labels(&[("createdAt", "2026-01-01T00:00:00.000Z")]) };
        assert!(S::lost_workspace(&*c, &own, &owner, now), "an own, running, old row is lost");
        let other = ListedRow {
            id: "m_2".into(),
            state: "running".into(),
            labels: labels(&[("createdAt", "2026-01-01T00:00:00.000Z"), ("wsp-owner", "someone-else")]),
        };
        assert!(!S::lost_workspace(&*c, &other, &owner, now), "another owner's is not");
        let gone = ListedRow { id: "m_3".into(), state: "gone".into(), labels: labels(&[]) };
        assert!(!S::lost_workspace(&*c, &gone, &owner, now), "only a running or paused one");
    });
}

// StatusCtx: the confirming read, the stop and the recording (status.ts:140-178); sweep-places.test.ts.

fn lost_row(id: &str, owner: &str, workspace: &str) -> ListedRow {
    ListedRow {
        id: id.into(),
        state: "running".into(),
        labels: IndexMap::from([
            ("wsp".to_string(), "1".to_string()),
            ("wsp-owner".to_string(), owner.to_string()),
            ("wsp-workspace".to_string(), workspace.to_string()),
            ("wsp-golden".to_string(), "snap_g".to_string()),
            ("createdAt".to_string(), "2026-01-01T00:00:00.000Z".to_string()),
        ]),
    }
}

#[test]
fn a_machine_the_provider_lists_is_confirmed_with_its_state_and_the_size_it_was_built() {
    run(plain, |_, c| async move {
        let confirmed = S::confirm(&*c, "default", "fk_1").await.expect("confirmed");
        assert_eq!(confirmed.state, "running", "machine.seen?.state ?? await machine.state()");
        assert_eq!(confirmed.stream_url, None);
        assert_eq!(confirmed.size, json!({ "cpu": 4, "memMb": 8192 }), "sizeBuilt(shapeOf(machine), the backend's default size)");
    });
}

#[test]
fn a_machine_that_is_stopped_for_being_unrecorded_is_asked_to_go_and_confirmed_gone() {
    run(plain, |_, c| async move {
        S::stop_unrecorded(&*c, "default", "fk_1").await.expect("stopped");
    });
}

#[test]
fn a_box_this_host_cannot_name_is_named_at_every_sweep_with_the_time_it_goes() {
    // sweep-places.test.ts:40.
    let clock = FakeClock::new(1_900_000_000_000);
    let fake = clock.clone();
    run(
        move |dir| RuntimeOptions { clock: Some(fake.clock()), ..plain(dir) },
        |_, c| async move {
            let owner = S::owner(&*c);
            let area = StatusArea::new(c.clone());
            let mut known = std::collections::BTreeSet::new();
            let mut failed = Vec::new();
            let (adopted, reaped) =
                area.adopt_lost(&[lost_row("fk_lost", &owner, "w_unknown")], &mut known, &mut failed, "default").await.expect("swept");
            assert!(adopted.is_empty() && reaped.is_empty());
            assert!(known.contains("fk_lost"), "the engine spares it this sweep");
            assert_eq!(failed.len(), 1);
            let goes = wsp_words::js_date::iso_string(1_900_000_000_000.0 + wsp_core::types::internal::UNRECORDED_DELETE_MS as f64)
                .expect("a date");
            assert_eq!(
            failed[0].message,
            format!("not recorded: no workspace of this host is w_unknown, so nothing here reaches this running machine of yours; deleted at {goes}, a day after this host first saw it")
        );
            let seen = S::store_get(&*c, wsp_core::types::internal::UNRECORDED, "fk_lost")
                .await
                .expect("get")
                .expect("its first sighting is kept");
            assert_eq!(seen["machineId"], json!("fk_lost"));
            assert_eq!(seen["place"], json!("default"));
        },
    );
}

#[test]
fn a_box_past_its_delay_is_deleted_and_named_once_more_as_it_goes() {
    // sweep-places.test.ts:54.
    let clock = FakeClock::new(1_900_000_000_000);
    let fake = clock.clone();
    run(
        move |dir| RuntimeOptions { clock: Some(fake.clock()), ..plain(dir) },
        |_, c| async move {
            let owner = S::owner(&*c);
            let area = StatusArea::new(c.clone());
            let row = lost_row("fk_lost", &owner, "w_unknown");
            let mut known = std::collections::BTreeSet::new();
            let mut failed = Vec::new();
            area.adopt_lost(std::slice::from_ref(&row), &mut known, &mut failed, "default").await.expect("first sweep");
            clock.advance(wsp_core::types::internal::UNRECORDED_DELETE_MS as i64 + 1);
            let (mut known, mut failed) = (std::collections::BTreeSet::new(), Vec::new());
            let (adopted, reaped) = area.adopt_lost(&[row], &mut known, &mut failed, "default").await.expect("second sweep");
            assert!(adopted.is_empty());
            assert_eq!(reaped.len(), 1, "deleted at the first sweep past its delay: {failed:?}");
            assert_eq!(reaped[0].id, "fk_lost");
            assert!(failed.is_empty());
            assert!(
                S::store_get(&*c, wsp_core::types::internal::UNRECORDED, "fk_lost").await.expect("get").is_none(),
                "its sighting is let go"
            );
        },
    );
}

#[test]
fn a_lost_record_of_a_workspace_this_host_still_names_is_recorded_again_not_killed() {
    // sweep-places.test.ts:131: the record is rebuilt off the name document and the confirming read.
    run(plain, |rt, c| async move {
        let owner = S::owner(&*c);
        let (created, _off) = listen(&rt, "workspace.created");
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web", "here"));
        S::store_put(&*c, wsp_core::types::internal::WORKSPACE_NAMES, "w_lost", json!({ "name": "alpha", "project": "p_1" }))
            .await
            .expect("put");
        let area = StatusArea::new(c.clone());
        let mut known = std::collections::BTreeSet::new();
        let mut failed = Vec::new();
        let (adopted, reaped) =
            area.adopt_lost(&[lost_row("fk_lost", &owner, "w_lost")], &mut known, &mut failed, "default").await.expect("swept");
        assert!(reaped.is_empty() && failed.is_empty(), "{failed:?}");
        assert_eq!(adopted.len(), 1);
        assert_eq!((adopted[0].workspace_id.as_str(), adopted[0].name.as_str()), ("w_lost", "alpha"));
        let entry = S::live(&*c, "w_lost").expect("attached to the live map");
        assert_eq!(entry.borrow().record.borrow().machine_id, "fk_lost");
        assert_eq!(entry.borrow().record.borrow().place, None, "the provider this host forks on is no place");
        let stored = S::store_get(&*c, wsp_core::types::internal::WORKSPACES, "w_lost").await.expect("get").expect("persisted");
        assert_eq!(stored["machineId"], json!("fk_lost"));
        assert_eq!(created.borrow().len(), 1, "workspace.created rides the bus");
        assert_eq!(created.borrow()[0]["workspace"]["id"], json!("w_lost"));
    });
}

#[test]
fn a_lost_machine_whose_project_this_host_cannot_name_is_reported_not_recorded() {
    run(plain, |_, c| async move {
        let owner = S::owner(&*c);
        S::store_put(&*c, wsp_core::types::internal::WORKSPACE_NAMES, "w_lost", json!({ "name": "alpha", "project": "p_gone" }))
            .await
            .expect("put");
        let area = StatusArea::new(c.clone());
        let mut known = std::collections::BTreeSet::new();
        let mut failed = Vec::new();
        let (adopted, _) =
            area.adopt_lost(&[lost_row("fk_lost", &owner, "w_lost")], &mut known, &mut failed, "default").await.expect("swept");
        assert!(adopted.is_empty());
        assert_eq!(
            failed[0].message,
            "not recorded: this host holds no project for it, and a workspace is one project's copy; it is a machine of yours still running"
        );
    });
}

/// The provider this host forks on is `default`; `box` is a second place with a backend of its own.
struct TwoPlaces(Arc<dyn MachineBackend>);

impl wsp_core::types::wiring::PlaceBackends for TwoPlaces {
    fn wired(&self) -> String {
        "default".into()
    }
    fn backend(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        matches!(place, "default" | "box").then(|| self.0.clone())
    }
    fn list(&self) -> Vec<String> {
        vec!["default".into(), "box".into()]
    }
}

#[test]
fn a_confirming_read_asks_the_backend_of_the_place_the_sweep_names() {
    // status.ts:140-153: `at.backend.get(row.id)` where `at` is the provider the listing came from.
    run(
        |dir| RuntimeOptions { places: Some(Rc::new(TwoPlaces(backend()))), ..plain(dir) },
        |_, c| async move {
            assert_eq!(S::places_wired(&*c), "default");
            assert_eq!(S::confirm(&*c, "box", "fk_1").await.expect("confirmed").state, "running");
            assert!(matches!(S::confirm(&*c, "nowhere", "fk_1").await, Err(wsp_account::account::status::ReadFailure::Failed(_))));
            assert!(matches!(S::stop_unrecorded(&*c, "nowhere", "fk_1").await, Err(wsp_account::account::status::ReadFailure::Failed(_))));
        },
    );
}

// The status tracker the context builds (status.ts:11-69) and `status.list` (status.test.ts).

#[test]
fn the_tracker_the_context_builds_lists_a_running_workspace_with_the_size_and_the_rate_off_its_backend() {
    // status.test.ts:100 "enriches views with machine state, reach, size, and rate from backend pricing".
    run(plain, |_, c| async move {
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web", "here"));
        put_live(&c, record("w_1", json!({}))).await;
        let tracker = c.usage_status_tracker();
        let statuses = tracker.list(Default::default()).await.expect("a list");
        assert_eq!(statuses.len(), 1);
        let status = serde_json::to_value(&statuses[0]).expect("a status");
        assert_eq!(status["name"], json!("alpha"));
        assert_eq!(status["phase"], json!("running"));
        assert_eq!(status["machineState"], json!("running"));
        assert_eq!(status["size"], json!({ "cpu": 2, "memMb": 4096 }));
        assert_eq!(status["rateUsdPerHour"], json!(0.08));
    });
}

#[test]
fn the_tracker_maps_a_napping_workspace_without_probing_it() {
    // status.test.ts:149 "maps napping and gone machines without probing".
    run(plain, |_, c| async move {
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web", "here"));
        let entry = put_live(&c, record("w_1", json!({ "phase": "napping" }))).await;
        let machine = entry.borrow().machine.clone();
        machine.pause(None).await.expect("paused");
        let statuses = c.usage_status_tracker().list(Default::default()).await.expect("a list");
        let status = serde_json::to_value(&statuses[0]).expect("a status");
        assert_eq!(status["machineState"], json!("paused"));
        assert_eq!(status["reach"]["state"], json!("napping"));
    });
}

#[test]
fn a_row_carries_what_the_runtime_says_about_it_between_polls() {
    // status.ts:13-43: the reason, the wake ask and the daemon's callbacks ride the row the tracker builds.
    run(plain, |_, c| async move {
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web", "here"));
        let entry = put_live(&c, record("w_1", json!({}))).await;
        entry.borrow_mut().wake_said = Some("the wake's own line".into());
        let statuses = c.usage_status_tracker().list(Default::default()).await.expect("a list");
        let status = serde_json::to_value(&statuses[0]).expect("a status");
        assert_eq!(status["reason"], json!("the wake's own line"));
    });
}

#[test]
fn an_empty_runtime_has_no_statuses() {
    run(plain, |_, c| async move {
        assert!(c.usage_status_tracker().list(Default::default()).await.expect("a list").is_empty());
    });
}

#[test]
fn a_poll_sets_the_reach_of_the_machine_the_record_holds_and_ignores_one_since_replaced() {
    // status.ts:49-61 `onPolled`, run by the area over the context.
    run(plain, |_, c| async move {
        let entry = put_live(&c, record("w_1", json!({}))).await;
        let area = StatusArea::new(c.clone());
        let status = |machine: &str, phase: &str, reach: &str| json!({ "id": "w_1", "machineId": machine, "phase": phase, "reach": { "state": reach } });
        area.on_polled(&[status("m_other", "running", "slow")]);
        assert_ne!(S::reach_of(&*c, &entry), "slow", "a sighting of a machine since replaced says nothing about the one under the record");
        area.on_polled(&[status("m_1", "running", "slow")]);
        assert_eq!(S::reach_of(&*c, &entry), "slow", "a running record's polled reach is kept");
        area.on_polled(&[status("m_1", "napping", "napping")]);
        assert_ne!(S::reach_of(&*c, &entry), "slow", "a record that is not running has none kept");
    });
}
