// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/threads/launch.ts at 65f6b89c7. Spec: spec/launch.md.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_adapters::port::{AgentLaunch, Env, HarnessAdapter, HarnessExec};
use wsp_core::ids::Ids;
use wsp_core::types::api::WarmAsk;
use wsp_core::types::internal::KeptLaunch;
use wsp_wire::types::harness_picks::RanPicks;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::harness::{HarnessCatalog, McpServerSpec, StartPicks};
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::SessionWarmResult;
use wsp_words::exit::HostError;

use crate::threads::agents::launch::{AdapterAsk, BuiltAdapter, DefaultsOn, LaunchServers};
use crate::threads::threads::{Entry, KeptHand, ThreadRecords};

/// launch.ts:212. Rules L1, L2.
pub fn served_to(wsp: Option<&McpServerSpec>, root_thread_id: &str, thread_id: &str) -> Option<McpServerSpec> {
    todo!()
}

/// launch.ts:221. `rows` is the place door's rows (None where the host has no door); `prompt` is the send's, written in
/// place. Answers the clouds held. Rules L3-L6.
pub fn clouds_told(rows: Option<Vec<PlaceView>>, records: &ThreadRecords, thread_id: &str, prompt: &mut String) -> Vec<String> {
    todo!()
}

/// The picks a launch names (launch.ts:230).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchAsked {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub access: Option<AccessChoice>,
    pub context_window: Option<String>,
    pub fast: Option<bool>,
}

/// What `picks_for` answers: the start picks and the window kept (launch.ts:249).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchPicks {
    pub picks: StartPicks,
    pub context_window: Option<String>,
}

/// What a thread's launch is minted with (TS `threadLaunch(...)`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThreadLaunch {
    pub scoped_device_id: Option<String>,
    pub env: IndexMap<String, String>,
    pub wsp: Option<McpServerSpec>,
}

/// What the launch area reads of the runtime. The TS module-level helpers that read the runtime's own tables
/// (`harnessCatalog`, `notRunRefused`) are here too, since their files are not ported in a crate below this one.
#[async_trait(?Send)]
pub trait LaunchCtx {
    fn ids(&self) -> Rc<dyn Ids>;
    async fn ready(&self) -> Result<(), HostError>;
    async fn entry_of(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Entry, HostError>;
    /// `ctx.moduleOf(entry.record.kind).keepsAgents`.
    fn keeps_agents(&self, entry: &Entry) -> bool;
    /// `ctx.keptBox(workspaceId)?.cap`.
    fn kept_box_cap(&self, workspace_id: &str) -> Option<u32>;
    fn closing(&self) -> bool;
    fn preferences_held(&self) -> Option<Preferences>;
    async fn preferences(&self) -> Result<Preferences, HostError>;
    fn default_agent_of(&self, prefs: &Preferences, entry: &Entry) -> String;
    fn agent_off(&self, entry: &Entry, harness: &str) -> bool;
    /// `ctx.adapterFor(entry, harness, env, waiting, servers, threadId, asksUntilStopped)`: `ask.named` is the harness.
    fn adapter_for(&self, entry: &Entry, ask: AdapterAsk) -> Result<BuiltAdapter, HostError>;
    /// Whether the adapter has a `warm` road (TS `adapter.warm !== undefined`; the Rust port answers it by calling).
    fn adapter_warms(&self, adapter: &dyn HarnessAdapter) -> bool;
    async fn confine_setup(&self, entry: &Entry, harness: &str) -> Result<(), HostError>;
    /// `harnessCatalog(harness) === undefined ? undefined : ctx.catalogOn(table, entry, adapter)`, with the table read.
    async fn catalog_on(&self, harness: &str, entry: &Entry, adapter: &dyn HarnessAdapter) -> Result<Option<HarnessCatalog>, HostError>;
    /// `ctx.defaultsOn(catalog, prefs, prefs.projectDefaults[project])`.
    fn defaults_on(&self, catalog: HarnessCatalog, prefs: &Preferences, project: &str) -> DefaultsOn;
    fn named_mode(&self, catalog: Option<&HarnessCatalog>, harness: &str, access: &AccessChoice) -> Option<String>;
    fn access_of(&self, workspace_id: &str, thread_id: &str, session: Option<&str>) -> Option<String>;
    fn ran_on(&self, workspace_id: &str, thread_id: &str, session: Option<&str>) -> RanPicks;
    fn resumed_fact(&self, workspace_id: &str, session: &str, fact: &str) -> Option<String>;
    /// `notRunRefused(catalog, model)`.
    fn not_run_refused(&self, catalog: Option<&HarnessCatalog>, model: Option<&str>) -> Option<HostError>;
    async fn thread_folder(&self, entry: &Entry, cwd: Option<&str>) -> Result<String, HostError>;
    /// `existsSync(path)` on this computer.
    fn folder_exists(&self, path: &str) -> bool;
    fn runs_in(&self, entry: &Entry, ran_in: Option<&str>, folder: &str) -> String;
    fn setup_place(&self, entry: &Entry) -> Option<String>;
    /// `setups.launchOf(place, harness)`.
    fn launch_of(&self, place: &str, harness: &str) -> Option<AgentLaunch>;
    /// `ctx.warmOn(workspaceId, harness)`: the standing warm agent's thread and launch.
    fn warm_on(&self, workspace_id: &str, harness: &str) -> Option<(String, KeptLaunch)>;
    fn rewarm(&self, thread_id: &str);
    /// `ctx.reapKept(threadId)`.
    fn reap_kept(&self, thread_id: &str);
    async fn thread_launch(&self, entry: &Entry, thread_id: &str, root_thread_id: &str, aside: bool) -> Result<ThreadLaunch, HostError>;
    /// `opts.vault?.() ?? {}`; the area reads `server_values_of` off it.
    fn vault(&self) -> Env;
    async fn launch_servers_for(&self, entry: &Entry, harness: &str, adapter: &dyn HarnessAdapter, cwd: &str) -> Result<LaunchServers, HostError>;
    fn hold_warm(&self, thread_id: &str, hold: KeptHand);
    async fn revoke_device(&self, device_id: &str) -> Result<(), HostError>;
}

/// What `launch_picks` answers: the catalog read and the picks for a session (launch.ts:264).
pub struct PicksRead {
    pub catalog: Option<HarnessCatalog>,
    pub picks_for: Box<dyn Fn(Option<&str>) -> Result<LaunchPicks, HostError>>,
}

#[derive(Default)]
struct LaunchState {
    warming: IndexMap<String, futures::future::Shared<futures::future::LocalBoxFuture<'static, Result<SessionWarmResult, HostError>>>>,
}

/// launch.ts:232. Clones share one state.
#[derive(Clone)]
pub struct LaunchArea {
    ctx: Rc<dyn LaunchCtx>,
    state: Rc<RefCell<LaunchState>>,
}

impl LaunchArea {
    pub fn new(ctx: Rc<dyn LaunchCtx>) -> Self {
        todo!()
    }

    /// launch.ts:237. Rules L7-L11.
    pub async fn launch_picks(&self, entry: &Entry, harness: &str, adapter: &dyn HarnessAdapter, prefs: &Preferences, thread_id: &str, asked: &LaunchAsked) -> Result<PicksRead, HostError> {
        todo!()
    }

    /// launch.ts:268. `mcp_servers` is the JSON of the servers a caller named; `version` is `None` for undefined and
    /// `Some(None)` for null. Rule L12.
    pub fn launch_fixed(&self, entry: &Entry, harness: &str, cwd: &str, mcp_servers: Option<&serde_json::Value>, version: Option<Option<&str>>) -> String {
        todo!()
    }

    /// launch.ts:314. Rules L13-L20.
    pub async fn warm(&self, workspace_id: &str, asked: WarmAsk, origin: Option<Caller>) -> Result<SessionWarmResult, HostError> {
        todo!()
    }
}

/// The adapter's `sessionHeld` road (TS `HarnessAdapter["sessionHeld"]`, not in wsp-adapters' port yet): what the
/// machine holds for a session, in words.
pub type SessionHeld = Rc<dyn Fn(String, Arc<dyn HarnessExec>) -> futures::future::LocalBoxFuture<'static, Result<String, String>>>;

/// The seed a resuming start hands its agent.
pub type Seed = Rc<dyn Fn() -> futures::future::LocalBoxFuture<'static, Result<String, HostError>>>;

/// Where a lost session is named in the log line.
#[derive(Clone, Debug, PartialEq)]
pub struct LostAt {
    pub session_id: String,
    pub thread_id: String,
    pub workspace_id: String,
}

/// launch.ts:340. `exec` is the machine's harness exec bounded by `SESSION_TITLE_TIMEOUT_MS`. Rules L21-L23.
pub fn seed_after_loss(session_held: Option<SessionHeld>, exec: Arc<dyn HarnessExec>, at: LostAt, seed: Seed) -> Seed {
    todo!()
}
