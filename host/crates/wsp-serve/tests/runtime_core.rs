// SPDX-License-Identifier: AGPL-3.0-only
//! What `runtimeCore` itself decides: spec/runtime_core.md. `CoreCtx` is faked over the script with a clock the test
//! moves; each test asserts the calls in order and the state the rule protects.
mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use support::{block, FakeBus, FakeDevices, Script};
use wsp_core::clock::{Cancel, Clock, Timer};
use wsp_core::devices::{DeviceDoor, PairedDevice};
use wsp_core::types::events::EventBus;
use wsp_engine::machine::MachineState;
use wsp_serve::runtime::core::{
    core_defaults, reads_state, took_the_resume, vault_paths_of, Core, CoreCtx, CoreDefaults, DefaultsAsked, FolderPick, LaunchReach, NamedCount, ProjectLook,
    StageAt, StandingEntry, StandingRows,
};
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

// ---- A clock the test moves ----

type Timers = Rc<RefCell<Vec<(u64, i64, Option<Timer>)>>>;

#[derive(Default)]
struct FakeClock {
    now: Cell<i64>,
    timers: Timers,
    next: Cell<u64>,
    /// Every delay asked, in order.
    asked: RefCell<Vec<i64>>,
}

impl FakeClock {
    fn at(now: i64) -> Rc<FakeClock> {
        let c = FakeClock::default();
        c.now.set(now);
        Rc::new(c)
    }
    /// Moves the clock and fires every timer due, oldest due first.
    fn advance(&self, ms: i64) {
        let target = self.now.get() + ms;
        loop {
            let due = {
                let mut timers = self.timers.borrow_mut();
                timers.sort_by_key(|(id, at, _)| (*at, *id));
                match timers.iter().position(|(_, at, f)| *at <= target && f.is_some()) {
                    Some(i) => {
                        let (_, at, f) = timers.remove(i);
                        Some((at, f.unwrap()))
                    }
                    None => None,
                }
            };
            match due {
                Some((at, f)) => {
                    self.now.set(at.max(self.now.get()));
                    f();
                }
                None => break,
            }
        }
        self.now.set(target);
    }
    fn armed(&self) -> usize {
        self.timers.borrow().iter().filter(|(_, _, f)| f.is_some()).count()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> i64 {
        self.now.get()
    }
    fn schedule(&self, f: Timer, ms: i64) -> Cancel {
        let id = self.next.get() + 1;
        self.next.set(id);
        self.asked.borrow_mut().push(ms);
        self.timers.borrow_mut().push((id, self.now.get() + ms, Some(f)));
        let timers = self.timers.clone();
        Rc::new(move || timers.borrow_mut().retain(|(i, _, _)| *i != id))
    }
}

// ---- The context ----

struct Ctx {
    script: Rc<Script>,
    bus: Rc<FakeBus>,
    clock: Rc<FakeClock>,
    devices: Rc<FakeDevices>,
    host_key: Option<String>,
    reach: Option<LaunchReach>,
    brokered: bool,
    live: RefCell<Vec<StandingEntry>>,
    projects: RefCell<Vec<ProjectView>>,
    threads: Vec<(String, String)>,
    sessions: Vec<SessionView>,
    snoozed: RefCell<IndexMap<String, f64>>,
    resumes: RefCell<IndexMap<String, f64>>,
    fails: RefCell<Vec<String>>,
}

impl Ctx {
    fn new() -> Rc<Ctx> {
        let script = Script::new();
        Rc::new(Ctx {
            bus: FakeBus::new(script.clone()),
            devices: FakeDevices::new(script.clone()),
            clock: FakeClock::at(1_000_000),
            host_key: None,
            reach: None,
            brokered: false,
            live: RefCell::new(vec![]),
            projects: RefCell::new(vec![]),
            threads: vec![],
            sessions: vec![],
            snoozed: RefCell::new(IndexMap::new()),
            resumes: RefCell::new(IndexMap::new()),
            fails: RefCell::new(vec![]),
            script,
        })
    }
    fn failing(&self, name: &str) -> Result<(), HostError> {
        if self.fails.borrow().iter().any(|f| f == name) {
            return Err(HostError::new(format!("{name} refused")));
        }
        Ok(())
    }
}

#[async_trait(?Send)]
impl CoreCtx for Ctx {
    fn bus(&self) -> Rc<dyn EventBus> {
        self.bus.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn devices(&self) -> Rc<dyn DeviceDoor> {
        self.devices.clone()
    }
    fn host_key(&self) -> Option<String> {
        self.host_key.clone()
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.script.record("ready", json!({}));
        Ok(())
    }
    fn agents_reach(&self, workspace_id: &str) -> Option<LaunchReach> {
        self.script.record("agentsReach", json!({ "id": workspace_id }));
        self.reach.clone()
    }
    fn brokered(&self, _workspace_id: &str) -> bool {
        self.brokered
    }
    fn record_name(&self, workspace_id: &str) -> String {
        format!("name-of-{workspace_id}")
    }
    async fn reread_public(&self, workspace_id: &str) -> Result<(), HostError> {
        self.script.record("rereadPublic", json!({ "id": workspace_id }));
        self.failing("rereadPublic")
    }
    async fn read_broker(&self) -> Result<(), HostError> {
        self.script.record("readBroker", json!({}));
        Ok(())
    }
    async fn daemon_settled(&self, workspace_id: &str) -> Result<(), HostError> {
        self.script.record("daemonSettled", json!({ "id": workspace_id }));
        Ok(())
    }
    async fn read_public_marketplaces(&self) -> Result<(), HostError> {
        self.script.record("readPublicMarketplaces", json!({}));
        Ok(())
    }
    fn turn_road(&self, kind: WorkspaceKind) -> WorkspaceOrigin {
        if kind == WorkspaceKind::Local {
            WorkspaceOrigin::Here
        } else {
            WorkspaceOrigin::Relayed
        }
    }
    fn live(&self) -> Vec<StandingEntry> {
        self.live.borrow().clone()
    }
    fn projects_held(&self) -> Vec<ProjectView> {
        self.projects.borrow().clone()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.threads.clone()
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.sessions.clone()
    }
    fn cap_held(&self, _turn_id: &str) -> bool {
        false
    }
    fn snoozed_until(&self, thread_id: &str) -> Option<f64> {
        self.snoozed.borrow().get(thread_id).copied()
    }
    fn limit_resume_at(&self, thread_id: &str) -> Option<f64> {
        self.resumes.borrow().get(thread_id).copied()
    }
    fn thread_workspace(&self, thread_id: &str) -> Option<String> {
        Some(format!("ws-of-{thread_id}"))
    }
    async fn resume_after_limit(&self, thread_id: &str) -> Result<(), HostError> {
        self.script.record("resumeAfterLimit", json!({ "threadId": thread_id }));
        self.failing("resumeAfterLimit")
    }
    async fn wake(&self, workspace_id: &str) -> Result<(), HostError> {
        self.script.record("wake", json!({ "id": workspace_id }));
        Ok(())
    }
    async fn exec(&self, workspace_id: &str, cmd: &str, timeout_ms: u64) -> Result<ExecResult, HostError> {
        self.script.call("exec", json!({ "id": workspace_id, "cmd": cmd, "timeoutMs": timeout_ms }))
    }
    async fn delete_workspace(&self, workspace_id: &str) -> Result<(), HostError> {
        self.script.record("deleteWorkspace", json!({ "id": workspace_id }));
        self.failing(&format!("delete {workspace_id}"))
    }
    async fn unsaved_fork(&self, workspace_id: &str, name: &str) -> Result<Option<String>, HostError> {
        self.script.record("unsavedFork", json!({ "id": workspace_id, "name": name }));
        Ok(Some(format!("{name} has unsaved work")))
    }
    async fn unsaved_project(&self, project: &ProjectView) -> Result<Option<String>, HostError> {
        self.script.record("unsavedProject", json!({ "id": project.id }));
        Ok(Some(format!("{} has unsaved work", project.name)))
    }
    async fn remove_project(&self, project_id: &str, force: bool, keep_look: bool) -> Result<(), HostError> {
        self.script.record("removeProject", json!({ "id": project_id, "force": force, "keepLook": keep_look }));
        self.failing(&format!("remove {project_id}"))
    }
    fn end_sessions(&self, workspace_id: &str, reason: &str) {
        self.script.record("endSessions", json!({ "id": workspace_id, "reason": reason }));
    }
    async fn drop_record(&self, workspace_id: &str) -> Result<(), HostError> {
        self.script.record("dropRecord", json!({ "id": workspace_id }));
        Ok(())
    }
    async fn store_delete_project(&self, project_id: &str) -> Result<(), HostError> {
        self.script.record("storeDeleteProject", json!({ "id": project_id }));
        self.projects.borrow_mut().retain(|p| p.id != project_id);
        Ok(())
    }
    async fn forget_icon(&self, project_id: &str) -> Result<(), HostError> {
        self.script.record("forgetIcon", json!({ "id": project_id }));
        Ok(())
    }
    async fn project_look(&self, project_id: &str) -> Result<Option<ProjectLook>, HostError> {
        let call = self.script.record("projectLook", json!({ "id": project_id }));
        Ok(self.script.answer::<Option<ProjectLook>>(&call).flatten())
    }
    async fn set_project_look(&self, project_id: &str, look: Option<ProjectLook>) -> Result<(), HostError> {
        self.script.record("setProjectLook", json!({ "id": project_id, "look": look }));
        Ok(())
    }
    async fn set_icon(&self, project_id: &str, image: Option<String>) -> Result<(), HostError> {
        self.script.record("setIcon", json!({ "id": project_id, "image": image }));
        Ok(())
    }
    fn keep_current(&self, place_id: &str) {
        self.script.record("keepCurrent", json!({ "place": place_id }));
    }
    fn place_name(&self, place_id: &str) -> String {
        format!("{place_id}-name")
    }
    async fn held_away_on(&self, place_id: &str) -> Result<Vec<String>, HostError> {
        let call = self.script.record("heldAwayOn", json!({ "place": place_id }));
        self.script.answer::<Result<Vec<String>, HostError>>(&call).unwrap_or(Ok(vec![]))
    }
    async fn hydrate(&self, workspace_id: &str) -> Result<bool, HostError> {
        let call = self.script.record("hydrate", json!({ "id": workspace_id }));
        Ok(self.script.answer::<bool>(&call).unwrap_or(true))
    }
    fn sync_daemon(&self, workspace_id: &str) {
        self.script.record("syncDaemon", json!({ "id": workspace_id }));
    }
    async fn reread_held(&self, workspace_id: &str, why: &str) -> Result<(), HostError> {
        self.script.record("rereadHeld", json!({ "id": workspace_id, "why": why }));
        Ok(())
    }
    fn held_on(&self, place_id: &str) -> Vec<String> {
        let call = self.script.record("heldOn", json!({ "place": place_id }));
        self.script.answer::<Vec<String>>(&call).unwrap_or_default()
    }
    async fn poll(&self, workspace_ids: Vec<String>) -> Result<(), HostError> {
        self.script.record("poll", json!({ "ids": workspace_ids }));
        Ok(())
    }
    fn place_away(&self, place_id: &str) -> bool {
        place_id == "laptop-asleep"
    }
}

fn project(id: &str, name: &str, computer: &str) -> ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "computer": computer, "source": { "kind": "folder", "path": format!("/src/{name}") }, "path": format!("/src/{name}"),
        "remote": "", "defaultBranch": "main", "memoryKey": "k", "memoryDir": "d", "createdAt": "2026-10-01T00:00:00.000Z"
    }))
        .unwrap_or_else(|e| panic!("build a ProjectView: {e}"))
}

fn standing(id: &str, kind: WorkspaceKind, place: &str, project: &str) -> StandingEntry {
    StandingEntry { id: id.into(), name: format!("name-{id}"), kind, place: Some(place.into()), project: project.into(), running: true, machine_id: format!("m-{id}"), sub: false }
}

fn session(workspace: &str, thread: &str, title: &str) -> SessionView {
    serde_json::from_value(json!({ "id": format!("s-{thread}"), "workspaceId": workspace, "harness": "claude", "status": "completed", "threadId": thread, "title": title }))
        .unwrap_or_else(|e| panic!("build a SessionView: {e}"))
}

fn marked(workspace: &str, thread: &str) -> Value {
    json!({ "type": "thread.marked", "workspaceId": workspace, "threadIds": [thread] })
}

/// Every bus event the core emitted, as JSON.
fn emitted(ctx: &Rc<Ctx>) -> Rc<RefCell<Vec<Value>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let keep = seen.clone();
    let _ = ctx.bus.on("*", Rc::new(move |e| {
        keep.borrow_mut().push(serde_json::to_value(e).unwrap());
        Ok(())
    }));
    seen
}

// ---- Defaults ----

#[test]
fn the_defaults_are_the_wiring_constants() {
    // C2. runtime.ts:498-515
    assert_eq!(
        core_defaults(&DefaultsAsked::default(), "the-mac"),
        CoreDefaults {
            provider_read_ms: 30_000,
            gone_confirm_ms: 5_000,
            late_read_ms: 180_000,
            daemon_hello_timeout_ms: 5_000,
            vault_cap_bytes: 200 * 1024 * 1024,
            default_idle_window_ms: 1_200_000,
            host_id: "the-mac".into(),
        }
    );
    let asked = DefaultsAsked { provider_read_ms: Some(1), gone_confirm_ms: Some(2), late_read_ms: Some(3), daemon_hello_timeout_ms: Some(4), vault_cap_bytes: Some(5), idle_default_window_ms: Some(6), host_id: Some("named".into()) };
    assert_eq!(
        core_defaults(&asked, "the-mac"),
        CoreDefaults { provider_read_ms: 1, gone_confirm_ms: 2, late_read_ms: 3, daemon_hello_timeout_ms: 4, vault_cap_bytes: 5, default_idle_window_ms: 6, host_id: "named".into() }
    );
}

#[test]
fn took_the_resume_only_running_or_starting() {
    // C3. runtime.ts:509
    assert!(took_the_resume(Some(MachineState::Running)));
    assert!(took_the_resume(Some(MachineState::Starting)));
    assert!(!took_the_resume(Some(MachineState::Paused)));
    assert!(!took_the_resume(Some(MachineState::Gone)));
    assert!(!took_the_resume(None));
}

// ---- A machine for the reads ----

struct Guest {
    script: Rc<Script>,
    state_hangs: bool,
}

#[async_trait(?Send)]
impl wsp_engine::machine::Machine for Guest {
    fn id(&self) -> &str {
        "m-guest"
    }
    fn kind(&self) -> wsp_engine::machine::MachineKind {
        serde_json::from_value(json!("sandbox")).unwrap()
    }
    async fn exec(&self, cmd: &str, _opts: wsp_engine::machine::ExecOptions) -> wsp_engine::errors::Result<ExecResult> {
        let call = self.script.record("machine.exec", json!({ "cmd": cmd }));
        Ok(self.script.answer::<ExecResult>(&call).unwrap_or(ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() }))
    }
    async fn run(&self, _s: &str, _o: wsp_engine::machine::RunOptions) -> wsp_engine::errors::Result<ExecResult> {
        panic!("not read")
    }
    async fn snapshot(&self, _n: &str, _l: wsp_engine::machine::MachineLife, _o: wsp_engine::machine::SnapshotOptions) -> wsp_engine::errors::Result<String> {
        panic!("not read")
    }
    async fn pause(&self, _w: Option<u64>) -> wsp_engine::errors::Result<()> {
        panic!("not read")
    }
    async fn resume(&self, _s: Option<&wsp_engine::Cancel>) -> wsp_engine::errors::Result<()> {
        panic!("not read")
    }
    async fn kill(&self) -> wsp_engine::errors::Result<()> {
        panic!("not read")
    }
    async fn state(&self) -> wsp_engine::errors::Result<MachineState> {
        self.script.record("machine.state", json!({}));
        if self.state_hangs {
            futures::future::pending::<()>().await;
        }
        let call = self.script.record("machine.stateAnswer", json!({}));
        self.script.answer::<wsp_engine::errors::Result<MachineState>>(&call).unwrap_or(Ok(MachineState::Running))
    }
    async fn download_url(&self, _p: &str) -> wsp_engine::errors::Result<String> {
        panic!("not read")
    }
    async fn upload_url(&self, _p: &str) -> wsp_engine::errors::Result<String> {
        panic!("not read")
    }
}

#[test]
fn reads_state_answers_the_providers_state() {
    // C3. runtime.ts:504-505
    block(async {
        let clock: Rc<dyn Clock> = FakeClock::at(0);
        let guest = Guest { script: Script::new(), state_hangs: false };
        assert_eq!(reads_state(&guest, &clock, 30_000).await, Some(MachineState::Running));
        guest.script.once::<wsp_engine::errors::Result<MachineState>>("machine.stateAnswer", Err(wsp_engine::errors::EngineError::Other { message: "boom".into(), code: None }));
        assert_eq!(reads_state(&guest, &clock, 30_000).await, None, "a read that failed is none");
    })
}

#[test]
fn reads_state_is_none_past_the_providers_read() {
    // C3. runtime.ts:505 (until, bounded on the runtime's clock)
    block(async {
        let clock = FakeClock::at(0);
        let dyn_clock: Rc<dyn Clock> = clock.clone();
        let guest = Guest { script: Script::new(), state_hangs: true };
        let mover = clock.clone();
        let (read, ()) = futures::join!(reads_state(&guest, &dyn_clock, 30_000), async move {
            tokio::task::yield_now().await;
            mover.advance(30_000);
        });
        assert_eq!(read, None);
        assert_eq!(clock.asked.borrow().clone(), vec![30_000], "the bound is the provider's read, on the runtime's clock");
    })
}

#[test]
fn vault_paths_drop_the_skipped_and_the_empty() {
    // C5. runtime.ts:517-528
    block(async {
        let guest = Guest { script: Script::new(), state_hangs: false };
        guest.script.once::<ExecResult>("machine.exec", ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() });
        guest.script.once::<ExecResult>("machine.exec", ExecResult { exit_code: 0.into(), stdout: ".cache\nprojects\n  \n.npm\n .claude \n.local\n".into(), stderr: String::new() });
        let paths = vault_paths_of(&guest, None).await.unwrap();
        assert_eq!(paths, vec!["/root/projects".to_string(), "/root/.claude".to_string()]);
        assert_eq!(
            guest.script.calls_to("machine.exec").iter().map(|c| c.args["cmd"].clone()).collect::<Vec<_>>(),
            vec![json!("date -u +%FT%TZ >> /root/.wsp-upgraded"), json!("ls -A /root")]
        );
    })
}

#[test]
fn vault_paths_given_read_nothing_off_the_machine() {
    // C5. runtime.ts:518
    block(async {
        let guest = Guest { script: Script::new(), state_hangs: false };
        let given = vec!["/root/a".to_string()];
        assert_eq!(vault_paths_of(&guest, Some(&given)).await.unwrap(), given);
        assert!(guest.script.calls().is_empty());
    })
}

#[test]
fn vault_enumeration_failing_names_stderrs_tail() {
    // C5. runtime.ts:522
    block(async {
        let guest = Guest { script: Script::new(), state_hangs: false };
        let stderr = format!("{}{}", "x".repeat(50), "y".repeat(200));
        guest.script.once::<ExecResult>("machine.exec", ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() });
        guest.script.once::<ExecResult>("machine.exec", ExecResult { exit_code: 2.into(), stdout: String::new(), stderr: stderr.clone() });
        let refused = vault_paths_of(&guest, None).await.err().unwrap();
        assert_eq!(refused.message, format!("vault enumeration failed: {}", "y".repeat(200)));
    })
}

// ---- A thread's launch ----

fn minted() -> PairedDevice {
    PairedDevice { device_id: "d-thread".into(), device_token: "tok-thread".into(), device: serde_json::from_value(json!({ "id": "d-thread", "name": "thread", "createdAt": "x", "lastSeenAt": "x" })).unwrap() }
}

#[test]
fn a_launch_with_no_road_mints_nothing() {
    // C6. runtime.ts:544-545
    block(async {
        let ctx = Ctx::new();
        let core = Core::new(ctx.clone());
        let launch = core.thread_launch("ws-1", WorkspaceKind::Cloud, "t-1", "t-1", false).await.unwrap();
        assert_eq!(launch.scoped, None);
        assert!(launch.env.is_empty());
        assert_eq!(launch.wsp, None);
        assert!(ctx.script.calls_to("devices.mint").is_empty());
    })
}

#[test]
fn the_mint_is_scoped_to_the_thread_and_its_road() {
    // C8. runtime.ts:554, spawn-guard.test.ts
    block(async {
        let mut ctx = Ctx::new();
        Rc::get_mut(&mut ctx).unwrap().reach = Some(LaunchReach { url: None, wsp: None });
        ctx.script.once::<Result<PairedDevice, HostError>>("devices.mint", Ok(minted()));
        let core = Core::new(ctx.clone());
        let launch = core.thread_launch("ws-1", WorkspaceKind::Cloud, "t-child", "t-root", true).await.unwrap();
        let mint = &ctx.script.calls_to("devices.mint")[0].args;
        assert_eq!(mint["name"], json!(format!("thread {}", wsp_words::words::computer::thread_word("t-child"))));
        assert_eq!(mint["scope"], json!({ "kind": "thread", "threadId": "t-child", "workspaceId": "ws-1", "rootThreadId": "t-root" }));
        assert_eq!(mint["road"], json!("relayed"));
        assert_eq!(mint["aside"], json!(true));
        assert_eq!(launch.scoped, Some(minted()));
        assert_eq!(launch.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>(), vec![(wsp_wire::types::env::HOST_TOKEN_ENV.to_string(), "tok-thread".to_string())], "no url, no key");
    })
}

#[test]
fn a_launch_names_the_url_and_key_only_where_they_stand() {
    // C9. runtime.ts:556-563
    block(async {
        let mut ctx = Ctx::new();
        let wsp: McpServerSpec = serde_json::from_value(json!({ "command": "wsp", "args": ["mcp"] })).unwrap_or_else(|e| panic!("build an McpServerSpec: {e}"));
        {
            let c = Rc::get_mut(&mut ctx).unwrap();
            c.reach = Some(LaunchReach { url: Some("https://host:7420".into()), wsp: Some(wsp.clone()) });
            c.host_key = Some("SHA256:hk".into());
        }
        ctx.script.on("devices.mint", |_| Ok::<PairedDevice, HostError>(minted()));
        let core = Core::new(ctx.clone());
        let launch = core.thread_launch("ws-1", WorkspaceKind::Local, "t-1", "t-1", false).await.unwrap();
        let env: Vec<(String, String)> = launch.env.into_iter().collect();
        use wsp_wire::types::env::{HOST_KEY_ENV, HOST_TOKEN_ENV, HOST_URL_ENV};
        assert_eq!(
            env,
            vec![(HOST_TOKEN_ENV.into(), "tok-thread".into()), (HOST_URL_ENV.into(), "https://host:7420".into()), (HOST_KEY_ENV.into(), "SHA256:hk".into())]
        );
        assert_eq!(launch.wsp, Some(wsp));
        assert_eq!(ctx.script.calls_to("devices.mint")[0].args["road"], json!("here"), "a turn beside the host is stamped here");

        let mut no_door = Ctx::new();
        Rc::get_mut(&mut no_door).unwrap().reach = Some(LaunchReach { url: Some("https://host:7420".into()), wsp: None });
        no_door.script.on("devices.mint", |_| Ok::<PairedDevice, HostError>(minted()));
        let launch = Core::new(no_door.clone()).thread_launch("ws-1", WorkspaceKind::Cloud, "t-1", "t-1", false).await.unwrap();
        assert_eq!(launch.env.keys().cloned().collect::<Vec<_>>(), vec![HOST_TOKEN_ENV.to_string(), HOST_URL_ENV.to_string()], "no place door, no key");
    })
}

#[test]
fn a_brokered_launch_reads_the_broker_first() {
    // C7. runtime.ts:549-553
    block(async {
        let mut ctx = Ctx::new();
        {
            let c = Rc::get_mut(&mut ctx).unwrap();
            c.reach = Some(LaunchReach { url: None, wsp: None });
            c.brokered = true;
        }
        ctx.fails.borrow_mut().push("rereadPublic".into());
        ctx.script.on("devices.mint", |_| Ok::<PairedDevice, HostError>(minted()));
        let core = Core::new(ctx.clone());
        let warned = wsp_core::console::capture();
        core.thread_launch("ws-1", WorkspaceKind::Cloud, "t-1", "t-1", false).await.unwrap();
        tokio::task::yield_now().await;
        let names = ctx.script.names();
        let mint_at = names.iter().position(|n| n == "devices.mint").unwrap();
        for read in ["readBroker", "daemonSettled", "readPublicMarketplaces", "rereadPublic"] {
            let at = names.iter().position(|n| n == read).unwrap_or_else(|| panic!("{read} was asked"));
            assert!(at < mint_at, "{read} before the mint: {names:?}");
        }
        assert_eq!(warned.lines(), vec!["the visibility of name-of-ws-1's repo was not read again: rereadPublic refused".to_string()]);
    })
}

// ---- Timers ----

#[test]
fn a_snooze_wakes_every_window_at_its_time() {
    // C10. thread-marks.test.ts:186
    block(async {
        let ctx = Ctx::new();
        let seen = emitted(&ctx);
        ctx.snoozed.borrow_mut().insert("t-1".into(), 1_060_000.0);
        let core = Core::new(ctx.clone());
        core.wake_at("t-1", 1_060_000.0);
        ctx.clock.advance(59_999);
        assert!(seen.borrow().is_empty());
        ctx.clock.advance(1);
        assert_eq!(seen.borrow().clone(), vec![marked("ws-of-t-1", "t-1")]);
        assert_eq!(core.armed().0, Vec::<String>::new(), "the wake forgets itself");
    })
}

#[test]
fn a_moved_snooze_says_nothing_and_a_new_one_replaces_the_old() {
    // C10. runtime.ts:865-873
    block(async {
        let ctx = Ctx::new();
        let seen = emitted(&ctx);
        let core = Core::new(ctx.clone());
        ctx.snoozed.borrow_mut().insert("t-1".into(), 1_010_000.0);
        core.wake_at("t-1", 1_010_000.0);
        ctx.snoozed.borrow_mut().insert("t-1".into(), 1_020_000.0);
        core.wake_at("t-1", 1_020_000.0);
        assert_eq!(ctx.clock.armed(), 1, "a new snooze cancels the old wake");
        ctx.clock.advance(10_000);
        assert!(seen.borrow().is_empty());
        ctx.snoozed.borrow_mut().shift_remove("t-1");
        ctx.clock.advance(10_000);
        assert!(seen.borrow().is_empty(), "a snooze that went says nothing");
    })
}

#[test]
fn a_long_snooze_rearms_past_the_timer_cap() {
    // C10. runtime.ts:874, 877 (MAX_TIMER_MS)
    block(async {
        let ctx = Ctx::new();
        let seen = emitted(&ctx);
        let core = Core::new(ctx.clone());
        let max = wsp_core::types::internal::MAX_TIMER_MS as i64;
        let until = 1_000_000 + max + 5_000;
        ctx.snoozed.borrow_mut().insert("t-1".into(), until as f64);
        core.wake_at("t-1", until as f64);
        assert_eq!(ctx.clock.asked.borrow().clone(), vec![max]);
        ctx.clock.advance(max);
        assert!(seen.borrow().is_empty(), "early: re-armed, not said");
        assert_eq!(ctx.clock.asked.borrow().clone(), vec![max, 5_000]);
        ctx.clock.advance(5_000);
        assert_eq!(seen.borrow().clone(), vec![marked("ws-of-t-1", "t-1")]);
    })
}

#[test]
fn a_past_snooze_arms_nothing() {
    // C10. runtime.ts:867-868
    block(async {
        let ctx = Ctx::new();
        let core = Core::new(ctx.clone());
        core.wake_at("t-1", 1_000_000.0);
        core.wake_at("t-2", 999_000.0);
        assert_eq!(ctx.clock.armed(), 0);
        assert!(ctx.clock.asked.borrow().is_empty());
    })
}

#[test]
fn resume_at_reset_fires_at_its_time_and_at_once_when_past() {
    // C11. usage-limit-resume.test.ts; runtime.ts:886-899
    block(async {
        let ctx = Ctx::new();
        let core = Core::new(ctx.clone());
        ctx.resumes.borrow_mut().insert("t-1".into(), 1_005_000.0);
        core.resume_on_reset("t-1", 1_005_000.0);
        ctx.clock.advance(4_999);
        assert!(ctx.script.calls_to("resumeAfterLimit").is_empty());
        ctx.clock.advance(1);
        tokio::task::yield_now().await;
        assert_eq!(ctx.script.calls_to("resumeAfterLimit")[0].args, json!({ "threadId": "t-1" }));

        ctx.resumes.borrow_mut().insert("t-2".into(), 900_000.0);
        core.resume_on_reset("t-2", 900_000.0);
        assert_eq!(ctx.clock.asked.borrow().last().copied(), Some(0), "a reset already past is armed at 0");
        ctx.clock.advance(0);
        tokio::task::yield_now().await;
        assert_eq!(ctx.script.calls_to("resumeAfterLimit").len(), 2);
    })
}

#[test]
fn a_moved_resume_does_nothing() {
    // C11. runtime.ts:892
    block(async {
        let ctx = Ctx::new();
        let core = Core::new(ctx.clone());
        ctx.resumes.borrow_mut().insert("t-1".into(), 1_005_000.0);
        core.resume_on_reset("t-1", 1_005_000.0);
        ctx.resumes.borrow_mut().insert("t-1".into(), 1_009_000.0);
        ctx.clock.advance(5_000);
        tokio::task::yield_now().await;
        assert!(ctx.script.calls_to("resumeAfterLimit").is_empty());
    })
}

#[test]
fn a_failed_resume_warns() {
    // C11. runtime.ts:894
    block(async {
        let ctx = Ctx::new();
        ctx.fails.borrow_mut().push("resumeAfterLimit".into());
        let core = Core::new(ctx.clone());
        let warned = wsp_core::console::capture();
        ctx.resumes.borrow_mut().insert("t-1".into(), 1_000_500.0);
        core.resume_on_reset("t-1", 1_000_500.0);
        ctx.clock.advance(500);
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }
        assert_eq!(warned.lines(), vec![format!("thread {} did not resume at its reset: resumeAfterLimit refused", wsp_words::words::computer::thread_word("t-1"))]);
    })
}

// ---- What stands on a computer ----

fn box_ctx() -> Rc<Ctx> {
    let mut ctx = Ctx::new();
    {
        let c = Rc::get_mut(&mut ctx).unwrap();
        c.threads = vec![("t-a".into(), "ws-fork".into()), ("t-b".into(), "ws-folder".into()), ("t-c".into(), "ws-elsewhere".into())];
        c.sessions = vec![session("ws-fork", "t-a", "fix login"), session("ws-folder", "t-b2", "tidy")];
    }
    *ctx.live.borrow_mut() = vec![
        standing("ws-fork", WorkspaceKind::Cloud, "box-1", "p-1"),
        standing("ws-folder", WorkspaceKind::Place, "box-1", "p-1"),
        standing("ws-elsewhere", WorkspaceKind::Place, "box-2", "p-1"),
    ];
    *ctx.projects.borrow_mut() = vec![project("p-1", "app", "box-1"), project("p-2", "site", "box-2")];
    ctx
}

#[test]
fn standing_on_splits_forks_folders_and_projects() {
    // C12. places-remove.test.ts; runtime.ts:908-927
    block(async {
        let ctx = box_ctx();
        let core = Core::new(ctx.clone());
        let on = core.standing_on("box-1").await.unwrap();
        assert_eq!(ctx.script.names()[0], "ready");
        assert_eq!(on.forks.iter().map(|e| e.id.clone()).collect::<Vec<_>>(), vec!["ws-fork"]);
        assert_eq!(on.folders.iter().map(|e| e.id.clone()).collect::<Vec<_>>(), vec!["ws-folder"]);
        assert_eq!(on.projects.iter().map(|p| p.id.clone()).collect::<Vec<_>>(), vec!["p-1"]);
        // The fork's one thread; the project counts its folder here (t-b and t-b2) and the one elsewhere (t-c).
        assert_eq!(on.rows, StandingRows { forks: vec![NamedCount { name: "fix login".into(), threads: 1 }], projects: vec![NamedCount { name: "app".into(), threads: 3 }] });
        assert_eq!(serde_json::to_string(&on.rows).unwrap(), r#"{"forks":[{"name":"fix login","threads":1}],"projects":[{"name":"app","threads":3}]}"#);
    })
}

#[test]
fn fork_name_is_its_one_threads_title_else_its_name() {
    // C13. runtime.ts:936-939
    let mut ctx = Ctx::new();
    Rc::get_mut(&mut ctx).unwrap().sessions = vec![session("ws-1", "t-1", "one"), session("ws-2", "t-2", "two"), session("ws-2", "t-3", "three")];
    let core = Core::new(ctx.clone());
    assert_eq!(core.fork_name(&standing("ws-1", WorkspaceKind::Cloud, "b", "p")), "one");
    assert_eq!(core.fork_name(&standing("ws-2", WorkspaceKind::Cloud, "b", "p")), "name-ws-2", "two threads: its own name");
    assert_eq!(core.fork_name(&standing("ws-3", WorkspaceKind::Cloud, "b", "p")), "name-ws-3", "no thread: its own name");
}

#[test]
fn forget_project_in_order() {
    // C14. runtime.ts:929-934
    block(async {
        let ctx = box_ctx();
        let seen = emitted(&ctx);
        let core = Core::new(ctx.clone());
        core.forget_project("p-2").await.unwrap();
        let names: Vec<String> = ctx.script.names().into_iter().filter(|n| n != "events.on").collect();
        assert_eq!(names, vec!["storeDeleteProject", "forgetIcon"]);
        assert_eq!(seen.borrow().clone(), vec![json!({ "type": "project.removed", "projectId": "p-2" })]);
    })
}

#[test]
fn holds_on_reads_unsaved_forks_then_projects() {
    // C16. runtime.ts:629-648
    block(async {
        let ctx = box_ctx();
        ctx.live.borrow_mut()[0].running = false;
        let core = Core::new(ctx.clone());
        let (rows, unsaved) = core.holds_on("box-1", true).await.unwrap();
        assert_eq!(unsaved, vec!["fix login has unsaved work".to_string(), "app has unsaved work".to_string()]);
        assert_eq!(rows.forks, vec![NamedCount { name: "fix login".into(), threads: 1 }]);
        let (_, none) = core.holds_on("box-1", false).await.unwrap();
        assert!(none.is_empty(), "a hold read without reading names nothing unsaved");
    })
}

#[test]
fn drop_on_stops_at_the_first_refusal_naming_those_gone() {
    // C17. runtime.ts:653-670
    block(async {
        let ctx = box_ctx();
        ctx.fails.borrow_mut().push("delete ws-folder".into());
        let core = Core::new(ctx.clone());
        let refused = core.drop_on("box-1").await.err().unwrap();
        assert_eq!(refused.message, wsp_wire::wire::machine_link::place_drop_stopped_line("delete ws-folder refused", &["fix login".to_string()]));
        assert_eq!(ctx.script.calls_to("deleteWorkspace").iter().map(|c| c.args["id"].clone()).collect::<Vec<_>>(), vec![json!("ws-fork"), json!("ws-folder")]);
        assert!(ctx.script.calls_to("removeProject").is_empty(), "the rest stop");
    })
}

#[test]
fn drop_on_takes_forks_then_folders_then_projects_with_force() {
    // C17. runtime.ts:665-669
    block(async {
        let ctx = box_ctx();
        let core = Core::new(ctx.clone());
        let rows = core.drop_on("box-1").await.unwrap();
        let order: Vec<String> = ctx.script.calls().into_iter().filter(|c| c.name == "deleteWorkspace" || c.name == "removeProject").map(|c| format!("{} {}", c.name, c.args["id"])).collect();
        assert_eq!(order, vec![r#"deleteWorkspace "ws-fork""#, r#"deleteWorkspace "ws-folder""#, r#"removeProject "p-1""#]);
        assert_eq!(ctx.script.calls_to("removeProject")[0].args, json!({ "id": "p-1", "force": true, "keepLook": false }));
        assert_eq!(rows.projects, vec![NamedCount { name: "app".into(), threads: 3 }]);
    })
}

#[test]
fn forget_on_ends_and_drops_then_forgets() {
    // C18. runtime.ts:671-679
    block(async {
        let ctx = box_ctx();
        let core = Core::new(ctx.clone());
        core.forget_on("box-1").await.unwrap();
        let order: Vec<String> = ctx.script.calls().into_iter().filter(|c| ["endSessions", "dropRecord", "storeDeleteProject"].contains(&c.name.as_str())).map(|c| format!("{} {}", c.name, c.args["id"])).collect();
        assert_eq!(order, vec![r#"endSessions "ws-fork""#, r#"dropRecord "ws-fork""#, r#"endSessions "ws-folder""#, r#"dropRecord "ws-folder""#, r#"storeDeleteProject "p-1""#]);
        assert_eq!(ctx.script.calls_to("endSessions")[0].args["reason"], json!(wsp_core::types::internal::DELETED_REASON));
    })
}

#[test]
fn machines_on_names_the_machines_there() {
    // C19. runtime.ts:686-689
    block(async {
        let ctx = box_ctx();
        ctx.live.borrow_mut()[1].machine_id = String::new();
        let core = Core::new(ctx.clone());
        assert_eq!(core.machines_on("box-1").await.unwrap(), vec!["m-ws-fork".to_string()]);
    })
}

#[test]
fn remove_folder_refuses_while_records_stand() {
    // C20. runtime.ts:756-762
    block(async {
        let ctx = box_ctx();
        let core = Core::new(ctx.clone());
        core.remove_folder("box-9", "p-1").await.unwrap();
        assert!(ctx.script.calls_to("storeDeleteProject").is_empty(), "a project not on that computer does nothing");
        let refused = core.remove_folder("box-1", "p-1").await.err().unwrap();
        assert_eq!(refused.message, wsp_words::projects::project_in_use_refusal("app", &["name-ws-fork".to_string(), "name-ws-folder".to_string(), "name-ws-elsewhere".to_string()]));
        ctx.live.borrow_mut().clear();
        core.remove_folder("box-1", "p-1").await.unwrap();
        assert_eq!(ctx.script.calls_to("storeDeleteProject")[0].args, json!({ "id": "p-1" }));
    })
}

// ---- The look a recipe moves ----

fn pick(icon: Option<&str>, hue: Option<&str>, image: Option<&str>) -> FolderPick {
    FolderPick { icon: icon.map(Into::into), hue: hue.map(Into::into), image: image.map(Into::into) }
}

#[test]
fn folder_look_keeps_what_the_person_set() {
    // C21. runtime.ts:583-589
    block(async {
        let ctx = Ctx::new();
        ctx.script.once::<Option<ProjectLook>>("projectLook", Some(ProjectLook { icon: Some("star".into()), hue: Some("red".into()) }));
        let core = Core::new(ctx.clone());
        // The icon moved, the hue did not: the recipe's new icon, the person's own hue; the image stayed.
        core.folder_look("p-1", &pick(Some("code"), Some("blue"), Some("h1")), &pick(Some("rocket"), Some("blue"), Some("h1"))).await.unwrap();
        assert_eq!(ctx.script.calls_to("setProjectLook")[0].args.to_string(), json!({ "id": "p-1", "look": { "icon": "rocket", "hue": "red" } }).to_string());
        assert!(ctx.script.calls_to("setIcon").is_empty(), "an image that did not move is not worn again");
    })
}

#[test]
fn folder_look_with_neither_writes_null_and_a_moved_image_is_worn_or_cleared() {
    // C21. runtime.ts:587-588
    block(async {
        let ctx = Ctx::new();
        let core = Core::new(ctx.clone());
        core.folder_look("p-1", &pick(None, None, Some("h1")), &pick(None, None, None)).await.unwrap();
        assert_eq!(ctx.script.calls_to("setProjectLook")[0].args, json!({ "id": "p-1", "look": null }));
        assert_eq!(ctx.script.calls_to("setIcon")[0].args, json!({ "id": "p-1", "image": null }), "an image gone is cleared");
        core.folder_look("p-1", &pick(None, None, None), &pick(None, None, Some("h2"))).await.unwrap();
        assert_eq!(ctx.script.calls_to("setIcon")[1].args, json!({ "id": "p-1", "image": "h2" }), "an image that moved is worn");
    })
}

// ---- Bus and place door wiring ----

fn stage_frame(place: &str, stage: &str, detail: Option<&str>) -> Value {
    let mut f = json!({ "type": "golden.stage", "name": "default", "stage": stage, "place": place });
    if let Some(d) = detail {
        f["detail"] = json!(d);
    }
    f
}

#[test]
fn place_setup_with_an_end_keeps_the_copy_current() {
    // C23. runtime.ts:774-777
    block(async {
        let ctx = Ctx::new();
        let core = Core::new(ctx.clone());
        core.on_bus_event(&json!({ "type": "place.setup", "placeId": "box-1" }));
        assert!(ctx.script.calls_to("keepCurrent").is_empty(), "a setup still running asks nothing");
        core.on_bus_event(&json!({ "type": "place.setup", "placeId": "box-1", "end": "done" }));
        assert_eq!(ctx.script.calls_to("keepCurrent").iter().map(|c| c.args.clone()).collect::<Vec<_>>(), vec![json!({ "place": "box-1" })]);
    })
}

#[test]
fn golden_stage_writes_and_drops_the_copy_row() {
    // C26. runtime.ts:841-846, places-image.test.ts
    block(async {
        let ctx = Ctx::new();
        let core = Core::new(ctx.clone());
        core.on_bus_event(&json!({ "type": "golden.stage", "name": "default", "stage": "creating" }));
        assert_eq!(core.copy_row("box-1"), None, "a frame naming no place writes no row");
        core.on_bus_event(&stage_frame("box-1", "creating", None));
        assert!(core.copy_row("box-1").is_some(), "a build running there is on its row");
        core.on_bus_event(&stage_frame("box-1", "sealed", None));
        assert_eq!(core.copy_row("box-1"), None, "a copy that stands drops the row");
    })
}

#[test]
fn place_removed_drops_the_copy_row() {
    // C15. runtime.ts:769-771
    block(async {
        let ctx = Ctx::new();
        let core = Core::new(ctx.clone());
        core.on_bus_event(&stage_frame("box-1", "creating", None));
        core.on_bus_event(&stage_frame("box-2", "creating", None));
        core.on_place_event(&json!({ "type": "place.removed", "placeId": "box-1" }));
        assert_eq!(core.copy_row("box-1"), None);
        assert!(core.copy_row("box-2").is_some(), "another place's row stays");
    })
}

#[test]
fn place_absent_and_present_re_emit_the_stage() {
    // C24. runtime.ts:781-791, places-image.test.ts
    block(async {
        let ctx = Ctx::new();
        let seen = emitted(&ctx);
        let core = Core::new(ctx.clone());
        let frame = json!({ "type": "golden.stage", "name": "default", "stage": "installing-tools", "detail": "apt-get install", "step": 3 });
        core.note_stage(StageAt { place: "box-1".into(), name: "default".into(), named: true, frame: frame.clone() });
        core.note_stage(StageAt { place: "box-2".into(), name: "default".into(), named: false, frame: frame.clone() });
        core.on_place_event(&json!({ "type": "place.absent", "placeId": "box-1" }));
        let dial_back = wsp_wire::wire::place_link::place_dial_back_line("box-1-name");
        assert_eq!(seen.borrow().clone(), vec![json!({ "type": "golden.stage", "name": "default", "stage": "installing-tools", "detail": dial_back, "step": 3, "place": "box-1" })]);
        core.on_place_event(&json!({ "type": "place.present", "placeId": "box-1" }));
        tokio::task::yield_now().await;
        assert_eq!(seen.borrow()[1], json!({ "type": "golden.stage", "name": "default", "stage": "installing-tools", "detail": "apt-get install", "step": 3, "place": "box-1" }));
        // A stage that did not name its place is re-said without one.
        core.on_place_event(&json!({ "type": "place.absent", "placeId": "box-2" }));
        assert!(seen.borrow().last().unwrap().get("place").is_none());
    })
}

#[test]
fn place_present_rehydrates_stand_ins_only() {
    // C25. runtime.ts:796-812, places.test.ts
    block(async {
        let ctx = Ctx::new();
        ctx.script.once::<Result<Vec<String>, HostError>>("heldAwayOn", Ok(vec!["ws-back".into(), "ws-still-away".into()]));
        ctx.script.once::<bool>("hydrate", true);
        ctx.script.once::<bool>("hydrate", false);
        ctx.script.once::<Vec<String>>("heldOn", vec!["ws-back".into(), "ws-live".into()]);
        let core = Core::new(ctx.clone());
        core.on_place_event(&json!({ "type": "place.present", "placeId": "box-1" }));
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        let names: Vec<String> = ctx.script.names().into_iter().filter(|n| n != "events.on" && n != "projectLook").collect();
        assert_eq!(names, vec!["ready", "heldAwayOn", "hydrate", "syncDaemon", "hydrate", "rereadHeld", "heldOn", "poll"]);
        assert_eq!(ctx.script.calls_to("syncDaemon")[0].args, json!({ "id": "ws-back" }));
        assert_eq!(ctx.script.calls_to("rereadHeld")[0].args, json!({ "id": "ws-still-away", "why": "record load" }));
        assert_eq!(ctx.script.calls_to("poll")[0].args, json!({ "ids": ["ws-back", "ws-live"] }));
    })
}

#[test]
fn row_says_failure_is_false_for_the_place_reasons() {
    // C27. runtime.ts:850-853
    let ctx = Ctx::new();
    let core = Core::new(ctx.clone());
    assert!(core.row_says_failure("box-1", &HostError::new("the build broke")));
    assert!(!core.row_says_failure("box-1", &wsp_machines::places::types::place_forks_nowhere("no forks there")));
    assert!(!core.row_says_failure("box-1", &wsp_machines::places::types::place_provisioning("a recipe runs there")));
    assert!(!core.row_says_failure("laptop-asleep", &HostError::new("the build broke")), "a place away is no build to show");
}
