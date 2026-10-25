// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/threads/agents.ts` at 65f6b89c7: the agents area. Specs: `spec/agents_lists.md`,
//! `spec/agents_launch.md`.
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use wsp_adapters::port::{AgentLaunch, Env, ExecStreamFactory, HarnessCatalogModelProbe, SessionRenameWrite};
use wsp_catalog::mcp_launch::TurnServer;
use wsp_core::clock::Clock;
use wsp_core::context::{Joined, StateCell};
use wsp_core::machine_exec::TurnWaiting;
use wsp_core::types::events::EventBus;
use wsp_adapters::codex::command::CodexBroker;
use wsp_core::types::harness::{Brokered, HarnessAdapterFactory};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_store::store::Store;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use super::threads::{Entry, Sessions, ThreadRecords};

pub mod launch;
pub mod lists;

/// How a kind hands an agent's MCP server values (`KindModule.serverValues`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerValuesRoad {
    Launch,
    Environment,
    None,
}

/// The person's setup for one agent on one computer: its variables and its launch (`setups.launchOf`).
#[derive(Clone, Debug, Default)]
pub struct SetupLaunch {
    pub env: Env,
    pub launch: Option<AgentLaunch>,
}

/// What a workspace whose credentials the host keeps launches with (`ctx.brokeredLaunch`).
#[derive(Clone, Default)]
pub struct BrokeredLaunch {
    pub vault: Env,
    pub env: Env,
    pub claude: Option<Brokered>,
    pub codex: Option<CodexBroker>,
}

/// What only a turn's launch adds to its exec factory's limits (agents.ts:643-651): the thread it groups under where
/// the kind ends threads, a line's try, and a sub-thread's kill reading. The join lays these over the place's turn
/// limits (`ctx.turnLimitOf(record)`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnLaunch {
    pub thread: Option<String>,
    pub asks_until_stopped: bool,
    pub killed_why: bool,
}

/// A project as the projects door lists it: the fields read here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectSeen {
    pub id: String,
    pub computer: String,
}

/// `opts.agentsReader.signsIn`'s answer: whether each address asks for a sign-in (`None` where never asked).
pub type SignsIn = Rc<dyn Fn(&str) -> Option<bool>>;

/// Every `ctx.<x>` the agents area calls (README R2).
pub trait AgentsCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn store(&self) -> Rc<dyn Store>;
    fn bus(&self) -> Rc<dyn EventBus>;
    fn sessions(&self) -> StateCell<Sessions>;
    fn thread_records(&self) -> StateCell<ThreadRecords>;
    fn entry(&self, workspace_id: &str) -> Option<Entry>;
    fn closing(&self) -> bool;
    /// `ctx.adapters`, in their order.
    fn adapters(&self) -> IndexMap<String, HarnessAdapterFactory>;
    /// `local?.homeDir`.
    fn local_home_dir(&self) -> Option<String>;
    /// `local?.home(id)`.
    fn local_home(&self, agent: &str) -> Option<String>;
    /// `homedir()`.
    fn process_home(&self) -> String;
    /// `opts.statePath`.
    fn state_path(&self) -> Option<String>;
    fn setup_off(&self, place: &str, agent: &str) -> bool;
    /// `setups.get(place, agent)?.configDir`.
    fn setup_config_dir(&self, place: &str, agent: &str) -> Option<String>;
    fn setup_launch(&self, place: &str, agent: &str) -> SetupLaunch;
    fn setup_refusal_moved(&self, place: &str, harness: &str);
    /// `ctx.placeDoorOf().exec(place, script, {timeoutMs})`.
    fn place_exec(&self, place: &str, script: &str, timeout_ms: u64) -> LocalBoxFuture<'static, Result<ExecResult, HostError>>;
    fn place_name(&self, place: &str) -> String;
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String>;
    /// `atProvider(placeId, ctx.places)`.
    fn at_provider(&self, place_id: Option<&str>) -> bool;
    fn place_agent_home(&self, place: &str, home: &str, agent: &str) -> String;
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), HostError>>;
    fn know_broker(&self, entry: &Entry, harness: &str) -> LocalBoxFuture<'static, Result<(), HostError>>;
    fn unreached_of(&self, entry: &Entry) -> Option<String>;
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()>;
    fn push_head(&self, thread_id: &str);
    /// The threads area's `writeSession`.
    fn write_session(
        &self,
        session: &str,
        write: LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>>,
    ) -> LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>>;
    // The kind's module (`ctx.moduleOf(record.kind)`).
    fn kind_home(&self, entry: &Entry, agent: &str) -> String;
    /// `kind.env(entry, harness, home)`, `home` being the area's `agent_home`.
    fn kind_env(&self, entry: &Entry, harness: Option<&str>, home: &dyn Fn(&str) -> String) -> Env;
    fn kind_server_values(&self, entry: &Entry) -> ServerValuesRoad;
    fn kind_ends_threads(&self, entry: &Entry) -> bool;
    fn kind_memory_key(&self, entry: &Entry, harness: &str) -> Option<String>;
    fn kind_login_stands(&self, entry: &Entry, agent: &str) -> bool;
    /// `runsInFolder(ctx.kindOf(computer))`.
    fn computer_runs_in_folder(&self, computer: &str) -> bool;
    fn brokered_launch(&self, entry: &Entry, harness: &str, token: Option<&str>) -> Option<BrokeredLaunch>;
    fn vault(&self) -> Env;
    fn vault_on(&self, entry: &Entry) -> Env;
    fn exec_factory_for(&self, entry: &Entry, turn: TurnLaunch, waiting: Option<TurnWaiting>) -> Arc<dyn ExecStreamFactory>;
    /// `opts.agentsReader?.signsIn?.(addresses)`.
    fn signs_in(&self, addresses: Vec<String>) -> LocalBoxFuture<'static, Option<SignsIn>>;
    fn resolve_project(&self, reference: &str, origin: Option<Caller>) -> LocalBoxFuture<'static, Result<ProjectSeen, HostError>>;
    fn list_projects(&self, origin: Option<Caller>) -> LocalBoxFuture<'static, Result<Vec<ProjectSeen>, HostError>>;
    fn project_folder(&self, project: &ProjectSeen) -> LocalBoxFuture<'static, Result<Entry, HostError>>;
}

/// One catalog entry (agents.ts:33).
#[derive(Clone)]
pub struct CatalogHeld {
    pub at: i64,
    pub catalog: Shared<LocalBoxFuture<'static, HarnessCatalog>>,
}

/// One account read (agents.ts:37).
#[derive(Clone)]
pub struct AccountRead {
    pub at: i64,
    pub models: Shared<LocalBoxFuture<'static, Option<Vec<HarnessCatalogModelProbe>>>>,
}

/// One title read (agents.ts:161).
#[derive(Clone)]
pub struct TitleRead {
    pub at: i64,
    pub done: Joined,
    pub live: bool,
    pub failed: bool,
}

/// The agents area (`agentsArea`, agents.ts:26).
pub struct AgentsArea {
    pub(crate) ctx: Rc<dyn AgentsCtx>,
    pub(crate) me: Weak<AgentsArea>,
    /// agents.ts:33
    pub catalogs: RefCell<IndexMap<String, CatalogHeld>>,
    /// agents.ts:37
    pub account_reads: RefCell<IndexMap<String, AccountRead>>,
    /// agents.ts:40
    pub heard_versions: RefCell<IndexMap<String, String>>,
    /// agents.ts:43, by an id per ask.
    pub agent_asks: RefCell<IndexMap<u64, Joined>>,
    /// agents.ts:161
    pub title_reads: RefCell<IndexMap<String, Rc<RefCell<TitleRead>>>>,
    /// agents.ts:277
    pub titles_asked: RefCell<IndexSet<String>>,
    /// agents.ts:353
    pub setup_refusals: RefCell<IndexMap<String, String>>,
    /// agents.ts:573
    pub cloud_servers: RefCell<IndexMap<String, Shared<LocalBoxFuture<'static, Option<Vec<TurnServer>>>>>>,
}

impl AgentsArea {
    /// Builds the area and registers its listeners: `agents.changed`, `workspace.deleted`, `workspace.upgraded`
    /// each drop the servers read for a workspace (agents.ts:577-585).
    pub fn new(ctx: Rc<dyn AgentsCtx>) -> Rc<AgentsArea> {
        todo!()
    }
    /// The asks in flight a close waits for (agents.ts:43).
    pub fn agent_asks(&self) -> Vec<Joined> {
        todo!()
    }
}
