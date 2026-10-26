// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 11 (#2476): the context's bodies for `AgentsCtx`, `LaunchCtx` and `ThreadsCtx` (124
//! methods), read against the TS at 65f6b89c7: runtime.ts:463-465, agents.ts:26-710, launch.ts:41-144,
//! threads.ts:70-1186 and the 13 TS tests the plan names (thread-defaults, thread-keeps-agent, thread-keeps-picks,
//! thread-keeps-process, thread-warm-agent, agents-brokered, vault, question-env, thread-marks, thread-head,
//! thread-lines, forget, replaces).
//!
//! Those 13 tests drive a whole runtime through `createOn` and `rt.sessions.start`; a Rust test cannot replace an
//! area, so each method is read at its own seam on the runtime `create_runtime` builds over `FakeBackend`, seeded
//! through the handles the traits hand out. The tests at the end of the file are the whole-runtime ones: each says
//! which packets' bodies it waits on ("waits: packet N") and is red until they land.
use std::cell::RefCell;
use std::future::Future;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::port::Env;
use wsp_core::agent_setup::AGENT_SETUPS;
use wsp_core::clock::FakeClock;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::internal::{SessionEntry, ThreadRecord};
use wsp_core::types::wiring::{LiveWorkspace, LocalWiring, Platform, RuntimeOptions, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_machines::machines::machines::{Entry, MachinesRuntime, Row};
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::{memory_store, Store};
use wsp_threads::threads::agents::AgentsCtx;
use wsp_threads::threads::launch::LaunchCtx;
use wsp_threads::threads::threads::ThreadsCtx;
use wsp_wire::types::thread_defaults::{AgentDefaults, Fields, ProjectOverrides};
use wsp_wire::views::session::SessionView;
use wsp_words::exit::HostError;

// ----------------------------------------------------------------------------------------------------------------
// Fixtures

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

fn options() -> RuntimeOptions {
    RuntimeOptions { backend: Some(backend()), store: Some(Rc::new(memory_store())), ..Default::default() }
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

fn with_door() -> RuntimeOptions {
    RuntimeOptions { place_links: Some(Rc::new(wiring())), ..options() }
}

/// This computer's wiring over a folder, as the TS tests' `fakeLocal` wires it.
fn local_in(root: &Path) -> LocalWiring {
    let folder = root.to_string_lossy().to_string();
    let home_root = folder.clone();
    LocalWiring {
        backend: Arc::new(LocalBackend::new(LocalBackendOptions { root: root.to_path_buf(), env: None })),
        folder: folder.clone(),
        exec_stream: Rc::new(|_, _| panic!("no exec stream in this test")),
        home: Rc::new(move |agent| format!("{home_root}/.{agent}")),
        home_dir: folder.clone(),
        roots_path: format!("{folder}/roots"),
        env: Rc::new(|| IndexMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())])),
        daemon_road: None,
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: None,
        platform: Platform::Linux,
        close: None,
    }
}

fn local_opts(root: &Path) -> RuntimeOptions {
    RuntimeOptions { local: Some(local_in(root)), ..options() }
}

/// The runtime on a throwaway state, inside the task set the owner thread runs, and its context.
fn run<Fut: Future<Output = ()>>(opts: RuntimeOptions, f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let runtime = create_runtime(opts);
        let ctx = runtime.context();
        f(runtime, ctx).await
    });
}

fn ag(c: &Rc<Ctx>) -> &dyn AgentsCtx {
    &**c
}
fn lc(c: &Rc<Ctx>) -> &dyn LaunchCtx {
    &**c
}
fn tc(c: &Rc<Ctx>) -> &dyn ThreadsCtx {
    &**c
}

fn record(id: &str, extra: Value) -> WorkspaceRecord {
    let mut base = json!({
        "id": id, "name": "pricing page", "machineId": "m_1", "phase": "running", "golden": "g",
        "createdAt": "2026-10-26T00:00:00.000Z", "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 },
        "firstLife": false,
    });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a workspace record")
}

/// A live entry over `record` on a fake machine, put in the runtime's live map through the machines' own handle.
async fn put_live(c: &Rc<Ctx>, record: WorkspaceRecord) -> Entry {
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
    MachinesRuntime::live(&**c).borrow_mut().insert(id, entry.clone());
    entry
}

fn view(id: &str, workspace: &str, extra: Value) -> SessionView {
    let mut base = json!({ "id": id, "workspaceId": workspace, "harness": "claude", "status": "completed" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a session view")
}

fn row(view: SessionView) -> Row {
    Rc::new(RefCell::new(SessionEntry {
        turn_id: format!("turn-{}", view.id),
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
    }))
}

fn thread(workspace: &str, extra: Value) -> ThreadRecord {
    let mut base = json!({ "workspaceId": workspace, "harness": "claude" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a thread record")
}

/// A factory that refuses, which is all the adapters table is read for here.
fn refusing() -> HarnessAdapterFactory {
    Arc::new(|_| Err(HostError::new("no adapter in this test")))
}

fn same<T: ?Sized>(a: &Rc<T>, b: &Rc<T>) -> bool {
    Rc::as_ptr(a) as *const () == Rc::as_ptr(b) as *const ()
}

/// The setups the person kept for agents, as the store holds them, read by `ready` (agent-setup.ts:82).
async fn kept_setup(store: &Rc<dyn Store>, place: &str, agent: &str, setup: Value) {
    store.put(AGENT_SETUPS, &format!("{place}:{agent}"), setup).await.expect("a put");
}

fn env_of(pairs: &[(&str, &str)]) -> Env {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

struct Ready(bool);

#[async_trait::async_trait(?Send)]
impl wsp_core::types::wiring::Broker for Ready {
    fn ready(&self) -> bool {
        self.0
    }
    async fn codex_claims(&self) -> Result<Option<wsp_adapters::pending::CodexClaims>, HostError> {
        Ok(None)
    }
}

// ----------------------------------------------------------------------------------------------------------------
// AgentsCtx (agents.ts:26-710, runtime.ts:463): what the agents area reads of the runtime.

#[test]
fn agents_clock_and_store_are_the_ones_the_host_wired() {
    let fake = FakeClock::new(1_700_000_000_000);
    let store: Rc<dyn Store> = Rc::new(memory_store());
    let opts = RuntimeOptions { clock: Some(fake.clock()), store: Some(store.clone()), ..options() };
    run(opts, |_, c| async move {
        assert_eq!(ag(&c).clock().now(), 1_700_000_000_000);
        fake.advance(7);
        assert_eq!(ag(&c).clock().now(), 1_700_000_000_007);
        store.put("things", "t1", json!({ "a": 1 })).await.expect("a put");
        assert_eq!(ag(&c).store().get("things", "t1").await.expect("a get"), Some(json!({ "a": 1 })));
    });
}

#[test]
fn agents_bus_is_the_one_every_area_emits_on() {
    run(options(), |_, c| async move {
        let heard = Rc::new(RefCell::new(0));
        let seen = heard.clone();
        let _off = ag(&c).bus().on("*", Rc::new(move |_: &wsp_wire::wire::events_union::EventUnion| (*seen.borrow_mut() += 1, Ok(())).1));
        let gone = json!({ "type": "workspace.gone", "workspaceId": "w_1", "machineId": "m_1", "reason": "x" });
        wsp_threads::threads::transcripts::TranscriptsCtx::emit(&*c, wsp_wire::wire::events_union::parse_event(&gone).expect("an event"))
            .expect("delivered");
        assert_eq!(*heard.borrow(), 1);
        let _off = tc(&c).bus().on("workspace.gone", Rc::new(|_: &wsp_wire::wire::events_union::EventUnion| Ok(())));
    });
}

#[test]
fn agents_sessions_thread_records_and_entry_are_the_shared_maps() {
    run(options(), |_, c| async move {
        assert!(same(&ag(&c).sessions(), &tc(&c).sessions()));
        assert!(same(&ag(&c).thread_records(), &tc(&c).thread_records()));
        assert!(ag(&c).entry("w_1").is_none());
        let put = put_live(&c, record("w_1", json!({}))).await;
        assert!(Rc::ptr_eq(&ag(&c).entry("w_1").expect("the live entry"), &put));
        assert!(ag(&c).entry("w_2").is_none());
        ag(&c).sessions().borrow_mut().insert("s_1".into(), row(view("s_1", "w_1", json!({}))));
        assert!(tc(&c).sessions().borrow().contains_key("s_1"));
        ag(&c).thread_records().borrow_mut().insert("t_1".into(), thread("w_1", json!({})));
        assert!(tc(&c).thread_records().borrow().contains_key("t_1"));
    });
}

#[test]
fn agents_closing_reads_the_runtime_state() {
    run(options(), |_, c| async move {
        assert!(!ag(&c).closing());
        wsp_account::account::channels::ChannelsCtx::state(&*c).with(|s| s.closing = true);
        assert!(ag(&c).closing());
        assert!(lc(&c).closing());
        assert!(tc(&c).closing());
    });
}

#[test]
fn agents_adapters_are_the_hosts_in_their_order() {
    let adapters: IndexMap<String, HarnessAdapterFactory> =
        IndexMap::from([("codex".to_string(), refusing()), ("claude".to_string(), refusing()), ("cursor".to_string(), refusing())]);
    run(RuntimeOptions { adapters, ..options() }, |_, c| async move {
        assert_eq!(ag(&c).adapters().keys().cloned().collect::<Vec<_>>(), ["codex", "claude", "cursor"]);
    });
    run(options(), |_, c| async move {
        assert!(ag(&c).adapters().is_empty());
    });
}

#[test]
fn agents_local_homes_are_the_local_wirings_and_none_where_there_is_no_local() {
    let root = std::env::temp_dir().join("wsp-agents-local-home");
    let folder = root.to_string_lossy().to_string();
    run(local_opts(&root), |_, c| async move {
        assert_eq!(ag(&c).local_home_dir().as_deref(), Some(folder.as_str()));
        assert_eq!(ag(&c).local_home("claude"), Some(format!("{folder}/.claude")));
        assert_eq!(ag(&c).local_home("codex"), Some(format!("{folder}/.codex")));
    });
    run(options(), |_, c| async move {
        assert_eq!(ag(&c).local_home_dir(), None);
        assert_eq!(ag(&c).local_home("claude"), None);
    });
}

#[test]
fn agents_process_home_is_the_home_this_process_runs_in_and_state_path_the_option() {
    run(RuntimeOptions { state_path: Some("/srv/wsp/state.db".into()), ..options() }, |_, c| async move {
        let home = std::env::var("HOME").expect("a HOME in the test environment");
        assert_eq!(ag(&c).process_home(), home);
        assert_eq!(ag(&c).state_path().as_deref(), Some("/srv/wsp/state.db"));
    });
    run(options(), |_, c| async move {
        assert_eq!(ag(&c).state_path(), None);
    });
}

#[test]
fn agents_setups_are_what_the_person_kept_for_an_agent_on_a_computer() {
    // agent-setup.ts:82-101: `off`, `get(..)?.configDir` and `launchOf` read the setups the store kept, loaded by `ready`.
    let store: Rc<dyn Store> = Rc::new(memory_store());
    let opts = RuntimeOptions { store: Some(store.clone()), ..options() };
    run(opts, |_, c| async move {
        kept_setup(&store, "here", "claude", json!({ "on": false })).await;
        kept_setup(
            &store,
            "here",
            "codex",
            json!({ "program": "/opt/codex", "args": ["--fast"], "configDir": "/srv/codex-home", "env": { "A_KEY": "b" } }),
        )
        .await;
        ag(&c).ready().await.expect("a host that has started");
        assert!(ag(&c).setup_off("here", "claude"));
        assert!(!ag(&c).setup_off("here", "codex"));
        assert!(!ag(&c).setup_off("elsewhere", "claude"));
        assert_eq!(ag(&c).setup_config_dir("here", "codex").as_deref(), Some("/srv/codex-home"));
        assert_eq!(ag(&c).setup_config_dir("here", "claude"), None);
        let launch = ag(&c).setup_launch("here", "codex");
        assert_eq!(launch.env.get("A_KEY").map(String::as_str), Some("b"));
        let own = launch.launch.expect("the launch the person set");
        assert_eq!(own.program.as_deref(), Some("/opt/codex"));
        assert_eq!(own.args, Some(vec!["--fast".to_string()]));
        let none = ag(&c).setup_launch("elsewhere", "codex");
        assert!(none.env.is_empty() && none.launch.is_none());
    });
}

#[test]
fn agents_place_exec_goes_through_the_door_and_says_so_where_there_is_none() {
    run(options(), |_, c| async move {
        let said = ag(&c).place_exec("here", "true", 1000).await.expect_err("no door is wired");
        assert_eq!(said.message, wsp_words::words::computer::no_kind_line("place"));
    });
    run(with_door(), |_, c| async move {
        assert!(ag(&c).place_exec("p_nowhere", "true", 1000).await.is_err());
    });
}

#[test]
fn agents_place_names_are_the_doors_and_a_places_id_is_the_records_own() {
    run(with_door(), |_, c| async move {
        assert_eq!(ag(&c).place_name("here"), "zingzys-mac");
        assert_eq!(tc(&c).place_name("here"), "zingzys-mac");
    });
    run(options(), |_, c| async move {
        let cloud = record("w_1", json!({ "place": "p_9" }));
        assert_eq!(ag(&c).place_id_of(&cloud).as_deref(), Some("p_9"));
        assert_eq!(tc(&c).place_id_of(&cloud).as_deref(), Some("p_9"));
        let local = record("w_2", json!({ "kind": "local" }));
        assert_eq!(ag(&c).place_id_of(&local).as_deref(), Some("here"));
    });
}

#[test]
fn agents_at_provider_reads_the_places_the_host_wired() {
    let backend = backend();
    let places = wsp_core::types::wiring::wired_place(wsp_core::types::wiring::WiredId::Fixed("solari".into()), backend.clone());
    let opts = RuntimeOptions { places: Some(places.clone()), backend: Some(backend), ..options() };
    run(opts, |_, c| async move {
        for place in [None, Some("solari"), Some("here"), Some("p_unknown")] {
            assert_eq!(ag(&c).at_provider(place), wsp_core::types::wiring::at_provider(place, &*places), "{place:?}");
        }
    });
}

#[test]
fn agents_place_agent_home_is_the_guests_login_home_under_the_given_home() {
    // kinds.ts:235: where the door has no login for the place, `underLoginHome(home, cloudHome(id))`.
    run(options(), |_, c| async move {
        let home = ag(&c).place_agent_home("p_1", "/root", "claude").expect("a home");
        assert!(home.starts_with("/root"), "{home}");
        assert!(home.ends_with(".claude") || home.contains("claude"), "{home}");
    });
}

#[test]
fn agents_ready_waits_for_the_start_and_answers_ok() {
    run(options(), |_, c| async move {
        ag(&c).ready().await.expect("the start");
        lc(&c).ready().await.expect("the start");
    });
}

#[test]
fn agents_the_kinds_modules_answer_for_the_entrys_kind() {
    let root = std::env::temp_dir().join("wsp-agents-kinds");
    let folder = root.to_string_lossy().to_string();
    run(local_opts(&root), |_, c| async move {
        let cloud = put_live(&c, record("w_1", json!({}))).await;
        let local = put_live(&c, record("w_2", json!({ "kind": "local", "folder": folder }))).await;
        // `home` is the kind's: this computer's agent folder for a local kind, the guest's for a cloud one.
        assert_eq!(ag(&c).kind_home(&local, "claude"), format!("{folder}/.claude"));
        assert_ne!(ag(&c).kind_home(&cloud, "claude"), format!("{folder}/.claude"));
        // `serverValues`: a cloud machine takes the servers' values by variable, this computer by file or launch.
        assert_eq!(ag(&c).kind_server_values(&cloud), Some(wsp_core::types::internal::ServerValuesRoad::Environment));
        assert_ne!(ag(&c).kind_server_values(&local), Some(wsp_core::types::internal::ServerValuesRoad::Environment));
        // `endThread` is a kind's own only where a computer groups a thread's processes: neither of these does.
        assert!(!ag(&c).kind_ends_threads(&cloud));
        assert!(!ag(&c).kind_ends_threads(&local));
        assert_eq!(
            ag(&c).kind_env(&local, Some("claude"), &|id| format!("/h/{id}")).get("PATH").map(String::as_str),
            Some("/usr/bin:/bin")
        );
        assert!(!ag(&c).kind_login_stands(&local, "claude"));
    });
}

#[test]
fn agents_a_computers_kind_says_whether_its_agents_run_in_a_folder() {
    run(options(), |_, c| async move {
        assert!(ag(&c).computer_runs_in_folder("here"));
        assert!(!ag(&c).computer_runs_in_folder("p_9"));
    });
}

#[test]
fn agents_brokered_launch_is_none_unbrokered_and_empty_without_a_token() {
    run(RuntimeOptions { broker: Some(Rc::new(Ready(true))), ..options() }, |_, c| async move {
        let cloud = put_live(&c, record("w_1", json!({}))).await;
        let launch = ag(&c).brokered_launch(&cloud, "claude", None).expect("no refusal").expect("a brokered launch");
        assert!(launch.env.is_empty() && launch.vault.is_empty());
        let token = ag(&c).brokered_launch(&cloud, "claude", Some("tok")).expect("no refusal").expect("a brokered launch");
        assert!(token.env.values().any(|v| v == "tok"), "{:?}", token.env);
        assert!(token.claude.is_some());
    });
    run(options(), |_, c| async move {
        let cloud = put_live(&c, record("w_1", json!({}))).await;
        assert!(ag(&c).brokered_launch(&cloud, "claude", Some("tok")).expect("no refusal").is_none());
        ag(&c).know_broker(&cloud, "codex").await.expect("an unbrokered workspace knows its broker at once");
    });
}

#[test]
fn agents_vault_is_the_hosts_and_a_workspaces_is_its_places() {
    let vault = Rc::new(|| env_of(&[("OPENAI_API_KEY", "sk-1")]));
    run(RuntimeOptions { vault: Some(vault), ..options() }, |_, c| async move {
        let cloud = put_live(&c, record("w_1", json!({}))).await;
        assert_eq!(ag(&c).vault(), env_of(&[("OPENAI_API_KEY", "sk-1")]));
        assert_eq!(lc(&c).vault(), env_of(&[("OPENAI_API_KEY", "sk-1")]));
        assert_eq!(tc(&c).vault(), env_of(&[("OPENAI_API_KEY", "sk-1")]));
        assert_eq!(ag(&c).vault_on(&cloud), env_of(&[("OPENAI_API_KEY", "sk-1")]));
        assert_eq!(tc(&c).vault_on(&cloud), env_of(&[("OPENAI_API_KEY", "sk-1")]));
    });
    run(options(), |_, c| async move {
        assert!(ag(&c).vault().is_empty() && lc(&c).vault().is_empty() && tc(&c).vault().is_empty());
    });
}

#[test]
fn agents_unreached_of_a_live_workspace_is_none_until_its_daemon_is_marked() {
    run(options(), |_, c| async move {
        let cloud = put_live(&c, record("w_1", json!({}))).await;
        assert_eq!(ag(&c).unreached_of(&cloud), None);
    });
}

#[test]
fn agents_exec_factory_for_is_the_kinds_stream_with_the_places_turn_limit_and_the_launchs_thread() {
    // agents.ts:643-651: the place's `turnLimitOf(record)` overlaid with the thread, `asksUntilStopped` and, for a sub
    // workspace, `killedWhy`; the kind's module makes the stream. Made here for a cloud and a sub workspace, with and
    // without a door, and each answers a factory whose runs attach on a machine that keeps its runs.
    for opts in [options(), with_door()] {
        run(opts, |_, c| async move {
            let plain = put_live(&c, record("w_1", json!({}))).await;
            let sub = put_live(&c, record("w_2", json!({ "sub": { "folder": "/root/w", "parent": "w_1" } }))).await;
            let launch =
                wsp_threads::threads::agents::TurnLaunch { thread: Some("t_1".into()), asks_until_stopped: true, killed_why: false };
            for (entry, turn) in [
                (&plain, wsp_threads::threads::agents::TurnLaunch::default()),
                (&plain, launch.clone()),
                (&sub, wsp_threads::threads::agents::TurnLaunch { killed_why: true, ..launch }),
            ] {
                let factory = ag(&c).exec_factory_for(entry, turn, None);
                let _ = factory.attaches();
            }
        });
    }
}

#[test]
fn agents_signs_in_reads_the_agents_reader_the_host_wired() {
    run(options(), |_, c| async move {
        assert!(ag(&c).signs_in(vec!["claude".into()]).await.is_none());
    });
}

#[test]
fn agents_projects_and_a_projects_folder_are_the_projects_areas() {
    run(options(), |_, c| async move {
        assert!(ag(&c).list_projects(None).await.expect("a list").is_empty());
        let said = ag(&c).resolve_project("nothing", None).await.expect_err("no such project");
        assert!(said.message.contains("nothing"), "{}", said.message);
    });
}

#[test]
fn agents_push_head_says_thread_head_for_a_thread_the_host_holds() {
    // agents.ts `pushHead`: the boot area reads the thread's facts and says them; a thread nobody holds says nothing.
    run(options(), |_, c| async move {
        tc(&c).thread_records().borrow_mut().insert("t_1".into(), thread("w_1", json!({})));
        ag(&c).sessions().borrow_mut().insert("s_1".into(), row(view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 10 }))));
        let heard = heard_on(&c, "thread.head");
        ag(&c).push_head("t_1");
        assert_eq!(heard.borrow().len(), 1);
        assert_eq!(heard.borrow()[0]["threadId"], json!("t_1"));
        assert_eq!(heard.borrow()[0]["workspaceId"], json!("w_1"));
        ag(&c).push_head("t_nobody");
        assert_eq!(heard.borrow().len(), 1);
    });
}

#[test]
fn agents_write_session_answers_what_the_write_answers() {
    use wsp_adapters::port::SessionRenameWrite;
    run(options(), |_, c| async move {
        let written = ag(&c).write_session("s_1", Box::pin(async { Ok(SessionRenameWrite::NoSession) })).await.expect("an answer");
        assert_eq!(written, SessionRenameWrite::NoSession);
        let refused =
            ag(&c).write_session("s_1", Box::pin(async { Err(HostError::new("the file is gone")) })).await.expect_err("the refusal");
        assert_eq!(refused.message, "the file is gone");
    });
}

#[test]
fn agents_a_setup_refusal_that_moved_tells_the_rows_of_that_agent_on_that_computer() {
    // boot.ts:867: a row of the harness on a workspace standing on the place is moved, and its push is said at the end of the task.
    run(options(), |_, c| async move {
        put_live(&c, record("w_1", json!({ "place": "p_9" }))).await;
        tc(&c).thread_records().borrow_mut().insert("t_1".into(), thread("w_1", json!({})));
        ag(&c).sessions().borrow_mut().insert("s_1".into(), row(view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 10 }))));
        let heard = heard_on(&c, "*");
        ag(&c).setup_refusal_moved("p_9", "claude");
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        let told = heard.borrow().iter().filter_map(|e| e["type"].as_str().map(str::to_string)).collect::<Vec<_>>();
        assert!(!told.is_empty(), "the moved row was pushed");
        let before = told.len();
        ag(&c).setup_refusal_moved("p_other", "claude");
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        assert_eq!(heard.borrow().len(), before, "another computer's rows stay");
    });
}

#[test]
fn agents_spawn_starts_the_work_now_and_lets_it_run_to_its_end() {
    run(options(), |_, c| async move {
        let ran = Rc::new(RefCell::new(Vec::<&str>::new()));
        let (first, second) = (ran.clone(), ran.clone());
        ag(&c).spawn(Box::pin(async move { first.borrow_mut().push("now") }));
        assert_eq!(*ran.borrow(), ["now"]);
        ag(&c).spawn(Box::pin(async move {
            tokio::task::yield_now().await;
            second.borrow_mut().push("later");
        }));
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        assert_eq!(*ran.borrow(), ["now", "later"]);
    });
}

// ----------------------------------------------------------------------------------------------------------------
// LaunchCtx (launch.ts:41-144, sessions.ts:247): what a thread's launch reads of the runtime.

/// An adapter that starts nothing and says whether it keeps an agent warm.
struct Stub {
    warms: bool,
}

impl wsp_adapters::port::HarnessAdapter for Stub {
    fn start(
        &self,
        _options: wsp_adapters::port::HarnessStartOptions,
    ) -> Result<Arc<dyn wsp_adapters::port::HarnessSession>, wsp_adapters::port::AdapterError> {
        Err(wsp_adapters::port::AdapterError("this adapter starts nothing".into()))
    }
    fn steers(&self) -> bool {
        false
    }
    fn warms(&self) -> bool {
        self.warms
    }
}

/// A kept agent that is never used.
struct Kept;

impl wsp_adapters::port::KeptAgent for Kept {
    fn next(
        &self,
        _turn: wsp_adapters::port::KeptTurn,
    ) -> Result<Arc<dyn wsp_adapters::port::HarnessSession>, wsp_adapters::port::AdapterError> {
        Err(wsp_adapters::port::AdapterError("unused".into()))
    }
    fn close(&self, _now: bool) -> futures::future::LocalBoxFuture<'static, ()> {
        Box::pin(async {})
    }
    fn exited(&self) -> wsp_adapters::port::SharedFuture<()> {
        futures::FutureExt::shared(Box::pin(futures::future::pending::<()>()) as futures::future::LocalBoxFuture<'static, ()>)
    }
}

fn hand(workspace: &str, harness: &str) -> wsp_threads::threads::threads::KeptHand {
    wsp_threads::threads::threads::KeptHand {
        workspace_id: workspace.into(),
        agent: Box::new(Kept),
        pid: None,
        launch: wsp_core::types::internal::KeptLaunch { fixed: format!("{{\"harness\":\"{harness}\"}}"), picks: IndexMap::new() },
        session: None,
        warm: Some(wsp_core::types::internal::WarmKeep { harness: harness.into(), claimed: false }),
        turn_token: "tok".into(),
        scope_device_id: None,
        waiting: Rc::new(std::cell::Cell::new(false)),
    }
}

#[test]
fn launch_ids_mint_distinct_uuids() {
    run(options(), |_, c| async move {
        let (a, b) = (lc(&c).ids().uuid(), lc(&c).ids().uuid());
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(tc(&c).ids().uuid().len(), 36);
    });
}

#[test]
fn launch_entry_of_is_the_boot_areas_and_says_what_it_finds_nowhere() {
    run(options(), |_, c| async move {
        let put = put_live(&c, record("w_1", json!({}))).await;
        let found = lc(&c).entry_of("w_1", None).await.expect("the live entry");
        assert!(Rc::ptr_eq(&found, &put));
        let said = lc(&c).entry_of("w_nope", None).await.err().expect("no such workspace");
        assert!(said.message.contains("w_nope"), "{}", said.message);
    });
}

#[test]
fn launch_a_kind_keeps_agents_only_where_the_kinds_module_says() {
    let root = std::env::temp_dir().join("wsp-launch-keeps");
    let folder = root.to_string_lossy().to_string();
    run(local_opts(&root), |_, c| async move {
        let cloud = put_live(&c, record("w_1", json!({}))).await;
        let local = put_live(&c, record("w_2", json!({ "kind": "local", "folder": folder }))).await;
        assert!(!lc(&c).keeps_agents(&cloud));
        assert!(lc(&c).keeps_agents(&local));
    });
}

#[test]
fn launch_kept_box_cap_is_the_cap_of_the_place_a_workspace_stands_on() {
    run(with_door(), |_, c| async move {
        put_live(&c, record("w_1", json!({ "place": "p_1" }))).await;
        put_live(&c, record("w_2", json!({}))).await;
        // threads.ts:132: no place, no box; a place the door does not hold has no memory to keep agents in.
        assert_eq!(lc(&c).kept_box_cap("w_nope"), None);
        assert_eq!(lc(&c).kept_box_cap("w_2"), None);
        assert_eq!(lc(&c).kept_box_cap("w_1"), Some(0));
    });
}

#[test]
fn launch_preferences_held_is_what_the_state_kept_and_preferences_reads_them() {
    run(options(), |_, c| async move {
        assert!(lc(&c).preferences_held().is_none());
        let prefs = wsp_wire::views::preferences::Preferences {
            default_agent: Some("codex".into()),
            ..wsp_wire::views::preferences::default_preferences()
        };
        let kept = prefs.clone();
        wsp_account::account::channels::ChannelsCtx::state(&*c).with(|s| s.preferences_held = Some(kept));
        assert_eq!(lc(&c).preferences_held(), Some(prefs.clone()));
        // waits: packet 16 (PreferencesCtx), which `PreferencesArea::get` reads.
        assert_eq!(lc(&c).preferences().await.expect("preferences"), prefs);
    });
}

#[test]
fn launch_the_default_agent_is_the_projects_then_the_persons_then_the_first_one_that_runs() {
    // thread-defaults.test.ts "runs the project's agent, then the default agent, then the catalog's first".
    let adapters: IndexMap<String, HarnessAdapterFactory> =
        IndexMap::from([("claude".to_string(), refusing()), ("codex".to_string(), refusing())]);
    run(RuntimeOptions { adapters, ..options() }, |_, c| async move {
        let entry = put_live(&c, record("w_1", json!({ "project": "p_1" }))).await;
        let mut prefs = wsp_wire::views::preferences::default_preferences();
        assert_eq!(lc(&c).default_agent_of(&prefs, &entry), "claude");
        prefs.default_agent = Some("codex".into());
        assert_eq!(lc(&c).default_agent_of(&prefs, &entry), "codex");
        prefs.project_defaults.insert(
            "p_1".into(),
            Fields::new(serde_json::from_value::<ProjectOverrides>(json!({ "agent": "claude" })).expect("overrides")),
        );
        assert_eq!(lc(&c).default_agent_of(&prefs, &entry), "claude");
    });
}

#[test]
fn launch_an_agent_is_off_where_the_person_turned_it_off_on_the_workspaces_computer() {
    let store: Rc<dyn Store> = Rc::new(memory_store());
    let root = std::env::temp_dir().join("wsp-launch-off");
    let folder = root.to_string_lossy().to_string();
    let opts = RuntimeOptions { store: Some(store.clone()), ..local_opts(&root) };
    run(opts, |_, c| async move {
        kept_setup(&store, "here", "codex", json!({ "on": false })).await;
        kept_setup(&store, "p_9", "claude", json!({ "on": false })).await;
        lc(&c).ready().await.expect("the start");
        let local = put_live(&c, record("w_1", json!({ "kind": "local", "folder": folder }))).await;
        let at_nine = put_live(&c, record("w_2", json!({ "place": "p_9" }))).await;
        let nowhere = put_live(&c, record("w_3", json!({}))).await;
        assert!(lc(&c).agent_off(&local, "codex"));
        assert!(!lc(&c).agent_off(&local, "claude"));
        assert!(lc(&c).agent_off(&at_nine, "claude"));
        assert!(!lc(&c).agent_off(&nowhere, "claude"));
        assert_eq!(lc(&c).setup_place(&local).as_deref(), Some("here"));
        assert_eq!(lc(&c).setup_place(&at_nine).as_deref(), Some("p_9"));
        assert_eq!(lc(&c).setup_place(&nowhere), None);
    });
}

#[test]
fn launch_a_launch_is_the_setups_where_the_person_kept_one() {
    let store: Rc<dyn Store> = Rc::new(memory_store());
    run(RuntimeOptions { store: Some(store.clone()), ..options() }, |_, c| async move {
        kept_setup(&store, "p_9", "codex", json!({ "program": "/opt/codex", "args": ["--fast"] })).await;
        lc(&c).ready().await.expect("the start");
        let own = lc(&c).launch_of("p_9", "codex").expect("the kept launch");
        assert_eq!(own.program.as_deref(), Some("/opt/codex"));
        assert_eq!(own.args, Some(vec!["--fast".to_string()]));
        assert!(lc(&c).launch_of("p_9", "claude").is_none());
        assert!(lc(&c).launch_of("p_1", "codex").is_none());
    });
}

#[test]
fn launch_adapter_for_builds_the_named_harness_from_the_hosts_table() {
    // agents.ts:625: a harness the host has no factory for is refused naming the ones it has; one it has is built by
    // calling its factory, whose own refusal reaches the caller.
    let adapters: IndexMap<String, HarnessAdapterFactory> = IndexMap::from([("claude".to_string(), refusing())]);
    run(RuntimeOptions { adapters, ..options() }, |_, c| async move {
        let entry = put_live(&c, record("w_1", json!({}))).await;
        let ask = |named: &str| wsp_threads::threads::agents::launch::AdapterAsk {
            named: Some(named.into()),
            turn_env: None,
            waiting: None,
            servers: Env::new(),
            thread: None,
            asks_until_stopped: false,
        };
        let said = lc(&c).adapter_for(&entry, ask("codex")).err().expect("no codex factory");
        assert!(said.message.contains("codex") && said.message.contains("claude"), "{}", said.message);
        let said = lc(&c).adapter_for(&entry, ask("claude")).err().expect("the factory refuses");
        assert_eq!(said.message, "no adapter in this test");
    });
}

#[test]
fn launch_an_adapter_warms_where_it_keeps_an_agent_up() {
    run(options(), |_, c| async move {
        assert!(lc(&c).adapter_warms(&Stub { warms: true }));
        assert!(!lc(&c).adapter_warms(&Stub { warms: false }));
    });
}

#[test]
fn launch_confining_a_setup_settles_at_once_where_none_is_kept() {
    run(options(), |_, c| async move {
        let entry = put_live(&c, record("w_1", json!({}))).await;
        lc(&c).confine_setup(&entry, "claude").await.expect("nothing kept, nothing to confine");
    });
}

#[test]
fn launch_a_catalog_is_read_for_a_harness_the_table_names_and_none_for_one_it_does_not() {
    run(options(), |_, c| async move {
        let entry = put_live(&c, record("w_1", json!({}))).await;
        let none = lc(&c).catalog_on("no-such-agent", &entry, &Stub { warms: false }).await.expect("no refusal");
        assert!(none.is_none());
    });
}

#[test]
fn launch_defaults_on_marks_what_a_new_thread_starts_on() {
    // thread-defaults.test.ts: the model, effort and access off the person's defaults, then the catalog's.
    run(options(), |_, c| async move {
        let catalog = wsp_core::harness_catalog::harness_catalog("claude").expect("claude's table");
        let mut prefs = wsp_wire::views::preferences::default_preferences();
        let plain = lc(&c).defaults_on(catalog.clone(), &prefs, "p_1");
        assert_eq!(plain.catalog.harness, "claude");
        assert!(plain.open_model.is_some());
        prefs.agent_defaults.insert(
            "claude".into(),
            Fields::new(
                serde_json::from_value::<AgentDefaults>(json!({ "model": "claude-sonnet-5", "effort": "low" })).expect("agent defaults"),
            ),
        );
        let own = lc(&c).defaults_on(catalog, &prefs, "p_1");
        assert_eq!(own.open_model.as_deref(), Some("claude-sonnet-5"));
        assert_eq!(own.open_effort.as_deref(), Some("low"));
    });
}

#[test]
fn launch_a_named_access_is_the_harnesss_own_mode_and_refused_where_it_has_none() {
    // thread-defaults.test.ts "permissionMode": ask is default, auto-edit acceptEdits, full bypassPermissions.
    run(options(), |_, c| async move {
        use wsp_wire::types::thread_defaults::AccessChoice;
        let catalog = wsp_core::harness_catalog::harness_catalog("claude").expect("claude's table");
        let mode = |access| lc(&c).named_mode(Some(&catalog), "claude", &access);
        assert_eq!(mode(AccessChoice::Ask).expect("a mode").as_deref(), Some("default"));
        assert_eq!(mode(AccessChoice::AutoEdit).expect("a mode").as_deref(), Some("acceptEdits"));
        assert_eq!(mode(AccessChoice::Full).expect("a mode").as_deref(), Some("bypassPermissions"));
        let said = lc(&c).named_mode(None, "no-such-agent", &AccessChoice::Full).expect_err("a harness with no table");
        assert!(said.message.contains("no-such-agent"), "{}", said.message);
    });
}

#[test]
fn launch_what_a_thread_ran_on_and_resumed_as_is_read_off_its_rows() {
    run(options(), |_, c| async move {
        assert_eq!(lc(&c).access_of("w_1", "t_1", None), None);
        assert_eq!(lc(&c).ran_on("w_1", "t_1", None), Default::default());
        assert_eq!(lc(&c).resumed_fact("w_1", "s_1", "model"), None);
        tc(&c).sessions().borrow_mut().insert(
            "s_1".into(),
            row(view("s_1", "w_1", json!({ "threadId": "t_1", "model": "claude-opus-5-5", "permissionMode": "plan", "startedAt": 5 }))),
        );
        assert_eq!(lc(&c).ran_on("w_1", "t_1", None).model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(lc(&c).access_of("w_1", "t_1", Some("s_1")).as_deref(), Some("plan"));
        assert_eq!(lc(&c).resumed_fact("w_1", "s_1", "model").as_deref(), Some("claude-opus-5-5"));
    });
}

#[test]
fn launch_a_model_that_was_not_run_is_refused_in_the_catalogs_words() {
    run(options(), |_, c| async move {
        let catalog = wsp_core::harness_catalog::harness_catalog("claude").expect("claude's table");
        assert!(lc(&c).not_run_refused(None, None).is_none());
        assert!(lc(&c).not_run_refused(Some(&catalog), None).is_none());
        let direct = wsp_core::harness_catalog::not_run_refused(Some(&catalog), Some("gpt-nowhere"));
        assert_eq!(lc(&c).not_run_refused(Some(&catalog), Some("gpt-nowhere")).map(|e| e.message), direct.map(|e| e.message));
    });
}

#[test]
fn launch_a_threads_folder_is_the_given_cwd_else_the_kinds_else_the_projects() {
    let root = std::env::temp_dir().join("wsp-launch-folder");
    let folder = root.to_string_lossy().to_string();
    run(local_opts(&root), |_, c| async move {
        let local = put_live(&c, record("w_1", json!({ "kind": "local", "folder": folder }))).await;
        assert_eq!(lc(&c).thread_folder(&local, Some("/srv/elsewhere")).await.expect("a folder"), "/srv/elsewhere");
        assert_eq!(lc(&c).thread_folder(&local, None).await.expect("a folder"), folder);
        let cloud = put_live(&c, record("w_2", json!({}))).await;
        let said = lc(&c).thread_folder(&cloud, None).await.expect_err("p_1 is not held");
        assert!(said.message.contains("p_1"), "{}", said.message);
    });
}

#[test]
fn launch_a_folder_exists_where_this_computer_has_it() {
    run(options(), |_, c| async move {
        let here = std::env::temp_dir();
        assert!(lc(&c).folder_exists(&here.to_string_lossy()));
        assert!(!lc(&c).folder_exists(&here.join("wsp-launch-no-such-folder").to_string_lossy()));
    });
}

#[test]
fn launch_runs_in_is_the_folder_the_thread_ran_in_or_the_one_it_starts_in() {
    run(options(), |_, c| async move {
        let entry = put_live(&c, record("w_1", json!({}))).await;
        assert_eq!(lc(&c).runs_in(&entry, Some("/srv/was"), "/srv/now"), "/srv/was");
        assert_eq!(lc(&c).runs_in(&entry, None, "/srv/now"), "/srv/now");
    });
}

#[test]
fn launch_a_warm_agent_is_held_found_rewarmed_and_reaped() {
    // thread-warm-agent.test.ts: the agent a composer warms stands for its workspace and harness until a send claims
    // it, and is gone once reaped.
    run(options(), |_, c| async move {
        assert!(lc(&c).warm_on("w_1", "claude").is_none());
        lc(&c).hold_warm("t_1", hand("w_1", "claude"));
        let (thread, launch) = lc(&c).warm_on("w_1", "claude").expect("the warm agent");
        assert_eq!(thread, "t_1");
        assert_eq!(launch.fixed, "{\"harness\":\"claude\"}");
        assert!(lc(&c).warm_on("w_1", "codex").is_none());
        assert!(lc(&c).warm_on("w_2", "claude").is_none());
        lc(&c).rewarm("t_1");
        lc(&c).rewarm("t_nobody");
        lc(&c).reap_kept("t_1");
        assert!(lc(&c).warm_on("w_1", "claude").is_none());
        lc(&c).reap_kept("t_nobody");
    });
}

#[test]
fn launch_revoking_a_device_answers_ok_for_one_held_or_not() {
    run(options(), |_, c| async move {
        lc(&c).revoke_device("d_nobody").await.expect("nothing to revoke is no refusal");
        tc(&c).revoke_device("d_nobody").await.expect("nothing to revoke is no refusal");
    });
}

#[test]
fn launch_spawn_starts_the_work_now() {
    run(options(), |_, c| async move {
        let ran = Rc::new(std::cell::Cell::new(false));
        let seen = ran.clone();
        lc(&c).spawn(Box::pin(async move { seen.set(true) }));
        assert!(ran.get());
    });
}

// ----------------------------------------------------------------------------------------------------------------
// ThreadsCtx (threads.ts:70-1186, runtime.ts:465): what the threads area reads of the runtime.

fn event(value: Value) -> wsp_wire::wire::events_union::EventUnion {
    wsp_wire::wire::events_union::parse_event(&value).expect("an event")
}

fn heard_on(c: &Rc<Ctx>, kind: &str) -> Rc<RefCell<Vec<Value>>> {
    let heard = Rc::new(RefCell::new(Vec::<Value>::new()));
    let seen = heard.clone();
    let off = tc(c).bus().on(
        kind,
        Rc::new(move |e: &wsp_wire::wire::events_union::EventUnion| {
            seen.borrow_mut().push(serde_json::to_value(e).expect("an event"));
            Ok(())
        }),
    );
    std::mem::forget(off);
    heard
}

#[test]
fn threads_clock_ids_store_and_bus_are_the_cores() {
    let fake = FakeClock::new(42_000);
    let store: Rc<dyn Store> = Rc::new(memory_store());
    run(RuntimeOptions { clock: Some(fake.clock()), store: Some(store.clone()), ..options() }, |_, c| async move {
        assert_eq!(tc(&c).clock().now(), 42_000);
        assert_ne!(tc(&c).ids().uuid(), tc(&c).ids().uuid());
        store.put("things", "t1", json!(1)).await.expect("a put");
        assert_eq!(tc(&c).store().get("things", "t1").await.expect("a get"), Some(json!(1)));
        let heard = heard_on(&c, "workspace.gone");
        let gone = json!({ "type": "workspace.gone", "workspaceId": "w_1", "machineId": "m_1", "reason": "x" });
        wsp_threads::threads::transcripts::TranscriptsCtx::emit(&*c, event(gone)).expect("delivered");
        assert_eq!(heard.borrow().len(), 1);
    });
}

#[test]
fn threads_live_records_are_every_live_workspaces_in_the_order_they_went_live() {
    run(options(), |_, c| async move {
        assert!(tc(&c).live_records().is_empty());
        put_live(&c, record("w_b", json!({}))).await;
        put_live(&c, record("w_a", json!({}))).await;
        let ids = tc(&c).live_records().into_iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(ids, ["w_b", "w_a"]);
        assert!(tc(&c).entry("w_a").is_some() && tc(&c).entry("w_z").is_none());
    });
}

#[test]
fn threads_taken_is_the_request_a_turn_took_off_the_index() {
    // threads.ts: `ctx.transcriptIndex.get(ws)?.taken.get(request)`.
    run(options(), |_, c| async move {
        assert!(tc(&c).taken("w_1", "r_1").is_none());
        let taken = wsp_core::types::internal::Taken {
            session_id: "s_1".into(),
            thread_id: "t_1".into(),
            turn_id: "turn_1".into(),
            outcome: serde_json::from_value(json!("started")).expect("an outcome"),
        };
        let put = taken.clone();
        wsp_threads::threads::transcripts::TranscriptsCtx::transcript_state(&*c)
            .with(|s| s.index.entry("w_1".to_string()).or_default().taken.insert("r_1".to_string(), put));
        assert_eq!(tc(&c).taken("w_1", "r_1"), Some(taken));
        assert!(tc(&c).taken("w_1", "r_2").is_none() && tc(&c).taken("w_2", "r_1").is_none());
    });
}

#[test]
fn threads_without_a_place_door_every_place_read_is_empty() {
    run(options(), |_, c| async move {
        assert!(!tc(&c).has_place_door());
        assert!(!tc(&c).place_linked("p_1"));
        assert!(tc(&c).on_place_move(Rc::new(|_| {})).is_none());
        assert_eq!(tc(&c).place_mem_mb("p_1"), None);
        assert_eq!(tc(&c).place_threads_at("p_1"), None);
    });
}

#[test]
fn threads_with_a_place_door_every_place_read_is_the_doors() {
    run(with_door(), |_, c| async move {
        assert!(tc(&c).has_place_door());
        assert!(!tc(&c).place_linked("p_unknown"));
        assert_eq!(tc(&c).place_mem_mb("p_unknown"), None);
        assert_eq!(tc(&c).place_name("here"), "zingzys-mac");
        let off = tc(&c).on_place_move(Rc::new(|_| {})).expect("a listener on the door");
        off();
        off();
    });
}

#[test]
fn threads_a_workspaces_place_and_provider_are_the_channels_areas() {
    run(options(), |_, c| async move {
        let cloud = record("w_1", json!({ "place": "p_9" }));
        let local = record("w_2", json!({ "kind": "local" }));
        assert_eq!(tc(&c).place_id_of(&cloud).as_deref(), Some("p_9"));
        assert_eq!(tc(&c).place_id_of(&local).as_deref(), Some("here"));
        assert_eq!(tc(&c).provider_of(&local), None);
        assert_eq!(tc(&c).provider_of(&cloud), None);
    });
}

#[test]
fn threads_root_of_walks_to_the_thread_the_record_names() {
    run(options(), |_, c| async move {
        tc(&c).thread_records().borrow_mut().insert("t_child".into(), thread("w_1", json!({ "rootThreadId": "t_root" })));
        assert_eq!(tc(&c).root_of("t_child").as_deref(), Some("t_root"));
        assert_eq!(tc(&c).root_of("t_alone").as_deref(), Some("t_alone"));
    });
}

#[test]
fn threads_boot_work_and_kept_writes_are_what_a_workspace_still_waits_on() {
    run(options(), |_, c| async move {
        assert!(tc(&c).boot_work("w_1").is_none());
        assert!(tc(&c).kept_writes("w_1").is_none());
    });
}

#[test]
fn threads_persist_sessions_writes_a_workspaces_rows_to_the_store() {
    // thread-marks.test.ts reads the same document: `store.get("sessions", ws.id).sessions`.
    let store: Rc<dyn Store> = Rc::new(memory_store());
    run(RuntimeOptions { store: Some(store.clone()), ..options() }, |_, c| async move {
        tc(&c).sessions().borrow_mut().insert("s_1".into(), row(view("s_1", "w_1", json!({ "threadId": "t_1", "endedAt": 12 }))));
        tc(&c).persist_sessions("w_1").await;
        let kept = store.get("sessions", "w_1").await.expect("a get").expect("the workspace's rows");
        assert_eq!(kept["sessions"][0]["id"], json!("s_1"));
        ag(&c).persist_sessions("w_1").await;
    });
}

#[test]
fn threads_record_stamps_a_session_event_and_open_thread_reads_it_back() {
    let fake = FakeClock::new(9_000);
    run(RuntimeOptions { clock: Some(fake.clock()), ..options() }, |_, c| async move {
        let heard = heard_on(&c, "session.delta");
        let delta = serde_json::from_value(
            json!({ "type": "session.delta", "workspaceId": "w_1", "sessionId": "s", "threadId": "t_1", "kind": "text", "text": "hi", "pos": 1 }),
        )
        .expect("a session event");
        tc(&c).record(delta);
        assert_eq!(heard.borrow().len(), 1);
        assert_eq!(heard.borrow()[0]["at"], json!(9_000));
        let events = tc(&c).open_thread("w_1", "t_1").await.expect("the thread's events");
        assert_eq!(events.len(), 1);
    });
}

#[test]
fn threads_keep_sent_images_settles_where_the_send_carried_none() {
    run(options(), |_, c| async move {
        tc(&c).keep_sent_images("w_1", "t_1", Some("r_1".into()), vec![]).await;
    });
}

#[test]
fn threads_a_thread_asked_for_that_is_nowhere_is_refused_and_marking_stamps_a_held_one() {
    // thread-marks.test.ts "a name nothing holds moves nothing": `no thread thr_nobo`, kind not-found; a held thread's
    // mark is said once on the bus for its workspace.
    run(options(), |_, c| async move {
        put_live(&c, record("w_1", json!({}))).await;
        tc(&c).thread_records().borrow_mut().insert("t_1".into(), thread("w_1", json!({})));
        let heard = heard_on(&c, "thread.marked");
        let stamps: wsp_core::types::internal::ThreadStamps = serde_json::from_value(json!({ "readAt": 5 })).expect("stamps");
        let said = tc(&c).mark(vec!["thr_nobody".into()], stamps.clone(), None).await.expect_err("nothing holds it");
        assert_eq!(said.message, "no thread thr_nobo");
        assert!(heard.borrow().is_empty());
        tc(&c).mark(vec!["t_1".into()], stamps, None).await.expect("a mark");
        assert_eq!(heard.borrow().len(), 1);
        assert_eq!(heard.borrow()[0]["workspaceId"], json!("w_1"));
        assert_eq!(heard.borrow()[0]["threadIds"], json!(["t_1"]));
    });
}

#[test]
fn threads_idle_and_port_roots_and_proc_scopes_move_without_a_refusal() {
    run(options(), |_, c| async move {
        put_live(&c, record("w_1", json!({}))).await;
        tc(&c).idle_hold("w_1");
        tc(&c).idle_release("w_1");
        tc(&c).port_roots_moved("w_1");
        tc(&c).proc_scopes_moved();
        tc(&c).end_snooze_for(&view("s_1", "w_1", json!({ "threadId": "t_1" })));
    });
}

#[test]
fn threads_a_workspace_reads_its_checkout_and_project_where_the_project_is_held() {
    run(options(), |_, c| async move {
        let cloud = record("w_1", json!({}));
        let said = tc(&c).project_held("p_1").expect_err("p_1 is not held");
        assert!(said.message.contains("p_1"), "{}", said.message);
        assert!(tc(&c).checkout_of(&cloud).is_err());
        let held: wsp_wire::views::workspace::ProjectView = serde_json::from_value(json!({
            "id": "p_1", "name": "web", "path": "/work/web", "computer": "zingzys-mac", "source": { "kind": "folder", "path": "/work/web" },
            "remote": "", "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
        }))
        .expect("a project");
        wsp_account::account::rules::RulesCtx::projects_held(&*c).borrow_mut().insert("p_1".into(), held.clone());
        assert_eq!(tc(&c).project_held("p_1").expect("held"), held);
    });
}

#[test]
fn threads_reach_computer_and_login_of_a_live_workspace_are_the_areas_that_know_them() {
    run(options(), |_, c| async move {
        let cloud = put_live(&c, record("w_1", json!({}))).await;
        assert_ne!(tc(&c).reach_of(&cloud), wsp_wire::views::workspace::ReachState::Gone);
        assert!(!tc(&c).computer_of(&cloud).is_empty());
        assert!(!tc(&c).login_stands(&cloud, "claude"));
        assert_eq!(tc(&c).turn_snapshot_ms(), None);
    });
    run(RuntimeOptions { turn_snapshot_ms: Some(250), ..options() }, |_, c| async move {
        assert_eq!(tc(&c).turn_snapshot_ms(), Some(250));
    });
}

#[test]
fn threads_with_a_daemon_asks_the_workspaces_daemon_and_says_so_where_it_has_none() {
    run(options(), |_, c| async move {
        let cloud = put_live(&c, record("w_1", json!({}))).await;
        assert!(tc(&c).with_daemon(&cloud, json!({ "type": "ping" })).await.is_err());
    });
}

#[test]
fn threads_the_ledgers_limits_are_the_usage_areas() {
    // waits: packet 15 (the usage ledger the context builds).
    run(options(), |_, c| async move {
        assert!(tc(&c).ledger_limits().await.expect("limits").is_empty());
    });
}

#[test]
fn threads_a_sub_line_is_none_for_a_workspace_that_is_no_sub() {
    // waits: packet 7 (SubLifeCtx).
    run(options(), |_, c| async move {
        put_live(&c, record("w_1", json!({}))).await;
        assert_eq!(tc(&c).sub_line("w_1", 0).await, None);
    });
}

#[test]
fn threads_start_interrupt_and_stop_try_are_the_sessions_areas() {
    // waits: packet 13 (SessionsCtx): the sessions area reads its own context to find a row.
    run(options(), |_, c| async move {
        assert!(tc(&c).interrupt("r_nobody", None).await.is_err());
        assert!(tc(&c).stop_try("r_nobody").await.is_err());
        assert!(tc(&c).start("w_nobody", Default::default(), None).await.is_err());
    });
}

#[test]
fn threads_spawn_starts_the_work_now() {
    run(options(), |_, c| async move {
        let ran = Rc::new(std::cell::Cell::new(false));
        let seen = ran.clone();
        tc(&c).spawn(Box::pin(async move { seen.set(true) }));
        assert!(ran.get());
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The TS tests that drive a whole runtime (thread-defaults, thread-keeps-agent, thread-marks). Each waits on the packets
// whose bodies a turn goes through: 6 and 8 (create a project and a workspace), 12 and 13 (`sessions.start`, the turn,
// the rows), 16 (`preferences`). The other ten named tests (thread-keeps-picks, thread-keeps-process, thread-warm-agent,
// agents-brokered, vault, question-env, thread-head, thread-lines, forget, replaces) start the real adapters over
// stand-in agent binaries or script the engine's stub; they are ported once packets 12 and 13 land, and the methods
// they read are covered one by one above.

use wsp_adapters::port::{AdapterEvent, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture};
use wsp_core::types::api::{Runtime, StartOptions};

const SESSION: &str = "44444444-4444-4444-8444-444444444444";

#[derive(Clone, Debug, Default, PartialEq)]
struct Launched {
    harness: String,
    model: Option<String>,
    effort: Option<String>,
    permission_mode: Option<String>,
}

struct Done(SharedFuture<wsp_adapters::port::TurnResult>);

impl HarnessSession for Done {
    fn local_id(&self) -> String {
        SESSION.into()
    }
    fn finished(&self) -> SharedFuture<wsp_adapters::port::TurnResult> {
        self.0.clone()
    }
    fn interrupt(&self) -> futures::future::LocalBoxFuture<'static, ()> {
        Box::pin(async {})
    }
}

/// An adapter per harness that records what each launch was handed and ends its turn at once (thread-defaults.test.ts).
struct Recorder {
    harness: String,
    launched: Rc<RefCell<Vec<Launched>>>,
}

impl HarnessAdapter for Recorder {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, wsp_adapters::port::AdapterError> {
        self.launched.borrow_mut().push(Launched {
            harness: self.harness.clone(),
            model: o.model.clone(),
            effort: o.effort.clone(),
            permission_mode: o.permission_mode.clone(),
        });
        let result: wsp_adapters::port::TurnResult =
            serde_json::from_value(json!({ "status": "completed", "text": "ok" })).expect("a result");
        let on = o.on_event.clone().expect("an event sink");
        for doc in [
            json!({ "type": "session.start", "sessionId": SESSION }),
            json!({ "type": "turn.done", "sessionId": SESSION, "result": result }),
            json!({ "type": "session.end", "sessionId": SESSION, "exitCode": 0, "sawResult": true }),
        ] {
            on(serde_json::from_value::<AdapterEvent>(doc).expect("an adapter event"));
        }
        let done: SharedFuture<wsp_adapters::port::TurnResult> =
            futures::FutureExt::shared(Box::pin(async move { result }) as futures::future::LocalBoxFuture<'static, _>);
        Ok(Arc::new(Done(done)))
    }
    fn steers(&self) -> bool {
        false
    }
}

fn recorders(launched: &Rc<RefCell<Vec<Launched>>>) -> IndexMap<String, HarnessAdapterFactory> {
    ["claude", "codex"]
        .map(|harness| {
            let launched = launched.clone();
            let factory: HarnessAdapterFactory =
                Arc::new(
                    move |_| Ok(Arc::new(Recorder { harness: harness.into(), launched: launched.clone() }) as Arc<dyn HarnessAdapter>),
                );
            (harness.to_string(), factory)
        })
        .into_iter()
        .collect()
}

/// A workspace on this computer over a fresh folder, and the runtime's start on it (the TS `createOn(rt, { on: HERE })`).
async fn here(rt: &AssembledRuntime, root: &Path) -> String {
    let folder = root.join("project");
    std::fs::create_dir_all(&folder).expect("a folder");
    let git = |args: &[&str]| assert!(std::process::Command::new("git").arg("-C").arg(&folder).args(args).status().expect("git").success());
    git(&["init", "-q"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "--allow-empty", "-m", "first"]);
    let project = rt
        .projects()
        .add(wsp_core::types::api::ProjectAddAsk { source: folder.to_string_lossy().into(), ..Default::default() }, None)
        .await
        .expect("a project");
    let options = wsp_core::types::wiring::CreateWorkspaceOptions { project: project.project.id, name: "mac".into(), ..Default::default() };
    rt.workspaces().create(options, None).await.expect("a workspace").view.id
}

async fn go(rt: &AssembledRuntime, workspace: &str, launched: &Rc<RefCell<Vec<Launched>>>, o: StartOptions) -> Launched {
    rt.sessions().start(workspace, o, None).await.expect("a start").finished().await;
    launched.borrow().last().cloned().expect("a launch")
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

#[test]
fn a_new_thread_runs_the_projects_agent_then_the_default_agent_then_the_catalogs_first_and_a_named_one_over_all() {
    // waits: packets 6, 8, 12, 13, 16. thread-defaults.test.ts, the first case.
    let root = std::env::temp_dir().join("wsp-agents-defaults-1");
    let launched = Rc::new(RefCell::new(Vec::new()));
    run(RuntimeOptions { adapters: recorders(&launched), ..local_opts(&root) }, |rt, _| async move {
        let ws = here(&rt, &root).await;
        assert_eq!(go(&rt, &ws, &launched, start("one")).await.harness, "claude");
        rt.preferences().set(serde_json::from_value(json!({ "defaultAgent": "codex" })).expect("a patch")).await.expect("set");
        assert_eq!(go(&rt, &ws, &launched, start("two")).await.harness, "codex");
        let project = rt.projects().list(None).await.expect("projects")[0].id.clone();
        rt.preferences()
            .set(serde_json::from_value(json!({ "projectDefaults": { project: { "agent": "claude" } } })).expect("a patch"))
            .await
            .expect("set");
        assert_eq!(go(&rt, &ws, &launched, start("three")).await.harness, "claude");
        assert_eq!(go(&rt, &ws, &launched, StartOptions { harness: Some("codex".into()), ..start("four") }).await.harness, "codex");
    });
}

#[test]
fn a_new_thread_runs_the_projects_model_and_access_over_the_agents_and_a_named_pick_over_all() {
    // waits: packets 6, 8, 12, 13, 16. thread-defaults.test.ts, the third case.
    let root = std::env::temp_dir().join("wsp-agents-defaults-3");
    let launched = Rc::new(RefCell::new(Vec::new()));
    run(RuntimeOptions { adapters: recorders(&launched), ..local_opts(&root) }, |rt, _| async move {
        let ws = here(&rt, &root).await;
        let first = go(&rt, &ws, &launched, start("one")).await;
        assert_eq!(
            (first.model.as_deref(), first.effort.as_deref(), first.permission_mode.as_deref()),
            (Some("claude-opus-5-5"), Some("high"), Some("bypassPermissions"))
        );
        rt.preferences()
            .set(
                serde_json::from_value(
                    json!({ "agentDefaults": { "claude": { "model": "claude-sonnet-5", "effort": "low", "access": "ask" } } }),
                )
                .expect("a patch"),
            )
            .await
            .expect("set");
        let second = go(&rt, &ws, &launched, start("two")).await;
        assert_eq!(
            (second.model.as_deref(), second.effort.as_deref(), second.permission_mode.as_deref()),
            (Some("claude-sonnet-5"), Some("low"), Some("default"))
        );
        let named = StartOptions {
            model: Some("claude-haiku-4-5-20251001".into()),
            access: Some(wsp_wire::types::thread_defaults::AccessChoice::Full),
            ..start("four")
        };
        let fourth = go(&rt, &ws, &launched, named).await;
        assert_eq!(
            (fourth.model.as_deref(), fourth.permission_mode.as_deref()),
            (Some("claude-haiku-4-5-20251001"), Some("bypassPermissions"))
        );
    });
}

#[test]
fn a_send_into_a_thread_naming_another_agent_is_refused_and_starts_nothing() {
    // waits: packets 6, 8, 12, 13. thread-keeps-agent.test.ts, the first case.
    let root = std::env::temp_dir().join("wsp-agents-keeps-1");
    let launched = Rc::new(RefCell::new(Vec::new()));
    run(RuntimeOptions { adapters: recorders(&launched), ..local_opts(&root) }, |rt, _| async move {
        let ws = here(&rt, &root).await;
        let open = StartOptions { harness: Some("codex".into()), permission_mode: Some("read-only".into()), ..start("one") };
        let first = rt.sessions().start(&ws, open, None).await.expect("a start");
        first.finished().await;
        let thread = first.view().thread_id.expect("a thread");
        let again = StartOptions { thread: Some(thread), harness: Some("claude".into()), ..start("two") };
        let said = rt.sessions().start(&ws, again, None).await.err().expect("refused");
        assert_eq!(said.message, "this thread runs on codex; open a new thread to run claude");
        assert_eq!(launched.borrow().len(), 1);
        assert_eq!(
            rt.sessions().list(Some(ws), None).await.expect("rows").iter().map(|s| s.harness.clone()).collect::<Vec<_>>(),
            ["codex"]
        );
    });
}

#[test]
fn a_send_into_a_thread_keeps_the_threads_own_access() {
    // waits: packets 6, 8, 12, 13. thread-keeps-agent.test.ts, the second case.
    let root = std::env::temp_dir().join("wsp-agents-keeps-2");
    let launched = Rc::new(RefCell::new(Vec::new()));
    run(RuntimeOptions { adapters: recorders(&launched), ..local_opts(&root) }, |rt, _| async move {
        let ws = here(&rt, &root).await;
        let open = StartOptions { harness: Some("codex".into()), permission_mode: Some("read-only".into()), ..start("one") };
        let first = rt.sessions().start(&ws, open, None).await.expect("a start");
        first.finished().await;
        assert_eq!(launched.borrow().last().and_then(|l| l.permission_mode.clone()).as_deref(), Some("read-only"));
        let thread = first.view().thread_id.expect("a thread");
        let again = StartOptions { thread: Some(thread), permission_mode: Some("danger-full-access".into()), ..start("two") };
        rt.sessions().start(&ws, again, None).await.expect("a start").finished().await;
        assert_eq!(launched.borrow().last().and_then(|l| l.permission_mode.clone()).as_deref(), Some("read-only"));
    });
}

#[test]
fn a_finished_turn_nobody_has_shown_reads_done_and_a_read_moves_its_stamp() {
    // waits: packets 6, 8, 12, 13. thread-marks.test.ts, the first case: `sessions.read` stamps every row of the thread
    // with one `readAt` and says `thread.marked` once.
    let root = std::env::temp_dir().join("wsp-agents-marks-1");
    let launched = Rc::new(RefCell::new(Vec::new()));
    let store: Rc<dyn Store> = Rc::new(memory_store());
    run(RuntimeOptions { store: Some(store.clone()), adapters: recorders(&launched), ..local_opts(&root) }, |rt, c| async move {
        store.put("reads", "since", json!({ "at": 1 })).await.expect("a put");
        let ws = here(&rt, &root).await;
        let heard = heard_on(&c, "thread.marked");
        let first = rt.sessions().start(&ws, start("build it"), None).await.expect("a start");
        first.finished().await;
        let thread = first.view().thread_id.expect("a thread");
        rt.sessions()
            .start(&ws, StartOptions { thread: Some(thread.clone()), ..start("and test it") }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        assert_eq!(rt.sessions().list(Some(ws.clone()), None).await.expect("rows")[0].read_at, Some(1.into()));
        rt.sessions().read(&thread, None).await.expect("a read");
        let rows = rt.sessions().list(Some(ws.clone()), None).await.expect("rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].read_at, rows[1].read_at);
        assert_eq!(heard.borrow().len(), 1);
        assert_eq!(heard.borrow()[0]["threadIds"], json!([thread]));
        assert_eq!(heard.borrow()[0]["workspaceId"], json!(ws));
    });
}
