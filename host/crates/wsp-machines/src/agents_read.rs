// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/agents-read.ts` at 65f6b89c7: the agents report off one computer or workspace, and every
//! sign-in, skill, server and plugin write there. Two units: `targets` (spec `agents_read_targets.md`) and `sign_ins`
//! (`agents_read_sign_ins.md`). The ports declared here are the methods wsp-core's empty `types::pending::{AgentsReader,
//! AgentsActs, SkillsActs, ServersActs, PluginsActs, ServerIcons, CallbackForwards}` handles stand for (ruling 2 of
//! #2448 comment 6400658264).
pub mod sign_ins;
pub mod targets;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;

use wsp_core::daemon_channel::DaemonChannel;
use wsp_core::types::pending::Clock;
use wsp_engine::machine::Machine;
use wsp_engine::target_line::TargetLogin;
use wsp_wire::types::agents_report::{
    AgentRow, AgentSignInState, AgentsProject, AgentsReport, AgentsSignInEvent, AgentsSignInEventState, AgentsSignInRun,
    AgentsTarget, McpRow, McpScope, PageReach, PluginAsk, PluginRow, ServerAdd, ServerAsk, ServerToolsAnswer, SignInLine,
    SkillAdded, SkillHit, SkillPreview, SkillRow,
};
use wsp_wire::types::thread_defaults::{AgentSetupSet, AgentSetupView};
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::place::PlaceProvisionRow;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::wire::capabilities::PauseMode;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_wire::wire::place_link::PlaceReport;
use wsp_words::exit::HostError;

/// agents-read.ts:16 `AgentsRead`: the report less what the runtime stamps on it, and the login its lines are handed to.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentsRead {
    pub home: String,
    pub user: String,
    pub agents: Vec<AgentRow>,
    pub skills: Vec<SkillRow>,
    pub servers: Vec<McpRow>,
    pub plugins: Option<Vec<PluginRow>>,
    pub refused: Vec<String>,
    pub projects: Option<Vec<AgentsProject>>,
    pub run_as: Option<String>,
}

/// agents-read.ts:19.
pub fn guest_wsp_mcp() -> McpServerSpec {
    McpServerSpec { command: "wsp".to_string(), args: vec!["mcp".to_string()], no_slate: None }
}

/// A machine's `exec` as a box target hands it: through the place door.
pub type BoxExec = Rc<dyn Fn(String, Option<u64>, Option<Vec<u8>>) -> LocalBoxFuture<'static, Result<ExecResult, HostError>>>;

/// agents-read.ts:27-36 `AgentsOn`: where a read runs.
#[derive(Clone)]
pub enum AgentsOn {
    Here {
        projects: Option<Vec<AgentsProject>>,
        stores: Option<IndexMap<String, String>>,
    },
    Box {
        name: Option<String>,
        exec: BoxExec,
        login: BoxLogin,
        sign_ins: Option<IndexMap<String, AgentSignInState>>,
        versions: Option<IndexMap<String, String>>,
        setup_rows: Option<Vec<PlaceProvisionRow>>,
        logins: Option<String>,
        relayed: Option<bool>,
        projects: Option<Vec<AgentsProject>>,
        stores: Option<IndexMap<String, String>>,
    },
    Machine {
        machine: Arc<dyn Machine>,
        projects: Option<Vec<AgentsProject>>,
        relayed: Option<bool>,
        brokered: Option<bool>,
        stores: Option<IndexMap<String, String>>,
        seeds: Option<IndexMap<String, String>>,
    },
}

/// A box target's login: `HOME` and `PATH` off its report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoxLogin {
    pub home: Option<String>,
    pub path: Option<String>,
}

/// agents-read.ts:39: the one project an act works in.
pub fn project_of(on: &AgentsOn) -> Option<String> {
    match projects_of(on) {
        Some(projects) if projects.len() == 1 => Some(projects[0].path.clone()),
        _ => None,
    }
}

/// The projects an `AgentsOn` covers, where it names any.
fn projects_of(on: &AgentsOn) -> Option<&Vec<AgentsProject>> {
    match on {
        AgentsOn::Here { projects, .. } | AgentsOn::Box { projects, .. } | AgentsOn::Machine { projects, .. } => projects.as_ref(),
    }
}

/// agents-read.ts:44 (A6).
pub fn page_reach_of(on: &AgentsOn, run_as: Option<&str>) -> PageReach {
    match on {
        AgentsOn::Here { .. } => PageReach::Here,
        AgentsOn::Box { relayed, .. } | AgentsOn::Machine { relayed, .. } if *relayed == Some(true) && run_as.is_none() => PageReach::Relay,
        _ => PageReach::None,
    }
}

/// agents-read.ts:48.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerToolsAsk {
    pub key: String,
    pub agent: String,
    pub name: String,
    pub refresh: Option<bool>,
}

/// agents-read.ts:56.
pub const SERVER_NOT_SET_UP: &str = "server-not-set-up";
/// agents-read.ts:59.
pub const SERVER_OFF: &str = "server-off";
/// agents-read.ts:190.
pub const NO_AGENTS_READER: &str = "this runtime carries no agents reader; the host that serves the app wires one";
/// agents-read.ts:248.
pub const SKILLS_SEARCH_LIMIT: u32 = 20;

/// agents-read.ts:57: an error carrying `code: "server-not-set-up"`.
pub fn is_server_not_set_up(e: &HostError) -> bool {
    e.code.as_deref() == Some(SERVER_NOT_SET_UP)
}

/// agents-read.ts:60: an error carrying `code: "server-off"`.
pub fn is_server_off(e: &HostError) -> bool {
    e.code.as_deref() == Some(SERVER_OFF)
}

/// One server resolved for a client, with the login a command there runs as.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedServer {
    pub spec: McpServerSpec,
    pub login: Option<TargetLogin>,
}

/// agents-read.ts:64-79 `AgentsReader`.
#[async_trait(?Send)]
pub trait AgentsReader {
    async fn read(&self, on: &AgentsOn, latest: bool) -> Result<AgentsRead, HostError>;
    /// Optional in the TS; a reader with nothing kept does nothing.
    fn forget(&self, key: &str);
    async fn tools(&self, on: &AgentsOn, ask: ServerToolsAsk) -> Result<ServerToolsAnswer, HostError>;
    /// Optional in the TS: false where the reader has no `server` road.
    fn has_server(&self) -> bool;
    async fn server(&self, on: &AgentsOn, agent: &str, name: &str) -> Result<ResolvedServer, HostError>;
}

/// agents-read.ts:82-90 `PtyLink`.
#[derive(Clone)]
pub struct PtyLink {
    pub op: Rc<dyn Fn(String, serde_json::Map<String, serde_json::Value>) -> LocalBoxFuture<'static, Result<serde_json::Map<String, serde_json::Value>, HostError>>>,
    /// Answers the reader's removal.
    pub on_event: Rc<dyn Fn(Rc<dyn Fn(&serde_json::Map<String, serde_json::Value>)>) -> Box<dyn FnOnce()>>,
    pub closed: Option<Shared<LocalBoxFuture<'static, ()>>>,
    pub bare: Option<bool>,
}

/// agents-read.ts:93-106 `SignInAsk`. wsp-core's `types::pending::SignInAsk` is a stand-in of three fields; this is
/// the whole ask.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SignInAsk {
    pub agent: String,
    pub server: Option<String>,
    pub terminal: Option<bool>,
    pub tool_there: Option<bool>,
    pub login: Option<bool>,
    pub scope: Option<McpScope>,
    pub project: Option<String>,
}

/// `Omit<AgentsSignInEvent, "type" | "signInId">`: one step of a sign-in.
#[derive(Clone, Debug, PartialEq)]
pub struct SignInStep {
    pub state: AgentsSignInEventState,
    pub url: Option<String>,
    pub code: Option<String>,
    pub paste: Option<bool>,
    pub said: Option<String>,
    pub pty_id: Option<String>,
}

/// A code writer a run hands over.
pub type CodeWriter = Rc<dyn Fn(String) -> LocalBoxFuture<'static, Result<(), HostError>>>;

/// agents-read.ts:111-117 `SignInRun`.
#[derive(Clone)]
pub struct SignInRun {
    pub link: PtyLink,
    pub emit: Rc<dyn Fn(SignInStep)>,
    pub typing: Rc<dyn Fn(Option<CodeWriter>)>,
    pub stop: Shared<LocalBoxFuture<'static, ()>>,
    pub forward: Option<Rc<dyn SignInForward>>,
}

/// agents-read.ts:122-126 `SignInForward`.
#[async_trait(?Send)]
pub trait SignInForward {
    async fn arm(&self, url: &str) -> Result<bool, HostError>;
    async fn deliver(&self, landed: &str) -> Result<(), HostError>;
    fn close(&self);
}

/// agents-read.ts:129-133 `CallbackForwards`.
pub trait CallbackForwards {
    fn reaches(&self, target: &AgentsTarget) -> bool;
    fn open(&self, target: &AgentsTarget) -> Option<Rc<dyn SignInForward>>;
}

/// A planned sign-in: the run the acts hand back.
pub type SignInPlan = Box<dyn FnOnce(SignInRun) -> LocalBoxFuture<'static, Result<(), HostError>>>;

/// agents-read.ts:136-145 `AgentsActs`.
#[async_trait(?Send)]
pub trait AgentsActs {
    async fn sign_in_line(&self, on: &AgentsOn, ask: &SignInAsk) -> Result<SignInLine, HostError>;
    async fn sign_in(&self, on: &AgentsOn, ask: &SignInAsk) -> Result<SignInPlan, HostError>;
    /// `way` "key" is the API key of an agent that mints a token.
    async fn key(&self, agent: &str, key: &str, way: Option<&str>) -> Result<(), HostError>;
    /// Answers the config file written.
    async fn add_tools(&self, on: &AgentsOn, agent: &str) -> Result<String, HostError>;
}

/// agents-read.ts:148 `SkillAsk`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillAsk {
    pub name: String,
    pub project: Option<bool>,
}

/// `skills.add`'s ask.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillAddAsk {
    pub skill: String,
    pub agents: Option<Vec<String>>,
    pub project: Option<bool>,
}

/// agents-read.ts:155-162 `SkillsActs`.
#[async_trait(?Send)]
pub trait SkillsActs {
    async fn search(&self, q: &str, limit: u32) -> Result<Vec<SkillHit>, HostError>;
    async fn get(&self, skill: &str) -> Result<SkillPreview, HostError>;
    async fn preview(&self, on: &AgentsOn, ask: &SkillAsk) -> Result<SkillPreview, HostError>;
    async fn add(&self, on: &AgentsOn, ask: &SkillAddAsk) -> Result<SkillAdded, HostError>;
    /// Answers the removed paths.
    async fn remove(&self, on: &AgentsOn, ask: &SkillAsk) -> Result<Vec<String>, HostError>;
    /// Answers the paths switched.
    async fn toggle(&self, on: &AgentsOn, ask: &SkillAsk, to: bool) -> Result<Vec<String>, HostError>;
}

/// agents-read.ts:166-170 `ServersActs`: each answers the config file written.
#[async_trait(?Send)]
pub trait ServersActs {
    async fn add(&self, on: &AgentsOn, ask: &ServerAdd) -> Result<String, HostError>;
    async fn remove(&self, on: &AgentsOn, ask: &ServerAsk) -> Result<String, HostError>;
    async fn toggle(&self, on: &AgentsOn, ask: &ServerAsk, to: bool) -> Result<String, HostError>;
}

/// agents-read.ts:174-176 `PluginsActs`.
#[async_trait(?Send)]
pub trait PluginsActs {
    async fn toggle(&self, on: &AgentsOn, ask: &PluginAsk, to: bool) -> Result<PluginRow, HostError>;
}

/// agents-read.ts:180-187 `ServerIcons`.
#[async_trait(?Send)]
pub trait ServerIcons {
    fn folder(&self) -> String;
    async fn icon(&self, host: &str, refresh: Option<bool>) -> Result<Option<String>, HostError>;
    fn forget(&self);
}

/// agents-read.ts:193-207 `AgentsWorkspace`.
#[derive(Clone)]
pub struct AgentsWorkspace {
    pub name: String,
    pub phase: WorkspacePhase,
    pub local: bool,
    pub machine: Arc<dyn Machine>,
    pub project: AgentsProject,
    pub stores: Option<IndexMap<String, String>>,
    pub seeds: Option<IndexMap<String, String>>,
    pub brokered: Option<bool>,
    pub pause_mode: Option<PauseMode>,
}

/// One step of `ghThere`'s setup stream: `(detail, at, row)`.
pub type GhStep = Rc<dyn Fn(String, Option<String>, Option<PlaceProvisionRow>)>;

/// The place door calls agents-read makes (`o.places()`); the join implements it over `places::types::PlaceDoor`.
#[async_trait(?Send)]
pub trait AgentsPlaces {
    async fn list(&self, now: i64) -> Result<Vec<PlaceView>, HostError>;
    async fn report_of(&self, place_id: &str) -> Result<Option<PlaceReport>, HostError>;
    async fn logins_again(&self, place_id: &str) -> Result<(), HostError>;
    fn sign_ins_at(&self, place_id: &str, agents: &[String]) -> Option<IndexMap<String, AgentSignInState>>;
    async fn exec(&self, place_id: &str, cmd: &str, timeout_ms: Option<u64>, stdin: Option<Vec<u8>>) -> Result<ExecResult, HostError>;
    async fn gh_there(&self, place_id: &str, on_step: GhStep) -> Result<Vec<PlaceProvisionRow>, HostError>;
    async fn key_landed(&self, agent: &str) -> Result<(), HostError>;
    async fn login_landed(&self, place_id: &str, agent: &str) -> Result<(), HostError>;
}

/// A daemon event pushed on a sign-in's channel.
pub type DaemonEvent = serde_json::Map<String, serde_json::Value>;

/// agents-read.ts:209-242 `AgentsReadOptions`.
#[derive(Clone)]
pub struct AgentsReadOptions {
    pub reader: Option<Rc<dyn AgentsReader>>,
    pub places: Rc<dyn Fn() -> Option<Rc<dyn AgentsPlaces>>>,
    pub workspace: Rc<dyn Fn(String, Option<Caller>) -> LocalBoxFuture<'static, Result<AgentsWorkspace, HostError>>>,
    pub place_stores: Option<Rc<dyn Fn(&str, &str) -> IndexMap<String, String>>>,
    pub stores_here: Option<Rc<dyn Fn() -> LocalBoxFuture<'static, Result<IndexMap<String, String>, HostError>>>>,
    pub projects: Option<Rc<dyn Fn(String) -> LocalBoxFuture<'static, Result<Vec<AgentsProject>, HostError>>>>,
    pub acts: Option<Rc<dyn AgentsActs>>,
    pub skills: Option<Rc<dyn SkillsActs>>,
    pub servers: Option<Rc<dyn ServersActs>>,
    pub plugins: Option<Rc<dyn PluginsActs>>,
    pub icons: Option<Rc<dyn ServerIcons>>,
    pub latest_on: Option<Rc<dyn Fn() -> LocalBoxFuture<'static, bool>>>,
    pub channel: Rc<dyn Fn(AgentsTarget, Rc<dyn Fn(DaemonEvent)>, Option<Caller>) -> LocalBoxFuture<'static, Result<Rc<dyn DaemonChannel>, HostError>>>,
    pub changed: Rc<dyn Fn(Option<AgentsTarget>)>,
    pub clock: Rc<dyn Clock>,
    pub relayed: Option<Rc<dyn Fn() -> bool>>,
    /// The host's log; none warns.
    pub log: Option<Rc<dyn Fn(&str)>>,
    pub setup_of: Option<Rc<dyn Fn(&str, &str) -> Option<AgentSetupView>>>,
    pub setup_write: Option<Rc<dyn Fn(String, String, AgentSetupSet) -> LocalBoxFuture<'static, Result<(), HostError>>>>,
    /// Random bytes for a sign-in's id (rule 3.16).
    pub random_bytes: Rc<dyn Fn(usize) -> Vec<u8>>,
    /// `console.warn`, where `log` is none.
    pub warn: Rc<dyn Fn(&str)>,
}

/// A napping workspace, which nothing here wakes.
#[derive(Clone, Debug, PartialEq)]
pub struct Napping {
    pub napping: String,
    pub pause_mode: Option<PauseMode>,
}

/// Where a target's lines run, or the napping workspace.
#[derive(Clone)]
pub enum OnOrNapping {
    On(AgentsOn),
    Napping(Napping),
}

/// A follower of a sign-in; two are the same follower only where `Rc::ptr_eq`.
pub type Follower = Rc<dyn Fn(AgentsSignInEvent)>;

/// agents-read.ts:314-321: one sign-in running.
pub struct Running {
    pub target: AgentsTarget,
    pub ask: SignInAsk,
    pub r#type: RefCell<Option<CodeWriter>>,
    pub followers: RefCell<Vec<Follower>>,
    pub last: RefCell<Option<AgentsSignInEvent>>,
    pub stopped_by: Cell<bool>,
    pub stop: Box<dyn Fn()>,
}

/// `follow`'s answer: the id and the follower's way out.
pub struct Following {
    pub sign_in_id: String,
    pub leave: Box<dyn FnOnce()>,
}

/// `agentsReads(o)` (agents-read.ts:258): every map is memory.
pub struct AgentsReads {
    pub o: AgentsReadOptions,
    /// `last`: each workspace's last report.
    pub last: RefCell<IndexMap<String, AgentsReport>>,
    /// `relays`.
    pub relays: RefCell<Vec<Rc<dyn CallbackForwards>>>,
    /// `running`, by sign-in id.
    pub running: RefCell<IndexMap<String, Rc<Running>>>,
    /// `ended`: each ended sign-in as listed, and when it ended.
    pub ended: RefCell<IndexMap<String, (AgentsSignInRun, i64)>>,
    /// `starting`, by join key.
    pub starting: RefCell<IndexMap<String, Shared<LocalBoxFuture<'static, Result<(String, Rc<Running>), HostError>>>>>,
}

impl AgentsReads {
    pub fn new(o: AgentsReadOptions) -> Rc<AgentsReads> {
        Rc::new(AgentsReads {
            o,
            last: RefCell::default(),
            relays: RefCell::default(),
            running: RefCell::default(),
            ended: RefCell::default(),
            starting: RefCell::default(),
        })
    }
}
