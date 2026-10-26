// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 2 (#2476): the context's kinds, rules, records, held-image, branch-of-record and place-door
//! adapters (`KindsCtx`, `BackendFor`, `ThreadEndsRuntime`, `ThreadEndsDoor`, `RulesCtx`, `RecordsCtx`, `HeldCtx`,
//! `BranchCtx`, `ImageDoor`, `AgentsPlaces`). Each test builds the runtime on a throwaway state over `FakeBackend` and
//! calls the context's method the way the area crate calls it, through `&dyn Trait`. The TS is the oracle: each
//! expectation is what `runtime.ts`, `kinds.ts`, `rules.ts`, `records.ts`, `held.ts` and `branch-of-record.ts` answer.
//! Tests that go through another packet's bodies (a `ChannelsArea` read waits on packet 3, a `ReachArea` read on 4,
//! boot on 5, the ops on 6 to 13) say so on the test.
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::codex::command::BrokerMode;
use wsp_adapters::pending::CodexClaims;
use wsp_core::clock::FakeClock;
use wsp_core::types::api::Runtime;
use wsp_core::types::internal::{released_backend, EndThread, RELEASED_LINE};
use wsp_core::types::wiring::{Broker, GoldenRecipe, LocalWiring, Platform, RuntimeOptions, WiredId, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions, LOCAL_MACHINE_ID};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_machines::agents_read::AgentsPlaces;
use wsp_machines::images::builders::ImageDoor;
use wsp_machines::images::held::HeldCtx;
use wsp_machines::images::records::RecordsCtx;
use wsp_machines::machines::kinds::{BrokerSaid, KindsCtx};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_life::BackendFor;
use wsp_machines::machines::thread_ends::{ThreadEndsDoor, ThreadEndsRuntime};
use wsp_machines::places::types::{HerePlace, PlaceExecOpts, PlaceWiring};
use wsp_machines::projects::branch_of_record::BranchCtx;
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::memory_store;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::runtime::BrokerStatus;
use wsp_words::exit::HostError;

use wsp_account::account::rules::RulesCtx;
use wsp_core::types::wiring::Local;

// ----------------------------------------------------------------------------------------------------------------
// Fixtures

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

fn plain() -> RuntimeOptions {
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
    RuntimeOptions { place_links: Some(Rc::new(wiring())), ..plain() }
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
    RuntimeOptions { local: Some(local_in(root)), ..plain() }
}

fn record(extra: Value) -> WorkspaceRecord {
    let mut base = json!({
        "id": "w_1", "name": "api", "machineId": "m_1", "phase": "running", "golden": "g",
        "createdAt": "2026-10-26T00:00:00.000Z", "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 },
        "firstLife": false,
    });
    for (k, v) in extra.as_object().expect("an object").iter() {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a workspace record")
}

fn project(id: &str, name: &str, path: &str) -> ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "path": path, "computer": "zingzys-mac", "source": { "kind": "folder", "path": path }, "remote": "",
        "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a project")
}

/// A live entry over a record and a machine, as `attach` makes it.
fn entry_over(record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry {
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
    Rc::new(RefCell::new(wsp_core::types::wiring::LiveWorkspace {
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
    }))
}

async fn machine_named(id: &str) -> Arc<dyn Machine> {
    backend().get(id).await.expect("a machine")
}

/// The runtime on a throwaway state, inside the task set the owner thread runs.
fn drive<Fut: std::future::Future<Output = ()>>(opts: RuntimeOptions, f: impl FnOnce(Rc<Ctx>, Rc<AssembledRuntime>) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let runtime = create_runtime(opts);
        let ctx: Rc<Ctx> = runtime.context();
        f(ctx, runtime).await
    });
}

/// The refusal of a call that was meant to fail, whether or not its answer can be printed.
trait Refused<E> {
    fn refused(self, why: &str) -> E;
}

impl<T, E> Refused<E> for Result<T, E> {
    fn refused(self, why: &str) -> E {
        match self {
            Err(e) => e,
            Ok(_) => panic!("expected a refusal: {why}"),
        }
    }
}

fn same_backend(a: &Arc<dyn MachineBackend>, b: &Arc<dyn MachineBackend>) -> bool {
    Arc::as_ptr(a) as *const () == Arc::as_ptr(b) as *const ()
}

fn kinds(c: &Rc<Ctx>) -> &dyn KindsCtx {
    &**c
}
fn ends_rt(c: &Rc<Ctx>) -> &dyn ThreadEndsRuntime {
    &**c
}
fn ends_door(c: &Rc<Ctx>) -> &dyn ThreadEndsDoor {
    &**c
}
fn rules(c: &Rc<Ctx>) -> &dyn RulesCtx {
    &**c
}
fn records(c: &Rc<Ctx>) -> &dyn RecordsCtx {
    &**c
}
fn held(c: &Rc<Ctx>) -> &dyn HeldCtx {
    &**c
}
fn branch(c: &Rc<Ctx>) -> &dyn BranchCtx {
    &**c
}
fn image_door(c: &Rc<Ctx>) -> &dyn ImageDoor {
    &**c
}
fn agents_places(c: &Rc<Ctx>) -> &dyn AgentsPlaces {
    &**c
}
fn backend_for(c: &Rc<Ctx>) -> &dyn BackendFor {
    &**c
}

/// The broker a host wires, scripted per test.
struct FakeBroker {
    ready: Cell<bool>,
    status: Option<BrokerStatus>,
    claims: Option<CodexClaims>,
}

#[async_trait::async_trait(?Send)]
impl Broker for FakeBroker {
    fn ready(&self) -> bool {
        self.ready.get()
    }
    async fn codex_claims(&self) -> Result<Option<CodexClaims>, HostError> {
        Ok(self.claims.clone())
    }
    fn status(&self) -> Option<Local<'static, BrokerStatus>> {
        let status = self.status.clone()?;
        Some(async move { Ok(status) }.boxed_local())
    }
}

fn broker(ready: bool, status: Option<BrokerStatus>, claims: Option<CodexClaims>) -> Rc<dyn Broker> {
    Rc::new(FakeBroker { ready: Cell::new(ready), status, claims })
}

fn github_status() -> BrokerStatus {
    BrokerStatus { claude: None, codex: None, github: Some("token".into()) }
}

// ----------------------------------------------------------------------------------------------------------------
// KindsCtx: this host's own options and clock (kinds.ts:29-60)

#[test]
fn kinds_now_reads_the_clock_the_host_wired() {
    let fake = FakeClock::new(1_700_000_000_000);
    let opts = RuntimeOptions { clock: Some(fake.clock()), ..plain() };
    drive(opts, |c, _| async move {
        assert_eq!(kinds(&c).now(), 1_700_000_000_000.0);
        assert_eq!(kinds(&c).now_ms(), 1_700_000_000_000.0);
        fake.advance(5);
        assert_eq!(kinds(&c).now(), 1_700_000_000_005.0);
    });
}

#[test]
fn kinds_state_path_is_the_option() {
    let opts = RuntimeOptions { state_path: Some("/srv/wsp/state.db".into()), ..plain() };
    drive(opts, |c, _| async move {
        assert_eq!(kinds(&c).state_path().as_deref(), Some("/srv/wsp/state.db"));
    });
    drive(plain(), |c, _| async move {
        assert_eq!(kinds(&c).state_path(), None);
    });
}

#[test]
fn kinds_own_backend_is_the_backend_the_host_wired() {
    let wired = backend();
    let opts = RuntimeOptions { backend: Some(wired.clone()), ..plain() };
    drive(opts, |c, _| async move {
        assert!(same_backend(&kinds(&c).own_backend(), &wired));
    });
}

#[test]
fn kinds_released_backend_refuses_in_the_words_of_a_released_machine() {
    drive(plain(), |c, _| async move {
        let released = kinds(&c).released_backend();
        let said = released.get("m_1").await.refused("a refusal").to_string();
        assert!(said.contains(RELEASED_LINE), "{said}");
        let direct = released_backend().get("m_1").await.refused("a refusal").to_string();
        assert_eq!(said, direct);
    });
}

#[test]
fn kinds_wired_place_is_the_places_table_wired_row() {
    drive(plain(), |c, _| async move {
        assert_eq!(kinds(&c).wired_place(), "default");
    });
    let backend = backend();
    let opts = RuntimeOptions {
        places: Some(wsp_core::types::wiring::wired_place(WiredId::Fixed("solari".into()), backend.clone())),
        backend: Some(backend),
        ..plain()
    };
    drive(opts, |c, _| async move {
        assert_eq!(kinds(&c).wired_place(), "solari");
    });
}

#[test]
fn kinds_local_machine_is_the_local_backends_machine() {
    let dir = tempfile::tempdir().expect("a folder");
    drive(local_opts(dir.path()), |c, _| async move {
        let machine = kinds(&c).local_machine().await.expect("this computer's machine");
        assert_eq!(machine.id(), LOCAL_MACHINE_ID);
    });
}

#[test]
fn kinds_local_machine_refuses_where_no_local_road_is_wired() {
    drive(plain(), |c, _| async move {
        let said = kinds(&c).local_machine().await.refused("a refusal").message();
        assert_eq!(said, wsp_words::words::computer::no_kind_line("local"));
    });
}

#[test]
fn kinds_has_local_and_its_reads_follow_the_wiring() {
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().to_string_lossy().to_string();
    drive(local_opts(dir.path()), |c, _| async move {
        let k = kinds(&c);
        assert!(k.has_local());
        assert!(k.local_backend().is_some());
        assert_eq!(k.local_home("claude"), format!("{root}/.claude"));
        assert_eq!(k.local_home_dir(), root);
        assert_eq!(k.local_scratch(), root);
        assert_eq!(k.local_roots_path().as_deref(), Some(format!("{root}/roots").as_str()));
        assert_eq!(k.local_env().get("PATH").map(String::as_str), Some("/usr/bin:/bin"));
        assert!(!k.has_local_daemon_road());
        assert!(!k.has_local_restart());
        assert!(k.local_daemon_road().await.is_none());
        assert!(k.here_daemon_version().await.is_none());
    });
    drive(plain(), |c, _| async move {
        let k = kinds(&c);
        assert!(!k.has_local());
        assert!(k.local_backend().is_none());
        assert_eq!(k.local_roots_path(), None);
    });
}

#[test]
fn kinds_local_env_is_asked_every_time_never_copied() {
    let dir = tempfile::tempdir().expect("a folder");
    let asked = Rc::new(Cell::new(0u32));
    let mut local = local_in(dir.path());
    let count = asked.clone();
    local.env = Rc::new(move || {
        count.set(count.get() + 1);
        IndexMap::from([("N".to_string(), count.get().to_string())])
    });
    let opts = RuntimeOptions { local: Some(local), ..plain() };
    drive(opts, |c, _| async move {
        let first = kinds(&c).local_env();
        let second = kinds(&c).local_env();
        assert_eq!(first.get("N").map(String::as_str), Some("1"));
        assert_eq!(second.get("N").map(String::as_str), Some("2"));
    });
}

#[test]
fn kinds_local_daemon_road_and_restart_follow_the_wiring() {
    let dir = tempfile::tempdir().expect("a folder");
    let mut local = local_in(dir.path());
    local.daemon_road = Some(Rc::new(|| {
        async { Ok(serde_json::from_value(json!({ "url": "http://127.0.0.1:7777", "expiresAt": 9, "daemonToken": "t" })).unwrap()) }
            .boxed_local()
    }));
    local.restart_daemon = Some(Rc::new(|| async { Ok(()) }.boxed_local()));
    let opts = RuntimeOptions { local: Some(local), ..plain() };
    drive(opts, |c, _| async move {
        let k = kinds(&c);
        assert!(k.has_local_daemon_road());
        assert!(k.has_local_restart());
        let road = k.local_daemon_road().await.expect("the road is wired").expect("it answers");
        assert_eq!(road.url, "http://127.0.0.1:7777");
    });
}

#[test]
fn kinds_agents_reach_reads_the_options() {
    let spec: wsp_wire::views::harness::McpServerSpec =
        serde_json::from_value(json!({ "command": "wsp", "args": ["mcp"] })).expect("an mcp spec");
    let opts = RuntimeOptions {
        agents: Some(wsp_core::types::wiring::AgentsReach { here_url: Some("http://127.0.0.1:1234".into()), wsp_mcp: Some(spec.clone()) }),
        ..plain()
    };
    drive(opts, |c, _| async move {
        assert_eq!(kinds(&c).agents_here_url().as_deref(), Some("http://127.0.0.1:1234"));
        assert_eq!(kinds(&c).agents_wsp_mcp(), Some(spec));
    });
    drive(plain(), |c, _| async move {
        assert_eq!(kinds(&c).agents_here_url(), None);
        assert_eq!(kinds(&c).agents_wsp_mcp(), None);
    });
}

#[test]
fn kinds_vault_is_what_the_host_hands_or_nothing() {
    let opts = RuntimeOptions { vault: Some(Rc::new(|| IndexMap::from([("OPENAI_API_KEY".to_string(), "sk-x".to_string())]))), ..plain() };
    drive(opts, |c, _| async move {
        assert_eq!(kinds(&c).vault().get("OPENAI_API_KEY").map(String::as_str), Some("sk-x"));
    });
    drive(plain(), |c, _| async move {
        assert!(kinds(&c).vault().is_empty());
    });
}

#[test]
fn kinds_machine_exec_opts_and_deploy_follow_the_options() {
    drive(plain(), |c, _| async move {
        assert!(kinds(&c).machine_exec_opts().is_none());
        assert!(!kinds(&c).has_deploy());
    });
    let opts = RuntimeOptions {
        machine_exec: Some(Default::default()),
        golden_recipe: Some(GoldenRecipe { setup: "true".into(), smoke: "true".into(), ..Default::default() }),
        ..plain()
    };
    drive(opts, |c, _| async move {
        assert!(kinds(&c).machine_exec_opts().is_some());
        assert!(!kinds(&c).has_deploy());
    });
}

#[test]
fn kinds_place_name_is_the_doors_name_else_the_hostname_or_the_providers() {
    drive(plain(), |c, _| async move {
        let k = kinds(&c);
        assert_eq!(k.place_name(HERE_PLACE_ID), wsp_core::host_id::hostname());
        assert_eq!(k.place_name("solari"), wsp_words::place_word::provider_key_name("solari"));
    });
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        assert_eq!(kinds(&c).place_name(HERE_PLACE_ID), door.name_of(HERE_PLACE_ID));
        assert_eq!(records(&c).wired(), "default");
    });
}

#[test]
fn kinds_deploy_daemon_runs_the_recipes_deploy_and_drops_its_detail() {
    let deployed = Rc::new(RefCell::new(Vec::<(String, Option<String>)>::new()));
    let seen = deployed.clone();
    let opts = RuntimeOptions {
        golden_recipe: Some(GoldenRecipe {
            setup: "true".into(),
            smoke: "true".into(),
            deploy_daemon: Some(Rc::new(move |machine, token| {
                seen.borrow_mut().push((machine.id().to_string(), token));
                async { Ok(Some("deployed".to_string())) }.boxed_local()
            })),
            ..Default::default()
        }),
        ..plain()
    };
    drive(opts, |c, _| async move {
        assert!(kinds(&c).has_deploy());
        let machine = machine_named("m_1").await;
        kinds(&c).deploy_daemon(&machine, Some("tok".into())).await.expect("a deploy");
        assert_eq!(*deployed.borrow(), vec![("m_1".to_string(), Some("tok".to_string()))]);
    });
}

struct HereAt(u32);

#[async_trait::async_trait(?Send)]
impl wsp_core::types::wiring::HereDaemon for HereAt {
    async fn version(&self) -> Result<u32, HostError> {
        Ok(self.0)
    }
    fn fix(&self) -> String {
        "run wsp".into()
    }
}

#[test]
fn kinds_here_daemon_version_is_the_wired_daemons() {
    let dir = tempfile::tempdir().expect("a folder");
    let mut local = local_in(dir.path());
    local.here_daemon = Some(Rc::new(HereAt(7)));
    let opts = RuntimeOptions { local: Some(local), ..plain() };
    drive(opts, |c, _| async move {
        assert_eq!(kinds(&c).here_daemon_version().await.expect("a here daemon").expect("a version"), Some(7));
    });
}

/// `opts.broker.codexModels`, bound: where the broker has no such read the models stay unread, which is `Ok(None)`.
#[test]
fn kinds_broker_codex_models_answer_none_where_the_broker_reads_none() {
    let opts = RuntimeOptions { broker: Some(broker(true, None, None)), ..plain() };
    drive(opts, |c, _| async move {
        let models = kinds(&c).broker_codex_models().expect("a broker holds the read");
        assert_eq!(models("1.2.3".to_string()).await.expect("a read"), None);
    });
}

/// Waits on packet 3 (`TranscriptsArea::daemon_token_of` rotates the token through `TranscriptsCtx`).
#[test]
fn kinds_daemon_token_of_a_machine_is_the_same_token_each_time() {
    drive(plain(), |c, _| async move {
        let machine = machine_named("m_1").await;
        let first = kinds(&c).daemon_token_of(&machine).await.expect("a token read");
        let second = kinds(&c).daemon_token_of(&machine).await.expect("a token read");
        assert_eq!(first, second);
        kinds(&c).forget_daemon_token("m_1");
        let third = kinds(&c).daemon_token_of(&machine).await.expect("a token read");
        assert_eq!(first, third);
    });
}

fn of_record() -> Value {
    json!({ "repoId": 4242, "owner": "acme", "name": "web", "branch": "wsp/x", "startSha": "a".repeat(40) })
}

/// branch-of-record.ts:167-179: the clone lines run on the entry's machine under a token minted for the thread its
/// first start opens.
#[test]
fn kinds_land_on_record_runs_the_clone_lines_on_the_entrys_machine() {
    let dir = tempfile::tempdir().expect("a folder");
    drive(local_opts(dir.path()), |c, _| async move {
        let machine = kinds(&c).local_machine().await.expect("this computer's machine");
        let entry = entry_over(record(json!({ "branchOfRecord": of_record() })), machine);
        entry.borrow_mut().opens_thread = Some("t_1".into());
        let proj = project("p_1", "web", "/work/web");
        let landing = kinds(&c).land_on_record(&entry.borrow(), proj.clone(), Some(vec!["echo landed".into()]));
        landing.await.expect("a landing");
        let failing = kinds(&c).land_on_record(&entry.borrow(), proj, Some(vec!["echo nope >&2; exit 3".into()]));
        let said = failing.await.refused("a failed clone").message();
        assert!(said.contains("nope"), "{said}");
    });
}

#[test]
fn kinds_land_on_record_refuses_a_thread_with_no_thread_to_scope_its_token_to() {
    let dir = tempfile::tempdir().expect("a folder");
    drive(local_opts(dir.path()), |c, _| async move {
        let machine = kinds(&c).local_machine().await.expect("this computer's machine");
        let entry = entry_over(record(json!({ "branchOfRecord": of_record(), "name": "api" })), machine);
        let landing = kinds(&c).land_on_record(&entry.borrow(), project("p_1", "web", "/work/web"), Some(vec!["true".into()]));
        let said = landing.await.refused("a refusal").message();
        assert_eq!(said, wsp_words::sub_thread::clone_no_thread_refusal("api").message);
    });
}

// ----------------------------------------------------------------------------------------------------------------
// KindsCtx: the broker (kinds.ts:236-330)

#[test]
fn kinds_broker_reads_are_the_options_broker() {
    drive(plain(), |c, _| async move {
        let k = kinds(&c);
        assert!(!k.broker_ready());
        assert!(!k.has_broker_status());
        assert!(k.broker_codex_models().is_none());
        assert!(k.broker_codex_claims().await.expect("no broker, no claims").is_none());
    });
    let claims = CodexClaims { account_id: "acct_1".into(), plan_type: Some("plus".into()), user_id: None, fedramp: None };
    let opts = RuntimeOptions { broker: Some(broker(true, Some(github_status()), Some(claims.clone()))), ..plain() };
    drive(opts, |c, _| async move {
        let k = kinds(&c);
        assert!(k.broker_ready());
        assert!(k.has_broker_status());
        assert_eq!(k.broker_status().await.expect("the status").github.as_deref(), Some("token"));
        assert_eq!(k.broker_codex_claims().await.expect("the claims"), Some(claims));
    });
}

#[test]
fn kinds_broker_ready_is_false_until_the_broker_says_ready() {
    let opts = RuntimeOptions { broker: Some(broker(false, None, None)), ..plain() };
    drive(opts, |c, _| async move {
        assert!(!kinds(&c).broker_ready());
        assert!(!kinds(&c).has_broker_status());
        assert!(!branch(&c).broker_ready());
    });
}

#[test]
fn kinds_a_record_is_brokered_when_it_is_a_cloud_fork_at_the_provider() {
    drive(plain(), |c, _| async move {
        let k = kinds(&c);
        assert!(k.is_brokered(&record(json!({ "kind": "cloud" }))));
        assert!(k.is_brokered(&record(json!({}))));
        assert!(k.is_brokered(&record(json!({ "place": "default" }))));
        assert!(!k.is_brokered(&record(json!({ "place": "pl_other" }))));
        assert!(!k.is_brokered(&record(json!({ "kind": "local" }))));
        assert!(!k.is_brokered(&record(json!({ "kind": "place", "place": "pl_other" }))));
    });
}

#[test]
fn kinds_broker_words_are_the_protocols() {
    drive(plain(), |c, _| async move {
        let k = kinds(&c);
        assert_eq!(k.unbrokered_refusal("claude").map(|e| e.message), None);
        assert_eq!(k.unbrokered_refusal("codex").map(|e| e.message), None);
        let cursor = k.unbrokered_refusal("cursor").expect("cursor holds no key there");
        assert_eq!(cursor.message, "Cursor does not run on a cloud machine yet: wsp keeps no key there. Use Claude Code or Codex.");
        assert_eq!(k.codex_login_unread_refusal().message, wsp_words::place_name::codex_login_unread_refusal().message);
    });
}

#[test]
fn kinds_placeholders_follow_what_the_broker_said() {
    drive(plain(), |c, _| async move {
        let said = BrokerSaid { status: Some(github_status()), claims: None };
        let got = kinds(&c).placeholders_of(&said, "tok");
        let want = wsp_machines::machines::brokered::placeholders_of(said.status.as_ref(), "tok");
        assert_eq!(got, want);
        let nothing = BrokerSaid { status: None, claims: None };
        assert!(kinds(&c).codex_broker_of(&nothing).is_none());
        let claims = CodexClaims { account_id: "acct_1".into(), ..Default::default() };
        let codex =
            BrokerSaid { status: Some(serde_json::from_value(json!({ "codex": "chatgpt" })).expect("a status")), claims: Some(claims) };
        let broker = kinds(&c).codex_broker_of(&codex).expect("a chatgpt login the host holds");
        assert!(matches!(broker.mode, BrokerMode::Chatgpt { .. }));
    });
}

#[test]
fn kinds_brokered_env_holds_the_token_and_the_public_marketplaces() {
    drive(plain(), |c, _| async move {
        let env = kinds(&c).brokered_env("tok_1", &record(json!({})));
        let want = wsp_machines::machines::brokered::brokered_env("tok_1", None, &[]);
        assert_eq!(env, want);
    });
}

// ----------------------------------------------------------------------------------------------------------------
// KindsCtx: the place door (kinds.ts:340-500)

#[test]
fn kinds_has_place_door_follows_the_wiring() {
    drive(plain(), |c, _| async move {
        assert!(!kinds(&c).has_place_door());
    });
    drive(with_door(), |c, _| async move {
        assert!(kinds(&c).has_place_door());
    });
}

#[test]
fn kinds_door_reads_are_the_doors_own_answers() {
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        let k = kinds(&c);
        for place in [HERE_PLACE_ID, "pl_nobody"] {
            assert_eq!(k.door_name_of(place), door.name_of(place), "{place}");
            assert_eq!(k.door_joined(place), door.joined(place), "{place}");
            assert_eq!(k.door_backend_of(place).is_some(), door.backend_of(place).is_some(), "{place}");
            assert_eq!(k.door_folder_computer(place).is_some(), door.folder_computer(place).is_some(), "{place}");
            assert_eq!(k.door_github_from_vault(place), door.github_from_vault(place), "{place}");
            assert_eq!(k.door_sign_ins_at(place), door.sign_ins_at(place, None), "{place}");
            assert_eq!(k.door_logins_of(place).is_some(), door.backend_of(place).and_then(|b| b.logins()).is_some(), "{place}");
        }
        let vault = IndexMap::from([("A".to_string(), "1".to_string())]);
        assert_eq!(k.door_vault_at("pl_nobody", &vault), Some(door.vault_at("pl_nobody", &vault)));
    });
}

#[test]
fn kinds_door_vault_has_no_answer_where_no_door_is_wired() {
    drive(plain(), |c, _| async move {
        let vault = IndexMap::from([("A".to_string(), "1".to_string())]);
        assert_eq!(kinds(&c).door_vault_at("pl_nobody", &vault), None);
        assert!(kinds(&c).door_backend_of("pl_nobody").is_none());
        assert!(!kinds(&c).door_joined("pl_nobody"));
    });
}

#[test]
fn kinds_door_exec_forward_and_report_ask_the_door() {
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        let k = kinds(&c);
        let want = door.exec("pl_nobody", "true", PlaceExecOpts { timeout_ms: Some(1000), stdin: None }).await.refused("no such place");
        let said = k.door_exec("pl_nobody", "true", 1000).await.refused("no such place").message();
        assert_eq!(said, want.message);
        let want = door
            .forward("pl_nobody", 80, wsp_machines::places::types::ForwardAsk { pane: None, standing: false })
            .await
            .map_err(|e| e.message);
        let said = k.door_forward("pl_nobody", 80, None, false).await.map_err(|e| e.message());
        assert_eq!(said.is_ok(), want.is_ok());
        assert_eq!(said.err(), want.err());
        assert_eq!(k.door_report_daemon_version("pl_nobody").await, None);
        let said = k.door_login_user("pl_nobody").await.refused("no folder computer").message();
        assert!(!said.is_empty());
    });
}

// ----------------------------------------------------------------------------------------------------------------
// KindsCtx: a live entry's computer, its project and its daemon (channels.ts:70-130; waits on packet 3 where noted)

#[test]
fn kinds_computer_of_names_the_computer_the_records_place_names() {
    drive(with_door(), |c, rt| async move {
        let machine = machine_named("m_1").await;
        let here = entry_over(record(json!({ "kind": "local" })), machine.clone());
        assert_eq!(kinds(&c).computer_of(&here.borrow()), "this computer");
        let cloud = entry_over(record(json!({})), machine.clone());
        assert_eq!(kinds(&c).computer_of(&cloud.borrow()), wsp_words::words::computer::MACHINE_WSP_FORKS);
        let door = rt.places().expect("the door");
        let placed = entry_over(record(json!({ "kind": "place", "place": HERE_PLACE_ID })), machine);
        assert_eq!(kinds(&c).computer_of(&placed.borrow()), door.name_of(HERE_PLACE_ID));
    });
}

#[test]
fn kinds_served_by_its_computer_only_where_the_machine_offers_the_daemon_frame() {
    drive(plain(), |c, _| async move {
        let entry = entry_over(record(json!({})), machine_named("m_1").await);
        assert!(!kinds(&c).served_by_its_computer(&entry.borrow()));
    });
}

/// Waits on packet 3 (`ChannelsCtx::projects_held`).
#[test]
fn kinds_project_held_and_checkout_of_read_the_held_projects() {
    drive(plain(), |c, _| async move {
        rules(&c).projects_held().borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web"));
        let k = kinds(&c);
        assert_eq!(k.project_held("p_1").expect("held").name, "web");
        let said = k.project_held("p_9").refused("not held").message();
        assert_eq!(said, wsp_words::projects::no_such_project_line("p_9", &["web".to_string()]));
        assert_eq!(k.checkout_of(&record(json!({}))).expect("the project's folder"), "/work/web");
        let sub = record(json!({ "sub": { "folder": "/work/sub", "branch": "b", "lead": "t_1", "repo": "r", "base": "main", "out": {} } }));
        assert_eq!(k.checkout_of(&sub).expect("the sub folder"), "/work/sub");
    });
}

#[test]
fn kinds_hello_version_is_none_for_a_machine_with_no_preview() {
    drive(plain(), |c, _| async move {
        let entry = entry_over(record(json!({})), machine_named("m_1").await);
        let version = kinds(&c).hello_version(&entry.borrow());
        assert_eq!(version.await, None);
    });
}

#[test]
fn kinds_clone_lines_are_the_project_landings() {
    drive(plain(), |c, _| async move {
        let o = json!({
            "source": "git", "remote": "https://github.com/acme/web.git", "checkout": "/work/web", "computer": "boat",
            "branch": "main",
        });
        let ask = wsp_core::project_landing::CloneAsk {
            source: wsp_core::project_sources::project_source("git").expect("the git source"),
            remote: "https://github.com/acme/web.git".into(),
            checkout: "/work/web".into(),
            computer: "boat".into(),
            branch: Some("main".into()),
            setup_git: None,
            brokered: None,
        };
        let lines = kinds(&c).clone_lines(o);
        assert!(!lines.is_empty());
        assert_eq!(lines, wsp_core::project_landing::clone_lines(&ask));
    });
}

#[test]
fn kinds_daemon_tokens_are_forgotten_by_machine_id() {
    drive(plain(), |c, _| async move {
        kinds(&c).forget_daemon_token("m_none");
    });
}

#[test]
fn kinds_sleep_settles_on_the_clock() {
    let fake = FakeClock::new(1_000);
    let opts = RuntimeOptions { clock: Some(fake.clock()), ..plain() };
    drive(opts, |c, _| async move {
        let slept = kinds(&c).sleep(50.0);
        let mut slept = std::pin::pin!(slept);
        assert!(futures::poll!(slept.as_mut()).is_pending());
        fake.advance(50);
        slept.await;
    });
}

#[test]
fn kinds_warn_is_a_host_log_line() {
    drive(plain(), |c, _| async move {
        kinds(&c).warn("a line");
        ends_rt(&c).warn("a line");
        records(&c).warn("a line");
        branch(&c).warn("a line");
    });
}

// ----------------------------------------------------------------------------------------------------------------
// BackendFor (kinds.ts backendFor)

#[test]
fn backend_for_a_cloud_record_is_the_wired_backend() {
    let wired = backend();
    let opts = RuntimeOptions { backend: Some(wired.clone()), ..plain() };
    drive(opts, |c, _| async move {
        let got = backend_for(&c).backend_for(&record(json!({}))).expect("the wired backend");
        assert!(same_backend(&got, &wired));
    });
}

#[test]
fn backend_for_a_record_with_no_machine_released_is_the_released_backend() {
    drive(plain(), |c, _| async move {
        let released = record(json!({ "phase": "released" }));
        let got = backend_for(&c).backend_for(&released).expect("a backend");
        let said = got.get("m_1").await.refused("released").to_string();
        assert!(said.contains(RELEASED_LINE), "{said}");
    });
}

// ----------------------------------------------------------------------------------------------------------------
// ThreadEndsRuntime and ThreadEndsDoor (thread-ends.ts)

#[test]
fn thread_ends_runtime_reads_and_writes_the_store() {
    drive(plain(), |c, _| async move {
        let rt = ends_rt(&c);
        assert_eq!(rt.store_get("thread-ends", "pl_1").await.expect("a read"), None);
        rt.store_put("thread-ends", "pl_1", json!({ "threads": ["t_1"] })).await.expect("a write");
        assert_eq!(rt.store_get("thread-ends", "pl_1").await.expect("a read"), Some(json!({ "threads": ["t_1"] })));
        rt.store_delete("thread-ends", "pl_1").await.expect("a delete");
        assert_eq!(rt.store_get("thread-ends", "pl_1").await.expect("a read"), None);
    });
}

/// Waits on packet 11 (`ThreadsArea::running_on` and `launching_on` read the sessions through `ThreadsCtx`) and on
/// packet 13 (`SessionsArea::stop_holds`).
#[test]
fn thread_ends_runtime_asks_the_threads_and_sessions_areas() {
    drive(with_door(), |c, _| async move {
        let rt = ends_rt(&c);
        assert!(!rt.running_on("t_none"));
        assert!(!rt.launching_on("t_none"));
        assert!(!rt.stop_holds("t_none"));
    });
}

/// Waits on packet 5 (`ready` is the boot's one start).
#[test]
fn thread_ends_runtime_ready_is_the_boot() {
    drive(plain(), |c, _| async move {
        ends_rt(&c).ready().await.expect("booted");
        held(&c).ready().await.expect("booted");
    });
}

#[test]
fn thread_ends_door_asks_the_place_door() {
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        let d = ends_door(&c);
        assert_eq!(d.name_of("pl_nobody"), door.name_of("pl_nobody"));
        assert_eq!(d.link("pl_nobody"), door.link("pl_nobody").is_some());
        let want = door.exec("pl_nobody", "true", PlaceExecOpts { timeout_ms: Some(500), stdin: None }).await.refused("no such place");
        let said = d.exec("pl_nobody", "true", 500).await.refused("no such place");
        assert_eq!(said.message, want.message);
    });
}

/// The ends area exists only where the host wired a place door: a kind-place start asks it, nothing else does.
#[test]
fn kinds_end_thread_and_end_owed_go_through_the_thread_ends_area() {
    drive(with_door(), |c, _| async move {
        let k = kinds(&c);
        let owed = k.end_owed("pl_nobody", "t_1").await.expect("nothing owed");
        assert!(owed.is_none());
        assert_eq!(k.end_thread("pl_nobody", "t_1", EndThread::default()).await.expect("nothing to end"), None);
    });
}

// ----------------------------------------------------------------------------------------------------------------
// RulesCtx (rules.ts:7-334)

#[test]
fn rules_no_machines_line_is_the_option() {
    let opts = RuntimeOptions { no_machines_line: Some("no machines here".into()), ..plain() };
    drive(opts, |c, _| async move {
        assert_eq!(rules(&c).no_machines_line().as_deref(), Some("no machines here"));
    });
    drive(plain(), |c, _| async move {
        assert_eq!(rules(&c).no_machines_line(), None);
    });
}

#[test]
fn rules_store_put_project_writes_the_projects_collection() {
    drive(plain(), |c, _| async move {
        let held = project("p_1", "web", "/work/web");
        rules(&c).store_put_project(&held).await.expect("a write");
        let stored = records(&c).store().get("projects", "p_1").await.expect("a read").expect("stored");
        assert_eq!(stored["name"], json!("web"));
        assert_eq!(stored["id"], json!("p_1"));
    });
}

#[test]
fn rules_door_name_of_is_none_where_no_door_is_wired() {
    drive(plain(), |c, _| async move {
        assert_eq!(rules(&c).door_name_of("pl_1"), None);
    });
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        assert_eq!(rules(&c).door_name_of("pl_1"), Some(door.name_of("pl_1")));
    });
}

#[test]
fn rules_landing_and_projects_held_are_shared_handles() {
    drive(plain(), |c, _| async move {
        let r = rules(&c);
        r.landing().borrow_mut().insert("t_1".into(), 2);
        assert_eq!(r.landing().borrow().get("t_1"), Some(&2));
        r.projects_held().borrow_mut().insert("p_1".into(), project("p_1", "web", "/w"));
        assert_eq!(r.projects_held().borrow().len(), 1);
    });
}

/// Waits on packet 3 (`ChannelsArea::project_held` reads the map through `ChannelsCtx::projects_held`).
#[test]
fn rules_project_held_reads_the_shared_map() {
    drive(plain(), |c, _| async move {
        let r = rules(&c);
        r.projects_held().borrow_mut().insert("p_1".into(), project("p_1", "web", "/w"));
        assert_eq!(r.project_held("p_1").expect("held").name, "web");
    });
}

#[test]
fn rules_live_maps_are_empty_on_a_fresh_runtime() {
    drive(plain(), |c, _| async move {
        let r = rules(&c);
        assert!(r.live().is_empty());
        assert!(r.live_get("w_1").is_none());
        assert_eq!(r.sessions_size(), 0);
        assert_eq!(r.thread_records_size(), 0);
        assert!(r.thread_record("t_1").is_none());
    });
}

/// Waits on packet 11 (`AgentsArea::rows_on` reads the sessions through `AgentsCtx`).
#[test]
fn rules_rows_on_a_thread_with_no_session_are_none() {
    drive(plain(), |c, _| async move {
        assert!(rules(&c).rows_on("t_none").is_empty());
    });
}

/// Waits on packet 16 (`PreferencesArea::get` and `set`).
#[test]
fn rules_preferences_read_and_write_through_the_preferences_area() {
    drive(plain(), |c, _| async move {
        let r = rules(&c);
        assert_eq!(r.target_workspace().await.expect("preferences"), None);
        r.set_preferences(json!({ "target": { "workspace": "w_9" } })).await.expect("a set");
        assert_eq!(r.target_workspace().await.expect("preferences").as_deref(), Some("w_9"));
    });
}

#[test]
fn rules_capabilities_are_the_backends_for_the_record() {
    let wired = backend();
    let caps = wired.capabilities();
    let opts = RuntimeOptions { backend: Some(wired), ..plain() };
    drive(opts, |c, _| async move {
        let r = rules(&c);
        let rec = record(json!({}));
        assert_eq!(r.capability(&rec, "replacesMachine"), caps.replaces_machine);
        assert_eq!(r.capability(&rec, "liveCloneForks"), caps.live_clone_forks);
        assert_eq!(r.capability(&rec, "diskSnapshots"), caps.disk_snapshots);
        assert!(!r.capability(&rec, "noSuchCapability"));
        assert_eq!(r.pause_mode_declared(&rec), caps.pause_mode.is_some());
        assert_eq!(r.forks_no_machines(&rec), caps.sizes.is_empty());
    });
}

#[test]
fn rules_the_kind_modules_answer_relayed_reach_and_mcp() {
    drive(plain(), |c, _| async move {
        let r = rules(&c);
        assert!(r.relayed(WorkspaceKind::Cloud, Some("m_1")));
        assert!(!r.relayed(WorkspaceKind::Local, None));
        let entry = entry_over(record(json!({})), machine_named("m_1").await);
        assert_eq!(r.turn_reach(&entry), Some(serde_json::Map::new()));
        assert!(r.wsp_mcp(&entry).is_some());
    });
}

/// Waits on packet 3 (`ChannelsArea::same_repository`, `project_held`, `place_id_of`, `agents_held`).
#[test]
fn rules_channels_reads_answer_the_channels_area() {
    drive(plain(), |c, _| async move {
        let r = rules(&c);
        r.projects_held().borrow_mut().insert("p_1".into(), project("p_1", "web", "/w"));
        assert_eq!(r.project_held("p_1").expect("held").name, "web");
        assert!(r.project_held("p_x").is_err());
        assert_eq!(r.place_id_of(&record(json!({ "kind": "local" }))).as_deref(), Some(HERE_PLACE_ID));
        assert_eq!(r.place_id_of(&record(json!({}))).as_deref(), Some("default"));
        let a = project("p_1", "web", "/w");
        let b = project("p_2", "web2", "/w2");
        assert!(r.same_repository(&a, &a, None));
        assert!(!r.same_repository(&a, &b, None));
        assert_eq!(r.agents_held(&record(json!({}))), wsp_wire::types::place_state::AGENTS_ON);
    });
}

#[test]
fn rules_kind_of_a_computer_and_place_name_follow_the_door() {
    drive(plain(), |c, _| async move {
        let r = rules(&c);
        assert_eq!(r.kind_of(HERE_PLACE_ID), WorkspaceKind::Local);
        assert_eq!(r.kind_of("default"), WorkspaceKind::Cloud);
    });
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        assert_eq!(rules(&c).place_name("pl_x"), door.name_of("pl_x"));
        assert_eq!(rules(&c).place_name(HERE_PLACE_ID), door.name_of(HERE_PLACE_ID));
    });
}

// ----------------------------------------------------------------------------------------------------------------
// RecordsCtx (records.ts)

#[test]
fn records_store_clock_and_wired_are_the_cores() {
    let fake = FakeClock::new(42);
    let opts = RuntimeOptions { clock: Some(fake.clock()), ..plain() };
    drive(opts, |c, _| async move {
        let r = records(&c);
        assert_eq!(r.clock().now(), 42);
        assert_eq!(r.wired(), "default");
        r.store().put("probe", "a", json!(1)).await.expect("a write");
        assert_eq!(r.store().get("probe", "a").await.expect("a read"), Some(json!(1)));
        assert_eq!(branch(&c).clock().now(), 42);
        assert_eq!(branch(&c).wired(), "default");
        assert_eq!(held(&c).wired(), "default");
        held(&c).store().put("probe", "b", json!(2)).await.expect("a write");
        assert_eq!(r.store().get("probe", "b").await.expect("a read"), Some(json!(2)));
    });
}

#[test]
fn records_live_records_are_none_on_a_fresh_runtime_and_a_bump_finds_no_workspace() {
    drive(plain(), |c, _| async move {
        let r = records(&c);
        assert!(r.live_records().is_empty());
        assert!(!r.bump_generation("w_none"));
        assert!(branch(&c).live_records().is_empty());
    });
}

#[test]
fn records_backend_for_is_the_kinds_areas() {
    let wired = backend();
    let opts = RuntimeOptions { backend: Some(wired.clone()), ..plain() };
    drive(opts, |c, _| async move {
        let got = records(&c).backend_for(&record(json!({}))).expect("the wired backend");
        assert!(same_backend(&got, &wired));
    });
}

#[test]
fn records_module_folder_is_the_kinds_modules() {
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().to_string_lossy().to_string();
    drive(local_opts(dir.path()), |c, _| async move {
        let r = records(&c);
        let local = record(json!({ "kind": "local" }));
        assert!(r.module_folder(&local).is_some());
        let cloud = record(json!({}));
        assert_eq!(r.module_folder(&cloud), None);
        let _ = &root;
    });
}

/// Waits on packet 3 (`ChannelsArea::home_of`, `provider_of`, `project_held` and the daemon notes).
#[test]
fn records_channels_reads_answer_the_channels_area() {
    drive(plain(), |c, _| async move {
        let r = records(&c);
        assert_eq!(r.provider_of(&record(json!({}))).as_deref(), Some("default"));
        assert_eq!(r.provider_of(&record(json!({ "kind": "local" }))), None);
        assert_eq!(r.daemon_note("w_none"), None);
        rules(&c).projects_held().borrow_mut().insert("p_1".into(), project("p_1", "web", "/w"));
        assert_eq!(r.project_ref("p_1").expect("held").name, "web");
        assert!(r.project_ref("p_x").is_err());
        let _ = r.home_of(&record(json!({ "kind": "local" })));
    });
}

#[test]
fn records_agents_of_is_the_rules_areas_as_json() {
    drive(plain(), |c, _| async move {
        let got = records(&c).agents_of(&record(json!({}))).expect("a record's agents are held");
        assert!(got.is_object());
    });
}

// ----------------------------------------------------------------------------------------------------------------
// HeldCtx (held.ts)

/// Waits on packet 5 for the boot read; the call itself is the records area's `drop_copy_recipe`.
#[test]
fn held_drop_copy_recipe_asks_the_records_area() {
    drive(plain(), |c, _| async move {
        let _ = held(&c).drop_copy_recipe("default", "default", 1).await;
    });
}

// ----------------------------------------------------------------------------------------------------------------
// BranchCtx (branch-of-record.ts)

#[test]
fn branch_home_dir_is_the_local_wiring_home_or_the_process_home() {
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().to_string_lossy().to_string();
    drive(local_opts(dir.path()), |c, _| async move {
        assert_eq!(branch(&c).home_dir(), root);
    });
    drive(plain(), |c, _| async move {
        assert!(!branch(&c).home_dir().is_empty());
    });
}

#[test]
fn branch_read_file_reads_this_computer_and_maps_a_failure() {
    let dir = tempfile::tempdir().expect("a folder");
    let path = dir.path().join("note.txt");
    std::fs::write(&path, "hello").expect("a file");
    let gone = dir.path().join("gone.txt");
    drive(plain(), |c, _| async move {
        assert_eq!(branch(&c).read_file(&path.to_string_lossy()).await.expect("the file"), "hello");
        let said = branch(&c).read_file(&gone.to_string_lossy()).await.refused("a refusal").message;
        assert!(said.contains("ENOENT"), "{said}");
    });
}

#[test]
fn branch_exec_here_runs_on_this_computers_machine() {
    let dir = tempfile::tempdir().expect("a folder");
    drive(local_opts(dir.path()), |c, _| async move {
        let ran = branch(&c).exec_here("echo hi", 5_000).await.expect("a run");
        assert_eq!(ran.stdout.trim(), "hi");
        assert_eq!(ran.exit_code, wsp_wire::types::js_number::JsInt::from(0));
    });
    drive(plain(), |c, _| async move {
        let said = branch(&c).exec_here("echo hi", 5_000).await.refused("no local road").message;
        assert_eq!(said, wsp_words::words::computer::no_kind_line("local"));
    });
}

#[test]
fn branch_env_exec_runs_a_script_on_a_machine_under_an_env() {
    let dir = tempfile::tempdir().expect("a folder");
    drive(local_opts(dir.path()), |c, _| async move {
        let machine = kinds(&c).local_machine().await.expect("this computer's machine");
        let env = IndexMap::from([("WSP_PROBE".to_string(), "seen".to_string())]);
        let script = wsp_engine::run_env::with_env_from_input("printf %s \"$WSP_PROBE\"");
        let ran = branch(&c).env_exec(machine, &script, env, 5_000).await.expect("a run");
        assert_eq!(ran.stdout, "seen");
    });
}

#[test]
fn branch_mint_and_revoke_go_to_the_device_door() {
    drive(plain(), |c, _| async move {
        let scope: wsp_wire::views::workspace_more::ThreadScope = serde_json::from_value(json!({
            "kind": "thread", "threadId": "t_1", "workspaceId": "w_1", "rootThreadId": "t_1",
        }))
        .expect("a thread scope");
        let minted = branch(&c).mint("thread t1", scope, 1_000, Default::default()).await.expect("a device");
        assert!(!minted.device_token.is_empty());
        assert!(branch(&c).revoke(&minted.device_id).await.expect("a revoke"));
        assert!(!branch(&c).revoke(&minted.device_id).await.expect("a second revoke"));
    });
}

#[test]
fn branch_turn_road_is_the_kinds_module_turn_road() {
    drive(plain(), |c, _| async move {
        let more = branch(&c).turn_road_of(&record(json!({}))).expect("the cloud kind has a module");
        let _ = more;
    });
    drive(plain(), |c, _| async move {
        let said = branch(&c).turn_road_of(&record(json!({ "kind": "local" }))).refused("no local module").message;
        assert_eq!(said, wsp_words::words::computer::no_kind_line("local"));
    });
}

#[test]
fn branch_marketplace_files_are_the_catalog_agents_that_name_github_repos() {
    drive(plain(), |c, _| async move {
        let files = branch(&c).marketplace_files();
        assert!(!files.is_empty());
        for file in &files {
            assert!(!file.path.starts_with("~/"), "{}", file.path);
            assert!((file.github_repos)("").is_empty());
        }
    });
}

#[test]
fn branch_brokered_git_env_and_on_record_lines_are_the_project_landings() {
    drive(plain(), |c, _| async move {
        let env = branch(&c).brokered_git_env("tok", None);
        assert_eq!(env, wsp_core::project_landing::brokered_git_env("tok", None, &[]));
        let of_record: wsp_wire::views::workspace::BranchOfRecord = serde_json::from_value(json!({
            "repoId": 4242, "owner": "acme", "name": "web", "branch": "wsp/x", "startSha": "a".repeat(40),
        }))
        .expect("a branch of record");
        let got = branch(&c).on_record_lines("/work/web", &of_record, "acme/web", false);
        assert!(!got.is_empty());
    });
}

/// Waits on packet 6 (the hop through `ChannelsArea` for the mint's thread road) for the ops; the area itself is built
/// over the context.
#[test]
fn branch_persist_writes_the_record_through_the_records_area() {
    drive(plain(), |c, _| async move {
        let rec = record(json!({}));
        let _ = branch(&c).persist(&rec).await;
    });
}

// ----------------------------------------------------------------------------------------------------------------
// ImageDoor and AgentsPlaces: the place door's adapters (builders.ts, golden.ts, image.ts, agents-read.ts)

#[test]
fn image_door_names_and_backends_are_the_doors_own() {
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        let d = image_door(&c);
        assert_eq!(d.name_of(HERE_PLACE_ID), Some(door.name_of(HERE_PLACE_ID)));
        assert_eq!(d.backend_of("pl_nobody").is_some(), door.backend_of("pl_nobody").is_some());
        let said = d.forking_backend("pl_nobody").await.refused("no such place").message;
        let want = door.forking_backend("pl_nobody").await.refused("no such place").message;
        assert_eq!(said, want);
    });
}

/// Waits on packet 5: the door's reads of its rows wait on the boot.
#[test]
fn image_door_place_words_and_rows_are_the_doors_own() {
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        let d = image_door(&c);
        for word in ["pl_nobody", HERE_PLACE_ID] {
            let want = door.place_for(word).await.map(|found| found.place_id).map_err(|e| e.message);
            assert_eq!(d.place_for(word).await.map_err(|e| e.message), want, "{word}");
        }
        let want = door.default_place().await.expect("a default").place_id;
        assert_eq!(d.default_place().await.expect("a default"), want);
        let want: Vec<String> = door.list(1_000).await.expect("rows").into_iter().map(|p| p.id).collect();
        let got: Vec<String> = d.list(1_000).await.expect("rows").into_iter().map(|p| p.id).collect();
        assert_eq!(got, want);
    });
}

#[test]
fn image_door_marks_the_default_place_as_the_door_does() {
    let marked = Rc::new(RefCell::new(None));
    let seen = marked.clone();
    drive(with_door(), |_, rt| async move {
        let door = rt.places().expect("the door");
        door.mark_default_if_none("pl_x").await.expect("a mark");
        *seen.borrow_mut() = Some(door.default_place().await.map(|d| d.place_id).map_err(|e| e.message));
    });
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        image_door(&c).mark_default_if_none("pl_x").await.expect("a mark");
        let got = door.default_place().await.map(|d| d.place_id).map_err(|e| e.message);
        assert_eq!(Some(got), marked.borrow().clone());
    });
}

/// Waits on packet 5: the door's reads of its rows wait on the boot.
#[test]
fn agents_places_rows_and_reports_are_the_doors_own() {
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        let p = agents_places(&c);
        let want: Vec<String> = door.list(1_000).await.expect("rows").into_iter().map(|row| row.id).collect();
        let got: Vec<String> = p.list(1_000).await.expect("rows").into_iter().map(|row| row.id).collect();
        assert_eq!(got, want);
        assert_eq!(p.report_of("pl_nobody").await.expect("a read"), door.report_of("pl_nobody").await.expect("a read"));
    });
}

#[test]
fn agents_places_sign_ins_exec_and_landings_are_the_doors_own() {
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        let p = agents_places(&c);
        let agents = ["claude".to_string()];
        assert_eq!(p.sign_ins_at("pl_nobody", &agents), door.sign_ins_at("pl_nobody", Some(&agents)));
        let ask = PlaceExecOpts { timeout_ms: Some(900), stdin: Some(b"in".to_vec()) };
        let want = door.exec("pl_nobody", "true", ask).await.refused("no such place");
        let said = p.exec("pl_nobody", "true", Some(900), Some(b"in".to_vec())).await.refused("no such place");
        assert_eq!(said.message, want.message);
        let want = door.logins_again("pl_nobody").await.err().map(|e| e.message);
        assert_eq!(p.logins_again("pl_nobody").await.err().map(|e| e.message), want);
        let want = door.login_landed("pl_nobody", "claude").await.err().map(|e| e.message);
        assert_eq!(p.login_landed("pl_nobody", "claude").await.err().map(|e| e.message), want);
        let want = door.key_landed("claude").await.err().map(|e| e.message);
        assert_eq!(p.key_landed("claude").await.err().map(|e| e.message), want);
    });
}

#[test]
fn agents_places_gh_there_asks_the_door_and_reports_each_step() {
    drive(with_door(), |c, rt| async move {
        let door = rt.places().expect("the door");
        let steps = Rc::new(RefCell::new(Vec::<String>::new()));
        let seen = steps.clone();
        let step: wsp_machines::agents_read::GhStep = Rc::new(move |detail, _at, _row| seen.borrow_mut().push(detail));
        let said = agents_places(&c).gh_there("pl_nobody", step).await.err().map(|e| e.message);
        let want = door.gh_there("pl_nobody", Arc::new(|_, _, _| {})).await.err().map(|e| e.message);
        assert_eq!(said, want);
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The adapters without a door: the TS reads `placeDoor` as undefined and the call throws a TypeError

#[test]
fn the_place_door_adapters_without_a_door_say_the_ts_type_error() {
    drive(plain(), |c, _| async move {
        let said = image_door(&c).forking_backend("pl_1").await.refused("no door").message;
        assert_eq!(said, "Cannot read properties of undefined (reading 'forkingBackend')");
        let said = agents_places(&c).logins_again("pl_1").await.refused("no door").message;
        assert_eq!(said, "Cannot read properties of undefined (reading 'loginsAgain')");
        let said = ends_door(&c).exec("pl_1", "true", 1).await.refused("no door").message;
        assert_eq!(said, "Cannot read properties of undefined (reading 'exec')");
        assert!(image_door(&c).backend_of("pl_1").is_none());
        assert_eq!(image_door(&c).name_of("pl_1"), None);
    });
}

// ----------------------------------------------------------------------------------------------------------------
// Whole-runtime tests ported from `machine-gates.test.ts` (65f6b89c7). Each runs the ops, so each waits on the boot
// (packet 5), the create (packet 6) and the workspaces area (packets 8 and 9).

/// machine-gates.test.ts: "a host holding cloud records with no provider says no provider is set up here", its second
/// half: a host restarted without its provider env holds a cloud workspace it can say nothing else about, and neither
/// rebuild nor upgrade tells a person their fork is their own computer. Waits on packets 5 (boot), 8 and 9 (the
/// workspaces ops).
#[test]
fn a_host_restarted_without_its_provider_says_no_provider_is_set_up_here() {
    let store = Rc::new(memory_store());
    let opts = RuntimeOptions {
        backend: Some(Arc::new(wsp_engine::no_provider_backend::NoProviderBackend::new(None))),
        store: Some(store.clone()),
        ..Default::default()
    };
    drive(opts, |c, rt| async move {
        use wsp_store::store::Store as _;
        store.put("projects", "p_1", serde_json::to_value(project("p_1", "web", "/work/web")).unwrap()).await.expect("a project");
        store.put("workspaces", "w_1", serde_json::to_value(record(json!({}))).unwrap()).await.expect("a record");
        let ws = rt.workspaces();
        for said in [ws.rebuild("w_1", None).await, ws.upgrade("w_1", None).await] {
            let said = said.refused("a refusal").message;
            assert_eq!(said, wsp_words::words::thread::NO_PROVIDER_LINE);
            assert!(!said.contains("which wsp does not run"));
        }
        let _ = c;
    });
}
