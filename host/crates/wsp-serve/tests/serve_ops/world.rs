use support::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::port::{TaskStop, TurnImage};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, Frame};
use wsp_core::devices::{DeviceDoor, HeldDevice};
use wsp_core::forwards::{ForwardEvent, ForwardsSource};
use wsp_core::types::api::*;
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::wiring::{
    AccessMove, Detach, EditorOpen, EditorRemote, FolderListAsk, HostEditor, HostFolders, HostSsh, HostTerminalConfig, InitDoor,
    LandRequest, LandedProject, Local, PackedProject, PackedState, ProjectBundler, ProjectLander, SessionHandle, StateRequest, SteerTaken,
};
use wsp_engine::vault::CacheRule;
use wsp_serve::serve::door::Served;
use wsp_serve::serve::served::Payload;
use wsp_serve::serve::socket::Socket;
use wsp_serve::serve::{
    FailedFn, ReleaseDoor, RestartDoor, ServeOptions, Unlisten, NO_BUNDLER, NO_EDITOR, NO_FOLDERS, NO_INIT, NO_LANDER, NO_RELEASE, NO_SSH,
    NO_TERMINAL_CONFIG,
};
use wsp_serve::serve_broker::{BrokerDoor, CloudSignIn};
use wsp_slates::pending::wire::{SlateEmpty, SlateView, SlateWriteAnswer};
use wsp_wire::types::agents_report::{AGENTS_KEY_REFUSAL, SIGN_IN_LINE_REFUSAL};
use wsp_wire::types::init_job::InitKeys;
use wsp_wire::types::release::{HOST_NO_RESTART_LINE, HOST_RESTART_TICKET_REFUSAL};
use wsp_wire::types::thread_defaults::ENV_VALUE_REFUSAL;
use wsp_wire::views::activity::ActivityEnded;
use wsp_wire::views::place_more::{PlaceSpend, PlaceView};
use wsp_wire::views::preferences::{
    editor_opens_here_line, ssh_copy_here_line, ssh_include_line, EditorChoice, EditorId, EDITOR_TICKET_REFUSAL, SSH_TICKET_REFUSAL,
};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_more::{WorkspaceOrigin, WorkspaceView};
use wsp_wire::wire::daemon_version::{place_behind_line, place_daemon_behind};
use wsp_wire::wire::limits::PortForward;
use wsp_wire::wire::machine_link::no_such_place_refusal;
use wsp_wire::wire::place_link::{PlaceReport, PLACES_TICKET_REFUSAL};
use wsp_wire::wire::runtime::{SessionAnswerResult, SessionStartOutcome, SessionStartResult};
use wsp_words::exit::{refusal, HostError, Kind};
use wsp_words::place_word::{place_said, PlaceRef};
use wsp_words::project_path::ssh_alias;
use wsp_words::views::activity::{processes_left_refusal, LeftVerb};
use wsp_words::words::computer::device_held_refusal;
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
    /// The browser `wsp init` let in: a device marked `here`, which takes the person's own road.
    async fn browser(&self) -> (Rc<Socket>, Rc<RecordingSink>) {
        let mut view = held(None, None);
        view.view.here = Some(from(json!(true)));
        self.as_device(view).await
    }
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
    stops: RefCell<Vec<(String, i64)>>,
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
    fn stop(&self, workspace_id: &str, port: i64) -> bool {
        self.stops.borrow_mut().push((workspace_id.to_string(), port));
        let mut open = self.open.borrow_mut();
        match open.iter().position(|f| f.workspace_id == workspace_id && f.port.get() == port) {
            Some(i) => {
                open.remove(i);
                true
            }
            None => false,
        }
    }
    fn on(&self, _f: Rc<dyn Fn(&ForwardEvent)>) -> Detach {
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
    async fn read(
        &self,
        scheme: Option<wsp_wire::views::project_bundle::TerminalScheme>,
    ) -> Result<wsp_wire::views::project_bundle::TerminalConfig, HostError> {
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
    json!({ "keys": { "box": false, "solari": true }, "keyProvider": "solari", "home": "/Users/me", "pricing": null })
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
            keys.0.insert(p.clone(), true);
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
        Ok(from(
            json!({ "source": self.source, "repo": true, "files": 3, "bytes": 10, "secrets": [], "excluded": [], "skipped": [], "agents": [] }),
        ))
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
