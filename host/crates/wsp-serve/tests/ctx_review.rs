// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 10 (#2476): the context's review and projects readings (`ReviewCtx`, `ProjectsCtx`,
//! `MoveRoads`, the account's `LandingRoad` and `ProjectIconsCtx`). Each test builds the runtime on a throwaway state
//! over `FakeBackend` (or a `LocalBackend` where a command must run) and calls the method the way the area crate
//! does, through `&dyn Trait`. The TS is the oracle: each expectation is what `workspaces.ts:1055-1247`,
//! `projects.ts`, `project-landing.ts`, `images.ts` and the area each member forwards to answer. A test that goes
//! through another packet's bodies says so on the test: the start-from and workspace seam reads wait on packet 8, the
//! phases and git reads on 9, the agents on 11, the builders on 14, the preferences on 16, and the whole-runtime ports
//! of `projects`, `project-icons`, `project-export` and `project-import` wait on all of them.
use std::cell::RefCell;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wsp_account::projects::images::ProjectIconsCtx;
use wsp_account::projects::projects::add::{JoinedComputer, LandingDeps, LandingKind, LandingRoad};
use wsp_account::projects::projects::moves::MoveRoads;
use wsp_account::projects::projects::ProjectsCtx;
use wsp_account::projects::workspaces::review::ReviewCtx;
use wsp_core::github_cache::memory_github_cache;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::wiring::{
    LiveWorkspace, LocalWiring, Platform, RuntimeOptions, SeedPack, SeedWiring, WorkspaceRecord, PORT_PROBE_BODY_CAP,
};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions, LOCAL_MACHINE_ID};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::memory_store;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::workspace::{Caller, ProjectSource, ProjectView, SeedChoice, SeedPlan, WorkspaceOrigin};
use wsp_wire::views::workspace_more::ProjectBranch;
use wsp_words::exit::HostError;
use wsp_words::words::computer::relayed_record_refusal;

// ----------------------------------------------------------------------------------------------------------------
// Fixtures

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

fn plain() -> RuntimeOptions {
    RuntimeOptions { backend: Some(backend()), store: Some(Rc::new(memory_store())), ..Default::default() }
}

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
fn entry_over(record: WorkspaceRecord, machine: Arc<dyn Machine>) -> wsp_machines::machines::machines::Entry {
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
    Rc::new(RefCell::new(LiveWorkspace {
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

async fn local_entry(root: &Path) -> wsp_machines::machines::machines::Entry {
    let local = LocalBackend::new(LocalBackendOptions { root: root.to_path_buf(), env: None });
    let machine = local.get(LOCAL_MACHINE_ID).await.expect("the local machine");
    entry_over(record(json!({})), machine)
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

fn projects(c: &Rc<Ctx>) -> &dyn ProjectsCtx {
    &**c
}
fn moves(c: &Rc<Ctx>) -> &dyn MoveRoads {
    &**c
}
fn review(c: &Rc<Ctx>) -> &dyn ReviewCtx {
    &**c
}
fn icons(c: &Rc<Ctx>) -> &dyn ProjectIconsCtx {
    &**c
}
fn road(c: &Rc<Ctx>, kind: LandingKind) -> Rc<dyn LandingRoad> {
    ProjectsCtx::landing_road(&**c, kind)
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git").arg("-C").arg(dir).args(args).status().expect("git runs").success();
    assert!(ok, "git {args:?} failed in {dir:?}");
}

fn repo_in(root: &Path, name: &str) -> std::path::PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("a folder");
    git(&dir, &["init", "-q", "-b", "main"]);
    dir
}

fn some_adapter() -> HarnessAdapterFactory {
    Arc::new(|_| Err(HostError::new("this test starts no agent")))
}

fn road_deps(computer_name: &str, mac_state_home: &str) -> LandingDeps {
    LandingDeps {
        computer: None,
        mac_state_home: mac_state_home.to_string(),
        computer_name: computer_name.to_string(),
        clone_env: IndexMap::new(),
    }
}

fn caller_relayed() -> Caller {
    Caller::Road(WorkspaceOrigin::Relayed)
}

struct NoSeed;

#[async_trait::async_trait(?Send)]
impl SeedWiring for NoSeed {
    async fn plan(&self, _folder: &str, _homes: &IndexMap<String, String>) -> Result<SeedPlan, HostError> {
        Err(HostError::new("no plan in this test"))
    }
    async fn pack(&self, _plan: SeedPlan, _choice: SeedChoice, _homes: &IndexMap<String, String>) -> Result<SeedPack, HostError> {
        Err(HostError::new("no pack in this test"))
    }
}

// ----------------------------------------------------------------------------------------------------------------
// ProjectsCtx: the options and the core's reads (projects.ts:108-150, 222, 425-452)

#[test]
fn projects_state_path_seed_and_cloud_projects_are_the_options() {
    drive(plain(), |c, _| async move {
        assert!(!projects(&c).has_state_path());
        assert!(projects(&c).seed_wiring().is_none());
        assert_eq!(projects(&c).cloud_projects(), None);
    });
    let seed: Rc<dyn SeedWiring> = Rc::new(NoSeed);
    let held = seed.clone();
    let opts =
        RuntimeOptions { state_path: Some("/tmp/wsp-p10/state.json".into()), seed: Some(seed), cloud_projects: Some(false), ..plain() };
    drive(opts, |c, _| async move {
        assert!(projects(&c).has_state_path());
        let got = projects(&c).seed_wiring().expect("the seed the options hold");
        assert!(Rc::ptr_eq(&got, &held));
        assert_eq!(projects(&c).cloud_projects(), Some(false));
    });
    drive(RuntimeOptions { cloud_projects: Some(true), ..plain() }, |c, _| async move {
        assert_eq!(projects(&c).cloud_projects(), Some(true));
    });
}

#[test]
fn projects_state_folder_is_the_folder_of_the_state_file_and_refuses_with_none() {
    drive(plain(), |c, _| async move {
        let e = projects(&c).state_folder().expect_err("no state file");
        assert_eq!(e.message, "this runtime was given no state file, so it keeps no worktrees or copies");
    });
    let opts = RuntimeOptions { state_path: Some("/tmp/wsp-p10/state.json".into()), ..plain() };
    drive(opts, |c, _| async move {
        assert_eq!(projects(&c).state_folder().expect("a folder"), "/tmp/wsp-p10");
        assert_eq!(icons(&c).state_folder().expect("a folder"), Path::new("/tmp/wsp-p10"));
    });
}

#[test]
fn projects_vault_github_token_is_the_vaults_one_where_it_holds_one() {
    drive(plain(), |c, _| async move {
        assert_eq!(projects(&c).vault_github_token(), None);
    });
    let empty: Rc<dyn Fn() -> IndexMap<String, String>> = Rc::new(IndexMap::new);
    drive(RuntimeOptions { vault: Some(empty), ..plain() }, |c, _| async move {
        assert_eq!(projects(&c).vault_github_token(), None);
    });
    let held: Rc<dyn Fn() -> IndexMap<String, String>> =
        Rc::new(|| IndexMap::from([(wsp_engine::run_env::GITHUB_TOKEN_ENV.to_string(), "tok".to_string())]));
    drive(RuntimeOptions { vault: Some(held), ..plain() }, |c, _| async move {
        assert_eq!(projects(&c).vault_github_token(), Some("tok".to_string()));
    });
}

#[test]
fn projects_local_claude_home_is_the_locals_claude_home() {
    let root = tempfile::tempdir().expect("a folder");
    drive(plain(), |c, _| async move {
        assert_eq!(projects(&c).local_claude_home(), None);
    });
    let at = root.path().to_string_lossy().to_string();
    drive(RuntimeOptions { local: Some(local_in(root.path())), ..plain() }, |c, _| async move {
        assert_eq!(projects(&c).local_claude_home(), Some(format!("{at}/.claude")));
    });
}

#[test]
fn projects_runs_agent_and_first_agent_read_the_adapters() {
    drive(plain(), |c, _| async move {
        assert!(!projects(&c).runs_agent("claude"));
        assert_eq!(projects(&c).first_agent(), wsp_catalog::catalog::default_agent().id);
    });
    let last = wsp_catalog::catalog::catalog_agents().last().expect("an agent").id.clone();
    let opts = RuntimeOptions { adapters: IndexMap::from([(last.clone(), some_adapter())]), ..plain() };
    drive(opts, |c, _| async move {
        assert!(projects(&c).runs_agent(&last));
        assert!(!projects(&c).runs_agent("nothing"));
        assert_eq!(projects(&c).first_agent(), last);
    });
}

#[test]
fn projects_place_door_reads_answer_none_and_false_without_a_door_or_a_place() {
    drive(plain(), |c, _| async move {
        assert!(projects(&c).folder_computer("pl_x").is_none());
        assert!(!projects(&c).github_from_vault("pl_x"));
    });
    drive(RuntimeOptions { place_links: Some(Rc::new(wiring())), ..plain() }, |c, _| async move {
        assert!(projects(&c).folder_computer("pl_x").is_none());
        assert!(!projects(&c).github_from_vault("pl_x"));
    });
}

#[test]
fn projects_refuse_recording_refuses_a_relayed_road_with_the_words_and_lets_the_rest_through() {
    drive(plain(), |c, _| async move {
        assert!(projects(&c).refuse_recording("a folder", None).is_ok());
        assert!(projects(&c).refuse_recording("a folder", Some(&Caller::Road(WorkspaceOrigin::Here))).is_ok());
        let e = projects(&c).refuse_recording("a folder", Some(&caller_relayed())).expect_err("relayed");
        assert_eq!(e.message, relayed_record_refusal("a folder"));
    });
}

#[test]
fn projects_rules_reads_answer_false_and_none_for_a_caller_with_no_scope() {
    drive(plain(), |c, _| async move {
        assert_eq!(projects(&c).project_of_scope(None), None);
        assert_eq!(projects(&c).project_of_scope(Some(&caller_relayed())), None);
        assert!(!projects(&c).project_reached(None, "pr_1").expect("a read"));
        assert!(!projects(&c).of_threads_repository(None, "pr_1").expect("a read"));
        let p = project("pr_1", "api", "/tmp/api");
        assert!(projects(&c).elsewhere_refusal(None, &p, "spawn").expect("a read").is_none());
    });
}

#[test]
fn projects_computer_rows_name_this_computer_and_the_wired_place() {
    drive(plain(), |c, _| async move {
        let rows = projects(&c).computer_rows().await.expect("rows");
        assert!(rows.iter().any(|r| r.id == HERE_PLACE_ID), "this computer is a row");
        assert!(rows.iter().any(|r| r.id == "default"), "the wired place is a row");
        assert_eq!(projects(&c).name_of_computer("default", &rows), "default");
        assert_eq!(projects(&c).name_of_computer("pl_unknown", &rows), "pl_unknown");
        let here = rows.iter().find(|r| r.id == HERE_PLACE_ID).expect("here");
        assert_eq!(projects(&c).name_of_computer(HERE_PLACE_ID, &rows), here.name);
    });
}

#[test]
fn projects_landing_place_is_none_for_this_computer_and_the_wired_provider() {
    drive(plain(), |c, _| async move {
        assert_eq!(projects(&c).landing_place(HERE_PLACE_ID).await.expect("a place"), None);
        assert_eq!(projects(&c).landing_place("default").await.expect("a place"), None);
    });
}

#[test]
fn projects_git_reads_here_name_the_top_the_remote_and_nothing_for_a_plain_folder() {
    let root = tempfile::tempdir().expect("a folder");
    let repo = repo_in(root.path(), "api");
    git(&repo, &["remote", "add", "origin", "https://github.com/acme/api.git"]);
    std::fs::create_dir_all(repo.join("packages/host")).expect("a folder");
    let plain_dir = root.path().join("plain");
    std::fs::create_dir_all(&plain_dir).expect("a folder");
    let repo_real = std::fs::canonicalize(&repo).expect("a path").to_string_lossy().to_string();
    let (repo_s, sub, plain_s) = (
        repo.to_string_lossy().to_string(),
        repo.join("packages/host").to_string_lossy().to_string(),
        plain_dir.to_string_lossy().to_string(),
    );
    drive(RuntimeOptions { local: Some(local_in(root.path())), ..plain() }, |c, _| async move {
        assert_eq!(projects(&c).git_top_of(&sub).await.expect("a read"), Some(repo_real.clone()));
        assert_eq!(projects(&c).git_top_of(&plain_s).await.expect("a read"), None);
        assert_eq!(projects(&c).remote_here(&repo_s).await.expect("a remote"), "https://github.com/acme/api.git");
        assert_eq!(projects(&c).remote_here(&plain_s).await.expect("a remote"), "");
    });
}

#[test]
fn projects_clone_here_refuses_a_url_no_clone_should_run_before_running_anything() {
    let root = tempfile::tempdir().expect("a folder");
    let into = root.path().join("etc").to_string_lossy().to_string();
    let source: ProjectSource = serde_json::from_value(json!({ "kind": "folder", "path": "/tmp/etc" })).expect("a source");
    drive(RuntimeOptions { local: Some(local_in(root.path())), ..plain() }, |c, _| async move {
        let e = projects(&c).clone_here("file:///etc", &source, &into).await.expect_err("refused");
        assert_eq!(e.message, wsp_words::projects::clone_url_refusal("file:///etc").expect("a refusal"));
        assert!(!Path::new(&into).exists());
    });
}

#[test]
fn projects_holds_thread_and_the_local_copier_read_the_folders_and_the_options() {
    // `holdsThread` is the folders area's (packet 8); a runtime holding nothing holds no thread.
    let root = tempfile::tempdir().expect("a folder");
    drive(RuntimeOptions { local: Some(local_in(root.path())), ..plain() }, |c, _| async move {
        assert!(!projects(&c).holds_thread("w_none"));
        assert!(projects(&c).copier_worktree_forget("/tmp/a", "/tmp/h", "pr_1").await.is_none(), "no copier is wired");
    });
}

#[test]
fn projects_thread_defaults_and_agent_lists_name_the_first_agent() {
    // `resolveThreadDefaults` over the lists one computer's agents answer; the lists come from the agents area (packet
    // 11) and the held preferences from packet 16, so this waits on both.
    drive(plain(), |c, _| async move {
        let lists = projects(&c).agent_lists(HERE_PLACE_ID, None).await.expect("lists");
        let first = projects(&c).first_agent();
        let defaults = projects(&c).thread_defaults("pr_1", &first, &lists);
        assert_eq!(defaults.agent.value, first);
    });
}

#[test]
fn projects_forking_at_and_landing_backend_read_the_builders() {
    // `forkingAt(undefined)` is the wired backend; a place no row names is refused with the computer's absent
    // sentence (packet 14 holds both).
    drive(plain(), |c, _| async move {
        assert!(projects(&c).forking_at(None).await.is_ok());
        let Err(e) = projects(&c).landing_backend("pl_gone").await else { panic!("refused") };
        assert!(e.message.contains("pl_gone") || !e.message.is_empty());
    });
}

#[test]
fn projects_delete_workspace_remember_project_and_icons_forward_to_their_areas() {
    // `ctx.workspaces.delete` (packet 8), `ctx.rememberProject` (rules, merged), `projectIcons` (needs the state
    // file and the preferences, packet 16).
    drive(plain(), |c, _| async move {
        let p = project("pr_1", "api", "/tmp/api");
        projects(&c).remember_project(p).await.expect("remembered");
        let e = projects(&c).delete_workspace("w_gone", None).await.expect_err("no such workspace");
        assert!(!e.message.is_empty());
        assert_eq!(projects(&c).read_icons(vec![]).await.expect("icons"), IndexMap::new());
    });
}

#[test]
fn projects_project_source_resolves_a_git_url_a_github_repo_and_a_folders_own_remote_and_default_branch() {
    // project-sources.ts:54-100: a git url and a host repo answer `main`; a folder here answers its origin and the
    // branch `origin/HEAD` names, which `ResolveAsk::folder_remote` (a plain remote) cannot carry.
    let root = tempfile::tempdir().expect("a folder");
    let repo = repo_in(root.path(), "api");
    git(&repo, &["remote", "add", "origin", "https://github.com/acme/api.git"]);
    git(&repo, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "--allow-empty", "-qm", "one"]);
    git(&repo, &["update-ref", "refs/remotes/origin/trunk", "HEAD"]);
    git(&repo, &["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/trunk"]);
    let at = repo.to_string_lossy().to_string();
    drive(RuntimeOptions { local: Some(local_in(root.path())), ..plain() }, |c, _| async move {
        let asked = || wsp_account::projects::projects::add::ResolveAsk {
            seeding: false,
            seed_plan: Rc::new(|_| Box::pin(async { Err(HostError::new("no plan in this test")) })),
            folder_remote: Rc::new(|_| Box::pin(async { Ok(String::new()) })),
        };
        let git_source: ProjectSource = serde_json::from_value(json!({ "kind": "git", "url": REPO })).expect("a source");
        let resolved = projects(&c).project_source(&git_source).resolve(&git_source, asked()).await.expect("resolved");
        assert_eq!((resolved.name.as_str(), resolved.remote.as_str(), resolved.default_branch.as_str()), ("frontend", REPO, "main"));
        assert!(resolved.seed.is_none() && resolved.seed_memory_key.is_none());
        let host_source: ProjectSource = serde_json::from_value(json!({ "kind": "github", "repo": "acme/lab" })).expect("a source");
        let resolved = projects(&c).project_source(&host_source).resolve(&host_source, asked()).await.expect("resolved");
        assert_eq!(
            (resolved.name.as_str(), resolved.remote.as_str(), resolved.default_branch.as_str()),
            ("lab", "https://github.com/acme/lab.git", "main")
        );
        let folder: ProjectSource = serde_json::from_value(json!({ "kind": "folder", "path": at })).expect("a source");
        let resolved = projects(&c).project_source(&folder).resolve(&folder, asked()).await.expect("resolved");
        assert_eq!(
            (resolved.name.as_str(), resolved.remote.as_str(), resolved.default_branch.as_str()),
            ("api", "https://github.com/acme/api.git", "trunk")
        );
    });
}

#[test]
fn projects_thread_defaults_name_the_first_agent_off_the_catalog_with_no_preference_held() {
    // resolveThreadDefaults({firstAgent, catalogOf, runs, prefs}): with no default agent and no project override the
    // agent is the first one an adapter runs, from the catalog.
    let first = wsp_catalog::catalog::catalog_agents().first().expect("an agent").id.clone();
    let opts = RuntimeOptions { adapters: IndexMap::from([(first.clone(), some_adapter())]), ..plain() };
    drive(opts, |c, _| async move {
        let lists: Vec<_> = wsp_core::harness_catalog::harness_catalogs().into_iter().filter(|l| l.harness == first).collect();
        let defaults = projects(&c).thread_defaults("pr_1", &first, &lists);
        assert_eq!(defaults.agent.value, first);
        assert_eq!(defaults.agent.from, wsp_wire::types::thread_defaults::DefaultFrom::Catalog);
    });
}

// ----------------------------------------------------------------------------------------------------------------
// MoveRoads (projects.ts:23-106, 482-574)

#[test]
fn moves_temp_dir_is_the_os_temp_folder_and_guest_agent_homes_are_the_engines() {
    drive(plain(), |c, _| async move {
        assert_eq!(moves(&c).temp_dir(), std::env::temp_dir().to_string_lossy());
        assert_eq!(moves(&c).guest_agent_homes(), wsp_engine::project_state::guest_agent_homes());
        assert!(moves(&c).guest_agent_homes().contains_key("claude"));
    });
}

#[test]
fn moves_machine_exec_and_run_run_on_the_entrys_machine() {
    let root = tempfile::tempdir().expect("a folder");
    let at = root.path().to_path_buf();
    drive(plain(), |c, _| async move {
        let entry = local_entry(&at).await;
        let said = moves(&c).machine_exec(&entry, "printf hi", 5_000).await.expect("ran");
        assert_eq!((said.exit_code.0 .0 as i64, said.stdout.as_str()), (0, "hi"));
        let failed = moves(&c).machine_exec(&entry, "exit 3", 5_000).await.expect("ran");
        assert_eq!(failed.exit_code.0 .0 as i64, 3);
        let ran = moves(&c).machine_run(&entry, "printf there", 5_000).await.expect("ran");
        assert_eq!((ran.exit_code.0 .0 as i64, ran.stdout.as_str()), (0, "there"));
    });
}

#[test]
fn moves_agents_on_machine_names_no_agent_for_no_ids_and_the_ones_found_for_the_rest() {
    let root = tempfile::tempdir().expect("a folder");
    let at = root.path().to_path_buf();
    drive(plain(), |c, _| async move {
        let entry = local_entry(&at).await;
        assert_eq!(moves(&c).agents_on_machine(&entry, &[]).await.expect("a read"), Vec::<String>::new());
        let found = moves(&c).agents_on_machine(&entry, &["claude".to_string()]).await.expect("a read");
        let machine = entry.borrow().machine.clone();
        let same = wsp_engine::project_state::agents_on_machine(&*machine, &["claude".to_string()]).await.expect("a read");
        assert_eq!(found.len(), same.len());
    });
}

#[test]
fn moves_scratch_is_the_kinds_scratch_and_provider_row_reads_the_computer_rows() {
    // The kind module's `scratch` (packet 2) and `computerRows` (packet 5): a record on the wired provider is a
    // provider row, one with no provider row is not.
    drive(plain(), |c, _| async move {
        let machine = backend().get("m_1").await.expect("a machine");
        let entry = entry_over(record(json!({ "kind": "cloud", "provider": "default" })), machine);
        assert!(moves(&c).scratch(&entry).is_ok());
        let here = entry_over(record(json!({ "kind": "local" })), backend().get("m_1").await.expect("a machine"));
        assert!(!moves(&c).provider_row(&here).await.expect("a read"));
    });
}

#[test]
fn moves_project_of_names_the_folder_the_import_landed_with_its_bytes() {
    // `ctx.projectOf(dest, bytes)` (kinds, packet 2): a record of the folder as the workspace's project.
    drive(plain(), |c, _| async move {
        let landed = moves(&c).project_of("/root/lab", 4096).expect("a project");
        assert_eq!(landed["dest"], json!("/root/lab"));
    });
}

// ----------------------------------------------------------------------------------------------------------------
// LandingRoad (project-landing.ts:71-104, 511-617): the account's road over wsp-core's

#[test]
fn landing_road_kind_is_each_roads_own() {
    drive(plain(), |c, _| async move {
        for kind in [LandingKind::Mac, LandingKind::Box, LandingKind::Provider] {
            assert_eq!(road(&c, kind).kind(), kind);
        }
    });
}

#[test]
fn landing_road_takes_files_only_on_a_box_and_lands_at_add_where_the_road_says() {
    drive(plain(), |c, _| async move {
        assert!(road(&c, LandingKind::Box).takes_files());
        assert!(!road(&c, LandingKind::Mac).takes_files());
        assert!(!road(&c, LandingKind::Provider).takes_files());
        for seeding in [false, true] {
            assert!(road(&c, LandingKind::Box).lands_at_add(seeding));
            assert!(!road(&c, LandingKind::Mac).lands_at_add(seeding));
            assert!(!road(&c, LandingKind::Provider).lands_at_add(seeding));
        }
    });
}

#[test]
fn landing_road_path_names_the_folder_each_road_keeps() {
    let source: ProjectSource =
        serde_json::from_value(json!({ "kind": "git", "url": "https://github.com/spoo-me/frontend.git" })).expect("a source");
    drive(plain(), |c, _| async move {
        let deps = road_deps("default", "/mac/.claude");
        // A provider's checkout is the first life's own folder under /root, as the TS `projectPathOn` names it.
        assert_eq!(road(&c, LandingKind::Provider).path("spoo-landing", &source, &deps), "/root/spoo-landing");
        // A folder source is its own path on this computer.
        let folder: ProjectSource = serde_json::from_value(json!({ "kind": "folder", "path": "/Users/z/lab" })).expect("a source");
        assert_eq!(road(&c, LandingKind::Mac).path("lab", &folder, &deps), "/Users/z/lab");
    });
}

#[test]
fn landing_road_memory_dir_keys_the_memory_to_the_road() {
    drive(plain(), |c, _| async move {
        let p = project("pr_1", "api", "/Users/z/api");
        let deps = road_deps("zingzys-mac", "/Users/z/.claude");
        assert_eq!(road(&c, LandingKind::Mac).memory_dir(&p, "-Users-z-api", &deps), "/Users/z/.claude/projects/-Users-z-api/memory");
        assert_eq!(
            road(&c, LandingKind::Provider).memory_dir(&p, "-root-api", &deps),
            wsp_core::project_landing::guest_memory_dir("-root-api")
        );
        let home = JoinedComputer { machine: backend().get("m_1").await.expect("a machine"), home: "/home/me".into() };
        let boxed = LandingDeps { computer: Some(home), ..road_deps("a-box", "") };
        assert_eq!(road(&c, LandingKind::Box).memory_dir(&p, "-home-me-api", &boxed), "/home/me/.claude-cfg/projects/-home-me-api/memory");
    });
}

#[test]
fn landing_road_remove_says_each_roads_own_line() {
    drive(plain(), |c, _| async move {
        let p = project("pr_1", "api", "/Users/z/api");
        let deps = road_deps("a-box", "");
        assert_eq!(
            road(&c, LandingKind::Mac).remove(&p, &deps).await.expect("a line"),
            wsp_words::projects::project_removed_here_line("api")
        );
        assert_eq!(
            road(&c, LandingKind::Provider).remove(&p, &deps).await.expect("a line"),
            wsp_words::projects::project_removed_at_provider_line("api", "a-box")
        );
        assert_eq!(
            road(&c, LandingKind::Box).remove(&p, &deps).await.expect("a line"),
            wsp_words::projects::project_left_on_computer_line("api", "a-box", "/Users/z/api")
        );
    });
}

#[test]
fn landing_road_has_only_the_members_each_road_has() {
    drive(plain(), |c, _| async move {
        let p = project("pr_1", "api", "/Users/z/api");
        let deps = road_deps("a-box", "");
        assert!(road(&c, LandingKind::Mac).folder_stands(&p, &deps).await.is_none());
        assert!(road(&c, LandingKind::Provider).folder_stands(&p, &deps).await.is_none());
        assert!(road(&c, LandingKind::Mac).seed_into(&p, vec![], &deps).await.is_none());
        assert!(road(&c, LandingKind::Mac).unsaved(&p, &deps).await.is_none());
        assert!(road(&c, LandingKind::Provider).unsaved(&p, &deps).await.is_none());
    });
}

#[test]
fn landing_road_branch_is_the_records_on_a_provider_and_the_folders_on_this_computer() {
    let root = tempfile::tempdir().expect("a folder");
    let repo = repo_in(root.path(), "api");
    let repo_s = repo.to_string_lossy().to_string();
    drive(RuntimeOptions { local: Some(local_in(root.path())), ..plain() }, |c, _| async move {
        let mut p = project("pr_1", "api", &repo_s);
        let deps = road_deps("zingzys-mac", "");
        // A provider names the record's branch and asks no machine: the base, else the default.
        assert_eq!(
            road(&c, LandingKind::Provider).branch(&p, &deps).await.expect("a branch"),
            ProjectBranch { branch: Some("main".into()), folder: false }
        );
        p.base = Some("release".into());
        assert_eq!(
            road(&c, LandingKind::Provider).branch(&p, &deps).await.expect("a branch"),
            ProjectBranch { branch: Some("release".into()), folder: false }
        );
        // The folder here names the branch it has checked out: `main` after `git init -b main`.
        assert_eq!(
            road(&c, LandingKind::Mac).branch(&p, &deps).await.expect("a branch"),
            ProjectBranch { branch: Some("main".into()), folder: true }
        );
    });
}

#[test]
fn landing_road_land_on_a_provider_and_on_this_computer_leaves_nothing() {
    drive(plain(), |c, _| async move {
        let p = project("pr_1", "api", "/Users/z/api");
        let deps = road_deps("default", "");
        for kind in [LandingKind::Provider, LandingKind::Mac] {
            let ask = wsp_account::projects::projects::add::LandAsk { project: p.clone(), seed: None, report: Rc::new(|_, _| {}) };
            let landed = road(&c, kind).land(ask, &deps).await.expect("landed");
            assert!(landed.fields.is_empty() && landed.notice.is_none());
        }
    });
}

#[test]
fn landing_road_a_box_with_a_login_reads_whether_a_folder_stands_and_lands_a_folder_source_in_place() {
    // project-landing.ts:523-527 (`test -e`, exit 0 or 1) and 545-546 (a folder source with no seed lands as it
    // stands, its git top its own path).
    let root = tempfile::tempdir().expect("a folder");
    let stands = root.path().join("api");
    std::fs::create_dir_all(&stands).expect("a folder");
    let (stands_s, missing_s, home) = (
        stands.to_string_lossy().to_string(),
        root.path().join("gone").to_string_lossy().to_string(),
        root.path().to_string_lossy().to_string(),
    );
    let at = root.path().to_path_buf();
    drive(plain(), |c, _| async move {
        let machine = LocalBackend::new(LocalBackendOptions { root: at, env: None }).get(LOCAL_MACHINE_ID).await.expect("a machine");
        let deps = LandingDeps { computer: Some(JoinedComputer { machine, home }), ..road_deps("a-box", "") };
        let here = project("pr_1", "api", &stands_s);
        assert!(road(&c, LandingKind::Box).folder_stands(&here, &deps).await.expect("a box has it").expect("a read"));
        let gone = project("pr_2", "gone", &missing_s);
        assert!(!road(&c, LandingKind::Box).folder_stands(&gone, &deps).await.expect("a box has it").expect("a read"));
        let ask = wsp_account::projects::projects::add::LandAsk { project: here, seed: None, report: Rc::new(|_, _| {}) };
        let landed = road(&c, LandingKind::Box).land(ask, &deps).await.expect("landed");
        assert_eq!(Value::Object(landed.fields), json!({ "git": { "top": stands_s } }));
        assert!(landed.notice.is_none());
    });
}

#[test]
fn landing_road_a_box_with_no_login_says_so_for_the_calls_that_need_one() {
    drive(plain(), |c, _| async move {
        let p = project("pr_1", "api", "/home/me/api");
        let deps = road_deps("a-box", "");
        let e = road(&c, LandingKind::Box).folder_stands(&p, &deps).await.expect("a box has the member").expect_err("no login");
        assert_eq!(e.message, "a-box answered no login to run the add as, so nothing can be cloned there");
    });
}

// ----------------------------------------------------------------------------------------------------------------
// ProjectIconsCtx (images.ts:137-190)

#[test]
fn icons_has_state_path_project_held_and_ids_read_the_core() {
    drive(plain(), |c, _| async move {
        assert!(!icons(&c).has_state_path());
        assert!(!icons(&c).project_held("pr_1"));
        assert_ne!(icons(&c).ids().uuid(), icons(&c).ids().uuid());
    });
    let opts = RuntimeOptions { state_path: Some("/tmp/wsp-p10/state.json".into()), ..plain() };
    drive(opts, |c, _| async move {
        assert!(icons(&c).has_state_path());
        projects(&c).remember_project(project("pr_1", "api", "/tmp/api")).await.expect("remembered");
        assert!(icons(&c).project_held("pr_1"));
        assert!(!icons(&c).project_held("pr_2"));
    });
}

#[test]
fn icons_recipes_are_the_options_shelf_and_a_recipes_changed_event_reaches_the_listener() {
    drive(plain(), |c, _| async move {
        assert!(icons(&c).recipes().is_none());
    });
    drive(plain(), |c, _| async move {
        let heard = Rc::new(std::cell::Cell::new(0));
        let seen = heard.clone();
        icons(&c).on_recipes_changed(Rc::new(move || seen.set(seen.get() + 1)));
        let event = serde_json::from_value(json!({ "type": "recipes.changed", "slug": "builders" })).expect("an event");
        wsp_machines::machines::create::CreateRuntime::emit(&*c, event).expect("emitted");
        assert_eq!(heard.get(), 1);
    });
}

#[test]
fn icons_ready_waits_on_the_boot_and_preferences_read_the_held_preferences() {
    // `ctx.ready()` is boot's (packet 5, which waits on 4); `ctx.preferences.get()` and `changePreferences` are the
    // preferences area's (packet 16).
    drive(plain(), |c, _| async move {
        icons(&c).ready().await.expect("booted");
        let prefs = icons(&c).preferences().await.expect("preferences");
        let changed = icons(&c).change_preferences(Box::new(|held| held)).await.expect("changed");
        assert_eq!(serde_json::to_value(&prefs).expect("json"), serde_json::to_value(&changed).expect("json"));
    });
}

// ----------------------------------------------------------------------------------------------------------------
// ReviewCtx: the core's reads and the one piece of logic, probe (workspaces.ts:1055-1247)

async fn answer(mut s: TcpStream) {
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    while !got.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut buf).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        got.extend_from_slice(&buf[..n]);
    }
    let head = String::from_utf8_lossy(&got).to_string();
    let (status, extra, body) = if head.starts_with("GET /r ") {
        ("302 Found", "location: /final\r\n", format!("moved {}", "x".repeat(5000)))
    } else if head.starts_with("GET /big ") {
        ("200 OK", "", "y".repeat(5000))
    } else if head.starts_with("GET /401 ") {
        ("401 Unauthorized", "", "no".to_string())
    } else {
        ("200 OK", "", "final".to_string())
    };
    let wire = format!("HTTP/1.1 {status}\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    let _ = s.write_all(wire.as_bytes()).await;
}

async fn serve() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.expect("a listener");
    let port = l.local_addr().expect("an address").port();
    tokio::spawn(async move {
        loop {
            let Ok((s, _)) = l.accept().await else { return };
            tokio::spawn(answer(s));
        }
    });
    port
}

#[test]
fn review_probe_follows_no_redirect_and_cuts_the_body_at_the_cap() {
    // node 22 for the same server: a 302 answers 302 with its own body, a 5000-byte body is cut at 2048.
    drive(plain(), |c, _| async move {
        let port = serve().await;
        let redirected = review(&c).probe(&format!("http://127.0.0.1:{port}/r")).await.expect("a probe");
        assert_eq!(
            serde_json::to_value(&redirected).expect("json"),
            json!({ "status": 302, "body": format!("moved {}", "x".repeat(PORT_PROBE_BODY_CAP - 6)) })
        );
        let big = review(&c).probe(&format!("http://127.0.0.1:{port}/big")).await.expect("a probe");
        assert_eq!(big.body.len(), PORT_PROBE_BODY_CAP);
        let refused = review(&c).probe(&format!("http://127.0.0.1:{port}/401")).await.expect("a probe");
        assert_eq!(serde_json::to_value(&refused).expect("json"), json!({ "status": 401, "body": "no" }));
    });
}

#[test]
fn review_probe_answers_fetchs_failure_for_a_port_nothing_listens_on() {
    drive(plain(), |c, _| async move {
        let l = TcpListener::bind("127.0.0.1:0").await.expect("a listener");
        let port = l.local_addr().expect("an address").port();
        drop(l);
        let e = review(&c).probe(&format!("http://127.0.0.1:{port}/")).await.expect_err("nothing listens");
        assert!(!e.message.is_empty());
    });
}

#[test]
fn review_machine_word_and_metrics_road_are_the_kinds_words() {
    let root = tempfile::tempdir().expect("a folder");
    let at = root.path().to_path_buf();
    drive(plain(), |c, _| async move {
        let local = local_entry(&at).await;
        local.borrow().record.borrow_mut().kind = serde_json::from_value(json!("local")).expect("a kind");
        let cloud = entry_over(record(json!({ "kind": "cloud" })), backend().get("m_1").await.expect("a machine"));
        assert_eq!(
            review(&c).machine_word(&local),
            wsp_words::workspace_state::machine_word(&serde_json::from_value(json!("local")).expect("a kind"))
        );
        assert_eq!(
            review(&c).machine_word(&cloud),
            wsp_words::workspace_state::machine_word(&serde_json::from_value(json!("cloud")).expect("a kind"))
        );
        // The cloud's load rides its daemon link; this computer's is read here, from the host.
        assert!(!review(&c).metrics_on_host(&cloud));
        assert!(review(&c).metrics_on_host(&local));
    });
}

#[test]
fn review_label_of_names_the_workspace_or_the_thread() {
    drive(plain(), |c, _| async move {
        let machine = backend().get("m_1").await.expect("a machine");
        let entry = entry_over(record(json!({ "name": "pricing page" })), machine);
        assert_eq!(review(&c).label_of(&entry, None), "pricing page");
        assert_eq!(
            review(&c).label_of(&entry, Some("t_abcdef012345")),
            wsp_core::types::internal::label_of("pricing page", Some("t_abcdef012345"))
        );
    });
}

#[test]
fn review_drop_page_cache_deletes_the_pages_key_from_the_github_cache() {
    let cache = memory_github_cache();
    let held = cache.clone();
    drive(RuntimeOptions { github_cache: Some(cache), ..plain() }, |c, _| async move {
        let key = wsp_account::account::pull_requests::PullRequestsArea::page_key("https://github.com/acme/api", 7);
        held.set(&key, wsp_core::github_cache::GitHubCacheEntry { body: json!(null), tag: Some("e".into()), fetched_at: 0 });
        assert!(held.get(&key).is_some());
        review(&c).drop_page_cache("https://github.com/acme/api", 7);
        assert!(held.get(&key).is_none());
    });
}

#[test]
fn review_sys_samples_is_none_where_no_local_reads_the_computer() {
    drive(plain(), |c, _| async move {
        assert!(review(&c).sys_samples(Rc::new(|_| {})).is_none());
    });
}

#[test]
fn review_sys_samples_hands_back_a_detach_that_takes_the_watch_away_whether_or_not_it_is_up_yet() {
    // `local?.sysSamples(fn)` answers its detach later; the caller holds one at once (workspaces.ts:1176-1177).
    use futures::FutureExt as _;
    let root = tempfile::tempdir().expect("a folder");
    let (attached, detached) = (Rc::new(std::cell::Cell::new(0)), Rc::new(std::cell::Cell::new(0)));
    let (a, d) = (attached.clone(), detached.clone());
    let mut local = local_in(root.path());
    local.sys_samples = Some(Rc::new(move |_listener| {
        a.set(a.get() + 1);
        let d = d.clone();
        async move {
            let detach: wsp_core::types::wiring::Detach = Box::new(move || d.set(d.get() + 1));
            Ok(detach)
        }
        .boxed_local()
    }));
    drive(RuntimeOptions { local: Some(local), ..plain() }, |c, _| async move {
        let detach = review(&c).sys_samples(Rc::new(|_| {})).expect("a watch");
        assert_eq!(attached.get(), 1);
        tokio::task::yield_now().await;
        assert_eq!(detached.get(), 0);
        detach();
        assert_eq!(detached.get(), 1);
        let early = review(&c).sys_samples(Rc::new(|_| {})).expect("a watch");
        early();
        tokio::task::yield_now().await;
        assert_eq!(detached.get(), 2);
    });
}

#[test]
fn review_ws_port_reach_is_the_engine_workspaces_and_says_its_failure() {
    // `entry.ws.portReach(port)`: a fake backend with no preview road refuses it rather than answering a url.
    drive(plain(), |c, _| async move {
        let entry = entry_over(record(json!({ "kind": "cloud" })), backend().get("m_1").await.expect("a machine"));
        let e = review(&c).ws_port_reach(&entry, 3000).await.expect_err("no preview road");
        assert!(!e.message.is_empty());
    });
}

#[test]
fn review_daemon_token_and_port_roots_do_nothing_for_a_workspace_the_host_does_not_hold() {
    drive(plain(), |c, _| async move {
        review(&c).forget_daemon_token("m_none");
        review(&c).port_roots_moved("w_none");
        assert!(review(&c).port_pids("w_none").borrow().is_empty());
    });
}

#[test]
fn review_refusal_for_and_reaches_row_let_a_caller_with_no_scope_through() {
    // `refusalFor(record, undefined)` is none (rules, merged); `reachesRow` is boot's (packet 5).
    drive(plain(), |c, _| async move {
        assert_eq!(review(&c).refusal_for(None, None).expect("a read"), None);
        let held = record(json!({}));
        assert_eq!(review(&c).refusal_for(Some(&held), None).expect("a read"), None);
        assert!(review(&c).reaches_row("t_1", "w_1", None));
    });
}

#[test]
fn review_kind_reads_answer_the_kinds_port_reach_and_daemon_road() {
    // The kind module's `portReach` and `daemonRoad` (packet 2) and `sharedDaemon`, `servedByItsComputer` (packet 3):
    // a fake backend's cloud workspace has no kind-own port route, so the engine workspace answers it.
    drive(plain(), |c, _| async move {
        let entry = entry_over(record(json!({ "kind": "cloud" })), backend().get("m_1").await.expect("a machine"));
        assert!(!review(&c).shared_daemon(&entry));
        assert!(review(&c).served_by_its_computer(&entry).is_none());
        assert!(review(&c).kind_port_reach(&entry, 3000, false).await.is_none());
    });
}

#[test]
fn review_start_from_forwards_read_the_projects_held_and_the_read_only_modes() {
    // `projectByRepo` reads `projectsHeld` (packet 8's `StartFromCtx`): with none held it is refused with the
    // repository's own sentence; `readOnlyOf` and `reviewers` read the harness table (packet 8).
    drive(plain(), |c, _| async move {
        let e = review(&c).project_by_repo("acme/api", None).expect_err("no project");
        assert_eq!(e.message, wsp_words::start::start_words::no_project_for_repo("acme/api"));
        assert_eq!(review(&c).reviewers().first().map(String::as_str), Some("codex"));
        assert!(review(&c).read_only_of("nothing").is_none());
    });
}

// ----------------------------------------------------------------------------------------------------------------
// Whole-runtime ports of `projects.test.ts` and `project-icons.test.ts`: every op awaits `ready()` (packets 4 and 5)
// and the add reads the builders (14); each test says what else it waits for.

use wsp_core::types::api::{ProjectAddAsk, Runtime};
use wsp_wire::wire::events_union::EventUnion;

const REPO: &str = "https://github.com/spoo-me/frontend.git";

fn with_local(root: &Path) -> RuntimeOptions {
    RuntimeOptions { local: Some(local_in(root)), adapters: IndexMap::from([("claude".to_string(), some_adapter())]), ..plain() }
}

fn ask(source: &str) -> ProjectAddAsk {
    ProjectAddAsk { source: source.to_string(), on: None, name: None, base: None, into: None, seed: None }
}

fn tap(runtime: &AssembledRuntime, kind: &str) -> Rc<RefCell<Vec<Value>>> {
    let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
    let held = seen.clone();
    let _ = runtime.events().on(
        kind,
        Rc::new(move |e: &EventUnion| {
            held.borrow_mut().push(serde_json::to_value(e).expect("an event"));
            Ok(())
        }),
    );
    seen
}

#[test]
fn port_a_folder_that_is_a_git_repo_is_a_project_on_this_computer_in_place_and_goes_out_as_an_event() {
    // projects.test.ts:75. Waits on 4, 5 (ready), 8 (the folders' reads) and 14 (`forkingAt`).
    let root = tempfile::tempdir().expect("a folder");
    let repo = repo_in(root.path(), "frontend");
    let at = std::fs::canonicalize(&repo).expect("a path").to_string_lossy().to_string();
    drive(with_local(root.path()), |_, rt| async move {
        let added = tap(&rt, "project.added");
        let project = rt.projects().add(ask(&at), None).await.expect("an add").project;
        let view = serde_json::to_value(&project).expect("json");
        assert_eq!(view["computer"], json!(HERE_PLACE_ID));
        assert_eq!(view["path"], json!(at));
        assert_eq!(view["source"], json!({ "kind": "folder", "path": at }));
        assert!(view["id"].as_str().expect("an id").starts_with("pr_"));
        assert_eq!(added.borrow().len(), 1);
        assert_eq!(added.borrow()[0]["project"], view);
        assert_eq!(rt.projects().list(None).await.expect("a list").len(), 1);
    });
}

#[test]
fn port_a_folder_under_a_repo_is_a_project_of_that_repo_its_top_kept_beside_its_path() {
    // projects.test.ts:96. Waits on 4, 5, 8, 14.
    let root = tempfile::tempdir().expect("a folder");
    let repo = repo_in(root.path(), "frontend");
    std::fs::create_dir_all(repo.join("packages/host")).expect("a folder");
    let sub = std::fs::canonicalize(repo.join("packages/host")).expect("a path").to_string_lossy().to_string();
    let top = std::fs::canonicalize(&repo).expect("a path").to_string_lossy().to_string();
    drive(with_local(root.path()), |_, rt| async move {
        let project = rt.projects().add(ask(&sub), None).await.expect("an add").project;
        let view = serde_json::to_value(&project).expect("json");
        assert_eq!(view["path"], json!(sub));
        assert_eq!(view["git"]["top"], json!(top));
    });
}

#[test]
fn port_a_folder_that_is_no_git_repo_is_a_project_with_no_branches() {
    // projects.test.ts:87. Waits on 4, 5, 8, 14.
    let root = tempfile::tempdir().expect("a folder");
    let plain_dir = std::fs::canonicalize(root.path()).expect("a path").join("plain");
    std::fs::create_dir_all(&plain_dir).expect("a folder");
    let at = plain_dir.to_string_lossy().to_string();
    drive(with_local(root.path()), |_, rt| async move {
        let project = rt.projects().add(ask(&at), None).await.expect("an add").project;
        let view = serde_json::to_value(&project).expect("json");
        assert_eq!(view["path"], json!(at));
        assert!(view.get("git").is_none());
    });
}

#[test]
fn port_a_repos_url_with_no_computer_names_the_ones_that_clone_and_records_nothing() {
    // projects.test.ts:105. Waits on 4, 5, 14.
    let root = tempfile::tempdir().expect("a folder");
    drive(with_local(root.path()), |_, rt| async move {
        let e = rt.projects().add(ask(REPO), None).await.expect_err("refused");
        assert_eq!(e.message, wsp_words::projects::no_computer_for_source_line(REPO, &["default"]));
        assert!(rt.projects().list(None).await.expect("a list").is_empty());
    });
}

#[test]
fn port_a_repos_url_on_a_computer_that_clones_records_the_clones_own_path_and_a_second_add_is_refused() {
    // projects.test.ts:111. Waits on 4, 5, 14.
    let root = tempfile::tempdir().expect("a folder");
    drive(with_local(root.path()), |_, rt| async move {
        let mut named = ask(REPO);
        named.on = Some("default".into());
        named.name = Some("spoo-landing".into());
        let project = rt.projects().add(named, None).await.expect("an add").project;
        let view = serde_json::to_value(&project).expect("json");
        assert_eq!(view["computer"], json!("default"));
        assert_eq!(view["path"], json!("/root/spoo-landing"));
        assert_eq!(view["source"], json!({ "kind": "git", "url": REPO }));
        let mut again = ask(REPO);
        again.on = Some("default".into());
        let e = rt.projects().add(again, None).await.expect_err("refused");
        assert_eq!(e.message, wsp_words::projects::same_source_refusal("spoo-landing", "default"));
    });
}

#[test]
fn port_a_project_is_named_by_id_or_by_name_and_a_word_that_names_none_is_refused_with_the_ones_there_are() {
    // projects.test.ts:263. Waits on 4, 5, 14.
    let root = tempfile::tempdir().expect("a folder");
    drive(with_local(root.path()), |_, rt| async move {
        let mut named = ask(REPO);
        named.on = Some("default".into());
        named.name = Some("spoo-landing".into());
        let project = rt.projects().add(named, None).await.expect("an add").project;
        assert_eq!(rt.projects().resolve(&project.id, None).await.expect("by id"), project);
        assert_eq!(rt.projects().resolve("spoo-landing", None).await.expect("by name"), project);
        let e = rt.projects().resolve("nothing", None).await.expect_err("refused");
        assert_eq!(e.message, "no project \"nothing\"; this host holds spoo-landing");
    });
}

#[test]
fn port_the_branch_a_new_thread_starts_on_is_the_projects_base_else_the_remotes_default_asking_no_machine() {
    // projects.test.ts:273. Waits on 4, 5, 14.
    let root = tempfile::tempdir().expect("a folder");
    drive(with_local(root.path()), |_, rt| async move {
        let mut plain_ask = ask(REPO);
        plain_ask.on = Some("default".into());
        plain_ask.name = Some("plain".into());
        let plain_project = rt.projects().add(plain_ask, None).await.expect("an add").project;
        let mut based = ask("https://github.com/acme/lab");
        based.on = Some("default".into());
        based.name = Some("based".into());
        based.base = Some("release".into());
        rt.projects().add(based, None).await.expect("an add");
        assert_eq!(
            rt.projects().branch(&plain_project.id, None).await.expect("a branch"),
            ProjectBranch { branch: Some(plain_project.default_branch.clone()), folder: false }
        );
        assert_eq!(
            rt.projects().branch("based", None).await.expect("a branch"),
            ProjectBranch { branch: Some("release".into()), folder: false }
        );
    });
}

#[test]
fn port_a_project_is_dropped_once_no_workspace_stands_on_it_and_the_drop_goes_out_as_an_event() {
    // projects.test.ts:250, without the workspace: a project nothing stands on is dropped. Waits on 4, 5, 8, 14, 16.
    let root = tempfile::tempdir().expect("a folder");
    let plain_dir = std::fs::canonicalize(root.path()).expect("a path").join("lab");
    std::fs::create_dir_all(&plain_dir).expect("a folder");
    let at = plain_dir.to_string_lossy().to_string();
    drive(with_local(root.path()), |_, rt| async move {
        let removed = tap(&rt, "project.removed");
        let project = rt.projects().add(ask(&at), None).await.expect("an add").project;
        rt.projects().remove(&project.id, None, None, None).await.expect("a removal");
        assert!(rt.projects().list(None).await.expect("a list").is_empty());
        let events = removed.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["projectId"], json!(project.id));
    });
}

#[test]
fn port_an_image_for_a_project_that_does_not_stand_is_refused_naming_it() {
    // project-icons.test.ts:171 ("a project that does not stand is refused"). Waits on 4, 5, 16.
    let root = tempfile::tempdir().expect("a folder");
    let state = root.path().join("state");
    std::fs::create_dir_all(&state).expect("a folder");
    let opts = RuntimeOptions { state_path: Some(state.join("state.json").to_string_lossy().to_string()), ..with_local(root.path()) };
    drive(opts, |_, rt| async move {
        let e = rt.projects().icon("pr_gone", Some("AAAA".into())).await.expect_err("refused");
        assert!(e.message.contains("pr_gone"), "{}", e.message);
        assert!(rt.projects().icons(vec!["0".repeat(64)]).await.expect("icons")["0".repeat(64).as_str()].is_none());
    });
}
