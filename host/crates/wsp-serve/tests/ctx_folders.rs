// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 8 (#2476): the context's folders, start-from and workspace-create readings, and the
//! seam every projects area shares (`Base`, `FoldersCtx`, `StartFromCtx`, `CreateCtx`, `WorkspacesCtx`). Each test
//! builds the runtime on a throwaway state over `FakeBackend` and calls the context's method the way the area crate
//! calls it, through `&dyn Trait`. The TS is the oracle: each expectation is what `folders.ts`, `start-from.ts`,
//! `workspaces.ts` and `runtime.ts` answer. A test that reaches another packet's body names the packet it waits for.
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_account::projects::folders::FoldersCtx;
use wsp_account::projects::start_from::StartFromCtx;
use wsp_account::projects::workspaces::create::CreateCtx;
use wsp_account::projects::workspaces::seam::{Base, EntryAsk, GitAsk};
use wsp_core::clock::FakeClock;
use wsp_core::context::RuntimeState;
use wsp_core::types::api::Runtime;
use wsp_core::types::events::EventListener;
use wsp_core::types::internal::{SessionEntry, ThreadRecord};
use wsp_core::types::wiring::{HereDaemon, LocalWiring, Platform, RuntimeOptions, WorkspaceRecord};
use wsp_engine::copier::{fake_copier, Copier, FakeCopierLog, WorktreeAsk, WorktreeRemove};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions, LOCAL_MACHINE_ID};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_machines::machines::boot::BootRuntime;
use wsp_machines::machines::kinds::KindsCtx;
use wsp_machines::machines::machines::Entry;
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::store::memory_store;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceView};
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

// ----------------------------------------------------------------------------------------------------------------
// Fixtures

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

fn plain() -> RuntimeOptions {
    RuntimeOptions { backend: Some(backend()), store: Some(Rc::new(memory_store())), ..Default::default() }
}

/// This computer's wiring over a folder, as the TS tests' `fakeLocal` wires it; the copier and the daemon beside the
/// host are the test's own.
fn local_in(root: &Path, copier: Option<Rc<dyn Copier>>, here_daemon: Option<Rc<dyn HereDaemon>>) -> LocalWiring {
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
        here_daemon,
        restart_daemon: None,
        sys_samples: None,
        copier,
        platform: Platform::Linux,
        close: None,
    }
}

fn local_opts(root: &Path) -> RuntimeOptions {
    RuntimeOptions { local: Some(local_in(root, None, None)), ..plain() }
}

/// A host over a folder with a fake copier, whose log the test keeps.
fn copier_opts(root: &Path) -> (RuntimeOptions, Arc<Mutex<FakeCopierLog>>) {
    let copier = fake_copier(None, None);
    let log = copier.log.clone();
    let copier: Rc<dyn Copier> = Rc::new(copier);
    let opts = RuntimeOptions {
        local: Some(local_in(root, Some(copier), None)),
        state_path: Some(root.join("state/state.json").to_string_lossy().to_string()),
        ..plain()
    };
    (opts, log)
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

fn project_on(id: &str, name: &str, path: &str, computer: &str) -> ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "path": path, "computer": computer, "source": { "kind": "folder", "path": path }, "remote": "",
        "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a project")
}

fn project(id: &str, name: &str, path: &str) -> ProjectView {
    project_on(id, name, path, HERE_PLACE_ID)
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

/// A live entry held by the context, on a machine of the fake backend.
async fn held_entry(c: &Rc<Ctx>, extra: Value) -> Entry {
    let record = record(extra);
    let entry = entry_over(record.clone(), machine_named(&record.machine_id).await);
    base(c).live().borrow_mut().insert(record.id.clone(), entry.clone());
    entry
}

/// The runtime on a throwaway state, inside the task set the owner thread runs.
fn drive<Fut: std::future::Future<Output = ()>>(opts: RuntimeOptions, f: impl FnOnce(Rc<Ctx>, Rc<AssembledRuntime>) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let runtime = create_runtime(opts).expect("a runtime");
        let ctx: Rc<Ctx> = runtime.context();
        f(ctx, runtime).await
    });
}

fn scratch(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(&format!("wsp-folders-{tag}-")).tempdir().expect("a scratch folder")
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A repo on main with one commit.
fn repo(root: &Path) -> std::path::PathBuf {
    let at = root.join("repo");
    std::fs::create_dir_all(&at).expect("a folder");
    git(&at, &["init", "-q", "-b", "main"]);
    git(&at, &["commit", "-q", "--allow-empty", "-m", "first"]);
    std::fs::canonicalize(&at).expect("a real path")
}

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

fn base(c: &Rc<Ctx>) -> &dyn Base {
    &**c
}
fn folders(c: &Rc<Ctx>) -> &dyn FoldersCtx {
    &**c
}
fn start_from(c: &Rc<Ctx>) -> &dyn StartFromCtx {
    &**c
}
fn create(c: &Rc<Ctx>) -> &dyn CreateCtx {
    &**c
}
fn scope(thread: &str, workspace: &str, root: &str) -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": root }))
        .expect("a thread scope")
}

fn caller_by(by: ThreadScope) -> Caller {
    Caller::Scoped { origin: wsp_wire::views::workspace_more::WorkspaceOrigin::Relayed, by }
}

fn session_row(id: &str, thread: &str) -> wsp_machines::machines::machines::Row {
    let view: SessionView =
        serde_json::from_value(json!({ "id": id, "workspaceId": "w_1", "harness": "claude", "status": "running", "threadId": thread }))
            .expect("a session view");
    Rc::new(RefCell::new(SessionEntry {
        view,
        turn_id: format!("turn-{id}"),
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

// ----------------------------------------------------------------------------------------------------------------
// Base: the seam every projects area shares (workspaces.ts, folders.ts, start-from.ts, projects.ts, images.ts)

/// Waits for boot, packet 5, and the bodies boot reads: the sub queue (7), the threads and turns (11, 12).
#[test]
fn base_ready_answers_once_boot_is_done() {
    drive(plain(), |c, _| async move {
        base(&c).ready().await.expect("booted");
    });
}

#[test]
fn base_clock_and_ids_are_the_ones_the_host_wired() {
    let fake = FakeClock::new(1_700_000_000_000);
    let opts = RuntimeOptions { clock: Some(fake.clock()), ..plain() };
    drive(opts, |c, _| async move {
        assert_eq!(base(&c).clock().now(), 1_700_000_000_000);
        fake.advance(7);
        assert_eq!(base(&c).clock().now(), 1_700_000_000_007);
        assert_eq!(base(&c).ids().bytes(4).len(), 4);
    });
}

#[test]
fn base_store_is_the_store_the_runtime_was_given() {
    drive(plain(), |c, _| async move {
        base(&c).store().put("probe", "a", json!({ "n": 1 })).await.expect("put");
        assert_eq!(BootRuntime::store(&*c).get("probe", "a").await.expect("get"), Some(json!({ "n": 1 })));
    });
}

#[test]
fn base_live_is_the_one_map_every_area_holds() {
    drive(plain(), |c, _| async move {
        assert!(Rc::ptr_eq(&base(&c).live(), &BootRuntime::live(&*c)));
        let entry = held_entry(&c, json!({})).await;
        let held = BootRuntime::live(&*c).borrow().get("w_1").cloned().expect("the entry shows through the other handle");
        assert!(Rc::ptr_eq(&held, &entry));
    });
}

#[test]
fn base_projects_held_is_the_one_map_every_area_holds() {
    drive(plain(), |c, _| async move {
        assert!(Rc::ptr_eq(&base(&c).projects_held(), &BootRuntime::projects_held(&*c)));
        base(&c).projects_held().borrow_mut().insert("p_1".into(), project("p_1", "api", "/srv/api"));
        assert!(BootRuntime::projects_held(&*c).borrow().contains_key("p_1"));
    });
}

#[test]
fn base_project_held_answers_the_held_project_and_refuses_one_it_does_not_hold() {
    drive(plain(), |c, _| async move {
        base(&c).projects_held().borrow_mut().insert("p_1".into(), project("p_1", "api", "/srv/api"));
        assert_eq!(base(&c).project_held("p_1").expect("held").name, "api");
        let said = base(&c).project_held("p_9").refused("not held").message;
        assert_eq!(said, wsp_words::projects::no_such_project_line("p_9", &["api"]));
    });
}

#[test]
fn base_session_views_are_every_row_in_the_order_they_were_kept() {
    drive(plain(), |c, _| async move {
        let sessions = BootRuntime::sessions(&*c);
        sessions.borrow_mut().insert("s_2".into(), session_row("s_2", "t_2"));
        sessions.borrow_mut().insert("s_1".into(), session_row("s_1", "t_1"));
        let ids: Vec<String> = base(&c).session_views().into_iter().map(|v| v.id).collect();
        assert_eq!(ids, ["s_2", "s_1"]);
    });
}

#[test]
fn base_thread_records_are_the_core_map_as_id_and_record_pairs_in_order() {
    drive(plain(), |c, _| async move {
        let records = BootRuntime::thread_records(&*c);
        let a: ThreadRecord = serde_json::from_value(json!({ "workspaceId": "w_1", "harness": "claude" })).expect("a thread record");
        let b: ThreadRecord = serde_json::from_value(json!({ "workspaceId": "w_2", "harness": "claude" })).expect("a thread record");
        records.borrow_mut().insert("t_b".into(), b);
        records.borrow_mut().insert("t_a".into(), a);
        let pairs = base(&c).thread_records();
        let ids: Vec<&str> = pairs.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["t_b", "t_a"]);
        assert_eq!(pairs[0].1.workspace_id, "w_2");
    });
}

#[test]
fn base_persist_writes_the_record_under_its_id() {
    drive(plain(), |c, _| async move {
        let r = record(json!({}));
        base(&c).persist(r.clone()).await.expect("persisted");
        let stored = base(&c).store().get("workspaces", "w_1").await.expect("get").expect("a row");
        assert_eq!(stored["name"], json!("api"));
        assert_eq!(stored["id"], json!("w_1"));
    });
}

#[test]
fn base_view_is_the_record_as_the_person_reads_it() {
    drive(plain(), |c, _| async move {
        base(&c).projects_held().borrow_mut().insert("p_1".into(), project("p_1", "api", "/srv/api"));
        let view = base(&c).view(&record(json!({}))).expect("a view");
        assert_eq!(view.id, "w_1");
        assert_eq!(view.name, "api");
        assert_eq!(view.project.id, "p_1");
        assert_eq!(view.project.name, "api");
    });
}

#[test]
fn base_emit_stamps_the_event_and_delivers_it_to_the_bus() {
    drive(plain(), |c, runtime| async move {
        let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
        let sink = seen.clone();
        let listener: EventListener = Rc::new(move |e: &EventUnion| {
            sink.borrow_mut().push(serde_json::to_value(e).expect("an event"));
            Ok(())
        });
        let _off = runtime.events().on("host.notice", listener);
        base(&c).emit(json!({ "type": "host.notice", "message": "hello" })).expect("emitted");
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0]["message"], json!("hello"));
        assert_eq!(seen[0]["seq"], json!(1));
    });
}

#[test]
fn base_emit_lets_a_listeners_failure_reach_the_caller() {
    drive(plain(), |c, runtime| async move {
        let failing: EventListener = Rc::new(|_| Err(HostError::new("no")));
        let _off = runtime.events().on("host.notice", failing);
        let said = base(&c).emit(json!({ "type": "host.notice", "message": "hello" })).refused("the listener's failure");
        assert_eq!(said.message, "no");
    });
}

/// Waits for boot (5 and the bodies it reads, 7, 11, 12).
#[test]
fn base_entry_of_answers_a_held_entry_and_refuses_an_absent_one() {
    drive(plain(), |c, _| async move {
        let entry = held_entry(&c, json!({})).await;
        let found = base(&c).entry_of("w_1", None, EntryAsk::default()).await.expect("the entry");
        assert!(Rc::ptr_eq(&found, &entry));
        let said = base(&c).entry_of("w_zz", None, EntryAsk::default()).await.refused("absent").message;
        assert!(said.contains("w_zz"), "{said}");
    });
}

#[test]
fn base_spawn_guard_frees_where_nobody_is_asking_and_refuses_a_thread_whose_folder_is_gone() {
    drive(plain(), |c, _| async move {
        let free = base(&c).spawn_guard("fork", None).expect("a free slot");
        free();
        free();
        let by = caller_by(scope("t_1", "w_gone", "t_1"));
        let said = base(&c).spawn_guard("fork", Some(&by)).refused("its workspace is not held").message;
        assert_eq!(said, wsp_words::words::computer::folder_gone_refusal("t_1", wsp_words::words::computer::SpawnAct::Fork));
    });
}

#[test]
fn base_kind_of_reads_the_computer_word() {
    drive(plain(), |c, _| async move {
        assert_eq!(base(&c).kind_of(HERE_PLACE_ID), WorkspaceKind::Local);
        assert_eq!(base(&c).kind_of("solari"), WorkspaceKind::Cloud);
    });
}

#[test]
fn base_place_name_is_this_computers_hostname_for_here_and_the_provider_key_name_else() {
    drive(plain(), |c, _| async move {
        assert_eq!(base(&c).place_name(HERE_PLACE_ID), wsp_core::host_id::hostname());
        assert_eq!(base(&c).place_name("solari"), wsp_words::place_word::provider_key_name("solari"));
    });
}

/// Waits for the channels' reads of a computer's rows (packet 3 is in; the kinds' reach reads are 4).
#[test]
fn base_copy_blocked_lets_an_entry_on_this_hosts_own_machines_through() {
    drive(plain(), |c, _| async move {
        let entry = held_entry(&c, json!({})).await;
        base(&c).copy_blocked(&entry).await.expect("not blocked");
    });
}

/// With no daemon wired, this computer has none to ask: the refusal the TS throws, coded `no-host-cli`.
#[test]
fn base_on_this_computer_refuses_where_no_daemon_is_wired() {
    drive(plain(), |c, _| async move {
        let work: wsp_account::projects::workspaces::seam::HereWork = Box::new(|_ask, _home| async { Ok(json!(1)) }.boxed_local());
        let refused = base(&c).on_this_computer(work).await.refused("no daemon here");
        assert_eq!(refused.message, "this host runs no daemon on this computer");
        assert_eq!(refused.code.as_deref(), Some("no-host-cli"));
    });
}

#[test]
fn base_git_here_runs_git_in_a_folder_on_this_computer() {
    let root = scratch("git-here");
    let folder = repo(root.path());
    drive(local_opts(root.path()), |c, _| async move {
        let ask = GitAsk {
            cwd: folder.to_string_lossy().to_string(),
            args: vec!["rev-parse".into(), "--abbrev-ref".into(), "HEAD".into()],
            timeout_ms: Some(10_000),
            input: None,
        };
        let res = base(&c).git_here(ask).await.expect("git ran");
        assert_eq!(res.exit_code, 0.into());
        assert_eq!(res.stdout.trim(), "main");
    });
}

#[test]
fn base_checkout_of_reads_the_folder_a_record_stands_in() {
    drive(plain(), |c, _| async move {
        base(&c).projects_held().borrow_mut().insert("p_1".into(), project("p_1", "api", "/srv/api"));
        let r = record(json!({}));
        assert_eq!(base(&c).checkout_of(&r).map_err(|e| e.message), KindsCtx::checkout_of(&*c, &r).map_err(|e| e.into_host().message));
        let gone = record(json!({ "project": "p_9" }));
        let said = base(&c).checkout_of(&gone).refused("a project not held").message;
        assert_eq!(said, wsp_words::projects::no_such_project_line("p_9", &["api"]));
    });
}

/// Waits for the preferences area's context (packet 16).
#[test]
fn base_preferences_reads_the_preferences_record() {
    drive(plain(), |c, _| async move {
        let prefs = base(&c).preferences().await.expect("preferences");
        let again = base(&c).preferences().await.expect("preferences");
        assert_eq!(serde_json::to_value(&prefs).expect("json"), serde_json::to_value(&again).expect("json"));
    });
}

#[test]
fn base_home_dir_is_the_wired_homes_folder_else_the_users_home() {
    let root = scratch("home");
    drive(local_opts(root.path()), |c, _| async move {
        assert_eq!(base(&c).home_dir(), root.path().to_string_lossy());
    });
    drive(plain(), |c, _| async move {
        let home = std::env::var("HOME").expect("HOME is set");
        assert_eq!(base(&c).home_dir(), home);
    });
}

#[test]
fn base_machine_id_is_the_entrys_machine() {
    drive(plain(), |c, _| async move {
        let entry = held_entry(&c, json!({ "machineId": "m_7" })).await;
        let id = {
            let held = entry.borrow();
            held.machine.id().to_string()
        };
        assert_eq!(base(&c).machine_id(&entry), id);
    });
}

// ----------------------------------------------------------------------------------------------------------------
// FoldersCtx (folders.ts:23-472)

#[test]
fn folders_state_folder_is_the_folder_the_state_file_sits_in() {
    let opts = RuntimeOptions { state_path: Some("/srv/wsp/state.db".into()), ..plain() };
    drive(opts, |c, _| async move {
        assert_eq!(folders(&c).state_folder().expect("a folder"), "/srv/wsp");
    });
}

#[test]
fn folders_state_folder_refuses_a_host_with_no_state_file_in_the_words_the_ts_throws() {
    drive(plain(), |c, _| async move {
        let said = folders(&c).state_folder().refused("no state file");
        assert_eq!(said.message, "this runtime was given no state file, so it keeps no worktrees or copies");
        assert_eq!(said.kind, Some(wsp_words::exit::Kind::Invalid));
    });
}

#[test]
fn folders_has_state_path_reads_the_option() {
    drive(plain(), |c, _| async move {
        assert!(!folders(&c).has_state_path());
    });
    drive(RuntimeOptions { state_path: Some("/srv/wsp/state.db".into()), ..plain() }, |c, _| async move {
        assert!(folders(&c).has_state_path());
    });
}

#[test]
fn folders_has_copier_reads_the_local_wiring() {
    let root = scratch("has-copier");
    drive(plain(), |c, _| async move {
        assert!(!folders(&c).has_copier());
    });
    drive(local_opts(root.path()), |c, _| async move {
        assert!(!folders(&c).has_copier());
    });
    let (opts, _log) = copier_opts(root.path());
    drive(opts, |c, _| async move {
        assert!(folders(&c).has_copier());
    });
}

#[test]
fn folders_copier_worktree_asks_the_wired_copier_and_answers_its_report() {
    let root = scratch("worktree");
    let (opts, log) = copier_opts(root.path());
    let home = root.path().join("state").to_string_lossy().to_string();
    drive(opts, |c, _| async move {
        let ask = WorktreeAsk {
            from: "/srv/api".into(),
            home: home.clone(),
            project: "api".into(),
            branch: "feat/one".into(),
            modules: vec![],
            checkpoint: None,
        };
        let report = folders(&c).copier_worktree(ask.clone()).await.expect("a worktree");
        assert_eq!(report.branch, "feat/one");
        assert!(report.made);
        assert_eq!(report.path, format!("{home}/worktrees/api/feat-one"));
        assert_eq!(log.lock().unwrap().worktrees, vec![ask]);
    });
}

#[test]
fn folders_copier_calls_refuse_in_the_words_of_a_host_with_no_daemon_binary_beside_it() {
    let root = scratch("no-copier");
    drive(local_opts(root.path()), |c, _| async move {
        let ask = WorktreeAsk {
            from: "/srv/api".into(),
            home: "/srv/state".into(),
            project: "api".into(),
            branch: "feat/one".into(),
            modules: vec![],
            checkpoint: None,
        };
        let said = folders(&c).copier_worktree(ask).await.refused("no copier").message;
        assert_eq!(said, wsp_core::types::wiring::NO_COPIER_HERE);
        let said = folders(&c).copier_worktree_mount("/srv/api", "/srv/state", "/srv/state/worktrees/api/x").await.refused("no copier");
        assert_eq!(said.message, wsp_core::types::wiring::NO_COPIER_HERE);
    });
}

#[test]
fn folders_copier_worktree_mount_hands_the_three_paths_to_the_copier() {
    let root = scratch("mount");
    let (opts, log) = copier_opts(root.path());
    drive(opts, |c, _| async move {
        folders(&c).copier_worktree_mount("/srv/api", "/srv/state", "/srv/state/worktrees/api/x").await.expect("mounted");
        assert_eq!(log.lock().unwrap().worktrees_mounted, ["/srv/state/worktrees/api/x"]);
    });
}

#[test]
fn folders_copier_worktree_remove_hands_the_ask_to_the_copier_and_drops_its_answer() {
    let root = scratch("remove");
    let (opts, log) = copier_opts(root.path());
    drive(opts, |c, _| async move {
        let ask =
            WorktreeRemove { from: "/srv/api".into(), home: "/srv/state".into(), path: "/srv/state/worktrees/api/x".into(), force: true };
        folders(&c).copier_worktree_remove(ask.clone()).await.expect("removed");
        assert_eq!(log.lock().unwrap().worktrees_removed, vec![ask]);
    });
}

struct HereAt;

#[async_trait::async_trait(?Send)]
impl HereDaemon for HereAt {
    async fn version(&self) -> Result<u32, HostError> {
        Ok(9)
    }
    fn fix(&self) -> String {
        "update it".into()
    }
}

#[test]
fn folders_here_daemon_is_the_one_the_local_wiring_holds() {
    let root = scratch("here-daemon");
    drive(plain(), |c, _| async move {
        assert!(folders(&c).here_daemon().is_none());
    });
    let here: Rc<dyn HereDaemon> = Rc::new(HereAt);
    let opts = RuntimeOptions { local: Some(local_in(root.path(), None, Some(here))), ..plain() };
    drive(opts, |c, _| async move {
        let held = folders(&c).here_daemon().expect("the daemon beside the host");
        assert_eq!(held.version().await.expect("a version"), 9);
    });
}

#[test]
fn folders_admit_folder_lets_a_kind_with_no_admission_through() {
    let root = scratch("admit");
    drive(local_opts(root.path()), |c, _| async move {
        folders(&c).admit_folder(WorkspaceKind::Local, "api", None).await.expect("admitted");
    });
}

#[test]
fn folders_folder_machine_is_this_computers_machine_at_the_backends_default_size() {
    let root = scratch("machine");
    drive(local_opts(root.path()), |c, _| async move {
        let (machine, size) = folders(&c).folder_machine(WorkspaceKind::Local, None).await.expect("a machine");
        assert_eq!(machine.id(), LOCAL_MACHINE_ID);
        let want = LocalBackend::new(LocalBackendOptions { root: root.path().to_path_buf(), env: None }).pricing().default_size();
        assert_eq!(size.cpu.0, want.cpu);
        assert_eq!(size.mem_mb.0, want.mem_mb);
    });
}

#[test]
fn folders_folder_machine_refuses_a_kind_this_host_has_no_backend_for() {
    drive(plain(), |c, _| async move {
        let said = folders(&c).folder_machine(WorkspaceKind::Local, None).await.refused("no local wiring").message;
        assert_eq!(said, wsp_words::words::computer::no_kind_line("local"));
    });
}

/// Waits for the machines area's context (packet 4): `attach` builds the entry through it.
#[test]
fn folders_attach_makes_the_record_a_live_entry_on_its_machine() {
    drive(plain(), |c, _| async move {
        let record = record(json!({}));
        let entry = folders(&c).attach(record, machine_named("m_1").await).expect("an entry");
        assert_eq!(entry.borrow().record.borrow().id, "w_1");
        assert!(base(&c).live().borrow().contains_key("w_1"));
    });
}

/// Waits for the create area's context (packet 6): `foldersOf` reads the live map through it.
#[test]
fn folders_folders_of_is_the_entries_of_a_project_in_the_order_held() {
    drive(plain(), |c, _| async move {
        held_entry(&c, json!({ "id": "w_1", "project": "p_1", "kind": "local" })).await;
        held_entry(&c, json!({ "id": "w_2", "project": "p_2", "kind": "local" })).await;
        held_entry(&c, json!({ "id": "w_3", "project": "p_1", "kind": "local" })).await;
        held_entry(&c, json!({ "id": "w_4", "project": "p_1", "kind": "cloud" })).await;
        let ids: Vec<String> = folders(&c).folders_of("p_1").iter().map(|e| e.borrow().record.borrow().id.clone()).collect();
        assert_eq!(ids, ["w_1", "w_3"]);
    });
}

#[test]
fn folders_elsewhere_refusal_is_none_for_a_caller_that_is_no_thread() {
    drive(plain(), |c, _| async move {
        let p = project("p_1", "api", "/srv/api");
        assert!(folders(&c).elsewhere_refusal(None, &p, "api").expect("a read").is_none());
    });
}

/// start-from.ts:112: a project not on a computer that copies its folders has no picks to read before a folder is made.
#[test]
fn folders_picks_hold_lets_a_project_on_a_cloud_computer_through() {
    drive(plain(), |c, _| async move {
        let p = project_on("p_1", "api", "/srv/api", "solari");
        folders(&c).picks_hold(&p, &Default::default()).await.expect("nothing to read");
    });
}
#[test]
fn folders_branch_at_reads_the_branch_a_folder_has_checked_out() {
    let root = scratch("branch-at");
    let folder = repo(root.path());
    drive(local_opts(root.path()), |c, _| async move {
        assert_eq!(folders(&c).branch_at(&folder.to_string_lossy()).await.expect("a read").as_deref(), Some("main"));
        let plain_folder = root.path().join("plain");
        std::fs::create_dir_all(&plain_folder).expect("a folder");
        assert_eq!(folders(&c).branch_at(&plain_folder.to_string_lossy()).await.expect("a read"), None);
    });
}

/// Waits for the daemon area's context (packet 4).
#[test]
fn folders_turn_runs_is_false_where_no_turn_runs_in_the_workspace() {
    drive(plain(), |c, _| async move {
        held_entry(&c, json!({})).await;
        assert!(!folders(&c).turn_runs("w_1"));
    });
}

/// Waits for the machines area's context (packet 4).
#[test]
fn folders_drop_workspace_takes_the_entry_out_of_the_live_map() {
    drive(plain(), |c, _| async move {
        held_entry(&c, json!({})).await;
        folders(&c).drop_workspace("w_1").await.expect("dropped");
        assert!(!base(&c).live().borrow().contains_key("w_1"));
    });
}

#[test]
fn folders_checkpoints_landed_returns_at_once_for_a_thread_with_none_landing() {
    drive(plain(), |c, _| async move {
        folders(&c).checkpoints_landed("t_none").await;
    });
}

/// Waits for the threads area's context (packet 11): `notARepo` reads the held project through it.
#[test]
fn folders_not_a_repo_is_true_for_a_folder_thread_on_a_project_with_no_git() {
    drive(plain(), |c, _| async move {
        base(&c).projects_held().borrow_mut().insert("p_1".into(), project("p_1", "api", "/srv/api"));
        let folder = record(json!({ "kind": "local", "place": "here" }));
        assert!(folders(&c).not_a_repo(&folder).expect("read"));
        assert!(!folders(&c).not_a_repo(&record(json!({ "kind": "cloud" }))).expect("read"));
    });
}

#[test]
fn folders_local_exec_runs_a_command_on_this_computers_machine() {
    let root = scratch("local-exec");
    drive(local_opts(root.path()), |c, _| async move {
        let res = folders(&c).local_exec("echo hello", 10_000, None).await.expect("ran");
        assert_eq!(res.exit_code, 0.into());
        assert_eq!(res.stdout, "hello\n");
        let res = folders(&c).local_exec("cat", 10_000, Some(b"piped".to_vec())).await.expect("ran");
        assert_eq!(res.stdout, "piped");
    });
}

#[test]
fn folders_state_is_the_one_cell_every_area_holds() {
    drive(plain(), |c, _| async move {
        BootRuntime::set_owner(&*c, "me");
        assert_eq!(folders(&c).state().with(|s: &mut RuntimeState| s.owner.clone()), "me");
        folders(&c).state().with(|s: &mut RuntimeState| s.sweep_stopped = true);
        assert!(BootRuntime::closed(&*c) || folders(&c).state().with(|s: &mut RuntimeState| s.sweep_stopped));
    });
}

#[test]
fn folders_spawn_runs_the_future_now_as_a_void_call_does() {
    drive(plain(), |c, _| async move {
        let done = Rc::new(Cell::new(false));
        let flag = done.clone();
        folders(&c).spawn(async move { flag.set(true) }.boxed_local());
        tokio::task::yield_now().await;
        assert!(done.get());
    });
}

// ----------------------------------------------------------------------------------------------------------------
// StartFromCtx (start-from.ts:15-262)

struct FakeBroker {
    ready: bool,
}

#[async_trait::async_trait(?Send)]
impl wsp_core::types::wiring::Broker for FakeBroker {
    fn ready(&self) -> bool {
        self.ready
    }
    async fn codex_claims(&self) -> Result<Option<wsp_adapters::pending::CodexClaims>, HostError> {
        Ok(None)
    }
    fn status(&self) -> Option<wsp_core::types::wiring::Local<'static, wsp_wire::wire::runtime::BrokerStatus>> {
        None
    }
}

#[test]
fn start_and_create_read_the_places_table_wired_row_and_a_host_with_no_door_guards_no_place() {
    drive(plain(), |c, _| async move {
        assert_eq!(start_from(&c).places_wired(), "default");
        assert_eq!(create(&c).places_wired(), "default");
        create(&c).place_guard("default").await.expect("no door, no guard");
    });
}

#[test]
fn start_spawn_runs_the_future_now() {
    drive(plain(), |c, _| async move {
        let done = Rc::new(Cell::new(false));
        let flag = done.clone();
        start_from(&c).spawn(async move { flag.set(true) }.boxed_local());
        tokio::task::yield_now().await;
        assert!(done.get());
    });
}

/// Waits for the projects area's context (packet 10): `landingDeps` reads the landing place through it.
#[test]
fn start_landing_place_is_no_place_for_this_computer() {
    drive(plain(), |c, _| async move {
        assert_eq!(start_from(&c).landing_place(HERE_PLACE_ID).await.expect("a read"), None);
    });
}

/// Waits for the projects area's context (packet 10).
#[test]
fn start_landing_kind_is_mac_for_this_computer_and_provider_for_a_cloud_one() {
    use wsp_account::projects::projects::add::LandingKind;
    drive(plain(), |c, _| async move {
        assert_eq!(start_from(&c).landing_kind(HERE_PLACE_ID), LandingKind::Mac);
        assert_eq!(start_from(&c).landing_kind("solari"), LandingKind::Provider);
    });
}

#[test]
fn start_has_no_setup_line_for_a_workspace_that_made_none_and_create_lays_agents_over_the_defaults() {
    drive(plain(), |c, _| async move {
        assert_eq!(start_from(&c).take_setup_line("w_1"), None);
        let patched = create(&c).agents_from(HERE_PLACE_ID, json!({ "maxMachines": 3 }).as_object().expect("an object"));
        assert_eq!(serde_json::to_value(&patched).expect("json")["maxMachines"], json!(3));
        let (p, by) = (project("p_1", "api", "/srv/api"), scope("t_1", "w_0", "t_0"));
        let view = create(&c).shown_view("ws_1", "api", WorkspaceKind::Cloud, "g", "2026-10-26T00:00:00.000Z", &p, &by, None);
        assert!(create(&c).drives(&view, None));
    });
}

#[test]
fn start_brokered_start_needs_a_ready_broker_and_a_provider_road() {
    let ready = RuntimeOptions { broker: Some(Rc::new(FakeBroker { ready: true })), ..plain() };
    drive(ready, |c, _| async move {
        assert!(start_from(&c).brokered_start(true));
        assert!(!start_from(&c).brokered_start(false));
    });
    let idle = RuntimeOptions { broker: Some(Rc::new(FakeBroker { ready: false })), ..plain() };
    drive(idle, |c, _| async move {
        assert!(!start_from(&c).brokered_start(true));
    });
    drive(plain(), |c, _| async move {
        assert!(!start_from(&c).brokered_start(true));
    });
}

#[test]
fn start_pull_request_of_record_refuses_a_remote_that_is_not_github_before_asking_anything() {
    drive(plain(), |c, _| async move {
        let said = start_from(&c).pull_request_of_record("", 7, "api", "zingzys-mac", true).await.refused("no github remote").message;
        assert!(said.contains("api"), "{said}");
    });
}

/// Waits for the create area's roads (packet 6): the create runs through them.
#[test]
fn start_create_workspace_refuses_a_project_this_host_does_not_hold() {
    drive(plain(), |c, _| async move {
        let opts = wsp_core::types::wiring::CreateWorkspaceOptions { project: "p_9".into(), name: "x".into(), ..Default::default() };
        let said = start_from(&c).create_workspace(opts, None).await.refused("no such project").message;
        assert!(said.contains("p_9"), "{said}");
    });
}

/// Waits for the phases area's context (packet 9): a delete runs through it.
#[test]
fn start_delete_workspace_refuses_a_workspace_this_host_does_not_hold() {
    drive(plain(), |c, _| async move {
        let said = start_from(&c).delete_workspace("w_zz", None).await.refused("no such workspace").message;
        assert!(said.contains("w_zz"), "{said}");
    });
}

/// Waits for the machines (4), create (6) and daemon (4) contexts: the folder's record is made through them.
#[test]
fn start_project_folder_is_the_one_record_of_the_projects_folder() {
    let root = scratch("project-folder");
    let folder = repo(root.path());
    let (opts, log) = copier_opts(root.path());
    drive(opts, |c, _| async move {
        let held = project("p_1", "api", &folder.to_string_lossy());
        base(&c).projects_held().borrow_mut().insert("p_1".into(), held.clone());
        let a = start_from(&c).project_folder(&held).await.expect("a folder record");
        let b = start_from(&c).project_folder(&held).await.expect("a folder record");
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(a.borrow().record.borrow().project, "p_1");
        assert!(a.borrow().record.borrow().worktree.is_none());
        assert!(log.lock().unwrap().asks.is_empty(), "no copy is made for a thread on a folder");
    });
}

/// Waits for the machines (4), create (6) and daemon (4) contexts.
#[test]
fn start_worktree_folder_is_the_record_of_the_worktree_holding_a_branch() {
    let root = scratch("worktree-folder");
    let folder = repo(root.path());
    let (opts, log) = copier_opts(root.path());
    drive(opts, |c, _| async move {
        let held = project("p_1", "api", &folder.to_string_lossy());
        base(&c).projects_held().borrow_mut().insert("p_1".into(), held.clone());
        let top = folder.to_string_lossy().to_string();
        let entry = start_from(&c).worktree_folder(&held, &top, "feat/one").await.expect("a worktree record");
        let wt = entry.borrow().record.borrow().worktree.clone().expect("a worktree on the record");
        assert_eq!(wt.branch.as_deref(), Some("feat/one"));
        assert!(wt.made);
        assert_eq!(log.lock().unwrap().worktrees.len(), 1);
    });
}

#[test]
fn start_worktrees_of_lists_what_git_lists_for_the_repo() {
    let root = scratch("worktrees-of");
    let folder = repo(root.path());
    let other = root.path().join("other");
    git(&folder, &["worktree", "add", "-q", "-b", "feat/two", &other.to_string_lossy()]);
    let other = std::fs::canonicalize(&other).expect("a real path");
    drive(local_opts(root.path()), |c, _| async move {
        let listed = start_from(&c).worktrees_of(&folder.to_string_lossy()).await.expect("a listing");
        let rows: Vec<(String, Option<String>)> = listed.into_iter().map(|w| (w.path, w.branch)).collect();
        assert_eq!(
            rows,
            [
                (folder.to_string_lossy().to_string(), Some("main".to_string())),
                (other.to_string_lossy().to_string(), Some("feat/two".to_string()))
            ]
        );
    });
}

#[test]
fn start_preferences_held_is_the_record_as_last_read_or_written() {
    drive(plain(), |c, _| async move {
        assert!(start_from(&c).preferences_held().is_none());
        let prefs = wsp_wire::views::preferences::default_preferences();
        folders(&c).state().with(|s: &mut RuntimeState| s.preferences_held = Some(prefs.clone()));
        assert_eq!(start_from(&c).preferences_held(), Some(prefs));
    });
}

/// Waits for the agents area's context (packet 11).
#[test]
fn start_default_agent_of_reads_the_agent_the_preferences_name() {
    drive(plain(), |c, _| async move {
        let entry = held_entry(&c, json!({})).await;
        let prefs = wsp_wire::views::preferences::default_preferences();
        let agent = start_from(&c).default_agent_of(&prefs, &entry);
        assert!(!agent.is_empty());
    });
}

#[test]
fn start_access_word_refusal_is_the_words_of_the_catalog_that_lacks_the_word() {
    use wsp_wire::types::thread_defaults::AccessChoice;
    drive(plain(), |c, _| async move {
        for catalog in wsp_core::harness_catalog::harness_catalogs() {
            for word in [AccessChoice::Ask, AccessChoice::AutoEdit, AccessChoice::Full, AccessChoice::Plan] {
                let want = wsp_words::thread_defaults::access_word_refusal(&catalog, word).map(|e| e.message);
                let said = start_from(&c).access_word_refusal(&catalog, &word).map(|e| e.message);
                assert_eq!(said, want, "{} on {word:?}", catalog.label);
            }
        }
    });
}

#[test]
fn start_start_picks_refusal_is_none_for_picks_a_catalog_runs_and_the_refusal_for_one_it_does_not() {
    use wsp_core::types::harness::StartPicksAsked;
    drive(plain(), |c, _| async move {
        let catalog = wsp_core::harness_catalog::harness_catalog("claude").expect("claude has a catalog");
        assert!(start_from(&c).start_picks_refusal(&catalog, &StartPicksAsked::default()).is_none());
        let bad = StartPicksAsked { model: Some("no-such-model".into()), ..Default::default() };
        let refused = start_from(&c).start_picks_refusal(&catalog, &bad).expect("a refusal");
        assert!(refused.message.contains("no-such-model"), "{}", refused.message);
    });
}

#[test]
fn start_read_only_modes_are_every_harness_and_its_read_only_word() {
    drive(plain(), |c, _| async move {
        let rows = start_from(&c).read_only_modes();
        let want: Vec<(String, Option<String>)> =
            wsp_core::harness_catalog::harness_catalogs().into_iter().map(|c| (c.harness, c.read_only_mode)).collect();
        assert_eq!(rows, want);
        assert!(rows.iter().any(|(id, mode)| id == "codex" && mode.is_some()));
    });
}

// ----------------------------------------------------------------------------------------------------------------
// CreateCtx (workspaces.ts:67-287, 1211-1215)

/// Waits for the projects area's context (packet 10).
#[test]
fn create_resolve_project_reads_the_project_door() {
    drive(plain(), |c, _| async move {
        base(&c).projects_held().borrow_mut().insert("p_1".into(), project("p_1", "api", "/srv/api"));
        assert_eq!(create(&c).resolve_project("api", None).await.expect("the project").id, "p_1");
        let said = create(&c).resolve_project("nope", None).await.refused("no such project").message;
        assert!(said.contains("nope"), "{said}");
    });
}

#[test]
fn create_landing_place_is_no_place_for_this_computer_and_the_wired_provider() {
    drive(plain(), |c, _| async move {
        assert_eq!(create(&c).landing_place(HERE_PLACE_ID).await.expect("a read"), None);
        assert_eq!(create(&c).landing_place("default").await.expect("a read"), None);
    });
}

/// Waits for the builders area's context (packet 14): the landing backend is read through it.
#[test]
fn create_landing_capabilities_are_the_capabilities_of_the_backend_the_create_forks_on() {
    drive(plain(), |c, _| async move {
        let caps = create(&c).landing_capabilities(None).await.expect("capabilities");
        let want = backend().capabilities();
        assert_eq!(serde_json::to_value(&caps).expect("json"), serde_json::to_value(&want).expect("json"));
    });
}

#[test]
fn create_backend_capabilities_are_the_capabilities_of_the_kinds_backend() {
    let root = scratch("caps");
    drive(local_opts(root.path()), |c, _| async move {
        let caps = create(&c).backend_capabilities(WorkspaceKind::Local, None).expect("capabilities");
        let want = LocalBackend::new(LocalBackendOptions { root: root.path().to_path_buf(), env: None }).capabilities();
        assert_eq!(serde_json::to_value(&caps).expect("json"), serde_json::to_value(&want).expect("json"));
    });
    drive(plain(), |c, _| async move {
        let said = create(&c).backend_capabilities(WorkspaceKind::Local, None).refused("no local kind").message;
        assert_eq!(said, wsp_words::words::computer::no_kind_line("local"));
    });
}

#[test]
fn create_act_refusal_is_the_error_naming_the_act_a_thread_may_not_take() {
    drive(plain(), |c, _| async move {
        let held = project("p_1", "api", "/srv/api");
        base(&c).projects_held().borrow_mut().insert("p_1".into(), held);
        held_entry(&c, json!({})).await;
        let by = scope("t_1", "w_1", "t_1");
        let said = create(&c).act_refusal(&by, "size").expect("an error to throw").message;
        assert!(said.contains(wsp_words::words::computer::SpawnAct::Size.words()), "{said}");
        let said = create(&c).act_refusal(&by, "agents").expect("an error to throw").message;
        assert!(said.contains(wsp_words::words::computer::SpawnAct::Agents.words()), "{said}");
    });
}

/// sub-spawn.ts:137: a line that came from no thread is refused before anything else is read.
#[test]
fn create_spawn_sub_refuses_a_line_that_came_from_no_thread() {
    drive(plain(), |c, _| async move {
        let sub = wsp_core::types::wiring::SubAsk { on: "box".into(), ..Default::default() };
        let opts = wsp_core::types::wiring::CreateWorkspaceOptions {
            project: "p_9".into(),
            name: "x".into(),
            sub: Some(sub),
            ..Default::default()
        };
        let said = create(&c).spawn_sub(opts, None).await.refused("no thread asked");
        assert_eq!(said.message, wsp_words::sub_thread::sub_no_thread_refusal("box").message);
    });
}

#[test]
fn create_place_of_thread_is_none_for_a_thread_whose_workspace_is_not_held() {
    drive(plain(), |c, _| async move {
        assert_eq!(create(&c).place_of_thread(&scope("t_1", "w_gone", "t_1")).expect("a read"), None);
    });
}

#[test]
fn create_name_given_trims_as_javascript_does() {
    drive(plain(), |c, _| async move {
        assert_eq!(create(&c).name_given("  api work \u{a0}\n"), "api work");
        assert_eq!(create(&c).child_started_line("kid", "feat/x"), wsp_words::tree::child_started_line("kid", "feat/x"));
    });
}

#[test]
fn create_refuse_recording_and_relayed_let_the_person_through() {
    drive(plain(), |c, _| async move {
        create(&c).refuse_recording("hello", None).expect("no refusal for a road with no thread");
        create(&c).refuse_relayed(WorkspaceKind::Cloud, "hello", "p_1", None).expect("no refusal for a road with no thread");
    });
}

#[test]
fn create_same_repository_is_true_for_one_remote_and_false_for_two() {
    drive(plain(), |c, _| async move {
        let mut a = project("p_1", "api", "/srv/api");
        let mut b = project_on("p_2", "api-box", "/srv/box/api", "a-box");
        a.remote = "https://github.com/dev/api.git".into();
        b.remote = "https://github.com/dev/api.git".into();
        assert!(create(&c).same_repository(&a, &b, None));
        b.remote = "https://github.com/dev/other.git".into();
        assert!(!create(&c).same_repository(&a, &b, None));
    });
}

#[test]
fn create_name_refusal_reads_a_blank_name_a_held_name_and_a_name_being_forked() {
    drive(plain(), |c, _| async move {
        assert_eq!(create(&c).name_refusal("   "), Some(wsp_words::words::thread::BLANK_NAME_REFUSAL.to_string()));
        assert_eq!(create(&c).name_refusal("fresh"), None);
        create(&c).forking_add("fresh");
        assert_eq!(create(&c).name_refusal("fresh"), Some(wsp_words::words::thread::name_taken_refusal("fresh")));
        create(&c).forking_delete("fresh");
        assert_eq!(create(&c).name_refusal("fresh"), None);
        held_entry(&c, json!({ "name": "api" })).await;
        assert_eq!(create(&c).name_refusal("api"), Some(wsp_words::words::thread::name_taken_refusal("api")));
    });
}

#[test]
fn create_supersede_failed_takes_the_failed_create_of_that_name_away() {
    drive(plain(), |c, _| async move {
        let p = project("p_1", "api", "/srv/api");
        let failed = create(&c).failed_view("ws_1", "retry", WorkspaceKind::Cloud, "g", 1_700_000_000_000, &p, "it broke");
        create(&c).failed_creates().borrow_mut().insert("ws_1".into(), failed);
        create(&c).supersede_failed("other");
        assert!(create(&c).failed_creates().borrow().contains_key("ws_1"));
        create(&c).supersede_failed("retry");
        assert!(!create(&c).failed_creates().borrow().contains_key("ws_1"));
    });
}

#[test]
fn create_stage_reporter_keeps_the_last_stage_of_a_create_in_flight() {
    use wsp_wire::views::workspace_events::WorkspaceCreateStage;
    drive(plain(), |c, _| async move {
        let report = create(&c).stage_reporter("ws_1", "api", 1_700_000_000_000, None);
        report(WorkspaceCreateStage::ForkRequested, "", None);
        let stages = create(&c).create_stages();
        assert_eq!(stages.len(), 1);
        assert_eq!(serde_json::to_value(&stages[0]).expect("json")["workspaceId"], json!("ws_1"));
    });
}

/// workspaces.ts:169: `{ id, name, machineId: "", phase: "queued", kind, golden, createdAt, project: refOf(project),
/// ...treeOf(spawned), ...(parent !== undefined ? { parentWorkspaceId } : {}) }`, keys in that order.
#[test]
fn create_shown_view_is_the_queued_view_with_its_keys_in_the_order_the_ts_writes_them() {
    drive(plain(), |c, _| async move {
        let p = project("p_1", "api", "/srv/api");
        let by = scope("t_1", "w_0", "t_0");
        let view = create(&c).shown_view("ws_1", "api", WorkspaceKind::Cloud, "g_1", "2026-10-26T00:00:00.000Z", &p, &by, Some("w_0"));
        let json = serde_json::to_value(&view).expect("json");
        let keys: Vec<&str> = json.as_object().expect("an object").keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "id",
                "name",
                "machineId",
                "phase",
                "kind",
                "golden",
                "createdAt",
                "project",
                "parentThreadId",
                "rootThreadId",
                "parentWorkspaceId"
            ]
        );
        assert_eq!(json["machineId"], json!(""));
        assert_eq!(json["phase"], json!("queued"));
        assert_eq!(json["kind"], json!("cloud"));
        assert_eq!(json["golden"], json!("g_1"));
        assert_eq!(json["project"], json!({ "id": "p_1", "name": "api", "path": "/srv/api", "computer": HERE_PLACE_ID }));
        assert_eq!(json["parentThreadId"], json!("t_1"));
        assert_eq!(json["rootThreadId"], json!("t_0"));
        assert_eq!(json["parentWorkspaceId"], json!("w_0"));
        let bare = create(&c).shown_view("ws_2", "b", WorkspaceKind::Local, "", "2026-10-26T00:00:00.000Z", &p, &by, None);
        assert!(serde_json::to_value(&bare).expect("json").get("parentWorkspaceId").is_none());
    });
}

/// Waits for the create area's roads (packet 6).
#[test]
fn create_create_staged_refuses_where_no_machine_can_be_forked() {
    drive(plain(), |c, _| async move {
        let p = project_on("p_1", "api", "/srv/api", "solari");
        let report = create(&c).stage_reporter("ws_1", "api", 1_700_000_000_000, None);
        let opts = wsp_core::types::wiring::CreateWorkspaceOptions { project: "p_1".into(), name: "api".into(), ..Default::default() };
        let free: wsp_account::projects::workspaces::seam::FreeSlot = Rc::new(|| {});
        let made = create(&c).create_staged(opts, &p, "ws_1", report, None, free, None).await;
        assert!(made.is_err());
    });
}

#[test]
fn create_shown_and_failed_creates_are_two_handles_the_area_shares() {
    drive(plain(), |c, _| async move {
        let p = project("p_1", "api", "/srv/api");
        let failed = create(&c).failed_view("ws_1", "api", WorkspaceKind::Cloud, "g", 1_700_000_000_000, &p, "it broke");
        create(&c).failed_creates().borrow_mut().insert("ws_1".into(), failed);
        assert!(create(&c).failed_creates().borrow().contains_key("ws_1"));
        assert!(create(&c).shown_creates().borrow().is_empty());
        assert!(!Rc::ptr_eq(&create(&c).failed_creates(), &create(&c).shown_creates()));
    });
}

/// create.ts:229 `failedView`: the keys in this order, the phase gone and the sentence in `gone`.
#[test]
fn create_failed_view_is_the_gone_view_a_failed_create_leaves() {
    drive(plain(), |c, _| async move {
        let p = project("p_1", "api", "/srv/api");
        let view = create(&c).failed_view("ws_1", "api", WorkspaceKind::Cloud, "g_1", 1_700_000_000_000, &p, "it broke");
        let json = serde_json::to_value(&view).expect("json");
        let keys: Vec<&str> = json.as_object().expect("an object").keys().map(String::as_str).collect();
        assert_eq!(keys, ["id", "name", "machineId", "phase", "kind", "golden", "createdAt", "project", "gone"]);
        assert_eq!(json["phase"], json!("gone"));
        assert_eq!(json["gone"], json!("it broke"));
        assert_eq!(json["createdAt"], json!("2023-11-14T22:13:20.000Z"));
    });
}

#[test]
fn create_boot_work_waits_for_the_boot_work_of_that_workspace_and_returns_at_once_for_none() {
    drive(plain(), |c, _| async move {
        create(&c).boot_work("w_none").await;
        let done = Rc::new(Cell::new(false));
        let flag = done.clone();
        let work: wsp_serve::runtime::ctx::Joined = async move {
            tokio::task::yield_now().await;
            flag.set(true);
        }
        .boxed_local()
        .shared();
        BootRuntime::boot_work(&*c).borrow_mut().insert("w_1".into(), work);
        create(&c).boot_work("w_1").await;
        assert!(done.get());
    });
}

#[test]
fn create_held_is_every_live_entry_in_the_order_held_and_listed_for_a_person_is_the_same() {
    drive(plain(), |c, _| async move {
        held_entry(&c, json!({ "id": "w_2", "name": "two" })).await;
        held_entry(&c, json!({ "id": "w_1", "name": "one" })).await;
        let ids = |rows: Vec<Entry>| rows.iter().map(|e| e.borrow().record.borrow().id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(create(&c).held()), ["w_2", "w_1"]);
        assert_eq!(ids(create(&c).listed_for(None).expect("a listing")), ["w_2", "w_1"]);
    });
}

#[test]
fn create_talks_to_tree_on_is_false_for_a_caller_that_is_no_thread() {
    drive(plain(), |c, _| async move {
        held_entry(&c, json!({})).await;
        assert!(!create(&c).talks_to_tree_on("w_1", None).expect("read"));
    });
}

#[test]
fn create_project_of_scope_is_the_project_of_the_workspace_a_thread_runs_in() {
    drive(plain(), |c, _| async move {
        held_entry(&c, json!({ "project": "p_1" })).await;
        let by = caller_by(scope("t_1", "w_1", "t_1"));
        assert_eq!(create(&c).project_of_scope(Some(&by)), Some("p_1".to_string()));
        assert_eq!(create(&c).project_of_scope(None), None);
    });
}

#[test]
fn create_the_rules_read_nothing_to_refuse_for_a_person_at_this_computer() {
    drive(plain(), |c, _| async move {
        let p = project("p_1", "api", "/srv/api");
        let r = record(json!({}));
        assert_eq!(create(&c).refusal_for(Some(&r), None).expect("a read"), None);
        assert!(!create(&c).of_threads_repository(None, "p_1").expect("a read"));
        assert!(create(&c).elsewhere_refusal(None, &p, "api").expect("a read").is_none());
        assert!(create(&c).away_for(&r, None, "work", "api").expect("a read").is_none());
        assert!(!create(&c).project_reached(None, "p_1").expect("a read"));
        assert!(create(&c).create_stages().is_empty());
        assert!(create(&c).find_places("zingzys-mac").await.expect("a read").is_empty());
    });
}

#[test]
fn the_workspaces_context_is_the_four_halves_together() {
    fn whole<T: wsp_account::projects::workspaces::WorkspacesCtx>(_: &T) {}
    drive(plain(), |c, _| async move {
        whole(&*c);
    });
}

// ----------------------------------------------------------------------------------------------------------------
// Ported from the TS whole-runtime tests (packages/runtime/test at 65f6b89c7): folder-threads, start-review,
// bring-back, and the list and get ops the `workspaces.list` and `workspaces.get` tapes read. Each goes through the
// public `workspaces()` and `projects()` members, so each waits on the packets whose bodies those ops reach: boot (5,
// merged) and its reads (4, 7, 11, 12), projects (10), create (6), phases (9).

/// The project of a repo on this computer, held as `projects.add` leaves it, so the ops here read no later packet's add.
fn held_repo(c: &Rc<Ctx>, folder: &Path) -> ProjectView {
    let mut held = project("p_1", "api", &folder.to_string_lossy());
    held.git = Some(serde_json::from_value(json!({ "top": folder })).expect("a git fact"));
    base(c).projects_held().borrow_mut().insert("p_1".into(), held.clone());
    held
}

/// folder-threads.test.ts:"runs in the project folder, two at once, on one record and with no copy made", up to the
/// turn: two asks for the folder of one project, by id and by name, answer one record in the project's own folder.
#[test]
fn op_folder_for_answers_one_record_in_the_project_folder_for_two_asks_at_once() {
    let root = scratch("folder-for");
    let folder = repo(root.path());
    let (opts, log) = copier_opts(root.path());
    drive(opts, |c, runtime| async move {
        let project = held_repo(&c, &folder);
        let ask = |word: &str| wsp_core::types::api::FolderForAsk { project: Some(word.to_string()), ..Default::default() };
        let (a, b) = futures::join!(
            runtime.workspaces().folder_for(ask(&project.id), None),
            runtime.workspaces().folder_for(ask(&project.name), None),
        );
        let (a, b) = (a.expect("a folder"), b.expect("a folder"));
        assert_eq!(a.workspace.id, b.workspace.id);
        assert_eq!(a.workspace.folder.as_deref(), Some(folder.to_string_lossy().as_ref()));
        assert!(a.workspace.worktree.is_none());
        let log = log.lock().unwrap();
        assert!(log.asks.is_empty());
        assert!(log.worktrees.is_empty());
    });
}

/// folder-threads.test.ts:"is what a create on a project here answers, whatever name it is given, twice over".
#[test]
fn op_create_on_a_project_here_answers_the_folders_record_whatever_name_it_is_given_twice_over() {
    let root = scratch("create-here");
    let folder = repo(root.path());
    let (opts, log) = copier_opts(root.path());
    drive(opts, |c, runtime| async move {
        let project = held_repo(&c, &folder);
        let ask =
            || wsp_core::types::wiring::CreateWorkspaceOptions { project: project.id.clone(), name: "hello".into(), ..Default::default() };
        let one = runtime.workspaces().create(ask(), None).await.expect("a record");
        let two = runtime.workspaces().create(ask(), None).await.expect("a record");
        assert_eq!(two.view.id, one.view.id);
        assert_eq!(one.view.folder.as_deref(), Some(folder.to_string_lossy().as_ref()));
        assert!(log.lock().unwrap().asks.is_empty());
    });
}

/// bring-back.test.ts:"a create naming a parent this host does not hold is refused, not landed as a root".
#[test]
fn op_create_naming_a_parent_this_host_does_not_hold_is_refused_not_landed_as_a_root() {
    let root = scratch("no-parent");
    let folder = repo(root.path());
    let (opts, _log) = copier_opts(root.path());
    drive(opts, |c, runtime| async move {
        let project = held_repo(&c, &folder);
        let ask = wsp_core::types::wiring::CreateWorkspaceOptions {
            project: project.id.clone(),
            name: "child".into(),
            parent: Some("ws_gone".into()),
            ..Default::default()
        };
        let said = runtime.workspaces().create(ask, None).await.refused("a parent that is not held");
        assert!(!said.message.is_empty());
    });
}

/// start-review.test.ts:"refuses a link no project here is a checkout of, in one sentence naming the repository".
#[test]
fn op_start_refuses_a_link_no_project_here_is_a_checkout_of_and_a_word_that_is_no_link() {
    let root = scratch("start-refuses");
    let folder = repo(root.path());
    let (opts, _log) = copier_opts(root.path());
    drive(opts, |c, runtime| async move {
        held_repo(&c, &folder);
        let link = wsp_core::types::api::StartFromLinkAsk { url: "https://github.com/someone/else/issues/1".into(), ..Default::default() };
        let said = runtime.workspaces().start(link, None).await.refused("no such repository").message;
        assert_eq!(said, wsp_words::start::start_words::no_project_for_repo("someone/else"));
        let word = wsp_core::types::api::StartFromLinkAsk { url: "fix the flaky test".into(), ..Default::default() };
        let said = runtime.workspaces().start(word, None).await.refused("no link").message;
        assert_eq!(said, wsp_words::start::start_words::not_a_link("fix the flaky test"));
    });
}

/// The `workspaces.list` tape: a person's list is every held workspace as its view, in the order held, and a fork a
/// thread asked for that has no record yet stands in it once.
#[test]
fn op_list_answers_the_held_workspaces_in_order_and_a_shown_fork_after_them() {
    drive(plain(), |c, runtime| async move {
        base(&c).projects_held().borrow_mut().insert("p_1".into(), project("p_1", "api", "/srv/api"));
        held_entry(&c, json!({ "id": "w_2", "name": "two" })).await;
        held_entry(&c, json!({ "id": "w_1", "name": "one" })).await;
        let p = project("p_1", "api", "/srv/api");
        let shown = create(&c).shown_view(
            "ws_3",
            "fork",
            WorkspaceKind::Cloud,
            "g",
            "2026-10-26T00:00:00.000Z",
            &p,
            &scope("t_1", "w_1", "t_0"),
            None,
        );
        create(&c).show_create(shown);
        let listed: Vec<WorkspaceView> = runtime.workspaces().list(None).await.expect("a list");
        let ids: Vec<&str> = listed.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["w_2", "w_1", "ws_3"]);
    });
}

/// The `workspaces.get` tape: the view of a held workspace, and the refusal for one that is not held.
#[test]
fn op_get_answers_the_view_of_a_held_workspace_and_refuses_one_that_is_not() {
    drive(plain(), |c, runtime| async move {
        base(&c).projects_held().borrow_mut().insert("p_1".into(), project("p_1", "api", "/srv/api"));
        held_entry(&c, json!({})).await;
        let view = runtime.workspaces().get("w_1", None, None).await.expect("a view");
        assert_eq!(view.id, "w_1");
        assert_eq!(view.project.id, "p_1");
        let said = runtime.workspaces().get("w_zz", None, None).await.refused("not held").message;
        assert!(said.contains("w_zz"), "{said}");
    });
}

/// The `worktree.*` tapes: a worktree made for a branch is a record with its own folder, one the copier made.
#[test]
fn op_worktree_makes_the_record_of_a_branch_through_the_copier() {
    let root = scratch("worktree-op");
    let folder = repo(root.path());
    let (opts, log) = copier_opts(root.path());
    drive(opts, |c, runtime| async move {
        let project = held_repo(&c, &folder);
        let made = runtime.workspaces().worktree(&project.id, "feat/one", None).await.expect("a worktree");
        assert_eq!(made.branch, "feat/one");
        assert!(made.made);
        assert_eq!(log.lock().unwrap().worktrees.len(), 1);
    });
}
