// SPDX-License-Identifier: AGPL-3.0-only
//! A runtime over the script: wsp-core's `Runtime`, with wsp-machines' `PlaceDoor`, wsp-slates' `Slates` and wsp-threads'
//! `ActivityDoor` as its three doors (`DynRuntime`). Each sub-API (`workspaces()`,
//! `sessions()`, ...) is the fake itself; every method records `<area>.<method in camelCase>` (`workspaces.get`,
//! `sessions.history`, `status.list`, `agents.read`, `harnesses.list`, `golden.get`, `usage.used`,
//! `preferences.get`, `conversations.list`, `projects.list`, `slates.write`, `activity.read`, `startRoads.startAt`,
//! ...) with its arguments as JSON (camelCase keys as the TS names them; the origin through `caller_json`), then
//! answers what the test scripted under that name. A `Result` method unscripted refuses `unscripted call: <name>`.
//!
//! `sessions.start` calls the start's `on_held` first with the `HeldStart` scripted under `sessions.start.held`, if any.
//! `places()` answers `self.places` (none unless the test sets it). `slates()` and `activity()` answer fakes that
//! record `slates.<method>` and `activity.<method>` with their params as JSON (a key only where the param is set) and
//! answer what the test scripted, typed as the real answer (`SlateWriteAnswer`, `ActivityEnded`, ...).
//!
//! Runtime's own members record `runtime.<name>` (`runtime.recipes`, `runtime.hereChannel`, `runtime.owner`,
//! `runtime.reap`, `runtime.close`, `runtime.watchPlaceSys`, `runtime.placeKills`, `runtime.ownProcess`). Every
//! `on_event` handed to `here_channel`, `workspaces.daemon_channel` and `workspaces.guest_channel` is kept on
//! `daemon_listeners` before the call answers. `status()` is `FakeStatus`: its `OriginStatusApi` methods record
//! `status.list` and `status.history`; the bare `StatusApi` ones record `statusApi.list` and `statusApi.history`,
//! and `watch`, `watched`, `poll`, `spend` record `status.<name>` for both.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{ready, LocalBoxFuture};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::status::{StatusApi, StatusListOptions, StatusWatchOptions};
use wsp_core::types::api::*;
use wsp_core::types::events::EventBus;
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::pending::{CallbackForwards, DaemonChannel, DeviceDoor, GoldenDelta, SignInAsk, SkillAsk};
use wsp_core::types::wiring::*;
use wsp_engine::machine::{Machine, MachineBackend, MachineListRow, MachineSpec, Waiting};
use wsp_engine::snapshot_storage::RetentionPlan;
use wsp_frames::{GitCommitReply, GitDiscardReply, ProcKind};
use wsp_machines::places::types::PlaceDoor;
use wsp_slates::pending::wire::{SlateEventAnswer, SlateReadAnswer, SlateStateAnswer, SlateView, SlateWriteAnswer, SlatesImageAnswer};
use wsp_slates::slates::{ApproveParams, CatalogParams, EventParams, ReadParams, SlateTarget, Slates, StateParams, WriteParams};
use wsp_threads::activity::{ActivityAsk, ActivityDoor, EndAsk, MeetVerb};
use wsp_wire::types::agents_report::*;
use wsp_wire::types::attachments::KeptAttachment;
use wsp_wire::types::bring_back::BringBackResult;
use wsp_wire::types::changes::{CheckoutReply, CommitDraft, ViewedMarks};
use wsp_wire::types::conversations::{ConversationRead, ConversationsAnswer, ResumeAsk};
use wsp_wire::types::pull_request::*;
use wsp_wire::types::start::{GitPrDiffReply, ReviewDraft};
use wsp_wire::types::thread_defaults::{AgentSetupSet, ThreadDefaults};
use wsp_wire::types::tree::MergeInResult;
use wsp_wire::types::usage::{AccountsAnswer, ResetAnswer, UsedAnswer};
use wsp_wire::types::workspace_look::WorkspaceLook;
use wsp_wire::views::activity::{ActivityEnded, ActivityView, ProcessesAsk};
use wsp_wire::views::golden_image::*;
use wsp_wire::views::harness::{HarnessCatalog, ReviewPostResult, StartResult};
use wsp_wire::views::place_more::{PlaceSpend, PlaceView};
use wsp_wire::views::preferences::{HistoryPage, Preferences, ThreadHead};
use wsp_wire::views::session::{SessionView, ThreadMarks};
use wsp_wire::views::session_events_more::{RunStep, SessionEvent, SessionRunEvent};
use wsp_wire::views::snapshot_lineage::{SealedImageView, SnapshotStorage};
use wsp_wire::views::workspace::{Caller, DaemonReachView, PortProbeView, PortReachView, ProjectSeed, ProjectView, SeedPlan};
use wsp_wire::views::workspace_events::WorkspaceCreatingEvent;
use wsp_wire::views::workspace_more::{ProjectBranch, ThreadScope, WorkspaceStatus, WorkspaceView, WorktreeMade};
use wsp_wire::wire::capabilities::Capabilities;
use wsp_wire::wire::limits::PortForward;
use wsp_wire::wire::machine_link::{ExecResult, ReadingsAnswer};
use wsp_wire::wire::runtime::*;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use super::fake_bus::FakeBus;
use super::fake_devices::FakeDevices;
use super::fake_places::{dbg, sj, FakePlaceDoor};
use super::script::{caller_json as oj, Call, Script};

pub struct FakeRuntime {
    pub script: Rc<Script>,
    pub bus: Rc<FakeBus>,
    pub devices: Rc<FakeDevices>,
    pub places: RefCell<Option<Rc<FakePlaceDoor>>>,
    /// What `backend().capabilities` reads: the stub backend's flags, as JSON.
    pub capabilities: RefCell<serde_json::Value>,
    /// Every `on_event` handed to `here_channel`, `workspaces.daemon_channel` and `workspaces.guest_channel`, oldest first.
    pub daemon_listeners: RefCell<Vec<DaemonEventListener>>,
}

impl FakeRuntime {
    /// A runtime with no place door and the default capabilities.
    pub fn new() -> Rc<FakeRuntime> {
        let script = Script::new();
        Rc::new(FakeRuntime {
            bus: FakeBus::new(script.clone()),
            devices: FakeDevices::new(script.clone()),
            places: RefCell::new(None),
            capabilities: RefCell::new(serde_json::json!({})),
            daemon_listeners: RefCell::new(Vec::new()),
            script,
        })
    }
    /// The same, with a place door over the same script.
    pub fn with_places() -> Rc<FakeRuntime> {
        let rt = FakeRuntime::new();
        *rt.places.borrow_mut() = Some(FakePlaceDoor::new(rt.script.clone()));
        rt
    }
}

fn start_json(o: &StartOptions) -> Value {
    json!({
        "prompt": o.prompt, "harness": o.harness, "thread": o.thread, "cwd": o.cwd, "model": o.model, "effort": o.effort,
        "permissionMode": o.permission_mode, "access": o.access.as_ref().map(sj), "contextWindow": o.context_window,
        "fast": o.fast, "startedBy": o.started_by.as_ref().map(sj), "as": o.r#as.as_ref().map(dbg),
        "requestId": o.request_id, "viaSlate": o.via_slate, "attempt": o.attempt, "notify": o.notify,
        "turnToken": o.turn_token, "title": o.title, "replaces": o.replaces, "fork": o.fork.as_ref().map(sj),
        "attachments": o.attachments.as_ref().map(sj), "mcpServers": o.mcp_servers.as_ref().map(sj),
        "behind": o.behind, "setup": o.setup, "afterLimit": o.after_limit, "wakesLead": o.wakes_lead, "owed": o.owed,
        "asksUntilStopped": o.asks_until_stopped, "onHeld": o.on_held.is_some(), "followed": o.followed,
        "outside": o.outside.as_ref().map(dbg),
    })
}

fn noop_detach() -> Detach {
    Box::new(|| {})
}

#[async_trait(?Send)]
impl Runtime for FakeRuntime {
    type Places = dyn PlaceDoor;
    type Slates = dyn Slates;
    type Activity = dyn ActivityDoor;
    fn events(&self) -> Rc<dyn EventBus> {
        self.bus.clone()
    }
    fn backend(&self) -> Arc<dyn MachineBackend> {
        let mut base = json!({
            "liveCloneForks": false, "replacesMachine": false, "previewUrls": false, "signedUrls": false,
            "callbackRelay": false, "diskSnapshots": false, "images": false, "snapshotsAnyLife": false,
            "snapshotListing": false, "templates": false, "kept": false, "copies": false, "ownNetwork": false, "sizes": [],
        });
        if let Value::Object(set) = self.capabilities.borrow().clone() {
            for (k, v) in set {
                base[k] = v;
            }
        }
        let capabilities = serde_json::from_value(base).expect("the scripted capabilities parse as Capabilities");
        Arc::new(FakeBackend { capabilities })
    }
    fn places(&self) -> Option<Rc<dyn PlaceDoor>> {
        self.places.borrow().clone().map(|p| p as Rc<dyn PlaceDoor>)
    }
    fn recipes(&self) -> Option<Rc<dyn RecipeShelf>> {
        self.script.call_or("runtime.recipes", json!({}), || None::<Rc<dyn RecipeShelf>>)
    }
    async fn here_channel(&self, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        self.daemon_listeners.borrow_mut().push(on_event);
        self.script.call("runtime.hereChannel", json!({ "onEvent": "_" }))
    }
    fn place_kills(&self, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        self.script.call_or("runtime.placeKills", json!({ "channel": "_" }), || channel)
    }
    async fn watch_place_sys(&self, place_id: &str, _listener: SysListener) -> Result<Detach, HostError> {
        self.script.call("runtime.watchPlaceSys", json!({ "placeId": place_id, "listener": "_" }))
    }
    fn own_process(&self, thread_id: &str, pid: u32, kind: ProcKind) -> Detach {
        self.script.call_or("runtime.ownProcess", json!({ "threadId": thread_id, "pid": pid, "kind": dbg(&kind) }), noop_detach)
    }
    fn agents(&self) -> &dyn AgentsApi {
        self
    }
    fn workspaces(&self) -> &dyn WorkspacesApi {
        self
    }
    fn projects(&self) -> &dyn ProjectsApi {
        self
    }
    fn sessions(&self) -> &dyn SessionsApi {
        self
    }
    fn harnesses(&self) -> &dyn HarnessesApi {
        self
    }
    fn golden(&self) -> &dyn GoldenApi {
        self
    }
    fn image(&self) -> &dyn ImageApi {
        self
    }
    fn status(&self) -> Rc<dyn OriginStatusApi> {
        Rc::new(FakeStatus { script: self.script.clone() })
    }
    fn usage(&self) -> Rc<dyn UsageDoor> {
        Rc::new(FakeUsage { script: self.script.clone() })
    }
    fn conversations(&self) -> &dyn ConversationsApi {
        self
    }
    fn slates(&self) -> Rc<dyn Slates> {
        Rc::new(FakeSlates { script: self.script.clone() })
    }
    fn activity(&self) -> Rc<dyn ActivityDoor> {
        Rc::new(FakeActivity { script: self.script.clone() })
    }
    fn devices(&self) -> Rc<dyn DeviceDoor> {
        self.devices.clone()
    }
    fn preferences(&self) -> &dyn PreferencesApi {
        self
    }
    async fn owner(&self) -> Result<String, HostError> {
        self.script.call("runtime.owner", json!({}))
    }
    async fn reap(&self, older_than_ms: Option<u64>, _say: Option<Rc<dyn Fn(&str)>>) -> Result<SweepResult, HostError> {
        self.script.call("runtime.reap", json!({ "olderThanMs": older_than_ms, "say": "_" }))
    }
    async fn close(&self) -> Result<(), HostError> {
        self.script.call("runtime.close", json!({}))
    }
}

#[async_trait(?Send)]
impl AgentsApi for FakeRuntime {
    async fn read(&self, target: AgentsTarget, origin: Option<Caller>) -> Result<AgentsReport, HostError> {
        self.script.call("agents.read", json!({ "target": sj(&target), "origin": oj(&origin) }))
    }
    async fn tools(&self, target: AgentsTarget, ask: ToolsAsk, origin: Option<Caller>) -> Result<ServerToolsAnswer, HostError> {
        self.script.call("agents.tools", json!({ "target": sj(&target), "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn sign_in(
        &self,
        target: AgentsTarget,
        ask: SignInAsk,
        _emit: Rc<dyn Fn(AgentsSignInEvent)>,
        origin: Option<Caller>,
    ) -> Result<SignInFollowing, HostError> {
        self.script.call("agents.signIn", json!({ "target": sj(&target), "ask": dbg(&ask), "emit": "_", "origin": oj(&origin) }))
    }
    fn sign_ins(&self) -> Vec<AgentsSignInRun> {
        self.script.call_or("agents.signIns", json!({}), Vec::new)
    }
    fn sign_in_follow(&self, sign_in_id: &str, _emit: Rc<dyn Fn(AgentsSignInEvent)>) -> Result<SignInFollowing, HostError> {
        self.script.call("agents.signInFollow", json!({ "signInId": sign_in_id, "emit": "_" }))
    }
    async fn sign_in_code(&self, sign_in_id: &str, code: &str) -> Result<(), HostError> {
        self.script.call("agents.signInCode", json!({ "signInId": sign_in_id, "code": code }))
    }
    fn sign_in_stop(&self, sign_in_id: &str) {
        self.script.call_or("agents.signInStop", json!({ "signInId": sign_in_id }), || ())
    }
    async fn sign_in_line(&self, target: AgentsTarget, ask: SignInAsk, origin: Option<Caller>) -> Result<SignInLine, HostError> {
        self.script.call("agents.signInLine", json!({ "target": sj(&target), "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn key(&self, agent: &str, key: &str, way: Option<KeyWay>) -> Result<(), HostError> {
        self.script.call("agents.key", json!({ "agent": agent, "key": key, "way": way.as_ref().map(dbg) }))
    }
    async fn add_tools(&self, target: AgentsTarget, agent: &str, origin: Option<Caller>) -> Result<String, HostError> {
        self.script.call("agents.addTools", json!({ "target": sj(&target), "agent": agent, "origin": oj(&origin) }))
    }
    fn forwards(&self, _relay: Rc<dyn CallbackForwards>) -> Detach {
        self.script.call_or("agents.forwards", json!({ "relay": "_" }), noop_detach)
    }
    async fn skills_search(&self, q: &str, limit: Option<u32>) -> Result<Vec<SkillHit>, HostError> {
        self.script.call("agents.skillsSearch", json!({ "q": q, "limit": limit }))
    }
    async fn skills_get(&self, skill: &str) -> Result<SkillPreview, HostError> {
        self.script.call("agents.skillsGet", json!({ "skill": skill }))
    }
    async fn skills_preview(&self, target: AgentsTarget, ask: SkillAsk, origin: Option<Caller>) -> Result<SkillPreview, HostError> {
        self.script.call("agents.skillsPreview", json!({ "target": sj(&target), "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn skills_add(&self, target: AgentsTarget, ask: SkillsAddAsk, origin: Option<Caller>) -> Result<SkillAdded, HostError> {
        self.script.call("agents.skillsAdd", json!({ "target": sj(&target), "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn skills_remove(&self, target: AgentsTarget, ask: SkillAsk, origin: Option<Caller>) -> Result<Vec<String>, HostError> {
        self.script.call("agents.skillsRemove", json!({ "target": sj(&target), "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn skills_toggle(&self, target: AgentsTarget, ask: SkillAsk, on: bool, origin: Option<Caller>) -> Result<Vec<String>, HostError> {
        self.script.call("agents.skillsToggle", json!({ "target": sj(&target), "ask": dbg(&ask), "on": on, "origin": oj(&origin) }))
    }
    async fn servers_add(&self, target: AgentsTarget, ask: ServerAdd, origin: Option<Caller>) -> Result<String, HostError> {
        self.script.call("agents.serversAdd", json!({ "target": sj(&target), "ask": sj(&ask), "origin": oj(&origin) }))
    }
    async fn servers_remove(&self, target: AgentsTarget, ask: ServerAsk, origin: Option<Caller>) -> Result<String, HostError> {
        self.script.call("agents.serversRemove", json!({ "target": sj(&target), "ask": sj(&ask), "origin": oj(&origin) }))
    }
    async fn servers_toggle(&self, target: AgentsTarget, ask: ServerAsk, on: bool, origin: Option<Caller>) -> Result<String, HostError> {
        self.script.call("agents.serversToggle", json!({ "target": sj(&target), "ask": sj(&ask), "on": on, "origin": oj(&origin) }))
    }
    async fn servers_icon(&self, host: &str, refresh: Option<bool>) -> Result<Option<String>, HostError> {
        self.script.call("agents.serversIcon", json!({ "host": host, "refresh": refresh }))
    }
    async fn plugins_toggle(&self, target: AgentsTarget, ask: PluginAsk, on: bool, origin: Option<Caller>) -> Result<PluginRow, HostError> {
        self.script.call("agents.pluginsToggle", json!({ "target": sj(&target), "ask": sj(&ask), "on": on, "origin": oj(&origin) }))
    }
    async fn setup(&self, place_id: &str, agent: &str, change: AgentSetupSet, origin: Option<Caller>) -> Result<AgentRow, HostError> {
        self.script.call("agents.setup", json!({ "placeId": place_id, "agent": agent, "change": sj(&change), "origin": oj(&origin) }))
    }
    async fn homes_here(&self) -> Result<IndexMap<String, String>, HostError> {
        self.script.call("agents.homesHere", json!({}))
    }
}

#[async_trait(?Send)]
impl WorkspacesApi for FakeRuntime {
    async fn create(&self, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError> {
        self.script.call("workspaces.create", json!({ "opts": dbg(&opts), "origin": oj(&origin) }))
    }
    async fn landing(&self, project: &str, sized: bool, origin: Option<Caller>) -> Result<Landing, HostError> {
        self.script.call("workspaces.landing", json!({ "project": project, "sized": sized, "origin": oj(&origin) }))
    }
    async fn get(&self, id: &str, origin: Option<Caller>, thread_id: Option<&str>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.get", json!({ "id": id, "origin": oj(&origin), "threadId": thread_id }))
    }
    async fn held_or_creating(&self, id: &str) -> Result<Option<WorkspaceView>, HostError> {
        self.script.call("workspaces.heldOrCreating", json!({ "id": id }))
    }
    fn thread_place(&self, scope: &ThreadScope) -> Result<Option<String>, HostError> {
        self.script.call_or("workspaces.threadPlace", json!({ "scope": sj(scope) }), || Ok(None))
    }
    async fn list(&self, origin: Option<Caller>) -> Result<Vec<WorkspaceView>, HostError> {
        self.script.call("workspaces.list", json!({ "origin": oj(&origin) }))
    }
    async fn resolve(&self, reference: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.resolve", json!({ "reference": reference, "origin": oj(&origin) }))
    }
    async fn nap(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.nap", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn wake(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.wake", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn resize(&self, id: &str, size: SizeAsk, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.resize", json!({ "id": id, "size": dbg(&size), "origin": oj(&origin) }))
    }
    async fn stop_wake(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.stopWake", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn upgrade(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.upgrade", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn rebuild(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.rebuild", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn restart_daemon(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.script.call("workspaces.restartDaemon", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn rename(&self, id: &str, name: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.rename", json!({ "id": id, "name": name, "origin": oj(&origin) }))
    }
    async fn look(&self, id: &str, look: WorkspaceLook, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.look", json!({ "id": id, "look": sj(&look), "origin": oj(&origin) }))
    }
    async fn update_daemon(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.script.call("workspaces.updateDaemon", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn delete(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.script.call("workspaces.delete", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn forget(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.script.call("workspaces.forget", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn touch(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.script.call("workspaces.touch", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn exec(&self, id: &str, cmd: &str, timeout_ms: Option<u64>, origin: Option<Caller>) -> Result<ExecResult, HostError> {
        self.script.call("workspaces.exec", json!({ "id": id, "cmd": cmd, "timeoutMs": timeout_ms, "origin": oj(&origin) }))
    }
    async fn exec_stream(
        &self,
        id: &str,
        argv: Vec<String>,
        cwd: Option<String>,
        origin: Option<Caller>,
    ) -> Result<RunningExec, HostError> {
        self.script.call("workspaces.execStream", json!({ "id": id, "argv": argv, "cwd": cwd, "origin": oj(&origin) }))
    }
    async fn bring_back(
        &self,
        workspace_id: &str,
        title: Option<String>,
        body: Option<String>,
        origin: Option<Caller>,
    ) -> Result<BringBackResult, HostError> {
        self.script
            .call("workspaces.bringBack", json!({ "workspaceId": workspace_id, "title": title, "body": body, "origin": oj(&origin) }))
    }
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError> {
        self.script.call("workspaces.folderFor", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn take_setup_line(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
        self.script.call("workspaces.takeSetupLine", json!({ "workspaceId": workspace_id, "origin": oj(&origin) }))
    }
    async fn folder(&self, project: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
        self.script.call("workspaces.folder", json!({ "project": project, "origin": oj(&origin) }))
    }
    async fn worktree(&self, project: &str, branch: &str, origin: Option<Caller>) -> Result<WorktreeMade, HostError> {
        self.script.call("workspaces.worktree", json!({ "project": project, "branch": branch, "origin": oj(&origin) }))
    }
    async fn worktree_remove(&self, ask: WorktreeRemoveAsk, origin: Option<Caller>) -> Result<(), HostError> {
        self.script.call("workspaces.worktreeRemove", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn checkout(&self, id: &str, origin: Option<Caller>, fresh: Option<bool>) -> Result<CheckoutReply, HostError> {
        self.script.call("workspaces.checkout", json!({ "id": id, "origin": oj(&origin), "fresh": fresh }))
    }
    async fn discard(&self, ask: DiscardAsk, origin: Option<Caller>) -> Result<GitDiscardReply, HostError> {
        self.script.call("workspaces.discard", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn commit(&self, ask: CommitAsk, origin: Option<Caller>) -> Result<GitCommitReply, HostError> {
        self.script.call("workspaces.commit", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn commit_draft(&self, ask: CommitDraftAsk, origin: Option<Caller>) -> Result<CommitDraft, HostError> {
        self.script.call("workspaces.commitDraft", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn viewed(&self, workspace_id: &str, mark: Option<ViewedMark>, origin: Option<Caller>) -> Result<ViewedMarks, HostError> {
        self.script.call("workspaces.viewed", json!({ "workspaceId": workspace_id, "mark": mark.as_ref().map(dbg), "origin": oj(&origin) }))
    }
    async fn pull_request_view(
        &self,
        workspace_id: &str,
        fresh: Option<bool>,
        origin: Option<Caller>,
    ) -> Result<PullRequestPage, HostError> {
        self.script.call("workspaces.pullRequestView", json!({ "workspaceId": workspace_id, "fresh": fresh, "origin": oj(&origin) }))
    }
    async fn pull_request_diff(&self, workspace_id: &str, origin: Option<Caller>) -> Result<GitPrDiffReply, HostError> {
        self.script.call("workspaces.pullRequestDiff", json!({ "workspaceId": workspace_id, "origin": oj(&origin) }))
    }
    async fn pull_request_send(
        &self,
        workspace_id: &str,
        items: Vec<PullRequestItem>,
        thread_id: Option<String>,
        origin: Option<Caller>,
    ) -> Result<PullRequestSendResult, HostError> {
        self.script.call(
            "workspaces.pullRequestSend",
            json!({ "workspaceId": workspace_id, "items": sj(&items), "threadId": thread_id, "origin": oj(&origin) }),
        )
    }
    async fn pull_request_reply(&self, ask: PullRequestReplyAsk, origin: Option<Caller>) -> Result<GitPrReplyReply, HostError> {
        self.script.call("workspaces.pullRequestReply", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn pull_request_resolve(
        &self,
        workspace_id: &str,
        thread_id: &str,
        resolved: bool,
        origin: Option<Caller>,
    ) -> Result<GitPrResolveReply, HostError> {
        self.script.call(
            "workspaces.pullRequestResolve",
            json!({ "workspaceId": workspace_id, "threadId": thread_id, "resolved": resolved, "origin": oj(&origin) }),
        )
    }
    async fn pull_request_react(
        &self,
        workspace_id: &str,
        subject: &str,
        content: ReactionContent,
        on: bool,
        origin: Option<Caller>,
    ) -> Result<GitPrReactReply, HostError> {
        self.script.call(
            "workspaces.pullRequestReact",
            json!({ "workspaceId": workspace_id, "subject": subject, "content": sj(&content), "on": on, "origin": oj(&origin) }),
        )
    }
    async fn fix(&self, ask: FixAsk, origin: Option<Caller>) -> Result<FixResult, HostError> {
        self.script.call("workspaces.fix", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn merge(&self, ask: MergeAsk, origin: Option<Caller>) -> Result<MergeResult, HostError> {
        self.script.call("workspaces.merge", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn update(&self, workspace_id: &str, thread_id: Option<String>, origin: Option<Caller>) -> Result<GitUpdateReply, HostError> {
        self.script.call("workspaces.update", json!({ "workspaceId": workspace_id, "threadId": thread_id, "origin": oj(&origin) }))
    }
    async fn merge_in(&self, ask: MergeInAsk, origin: Option<Caller>) -> Result<MergeInResult, HostError> {
        self.script.call("workspaces.mergeIn", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn start(&self, ask: StartFromLinkAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
        self.script.call("workspaces.start", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn review(&self, ask: ReviewAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
        self.script.call("workspaces.review", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn review_draft(&self, ask: ReviewDraftAsk, origin: Option<Caller>) -> Result<Option<ReviewDraft>, HostError> {
        self.script.call("workspaces.reviewDraft", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn review_post(
        &self,
        workspace_id: &str,
        thread_id: Option<String>,
        origin: Option<Caller>,
    ) -> Result<ReviewPostResult, HostError> {
        self.script.call("workspaces.reviewPost", json!({ "workspaceId": workspace_id, "threadId": thread_id, "origin": oj(&origin) }))
    }
    async fn daemon_reach(&self, id: &str, origin: Option<Caller>) -> Result<DaemonReachView, HostError> {
        self.script.call("workspaces.daemonReach", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn token_refused(&self, id: &str) -> Result<(), HostError> {
        self.script.call("workspaces.tokenRefused", json!({ "id": id }))
    }
    async fn daemon_channel(
        &self,
        id: &str,
        on_event: DaemonEventListener,
        origin: Option<Caller>,
    ) -> Result<Rc<dyn DaemonChannel>, HostError> {
        self.daemon_listeners.borrow_mut().push(on_event);
        self.script.call("workspaces.daemonChannel", json!({ "id": id, "onEvent": "_", "origin": oj(&origin) }))
    }
    async fn guest_channel(&self, id: &str, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        self.daemon_listeners.borrow_mut().push(on_event);
        self.script.call("workspaces.guestChannel", json!({ "id": id, "onEvent": "_" }))
    }
    async fn served_by_its_computer(&self, id: &str, origin: Option<Caller>) -> Result<bool, HostError> {
        self.script.call("workspaces.servedByItsComputer", json!({ "id": id, "origin": oj(&origin) }))
    }
    async fn watch_sys(&self, id: &str, _listener: SysListener, origin: Option<Caller>) -> Result<Detach, HostError> {
        self.script.call("workspaces.watchSys", json!({ "id": id, "listener": "_", "origin": oj(&origin) }))
    }
    async fn port_reach(&self, id: &str, port: u16, origin: Option<Caller>) -> Result<PortReachView, HostError> {
        self.script.call("workspaces.portReach", json!({ "id": id, "port": port, "origin": oj(&origin) }))
    }
    async fn port_probe(&self, id: &str, port: u16, origin: Option<Caller>) -> Result<PortProbeView, HostError> {
        self.script.call("workspaces.portProbe", json!({ "id": id, "port": port, "origin": oj(&origin) }))
    }
    async fn origin_refusal(&self, id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
        self.script.call("workspaces.originRefusal", json!({ "id": id, "origin": oj(&origin) }))
    }
    fn seen_by(&self, event: &Value, origin: Option<&Caller>) -> Result<bool, HostError> {
        self.script.call_or("workspaces.seenBy", json!({ "event": event, "origin": oj(&origin.cloned()) }), || Ok(true))
    }
    fn shown_to(&self, event: Value, origin: Option<&Caller>) -> Result<Value, HostError> {
        let args = json!({ "event": event, "origin": oj(&origin.cloned()) });
        self.script.call_or("workspaces.shownTo", args, || Ok(event))
    }
    fn creating(&self) -> Vec<WorkspaceCreatingEvent> {
        self.script.call_or("workspaces.creating", json!({}), Vec::new)
    }
}

#[async_trait(?Send)]
impl ProjectsApi for FakeRuntime {
    async fn add(&self, ask: ProjectAddAsk, origin: Option<Caller>) -> Result<ProjectAdded, HostError> {
        self.script.call("projects.add", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn seed_plan(&self, source: &str, on: Option<String>) -> Result<SeedPlan, HostError> {
        self.script.call("projects.seedPlan", json!({ "source": source, "on": on }))
    }
    async fn seed(&self, reference: &str, ask: SeedAsk, origin: Option<Caller>) -> Result<ProjectSeed, HostError> {
        self.script.call("projects.seed", json!({ "reference": reference, "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn list(&self, origin: Option<Caller>) -> Result<Vec<ProjectView>, HostError> {
        self.script.call("projects.list", json!({ "origin": oj(&origin) }))
    }
    async fn defaults(&self, origin: Option<Caller>) -> Result<IndexMap<String, ThreadDefaults>, HostError> {
        self.script.call("projects.defaults", json!({ "origin": oj(&origin) }))
    }
    async fn computers(&self) -> Result<Vec<ComputerName>, HostError> {
        self.script.call("projects.computers", json!({}))
    }
    async fn resolve(&self, reference: &str, origin: Option<Caller>) -> Result<ProjectView, HostError> {
        self.script.call("projects.resolve", json!({ "reference": reference, "origin": oj(&origin) }))
    }
    async fn branch(&self, reference: &str, origin: Option<Caller>) -> Result<ProjectBranch, HostError> {
        self.script.call("projects.branch", json!({ "reference": reference, "origin": oj(&origin) }))
    }
    async fn remove(
        &self,
        id: &str,
        origin: Option<Caller>,
        force: Option<bool>,
        check: Option<bool>,
    ) -> Result<ProjectRemoved, HostError> {
        self.script.call("projects.remove", json!({ "id": id, "origin": oj(&origin), "force": force, "check": check }))
    }
    async fn icon(&self, project_id: &str, png: Option<String>) -> Result<Option<String>, HostError> {
        self.script.call("projects.icon", json!({ "projectId": project_id, "png": png }))
    }
    async fn icons(&self, hashes: Vec<String>) -> Result<IndexMap<String, Option<String>>, HostError> {
        self.script.call("projects.icons", json!({ "hashes": hashes }))
    }
    async fn import(
        &self,
        _opts: ProjectImportOptions,
        origin: Option<Caller>,
    ) -> Result<wsp_wire::views::project_bundle::ProjectImportResult, HostError> {
        self.script.call("projects.import", json!({ "opts": "_", "origin": oj(&origin) }))
    }
    async fn export(
        &self,
        _opts: ProjectExportOptions,
        origin: Option<Caller>,
    ) -> Result<wsp_wire::views::project_bundle::ProjectExportResult, HostError> {
        self.script.call("projects.export", json!({ "opts": "_", "origin": oj(&origin) }))
    }
}

#[async_trait(?Send)]
impl SessionsApi for FakeRuntime {
    async fn start(&self, workspace_id: &str, opts: StartOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError> {
        let args = json!({ "workspaceId": workspace_id, "opts": start_json(&opts), "origin": oj(&origin) });
        if let Some(on_held) = &opts.on_held {
            let held = self.script.answer::<HeldStart>(&Call { name: "sessions.start.held".into(), args: Value::Null });
            if let Some(held) = held {
                on_held(held);
            }
        }
        self.script.call("sessions.start", args)
    }
    async fn list(&self, workspace_id: Option<String>, origin: Option<Caller>) -> Result<Vec<SessionView>, HostError> {
        self.script.call("sessions.list", json!({ "workspaceId": workspace_id, "origin": oj(&origin) }))
    }
    async fn history(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Vec<SessionEvent>, HostError> {
        self.script.call("sessions.history", json!({ "workspaceId": workspace_id, "origin": oj(&origin) }))
    }
    async fn thread_history(&self, workspace_id: &str, asked: HistoryAsk, origin: Option<Caller>) -> Result<Vec<SessionEvent>, HostError> {
        self.script.call("sessions.threadHistory", json!({ "workspaceId": workspace_id, "asked": dbg(&asked), "origin": oj(&origin) }))
    }
    async fn page(&self, workspace_id: &str, window: PageWindow, origin: Option<Caller>) -> Result<HistoryPage, HostError> {
        self.script.call("sessions.page", json!({ "workspaceId": workspace_id, "window": dbg(&window), "origin": oj(&origin) }))
    }
    async fn head(&self, thread_id: &str, origin: Option<Caller>, after_read: Option<bool>) -> Result<ThreadHead, HostError> {
        self.script.call("sessions.head", json!({ "threadId": thread_id, "origin": oj(&origin), "afterRead": after_read }))
    }
    async fn attachment(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: &str,
        index: u32,
        origin: Option<Caller>,
    ) -> Result<KeptAttachment, HostError> {
        self.script.call(
            "sessions.attachment",
            json!({ "workspaceId": workspace_id, "threadId": thread_id, "requestId": request_id, "index": index, "origin": oj(&origin) }),
        )
    }
    async fn interrupt(&self, session_id: &str, origin: Option<Caller>, task: Option<String>) -> Result<SessionInterruptResult, HostError> {
        self.script.call("sessions.interrupt", json!({ "sessionId": session_id, "origin": oj(&origin), "task": task }))
    }
    async fn steer(&self, session_id: &str, ask: SteerAsk, origin: Option<Caller>) -> Result<SessionSteerResult, HostError> {
        self.script.call("sessions.steer", json!({ "sessionId": session_id, "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn answer(&self, session_id: &str, ask: AnswerAsk, origin: Option<Caller>) -> Result<SessionAnswerResult, HostError> {
        self.script.call("sessions.answer", json!({ "sessionId": session_id, "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn access(&self, session_id: &str, permission_mode: &str, origin: Option<Caller>) -> Result<SessionAccessResult, HostError> {
        self.script.call("sessions.access", json!({ "sessionId": session_id, "permissionMode": permission_mode, "origin": oj(&origin) }))
    }
    async fn rename(&self, session_id: &str, title: &str, origin: Option<Caller>) -> Result<SessionRenameResult, HostError> {
        self.script.call("sessions.rename", json!({ "sessionId": session_id, "title": title, "origin": oj(&origin) }))
    }
    async fn read(&self, thread_id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.script.call("sessions.read", json!({ "threadId": thread_id, "origin": oj(&origin) }))
    }
    async fn note_waited(&self, thread_id: &str, ms: f64, said: Option<String>) -> Result<(), HostError> {
        self.script.call("sessions.noteWaited", json!({ "threadId": thread_id, "ms": ms, "said": said }))
    }
    fn note_retry(&self, thread_id: &str, attempt: u32, why: &str) {
        self.script.call_or("sessions.noteRetry", json!({ "threadId": thread_id, "attempt": attempt, "why": why }), || ())
    }
    async fn replaceable(&self, thread_id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.script.call("sessions.replaceable", json!({ "threadId": thread_id, "origin": oj(&origin) }))
    }
    async fn fork(&self, opts: ForkOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError> {
        let args =
            json!({ "opts": { "start": start_json(&opts.start), "fork": sj(&opts.fork), "branch": opts.branch }, "origin": oj(&origin) });
        self.script.call("sessions.fork", args)
    }
    async fn warm(&self, workspace_id: &str, ask: WarmAsk, origin: Option<Caller>) -> Result<SessionWarmResult, HostError> {
        self.script.call("sessions.warm", json!({ "workspaceId": workspace_id, "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn settle(&self, thread_ids: Vec<String>, origin: Option<Caller>, ask: SettleAsk) -> Result<SessionSettleResult, HostError> {
        self.script.call("sessions.settle", json!({ "threadIds": thread_ids, "origin": oj(&origin), "ask": dbg(&ask) }))
    }
    async fn mark(&self, thread_ids: Vec<String>, marks: ThreadMarks, origin: Option<Caller>) -> Result<(), HostError> {
        self.script.call("sessions.mark", json!({ "threadIds": thread_ids, "marks": sj(&marks), "origin": oj(&origin) }))
    }
    async fn restore(&self, thread_ids: Vec<String>, origin: Option<Caller>) -> Result<SessionRestoreResult, HostError> {
        self.script.call("sessions.restore", json!({ "threadIds": thread_ids, "origin": oj(&origin) }))
    }
    async fn search(&self, query: &str, origin: Option<Caller>) -> Result<SessionSearchResult, HostError> {
        self.script.call("sessions.search", json!({ "query": query, "origin": oj(&origin) }))
    }
    async fn aside(
        &self,
        session_id: &str,
        question: &str,
        origin: Option<Caller>,
        ask_id: Option<String>,
    ) -> Result<SessionAsideResult, HostError> {
        self.script.call("sessions.aside", json!({ "sessionId": session_id, "question": question, "origin": oj(&origin), "askId": ask_id }))
    }
    async fn rewind(&self, thread_id: &str, ask: RewindAsk, origin: Option<Caller>) -> Result<SessionRewindResult, HostError> {
        self.script.call("sessions.rewind", json!({ "threadId": thread_id, "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn run(&self, step: RunStep, origin: Option<Caller>) -> Result<SessionRunEvent, HostError> {
        self.script.call("sessions.run", json!({ "step": sj(&step), "origin": oj(&origin) }))
    }
    async fn forget(&self, thread_id: &str, origin: Option<Caller>, check: Option<bool>) -> Result<(), HostError> {
        self.script.call("sessions.forget", json!({ "threadId": thread_id, "origin": oj(&origin), "check": check }))
    }
    async fn delete(&self, thread_id: &str, origin: Option<Caller>, ask: DeleteThreadAsk) -> Result<ThreadDeleted, HostError> {
        self.script.call("sessions.delete", json!({ "threadId": thread_id, "origin": oj(&origin), "ask": dbg(&ask) }))
    }
}

#[async_trait(?Send)]
impl HarnessesApi for FakeRuntime {
    async fn list(
        &self,
        workspace_id: Option<String>,
        origin: Option<Caller>,
        at: Option<CatalogAt>,
    ) -> Result<Vec<HarnessCatalog>, HostError> {
        self.script.call("harnesses.list", json!({ "workspaceId": workspace_id, "origin": oj(&origin), "at": at.as_ref().map(dbg) }))
    }
}

#[async_trait(?Send)]
impl GoldenApi for FakeRuntime {
    async fn build(&self, opts: GoldenBuildRequest) -> Result<Sealed, HostError> {
        self.script.call("golden.build", json!({ "opts": dbg(&opts) }))
    }
    async fn get(&self, name: Option<String>) -> Result<Option<GoldenManifest>, HostError> {
        self.script.call("golden.get", json!({ "name": name }))
    }
    async fn build_place(&self, word: Option<String>) -> Result<BuildPlace, HostError> {
        self.script.call("golden.buildPlace", json!({ "word": word }))
    }
    async fn prepare(&self, ask: PrepareAsk) -> Result<GoldenBuilderView, HostError> {
        let args = json!({ "ask": { "name": ask.name, "kind": ask.kind.as_ref().map(sj), "recipe": ask.recipe.is_some(), "place": ask.place, "copy": ask.copy } });
        self.script.call("golden.prepare", args)
    }
    async fn prepare_at(&self, name: Option<String>, place: Option<String>) -> Result<(String, String), HostError> {
        self.script.call("golden.prepareAt", json!({ "name": name, "place": place }))
    }
    async fn seal(&self, builder_id: &str, logins: Option<Vec<GoldenLogin>>, keep_builder: Option<bool>) -> Result<Sealed, HostError> {
        self.script.call("golden.seal", json!({ "builderId": builder_id, "logins": logins.as_ref().map(sj), "keepBuilder": keep_builder }))
    }
    async fn recipe(&self, name: Option<String>) -> Result<Option<RecipeDigest>, HostError> {
        self.script.call("golden.recipe", json!({ "name": name }))
    }
    async fn upgrade(&self, ask: UpgradeAsk) -> Result<GoldenUpgradeResult, HostError> {
        let args = json!({ "ask": { "name": ask.name, "keepPrevious": ask.keep_previous, "logins": ask.logins.as_ref().map(sj), "recipe": ask.recipe.is_some() } });
        self.script.call("golden.upgrade", args)
    }
    async fn builder_reach(&self, builder_id: &str) -> Result<DaemonReachView, HostError> {
        self.script.call("golden.builderReach", json!({ "builderId": builder_id }))
    }
    async fn builders(&self) -> Result<Vec<GoldenBuilderView>, HostError> {
        self.script.call("golden.builders", json!({}))
    }
    async fn kill(&self, builder_id: &str) -> Result<(), HostError> {
        self.script.call("golden.kill", json!({ "builderId": builder_id }))
    }
    async fn rollback(&self, version: u32, name: Option<String>) -> Result<GoldenManifest, HostError> {
        self.script.call("golden.rollback", json!({ "version": version, "name": name }))
    }
    async fn promote(&self, name: Option<String>) -> Result<Option<Vec<GoldenPromotion>>, HostError> {
        self.script.call("golden.promote", json!({ "name": name }))
    }
    async fn storage(&self) -> Result<Option<SnapshotStorage>, HostError> {
        self.script.call("golden.storage", json!({}))
    }
    async fn orphans(&self) -> Result<Option<AccountOrphans>, HostError> {
        self.script.call("golden.orphans", json!({}))
    }
    async fn delete_orphans(&self) -> Result<Option<OrphansDeleted>, HostError> {
        self.script.call("golden.deleteOrphans", json!({}))
    }
    async fn retention_places(&self, name: Option<String>) -> Result<Vec<String>, HostError> {
        self.script.call("golden.retentionPlaces", json!({ "name": name }))
    }
    async fn retention(&self, name: Option<String>, place: Option<String>) -> Result<Option<PlacedRetention>, HostError> {
        self.script.call("golden.retention", json!({ "name": name, "place": place }))
    }
    async fn prune(&self, name: Option<String>, place: Option<String>) -> Result<Pruned, HostError> {
        self.script.call("golden.prune", json!({ "name": name, "place": place }))
    }
}

#[async_trait(?Send)]
impl ImageApi for FakeRuntime {
    async fn get(&self, name: Option<String>) -> Result<SealedImageView, HostError> {
        self.script.call("image.get", json!({ "name": name }))
    }
    async fn vault(&self, name: Option<String>) -> Result<(SealedImage, Vec<u8>), HostError> {
        self.script.call("image.vault", json!({ "name": name }))
    }
    async fn build(&self, ask: ImageBuildAsk) -> Result<SealedImageBuilt, HostError> {
        self.script.call("image.build", json!({ "ask": { "place": ask.place, "name": ask.name, "force": ask.force } }))
    }
    async fn keep_current(&self, place: &str, name: Option<String>) {
        self.script.call_or("image.keepCurrent", json!({ "place": place, "name": name }), || ())
    }
}

#[async_trait(?Send)]
impl ConversationsApi for FakeRuntime {
    async fn list(&self, project: Option<String>, agent: Option<String>, origin: Option<Caller>) -> Result<ConversationsAnswer, HostError> {
        self.script.call("conversations.list", json!({ "project": project, "agent": agent, "origin": oj(&origin) }))
    }
    async fn read(&self, ask: ConversationAsk, origin: Option<Caller>) -> Result<ConversationRead, HostError> {
        self.script.call("conversations.read", json!({ "ask": dbg(&ask), "origin": oj(&origin) }))
    }
    async fn opening(
        &self,
        workspace_id: &str,
        harness: Option<String>,
        resume: ResumeAsk,
        origin: Option<Caller>,
    ) -> Result<OutsideOpening, HostError> {
        self.script.call(
            "conversations.opening",
            json!({ "workspaceId": workspace_id, "harness": harness, "resume": sj(&resume), "origin": oj(&origin) }),
        )
    }
    async fn placed(&self, start: PlacedStart, resume: ResumeAsk, origin: Option<Caller>) -> Result<Placed, HostError> {
        self.script.call("conversations.placed", json!({ "start": dbg(&start), "resume": sj(&resume), "origin": oj(&origin) }))
    }
}

#[async_trait(?Send)]
impl PreferencesApi for FakeRuntime {
    async fn get(&self) -> Result<Preferences, HostError> {
        self.script.call("preferences.get", json!({}))
    }
    async fn set(&self, patch: wsp_wire::views::preferences::PreferencesPatch) -> Result<PreferencesSet, HostError> {
        self.script.call("preferences.set", json!({ "patch": sj(&patch) }))
    }
}

pub struct FakeStatus {
    script: Rc<Script>,
}

impl FakeStatus {
    fn call_str<T: 'static>(&self, name: &str, args: Value) -> Result<T, String> {
        let call = self.script.record(name, args);
        self.script.answer::<Result<T, String>>(&call).unwrap_or_else(|| Err(format!("unscripted call: {name}")))
    }
}

#[async_trait(?Send)]
impl StatusApi for FakeStatus {
    async fn list(&self, opts: StatusListOptions) -> Result<Vec<WorkspaceStatus>, String> {
        self.call_str("statusApi.list", json!({ "opts": dbg(&opts) }))
    }
    fn watch(&self, opts: StatusWatchOptions) -> Rc<dyn Fn()> {
        self.script.call_or("status.watch", json!({ "opts": dbg(&opts) }), || Rc::new(|| {}) as Rc<dyn Fn()>)
    }
    fn watched(&self) -> bool {
        self.script.call_or("status.watched", json!({}), || false)
    }
    async fn poll(&self, ids: &[String]) {
        self.script.call_or("status.poll", json!({ "ids": ids }), || ())
    }
    async fn history(&self, workspace_id: &str) -> Vec<wsp_core::status::CostPoint> {
        self.script.call_or("statusApi.history", json!({ "workspaceId": workspace_id }), Vec::new)
    }
    async fn spend(&self, places: &[PlaceView], at: Option<i64>) -> Result<Vec<PlaceSpend>, String> {
        self.call_str("status.spend", json!({ "places": sj(&places), "at": at }))
    }
}

#[async_trait(?Send)]
impl OriginStatusApi for FakeStatus {
    async fn list(&self, opts: Option<StatusListOptions>, origin: Option<Caller>) -> Result<Vec<WorkspaceStatus>, HostError> {
        self.script.call("status.list", json!({ "opts": opts.as_ref().map(dbg), "origin": oj(&origin) }))
    }
    async fn history(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Vec<wsp_core::status::CostPoint>, HostError> {
        self.script.call("status.history", json!({ "workspaceId": workspace_id, "origin": oj(&origin) }))
    }
}

pub struct FakeUsage {
    script: Rc<Script>,
}

#[async_trait(?Send)]
impl UsageDoor for FakeUsage {
    async fn used(
        &self,
        range: wsp_wire::types::usage::UsageRange,
        split: wsp_wire::types::usage::UsageSplit,
        outside: Option<bool>,
    ) -> Result<UsedAnswer, HostError> {
        self.script.call("usage.used", json!({ "range": sj(&range), "split": sj(&split), "outside": outside }))
    }
    async fn accounts(&self, fresh: Option<bool>) -> Result<AccountsAnswer, HostError> {
        self.script.call("usage.accounts", json!({ "fresh": fresh }))
    }
    async fn readings(
        &self,
        target: ReadingsTarget,
        range: wsp_wire::types::usage::ReadingsRange,
        origin: Option<Caller>,
    ) -> Result<ReadingsAnswer, HostError> {
        self.script.call("usage.readings", json!({ "target": dbg(&target), "range": sj(&range), "origin": oj(&origin) }))
    }
    async fn reset(&self, account: String, credit_id: Option<String>, on: Option<String>) -> Result<ResetAnswer, HostError> {
        self.script.call("usage.reset", json!({ "account": account, "creditId": credit_id, "on": on }))
    }
}

/// A backend that answers capabilities only.
pub struct FakeBackend {
    capabilities: Capabilities,
}

#[async_trait(?Send)]
impl MachineBackend for FakeBackend {
    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }
    fn pricing(&self) -> Arc<dyn wsp_engine::machine::BackendPricing> {
        panic!("the fake backend answers capabilities only")
    }
    async fn create(
        &self,
        _spec: MachineSpec,
        _waiting: Option<Waiting>,
        _sent: Option<String>,
    ) -> wsp_engine::errors::Result<Arc<dyn Machine>> {
        panic!("the fake backend answers capabilities only")
    }
    async fn get(&self, _id: &str) -> wsp_engine::errors::Result<Arc<dyn Machine>> {
        panic!("the fake backend answers capabilities only")
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> wsp_engine::errors::Result<Vec<MachineListRow>> {
        panic!("the fake backend answers capabilities only")
    }
    async fn delete_snapshot(&self, _id: &str) -> wsp_engine::errors::Result<()> {
        panic!("the fake backend answers capabilities only")
    }
}

/// `slates.*`: each call's params as JSON, a key only where the param is set.
pub struct FakeSlates {
    script: Rc<Script>,
}

fn target_json(target: &SlateTarget) -> serde_json::Map<String, Value> {
    let mut out = serde_json::Map::new();
    if let Some(t) = &target.thread_id {
        out.insert("threadId".into(), json!(t));
    }
    if let Some(t) = &target.turn_token {
        out.insert("turnToken".into(), json!(t));
    }
    out
}

fn set<T: serde::Serialize>(out: &mut serde_json::Map<String, Value>, key: &str, v: &Option<T>) {
    if let Some(v) = v {
        out.insert(key.into(), serde_json::to_value(v).unwrap());
    }
}

fn write_json(p: &WriteParams) -> Value {
    let mut o = target_json(&p.target);
    set(&mut o, "text", &p.text);
    set(&mut o, "document", &p.document);
    set(&mut o, "check", &p.check);
    set(&mut o, "ifVersion", &p.if_version);
    set(&mut o, "values", &p.values);
    if let Some(press) = &p.press {
        let mut pr = serde_json::Map::new();
        pr.insert("piece".into(), json!(press.piece));
        set(&mut pr, "index", &press.index);
        set(&mut pr, "action", &press.action);
        o.insert("press".into(), Value::Object(pr));
    }
    Value::Object(o)
}

fn state_json(p: &StateParams) -> Value {
    let mut o = target_json(&p.target);
    set(&mut o, "values", &p.values);
    set(&mut o, "start", &p.start);
    set(&mut o, "ifVersion", &p.if_version);
    Value::Object(o)
}

fn read_json(p: &ReadParams) -> Value {
    let mut o = target_json(&p.target);
    set(&mut o, "values", &p.values);
    set(&mut o, "text", &p.text);
    set(&mut o, "sketch", &p.sketch);
    set(&mut o, "document", &p.document);
    Value::Object(o)
}

fn catalog_json(p: &CatalogParams) -> Value {
    let mut o = target_json(&p.target);
    set(&mut o, "name", &p.name);
    Value::Object(o)
}

fn event_json(p: &EventParams) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("threadId".into(), json!(p.thread_id));
    o.insert("version".into(), json!(p.version));
    o.insert("piece".into(), json!(p.piece));
    o.insert("event".into(), json!(p.event));
    o.insert("requestId".into(), json!(p.request_id));
    if let Some(scope) = &p.scope {
        let mut sc = serde_json::Map::new();
        set(&mut sc, "item", &scope.item);
        sc.insert("index".into(), json!(scope.index));
        o.insert("scope".into(), Value::Object(sc));
    }
    set(&mut o, "rowAction", &p.row_action);
    Value::Object(o)
}

#[async_trait(?Send)]
impl Slates for FakeSlates {
    async fn get(&self, thread_id: &str) -> Result<Option<SlateView>, HostError> {
        self.script.call("slates.get", json!({ "threadId": thread_id }))
    }
    async fn write(&self, p: WriteParams, caller: Option<&Caller>) -> Result<SlateWriteAnswer, HostError> {
        self.script.call("slates.write", json!({ "params": write_json(&p), "origin": oj(&caller.cloned()) }))
    }
    async fn state(&self, p: StateParams, caller: Option<&Caller>) -> Result<SlateStateAnswer, HostError> {
        self.script.call("slates.state", json!({ "params": state_json(&p), "origin": oj(&caller.cloned()) }))
    }
    async fn read(&self, p: ReadParams, caller: Option<&Caller>) -> Result<SlateReadAnswer, HostError> {
        self.script.call("slates.read", json!({ "params": read_json(&p), "origin": oj(&caller.cloned()) }))
    }
    async fn catalog(&self, p: CatalogParams, caller: Option<&Caller>) -> Result<String, HostError> {
        self.script.call("slates.catalog", json!({ "params": catalog_json(&p), "origin": oj(&caller.cloned()) }))
    }
    async fn shown(&self, thread_id: &str) -> Result<(), HostError> {
        self.script.call("slates.shown", json!({ "threadId": thread_id }))
    }
    async fn revoke(&self, thread_id: &str, key: &str) -> Result<(), HostError> {
        self.script.call("slates.revoke", json!({ "threadId": thread_id, "key": key }))
    }
    async fn event(&self, p: EventParams) -> Result<SlateEventAnswer, HostError> {
        self.script.call("slates.event", json!({ "params": event_json(&p) }))
    }
    async fn approve(&self, p: ApproveParams) -> Result<(), HostError> {
        self.script
            .call("slates.approve", json!({ "threadId": p.thread_id, "key": p.key, "scope": format!("{:?}", p.scope).to_lowercase() }))
    }
    async fn cancel(&self, thread_id: &str, run: &str) -> Result<(), HostError> {
        self.script.call("slates.cancel", json!({ "threadId": thread_id, "run": run }))
    }
    fn subscribe(&self, thread_id: &str, sources: &[String]) -> Rc<dyn Fn()> {
        let args = json!({ "threadId": thread_id, "sources": sources });
        self.script.record("slates.subscribe", args.clone());
        let script = self.script.clone();
        Rc::new(move || {
            script.record("slates.release", args.clone());
        })
    }
    async fn resolve(&self, thread_id: &str, paths: &[String]) -> Result<IndexMap<String, Value>, HostError> {
        self.script.call("slates.resolve", json!({ "threadId": thread_id, "paths": paths }))
    }
    async fn image(&self, thread_id: &str, src: &str, have: Option<&str>) -> Result<SlatesImageAnswer, HostError> {
        self.script.call("slates.image", json!({ "threadId": thread_id, "src": src, "have": have }))
    }
    async fn turn_ended(&self, _thread_id: &str, _turn_id: &str) -> Result<(), HostError> {
        panic!("serve never tells the slates a turn ended")
    }
    async fn rewound(&self, _thread_id: &str, _turn_id: &str, _cut: &[String]) -> Result<(), HostError> {
        panic!("serve never tells the slates a thread rewound")
    }
    async fn forget(&self, _thread_id: &str, _keep: bool) -> Result<(), HostError> {
        panic!("serve never makes the slates forget a thread")
    }
    fn close_servers(&self, _thread_id: &str) {
        panic!("serve never closes a thread's slate servers")
    }
    fn watches_pr(&self, _workspace_id: &str) -> bool {
        panic!("serve never asks what the slates watch")
    }
    fn napped(&self, _workspace_id: &str) {
        panic!("serve never tells the slates a workspace napped")
    }
    async fn ready(&self) -> Result<(), HostError> {
        panic!("serve never waits on the slates")
    }
    async fn settled(&self) {
        panic!("serve never waits on the slates")
    }
    fn close(&self) {
        panic!("serve never closes the slates")
    }
}

/// `activity.*`: each call's params as JSON.
pub struct FakeActivity {
    script: Rc<Script>,
}

#[async_trait(?Send)]
impl ActivityDoor for FakeActivity {
    async fn read(&self, ask: ActivityAsk, caller: Option<Caller>) -> Result<ActivityView, HostError> {
        self.script.call(
            "activity.read",
            json!({ "computer": ask.computer, "thread": ask.thread, "threadIds": ask.thread_ids, "origin": oj(&caller) }),
        )
    }
    async fn end(&self, ask: EndAsk, caller: Option<Caller>) -> Result<ActivityEnded, HostError> {
        self.script
            .call("activity.end", json!({ "computer": ask.computer, "pid": ask.pid, "startedAt": ask.started_at, "origin": oj(&caller) }))
    }
    async fn meet(
        &self,
        thread_ids: &[String],
        processes: ProcessesAsk,
        verb: MeetVerb,
        caller: Option<Caller>,
    ) -> Result<Vec<ActivityEnded>, HostError> {
        self.script.call(
            "activity.meet",
            json!({ "threadIds": thread_ids, "processes": sj(&processes), "verb": dbg(&verb), "origin": oj(&caller) }),
        )
    }
}
