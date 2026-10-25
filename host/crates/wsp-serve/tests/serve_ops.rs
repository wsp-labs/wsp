// SPDX-License-Identifier: AGPL-3.0-only
//! The op switch's own ops: the prefix ops, sessions, images, usage, forwards, folders, agents, editors, the host's
//! doors and the catch. Each TS case is ported at this crate's seam: the area is scripted, and the test reads what the
//! socket sent and what the runtime was asked. Spec: spec/serve_ops.md.
mod support;
use support::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, Frame};
use wsp_core::devices::{DeviceDoor, HeldDevice};
use wsp_core::types::api::*;
use wsp_core::types::events::EventBus;
use wsp_core::types::pending::{ActivityDoor, DeviceDoor as RuntimeDevices, Slates};
use wsp_core::types::wiring::{
    EditorOpen, EditorRemote, FolderListAsk, HostEditor, HostFolders, HostSsh, HostTerminalConfig, InitDoor, LandRequest, LandedProject, PackedProject,
    PackedState, ProjectBundler, ProjectLander, RecipeShelf, StateRequest, SweepResult, SysListener,
};
use wsp_engine::machine::MachineBackend;
use wsp_engine::vault::CacheRule;
use wsp_frames::ProcKind;
use wsp_serve::serve::door::Served;
use wsp_serve::serve::served::{ForwardsSource, Payload, ServedRuntime, SocketHooks, SocketOps};
use wsp_serve::serve::socket::Socket;
use wsp_serve::serve::{
    FailedFn, ReleaseDoor, RestartDoor, ServeOptions, Unlisten, NO_BUNDLER, NO_EDITOR, NO_FOLDERS, NO_INIT, NO_LANDER, NO_RELEASE, NO_SSH,
    NO_TERMINAL_CONFIG,
};
use wsp_serve::serve_broker::{BrokerDoor, CloudSignIn};
use wsp_serve::serve_start::{AnswerFn, StartFn, StartFolder, StartRoads};
use wsp_wire::types::agents_report::{AGENTS_KEY_REFUSAL, SIGN_IN_LINE_REFUSAL};
use wsp_wire::types::init_job::InitKeys;
use wsp_wire::types::release::{HOST_NO_RESTART_LINE, HOST_RESTART_TICKET_REFUSAL};
use wsp_wire::types::thread_defaults::ENV_VALUE_REFUSAL;
use wsp_wire::views::place_more::{PlaceSpend, PlaceView};
use wsp_wire::views::preferences::{editor_opens_here_line, ssh_copy_here_line, ssh_include_line, EditorChoice, EditorId, EDITOR_TICKET_REFUSAL, SSH_TICKET_REFUSAL};
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_more::{WorkspaceOrigin, WorkspaceView};
use wsp_wire::wire::daemon_version::{place_behind_line, place_daemon_behind};
use wsp_wire::wire::limits::PortForward;
use wsp_wire::wire::machine_link::no_such_place_refusal;
use wsp_wire::wire::place_link::{PlaceReport, PLACES_TICKET_REFUSAL};
use wsp_wire::wire::runtime::SessionStartResult;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::{refusal, HostError, Kind};
use wsp_words::place_word::{place_said, PlaceRef};
use wsp_words::project_path::ssh_alias;
use wsp_words::views::activity::{processes_left_refusal, LeftVerb};
use wsp_words::words::units::provider_folders_refusal;
use wsp_words::workspace_state::absent_computer;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures and the rig.

fn from<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("fixture does not parse: {e}; {v}"))
}

fn args_of(rt: &FakeRuntime, name: &str, n: usize) -> Value {
    rt.script.calls_to(name).get(n).unwrap_or_else(|| panic!("no call {n} of {name}; calls: {:?}", rt.script.names())).args.clone()
}

fn keys(v: &Value) -> Vec<String> {
    v.as_object().expect("an object").keys().cloned().collect()
}

fn payload(v: Value) -> Payload {
    match v {
        Value::Object(m) => m,
        other => panic!("not an object: {other}"),
    }
}

const WHEN: &str = "2026-10-25T00:00:00.000Z";

fn workspace(id: &str, name: &str, kind: &str, phase: &str, extra: Value) -> WorkspaceView {
    let mut v = json!({
        "id": id, "name": name, "machineId": "m_1", "phase": phase, "kind": kind, "golden": "default", "createdAt": WHEN,
        "project": { "id": "pr_1", "name": "app", "path": "/app", "computer": "pl_1" },
    });
    for (k, x) in extra.as_object().expect("extra is an object") {
        v[k.as_str()] = x.clone();
    }
    from(v)
}

fn project(id: &str, computer: &str, checkout: Option<&str>) -> ProjectView {
    let mut v = json!({
        "id": id, "name": id, "computer": computer, "source": { "kind": "folder", "path": "/app" }, "path": "/app", "remote": "",
        "defaultBranch": "main", "memoryKey": "k", "memoryDir": "/m", "createdAt": WHEN,
    });
    if let Some(c) = checkout {
        v["checkout"] = json!(c);
    }
    from(v)
}

fn place(id: &str, name: &str, kind: &str) -> PlaceView {
    from(json!({ "id": id, "kind": kind, "name": name, "default": false }))
}

fn said(name: &str, kind: &str) -> String {
    place_said(&PlaceRef { id: name.into(), name: name.into(), kind: Some(kind.into()) })
}

fn target() -> Value {
    json!({ "placeId": "pl_1" })
}

struct Rig {
    rt: Rc<FakeRuntime>,
    served: Rc<Served>,
    failed: Rc<RefCell<Vec<(String, String)>>>,
    logs: Rc<RefCell<Vec<String>>>,
}

fn rig_over(rt: Rc<FakeRuntime>, tweak: impl FnOnce(&mut ServeOptions)) -> Rig {
    let failed: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let told = failed.clone();
    let (log, logs) = log_lines();
    let mut o = ServeOptions {
        devices: Some(rt.devices.clone() as Rc<dyn DeviceDoor>),
        failed: Some(Rc::new(move |op: &str, e: &HostError| told.borrow_mut().push((op.to_string(), e.message.clone()))) as FailedFn),
        log: Some(log),
        ..opts()
    };
    tweak(&mut o);
    let served = served_with(&rt, o);
    Rig { rt, served, failed, logs }
}

fn rig(tweak: impl FnOnce(&mut ServeOptions)) -> Rig {
    rig_over(FakeRuntime::new(), tweak)
}

fn held(scope: Option<Value>, road: Option<WorkspaceOrigin>) -> HeldDevice {
    let mut view = json!({ "id": "dev_1", "name": "laptop", "createdAt": WHEN, "lastSeenAt": WHEN });
    if let Some(s) = scope {
        view["scope"] = s;
    }
    HeldDevice { view: from(view), road }
}

impl Rig {
    async fn host(&self) -> (Rc<Socket>, Rc<RecordingSink>) {
        host_socket(&self.served).await
    }
    async fn as_device(&self, device: HeldDevice) -> (Rc<Socket>, Rc<RecordingSink>) {
        self.rt.script.once::<Result<Option<HeldDevice>, HostError>>("devices.match", Ok(Some(device)));
        let (socket, sink) = open(&self.served, "");
        let reply = request(&socket, &sink, 0, "auth", json!({ "token": "device-token" })).await;
        assert_eq!(reply["ok"], json!(true), "the device token lets the socket in: {reply}");
        (socket, sink)
    }
    /// A computer the person paired: on the own road, never the host.
    async fn paired(&self) -> (Rc<Socket>, Rc<RecordingSink>) {
        self.as_device(held(None, None)).await
    }
    /// A thread's own token, on the road its mint recorded.
    async fn thread(&self, workspace_id: &str) -> (Rc<Socket>, Rc<RecordingSink>) {
        let scope = json!({ "kind": "thread", "threadId": "th_1", "workspaceId": workspace_id, "rootThreadId": "th_0" });
        self.as_device(held(Some(scope), Some(WorkspaceOrigin::Here))).await
    }
    /// A socket let in on a relay ticket: not the person's own road.
    async fn ticketed(&self) -> (Rc<Socket>, Rc<RecordingSink>) {
        let (host, sink) = self.host().await;
        let minted = request(&host, &sink, 90, "ticket.issue", json!({ "purpose": "relay" })).await;
        open(&self.served, &format!("?ticket={}", minted["ticket"].as_str().expect("a ticket")))
    }
    fn failed_ops(&self) -> Vec<String> {
        self.failed.borrow().iter().map(|(op, _)| op.clone()).collect()
    }
}

fn ticket_reply(id: u64) -> Value {
    json!({ "id": id, "ok": false, "error": PLACES_TICKET_REFUSAL, "kind": "ticket" })
}

// ---------------------------------------------------------------------------------------------------------------
// The doors the host wires, as small fakes that record what they were asked.

struct Broker {
    github: Option<String>,
}

#[async_trait(?Send)]
impl BrokerDoor for Broker {
    async fn status(&self) -> Option<Result<wsp_wire::wire::runtime::BrokerStatus, HostError>> {
        Some(Ok(from(json!({ "github": self.github }))))
    }
    async fn check_key(&self, _provider: &str, _key: &str) -> Option<Result<wsp_wire::wire::runtime::ProviderKeyWorks, HostError>> {
        None
    }
    fn cloud_sign_in(&self) -> Option<CloudSignIn> {
        None
    }
}

struct Forwards {
    open: RefCell<Vec<PortForward>>,
    stops: RefCell<Vec<(String, u16)>>,
}

impl Forwards {
    fn with(open: Vec<Value>) -> Rc<Forwards> {
        Rc::new(Forwards { open: RefCell::new(open.into_iter().map(from).collect()), stops: RefCell::new(Vec::new()) })
    }
}

impl ForwardsSource for Forwards {
    fn list(&self) -> Vec<PortForward> {
        self.open.borrow().clone()
    }
    fn stop(&self, workspace_id: &str, port: u16) -> bool {
        self.stops.borrow_mut().push((workspace_id.to_string(), port));
        let mut open = self.open.borrow_mut();
        match open.iter().position(|f| f.workspace_id == workspace_id && f.port.get() == i64::from(port)) {
            Some(i) => {
                open.remove(i);
                true
            }
            None => false,
        }
    }
    fn on(&self, _f: Rc<dyn Fn(&Value)>) -> Box<dyn FnOnce()> {
        Box::new(|| {})
    }
}

fn forward(workspace_id: &str, port: u16) -> Value {
    json!({ "workspaceId": workspace_id, "port": port, "startedAt": "2026-09-04T10:00:00.000Z", "name": "api", "kind": "url" })
}

struct Folders {
    asked: RefCell<Vec<Value>>,
}

#[async_trait(?Send)]
impl HostFolders for Folders {
    async fn list(&self, req: FolderListAsk) -> Result<wsp_wire::views::project_bundle::HostFolderListing, HostError> {
        let mut asked = serde_json::Map::new();
        if let Some(d) = req.dir {
            asked.insert("dir".into(), json!(d));
        }
        if let Some(h) = req.hidden {
            asked.insert("hidden".into(), json!(h));
        }
        if let Some(r) = req.repos {
            asked.insert("repos".into(), json!(r));
        }
        if let Some(w) = req.wide {
            asked.insert("wide".into(), json!(w));
        }
        self.asked.borrow_mut().push(Value::Object(asked));
        Ok(from(listing()))
    }
}

fn listing() -> Value {
    json!({ "dir": "/home/me", "roots": ["/home/me"], "folders": [], "hidden": 0 })
}

struct Terminal {
    schemes: RefCell<Vec<Option<wsp_wire::views::project_bundle::TerminalScheme>>>,
}

#[async_trait(?Send)]
impl HostTerminalConfig for Terminal {
    async fn read(&self, scheme: Option<wsp_wire::views::project_bundle::TerminalScheme>) -> Result<wsp_wire::views::project_bundle::TerminalConfig, HostError> {
        self.schemes.borrow_mut().push(scheme);
        Ok(from(json!({ "files": [], "fontFamily": ["Menlo"], "palette": [] })))
    }
}

struct Editor {
    log: Rc<RefCell<Vec<String>>>,
    opens: RefCell<Vec<EditorOpen>>,
    remote_refusal: Option<String>,
}

#[async_trait(?Send)]
impl HostEditor for Editor {
    async fn list(&self) -> Result<Vec<EditorChoice>, HostError> {
        self.log.borrow_mut().push("list".into());
        Ok(vec![from(json!({ "id": "vscode", "name": "VS Code" }))])
    }
    async fn open(&self, req: EditorOpen) -> Result<EditorId, HostError> {
        self.log.borrow_mut().push("open".into());
        let id = req.editor.unwrap_or(EditorId::Vscode);
        self.opens.borrow_mut().push(req);
        Ok(id)
    }
    async fn remote_refusal(&self, editor: Option<EditorId>, name: &str) -> Result<Option<String>, HostError> {
        self.log.borrow_mut().push(format!("remoteRefusal {editor:?} {name}"));
        Ok(self.remote_refusal.clone())
    }
}

struct Ssh {
    log: Rc<RefCell<Vec<String>>>,
    include: Cell<bool>,
}

#[async_trait(?Send)]
impl HostSsh for Ssh {
    async fn port(&self, id: &str, name: &str) -> Result<u16, HostError> {
        self.log.borrow_mut().push(format!("port {id} {name}"));
        Ok(2222)
    }
    async fn include(&self) -> Result<bool, HostError> {
        self.log.borrow_mut().push("include".into());
        Ok(self.include.get())
    }
    async fn set_include(&self, on: bool) -> Result<bool, HostError> {
        self.log.borrow_mut().push(format!("setInclude {on}"));
        self.include.set(on);
        Ok(on)
    }
}

fn setup() -> Value {
    json!({ "keys": { "box": false, "solari": true }, "keyProvider": "solari", "home": "/Users/me", "agents": [], "pricing": null, "job": null })
}

struct Init {
    calls: RefCell<Vec<Value>>,
}

#[async_trait(?Send)]
impl InitDoor for Init {
    async fn get(&self, on: Option<String>) -> Result<wsp_wire::wire::capabilities::InitSetup, HostError> {
        self.calls.borrow_mut().push(json!(["get", on]));
        Ok(from(setup()))
    }
    async fn keys(&self, provider: Option<String>, key: Option<String>) -> Result<wsp_wire::wire::capabilities::InitSetup, HostError> {
        self.calls.borrow_mut().push(json!(["keys", provider, key]));
        let mut keys: InitKeys = from(json!({ "box": false, "solari": true }));
        if let Some(p) = &provider {
            keys.insert(p.clone(), true);
        }
        let mut s = setup();
        s["keys"] = serde_json::to_value(&keys).unwrap();
        Ok(from(s))
    }
}

struct RefusingInit(HostError);

#[async_trait(?Send)]
impl InitDoor for RefusingInit {
    async fn get(&self, _on: Option<String>) -> Result<wsp_wire::wire::capabilities::InitSetup, HostError> {
        Err(self.0.clone())
    }
    async fn keys(&self, _provider: Option<String>, _key: Option<String>) -> Result<wsp_wire::wire::capabilities::InitSetup, HostError> {
        Err(self.0.clone())
    }
}

struct Release {
    checks: RefCell<Vec<bool>>,
}

#[async_trait(?Send)]
impl ReleaseDoor for Release {
    fn get(&self) -> wsp_wire::types::release::ReleaseView {
        from(json!({ "state": "read", "installed": "0.3.7" }))
    }
    async fn check(&self, force: bool) -> Result<wsp_wire::types::release::ReleaseView, HostError> {
        self.checks.borrow_mut().push(force);
        Ok(from(json!({ "state": "read", "installed": "0.3.8" })))
    }
    fn on(&self, _f: Rc<dyn Fn(&wsp_wire::types::release::ReleaseChangedEvent)>) -> Unlisten {
        Box::new(|| {})
    }
}

struct Bundler {
    source: String,
}

#[async_trait(?Send)]
impl ProjectBundler for Bundler {
    async fn plan(&self) -> Result<wsp_wire::views::project_bundle::ProjectPlan, HostError> {
        Ok(from(json!({ "source": self.source, "repo": true, "files": 3, "bytes": 10, "secrets": [], "excluded": [], "skipped": [], "agents": [] })))
    }
    async fn pack(&self, _carry: &[String], _rewrite: &[String], _provider: bool) -> Result<PackedProject, HostError> {
        Err(HostError::new("the test's bundler packs nothing"))
    }
    async fn pack_state(&self, _req: StateRequest) -> Result<PackedState, HostError> {
        Err(HostError::new("the test's bundler packs nothing"))
    }
}

struct Lander;

#[async_trait(?Send)]
impl ProjectLander for Lander {
    fn caches(&self) -> CacheRule {
        CacheRule::default()
    }
    async fn probe(&self, _dest: &str) -> Result<Option<u64>, HostError> {
        Ok(None)
    }
    async fn land(&self, _req: LandRequest) -> Result<LandedProject, HostError> {
        Err(HostError::new("the test's lander lands nothing"))
    }
}

/// A link to a computer's daemon: records every frame and every close, answers what the test set.
struct Link {
    sent: RefCell<Vec<Frame>>,
    closes: Cell<u32>,
    reply: RefCell<Option<Result<Frame, ChannelError>>>,
}

fn link(reply: Result<Value, ChannelError>) -> Rc<Link> {
    Rc::new(Link { sent: RefCell::new(Vec::new()), closes: Cell::new(0), reply: RefCell::new(Some(reply.map(payload))) })
}

#[async_trait(?Send)]
impl DaemonChannel for Link {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        self.sent.borrow_mut().push(frame);
        self.reply.borrow_mut().take().expect("the link answers once")
    }
    fn close(&self) {
        self.closes.set(self.closes.get() + 1);
    }
    async fn closed(&self) -> ChannelEnd {
        ChannelEnd { code: 1000, reason: String::new() }
    }
}

/// A start road that answers what the test hands it and then stops.
struct AnswerRoads {
    answers: Vec<SessionStartResult>,
    held_asked: Cell<Option<bool>>,
}

#[async_trait(?Send)]
impl StartRoads for AnswerRoads {
    async fn start_at(&self, _w: &dyn WorkspacesApi, _msg: &RuntimeRequest, _origin: Option<Caller>, _opens: bool) -> Result<StartFolder, HostError> {
        Ok(StartFolder { workspace_id: "ws_1".into(), cwd: None, setup: None })
    }
    async fn answered_start(&self, answer_held: bool, _start: StartFn, answer: AnswerFn) -> Result<(), HostError> {
        self.held_asked.set(Some(answer_held));
        for a in &self.answers {
            answer(a.clone());
        }
        Ok(())
    }
}

/// The fake runtime with its start road swapped: every other call goes to the fake.
struct WithRoads {
    inner: Rc<FakeRuntime>,
    roads: Rc<dyn StartRoads>,
}

#[async_trait(?Send)]
impl Runtime for WithRoads {
    fn events(&self) -> Rc<dyn EventBus> {
        self.inner.events()
    }
    fn backend(&self) -> Arc<dyn MachineBackend> {
        self.inner.backend()
    }
    fn places(&self) -> Option<Rc<dyn wsp_core::types::pending::PlaceDoor>> {
        self.inner.places()
    }
    fn recipes(&self) -> Option<Rc<dyn RecipeShelf>> {
        self.inner.recipes()
    }
    async fn here_channel(&self, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        self.inner.here_channel(on_event).await
    }
    fn place_kills(&self, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        self.inner.place_kills(channel)
    }
    async fn watch_place_sys(&self, place_id: &str, listener: SysListener) -> Result<wsp_core::types::wiring::Detach, HostError> {
        self.inner.watch_place_sys(place_id, listener).await
    }
    fn own_process(&self, thread_id: &str, pid: u32, kind: ProcKind) -> wsp_core::types::wiring::Detach {
        self.inner.own_process(thread_id, pid, kind)
    }
    fn agents(&self) -> &dyn AgentsApi {
        self.inner.agents()
    }
    fn workspaces(&self) -> &dyn WorkspacesApi {
        self.inner.workspaces()
    }
    fn projects(&self) -> &dyn ProjectsApi {
        self.inner.projects()
    }
    fn sessions(&self) -> &dyn SessionsApi {
        self.inner.sessions()
    }
    fn harnesses(&self) -> &dyn HarnessesApi {
        self.inner.harnesses()
    }
    fn golden(&self) -> &dyn GoldenApi {
        self.inner.golden()
    }
    fn image(&self) -> &dyn ImageApi {
        self.inner.image()
    }
    fn status(&self) -> Rc<dyn wsp_core::types::wiring::OriginStatusApi> {
        self.inner.status()
    }
    fn usage(&self) -> Rc<dyn wsp_core::types::wiring::UsageDoor> {
        self.inner.usage()
    }
    fn conversations(&self) -> &dyn ConversationsApi {
        self.inner.conversations()
    }
    fn slates(&self) -> Rc<dyn Slates> {
        self.inner.slates()
    }
    fn activity(&self) -> Rc<dyn ActivityDoor> {
        self.inner.activity()
    }
    fn devices(&self) -> Rc<dyn RuntimeDevices> {
        self.inner.devices()
    }
    fn preferences(&self) -> &dyn PreferencesApi {
        self.inner.preferences()
    }
    async fn owner(&self) -> Result<String, HostError> {
        self.inner.owner().await
    }
    async fn reap(&self, older_than_ms: Option<u64>, say: Option<Rc<dyn Fn(&str)>>) -> Result<SweepResult, HostError> {
        self.inner.reap(older_than_ms, say).await
    }
    async fn close(&self) -> Result<(), HostError> {
        self.inner.close().await
    }
}

impl ServedRuntime for WithRoads {
    fn place_door(&self) -> Option<Rc<dyn wsp_machines::places::types::PlaceDoor>> {
        self.inner.place_door()
    }
    fn slate_door(&self) -> Rc<dyn wsp_serve::serve_slates::SlateDoor> {
        self.inner.slate_door()
    }
    fn activity_door(&self) -> Rc<dyn wsp_serve::serve_processes::ActivityOps> {
        self.inner.activity_door()
    }
    fn sys_ops(&self, socket: SocketHooks) -> Rc<dyn SocketOps> {
        self.inner.sys_ops(socket)
    }
    fn sign_in_ops(&self, socket: SocketHooks, cloud: Option<CloudSignIn>) -> Rc<dyn SocketOps> {
        self.inner.sign_in_ops(socket, cloud)
    }
    fn is_sign_in_op(&self, msg: &RuntimeRequest) -> bool {
        self.inner.is_sign_in_op(msg)
    }
    fn forwards(&self, host: Option<Rc<dyn ForwardsSource>>) -> Rc<dyn ForwardsSource> {
        ServedRuntime::forwards(&*self.inner, host)
    }
    fn start_roads(&self) -> Rc<dyn StartRoads> {
        self.roads.clone()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// O1. The prefix ops.

// O1. serve.ts:869-878
#[test]
fn prefix_ops_spread_their_helpers_answer() {
    block(async {
        let r = rig(|o| o.broker = Some(Rc::new(Broker { github: Some("zingzy".into()) })));
        let rt = &r.rt;
        let (host, sink) = r.host().await;

        rt.script.once::<Result<Value, HostError>>("slates.get", Ok(json!({ "v": 2 })));
        request(&host, &sink, 1, "slates.get", json!({ "threadId": "t1" })).await;
        assert_eq!(sink.last_text(), r#"{"id":1,"ok":true,"slate":{"v":2}}"#);

        rt.script.once::<Result<IndexMap<String, wsp_wire::types::thread_defaults::ThreadDefaults>, HostError>>("projects.defaults", Ok(IndexMap::new()));
        request(&host, &sink, 2, "projects.defaults", json!({})).await;
        assert_eq!(sink.last_text(), r#"{"id":2,"ok":true,"defaults":{}}"#);

        request(&host, &sink, 3, "appProcs.watch", json!({})).await;
        assert_eq!(sink.last_text(), r#"{"id":3,"ok":true,"procs":[]}"#, "the hub's answer, from the socket's own road");

        rt.script.once::<Result<Payload, HostError>>("activity.end", Ok(payload(json!({ "ended": [4200] }))));
        request(&host, &sink, 4, "activity.end", json!({ "pid": 4200, "startedAt": 7 })).await;
        assert_eq!(sink.last_text(), r#"{"id":4,"ok":true,"ended":[4200]}"#);

        rt.script.once::<Result<wsp_wire::types::conversations::ConversationsAnswer, HostError>>("conversations.list", Ok(from(json!({ "rows": [], "held": [] }))));
        request(&host, &sink, 5, "conversations.list", json!({})).await;
        assert_eq!(sink.last_text(), r#"{"id":5,"ok":true,"rows":[],"held":[]}"#);

        request(&host, &sink, 6, "broker.status", json!({})).await;
        assert_eq!(sink.last_text(), r#"{"id":6,"ok":true,"github":"zingzy"}"#);
        assert!(r.failed.borrow().is_empty());
    })
}

// O1. serve.ts:877
#[test]
fn sign_in_ops_are_the_own_road() {
    block(async {
        let r = rig(|_| {});
        let (host, host_sink) = r.host().await;
        let reply = request(&host, &host_sink, 1, "agents.signIns", json!({})).await;
        assert_eq!(reply, json!({ "id": 1, "ok": true }));
        assert_eq!(r.rt.script.calls_to("signIn.answer").len(), 1);
        assert_eq!(args_of(&r.rt, "signIn.answer", 0)["op"], json!("agents.signIns"));

        let (ticketed, ticket_sink) = r.ticketed().await;
        let refused = request(&ticketed, &ticket_sink, 1, "agents.signIns", json!({})).await;
        assert_eq!(refused, ticket_reply(1));
        assert_eq!(r.rt.script.calls_to("signIn.answer").len(), 1, "a socket off the own road reaches no sign-in road");
        let stop = request(&ticketed, &ticket_sink, 2, "agents.signInStop", json!({ "signInId": "si_1" })).await;
        assert_eq!(stop, ticket_reply(2));
        assert!(r.failed.borrow().is_empty(), "a refusal the switch sends is not a failed op");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O2. The catch.

// O2. serve-refusal.test.ts:50
#[test]
fn a_failed_op_carries_kind_fix_and_left_and_tells_failed() {
    block(async {
        let r = rig(|_| {});
        let left: Vec<wsp_wire::views::activity::LeftThread> = from(json!([{
            "threadId": "t-1", "title": "fix login", "workspaceId": "ws-1", "memory": 100, "cpu": 0,
            "processes": [{ "pid": 4200, "ppid": 1, "depth": 0, "startedAt": 1, "kind": "dev", "name": "vite", "command": "node vite", "memory": 100, "cpu": 0, "ports": [5173] }],
        }]));
        let e = processes_left_refusal(LeftVerb::Settle, &left, None);
        r.rt.script.once::<Result<wsp_wire::wire::runtime::SessionSettleResult, HostError>>("sessions.settle", Err(e.clone()));
        let (host, sink) = r.host().await;
        let reply = request(&host, &sink, 1, "sessions.settle", json!({ "threadIds": ["t-1"], "processes": "ask" })).await;
        let expected = json!({
            "id": 1, "ok": false, "error": e.message, "kind": "processes-left", "fix": e.fix.clone().unwrap(), "left": serde_json::to_value(&left).unwrap(),
        });
        assert_eq!(reply, expected);
        assert_eq!(reply.to_string(), expected.to_string(), "error, kind, fix, then left");
        assert_eq!(*r.failed.borrow(), vec![("sessions.settle".to_string(), e.message.clone())]);

        let still = request(&host, &sink, 2, "sessions.list", json!({})).await;
        assert_eq!(still["ok"], json!(false), "the socket stays and answers the next frame (here: unscripted)");
    })
}

// O2. serve-refusal.test.ts:42
#[test]
fn a_refusal_carries_the_fix_beside_the_sentence() {
    block(async {
        let key = "sk-ant-x-fake-refusal-key";
        let e = refusal(&format!("Box refused {key}\nits answer was 401"), "Check it and paste it again.", Some(Kind::Auth));
        let door = Rc::new(RefusingInit(e.clone()));
        let r = rig(|o| o.init = Some(door));
        let (host, sink) = r.host().await;
        let reply = request(&host, &sink, 1, "init.keys", json!({ "provider": "box", "key": key })).await;
        let expected = json!({ "id": 1, "ok": false, "error": e.message, "kind": "auth", "fix": "Check it and paste it again." });
        assert_eq!(reply.to_string(), expected.to_string());
        assert_eq!(*r.failed.borrow(), vec![("init.keys".to_string(), e.message.clone())]);
        let logged = r.logs.borrow().clone();
        assert_eq!(logged.len(), 1, "one line for the refusal, after it was sent");
        assert!(logged[0].starts_with("refused init.keys kind=auth: Box refused "));
        assert!(!logged[0].contains(key), "the key the request carried is never logged");
    })
}

// O2. A failure with no kind and no fix carries its sentence alone; a slate write's lists ride beside it.
#[test]
fn a_failure_carries_only_what_it_has() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        r.rt.script.once::<Result<Payload, HostError>>("slates.write", Err(HostError::new("boom")));
        let plain = request(&host, &sink, 1, "slates.write", json!({ "threadId": "t1", "text": "x" })).await;
        assert_eq!(plain.to_string(), json!({ "id": 1, "ok": false, "error": "boom" }).to_string());

        let bad = HostError {
            errors: Some(vec![json!({ "path": "rows", "said": "a row needs a label" })]),
            warnings: Some(vec![json!({ "path": "title", "said": "long" })]),
            ..HostError::new("the slate is not valid")
        };
        r.rt.script.once::<Result<Payload, HostError>>("slates.write", Err(bad));
        let lists = request(&host, &sink, 2, "slates.write", json!({ "threadId": "t1", "text": "x" })).await;
        let expected = json!({
            "id": 2, "ok": false, "error": "the slate is not valid",
            "errors": [{ "path": "rows", "said": "a row needs a label" }], "warnings": [{ "path": "title", "said": "long" }],
        });
        assert_eq!(lists.to_string(), expected.to_string());
        assert_eq!(r.failed_ops(), vec!["slates.write", "slates.write"]);

        let not_left = HostError { left: Some(vec![json!({ "threadId": "t" })]), ..HostError::with_kind("no", Kind::Usage) };
        r.rt.script.once::<Result<Payload, HostError>>("slates.write", Err(not_left));
        let no_left = request(&host, &sink, 3, "slates.write", json!({ "threadId": "t1", "text": "x" })).await;
        assert!(no_left.get("left").is_none(), "left rides only on a processes-left refusal");
        assert_eq!(no_left["kind"], json!("usage"));
    })
}

// O2, R5. A refusal the switch sends itself is not told to failed.
#[test]
fn a_refusal_the_switch_sends_is_not_told_to_failed() {
    block(async {
        let r = rig(|_| {});
        let (ticketed, sink) = r.ticketed().await;
        request(&ticketed, &sink, 1, "usage.accounts", json!({})).await;
        let (host, host_sink) = r.host().await;
        request(&host, &host_sink, 2, "places.readings", json!({ "range": "day" })).await;
        assert!(r.failed.borrow().is_empty());
        request(&host, &host_sink, 3, "sessions.list", json!({})).await;
        assert_eq!(r.failed_ops(), vec!["sessions.list"], "only the catch tells it");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O3. A door the host did not wire.

// O3. serve.test.ts:1186
#[test]
fn each_unwired_door_refuses_in_its_own_sentence() {
    block(async {
        let r = rig(|_| {});
        r.rt.script.on::<Result<WorkspaceView, HostError>>("workspaces.get", |_| Ok(workspace("ws_1", "app", "local", "running", json!({}))));
        let (host, sink) = r.host().await;
        let cases: Vec<(&str, Value, &str)> = vec![
            ("host.folders", json!({}), NO_FOLDERS),
            ("host.terminalConfig", json!({}), NO_TERMINAL_CONFIG),
            ("editor.list", json!({}), NO_EDITOR),
            ("editor.open", json!({ "workspaceId": "ws_1", "path": "a.rs", "editor": "zed" }), NO_EDITOR),
            ("ssh.include", json!({}), NO_SSH),
            ("ssh.port", json!({ "workspaceId": "ws_1" }), NO_SSH),
            ("release.get", json!({}), NO_RELEASE),
            ("release.check", json!({}), NO_RELEASE),
            ("init.get", json!({}), NO_INIT),
            ("init.keys", json!({ "provider": "box" }), NO_INIT),
            ("project.plan", json!({ "source": "/src" }), NO_BUNDLER),
            ("project.import", json!({ "workspaceId": "ws_1", "source": "/src", "dest": "/d" }), NO_BUNDLER),
            ("project.export", json!({ "workspaceId": "ws_1", "source": "/src", "dest": "/d" }), NO_LANDER),
        ];
        for (i, (op, fields, sentence)) in cases.iter().enumerate() {
            let id = (i + 1) as u64;
            let reply = request(&host, &sink, id, op, fields.clone()).await;
            assert_eq!(reply.to_string(), json!({ "id": id, "ok": false, "error": sentence }).to_string(), "{op}");
        }
        let ops: Vec<String> = cases.iter().map(|(op, _, _)| op.to_string()).collect();
        assert_eq!(r.failed_ops(), ops, "each is the catch's, told in turn");
        assert!(r.rt.script.calls_to("projects.import").is_empty() && r.rt.script.calls_to("projects.export").is_empty(), "the door is read before the area is asked");
        assert_eq!(r.rt.script.calls_to("workspaces.get").len(), 1, "ssh.port reads its door before the workspace; only editor.open read one");
        let (_, again) = r.host().await;
        assert!(again.frames().len() == 1, "the host started and served without any of them");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Sessions.

fn started(outcome: &str) -> SessionStartResult {
    from(json!({
        "session": { "id": "s1", "workspaceId": "ws_1", "harness": "claude", "status": "running" },
        "outcome": outcome, "turnId": "tu_1",
    }))
}

// O4. serve.test.ts:634
#[test]
fn sessions_start_sends_every_answer_under_one_id() {
    block(async {
        let rt = FakeRuntime::new();
        let roads = Rc::new(AnswerRoads { answers: vec![started("held"), started("started")], held_asked: Cell::new(None) });
        let wrapped = Rc::new(WithRoads { inner: rt.clone(), roads: roads.clone() });
        let served = Served::new(wrapped, opts()).expect("a host token serves");
        let (host, sink) = host_socket(&served).await;
        send(&host, json!({ "id": 7, "op": "sessions.start", "prompt": "go", "project": "app", "answerHeld": true })).await;
        let replies = sink.replies(7);
        assert_eq!(replies.len(), 2, "held, then started");
        assert_eq!(replies[0]["outcome"], json!("held"));
        assert_eq!(replies[1]["outcome"], json!("started"));
        for reply in &replies {
            assert_eq!(&keys(reply)[..2], ["id", "ok"]);
            assert_eq!(reply["ok"], json!(true));
            assert_eq!(reply["turnId"], json!("tu_1"));
        }
        assert_eq!(roads.held_asked.get(), Some(true), "answerHeld rides to the start road");
    })
}

// O5. thread-warm-agent.test.ts
#[test]
fn sessions_warm_strips_the_folder_fields() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        rt.script.once::<Result<StartFolder, HostError>>("startRoads.startAt", Ok(StartFolder { workspace_id: "ws_1".into(), cwd: Some("/at".into()), setup: None }));
        rt.script.once::<Result<wsp_wire::wire::runtime::SessionWarmResult, HostError>>("sessions.warm", Ok(from(json!({ "warm": "started" }))));
        let (host, sink) = r.host().await;
        let frame = json!({ "workspaceId": "ws_1", "project": "app", "cwd": "/ignored", "harness": "claude", "model": "opus", "fast": true });
        let reply = request(&host, &sink, 1, "sessions.warm", frame.clone()).await;
        assert_eq!(reply.to_string(), r#"{"id":1,"ok":true,"warm":"started"}"#);
        let ask = WarmAsk { harness: Some("claude".into()), cwd: Some("/at".into()), model: Some("opus".into()), fast: Some(true), ..Default::default() };
        let warm = args_of(rt, "sessions.warm", 0);
        assert_eq!(warm["workspaceId"], json!("ws_1"));
        assert_eq!(warm["ask"], json!(format!("{ask:?}")), "the folder's cwd, not the frame's; no project");
        let at = args_of(rt, "startRoads.startAt", 0);
        assert_eq!(at["opens"], json!(false));
        assert_eq!(at["msg"]["project"], json!("app"), "the folder is found from the frame as sent");

        rt.script.once::<Result<StartFolder, HostError>>("startRoads.startAt", Ok(StartFolder { workspace_id: "ws_2".into(), cwd: None, setup: None }));
        rt.script.once::<Result<wsp_wire::wire::runtime::SessionWarmResult, HostError>>("sessions.warm", Ok(from(json!({ "warm": "none" }))));
        request(&host, &sink, 2, "sessions.warm", frame).await;
        let bare = WarmAsk { harness: Some("claude".into()), model: Some("opus".into()), fast: Some(true), ..Default::default() };
        assert_eq!(args_of(rt, "sessions.warm", 1)["ask"], json!(format!("{bare:?}")), "no cwd from the folder, none from the frame");
    })
}

// O6. serve.test.ts:508
#[test]
fn harnesses_list_passes_the_catalog_place_only_when_named() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        r.rt.script.once::<Result<Vec<wsp_wire::views::harness::HarnessCatalog>, HostError>>("harnesses.list", Ok(vec![]));
        r.rt.script.once::<Result<Vec<wsp_wire::views::harness::HarnessCatalog>, HostError>>("harnesses.list", Ok(vec![]));
        let plain = request(&host, &sink, 1, "harnesses.list", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(plain.to_string(), r#"{"id":1,"ok":true,"harnesses":[]}"#);
        request(&host, &sink, 2, "harnesses.list", json!({ "project": "app", "computer": "pl_1" })).await;
        let first = args_of(&r.rt, "harnesses.list", 0);
        assert_eq!(first["workspaceId"], json!("ws_1"));
        assert_eq!(first["at"], json!(format!("{:?}", CatalogAt::default())), "nothing named, nothing passed");
        let second = args_of(&r.rt, "harnesses.list", 1);
        assert_eq!(second["workspaceId"], Value::Null);
        assert_eq!(second["at"], json!(format!("{:?}", CatalogAt { project: Some("app".into()), computer: Some("pl_1".into()) })));
    })
}

// O7. serve.test.ts:388
#[test]
fn history_with_a_thread_reads_a_page() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let page = json!({ "events": [], "pos": 3, "total": 3 });
        r.rt.script.once::<Result<wsp_wire::views::preferences::HistoryPage, HostError>>("sessions.page", Ok(from(page.clone())));
        r.rt.script.once::<Result<wsp_wire::views::preferences::HistoryPage, HostError>>("sessions.page", Ok(from(page)));
        let reply = request(&host, &sink, 1, "sessions.history", json!({ "workspaceId": "ws_1", "threadId": "t1", "before": 3, "limit": 2 })).await;
        assert_eq!(reply.to_string(), r#"{"id":1,"ok":true,"events":[],"pos":3,"total":3}"#, "the page is spread beside id and ok");
        request(&host, &sink, 2, "sessions.history", json!({ "workspaceId": "ws_1", "threadId": "t1" })).await;
        let window = |w: PageWindow| json!(format!("{w:?}"));
        assert_eq!(args_of(&r.rt, "sessions.page", 0)["window"], window(PageWindow { thread_id: "t1".into(), before: Some(3), limit: Some(2) }));
        assert_eq!(args_of(&r.rt, "sessions.page", 1)["window"], window(PageWindow { thread_id: "t1".into(), before: None, limit: None }));
        assert!(r.rt.script.calls_to("sessions.history").is_empty() && r.rt.script.calls_to("sessions.threadHistory").is_empty());
    })
}

// O7. serve.test.ts:388
#[test]
fn history_with_a_window_and_no_thread_is_usage() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let reply = request(&host, &sink, 1, "sessions.history", json!({ "workspaceId": "ws_1", "limit": 3 })).await;
        let expected = json!({
            "id": 1, "ok": false, "error": "before and limit page one thread's events. Name the thread with threadId.",
            "kind": "usage", "fix": "Name the thread with threadId.",
        });
        assert_eq!(reply.to_string(), expected.to_string());
        let before = request(&host, &sink, 2, "sessions.history", json!({ "workspaceId": "ws_1", "before": 3 })).await;
        assert_eq!(before["kind"], json!("usage"));
        assert!(r.rt.script.names().iter().all(|n| !n.starts_with("sessions.")), "nothing is read for the refused window");
        assert_eq!(r.failed_ops(), vec!["sessions.history", "sessions.history"]);
    })
}

// O7. serve.test.ts:426
#[test]
fn history_whole_reads_thread_history() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let events = || Ok(Vec::<wsp_wire::views::session_events_more::SessionEvent>::new());
        r.rt.script.once::<Result<Vec<wsp_wire::views::session_events_more::SessionEvent>, HostError>>("sessions.threadHistory", events());
        r.rt.script.once::<Result<Vec<wsp_wire::views::session_events_more::SessionEvent>, HostError>>("sessions.threadHistory", events());
        let one = request(&host, &sink, 1, "sessions.history", json!({ "workspaceId": "ws_1", "threadId": "t1", "whole": true })).await;
        assert_eq!(one.to_string(), r#"{"id":1,"ok":true,"events":[]}"#);
        request(&host, &sink, 2, "sessions.history", json!({ "workspaceId": "ws_1", "whole": true, "before": 4 })).await;
        let asked = |a: HistoryAsk| json!(format!("{a:?}"));
        assert_eq!(args_of(&r.rt, "sessions.threadHistory", 0)["asked"], asked(HistoryAsk { thread_id: Some("t1".into()), before: None, limit: None }));
        assert_eq!(
            args_of(&r.rt, "sessions.threadHistory", 1)["asked"],
            asked(HistoryAsk { thread_id: None, before: Some(4), limit: None }),
            "the area, not the switch, refuses whole with a window and no thread"
        );
        assert!(r.rt.script.calls_to("sessions.page").is_empty() && r.rt.script.calls_to("sessions.history").is_empty());
    })
}

// O7. serve.test.ts:374
#[test]
fn history_plain_reads_the_folder() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        r.rt.script.once::<Result<Vec<wsp_wire::views::session_events_more::SessionEvent>, HostError>>("sessions.history", Ok(vec![]));
        let reply = request(&host, &sink, 1, "sessions.history", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(reply.to_string(), r#"{"id":1,"ok":true,"events":[]}"#);
        assert_eq!(args_of(&r.rt, "sessions.history", 0)["workspaceId"], json!("ws_1"));
        let missing = request(&host, &sink, 2, "sessions.history", json!({ "workspaceId": "ws_nope" })).await;
        assert_eq!(missing["ok"], json!(false), "an unknown workspace is the area's refusal");
    })
}

// O8. serve.test.ts:661
#[test]
fn session_ops_pass_only_the_fields_sent() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let (host, sink) = r.host().await;
        let stop = || HostError::new("stop");

        rt.script.once::<Result<Vec<wsp_wire::views::session::SessionView>, HostError>>("sessions.list", Ok(vec![]));
        let listed = request(&host, &sink, 1, "sessions.list", json!({})).await;
        assert_eq!(listed.to_string(), r#"{"id":1,"ok":true,"sessions":[]}"#);
        assert_eq!(args_of(rt, "sessions.list", 0)["workspaceId"], Value::Null);

        rt.script.once::<Result<wsp_wire::wire::runtime::SessionInterruptResult, HostError>>("sessions.interrupt", Ok(from(json!({ "outcome": "accepted" }))));
        let interrupted = request(&host, &sink, 2, "sessions.interrupt", json!({ "sessionId": "s1", "task": "build" })).await;
        assert_eq!(interrupted, json!({ "id": 2, "ok": true, "outcome": "accepted" }));
        let a = args_of(rt, "sessions.interrupt", 0);
        assert_eq!((a["sessionId"].clone(), a["task"].clone()), (json!("s1"), json!("build")));

        rt.script.once::<Result<wsp_wire::wire::runtime::SessionAnswerResult, HostError>>("sessions.answer", Ok(from(json!({ "outcome": "answered" }))));
        let answered = request(&host, &sink, 3, "sessions.answer", json!({ "sessionId": "s1", "askId": "a1", "optionId": "o1" })).await;
        assert_eq!(answered, json!({ "id": 3, "ok": true, "outcome": "answered" }));
        let plain = AnswerAsk { ask_id: "a1".into(), option_id: "o1".into(), reason: None };
        assert_eq!(args_of(rt, "sessions.answer", 0)["ask"], json!(format!("{plain:?}")), "no reason, none passed");

        rt.script.once::<Result<wsp_wire::wire::runtime::SessionSteerResult, HostError>>("sessions.steer", Err(stop()));
        request(&host, &sink, 4, "sessions.steer", json!({ "sessionId": "s1", "prompt": "and this" })).await;
        let steer = SteerAsk { prompt: "and this".into(), request_id: None, attachments: None };
        assert_eq!(args_of(rt, "sessions.steer", 0)["ask"], json!(format!("{steer:?}")));

        rt.script.once::<Result<wsp_wire::wire::runtime::SessionRewindResult, HostError>>("sessions.rewind", Err(stop()));
        request(&host, &sink, 5, "sessions.rewind", json!({ "threadId": "t1", "turnId": "tu_1", "files": false })).await;
        let rewind = RewindAsk { turn_id: Some("tu_1".into()), files: Some(false), undo: None };
        assert_eq!(args_of(rt, "sessions.rewind", 0)["ask"], json!(format!("{rewind:?}")), "a false files is sent, an absent undo is not");

        rt.script.once::<Result<(), HostError>>("sessions.forget", Ok(()));
        rt.script.once::<Result<(), HostError>>("sessions.forget", Ok(()));
        let forgot = request(&host, &sink, 6, "sessions.forget", json!({ "threadId": "t1", "check": false })).await;
        assert_eq!(forgot, json!({ "id": 6, "ok": true }));
        request(&host, &sink, 7, "sessions.forget", json!({ "threadId": "t1", "check": true })).await;
        assert_eq!(args_of(rt, "sessions.forget", 0)["check"], Value::Null, "check rides only when true");
        assert_eq!(args_of(rt, "sessions.forget", 1)["check"], json!(true));

        rt.script.once::<Result<(), HostError>>("sessions.read", Ok(()));
        rt.script.once::<Result<(), HostError>>("sessions.replaceable", Ok(()));
        rt.script.once::<Result<(), HostError>>("sessions.mark", Ok(()));
        for (id, op, fields) in [
            (8, "sessions.read", json!({ "threadId": "t1" })),
            (9, "sessions.replaceable", json!({ "threadId": "t1" })),
            (10, "sessions.mark", json!({ "threadIds": ["t1", "t2"], "marks": { "resumeAtReset": true } })),
        ] {
            assert_eq!(request(&host, &sink, id, op, fields).await, json!({ "id": id, "ok": true }), "{op}");
        }
        let mark = args_of(rt, "sessions.mark", 0);
        assert_eq!((mark["threadIds"].clone(), mark["marks"].clone()), (json!(["t1", "t2"]), json!({ "resumeAtReset": true })));

        rt.script.once::<Result<wsp_wire::views::preferences::ThreadHead, HostError>>("sessions.head", Err(stop()));
        rt.script.once::<Result<wsp_wire::views::preferences::ThreadHead, HostError>>("sessions.head", Err(stop()));
        request(&host, &sink, 11, "sessions.head", json!({ "threadId": "t1" })).await;
        request(&host, &sink, 12, "sessions.head", json!({ "threadId": "t1", "afterRead": true })).await;
        assert_eq!(args_of(rt, "sessions.head", 0)["afterRead"], Value::Null);
        assert_eq!(args_of(rt, "sessions.head", 1)["afterRead"], json!(true));

        let ran = json!({ "workspaceId": "ws_1", "sessionId": "s1", "runId": "r1", "block": "b1", "command": "ls", "state": "running" });
        rt.script.once::<Result<wsp_wire::views::session_events_more::SessionRunEvent, HostError>>("sessions.run", Ok(from(ran.clone())));
        let run = request(&host, &sink, 13, "sessions.run", json!({ "runId": "r1", "block": "b1", "command": "ls", "state": "running", "threadId": "t1", "turnId": "tu_1" })).await;
        assert_eq!(keys(&run), vec!["id", "ok", "run"]);
        assert_eq!(run["run"]["runId"], json!("r1"));
        let step = args_of(rt, "sessions.run", 0)["step"].clone();
        assert_eq!((step["runId"].clone(), step["threadId"].clone(), step["turnId"].clone()), (json!("r1"), json!("t1"), json!("tu_1")));
        assert!(step.get("id").is_none() && step.get("op").is_none(), "the step is the frame less id and op");
    })
}

// O8. serve.test.ts:661
#[test]
fn interrupting_twice_reaches_the_runtime_twice() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        for outcome in ["accepted", "not-running"] {
            r.rt.script.once::<Result<wsp_wire::wire::runtime::SessionInterruptResult, HostError>>("sessions.interrupt", Ok(from(json!({ "outcome": outcome }))));
        }
        let first = request(&host, &sink, 1, "sessions.interrupt", json!({ "sessionId": "s1" })).await;
        let second = request(&host, &sink, 2, "sessions.interrupt", json!({ "sessionId": "s1" })).await;
        assert_eq!(first, json!({ "id": 1, "ok": true, "outcome": "accepted" }));
        assert_eq!(second, json!({ "id": 2, "ok": true, "outcome": "not-running" }));
        assert_eq!(r.rt.script.calls_to("sessions.interrupt").len(), 2, "the switch keeps no memory of the first");
        assert_eq!(args_of(&r.rt, "sessions.interrupt", 0), args_of(&r.rt, "sessions.interrupt", 1));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Images, usage and forwards.

// O9. serve.test.ts:362
#[test]
fn capabilities_are_the_backends_flags() {
    block(async {
        let r = rig(|_| {});
        *r.rt.capabilities.borrow_mut() = json!({ "images": true, "previewUrls": true, "copies": true });
        let (host, sink) = r.host().await;
        let reply = request(&host, &sink, 1, "capabilities.get", json!({})).await;
        assert_eq!(reply["ok"], json!(true));
        let c = &reply["capabilities"];
        assert_eq!((c["images"].clone(), c["previewUrls"].clone(), c["copies"].clone(), c["liveCloneForks"].clone()), (json!(true), json!(true), json!(true), json!(false)));
        assert_eq!(c["sizes"], json!([]));
        assert!(r.failed.borrow().is_empty());
    })
}

fn version(n: u32) -> Value {
    json!({
        "version": n, "snapshotId": format!("snap_golden-v{n}"), "baseTemplate": "base", "setupSha": format!("sha{n}"),
        "createdAt": format!("2026-08-{}T00:00:00.000Z", 10 + n), "smoke": { "cmd": "true", "exitCode": 0 },
    })
}

fn manifest(head: u32) -> wsp_wire::views::golden_image::GoldenManifest {
    from(json!({ "head": head, "versions": [version(1), version(2)] }))
}

// O10. serve.test.ts:834
#[test]
fn snapshots_list_names_default_and_null_head() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let m = manifest(2);
        let shown = serde_json::to_value(&m).unwrap();
        r.rt.script.once::<Result<Option<wsp_wire::views::golden_image::GoldenManifest>, HostError>>("golden.get", Ok(Some(m)));
        let listed = request(&host, &sink, 1, "snapshots.list", json!({})).await;
        assert_eq!(listed["lineage"], json!({ "name": "default", "head": 2, "versions": shown["versions"] }));
        assert_eq!(keys(&listed["lineage"]), vec!["name", "head", "versions"]);
        assert_eq!(args_of(&r.rt, "golden.get", 0)["name"], json!("default"));

        r.rt.script.once::<Result<Option<wsp_wire::views::golden_image::GoldenManifest>, HostError>>("golden.get", Ok(None));
        let empty = request(&host, &sink, 2, "snapshots.list", json!({ "name": "other" })).await;
        assert_eq!(empty.to_string(), r#"{"id":2,"ok":true,"lineage":{"name":"other","head":null,"versions":[]}}"#);
        assert_eq!(args_of(&r.rt, "golden.get", 1)["name"], json!("other"));
    })
}

// O10. serve.test.ts:834, 855
#[test]
fn snapshots_rollback_says_untouched() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let m = manifest(1);
        let shown = serde_json::to_value(&m).unwrap();
        r.rt.script.once::<Result<wsp_wire::views::golden_image::GoldenManifest, HostError>>("golden.rollback", Ok(m));
        let rolled = request(&host, &sink, 1, "snapshots.rollback", json!({ "version": 1 })).await;
        assert_eq!(keys(&rolled), vec!["id", "ok", "lineage", "existingWorkspaces"]);
        assert_eq!(rolled["lineage"], json!({ "name": "default", "head": 1, "versions": shown["versions"] }));
        assert_eq!(rolled["existingWorkspaces"], json!("untouched"));
        let a = args_of(&r.rt, "golden.rollback", 0);
        assert_eq!((a["version"].clone(), a["name"].clone()), (json!(1), json!("default")));

        let e = HostError::with_kind("snapshot v9 is not in the manifest", Kind::Missing);
        r.rt.script.once::<Result<wsp_wire::views::golden_image::GoldenManifest, HostError>>("golden.rollback", Err(e.clone()));
        let refused = request(&host, &sink, 2, "snapshots.rollback", json!({ "version": 9 })).await;
        assert_eq!(refused.to_string(), json!({ "id": 2, "ok": false, "error": e.message, "kind": "missing" }).to_string());
        assert!(r.rt.script.calls_to("golden.get").is_empty(), "nothing else is read for a refused rollback");
    })
}

// O10. serve.test.ts:855
#[test]
fn snapshots_storage_null_when_none() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        r.rt.script.once::<Result<Option<wsp_wire::views::snapshot_lineage::SnapshotStorage>, HostError>>("golden.storage", Ok(None));
        let reply = request(&host, &sink, 1, "snapshots.storage", json!({})).await;
        assert_eq!(reply.to_string(), r#"{"id":1,"ok":true,"storage":null}"#);
    })
}

// O9. serve.test.ts:735
#[test]
fn golden_and_image_ops_pass_only_the_fields_sent() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let (host, sink) = r.host().await;
        rt.script.once::<Result<Option<wsp_wire::views::golden_image::GoldenManifest>, HostError>>("golden.get", Ok(None));
        let got = request(&host, &sink, 1, "golden.get", json!({ "name": "default" })).await;
        assert_eq!(got.to_string(), r#"{"id":1,"ok":true,"manifest":null}"#);

        let stop = || HostError::new("stop");
        rt.script.once::<Result<wsp_wire::views::golden_image::GoldenBuilderView, HostError>>("golden.prepare", Err(stop()));
        rt.script.once::<Result<wsp_wire::views::golden_image::GoldenBuilderView, HostError>>("golden.prepare", Err(stop()));
        request(&host, &sink, 2, "golden.prepare", json!({ "name": "default" })).await;
        request(&host, &sink, 3, "golden.prepare", json!({ "name": "default", "kind": "desktop" })).await;
        assert_eq!(args_of(rt, "golden.prepare", 0)["ask"]["kind"], Value::Null, "no kind named, none passed");
        assert_eq!(args_of(rt, "golden.prepare", 1)["ask"]["kind"], json!("desktop"));
        assert_eq!(args_of(rt, "golden.prepare", 1)["ask"]["name"], json!("default"));

        rt.script.once::<Result<wsp_core::types::api::Sealed, HostError>>("golden.seal", Err(stop()));
        request(&host, &sink, 4, "golden.seal", json!({ "builderId": "b_1" })).await;
        assert_eq!(args_of(rt, "golden.seal", 0)["builderId"], json!("b_1"));
        assert_eq!(args_of(rt, "golden.seal", 0)["logins"], Value::Null);

        rt.script.once::<Result<wsp_wire::views::snapshot_lineage::SealedImageView, HostError>>("image.get", Err(stop()));
        request(&host, &sink, 5, "image.get", json!({})).await;
        assert_eq!(args_of(rt, "image.get", 0)["name"], Value::Null);

        rt.script.once::<Result<wsp_wire::views::golden_image::SealedImageBuilt, HostError>>("image.build", Err(stop()));
        rt.script.once::<Result<wsp_wire::views::golden_image::SealedImageBuilt, HostError>>("image.build", Err(stop()));
        request(&host, &sink, 6, "image.build", json!({ "place": "pl_1" })).await;
        request(&host, &sink, 7, "image.build", json!({ "place": "pl_1", "name": "mine", "force": false })).await;
        assert_eq!(args_of(rt, "image.build", 0)["ask"], json!({ "place": "pl_1", "name": null, "force": null }));
        assert_eq!(args_of(rt, "image.build", 1)["ask"], json!({ "place": "pl_1", "name": "mine", "force": false }), "a false force is sent");
    })
}

fn spend_row() -> PlaceSpend {
    from(json!({ "place": "box", "todayUsd": 1.5, "monthUsd": 10, "rateUsdPerHour": 0.1 }))
}

// O11. serve-refusal.test.ts:164
#[test]
fn cost_spend_for_a_thread_reads_its_places() {
    block(async {
        let rt = FakeRuntime::with_places();
        let r = rig_over(rt.clone(), |o| o.now = Some(Rc::new(|| 4242)));
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_1", "box1", "cloud", "running", json!({}))));
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(vec![place("pl_1", "laptop", "computer"), place("box", "box", "provider")]));
        rt.script.once::<Result<Vec<PlaceSpend>, String>>("status.spend", Ok(vec![spend_row()]));
        let (thread, sink) = r.thread("ws_1").await;
        let reply = request(&thread, &sink, 1, "cost.spend", json!({})).await;
        assert_eq!(keys(&reply), vec!["id", "ok", "places"]);
        assert_eq!(reply["places"], json!([serde_json::to_value(spend_row()).unwrap()]));
        let asked = args_of(&rt, "status.spend", 0)["places"].clone();
        assert_eq!(asked, json!([{ "id": "box", "kind": "provider", "name": "box", "label": said("box", "provider"), "default": false }]), "a machine's thread reads the clouds alone");
        assert_eq!(args_of(&rt, "places.list", 0)["now"], json!(4242));
        assert_eq!(args_of(&rt, "workspaces.get", 0)["id"], json!("ws_1"));
        assert!(r.failed.borrow().is_empty());
    })
}

// O11. serve-refusal.test.ts:164
#[test]
fn cost_spend_is_the_own_road_and_reads_the_places_it_holds() {
    block(async {
        let rt = FakeRuntime::with_places();
        let r = rig_over(rt.clone(), |_| {});
        let (ticketed, ticket_sink) = r.ticketed().await;
        let refused = request(&ticketed, &ticket_sink, 1, "cost.spend", json!({})).await;
        assert_eq!(refused, ticket_reply(1));
        assert!(rt.script.calls_to("status.spend").is_empty() && rt.script.calls_to("places.list").is_empty());

        let rows = vec![place("pl_1", "laptop", "computer")];
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows.clone()));
        rt.script.once::<Result<Vec<PlaceSpend>, String>>("status.spend", Ok(vec![]));
        let (host, sink) = r.host().await;
        let reply = request(&host, &sink, 2, "cost.spend", json!({})).await;
        assert_eq!(reply.to_string(), r#"{"id":2,"ok":true,"places":[]}"#);
        assert_eq!(args_of(&rt, "status.spend", 0)["places"], json!([serde_json::to_value(&rows[0]).unwrap()]), "the rows as the list holds them, road and all");

        let bare = rig(|_| {});
        bare.rt.script.once::<Result<Vec<PlaceSpend>, String>>("status.spend", Ok(vec![]));
        let (bare_host, bare_sink) = bare.host().await;
        request(&bare_host, &bare_sink, 1, "cost.spend", json!({})).await;
        assert_eq!(args_of(&bare.rt, "status.spend", 0)["places"], json!([]), "a runtime with no places spends over none");
    })
}

// O11. serve.test.ts
#[test]
fn cost_history_passes_the_origin() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        r.rt.script.once::<Result<Vec<wsp_wire::views::workspace_events::WorkspaceCostEvent>, HostError>>("status.history", Ok(vec![]));
        let reply = request(&host, &sink, 1, "cost.history", json!({ "workspaceId": "ws_1", "origin": "here" })).await;
        assert_eq!(reply.to_string(), r#"{"id":1,"ok":true,"points":[]}"#);
        let a = args_of(&r.rt, "status.history", 0);
        assert_eq!((a["workspaceId"].clone(), a["origin"].clone()), (json!("ws_1"), json!("here")));
    })
}

// O12. serve-refusal.test.ts:164
#[test]
fn usage_ops_are_the_own_road() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let (ticketed, ticket_sink) = r.ticketed().await;
        let asks: Vec<(&str, Value)> = vec![
            ("usage.used", json!({ "range": "week", "split": "agent" })),
            ("usage.accounts", json!({})),
            ("usage.reset", json!({ "account": "a1" })),
            ("places.readings", json!({ "placeId": "pl_1", "range": "day" })),
        ];
        for (i, (op, fields)) in asks.iter().enumerate() {
            let id = (i + 1) as u64;
            assert_eq!(request(&ticketed, &ticket_sink, id, op, fields.clone()).await, ticket_reply(id), "{op}");
        }
        assert!(rt.script.names().iter().all(|n| !n.starts_with("usage.")), "a socket off the own road reads no usage");

        let (host, sink) = r.host().await;
        let used = json!({ "range": "week", "split": "agent", "rows": [], "series": [], "since": 1, "until": 2 });
        rt.script.once::<Result<wsp_wire::types::usage::UsedAnswer, HostError>>("usage.used", Ok(from(used)));
        rt.script.once::<Result<wsp_wire::types::usage::UsedAnswer, HostError>>("usage.used", Ok(from(json!({ "range": "day", "split": "model", "rows": [], "series": [], "since": 1, "until": 2 }))));
        let reply = request(&host, &sink, 1, "usage.used", json!({ "range": "week", "split": "agent" })).await;
        assert_eq!(keys(&reply), vec!["id", "ok", "used"]);
        assert_eq!(reply["used"]["range"], json!("week"));
        let a = args_of(rt, "usage.used", 0);
        assert_eq!((a["range"].clone(), a["split"].clone(), a["outside"].clone()), (json!("week"), json!("agent"), Value::Null));
        request(&host, &sink, 2, "usage.used", json!({ "range": "day", "split": "model", "outside": false })).await;
        assert_eq!(args_of(rt, "usage.used", 1)["outside"], json!(false), "a false outside is sent");

        rt.script.once::<Result<wsp_wire::types::usage::AccountsAnswer, HostError>>("usage.accounts", Ok(from(json!({ "accounts": [] }))));
        rt.script.once::<Result<wsp_wire::types::usage::AccountsAnswer, HostError>>("usage.accounts", Ok(from(json!({ "accounts": [] }))));
        let accounts = request(&host, &sink, 3, "usage.accounts", json!({ "fresh": false })).await;
        assert_eq!(accounts.to_string(), r#"{"id":3,"ok":true,"accounts":[]}"#);
        request(&host, &sink, 4, "usage.accounts", json!({ "fresh": true })).await;
        assert_eq!(args_of(rt, "usage.accounts", 0)["fresh"], Value::Null, "fresh rides only when true");
        assert_eq!(args_of(rt, "usage.accounts", 1)["fresh"], json!(true));

        rt.script.once::<Result<wsp_wire::types::usage::ResetAnswer, HostError>>("usage.reset", Ok(from(json!({ "outcome": "reset", "said": "reset it" }))));
        let reset = request(&host, &sink, 5, "usage.reset", json!({ "account": "a1", "creditId": "c1" })).await;
        assert_eq!(reset.to_string(), r#"{"id":5,"ok":true,"outcome":"reset","said":"reset it"}"#);
        let a = args_of(rt, "usage.reset", 0);
        assert_eq!((a["account"].clone(), a["creditId"].clone(), a["on"].clone()), (json!("a1"), json!("c1"), Value::Null));
    })
}

// O13. serve.ts:1649
#[test]
fn readings_name_a_place_or_a_workspace() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let neither = request(&host, &sink, 1, "places.readings", json!({ "range": "day" })).await;
        assert_eq!(neither.to_string(), r#"{"id":1,"ok":false,"error":"places.readings names a placeId or a workspaceId","kind":"usage"}"#);
        assert!(r.rt.script.calls_to("usage.readings").is_empty());
        assert!(r.failed.borrow().is_empty(), "the switch sent it");

        let answer = json!({ "points": [], "stepMs": 1000, "from": 1, "to": 2 });
        r.rt.script.once::<Result<wsp_wire::wire::machine_link::ReadingsAnswer, HostError>>("usage.readings", Ok(from(answer.clone())));
        r.rt.script.once::<Result<wsp_wire::wire::machine_link::ReadingsAnswer, HostError>>("usage.readings", Ok(from(answer)));
        let both = request(&host, &sink, 2, "places.readings", json!({ "placeId": "pl_1", "workspaceId": "ws_1", "range": "day" })).await;
        assert_eq!(both.to_string(), r#"{"id":2,"ok":true,"points":[],"stepMs":1000,"from":1,"to":2}"#);
        assert_eq!(args_of(&r.rt, "usage.readings", 0)["target"], json!("Workspace(\"ws_1\")"), "a workspaceId wins over a placeId");
        request(&host, &sink, 3, "places.readings", json!({ "placeId": "pl_1", "range": "week" })).await;
        assert_eq!(args_of(&r.rt, "usage.readings", 1)["target"], json!("Place(\"pl_1\")"));
        assert_eq!(args_of(&r.rt, "usage.readings", 1)["range"], json!("week"));
    })
}

// O14. serve.test.ts:893
#[test]
fn forwards_list_shows_only_drivable_and_stop_refuses_an_unknown() {
    block(async {
        let source = Forwards::with(vec![forward("ws_1", 8123), forward("ws_2", 8200)]);
        let r = rig(|o| o.forwards = Some(source.clone() as Rc<dyn ForwardsSource>));
        r.rt.script.on::<Result<Option<String>, HostError>>("workspaces.originRefusal", |call| {
            Ok(if call.args["id"] == json!("ws_2") { Some("no workspace ws_2 on this road".to_string()) } else { None })
        });
        let (host, sink) = r.host().await;
        let listed = request(&host, &sink, 1, "forwards.list", json!({})).await;
        assert_eq!(listed, json!({ "id": 1, "ok": true, "forwards": [forward("ws_1", 8123)] }));
        let asked: Vec<Value> = r.rt.script.calls_to("workspaces.originRefusal").iter().map(|c| c.args["id"].clone()).collect();
        assert_eq!(asked, vec![json!("ws_1"), json!("ws_2")], "each asked in the source's order");

        let refused = request(&host, &sink, 2, "forwards.stop", json!({ "workspaceId": "ws_2", "port": 8200 })).await;
        assert_eq!(refused, json!({ "id": 2, "ok": false, "error": "no workspace ws_2 on this road" }));
        assert!(source.stops.borrow().is_empty(), "a forward the caller may not drive is not stopped");

        let stopped = request(&host, &sink, 3, "forwards.stop", json!({ "workspaceId": "ws_1", "port": 8123 })).await;
        assert_eq!(stopped, json!({ "id": 3, "ok": true }));
        let again = request(&host, &sink, 4, "forwards.stop", json!({ "workspaceId": "ws_1", "port": 8123 })).await;
        assert_eq!(again, json!({ "id": 4, "ok": false, "error": "nothing is forwarding localhost:8123 there" }));
        assert_eq!(*source.stops.borrow(), vec![("ws_1".to_string(), 8123), ("ws_1".to_string(), 8123)]);
        let after = request(&host, &sink, 5, "forwards.list", json!({})).await;
        assert_eq!(after["forwards"], json!([]));

        let low = request_unchecked(&host, &sink, 6, "forwards.stop", json!({ "workspaceId": "ws_1", "port": 80 })).await;
        assert_eq!(low["ok"], json!(false));
        assert_eq!(source.stops.borrow().len(), 2, "below 1024 never reaches the source");
        assert_eq!(args_of(&r.rt, "workspaces.originRefusal", 0)["origin"], Value::Null);
    })
}

// O14. serve.test.ts:928
#[test]
fn without_a_source_the_list_is_empty_and_a_stop_fails() {
    block(async {
        let r = rig(|_| {});
        r.rt.script.on::<Result<Option<String>, HostError>>("workspaces.originRefusal", |_| Ok(None));
        let (host, sink) = r.host().await;
        let listed = request(&host, &sink, 1, "forwards.list", json!({})).await;
        assert_eq!(listed, json!({ "id": 1, "ok": true, "forwards": [] }));
        let stop = request(&host, &sink, 2, "forwards.stop", json!({ "workspaceId": "ws_1", "port": 8123 })).await;
        assert_eq!(stop, json!({ "id": 2, "ok": false, "error": "nothing is forwarding localhost:8123 there" }));
        assert_eq!(r.failed_ops(), vec!["forwards.stop"]);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O15. Folders.

// O15. places-agents.test.ts
#[test]
fn host_folders_wide_only_for_this_computers_own_window() {
    block(async {
        let folders = Rc::new(Folders { asked: RefCell::new(Vec::new()) });
        let r = rig(|o| o.folders = Some(folders.clone() as Rc<dyn HostFolders>));
        let (host, sink) = r.host().await;
        let asked = || folders.asked.borrow().clone();

        let reply = request(&host, &sink, 1, "host.folders", json!({ "origin": "here", "dir": "/x", "hidden": false, "repos": true })).await;
        assert_eq!(reply, json!({ "id": 1, "ok": true, "listing": listing() }));
        assert_eq!(asked()[0], json!({ "dir": "/x", "hidden": false, "repos": true, "wide": true }));

        request(&host, &sink, 2, "host.folders", json!({ "origin": "here", "on": "here", "repos": false })).await;
        assert_eq!(asked()[1], json!({ "wide": true }), "on: here is this computer; repos false is not sent");
        request(&host, &sink, 3, "host.folders", json!({})).await;
        assert_eq!(asked()[2], json!({ "wide": false }), "a frame that names no road is not the own window");
        request(&host, &sink, 4, "host.folders", json!({ "origin": "relayed" })).await;
        assert_eq!(asked()[3], json!({ "wide": false }));

        let (paired, paired_sink) = r.paired().await;
        request(&paired, &paired_sink, 1, "host.folders", json!({ "origin": "here" })).await;
        assert_eq!(asked()[4], json!({ "wide": false }), "a paired computer reads paired whatever it sends");

        let (thread, thread_sink) = r.thread("ws_1").await;
        let reply = request(&thread, &thread_sink, 1, "host.folders", json!({ "origin": "here" })).await;
        assert_eq!(reply["ok"], json!(true));
        assert_eq!(asked()[5], json!({ "wide": false }), "a thread stays inside the home folder and the projects");
        assert_eq!(asked().len(), 6);
        assert!(r.rt.script.calls_to("places.list").is_empty(), "this computer is read through the folders door, never the place door");
    })
}

// O15. places-agents.test.ts
#[test]
fn place_folders_refuse_in_order_and_close_the_link() {
    block(async {
        let rt = FakeRuntime::with_places();
        let folders = Rc::new(Folders { asked: RefCell::new(Vec::new()) });
        let r = rig_over(rt.clone(), |o| {
            o.folders = Some(folders.clone() as Rc<dyn HostFolders>);
            o.now = Some(Rc::new(|| 1234));
        });
        let rows = || vec![place("pl_1", "laptop", "computer"), place("box", "box", "provider")];
        let report = |version: i64| -> PlaceReport {
            from(json!({
                "name": "laptop", "platform": "linux", "arch": "x64", "os": "linux", "shape": { "cpu": 2, "memMb": 4096 }, "login": {},
                "runsWorkspaces": true, "engine": "none", "daemonVersion": version, "wsp": [], "agents": [], "dialed": "10.0.0.2",
            }))
        };
        let (host, sink) = r.host().await;
        let on = |place: &str| json!({ "on": place, "dir": "/d" });

        // 1. A word that names no row.
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        let held = vec![said("laptop", "computer"), said("box", "provider")];
        let nobody = request(&host, &sink, 1, "host.folders", on("pl_9")).await;
        assert_eq!(nobody.to_string(), json!({ "id": 1, "ok": false, "error": no_such_place_refusal("pl_9", &held), "kind": "usage" }).to_string());
        assert_eq!(args_of(&rt, "places.list", 0)["now"], json!(1234));

        // 2. A provider keeps no computer to browse; nothing more is asked of it.
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        let provider = request(&host, &sink, 2, "host.folders", on("box")).await;
        assert_eq!(provider.to_string(), json!({ "id": 2, "ok": false, "error": provider_folders_refusal(&said("box", "provider")), "kind": "usage" }).to_string());
        assert!(rt.script.calls_to("places.reportOf").is_empty());

        // 3. A daemon older than the folders op.
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        rt.script.once::<Result<Option<PlaceReport>, HostError>>("places.reportOf", Ok(Some(report(60))));
        let behind = request(&host, &sink, 3, "host.folders", on("pl_1")).await;
        let word = place_daemon_behind(Some(60)).expect("60 is behind");
        assert_eq!(behind.to_string(), json!({ "id": 3, "ok": false, "error": place_behind_line("laptop", &word) }).to_string());
        assert_eq!(args_of(&rt, "places.reportOf", 0)["placeId"], json!("pl_1"));
        assert!(rt.script.calls_to("places.channel").is_empty(), "no link is opened for a daemon that cannot answer");

        // 4. No link held.
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        rt.script.once::<Result<Option<PlaceReport>, HostError>>("places.reportOf", Ok(Some(report(80))));
        rt.script.once::<Option<Rc<dyn DaemonChannel>>>("places.channel", None);
        let absent = request(&host, &sink, 4, "host.folders", on("pl_1")).await;
        assert_eq!(absent.to_string(), json!({ "id": 4, "ok": false, "error": absent_computer("laptop", None).sentence }).to_string());
        assert!(rt.script.calls_to("projects.list").is_empty());

        // 5. The link answers: the folders of that computer's projects ride the frame, and the link is closed.
        let l = link(Ok(json!({ "ok": true, "dir": "/d", "roots": ["/home/u"], "folders": [], "hidden": 0 })));
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        rt.script.once::<Result<Option<PlaceReport>, HostError>>("places.reportOf", Ok(Some(report(80))));
        rt.script.once::<Option<Rc<dyn DaemonChannel>>>("places.channel", Some(l.clone() as Rc<dyn DaemonChannel>));
        rt.script.once::<Result<Vec<ProjectView>, HostError>>("projects.list", Ok(vec![project("pr_1", "pl_1", Some("/co")), project("pr_2", "pl_1", None), project("pr_3", "pl_2", Some("/else"))]));
        let ok = request(&host, &sink, 5, "host.folders", json!({ "on": "pl_1", "dir": "/d", "repos": true })).await;
        assert_eq!(ok, json!({ "id": 5, "ok": true, "listing": { "dir": "/d", "roots": ["/home/u"], "folders": [], "hidden": 0 } }));
        let sent = l.sent.borrow().clone();
        assert_eq!(sent.len(), 1);
        assert_eq!(Value::Object(sent[0].clone()), json!({ "op": "fs.folders", "dir": "/d", "repos": true, "projects": ["/co", "/app"] }));
        assert_eq!(l.closes.get(), 1);
        assert!(folders.asked.borrow().is_empty(), "another computer is not read through this computer's folders door");

        // 6. The daemon says no: its words fail the op, and the link is closed all the same.
        let l = link(Ok(json!({ "ok": false, "error": "not a folder" })));
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        rt.script.once::<Result<Option<PlaceReport>, HostError>>("places.reportOf", Ok(Some(report(80))));
        rt.script.once::<Option<Rc<dyn DaemonChannel>>>("places.channel", Some(l.clone() as Rc<dyn DaemonChannel>));
        rt.script.once::<Result<Vec<ProjectView>, HostError>>("projects.list", Ok(vec![]));
        let no = request(&host, &sink, 6, "host.folders", on("pl_1")).await;
        assert_eq!(no, json!({ "id": 6, "ok": false, "error": "not a folder" }));
        assert_eq!(l.closes.get(), 1);

        // 7. The link itself fails.
        let l = link(Err(ChannelError::Refused { message: "connection lost".into() }));
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        rt.script.once::<Result<Option<PlaceReport>, HostError>>("places.reportOf", Ok(None));
        rt.script.once::<Option<Rc<dyn DaemonChannel>>>("places.channel", Some(l.clone() as Rc<dyn DaemonChannel>));
        rt.script.once::<Result<Vec<ProjectView>, HostError>>("projects.list", Ok(vec![]));
        let lost = request(&host, &sink, 7, "host.folders", on("pl_1")).await;
        assert_eq!(lost, json!({ "id": 7, "ok": false, "error": "connection lost" }));
        assert_eq!(l.closes.get(), 1, "closed whatever happens");

        // A socket off the own road is refused before any row is read.
        let (ticketed, ticket_sink) = r.ticketed().await;
        let before = rt.script.calls_to("places.list").len();
        let refused = request(&ticketed, &ticket_sink, 1, "host.folders", on("pl_1")).await;
        assert_eq!(refused, ticket_reply(1));
        assert_eq!(rt.script.calls_to("places.list").len(), before);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O16-O19. Agents.

// O16, O17. places-agents.test.ts
#[test]
fn agents_ops_gates_and_sentences() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let (ticketed, ticket_sink) = r.ticketed().await;
        let t = target();

        for (id, op, fields) in [(1, "agents.read", json!({ "target": t })), (2, "servers.tools", json!({ "target": t, "agent": "claude", "name": "s" }))] {
            let reply = request(&ticketed, &ticket_sink, id, op, fields).await;
            assert_eq!(reply.to_string(), json!({ "id": id, "ok": false, "error": PLACES_TICKET_REFUSAL }).to_string(), "{op}: no kind, as the TS sends it");
        }
        let gated: Vec<(&str, Value)> = vec![
            ("servers.icon", json!({ "host": "x.com" })),
            ("agents.addTools", json!({ "target": t, "agent": "claude" })),
            ("agents.setup", json!({ "placeId": "pl_1", "agent": "claude" })),
            ("skills.search", json!({ "q": "a" })),
            ("skills.get", json!({ "skill": "x" })),
            ("skills.preview", json!({ "target": t, "name": "n" })),
            ("skills.add", json!({ "target": t, "skill": "x" })),
            ("skills.remove", json!({ "target": t, "name": "n" })),
            ("skills.toggle", json!({ "target": t, "name": "n", "on": true })),
            ("servers.add", json!({ "target": t, "agent": "claude", "name": "n" })),
            ("servers.remove", json!({ "target": t, "agent": "claude", "name": "n" })),
            ("servers.toggle", json!({ "target": t, "agent": "claude", "name": "n", "on": true })),
            ("plugins.toggle", json!({ "target": t, "agent": "claude", "plugin": "p", "on": true })),
        ];
        for (i, (op, fields)) in gated.iter().enumerate() {
            let id = 10 + i as u64;
            assert_eq!(request(&ticketed, &ticket_sink, id, op, fields.clone()).await, ticket_reply(id), "{op}");
        }
        assert!(rt.script.names().iter().all(|n| !n.starts_with("agents.")), "a socket off the own road reaches no agents area: {:?}", rt.script.names());
        assert!(r.failed.borrow().is_empty());

        // agents.signInLine: host only.
        let (paired, paired_sink) = r.paired().await;
        let line = json!({ "target": t, "agent": "claude" });
        let by_device = request(&paired, &paired_sink, 1, "agents.signInLine", line.clone()).await;
        assert_eq!(by_device.to_string(), json!({ "id": 1, "ok": false, "error": SIGN_IN_LINE_REFUSAL }).to_string());
        let off_road = request(&ticketed, &ticket_sink, 40, "agents.signInLine", line.clone()).await;
        assert_eq!(off_road, ticket_reply(40));
        assert!(rt.script.calls_to("agents.signInLine").is_empty());
        let (host, sink) = r.host().await;
        rt.script.once::<Result<wsp_wire::types::agents_report::SignInLine, HostError>>("agents.signInLine", Ok(from(json!({ "command": "wsp login claude" }))));
        let ok = request(&host, &sink, 1, "agents.signInLine", json!({ "target": t, "agent": "claude", "name": "srv" })).await;
        assert_eq!(ok["line"]["command"], json!("wsp login claude"));
        let ask = wsp_core::types::pending::SignInAsk { agent: "claude".into(), server: Some("srv".into()), terminal: None };
        assert_eq!(args_of(rt, "agents.signInLine", 0)["ask"], json!(format!("{ask:?}")), "name rides as server");

        // agents.key: host only; the key never comes back.
        let by_device = request(&paired, &paired_sink, 2, "agents.key", json!({ "agent": "claude", "key": "sk-secret-1" })).await;
        assert_eq!(by_device.to_string(), json!({ "id": 2, "ok": false, "error": AGENTS_KEY_REFUSAL }).to_string());
        assert!(rt.script.calls_to("agents.key").is_empty());
        rt.script.once::<Result<(), HostError>>("agents.key", Ok(()));
        let keyed = request(&host, &sink, 2, "agents.key", json!({ "agent": "claude", "key": "sk-secret-1", "way": "token" })).await;
        assert_eq!(keyed.to_string(), r#"{"id":2,"ok":true}"#);
        let a = args_of(rt, "agents.key", 0);
        assert_eq!((a["agent"].clone(), a["key"].clone(), a["way"].clone()), (json!("claude"), json!("sk-secret-1"), json!("Token")));

        // A paired computer reads agents, stamped with its road.
        rt.script.once::<Result<wsp_wire::types::agents_report::AgentsReport, HostError>>("agents.read", Err(HostError::new("stop")));
        request(&paired, &paired_sink, 3, "agents.read", json!({ "target": t })).await;
        let read = args_of(rt, "agents.read", 0);
        assert_eq!((read["target"].clone(), read["origin"].clone()), (t, json!("paired")));
    })
}

// O18. vault.test.ts
#[test]
fn agents_setup_env_values_only_from_the_host() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let stop = || Err::<wsp_wire::types::agents_report::AgentRow, _>(HostError::new("stop"));
        let (paired, paired_sink) = r.paired().await;
        let with_value = json!({ "placeId": "pl_1", "agent": "claude", "env": { "TOKEN": "v-1" } });
        let refused = request(&paired, &paired_sink, 1, "agents.setup", with_value.clone()).await;
        assert_eq!(refused.to_string(), json!({ "id": 1, "ok": false, "error": ENV_VALUE_REFUSAL }).to_string());
        assert!(rt.script.calls_to("agents.setup").is_empty());

        rt.script.once::<Result<wsp_wire::types::agents_report::AgentRow, HostError>>("agents.setup", stop());
        let cleared = request(&paired, &paired_sink, 2, "agents.setup", json!({ "placeId": "pl_1", "agent": "claude", "on": true, "env": { "TOKEN": null } })).await;
        assert_eq!(cleared["error"], json!("stop"), "a socket that is no host's may clear a variable");
        let change = args_of(rt, "agents.setup", 0);
        assert_eq!((change["placeId"].clone(), change["agent"].clone()), (json!("pl_1"), json!("claude")));
        assert_eq!(change["change"]["on"], json!(true));
        assert_eq!(change["change"]["env"], json!({ "TOKEN": null }));
        assert_eq!(change["origin"], json!("paired"));

        rt.script.once::<Result<wsp_wire::types::agents_report::AgentRow, HostError>>("agents.setup", stop());
        let (host, sink) = r.host().await;
        request(&host, &sink, 1, "agents.setup", with_value).await;
        assert_eq!(args_of(rt, "agents.setup", 1)["change"]["env"], json!({ "TOKEN": "v-1" }), "the host's own token may put a value in");
        let change = args_of(rt, "agents.setup", 1)["change"].clone();
        for key in ["id", "op", "placeId", "agent"] {
            assert!(change.get(key).is_none(), "{key} is not part of the change");
        }

        let (ticketed, ticket_sink) = r.ticketed().await;
        let off = request(&ticketed, &ticket_sink, 1, "agents.setup", json!({ "placeId": "pl_1", "agent": "claude", "env": { "TOKEN": "v-1" } })).await;
        assert_eq!(off, ticket_reply(1), "the road is refused before the env is read");
    })
}

// O19. places-agents.test.ts, vault.test.ts
#[test]
fn skills_and_servers_pass_their_fields() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let (host, sink) = r.host().await;
        let t = target();
        let stop = || HostError::new("stop");

        rt.script.once::<Result<Vec<wsp_wire::types::agents_report::SkillHit>, HostError>>("agents.skillsSearch", Ok(vec![]));
        rt.script.once::<Result<Vec<wsp_wire::types::agents_report::SkillHit>, HostError>>("agents.skillsSearch", Ok(vec![]));
        let found = request(&host, &sink, 1, "skills.search", json!({ "q": "pdf" })).await;
        assert_eq!(found.to_string(), r#"{"id":1,"ok":true,"skills":[]}"#);
        request(&host, &sink, 2, "skills.search", json!({ "q": "pdf", "limit": 5 })).await;
        assert_eq!((args_of(rt, "agents.skillsSearch", 0)["q"].clone(), args_of(rt, "agents.skillsSearch", 0)["limit"].clone()), (json!("pdf"), Value::Null));
        assert_eq!(args_of(rt, "agents.skillsSearch", 1)["limit"], json!(5));

        rt.script.once::<Result<wsp_wire::types::agents_report::SkillPreview, HostError>>("agents.skillsGet", Err(stop()));
        request(&host, &sink, 3, "skills.get", json!({ "skill": "owner/pdf" })).await;
        assert_eq!(args_of(rt, "agents.skillsGet", 0)["skill"], json!("owner/pdf"));

        rt.script.once::<Result<wsp_wire::types::agents_report::SkillPreview, HostError>>("agents.skillsPreview", Err(stop()));
        request(&host, &sink, 4, "skills.preview", json!({ "target": t, "name": "pdf", "project": false })).await;
        let preview = wsp_core::types::pending::SkillAsk { name: "pdf".into(), project: Some(false) };
        assert_eq!(args_of(rt, "agents.skillsPreview", 0)["ask"], json!(format!("{preview:?}")), "a false project is sent");
        assert_eq!(args_of(rt, "agents.skillsPreview", 0)["target"], t);

        rt.script.once::<Result<wsp_wire::types::agents_report::SkillAdded, HostError>>("agents.skillsAdd", Err(stop()));
        rt.script.once::<Result<wsp_wire::types::agents_report::SkillAdded, HostError>>("agents.skillsAdd", Err(stop()));
        request(&host, &sink, 5, "skills.add", json!({ "target": t, "skill": "pdf" })).await;
        request(&host, &sink, 6, "skills.add", json!({ "target": t, "skill": "pdf", "agents": ["claude"], "project": true })).await;
        let add = |agents: Option<Vec<String>>, project: Option<bool>| json!(format!("{:?}", SkillsAddAsk { skill: "pdf".into(), agents, project }));
        assert_eq!(args_of(rt, "agents.skillsAdd", 0)["ask"], add(None, None));
        assert_eq!(args_of(rt, "agents.skillsAdd", 1)["ask"], add(Some(vec!["claude".into()]), Some(true)));

        rt.script.once::<Result<Vec<String>, HostError>>("agents.skillsRemove", Err(stop()));
        rt.script.once::<Result<Vec<String>, HostError>>("agents.skillsToggle", Err(stop()));
        request(&host, &sink, 7, "skills.remove", json!({ "target": t, "name": "pdf" })).await;
        request(&host, &sink, 8, "skills.toggle", json!({ "target": t, "name": "pdf", "project": true, "on": false })).await;
        let remove = wsp_core::types::pending::SkillAsk { name: "pdf".into(), project: None };
        assert_eq!(args_of(rt, "agents.skillsRemove", 0)["ask"], json!(format!("{remove:?}")));
        let toggle = wsp_core::types::pending::SkillAsk { name: "pdf".into(), project: Some(true) };
        assert_eq!(args_of(rt, "agents.skillsToggle", 0)["ask"], json!(format!("{toggle:?}")));
        assert_eq!(args_of(rt, "agents.skillsToggle", 0)["on"], json!(false));

        rt.script.once::<Result<String, HostError>>("agents.serversAdd", Err(stop()));
        request(&host, &sink, 9, "servers.add", json!({ "target": t, "agent": "claude", "name": "s", "command": "npx", "args": ["x"], "env": { "K": "v" } })).await;
        let add = args_of(rt, "agents.serversAdd", 0);
        assert_eq!(add["target"], t);
        let ask = &add["ask"];
        assert_eq!((ask["agent"].clone(), ask["name"].clone(), ask["command"].clone()), (json!("claude"), json!("s"), json!("npx")));
        assert_eq!((ask["args"].clone(), ask["env"].clone()), (json!(["x"]), json!({ "K": "v" })));
        for key in ["target", "id", "op"] {
            assert!(ask.get(key).is_none(), "{key} is not part of the server");
        }
        assert!(ask.get("url").map_or(true, Value::is_null), "what was not sent is not sent");

        rt.script.once::<Result<String, HostError>>("agents.serversRemove", Err(stop()));
        rt.script.once::<Result<String, HostError>>("agents.serversToggle", Err(stop()));
        request(&host, &sink, 10, "servers.remove", json!({ "target": t, "agent": "claude", "name": "s" })).await;
        request(&host, &sink, 11, "servers.toggle", json!({ "target": t, "agent": "claude", "name": "s", "scope": "user", "on": true })).await;
        let remove = &args_of(rt, "agents.serversRemove", 0)["ask"];
        assert_eq!((remove["agent"].clone(), remove["name"].clone()), (json!("claude"), json!("s")));
        assert!(remove.get("scope").map_or(true, Value::is_null));
        let toggle = args_of(rt, "agents.serversToggle", 0);
        assert_eq!(toggle["ask"]["scope"], json!("user"));
        assert_eq!(toggle["on"], json!(true));

        rt.script.once::<Result<wsp_wire::types::agents_report::PluginRow, HostError>>("agents.pluginsToggle", Err(stop()));
        request(&host, &sink, 12, "plugins.toggle", json!({ "target": t, "agent": "claude", "plugin": "p", "on": false })).await;
        let plugin = args_of(rt, "agents.pluginsToggle", 0);
        assert_eq!((plugin["ask"].clone(), plugin["on"].clone()), (json!({ "agent": "claude", "plugin": "p" }), json!(false)));

        rt.script.once::<Result<wsp_wire::types::agents_report::ServerToolsAnswer, HostError>>("agents.tools", Err(stop()));
        rt.script.once::<Result<wsp_wire::types::agents_report::ServerToolsAnswer, HostError>>("agents.tools", Err(stop()));
        request(&host, &sink, 13, "servers.tools", json!({ "target": t, "agent": "claude", "name": "s" })).await;
        request(&host, &sink, 14, "servers.tools", json!({ "target": t, "agent": "claude", "name": "s", "refresh": true })).await;
        let tools = |refresh: Option<bool>| json!(format!("{:?}", ToolsAsk { agent: "claude".into(), name: "s".into(), refresh }));
        assert_eq!(args_of(rt, "agents.tools", 0)["ask"], tools(None));
        assert_eq!(args_of(rt, "agents.tools", 1)["ask"], tools(Some(true)));

        rt.script.once::<Result<Option<String>, HostError>>("agents.serversIcon", Ok(None));
        let icon = request(&host, &sink, 15, "servers.icon", json!({ "host": "x.com", "refresh": true })).await;
        assert_eq!(icon.to_string(), r#"{"id":15,"ok":true,"icon":null}"#);
        let a = args_of(rt, "agents.serversIcon", 0);
        assert_eq!((a["host"].clone(), a["refresh"].clone()), (json!("x.com"), json!(true)));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O20, O21. Editors and ssh.

fn editor_rig(include: bool, remote_refusal: Option<&str>) -> (Rig, Rc<Editor>, Rc<Ssh>, Rc<RefCell<Vec<String>>>) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let editor = Rc::new(Editor { log: log.clone(), opens: RefCell::new(Vec::new()), remote_refusal: remote_refusal.map(String::from) });
    let ssh = Rc::new(Ssh { log: log.clone(), include: Cell::new(include) });
    let (e, s) = (editor.clone(), ssh.clone());
    let r = rig(move |o| {
        o.editor = Some(e as Rc<dyn HostEditor>);
        o.ssh = Some(s as Rc<dyn HostSsh>);
    });
    (r, editor, ssh, log)
}

// O20. editor-door.test.ts
#[test]
fn editor_open_here_and_over_ssh() {
    block(async {
        let (r, editor, _ssh, log) = editor_rig(true, None);
        let rt = &r.rt;
        let (host, sink) = r.host().await;

        // A workspace on this computer opens inside its worktree, then its project.
        rt.script.once::<Result<WorkspaceView, HostError>>(
            "workspaces.get",
            Ok(workspace("ws_1", "app", "local", "running", json!({ "worktree": { "path": "/wt/app", "made": true } }))),
        );
        let here = request(&host, &sink, 1, "editor.open", json!({ "workspaceId": "ws_1", "path": "src/a.ts", "line": 7, "editor": "zed" })).await;
        assert_eq!(here, json!({ "id": 1, "ok": true, "editor": "zed" }));
        assert_eq!(
            editor.opens.borrow()[0],
            EditorOpen { path: "src/a.ts".into(), line: Some(7), inside: vec!["/wt/app".into(), "/app".into()], editor: Some(EditorId::Zed), remote: None }
        );
        assert_eq!(args_of(rt, "workspaces.get", 0)["id"], json!("ws_1"));
        assert!(rt.script.calls_to("preferences.get").is_empty(), "a named editor reads no preferences");
        assert_eq!(*log.borrow(), vec!["open".to_string()]);

        // A napping workspace elsewhere is woken once the editor says it can open a remote.
        log.borrow_mut().clear();
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_2", "box1", "cloud", "napping", json!({ "folder": "/home/u/app" }))));
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.wake", Ok(workspace("ws_2", "box1-woken", "cloud", "running", json!({}))));
        let over = request(&host, &sink, 2, "editor.open", json!({ "workspaceId": "ws_2", "path": "a.rs", "editor": "vscode" })).await;
        assert_eq!(over, json!({ "id": 2, "ok": true, "editor": "vscode" }));
        assert_eq!(
            *log.borrow(),
            vec!["remoteRefusal Some(Vscode) box1".to_string(), "include".to_string(), "port ws_2 box1-woken".to_string(), "open".to_string()],
            "the editor is asked first, then the include, then the port on the woken workspace, then the editor opens"
        );
        assert_eq!(args_of(rt, "workspaces.wake", 0)["id"], json!("ws_2"));
        let remote = EditorRemote { alias: ssh_alias("box1"), folder: "/home/u/app".into(), name: "box1".into() };
        assert_eq!(editor.opens.borrow()[1], EditorOpen { path: "a.rs".into(), line: None, inside: vec![], editor: Some(EditorId::Vscode), remote: Some(remote) });

        // A running one is not woken, and without a folder the project's path is the folder.
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_3", "box2", "cloud", "running", json!({}))));
        request(&host, &sink, 3, "editor.open", json!({ "workspaceId": "ws_3", "path": "b.rs", "editor": "vscode" })).await;
        assert_eq!(rt.script.calls_to("workspaces.wake").len(), 1);
        let last = editor.opens.borrow()[2].clone();
        assert_eq!(last.remote, Some(EditorRemote { alias: ssh_alias("box2"), folder: "/app".into(), name: "box2".into() }));
        assert!(log.borrow().contains(&"port ws_3 box2".to_string()));
    })
}

// O20. editor-door.test.ts
#[test]
fn editor_refuses_without_ssh_include() {
    block(async {
        let (r, editor, _ssh, log) = editor_rig(false, None);
        let rt = &r.rt;
        let (host, sink) = r.host().await;
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_2", "box1", "cloud", "napping", json!({}))));
        let reply = request(&host, &sink, 1, "editor.open", json!({ "workspaceId": "ws_2", "path": "a.rs", "editor": "zed" })).await;
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": false, "error": ssh_include_line("box1"), "kind": "sshInclude" }).to_string());
        assert!(rt.script.calls_to("workspaces.wake").is_empty(), "no wake for a line the person has not added");
        assert!(editor.opens.borrow().is_empty());
        assert_eq!(log.borrow().iter().filter(|l| l.starts_with("port")).count(), 0);
        assert!(r.failed.borrow().is_empty(), "the switch sent it");

        // The editor that cannot open a remote costs no wake and no include read.
        let (r2, editor2, _s, log2) = editor_rig(true, Some("zed cannot open a remote folder"));
        r2.rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_2", "box1", "cloud", "napping", json!({}))));
        let (host2, sink2) = r2.host().await;
        let refused = request(&host2, &sink2, 1, "editor.open", json!({ "workspaceId": "ws_2", "path": "a.rs", "editor": "zed" })).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": "zed cannot open a remote folder" }));
        assert_eq!(*log2.borrow(), vec!["remoteRefusal Some(Zed) box1".to_string()]);
        assert!(r2.rt.script.calls_to("workspaces.wake").is_empty() && editor2.opens.borrow().is_empty());
        assert_eq!(r2.failed_ops(), vec!["editor.open"]);

        // No ssh door at all: the editor is not asked.
        let log3 = Rc::new(RefCell::new(Vec::new()));
        let editor3 = Rc::new(Editor { log: log3.clone(), opens: RefCell::new(Vec::new()), remote_refusal: None });
        let r3 = rig(|o| o.editor = Some(editor3 as Rc<dyn HostEditor>));
        r3.rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_2", "box1", "cloud", "running", json!({}))));
        let (host3, sink3) = r3.host().await;
        let none = request(&host3, &sink3, 1, "editor.open", json!({ "workspaceId": "ws_2", "path": "a.rs", "editor": "zed" })).await;
        assert_eq!(none, json!({ "id": 1, "ok": false, "error": editor_opens_here_line("box1") }));
        assert!(log3.borrow().is_empty());

        // A socket off the own road reads neither the list nor the open.
        let (ticketed, ticket_sink) = r.ticketed().await;
        for (id, op, fields) in [(1, "editor.list", json!({})), (2, "editor.open", json!({ "workspaceId": "ws_2", "path": "a.rs" }))] {
            let reply = request(&ticketed, &ticket_sink, id, op, fields).await;
            assert_eq!(reply, json!({ "id": id, "ok": false, "error": EDITOR_TICKET_REFUSAL, "kind": "ticket" }), "{op}");
        }
        let listed = request(&host, &sink, 2, "editor.list", json!({})).await;
        assert_eq!(listed, json!({ "id": 2, "ok": true, "editors": [{ "id": "vscode", "name": "VS Code" }] }));
    })
}

// O21. ssh-door.test.ts
#[test]
fn ssh_port_refuses_a_workspace_here() {
    block(async {
        let (r, _editor, ssh, log) = editor_rig(true, None);
        let rt = &r.rt;
        let (host, sink) = r.host().await;
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_1", "app", "local", "running", json!({}))));
        let here = request(&host, &sink, 1, "ssh.port", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(here, json!({ "id": 1, "ok": false, "error": ssh_copy_here_line("app") }));
        assert_eq!(r.failed_ops(), vec!["ssh.port"]);
        assert!(log.borrow().iter().all(|l| !l.starts_with("port")), "no port is asked for a copy on this computer");

        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_2", "box1", "cloud", "napping", json!({}))));
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.wake", Ok(workspace("ws_2", "box1", "cloud", "running", json!({}))));
        let there = request(&host, &sink, 2, "ssh.port", json!({ "workspaceId": "ws_2" })).await;
        assert_eq!(there, json!({ "id": 2, "ok": true, "port": 2222 }));
        assert!(log.borrow().contains(&"port ws_2 box1".to_string()));
        assert_eq!(args_of(rt, "workspaces.wake", 0)["id"], json!("ws_2"));

        log.borrow_mut().clear();
        let read = request(&host, &sink, 3, "ssh.include", json!({})).await;
        assert_eq!(read, json!({ "id": 3, "ok": true, "sshInclude": true }));
        let set = request(&host, &sink, 4, "ssh.include", json!({ "on": false })).await;
        assert_eq!(set, json!({ "id": 4, "ok": true, "sshInclude": false }));
        assert_eq!(*log.borrow(), vec!["include".to_string(), "setInclude false".to_string()]);
        assert!(!ssh.include.get());

        let (ticketed, ticket_sink) = r.ticketed().await;
        let off = request(&ticketed, &ticket_sink, 1, "ssh.port", json!({ "workspaceId": "ws_2" })).await;
        assert_eq!(off, json!({ "id": 1, "ok": false, "error": SSH_TICKET_REFUSAL, "kind": "ticket" }));
        let off = request(&ticketed, &ticket_sink, 2, "ssh.include", json!({})).await;
        assert_eq!(off["kind"], json!("ticket"));
        assert_eq!(log.borrow().len(), 2, "the door is not read for a socket off the own road");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O22. The host's doors.

// O22. serve.test.ts:1174
#[test]
fn init_keys_reaches_the_door_and_the_key_never_comes_back() {
    block(async {
        let door = Rc::new(Init { calls: RefCell::new(Vec::new()) });
        let r = rig(|o| o.init = Some(door.clone() as Rc<dyn InitDoor>));
        let (host, sink) = r.host().await;
        let got = request(&host, &sink, 1, "init.get", json!({})).await;
        assert_eq!(got["setup"], setup());
        request(&host, &sink, 2, "init.get", json!({ "on": "pl_1" })).await;
        let saved = request(&host, &sink, 3, "init.keys", json!({ "provider": "box", "key": "ascii_live_fake" })).await;
        assert_eq!(keys(&saved), vec!["id", "ok", "setup"]);
        let mut expected = setup();
        expected["keys"] = json!({ "box": true, "solari": true });
        assert_eq!(saved["setup"], expected);
        assert!(!saved.to_string().contains("ascii_live_fake"), "a key never comes back");
        request(&host, &sink, 4, "init.keys", json!({ "provider": "solari" })).await;
        assert_eq!(
            *door.calls.borrow(),
            vec![json!(["get", null]), json!(["get", "pl_1"]), json!(["keys", "box", "ascii_live_fake"]), json!(["keys", "solari", null])],
            "the door is handed each field the frame named and no other"
        );
    })
}

// O22. serve.test.ts, release.test.ts
#[test]
fn host_doors_pass_their_fields() {
    block(async {
        let terminal = Rc::new(Terminal { schemes: RefCell::new(Vec::new()) });
        let release = Rc::new(Release { checks: RefCell::new(Vec::new()) });
        let r = rig(|o| {
            o.terminal_config = Some(terminal.clone() as Rc<dyn HostTerminalConfig>);
            o.release = Some(release.clone() as Rc<dyn ReleaseDoor>);
        });
        let (host, sink) = r.host().await;

        let config = request(&host, &sink, 1, "host.terminalConfig", json!({ "scheme": "dark" })).await;
        assert_eq!(keys(&config), vec!["id", "ok", "config"]);
        assert_eq!(config["config"]["fontFamily"], json!(["Menlo"]));
        request(&host, &sink, 2, "host.terminalConfig", json!({})).await;
        assert_eq!(*terminal.schemes.borrow(), vec![Some(wsp_wire::views::project_bundle::TerminalScheme::Dark), None]);

        let got = request(&host, &sink, 3, "release.get", json!({})).await;
        assert_eq!(got["release"]["installed"], json!("0.3.7"));
        assert!(release.checks.borrow().is_empty(), "get reads nothing from the network");
        request(&host, &sink, 4, "release.check", json!({})).await;
        request(&host, &sink, 5, "release.check", json!({ "force": false })).await;
        let forced = request(&host, &sink, 6, "release.check", json!({ "force": true })).await;
        assert_eq!(*release.checks.borrow(), vec![false, false, true], "force is read as === true");
        assert_eq!(forced["release"]["installed"], json!("0.3.8"));

        r.rt.script.once::<Result<wsp_core::types::api::PreferencesSet, HostError>>("preferences.set", Err(HostError::new("stop")));
        request(&host, &sink, 7, "preferences.set", json!({ "patch": {} })).await;
        assert_eq!(args_of(&r.rt, "preferences.set", 0)["patch"], json!({}));
        r.rt.script.once::<Result<wsp_wire::views::preferences::Preferences, HostError>>("preferences.get", Err(HostError::new("stop")));
        request(&host, &sink, 8, "preferences.get", json!({})).await;
        assert_eq!(r.rt.script.calls_to("preferences.get").len(), 1);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O23. Restart.

struct Restart {
    refusal: Option<String>,
    started: Cell<u32>,
    fail: bool,
    gate: Rc<tokio::sync::Notify>,
    reply_seen: RefCell<Vec<bool>>,
    sink: RefCell<Option<Rc<RecordingSink>>>,
}

fn restart_door(fail: bool, refusal: Option<&str>) -> Rc<Restart> {
    Rc::new(Restart {
        refusal: refusal.map(String::from),
        started: Cell::new(0),
        fail,
        gate: Rc::new(tokio::sync::Notify::new()),
        reply_seen: RefCell::new(Vec::new()),
        sink: RefCell::new(None),
    })
}

#[async_trait(?Send)]
impl RestartDoor for Restart {
    fn refusal(&self) -> Option<String> {
        self.refusal.clone()
    }
    async fn restart(&self) -> Result<(), HostError> {
        self.started.set(self.started.get() + 1);
        let seen = self.sink.borrow().as_ref().map_or(false, |s| s.frames().iter().any(|f| f["ok"] == json!(true)));
        self.reply_seen.borrow_mut().push(seen);
        if self.fail {
            return Err(HostError::new("boom"));
        }
        self.gate.notified().await;
        Ok(())
    }
}

async fn settle() {
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
}

// O23. restart-door.test.ts
#[test]
fn host_restart_replies_then_restarts_once() {
    block(async {
        let door = restart_door(false, None);
        let r = rig(|o| o.restart = Some(door.clone() as Rc<dyn RestartDoor>));
        let (host, sink) = r.host().await;
        *door.sink.borrow_mut() = Some(sink.clone());
        let first = request(&host, &sink, 1, "host.restart", json!({})).await;
        assert_eq!(first, json!({ "id": 1, "ok": true }));
        settle().await;
        assert_eq!(door.started.get(), 1);
        assert_eq!(*door.reply_seen.borrow(), vec![true], "the restart starts after the reply is sent");

        let second = request(&host, &sink, 2, "host.restart", json!({})).await;
        assert_eq!(second, json!({ "id": 2, "ok": true }));
        settle().await;
        assert_eq!(door.started.get(), 1, "an ask that overlaps joins the restart under way");

        door.gate.notify_one();
        settle().await;
        request(&host, &sink, 3, "host.restart", json!({})).await;
        settle().await;
        assert_eq!(door.started.get(), 1, "a restart that went through is never started twice");
        assert!(r.failed.borrow().is_empty());
    })
}

// O23. restart-door.test.ts
#[test]
fn host_restart_is_the_own_road_and_names_what_would_stop_it() {
    block(async {
        let door = restart_door(false, None);
        let r = rig(|o| o.restart = Some(door.clone() as Rc<dyn RestartDoor>));
        let (ticketed, ticket_sink) = r.ticketed().await;
        let off = request(&ticketed, &ticket_sink, 1, "host.restart", json!({})).await;
        assert_eq!(off, json!({ "id": 1, "ok": false, "error": HOST_RESTART_TICKET_REFUSAL, "kind": "ticket" }));
        settle().await;
        assert_eq!(door.started.get(), 0);

        let none = rig(|_| {});
        let (host, sink) = none.host().await;
        let unwired = request(&host, &sink, 1, "host.restart", json!({})).await;
        assert_eq!(unwired, json!({ "id": 1, "ok": false, "error": HOST_NO_RESTART_LINE }));
        assert_eq!(none.failed_ops(), vec!["host.restart"]);

        let refusing = restart_door(false, Some("this host was started by hand; start it again there"));
        let r = rig(|o| o.restart = Some(refusing.clone() as Rc<dyn RestartDoor>));
        let (host, sink) = r.host().await;
        let refused = request(&host, &sink, 1, "host.restart", json!({})).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": "this host was started by hand; start it again there" }));
        settle().await;
        assert_eq!(refusing.started.get(), 0);
        assert_eq!(sink.replies(1).len(), 1, "no ok is sent before a refusal");
    })
}

// O23. restart-door.test.ts
#[test]
fn a_failed_restart_is_logged_and_tried_again() {
    block(async {
        let door = restart_door(true, None);
        let r = rig(|o| o.restart = Some(door.clone() as Rc<dyn RestartDoor>));
        let (host, sink) = r.host().await;
        let first = request(&host, &sink, 1, "host.restart", json!({})).await;
        assert_eq!(first, json!({ "id": 1, "ok": true }));
        settle().await;
        assert_eq!(door.started.get(), 1);
        assert_eq!(*r.logs.borrow(), vec!["host.restart failed: boom".to_string()]);

        let again = request(&host, &sink, 2, "host.restart", json!({})).await;
        assert_eq!(again, json!({ "id": 2, "ok": true }));
        settle().await;
        assert_eq!(door.started.get(), 2, "a failed restart forgets itself, so the next ask tries again");
        assert_eq!(*r.logs.borrow(), vec!["host.restart failed: boom".to_string(), "host.restart failed: boom".to_string()]);
        assert!(r.failed.borrow().is_empty(), "the failure is logged, not told to failed: the reply was already sent");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O24. Project ops.

// O24. project-import.test.ts, project-export.test.ts
#[test]
fn project_ops_hand_the_bundler_and_lander() {
    block(async {
        let sources = Rc::new(RefCell::new(Vec::new()));
        let kept = sources.clone();
        let r = rig(move |o| {
            o.projects = Some(Rc::new(move |source: &str| {
                kept.borrow_mut().push(source.to_string());
                Rc::new(Bundler { source: source.to_string() }) as Rc<dyn ProjectBundler>
            }));
            o.landing = Some(Rc::new(Lander) as Rc<dyn ProjectLander>);
        });
        let rt = &r.rt;
        let (host, sink) = r.host().await;

        let plan = json!({ "source": "/s", "branch": "main", "uncommitted": 0, "files": [], "remembered": false });
        rt.script.once::<Result<wsp_wire::views::workspace::SeedPlan, HostError>>("projects.seedPlan", Ok(from(plan)));
        let seeded = request(&host, &sink, 1, "project.seed.plan", json!({ "source": "/s", "on": "pl_1" })).await;
        assert_eq!(keys(&seeded), vec!["id", "ok", "plan"]);
        assert_eq!(seeded["plan"]["source"], json!("/s"));
        let a = args_of(rt, "projects.seedPlan", 0);
        assert_eq!((a["source"].clone(), a["on"].clone()), (json!("/s"), json!("pl_1")));
        assert!(sources.borrow().is_empty(), "the seed plan reads no folder through the bundler");

        let planned = request(&host, &sink, 2, "project.plan", json!({ "source": "/src" })).await;
        assert_eq!(keys(&planned), vec!["id", "ok", "plan"]);
        assert_eq!(planned["plan"]["source"], json!("/src"));
        assert_eq!(planned["plan"]["files"], json!(3));
        assert_eq!(*sources.borrow(), vec!["/src".to_string()], "the bundler is made over the frame's source");
        assert!(rt.script.calls_to("projects.seedPlan").len() == 1, "project.plan reads the folder, not the area");

        rt.script.once::<Result<wsp_wire::views::project_bundle::ProjectImportResult, HostError>>(
            "projects.import",
            Ok(from(json!({
                "dest": "/d", "files": 3, "bytes": 10, "parts": 1, "cut": [], "rewritten": [], "agents": [],
                "project": { "name": "app", "dest": "/d", "importedAt": WHEN },
            }))),
        );
        let imported = request(
            &host, &sink, 3, "project.import",
            json!({ "workspaceId": "ws_1", "source": "/src2", "dest": "/d", "replace": true, "carry": ["a"], "rewrite": [], "agents": ["claude"], "origin": "here" }),
        )
        .await;
        assert_eq!(keys(&imported), vec!["id", "ok", "imported"]);
        assert_eq!(imported["imported"]["dest"], json!("/d"));
        assert_eq!(*sources.borrow(), vec!["/src".to_string(), "/src2".to_string()], "the import's bundler is made over its own source");
        assert_eq!(args_of(rt, "projects.import", 0)["origin"], json!("here"));

        rt.script.once::<Result<wsp_wire::views::project_bundle::ProjectExportResult, HostError>>(
            "projects.export",
            Ok(from(json!({ "dest": "/e", "files": 2, "bytes": 5, "excluded": [], "agents": [] }))),
        );
        let exported = request(&host, &sink, 4, "project.export", json!({ "workspaceId": "ws_1", "source": "/work", "dest": "/e" })).await;
        assert_eq!(keys(&exported), vec!["id", "ok", "exported"]);
        assert_eq!(exported["exported"]["dest"], json!("/e"));
        assert_eq!(rt.script.calls_to("projects.export").len(), 1);
        assert_eq!(sources.borrow().len(), 2, "an export makes no bundler: it hands the lander");
        assert!(r.failed.borrow().is_empty());
    })
}
