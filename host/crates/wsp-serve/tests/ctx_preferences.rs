// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 16 (#2476): the context's bodies for `PreferencesCtx` (35 methods, preferences.ts:13-156),
//! `ServerIcons` (2), `ActivityCtx` and `DaemonAsk` (27, activity.ts:97-424), `SlatesAreaCtx` (49, account/slates.ts)
//! and `PipeAreaContext` (10, pipe.ts:402-466), read against the TS at 65f6b89c7 (runtime.ts:428, :435, :486, :488),
//! and `Ctx::build_agents_read`, which is `agentsReads({ .. })` of preferences.ts:99-151.
//!
//! The TS tests the plan names (preferences, older-agent-lists, slates, slate-mcp, slate-here-http, places-agents)
//! build a whole runtime, run `serveRuntime` over a socket or a stub `claude` binary and drive projects, folders and
//! turns. A Rust test cannot replace an area, and the roads those tests drive belong to packets 8 to 13 and 17, so
//! the preferences record is read through `Runtime::preferences()` as the TS test reads it, and every other method is
//! read at its own seam on the runtime `create_runtime` builds over `FakeBackend` and the sqlite store in a temp folder.
//!
//! Not ported, because the host ports they wire are not Rust yet (ruling 3, the host ports are `wsp-setup`'s, and the
//! markers in wsp-core's `types/pending.rs` are empty traits, so a reader or an icon store cannot be handed in):
//! preferences.test.ts "agent version checks follow the person's switch" and "icons turned off whose folder cannot be
//! deleted still save ...". Not ported, because they need a stub `claude` binary and packets 8, 10 and 17:
//! older-agent-lists.test.ts (four tests), places-agents.test.ts and the slate tests that start a turn.
//!
//! A test that goes through a method of another packet's trait is red until that packet lands: the packet is named
//! in the test's last doc line, `waits on packet N`.
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_account::account::core_types::AccountError;
use wsp_account::account::preferences::PreferencesCtx as P;
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, DaemonChannelOptions, Frame};
use wsp_core::pipe::area::PipeAreaContext as Pipe;
use wsp_core::types::api::Runtime as _;
use wsp_core::types::events::EventListener;
use wsp_core::types::internal::{SessionEntry, ThreadRecord};
use wsp_core::types::wiring::{LiveWorkspace, Local, LocalWiring, Platform, RuntimeOptions, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_machines::machines::boot::BootRuntime;
use wsp_machines::machines::machines::{Entry, Row};
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_slates::account::slates::SlatesAreaCtx as S;
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::{StateWriter, Store};
use wsp_threads::activity::ActivityCtx as A;
use wsp_wire::types::agents_report::{AgentsTarget, AgentsTargetV0, AgentsTargetV1};
use wsp_wire::views::preferences::default_preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures.

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

/// The sqlite store a host opens, in a folder of its own.
fn sqlite(dir: &std::path::Path) -> Rc<dyn Store> {
    let writer = StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() };
    Rc::new(wsp_store::sqlite_store::sqlite_store(&dir.join("state.json"), writer))
}

/// The environment these runtimes read labs off: this file's, not the shell that started the run.
fn no_labs() -> Option<IndexMap<String, Option<String>>> {
    Some(IndexMap::new())
}

fn options(store: Rc<dyn Store>) -> RuntimeOptions {
    RuntimeOptions { backend: Some(backend()), store: Some(store), env: no_labs(), ..Default::default() }
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

/// A runtime over a fresh sqlite store.
fn run_fresh<Fut: Future<Output = ()>>(f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>) -> Fut) {
    let dir = tempfile::tempdir().expect("a folder");
    run(options(sqlite(dir.path())), f);
}

fn run_with<Fut: Future<Output = ()>>(adjust: impl FnOnce(&mut RuntimeOptions), f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>) -> Fut) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut opts = options(sqlite(dir.path()));
    adjust(&mut opts);
    run(opts, f);
}

fn record(id: &str, extra: Value) -> WorkspaceRecord {
    let mut base = json!({
        "id": id, "name": "pricing page", "machineId": "m_1", "phase": "running", "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z",
        "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false, "kind": "local",
    });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a workspace record")
}

fn project(id: &str, name: &str, path: &str, computer: &str) -> wsp_wire::views::workspace::ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "path": path, "computer": computer, "source": { "kind": "folder", "path": path }, "remote": "",
        "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a project")
}

fn hold_project(c: &Rc<Ctx>, id: &str, name: &str, path: &str, computer: &str) {
    BootRuntime::projects_held(&**c).borrow_mut().insert(id.to_string(), project(id, name, path, computer));
}

fn view(id: &str, workspace: &str, extra: Value) -> SessionView {
    let mut base = json!({ "id": id, "workspaceId": workspace, "harness": "claude", "status": "completed" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a session view")
}

fn session_entry(view: SessionView) -> SessionEntry {
    SessionEntry {
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
    }
}

fn put_row(c: &Rc<Ctx>, key: &str, view: SessionView) -> Row {
    let row: Row = Rc::new(RefCell::new(session_entry(view)));
    BootRuntime::sessions(&**c).borrow_mut().insert(key.to_string(), row.clone());
    row
}

fn put_thread(c: &Rc<Ctx>, id: &str, workspace: &str, extra: Value) {
    let mut base = json!({ "workspaceId": workspace, "harness": "claude" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    let record: ThreadRecord = serde_json::from_value(base).expect("a thread record");
    BootRuntime::thread_records(&**c).borrow_mut().insert(id.to_string(), record);
}

/// A live entry over `record` on a fake machine, put in the runtime's live map through the boot area's handle.
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

fn listen(runtime: &AssembledRuntime, kind: &str) -> Rc<RefCell<Vec<Value>>> {
    let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
    let sink = seen.clone();
    let listener: EventListener = Rc::new(move |e: &EventUnion| {
        sink.borrow_mut().push(serde_json::to_value(e).expect("an event"));
        Ok(())
    });
    std::mem::forget(runtime.events().on(kind, listener));
    seen
}

fn emit(c: &Rc<Ctx>, event: Value) {
    let event = wsp_wire::wire::events_union::parse_event(&event).expect("an event");
    BootRuntime::emit(&**c, event).expect("emit");
}

fn patch(value: Value) -> wsp_wire::views::preferences::PreferencesPatch {
    serde_json::from_value(value).expect("a preferences patch")
}

fn prefs(p: &wsp_wire::views::preferences::Preferences) -> Value {
    serde_json::to_value(p).expect("preferences are JSON")
}

/// A channel that answers each frame with itself and `ok: true`, keeps what it was sent, and never ends.
struct Echo {
    closed: Cell<bool>,
    sent: RefCell<Vec<Frame>>,
}

impl Echo {
    fn new() -> Rc<Echo> {
        Rc::new(Echo { closed: Cell::new(false), sent: RefCell::default() })
    }
}

#[async_trait(?Send)]
impl DaemonChannel for Echo {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        self.sent.borrow_mut().push(frame.clone());
        let mut reply = frame;
        reply.insert("ok".to_string(), json!(true));
        Ok(reply)
    }
    fn close(&self) {
        self.closed.set(true);
    }
    async fn closed(&self) -> ChannelEnd {
        futures::future::pending::<ChannelEnd>().await
    }
}

/// An opener that hands out `channel` and keeps the url and token it was asked to dial.
fn opener(
    channel: Rc<Echo>,
    dialled: Rc<RefCell<Vec<(String, String)>>>,
) -> Rc<dyn Fn(DaemonChannelOptions) -> Local<'static, Rc<dyn DaemonChannel>>> {
    Rc::new(move |o| {
        dialled.borrow_mut().push((o.url.clone(), o.token.clone()));
        let channel: Rc<dyn DaemonChannel> = channel.clone();
        Box::pin(async move { Ok(channel) })
    })
}

/// This computer's own wiring, its daemon road the one given.
fn local_wiring(url: Option<&str>) -> LocalWiring {
    LocalWiring {
        backend: backend(),
        folder: "/tmp/wsp-local".into(),
        exec_stream: Rc::new(|_, _| unimplemented!("no turn runs in these tests")),
        home: Rc::new(|_| "/home/me/.claude".to_string()),
        home_dir: "/home/me".into(),
        roots_path: "/home/me/.wsp/roots".into(),
        env: Rc::new(|| IndexMap::from([("PATH".to_string(), "/usr/bin".to_string())])),
        daemon_road: url.map(|url| {
            let url = url.to_string();
            let held: Rc<dyn Fn() -> Local<'static, wsp_wire::views::workspace::DaemonReachView>> = Rc::new(move || {
                let url = url.clone();
                Box::pin(async move {
                    Ok(wsp_wire::views::workspace::DaemonReachView {
                        url,
                        expires_at: wsp_wire::types::js_number::JsNumber(9.0e15),
                        daemon_token: Some("tok-7".into()),
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

// ---------------------------------------------------------------------------------------------------------------
// preferences.test.ts: the record over `Runtime::preferences()`.

#[test]
fn preferences_read_as_the_defaults_take_a_patch_keep_it_for_the_next_runtime_and_push_the_record() {
    let dir = tempfile::tempdir().expect("a folder");
    let store = sqlite(dir.path());
    run(options(store.clone()), |rt, _| async move {
        let changed = listen(&rt, "preferences.changed");
        assert_eq!(prefs(&rt.preferences().get().await.expect("get")), prefs(&default_preferences()));
        let set = rt
            .preferences()
            .set(patch(json!({ "theme": "light", "sidebarWidth": 312, "terminalZoom": { "ws_a": 2 } })))
            .await
            .expect("set");
        let mut expected = prefs(&default_preferences());
        expected["theme"] = json!("light");
        expected["sidebarWidth"] = json!(312);
        expected["terminalZoom"] = json!({ "ws_a": 2 });
        assert_eq!(prefs(&set.preferences), expected);
        assert_eq!(set.notice, None);
        assert_eq!(changed.borrow().len(), 1, "every socket hears the record once");
        assert_eq!(changed.borrow()[0]["preferences"], expected);

        // A second patch lands on the first, a null width clears the width alone, and a zoom entry lands beside the others.
        let second = rt
            .preferences()
            .set(patch(json!({ "sidebarMode": "spaces", "sidebarWidth": null, "terminalZoom": { "ws_b": -1 } })))
            .await
            .expect("set");
        let second = prefs(&second.preferences);
        assert_eq!((&second["theme"], &second["sidebarMode"], &second["terminalSize"]), (&json!("light"), &json!("spaces"), &json!("app")));
        assert_eq!(second["terminalZoom"], json!({ "ws_a": 2, "ws_b": -1 }));
        let third = rt.preferences().set(patch(json!({ "terminalZoom": { "ws_b": null } }))).await.expect("set");
        assert_eq!(prefs(&third.preferences)["terminalZoom"], json!({ "ws_a": 2 }));
        assert!(prefs(&rt.preferences().get().await.expect("get")).get("sidebarWidth").is_none());
    });
    run(options(store), |rt, _| async move {
        let read = prefs(&rt.preferences().get().await.expect("get"));
        assert_eq!(
            (&read["theme"], &read["sidebarMode"], &read["terminalZoom"]),
            (&json!("light"), &json!("spaces"), &json!({ "ws_a": 2 }))
        );
    });
}

#[test]
fn a_dismissed_release_and_the_usage_switch_land_on_the_record_for_the_next_runtime_on_the_store() {
    let dir = tempfile::tempdir().expect("a folder");
    let store = sqlite(dir.path());
    run(options(store.clone()), |rt, _| async move {
        let changed = listen(&rt, "preferences.changed");
        assert_eq!(prefs(&rt.preferences().get().await.expect("get"))["productUsage"], json!(true));
        let set = rt.preferences().set(patch(json!({ "updateDismissed": "0.3.3", "productUsage": false }))).await.expect("set");
        assert_eq!(prefs(&set.preferences)["updateDismissed"], json!("0.3.3"));
        assert_eq!(prefs(&set.preferences)["productUsage"], json!(false));
        assert_eq!(changed.borrow()[0]["preferences"]["updateDismissed"], json!("0.3.3"));
    });
    run(options(store), |rt, _| async move {
        let read = prefs(&rt.preferences().get().await.expect("get"));
        assert_eq!((&read["updateDismissed"], &read["productUsage"]), (&json!("0.3.3"), &json!(false)));
    });
}

#[test]
fn a_change_to_the_defaults_that_cannot_stand_is_refused_as_usage_and_the_record_stands() {
    run_fresh(|rt, c| async move {
        let changed = listen(&rt, "preferences.changed");
        let refused =
            rt.preferences().set(patch(json!({ "defaultAgent": "nope" }))).await.expect_err("an agent this host runs no thread of");
        assert_eq!(refused.kind, Some(wsp_words::exit::Kind::Usage));
        let held = rt
            .preferences()
            .set(patch(json!({ "projectDefaults": { "p_9": { "agent": "claude" } } })))
            .await
            .expect_err("a project this host does not hold");
        assert_eq!(held.kind, Some(wsp_words::exit::Kind::Usage));
        assert!(changed.borrow().is_empty(), "a refused patch is not kept and is not told");
        assert_eq!(prefs(&rt.preferences().get().await.expect("get")), prefs(&default_preferences()));
        assert!(!P::project_held(&*c, "p_9"));
    });
}

#[test]
fn the_access_picked_in_a_workspace_and_a_computers_icon_land_on_the_record() {
    let dir = tempfile::tempdir().expect("a folder");
    let store = sqlite(dir.path());
    run(options(store.clone()), |rt, _| async move {
        let one = rt.preferences().set(patch(json!({ "access": { "ws_a": "bypassPermissions" } }))).await.expect("set");
        assert_eq!(prefs(&one.preferences)["access"], json!({ "ws_a": "bypassPermissions" }));
        let two = rt.preferences().set(patch(json!({ "access": { "ws_b": "acceptEdits" } }))).await.expect("set");
        assert_eq!(prefs(&two.preferences)["access"], json!({ "ws_a": "bypassPermissions", "ws_b": "acceptEdits" }));
        let three = rt.preferences().set(patch(json!({ "access": { "ws_a": null } }))).await.expect("set");
        assert_eq!(prefs(&three.preferences)["access"], json!({ "ws_b": "acceptEdits" }));
        let looks = rt
            .preferences()
            .set(patch(json!({ "computerLook": { "pl_box": { "icon": "server" }, "pl_mac": { "icon": "laptop" } } })))
            .await
            .expect("set");
        assert_eq!(prefs(&looks.preferences)["computerLook"], json!({ "pl_box": { "icon": "server" }, "pl_mac": { "icon": "laptop" } }));
        let cleared = rt.preferences().set(patch(json!({ "computerLook": { "pl_mac": null } }))).await.expect("set");
        assert_eq!(prefs(&cleared.preferences)["computerLook"], json!({ "pl_box": { "icon": "server" } }));
    });
    run(options(store), |rt, _| async move {
        let read = prefs(&rt.preferences().get().await.expect("get"));
        assert_eq!(read["access"], json!({ "ws_b": "acceptEdits" }));
        assert_eq!(read["computerLook"], json!({ "pl_box": { "icon": "server" } }));
    });
}

#[test]
fn a_record_kept_before_computers_had_icons_reads_with_none() {
    let dir = tempfile::tempdir().expect("a folder");
    let store = sqlite(dir.path());
    run(options(store.clone()), |rt, _| async move {
        store
            .put("preferences", "default", json!({ "theme": "light", "projectLook": { "pr_1": { "icon": "rocket" } } }))
            .await
            .expect("put");
        let mut expected = prefs(&default_preferences());
        expected["theme"] = json!("light");
        expected["projectLook"] = json!({ "pr_1": { "icon": "rocket" } });
        expected["computerLook"] = json!({});
        assert_eq!(prefs(&rt.preferences().get().await.expect("get")), expected);
    });
}

#[test]
fn two_patches_landing_at_once_keep_both_fields() {
    run_fresh(|rt, _| async move {
        let (a, b) = futures::future::join(
            rt.preferences().set(patch(json!({ "theme": "light" }))),
            rt.preferences().set(patch(json!({ "sidebarMode": "spaces" }))),
        )
        .await;
        let (a, b) = (prefs(&a.expect("a").preferences), prefs(&b.expect("b").preferences));
        assert_eq!(a["theme"], json!("light"));
        assert_eq!((&b["theme"], &b["sidebarMode"]), (&json!("light"), &json!("spaces")));
        let read = prefs(&rt.preferences().get().await.expect("get"));
        assert_eq!((&read["theme"], &read["sidebarMode"]), (&json!("light"), &json!("spaces")));
    });
}

#[test]
fn a_runtime_reads_labs_off_its_own_environment_once_and_stamps_it_on_every_read() {
    run_with(
        |o| o.env = Some(IndexMap::from([("WSP_LABS".to_string(), Some("1".to_string()))])),
        |rt, _| async move {
            assert_eq!(prefs(&rt.preferences().get().await.expect("get"))["labs"], json!(true));
            let set = rt.preferences().set(patch(json!({ "theme": "dark" }))).await.expect("set");
            assert_eq!(prefs(&set.preferences)["labs"], json!(true));
        },
    );
    run_fresh(|rt, _| async move {
        assert_eq!(prefs(&rt.preferences().get().await.expect("get"))["labs"], json!(false));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// PreferencesCtx and ServerIcons: each member of the context the preferences area reads (preferences.ts:13-156).

#[test]
fn the_environment_is_the_options_own_else_the_process() {
    run_fresh(|_, c| async move {
        assert!(P::env(&*c).is_empty(), "an environment handed in is the whole of it, a None value is absent");
    });
    run_with(
        |o| o.env = Some(IndexMap::from([("A".to_string(), Some("1".to_string())), ("B".to_string(), None)])),
        |_, c| async move { assert_eq!(P::env(&*c), IndexMap::from([("A".to_string(), "1".to_string())])) },
    );
    let dir = tempfile::tempdir().expect("a folder");
    run(RuntimeOptions { env: None, ..options(sqlite(dir.path())) }, |_, c| async move {
        assert_eq!(P::env(&*c).get("PATH"), std::env::var("PATH").ok().as_ref());
    });
}

#[test]
fn the_home_folder_is_the_processs_and_the_adapters_are_listed_in_the_order_given() {
    run_with(
        |o| {
            for id in ["codex", "claude"] {
                o.adapters.insert(id.to_string(), Arc::new(|_| Err(HostError::new("not built here"))));
            }
        },
        |_, c| async move {
            assert_eq!(P::home_dir(&*c), std::env::var("HOME").expect("HOME is set"));
            assert_eq!(P::adapter_ids(&*c), vec!["codex".to_string(), "claude".to_string()]);
        },
    );
    run_fresh(|_, c| async move { assert!(P::adapter_ids(&*c).is_empty()) });
}

#[test]
fn an_access_table_is_the_harnesss_own_or_a_label_with_no_modes() {
    run_fresh(|_, c| async move {
        let claude = P::access_table(&*c, "claude");
        let catalog = wsp_core::harness_catalog::harness_catalog("claude").expect("claude is in the table");
        assert_eq!(claude.label, catalog.label);
        assert_eq!(claude.table, serde_json::to_value(&catalog).expect("a catalog is JSON"));
        let none = P::access_table(&*c, "nope");
        assert_eq!(none.label, "nope");
        assert_eq!(none.table, json!({ "label": "nope", "permissionModes": [] }));
    });
}

#[test]
fn a_project_is_held_by_id_and_its_refusal_names_the_ones_held() {
    run_fresh(|_, c| async move {
        assert!(!P::project_held(&*c, "p_1"));
        let none = P::project_of(&*c, "p_1").expect_err("not held");
        hold_project(&c, "p_1", "web", "/work/web", "zingzys-mac");
        hold_project(&c, "p_2", "api", "/work/api", "zingzys-mac");
        assert!(P::project_held(&*c, "p_1"));
        assert_eq!(P::project_of(&*c, "p_2").expect("held"), ("p_2".to_string(), "api".to_string()));
        assert_eq!(
            P::project_of(&*c, "p_9").expect_err("not held").message,
            wsp_words::projects::no_such_project_line("p_9", &["web".to_string(), "api".to_string()])
        );
        assert!(none.message.contains("p_1"));
    });
}

#[test]
fn the_default_agent_is_the_first_the_host_runs_a_thread_of_else_the_catalogs_own() {
    run_fresh(|_, c| async move {
        let held = default_preferences();
        assert_eq!(P::default_agent_of(&*c, &held), wsp_catalog::catalog::default_agent().id.clone());
    });
    run_with(
        |o| {
            o.adapters.insert("codex".to_string(), Arc::new(|_| Err(HostError::new("not built here"))));
        },
        |_, c| async move { assert_eq!(P::default_agent_of(&*c, &default_preferences()), "codex") },
    );
}

#[test]
fn the_thread_keys_are_each_rows_thread_else_its_own_id_and_a_record_makes_a_thread_held() {
    run_fresh(|_, c| async move {
        put_row(&c, "turn-1", view("s_1", "w_1", json!({ "threadId": "t_a" })));
        put_row(&c, "turn-2", view("s_2", "w_1", json!({})));
        assert_eq!(P::session_thread_keys(&*c), vec!["t_a".to_string(), "s_2".to_string()]);
        assert!(!P::thread_record_held(&*c, "t_a"));
        put_thread(&c, "t_a", "w_1", json!({}));
        assert!(P::thread_record_held(&*c, "t_a"));
    });
}

#[test]
fn the_record_is_stored_held_and_told_through_the_stores_and_the_buss_own_members() {
    run_fresh(|rt, c| async move {
        let changed = listen(&rt, "preferences.changed");
        assert_eq!(P::stored(&*c).await.expect("stored"), None);
        assert!(P::held(&*c).is_none());
        let mut next = default_preferences();
        next.sidebar_width = Some(250.into());
        P::store(&*c, &next).await.expect("store");
        assert_eq!(P::stored(&*c).await.expect("stored").expect("a record")["sidebarWidth"], json!(250));
        P::set_held(&*c, next.clone());
        assert_eq!(P::held(&*c), Some(next.clone()));
        P::ready(&*c).await;
        P::emit(&*c, json!({ "type": "preferences.changed", "preferences": prefs(&next) }));
        assert_eq!(changed.borrow().len(), 1);
        assert_eq!(changed.borrow()[0]["preferences"]["sidebarWidth"], json!(250));
    });
}

fn report(workspace: &str) -> wsp_wire::types::agents_report::AgentsReport {
    serde_json::from_value(json!({
        "target": { "workspaceId": workspace }, "home": "/h", "user": "u", "agents": [], "skills": [], "servers": [],
        "refused": [], "readAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a report")
}

#[test]
fn a_deleted_workspace_is_told_to_the_preferences_area_which_forgets_its_read() {
    run_fresh(|_, c| async move {
        let reads = c.preferences_seams().agents_read;
        reads.last.borrow_mut().insert("w_1".to_string(), report("w_1"));
        reads.last.borrow_mut().insert("w_2".to_string(), report("w_2"));
        emit(&c, json!({ "type": "workspace.deleted", "workspaceId": "w_1" }));
        assert!(!reads.last.borrow().contains_key("w_1"), "the workspace.deleted listener forgets its report");
        assert!(reads.last.borrow().contains_key("w_2"));
        P::forget_read(&*c, "w_2");
        assert!(!reads.last.borrow().contains_key("w_2"));
    });
}

#[test]
fn a_workspaces_facts_for_the_agents_read_are_read_off_its_live_entry() {
    run_fresh(|_, c| async move {
        hold_project(&c, "p_1", "web", "/work/web", "zingzys-mac");
        let entry = put_live(&c, record("w_1", json!({}))).await;
        let got = P::entry_of(&*c, "w_1", None).await.expect("a live entry");
        assert!(Rc::ptr_eq(&got, &entry), "the one live entry, not a copy");
        assert!(P::entry_of(&*c, "w_9", None).await.is_err());
        assert!(P::is_local(&*c, &entry));
        assert!(!P::at_provider(&*c, &entry), "a local workspace stands on no provider");
        assert_eq!(P::checkout_of(&*c, &entry).expect("a checkout"), "/work/web");
        assert_eq!(P::pause_mode(&*c, &entry), None);
        let cloud = put_live(&c, record("w_2", json!({ "kind": "cloud", "place": "pl_box" }))).await;
        assert!(!P::is_local(&*c, &cloud));
        assert!(!P::at_provider(&*c, &cloud), "a joined computer is not a provider");
        assert_eq!(P::checkout_of(&*c, &cloud).expect("a checkout"), "/work/web");
        let lost = put_live(&c, record("w_3", json!({ "project": "p_9" }))).await;
        assert!(P::checkout_of(&*c, &lost).is_err(), "the TS throws where the project is gone");
    });
}

#[test]
fn the_catalog_agents_carry_their_state_home_and_plugin_seed_variables() {
    run_fresh(|_, c| async move {
        let agents = P::catalog_agents(&*c);
        let table = wsp_catalog::catalog::catalog_agents();
        assert_eq!(agents.iter().map(|a| a.id.clone()).collect::<Vec<_>>(), table.iter().map(|a| a.id.clone()).collect::<Vec<_>>());
        for (keys, agent) in agents.iter().zip(&table) {
            assert_eq!(keys.state_home_env, agent.state_home_env);
            assert_eq!(keys.plugin_seed_env, agent.plugins.and_then(|p| p.seed()).map(|seed| seed.env().to_string()));
        }
    });
}

#[test]
fn a_thread_env_is_the_agents_own_for_the_workspace_and_the_projects_on_a_place_are_its_checkouts() {
    run_fresh(|_, c| async move {
        hold_project(&c, "p_1", "web", "/work/web", "pl_box");
        let mut other = project("p_2", "api", "/work/api", "pl_box");
        other.checkout = Some("/srv/api".into());
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_2".into(), other);
        hold_project(&c, "p_3", "docs", "/work/docs", "pl_other");
        assert_eq!(
            P::projects_on(&*c, "pl_box"),
            vec![
                ("p_1".to_string(), "web".to_string(), "/work/web".to_string()),
                ("p_2".to_string(), "api".to_string(), "/srv/api".to_string())
            ]
        );
        let entry = put_live(&c, record("w_1", json!({ "kind": "cloud", "place": "pl_box" }))).await;
        // waits on packet 11: `AgentsCtx::kind_env`, which the agents area's `thread_env` reads.
        let env = P::thread_env(&*c, &entry, "claude");
        assert_eq!(env, c.preferences_seams().agents.thread_env(&entry, Some("claude")));
    });
}

#[test]
fn this_computers_channel_dials_the_local_road_and_a_place_with_no_link_has_none() {
    let dialled: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let seen = dialled.clone();
    let echo = Echo::new();
    run_with(
        |o| {
            o.local = Some(local_wiring(Some("ws://127.0.0.1:7")));
            o.daemon_channel = Some(opener(echo.clone(), dialled.clone()));
        },
        |_, c| async move {
            // waits on packet 2: `KindsArea::local_road`.
            P::local_channel(&*c, Rc::new(|_| {})).await.expect("a channel");
            assert_eq!(*seen.borrow(), vec![("ws://127.0.0.1:7".to_string(), "tok-7".to_string())]);
        },
    );
    run_fresh(|_, c| async move {
        assert!(P::place_channel(&*c, "pl_box", Rc::new(|_| {})).is_none(), "no door, no link");
        assert_eq!(P::place_name(&*c, "pl_box"), None);
    });
    run_with(
        |o| o.place_links = Some(Rc::new(wiring())),
        |_, c| async move {
            assert_eq!(P::place_name(&*c, wsp_wire::types::place_word::HERE_PLACE_ID).as_deref(), Some("zingzys-mac"));
            assert!(P::place_channel(&*c, "pl_unknown", Rc::new(|_| {})).is_none(), "a place the door has no link for");
        },
    );
}

#[test]
fn a_setup_is_written_and_read_back_as_names_only_and_a_refusal_it_moved_is_dropped() {
    run_with(
        |o| {
            o.adapters.insert("claude".to_string(), Arc::new(|_| Err(HostError::new("not built here"))));
        },
        |_, c| async move {
            let view = P::setup_view(&*c, "pl_box", "claude");
            assert_eq!(view, json!({ "on": true, "envNames": [] }));
            P::setup_write(&*c, "pl_box", "claude", json!({ "on": false, "args": ["--x"], "env": { "ZED": "1", "ALPHA": "2" } }))
                .await
                .expect("written");
            assert_eq!(P::setup_view(&*c, "pl_box", "claude"), json!({ "on": false, "args": ["--x"], "envNames": ["ALPHA", "ZED"] }));
            assert!(!P::drop_setup_refusal(&*c, "pl_box", "claude"), "no refusal stands");
            c.preferences_seams()
                .agents
                .setup_refusals
                .borrow_mut()
                .insert(wsp_core::agent_setup::key_of("pl_box", "claude"), "no good".to_string());
            assert!(P::drop_setup_refusal(&*c, "pl_box", "claude"));
            assert!(!P::drop_setup_refusal(&*c, "pl_box", "claude"), "dropped once");
            P::setup_refusal_moved(&*c, "pl_box", "claude");
        },
    );
}

#[test]
fn the_backends_callback_relay_is_what_the_agents_read_calls_relayed() {
    run_fresh(|_, c| async move {
        assert!(!P::relayed(&*c), "the fake backend relays no callback");
    });
}

#[test]
fn no_server_icons_are_held_where_the_host_wired_none() {
    run_fresh(|_, c| async move {
        assert!(P::server_icons(&*c).is_none());
        assert_eq!(
            wsp_account::account::preferences::ServerIcons::folder(&*c),
            "",
            "a stand-in answers plain where the TS has no icons to read"
        );
        assert!(wsp_account::account::preferences::ServerIcons::forget(&*c).is_ok());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// `Ctx::build_agents_read` (preferences.ts:99-151): the options the reader is built over.

fn here_target() -> AgentsTarget {
    AgentsTarget::V0(AgentsTargetV0 { place_id: "here".into(), project: None })
}

#[test]
fn a_read_with_no_reader_wired_refuses_in_the_reader_missing_sentence() {
    run_fresh(|_, c| async move {
        let reads = c.preferences_seams().agents_read;
        assert!(reads.o.reader.is_none() && reads.o.acts.is_none() && reads.o.skills.is_none());
        assert!(reads.o.servers.is_none() && reads.o.plugins.is_none() && reads.o.icons.is_none());
        let refused = reads.read(&here_target(), None).await.expect_err("no reader");
        assert_eq!(refused.message, wsp_machines::agents_read::NO_AGENTS_READER);
    });
}

#[test]
fn the_reader_tells_the_bus_what_changed_and_follows_the_persons_switch_and_the_backend() {
    run_fresh(|rt, c| async move {
        let changed = listen(&rt, "agents.changed");
        let reads = c.preferences_seams().agents_read;
        (reads.o.changed)(None);
        (reads.o.changed)(Some(AgentsTarget::V1(AgentsTargetV1 { workspace_id: "w_1".into() })));
        let seen: Vec<Value> = changed.borrow().clone();
        assert_eq!(seen.len(), 2);
        assert!(seen[0].get("target").is_none());
        assert_eq!(seen[1]["target"], json!({ "workspaceId": "w_1" }));
        assert!((reads.o.latest_on.as_ref().expect("a latest_on"))().await, "version checks are on by default");
        rt.preferences().set(patch(json!({ "agentVersions": false }))).await.expect("set");
        assert!(!(reads.o.latest_on.as_ref().expect("a latest_on"))().await, "read at every ask");
        assert!(!(reads.o.relayed.as_ref().expect("relayed"))());
    });
}

#[test]
fn the_reader_reads_a_setup_only_for_an_agent_this_host_runs_and_refuses_a_write_for_any_other() {
    run_with(
        |o| {
            o.adapters.insert("claude".to_string(), Arc::new(|_| Err(HostError::new("not built here"))));
        },
        |_, c| async move {
            let reads = c.preferences_seams().agents_read;
            let of = reads.o.setup_of.as_ref().expect("setup_of");
            assert!(of("pl_box", "nope").is_none());
            assert!(!of("pl_box", "claude").expect("a view").env_names.len() > 0);
            let write = reads.o.setup_write.as_ref().expect("setup_write");
            let refused = write("pl_box".into(), "nope".into(), serde_json::from_value(json!({ "on": false })).expect("a setup change"))
                .await
                .expect_err("no adapter");
            assert_eq!(refused.kind, Some(wsp_words::exit::Kind::Usage));
        },
    );
}

#[test]
fn the_projects_on_a_computer_wait_for_the_load_and_list_checkout_before_path() {
    run_fresh(|_, c| async move {
        hold_project(&c, "p_1", "web", "/work/web", "pl_box");
        let mut other = project("p_2", "api", "/work/api", "pl_box");
        other.checkout = Some("/srv/api".into());
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_2".into(), other);
        let reads = c.preferences_seams().agents_read;
        let listed = (reads.o.projects.as_ref().expect("projects"))("pl_box".into()).await.expect("projects");
        assert_eq!(
            listed.iter().map(|p| (p.id.as_str(), p.path.as_str())).collect::<Vec<_>>(),
            vec![("p_1", "/work/web"), ("p_2", "/srv/api")]
        );
    });
}

#[test]
fn a_channel_for_this_computer_dials_the_local_road_and_one_for_a_missing_computer_says_so() {
    let dialled: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let seen = dialled.clone();
    let echo = Echo::new();
    run_with(
        |o| {
            o.local = Some(local_wiring(Some("ws://127.0.0.1:7")));
            o.daemon_channel = Some(opener(echo.clone(), dialled.clone()));
        },
        |_, c| async move {
            let reads = c.preferences_seams().agents_read;
            // waits on packet 2: `KindsArea::local_road`.
            (reads.o.channel)(here_target(), Rc::new(|_| {}), None).await.map(|_| ()).expect("a channel");
            assert_eq!(*seen.borrow(), vec![("ws://127.0.0.1:7".to_string(), "tok-7".to_string())]);
            let lost =
                (reads.o.channel)(AgentsTarget::V0(AgentsTargetV0 { place_id: "pl_gone".into(), project: None }), Rc::new(|_| {}), None)
                    .await
                    .map(|_| ())
                    .expect_err("no link");
            assert_eq!(lost.message, wsp_words::workspace_state::absent_computer("pl_gone", None).sentence);
        },
    );
}

#[test]
fn the_places_the_reader_asks_are_the_door_only_where_the_host_wired_one() {
    run_fresh(|_, c| async move {
        assert!(((c.preferences_seams().agents_read.o.places)()).is_none());
    });
    run_with(
        |o| o.place_links = Some(Rc::new(wiring())),
        |_, c| async move {
            let places = ((c.preferences_seams().agents_read.o.places)()).expect("the door");
            assert!(places.sign_ins_at("pl_box", &["claude".to_string()]).is_none());
        },
    );
}

#[test]
fn a_workspaces_facts_for_the_reader_are_its_name_phase_machine_project_and_checkout() {
    run_fresh(|_, c| async move {
        hold_project(&c, "p_1", "web", "/work/web", "zingzys-mac");
        let entry = put_live(&c, record("w_1", json!({}))).await;
        let reads = c.preferences_seams().agents_read;
        let facts = (reads.o.workspace)("w_1".into(), None).await.expect("facts");
        assert_eq!((facts.name.as_str(), facts.local), ("pricing page", true));
        assert_eq!(facts.phase, wsp_wire::views::workspace::WorkspacePhase::Running);
        assert!(Arc::ptr_eq(&facts.machine, &entry.borrow().machine));
        assert_eq!((facts.project.id.as_str(), facts.project.name.as_str(), facts.project.path.as_str()), ("p_1", "web", "/work/web"));
        assert_eq!(
            (facts.stores, facts.seeds, facts.brokered),
            (Some(IndexMap::new()), Some(IndexMap::new()), None),
            "a workspace here keeps no store of its own"
        );
        assert!((reads.o.workspace)("w_9".into(), None).await.is_err());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ActivityCtx and DaemonAsk (activity.ts:97-424, runtime.ts:428).

fn places_of(rt: &AssembledRuntime) -> Rc<dyn wsp_machines::places::types::PlaceDoor> {
    rt.places().expect("a runtime wired with place links has a door")
}

#[test]
fn the_activity_door_reads_the_place_door_only_where_the_host_wired_one() {
    run_fresh(|_, c| async move {
        assert!(!A::has_door(&*c));
        assert_eq!(A::place_name(&*c, "pl_box"), None);
        assert!(A::place_rows(&*c).await.expect("no door reads no rows").is_empty());
        assert!(!A::joined(&*c, "pl_box"));
        assert!(!A::has_link(&*c, "pl_box"));
        assert!(A::box_channel(&*c, "pl_box", Rc::new(|_| {})).is_none());
    });
    run_with(
        |o| o.place_links = Some(Rc::new(wiring())),
        |rt, c| async move {
            assert!(A::has_door(&*c));
            assert_eq!(A::place_name(&*c, wsp_wire::types::place_word::HERE_PLACE_ID).as_deref(), Some("zingzys-mac"));
            let door = places_of(&rt);
            let rows = A::place_rows(&*c).await.expect("rows");
            assert_eq!(rows, door.rows().await.expect("the door's own rows"));
            assert!(!A::joined(&*c, "pl_unknown"));
            assert!(!A::has_link(&*c, "pl_unknown"));
            assert!(A::box_channel(&*c, "pl_unknown", Rc::new(|_| {})).is_none(), "the door holds no link to it");
        },
    );
}

#[test]
fn the_activity_door_reads_the_clock_the_pid_the_start_and_the_runtimes_own_maps() {
    run_with(
        |o| o.clock = Some(wsp_core::clock::FakeClock::new(5_000).clock()),
        |_, c| async move {
            assert_eq!(A::clock(&*c).now(), 5_000);
            assert_eq!(A::host_pid(&*c), std::process::id());
            A::ready(&*c).await.expect("the one start every road waits on");
            assert!(Rc::ptr_eq(&A::sessions(&*c), &BootRuntime::sessions(&*c)));
            assert!(Rc::ptr_eq(&A::records(&*c), &BootRuntime::thread_records(&*c)));
            assert!(A::live_entry(&*c, "w_1").is_none());
            let entry = put_live(&c, record("w_1", json!({}))).await;
            assert!(Rc::ptr_eq(&A::live_entry(&*c, "w_1").expect("live"), &entry));
        },
    );
}

#[test]
fn a_local_daemon_is_held_where_the_options_name_a_road_and_its_channel_asks_and_closes() {
    run_fresh(|_, c| async move {
        assert!(!A::has_local_daemon(&*c));
        let refused = A::local_channel(&*c, Rc::new(|_| {}), false).await.map(|_| ()).expect_err("no daemon road here");
        assert!(!refused.message.is_empty());
    });
    let dialled: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let seen = dialled.clone();
    let echo = Echo::new();
    let held = echo.clone();
    run_with(
        |o| {
            o.local = Some(local_wiring(Some("ws://127.0.0.1:7")));
            o.daemon_channel = Some(opener(echo, dialled.clone()));
        },
        |_, c| async move {
            assert!(A::has_local_daemon(&*c));
            // waits on packet 2: `KindsArea::local_road`.
            for guarded in [false, true] {
                let ask = A::local_channel(&*c, Rc::new(|_| {}), guarded).await.map(|_| ()).map_err(|e| e.message);
                assert!(ask.is_ok(), "{ask:?}");
            }
            assert_eq!(seen.borrow().len(), 2);
            assert_eq!(seen.borrow()[0], ("ws://127.0.0.1:7".to_string(), "tok-7".to_string()));
            let ask = A::local_channel(&*c, Rc::new(|_| {}), false).await.map_err(|e| e.message).expect("a channel");
            let reply = ask.ask(json!({ "op": "ports.watch" })).await.expect("a reply");
            assert_eq!(reply["op"], json!("ports.watch"));
            assert_eq!(reply["ok"], json!(true));
            assert!(!held.closed.get());
            ask.close();
            assert!(held.closed.get(), "close ends the channel it asked over");
        },
    );
}

#[test]
fn a_daemon_ask_over_a_refusing_channel_fails_in_the_daemons_own_words() {
    struct Refusing;
    #[async_trait(?Send)]
    impl DaemonChannel for Refusing {
        async fn send(&self, _frame: Frame) -> Result<Frame, ChannelError> {
            let mut reply = Frame::new();
            reply.insert("ok".to_string(), json!(false));
            reply.insert("code".to_string(), json!("not-found"));
            reply.insert("error".to_string(), json!("no such pid"));
            Ok(reply)
        }
        fn close(&self) {}
        async fn closed(&self) -> ChannelEnd {
            futures::future::pending::<ChannelEnd>().await
        }
    }
    run_with(
        |o| {
            o.local = Some(local_wiring(Some("ws://127.0.0.1:7")));
            let refusing: Rc<dyn DaemonChannel> = Rc::new(Refusing);
            o.daemon_channel = Some(Rc::new(move |_| {
                let refusing = refusing.clone();
                Box::pin(async move { Ok(refusing) })
            }));
        },
        |_, c| async move {
            // waits on packet 2: `KindsArea::local_road`.
            let ask = A::local_channel(&*c, Rc::new(|_| {}), false).await.map_err(|e| e.message).expect("a channel");
            let refused: AccountError = ask.ask(json!({ "op": "proc.kill" })).await.expect_err("the daemon refused");
            assert_eq!(refused.message(), "no such pid");
        },
    );
}

#[test]
fn a_thread_is_read_off_the_rows_the_records_and_the_runtimes_own_facts() {
    run_fresh(|_, c| async move {
        assert!(A::latest_on(&*c, "t_a").is_none());
        assert!(A::thread_facts(&*c, "t_a").is_none());
        assert_eq!(A::kept_pid(&*c, "t_a"), None);
        put_row(&c, "turn-1", view("s_1", "w_1", json!({ "threadId": "t_a", "startedAt": 10 })));
        put_row(&c, "turn-2", view("s_2", "w_1", json!({ "threadId": "t_a", "startedAt": 20 })));
        assert_eq!(A::latest_on(&*c, "t_a").expect("a latest row").id, "s_2");
        assert_eq!(A::latest_on(&*c, "t_other").map(|v| v.id), None);
        assert!(A::scopes_here(&*c).is_empty(), "no turn runs here");
        A::reap_kept_now(&*c, "t_a");
    });
}

#[test]
fn what_a_caller_may_drive_is_the_rules_and_the_threads_own() {
    run_fresh(|_, c| async move {
        use wsp_wire::views::workspace_kind::WorkspaceKind;
        assert!(A::drives(&*c, WorkspaceKind::Local, None), "a person's own socket drives every kind");
        let local = record("w_1", json!({}));
        assert!(A::refusal_for(&*c, &local, None).expect("no throw").is_none());
        assert_eq!(A::settles_thread(&*c, "t_a", None), c.preferences_seams().threads.settles_thread("t_a", None));
    });
}

#[test]
fn a_thread_ends_through_its_kinds_module_and_a_daemon_frame_goes_through_its_road() {
    run_fresh(|_, c| async move {
        hold_project(&c, "p_1", "web", "/work/web", "zingzys-mac");
        let entry = put_live(&c, record("w_1", json!({}))).await;
        // waits on packet 2: `KindsArea::module_of` answers the local module, which holds no `end_thread`.
        assert!(A::end_thread(&*c, &entry, "t_a", Default::default()).is_none(), "a local workspace has no end of its own");
        let refused = A::with_daemon(&*c, &entry, json!({ "op": "ports.watch" })).await.expect_err("this machine runs no daemon");
        assert!(!refused.message().is_empty());
    });
}

#[test]
fn a_threads_servers_close_through_the_slates_area() {
    run_fresh(|_, c| async move {
        A::close_servers(&*c, "t_a");
        S::wake(&*c, "w_nowhere").await.expect_err("a workspace this host does not hold");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// PipeAreaContext (pipe.ts:402-466): the reads of the pool's wiring.

#[test]
fn the_pipes_reads_are_the_live_map_the_clock_and_the_one_start() {
    run_fresh(|_, c| async move {
        assert!(Pipe::live_ids(&*c).is_empty());
        assert!(Pipe::live(&*c, "w_1").is_none());
        put_live(&c, record("w_1", json!({ "machineId": "m_1" }))).await;
        put_live(&c, record("w_2", json!({ "machineId": "m_2", "phase": "napping" }))).await;
        assert_eq!(Pipe::live_ids(&*c), vec!["w_1".to_string(), "w_2".to_string()]);
        let one = Pipe::live(&*c, "w_2").expect("live");
        assert_eq!((one.id.as_str(), one.phase.as_str(), one.machine_id.as_str()), ("w_2", "napping", "m_2"));
        Pipe::ready(&*c).await.expect("the one start every road waits on");
        assert!(Pipe::clock(&*c).now() > 0);
    });
}

#[test]
fn a_pipes_route_is_the_workspaces_own_daemon_reach_and_a_missing_one_is_named() {
    run_fresh(|_, c| async move {
        let entry = put_live(&c, record("w_1", json!({}))).await;
        let ws = entry.borrow().ws.clone();
        let want = ws.daemon_reach().await.map(|reach| reach.url).map_err(|e| e.to_string());
        assert_eq!(Pipe::daemon_reach_url(&*c, "w_1").await, want);
        let missing = Pipe::daemon_reach_url(&*c, "w_9").await.expect_err("no such workspace");
        assert!(missing.contains("w_9"), "{missing}");
        assert!(Pipe::daemon_token_of(&*c, "w_9").await.expect_err("no such workspace").contains("w_9"));
        assert!(Pipe::daemon_version(&*c, "w_9").await.expect_err("no such workspace").contains("w_9"));
    });
}

#[test]
fn a_pipes_token_is_the_machines_and_a_forgotten_one_is_dropped_from_the_cache() {
    run_with(
        |o| o.daemon_token = Some("a".repeat(64)),
        |_, c| async move {
            use wsp_threads::threads::transcripts::DaemonAnswered;
            put_live(&c, record("w_1", json!({ "machineId": "m_1" }))).await;
            let transcripts = c.preferences_seams().transcripts;
            transcripts.daemon_tokens.borrow_mut().insert("m_1".to_string(), DaemonAnswered { has_daemon: true, at: 0 });
            let token = Pipe::daemon_token_of(&*c, "w_1").await.expect("a read").expect("the machine runs a daemon");
            assert_eq!(token, transcripts.token_for_machine("m_1"));
            Pipe::forget_daemon_token(&*c, "m_1");
            assert!(transcripts.daemon_tokens.borrow().get("m_1").is_none());
        },
    );
}

#[test]
fn a_pipes_daemon_version_is_what_the_kinds_module_reads() {
    run_with(
        |o| o.local = Some(local_wiring(None)),
        |_, c| async move {
            put_live(&c, record("w_1", json!({}))).await;
            // waits on packet 2: `KindsArea::module_of` and the local module's `daemon_version`.
            let version = Pipe::daemon_version(&*c, "w_1").await;
            assert!(version.is_ok(), "{version:?}");
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// SlatesAreaCtx (account/slates.ts): each member of the context the slate area reads.

#[test]
fn the_slate_areas_plain_reads_are_the_stores_the_clock_the_home_and_the_process() {
    run_with(
        |o| {
            o.state_path = Some("/var/wsp/state.json".into());
            o.clock = Some(wsp_core::clock::FakeClock::new(5_000).clock());
            o.local = Some(local_wiring(None));
            o.adapters.insert("claude".to_string(), Arc::new(|_| Err(HostError::new("not built here"))));
        },
        |_, c| async move {
            assert!(S::store(&*c).keys("things").await.expect("keys").is_empty());
            assert_eq!(S::clock(&*c).now(), 5_000);
            assert_eq!(S::home_dir(&*c), std::env::var("HOME").expect("HOME is set"));
            assert_eq!(S::process_env(&*c).get("HOME"), std::env::var("HOME").ok().as_ref());
            assert_eq!(S::local_env(&*c), Some(IndexMap::from([("PATH".to_string(), "/usr/bin".to_string())])));
            assert_eq!(S::state_folder(&*c).as_deref(), Some("/var/wsp"));
            assert_eq!(S::place_name_here(&*c), wsp_core::host_id::hostname());
            assert_eq!(S::agent_label(&*c, "claude"), "Claude Code");
            assert_eq!(S::agent_label(&*c, "nope"), "nope");
            assert!(S::path_exists(&*c, "/"));
            assert!(!S::path_exists(&*c, "/no/such/folder/anywhere"));
            S::warn(&*c, "a slate warned");
        },
    );
    run_fresh(|_, c| async move {
        assert_eq!(S::local_env(&*c), None);
        assert_eq!(S::state_folder(&*c), None, "no state path, no slates folder");
    });
}

#[test]
fn a_slates_event_is_recorded_for_its_thread_and_a_live_event_goes_to_the_windows_alone() {
    run_fresh(|rt, c| async move {
        let recorded = listen(&rt, "session.slate");
        let run = listen(&rt, "slate.run");
        S::record(
            &*c,
            serde_json::from_value(json!({
                "workspaceId": "w_1", "sessionId": "s_1", "threadId": "t_a", "cause": "write", "version": 1,
                "by": "agent", "pieces": ["p1"],
            }))
            .expect("a slate event"),
        );
        assert_eq!(recorded.borrow().len(), 1);
        assert_eq!(recorded.borrow()[0]["threadId"], json!("t_a"));
        S::emit(
            &*c,
            wsp_slates::slates::SlateEmit::Run(
                serde_json::from_value(json!({ "workspaceId": "w_1", "threadId": "t_a", "run": "r1", "lines": ["hi"] }))
                    .expect("a run event"),
            ),
        );
        assert_eq!(run.borrow().len(), 1);
        assert_eq!(run.borrow()[0]["lines"], json!(["hi"]));
        assert_eq!(recorded.borrow().len(), 1, "a live event is never recorded");
    });
}

#[test]
fn a_thread_is_read_by_its_latest_row_its_record_and_its_running_turn() {
    run_fresh(|_, c| async move {
        assert!(S::latest_on(&*c, "t_a").is_none() && S::thread_record(&*c, "t_a").is_none() && S::running_turn(&*c, "t_a").is_none());
        put_thread(&c, "t_a", "w_1", json!({ "permissionMode": "plan" }));
        put_row(&c, "turn-1", view("s_1", "w_1", json!({ "threadId": "t_a", "startedAt": 10 })));
        let running = put_row(&c, "turn-2", view("s_2", "w_1", json!({ "threadId": "t_a", "startedAt": 20, "status": "running" })));
        assert_eq!(S::latest_on(&*c, "t_a").expect("a row").id, "s_2");
        assert_eq!(S::thread_record(&*c, "t_a").expect("a record").workspace_id, "w_1");
        assert_eq!(S::running_turn(&*c, "t_a"), Some(running.borrow().turn_id.clone()));
        assert_eq!(S::rows_on(&*c, "t_a").iter().map(|v| v.id.clone()).collect::<Vec<_>>(), vec!["s_1".to_string(), "s_2".to_string()]);
        assert!(S::rows_on(&*c, "t_other").is_empty());
        assert_eq!(S::root_of(&*c, "t_a"), "t_a", "a thread with no lead is its own root");
        assert_eq!(S::tree_under(&*c, "t_a"), c.preferences_seams().threads.tree_under("t_a"));
    });
}

#[test]
fn a_tool_asks_what_the_harnesss_table_says_for_the_mode() {
    run_fresh(|_, c| async move {
        use wsp_core::harness_catalog::tool_asks;
        for (harness, mode) in
            [(None, None), (Some("claude"), None), (Some("claude"), Some("bypassPermissions")), (Some("codex"), Some("plan"))]
        {
            assert_eq!(S::tool_asks(&*c, harness, mode), tool_asks(harness, mode), "{harness:?} {mode:?}");
        }
    });
}

#[test]
fn a_workspaces_folders_and_computer_are_the_kinds_and_the_channels_reading() {
    run_with(
        |o| o.local = Some(local_wiring(None)),
        |_, c| async move {
            hold_project(&c, "p_1", "web", "/work/web", "zingzys-mac");
            let local = put_live(&c, record("w_1", json!({}))).await;
            assert!(Rc::ptr_eq(&S::live(&*c, "w_1").expect("live"), &local));
            assert!(S::live(&*c, "w_9").is_none());
            assert_eq!(S::checkout_of(&*c, &local).expect("a checkout"), "/work/web");
            assert_eq!(S::computer_of(&*c, &local), wsp_words::workspace_state::machine_word(&local.borrow().record.borrow().kind));
            assert_eq!(
                S::runs_in(&*c, &local, Some("/work/web/sub"), "/work/web"),
                c.preferences_seams().folders.runs_in(&local, Some("/work/web/sub"), "/work/web")
            );
            // waits on packet 2: `KindsArea::module_of`, whose local module answers the folder.
            assert_eq!(S::kind_folder(&*c, &local), Some("/work/web".to_string()));
            let lost = put_live(&c, record("w_3", json!({ "project": "p_9" }))).await;
            assert!(S::checkout_of(&*c, &lost).is_err(), "the TS throws where the project is gone");
            assert_eq!(S::project_name(&*c, "p_1").expect("held"), "web");
            assert!(S::project_name(&*c, "p_9").is_err());
            assert_eq!(S::folder_of(&*c, "w_1", "no-session"), None);
            assert_eq!(S::transcript(&*c, "w_1"), Vec::<Value>::new());
        },
    );
}

#[test]
fn a_thread_off_this_computer_is_a_box_only_when_its_workspace_is_not_local() {
    run_fresh(|_, c| async move {
        hold_project(&c, "p_1", "web", "/work/web", "pl_box");
        put_live(&c, record("w_1", json!({}))).await;
        put_live(&c, record("w_2", json!({ "kind": "cloud", "place": "pl_box" }))).await;
        put_thread(&c, "t_here", "w_1", json!({}));
        put_thread(&c, "t_away", "w_2", json!({}));
        assert!(S::box_of(&*c, "t_here").is_none());
        let away = S::box_of(&*c, "t_away").expect("a box thread");
        assert_eq!(away.borrow().record.borrow().id, "w_2");
        assert!(S::box_of(&*c, "t_unknown").is_none());
        assert!(S::place_of(&*c, "t_away").is_none(), "a fake machine is not a joined computer's folder");
    });
}

#[test]
fn a_places_standing_is_the_places_own_list_and_the_backend_prices_a_size() {
    run_fresh(|_, c| async move {
        assert!(S::at_provider(&*c, "default"), "the one place the runtime wires over its backend");
        assert!(!S::at_provider(&*c, "pl_box"));
        let sized = put_live(&c, record("w_1", json!({ "kind": "cloud" }))).await;
        let rate = S::entry_rate_usd_per_hour(&*c, &sized).expect("a size is priced");
        let size = sized.borrow().record.borrow().size.clone();
        assert_eq!(rate, backend().pricing().rate_usd_per_hour(wsp_engine::machine::Size { cpu: size.cpu.0, mem_mb: size.mem_mb.0 }));
        assert_eq!(S::accrued(&*c, "w_1"), None);
        c.preferences_seams().transcripts.accrued.borrow_mut().insert("w_1".to_string(), 1.5);
        assert_eq!(S::accrued(&*c, "w_1"), Some(1.5));
    });
}

#[test]
fn the_slate_area_waits_on_the_load_and_a_workspaces_own_boot_work() {
    run_fresh(|_, c| async move {
        S::ready(&*c).await.expect("the one start every road waits on");
        S::boot_work(&*c, "w_1").await;
        assert!(!S::settled_now(&*c, "t_a", None), "None is never");
        let key = S::settle_after(&*c).await;
        let kept = serde_json::to_value(default_preferences().settle_after).expect("a word");
        assert_eq!(Value::String(key), kept);
        assert_eq!(
            S::thread_of_token(&*c, "no-such-token").map_err(|e| e.message),
            c.preferences_seams().threads.thread_of_token("no-such-token").map_err(|e| e.message)
        );
    });
}

#[test]
fn a_press_in_the_queue_is_heard_by_its_request_id_until_the_listener_is_dropped() {
    run_fresh(|_, c| async move {
        let heard = Rc::new(Cell::new(0));
        let count = heard.clone();
        let off = S::on_queued(&*c, "req_1", Box::new(move || count.set(count.get() + 1)));
        let queued = |request: &str| json!({ "type": "session.queued", "workspaceId": "w_1", "threadId": "t_a", "harness": "claude", "prompt": "go", "requestId": request });
        emit(&c, queued("req_2"));
        assert_eq!(heard.get(), 0, "another request's queue is not this one's");
        emit(&c, queued("req_1"));
        assert_eq!(heard.get(), 1);
        off();
        emit(&c, queued("req_1"));
        assert_eq!(heard.get(), 1, "the answer stops listening");
    });
}

#[test]
fn a_napped_workspace_is_told_to_the_slate_area_by_its_id() {
    run_fresh(|_, c| async move {
        let napped: Rc<RefCell<Vec<String>>> = Rc::default();
        let held = napped.clone();
        S::on_napped(&*c, Box::new(move |id| held.borrow_mut().push(id.to_string())));
        emit(&c, json!({ "type": "workspace.napped", "workspaceId": "w_1" }));
        assert_eq!(*napped.borrow(), vec!["w_1".to_string()]);
    });
}

#[test]
fn a_boxs_daemon_reads_fail_in_the_words_the_machine_gives_and_the_servers_wait_on_the_host_ports() {
    run_fresh(|_, c| async move {
        let box_entry = put_live(&c, record("w_2", json!({ "kind": "cloud", "place": "pl_box" }))).await;
        assert!(S::daemon_image(&*c, &box_entry, "/x.png".into()).await.is_err());
        assert!(S::daemon_hash(&*c, &box_entry, "/".into(), vec!["a".into()]).await.is_err());
        assert!(S::stdio_road(&*c, &box_entry, None).await.map(|_| ()).is_err());
        let target = AgentsTarget::V1(AgentsTargetV1 { workspace_id: "w_2".into() });
        let said = S::agents_server(&*c, target, "claude", "wsp").await.map(|_| ()).expect_err("no reader wired");
        assert_eq!(said, wsp_slates::account::slates::ServerReadError::Other(wsp_machines::agents_read::NO_AGENTS_READER.to_string()));
        assert!(S::agents_reader_server(&*c, vec![], "claude", "wsp").is_none(), "this host reads no agent's MCP config");
    });
}

#[test]
fn a_watch_on_a_workspaces_pull_request_and_a_process_a_thread_owns_go_to_their_areas() {
    run_fresh(|_, c| async move {
        hold_project(&c, "p_1", "web", "/work/web", "zingzys-mac");
        let entry = put_live(&c, record("w_1", json!({}))).await;
        S::poll_pull_request(&*c, &entry);
        let release = S::own_process(&*c, "t_a", 4242, wsp_frames::ProcKind::Agent);
        release();
    });
}

#[test]
fn a_press_starts_a_turn_through_the_sessions_door_and_a_threads_account_is_the_turns_own() {
    run_fresh(|_, c| async move {
        // waits on packet 13: `SessionsApi::start`.
        let refused = S::start_session(
            &*c,
            wsp_slates::slates::Delivery {
                thread_id: "t_a".into(),
                workspace_id: "w_9".into(),
                prompt: "go".into(),
                request_id: "req_1".into(),
            },
        )
        .await
        .expect_err("a workspace this host does not hold");
        assert!(!refused.message.is_empty());
        let entry = put_live(&c, record("w_1", json!({}))).await;
        assert_eq!(S::usage_account_key(&*c, &entry, "claude"), c.preferences_seams().threads.usage_account_of(&entry, "claude", None).0);
    });
}

#[test]
fn the_accounts_a_slates_source_reads_are_the_usage_pages_and_none_before_a_turn_ran() {
    run_fresh(|_, c| async move {
        // waits on packet 15: `UsageArea::usage_accounts`.
        assert!(S::usage_accounts(&*c).await.is_empty());
    });
}
