use std::cell::{Cell, RefCell};

use std::collections::VecDeque;

use std::rc::Rc;

use std::sync::{Arc, Mutex};

use support::*;

use wsp_core::console;

use wsp_core::types::internal::lost_session_log_line;

use wsp_wire::types::env::{NO_SLATE_MCP_ARG, SCOPED_MCP_ARG, THREAD_ENV, TURN_TOKEN_ENV};

use wsp_wire::views::harness::start_picks;

use wsp_wire::views::preferences::default_preferences;

use wsp_wire::wire::runtime::{SessionWarmResult, SessionWarmResultWarm};

use wsp_words::place_word::{place_said, PlaceRef};

use wsp_words::sub_thread::{cloud_added_line, cloud_removed_line};

fn err(message: &str, kind: Option<Kind>) -> HostError {
    match kind {
        Some(k) => HostError::with_kind(message, k),
        None => HostError::new(message),
    }
}

fn warm_result(warm: SessionWarmResultWarm) -> SessionWarmResult {
    SessionWarmResult { warm }
}

fn started() -> SessionWarmResult {
    warm_result(SessionWarmResultWarm::Started)
}

fn standing() -> SessionWarmResult {
    warm_result(SessionWarmResultWarm::Standing)
}

fn none() -> SessionWarmResult {
    warm_result(SessionWarmResultWarm::None)
}

/// The calls, in order, must hold each of `wanted` one after the other (others may sit between).
fn in_order(calls: &[String], wanted: &[&str]) {
    let mut at = 0;
    for w in wanted {
        match calls[at..].iter().position(|c| c == w) {
            Some(i) => at += i + 1,
            None => panic!("{w} not found in order after position {at} of {calls:?}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// served_to

fn wsp_server(args: &[&str]) -> McpServerSpec {
    McpServerSpec { command: "/usr/local/bin/node".into(), args: args.iter().map(|a| a.to_string()).collect(), no_slate: None }
}

// ---------------------------------------------------------------------------------------------------------------
// clouds_told

fn provider(name: &str) -> PlaceView {
    serde_json::from_value(json!({ "id": name, "kind": "provider", "name": name, "default": false })).expect("a provider row")
}

fn computer(name: &str) -> PlaceView {
    serde_json::from_value(json!({ "id": name, "kind": "computer", "name": name, "default": true })).expect("a computer row")
}

/// What the cloud named `name` is called to a person.
fn said_of(name: &str) -> String {
    place_said(&PlaceRef { id: name.to_string(), name: name.to_string(), kind: Some("provider".into()) })
}

fn with_clouds(host: &FakeHost, clouds: Option<Vec<&str>>) {
    let mut doc = json!({ "harness": "claude" });
    if let Some(c) = clouds {
        doc["clouds"] = json!(c);
    }
    host.add_record("t1", "w1", thread_record(doc));
}

// ---------------------------------------------------------------------------------------------------------------
// launch_picks

fn catalog_json() -> Value {
    json!({
        "harness": "claude", "label": "Claude Code", "source": "harness", "version": "2.1.0",
        "models": [
            { "value": "claude-opus-5", "label": "Opus 5", "isDefault": true, "efforts": ["low", "high", "max"], "contextWindows": ["1m"] },
            { "value": "claude-sonnet-5", "label": "Sonnet 5", "efforts": ["low", "high", "max"], "contextWindows": [] },
        ],
        "efforts": [{ "value": "low", "label": "low" }, { "value": "high", "label": "high", "isDefault": true }, { "value": "max", "label": "max" }],
        "contextWindows": [{ "value": "1m", "label": "1M" }],
        "permissionModes": [
            { "value": "default", "label": "default" },
            { "value": "acceptEdits", "label": "acceptEdits" },
            { "value": "bypassPermissions", "label": "bypassPermissions", "isDefault": true },
        ],
        "steers": false, "renames": false, "images": false,
    })
}

fn catalog_over(over: Value) -> HarnessCatalog {
    let mut whole = catalog_json();
    for (k, v) in over.as_object().expect("over is an object") {
        whole[k] = v.clone();
    }
    serde_json::from_value(whole).expect("a catalog the fixture builds")
}

fn catalog() -> HarnessCatalog {
    catalog_over(json!({}))
}

fn set_catalog(host: &FakeHost, c: Option<HarnessCatalog>) {
    host.hooks.set::<dyn Fn(&str, &Entry, &dyn HarnessAdapter) -> LocalBoxFuture<'static, Result<Option<HarnessCatalog>, HostError>>>(
        "LaunchCtx::catalog_on",
        Rc::new(move |_h: &str, _e: &Entry, _a: &dyn HarnessAdapter| {
            let c = c.clone();
            async move { Ok(c) }.boxed_local()
        }),
    );
}

fn set_ran(host: &FakeHost, ran: Rc<RefCell<RanPicks>>) {
    host.hooks.set::<dyn Fn(&str, &str, Option<&str>) -> RanPicks>(
        "LaunchCtx::ran_on",
        Rc::new(move |_w: &str, _t: &str, _s: Option<&str>| ran.borrow().clone()),
    );
}

fn ran(model: Option<&str>, effort: Option<&str>, window: Option<&str>) -> RanPicks {
    RanPicks {
        model: model.map(String::from),
        effort: effort.map(String::from),
        context_window: window.map(String::from),
        permission_mode: None,
    }
}

fn picks(model: Option<&str>, effort: Option<&str>, mode: Option<&str>) -> StartPicks {
    StartPicks { model: model.map(String::from), effort: effort.map(String::from), permission_mode: mode.map(String::from), fast: None }
}

fn launch_picks_of(p: StartPicks, window: Option<&str>) -> LaunchPicks {
    LaunchPicks { picks: p, context_window: window.map(String::from) }
}

/// A host holding workspace w1 whose catalog is `catalog()`, and the area over it.
fn picks_rig() -> (Rc<FakeHost>, LaunchArea, Entry) {
    let host = FakeHost::new();
    let e = host.add_entry(entry("w1", json!({})));
    set_catalog(&host, Some(catalog()));
    let area = LaunchArea::new(host.clone());
    (host, area, e)
}

async fn read_picks(area: &LaunchArea, e: &Entry, asked: &LaunchAsked) -> Result<PicksRead, HostError> {
    let adapter: Arc<dyn HarnessAdapter> = Arc::new(FakeAdapter::warming());
    area.launch_picks(e, "claude", adapter, &default_preferences(), "t1", asked).await
}

// ---------------------------------------------------------------------------------------------------------------
// launch_fixed

fn fixed_rig() -> (Rc<FakeHost>, LaunchArea, Entry) {
    let host = FakeHost::new();
    let e = host.add_entry(entry("w1", json!({})));
    let area = LaunchArea::new(host.clone());
    (host, area, e)
}

// ---------------------------------------------------------------------------------------------------------------
// warm

#[derive(Clone, Debug, PartialEq)]
struct Warmed {
    cwd: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    permission_mode: Option<String>,
    context_window: Option<String>,
    fast: Option<bool>,
    mcp_servers: Option<Value>,
    server_values: Option<ServerValues>,
    version: Option<String>,
}

/// An adapter whose `warm` road records what it was handed and answers a kept agent named `kept-<n>`.
struct FakeAdapter {
    warms: bool,
    mcp: bool,
    denies: bool,
    fail: Option<String>,
    log: Mutex<Vec<Warmed>>,
}

impl FakeAdapter {
    fn warming() -> FakeAdapter {
        FakeAdapter { warms: true, mcp: false, denies: false, fail: None, log: Mutex::new(Vec::new()) }
    }
    fn warmed(&self) -> Vec<Warmed> {
        self.log.lock().unwrap().clone()
    }
}

impl HarnessAdapter for FakeAdapter {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("the warm tests never start a turn".into()))
    }
    fn warm(&self, o: HarnessWarmOptions) -> Option<Result<Box<dyn KeptAgent>, AdapterError>> {
        if !self.warms {
            return None;
        }
        let mut log = self.log.lock().unwrap();
        log.push(Warmed {
            cwd: o.cwd,
            model: o.model,
            effort: o.effort,
            permission_mode: o.permission_mode,
            context_window: o.context_window,
            fast: o.fast,
            mcp_servers: o.mcp_servers.map(|m| serde_json::to_value(m).expect("servers serialize")),
            server_values: o.server_values,
            version: o.version,
        });
        if let Some(why) = &self.fail {
            return Some(Err(AdapterError(why.clone())));
        }
        Some(Ok(Box::new(FakeKept { tag: format!("kept-{}", log.len()) })))
    }
    fn steers(&self) -> bool {
        false
    }
    fn mcp_servers(&self) -> bool {
        self.mcp
    }
    fn denies_servers(&self) -> bool {
        self.denies
    }
}

/// A kept agent that names itself through its session file's folder.
struct FakeKept {
    tag: String,
}

impl KeptAgent for FakeKept {
    fn next(&self, _turn: KeptTurn) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("the warm tests never take a turn".into()))
    }
    fn close(&self, _now: bool) -> LocalBoxFuture<'static, ()> {
        futures::future::ready(()).boxed_local()
    }
    fn exited(&self) -> SharedFuture<()> {
        futures::future::ready(()).boxed_local().shared()
    }
    fn session_file(&self) -> Option<SessionFile> {
        Some(SessionFile { folder: self.tag.clone(), name: String::new(), depth: 0 })
    }
}

fn harness_of(h: &KeptHand) -> String {
    h.warm.as_ref().expect("a warm hold names its harness").harness.clone()
}

fn kept_tag(h: &KeptHand) -> String {
    h.agent.session_file().expect("a fake kept agent").folder
}

type Standing = Rc<RefCell<IndexMap<(String, String), (String, KeptLaunch)>>>;

/// The host with workspace w1, the fake adapter on every `adapter_for`, and the keeper's one road the area reads and
/// writes: `hold_warm` stands an agent per workspace and agent, `reap_kept` takes one away.
struct Rig {
    host: Rc<FakeHost>,
    area: LaunchArea,
    adapter: Arc<FakeAdapter>,
    asks: Rc<RefCell<Vec<(String, AdapterAsk)>>>,
    held: Rc<RefCell<Vec<(String, KeptHand)>>>,
    standing: Standing,
}

fn rig() -> Rig {
    rig_with(FakeAdapter::warming())
}

fn rig_with(adapter: FakeAdapter) -> Rig {
    let host = FakeHost::new();
    host.add_entry(entry("w1", json!({})));
    host.hooks.set::<dyn Fn() -> Env>("LaunchCtx::vault", Rc::new(Env::default));
    *host.ids.uuids.borrow_mut() = (1..=8).map(|n| format!("thr-{n}")).collect();
    let adapter = Arc::new(adapter);
    let asks: Rc<RefCell<Vec<(String, AdapterAsk)>>> = Rc::new(RefCell::new(Vec::new()));
    let held: Rc<RefCell<Vec<(String, KeptHand)>>> = Rc::new(RefCell::new(Vec::new()));
    let standing: Standing = Rc::new(RefCell::new(IndexMap::new()));
    let (a, log) = (adapter.clone(), asks.clone());
    host.hooks.set::<dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>>(
        "LaunchCtx::adapter_for",
        Rc::new(move |_e: &Entry, ask: AdapterAsk| {
            let h = ask.named.clone().unwrap_or_default();
            log.borrow_mut().push((h.clone(), ask));
            Ok(BuiltAdapter { harness: h, adapter: a.clone() })
        }),
    );
    let warms = adapter.warms;
    host.hooks.set::<dyn Fn(&dyn HarnessAdapter) -> bool>("LaunchCtx::adapter_warms", Rc::new(move |_a: &dyn HarnessAdapter| warms));
    let (st, hd) = (standing.clone(), held.clone());
    host.hooks.set::<dyn Fn(&str, KeptHand)>(
        "LaunchCtx::hold_warm",
        Rc::new(move |t: &str, hold: KeptHand| {
            st.borrow_mut().insert((hold.workspace_id.clone(), harness_of(&hold)), (t.to_string(), hold.launch.clone()));
            hd.borrow_mut().push((t.to_string(), hold));
        }),
    );
    let st = standing.clone();
    host.hooks.set::<dyn Fn(&str, &str) -> Option<(String, KeptLaunch)>>(
        "LaunchCtx::warm_on",
        Rc::new(move |w: &str, h: &str| st.borrow().get(&(w.to_string(), h.to_string())).cloned()),
    );
    let st = standing.clone();
    host.hooks.set::<dyn Fn(&str)>("LaunchCtx::reap_kept", Rc::new(move |t: &str| st.borrow_mut().retain(|_, v| v.0 != t)));
    let area = LaunchArea::new(host.clone());
    Rig { host, area, adapter, asks, held, standing }
}

fn ask() -> WarmAsk {
    WarmAsk { harness: Some("claude".into()), ..Default::default() }
}

fn ask_with(model: Option<&str>, effort: Option<&str>, mode: Option<&str>) -> WarmAsk {
    WarmAsk { model: model.map(String::from), effort: effort.map(String::from), permission_mode: mode.map(String::from), ..ask() }
}

async fn warm(r: &Rig, a: WarmAsk) -> Result<SessionWarmResult, HostError> {
    r.area.warm("w1", a, None).await
}

fn thread_scope() -> Caller {
    let by: ThreadScope =
        serde_json::from_value(json!({ "kind": "thread", "threadId": "t-lead", "workspaceId": "w1", "rootThreadId": "t-lead" }))
            .expect("a scope");
    Caller::Scoped { origin: WorkspaceOrigin::Here, by }
}

/// A launch's picks as the held agent's `launch` writes them.
fn held_picks(r: &Rig, at: usize) -> Value {
    serde_json::to_value(&r.held.borrow()[at].1.launch.picks).unwrap()
}

/// The gates `confine_setup` waits at, one per call in order; a call with no gate left passes.
fn gate_confine_setup(host: &FakeHost, gates: Vec<futures::channel::oneshot::Receiver<Result<(), HostError>>>) {
    let queue = Rc::new(RefCell::new(VecDeque::from(gates)));
    host.hooks.set::<dyn Fn(&Entry, &str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
        "LaunchCtx::confine_setup",
        Rc::new(move |_e: &Entry, _h: &str| {
            let gate = queue.borrow_mut().pop_front();
            async move {
                match gate {
                    Some(g) => g.await.expect("the gate is opened"),
                    None => Ok(()),
                }
            }
            .boxed_local()
        }),
    );
}

type Asked = tokio::task::JoinHandle<Result<SessionWarmResult, HostError>>;

fn ask_in_flight(r: &Rig, workspace: &'static str, a: WarmAsk) -> Asked {
    let area = r.area.clone();
    tokio::task::spawn_local(async move { area.warm(workspace, a, None).await })
}

// ---------------------------------------------------------------------------------------------------------------
// seed_after_loss

const FIRST: &str = "01a1424a-7c2e-7d31-9f00-5b2a1c3d4e5f";

const SECOND: &str = "01a1424b-fa29-7aa0-8c11-0d9e8f7a6b5c";

fn lost_at() -> LostAt {
    LostAt { session_id: FIRST.into(), thread_id: SECOND.into(), workspace_id: "ws_a".into() }
}

/// A machine exec that says what the machine holds and keeps every command it was asked.
struct HeldExec {
    commands: Mutex<Vec<String>>,
}

impl HarnessExec for HeldExec {
    fn exec(&self, command: &str, _env: Option<&Env>) -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
        self.commands.lock().unwrap().push(command.to_string());
        futures::future::ready(Ok("boot b1; rollout files: none; index row: state_5.sqlite".to_string())).boxed_local()
    }
}

/// A seed that counts its asks and answers the thread so far.
fn counting_seed(asked: Rc<Cell<u32>>) -> Seed {
    Rc::new(move || {
        asked.set(asked.get() + 1);
        async { Ok("the thread so far".to_string()) }.boxed_local()
    })
}

/// A `session_held` road that reads the machine through the exec it is handed and keeps the sessions it was asked for.
fn reading(asked: Rc<RefCell<Vec<String>>>) -> SessionHeld {
    Rc::new(move |session: String, exec: Arc<dyn HarnessExec>| {
        asked.borrow_mut().push(session.clone());
        async move { exec.exec(&format!("held {session}"), None).await.map_err(|e| e.message) }.boxed_local()
    })
}
