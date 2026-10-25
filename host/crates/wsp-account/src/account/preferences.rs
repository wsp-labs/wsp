// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/preferences.ts at 65f6b89c7: the person's defaults and the wiring of the
//! agents read. Spec: spec/preferences.md.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::Value;

use wsp_core::daemon_channel::DaemonChannel;
use wsp_wire::views::preferences::{Preferences, PreferencesPatch};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use crate::account::core_types::{AccountResult, Entry};

/// What `set` answers (preferences.ts:94).
#[derive(Clone, Debug, PartialEq)]
pub struct SetAnswer {
    pub preferences: Preferences,
    pub notice: Option<String>,
}

/// A harness's access table as `accessRefusal` reads it (`harnessCatalog(id) ?? { label, permissionModes: [] }`).
#[derive(Clone, Debug, PartialEq)]
pub struct AccessTable {
    pub label: String,
    /// The table whole, as the TS object `accessRefusal` takes.
    pub table: Value,
}

/// The server icons the options wire (`opts.serverIcons`): the folder they keep and the forget, failing in node's
/// words (`EACCES: permission denied, rmdir '/x'`).
pub trait ServerIcons {
    fn folder(&self) -> String;
    fn forget(&self) -> Result<(), String>;
}

/// What `agentsRead.workspace(id, origin)` answers (preferences.ts:102-121), keys in this order.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentsWorkspace {
    pub name: String,
    pub phase: String,
    pub local: bool,
    pub project_id: String,
    pub project_name: String,
    pub project_path: String,
    pub stores: IndexMap<String, String>,
    pub seeds: IndexMap<String, String>,
    pub brokered: bool,
    pub pause_mode: Option<String>,
}

/// A catalog agent as the agents read wires it: its id, the env key of its state home, the env key of its plugin seed.
#[derive(Clone, Debug, PartialEq)]
pub struct CatalogAgentKeys {
    pub id: String,
    pub state_home_env: Option<String>,
    pub plugin_seed_env: Option<String>,
}

/// Where a channel of the agents read goes (preferences.ts:135).
#[derive(Clone, Debug, PartialEq)]
pub enum ChannelTarget {
    Workspace(String),
    Place(String),
}

/// What the area reads off the runtime's context.
#[async_trait(?Send)]
pub trait PreferencesCtx {
    /// `opts.env ?? process.env`, read once.
    fn env(&self) -> IndexMap<String, String>;
    fn server_icons(&self) -> Option<Rc<dyn ServerIcons>>;
    /// `homedir()`.
    fn home_dir(&self) -> String;
    /// `Object.keys(adapters)`, in order.
    fn adapter_ids(&self) -> Vec<String>;
    /// `harnessCatalog(id) ?? { label: ctx.agentLabel(id), permissionModes: [] }`.
    fn access_table(&self, agent: &str) -> AccessTable;
    fn project_held(&self, id: &str) -> bool;
    /// `ctx.projectHeld(id)` as the agents read takes it: the project's id and name; its refusal where none is held.
    fn project_of(&self, id: &str) -> Result<(String, String), HostError>;
    /// `ctx.defaultAgentOf(prefs, undefined)`.
    fn default_agent_of(&self, prefs: &Preferences) -> String;
    /// `threadKeyOf(row.view)` of every session row.
    fn session_thread_keys(&self) -> Vec<String>;
    fn thread_record_held(&self, thread_id: &str) -> bool;
    async fn ready(&self);
    /// `store.get(PREFERENCES, PREFERENCES_ID)`.
    async fn stored(&self) -> AccountResult<Option<Value>>;
    /// `store.put(PREFERENCES, PREFERENCES_ID, next)`.
    async fn store(&self, prefs: &Preferences) -> AccountResult<()>;
    /// `ctx.state.preferencesHeld`.
    fn held(&self) -> Option<Preferences>;
    fn set_held(&self, prefs: Preferences);
    /// `bus.emit(event)`.
    fn emit(&self, event: Value);
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>);

    // The agents read's members (preferences.ts:99-151).
    async fn entry_of(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<Entry, HostError>;
    fn catalog_agents(&self) -> Vec<CatalogAgentKeys>;
    fn thread_env(&self, entry: &Entry, agent: &str) -> IndexMap<String, String>;
    fn checkout_of(&self, entry: &Entry) -> Result<String, HostError>;
    fn is_local(&self, entry: &Entry) -> bool;
    /// `atProvider(ctx.placeIdOf(record), ctx.places)`.
    fn at_provider(&self, entry: &Entry) -> bool;
    fn pause_mode(&self, entry: &Entry) -> Option<String>;
    /// The projects on a place: id, name, `checkout ?? path`.
    fn projects_on(&self, place_id: &str) -> Vec<(String, String, String)>;
    async fn workspace_channel(&self, workspace_id: &str, on_event: Rc<dyn Fn(Value)>, origin: Option<&Caller>) -> AccountResult<Rc<dyn DaemonChannel>>;
    async fn local_channel(&self, on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>>;
    fn place_channel(&self, place_id: &str, on_event: Rc<dyn Fn(Value)>) -> Option<Rc<dyn DaemonChannel>>;
    fn place_name(&self, place_id: &str) -> Option<String>;
    /// `setups.set(placeId, agent, change, ...)`.
    async fn setup_write(&self, place_id: &str, agent: &str, change: Value) -> AccountResult<()>;
    /// `setupView(setups.get(placeId, agent))`: the setup as the app reads it, names only.
    fn setup_view(&self, place_id: &str, agent: &str) -> Value;
    /// `ctx.setupRefusals.delete(keyOf(placeId, agent))`.
    fn drop_setup_refusal(&self, place_id: &str, agent: &str) -> bool;
    fn setup_refusal_moved(&self, place_id: &str, agent: &str);
    /// `backend.capabilities.callbackRelay`.
    fn relayed(&self) -> bool;
    /// `agentsRead.forget(id)`.
    fn forget_read(&self, workspace_id: &str);
}

/// preferences.ts:13 `preferencesArea(ctx)`.
pub struct PreferencesArea {
    pub(crate) ctx: Rc<dyn PreferencesCtx>,
    /// preferences.ts:17: the last write in the chain.
    pub(crate) writes: RefCell<Option<Shared<LocalBoxFuture<'static, ()>>>>,
    /// preferences.ts:19.
    pub(crate) labs: Value,
}

impl PreferencesArea {
    /// F1, preferences.ts:13: reads `labs` off the env once, registers the `workspace.deleted` forget.
    pub fn new(ctx: Rc<dyn PreferencesCtx>) -> Rc<PreferencesArea> {
        todo!()
    }

    /// F2, preferences.ts:85.
    pub async fn get(&self) -> AccountResult<Preferences> {
        todo!()
    }

    /// F3, F4, F5, F6, preferences.ts:86.
    pub async fn set(self: &Rc<Self>, patch: PreferencesPatch) -> AccountResult<SetAnswer> {
        todo!()
    }

    /// F3, preferences.ts:72: the record moved by `change` in its turn among every other write.
    pub async fn change_preferences(self: &Rc<Self>, change: Box<dyn FnOnce(Preferences) -> AccountResult<Preferences>>) -> AccountResult<Preferences> {
        todo!()
    }

    /// F5, preferences.ts:35.
    pub fn defaults_refusal(&self, patch: &PreferencesPatch, next: &Preferences) -> Result<(), HostError> {
        todo!()
    }

    /// F4, preferences.ts:67: the shut entries of leads still held.
    pub fn held_leads(&self, shut: &IndexMap<String, bool>) -> IndexMap<String, bool> {
        todo!()
    }

    /// F6, preferences.ts:21.
    pub fn icons_forgotten(&self) -> Option<String> {
        todo!()
    }

    /// F7, preferences.ts:102.
    pub async fn agents_workspace(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<AgentsWorkspace, HostError> {
        todo!()
    }

    /// F7, preferences.ts:125.
    pub async fn agents_projects(&self, place_id: &str) -> Vec<(String, String, String)> {
        todo!()
    }

    /// F7, preferences.ts:135.
    pub async fn agents_channel(&self, target: ChannelTarget, on_event: Rc<dyn Fn(Value)>, origin: Option<&Caller>) -> AccountResult<Rc<dyn DaemonChannel>> {
        todo!()
    }

    /// F7, preferences.ts:142.
    pub fn agents_changed(&self, target: Option<Value>) {
        todo!()
    }

    /// F7, preferences.ts:143: nothing for an agent with no adapter, else the context's `setup_view`.
    pub fn agents_setup_of(&self, place_id: &str, agent: &str) -> Option<Value> {
        todo!()
    }

    /// F7, preferences.ts:129: whether the person lets this host ask the vendors for each agent's newest version.
    pub async fn agents_latest_on(&self) -> AccountResult<bool> {
        todo!()
    }

    /// F7, preferences.ts:146: the backend's `callbackRelay`.
    pub fn agents_relayed(&self) -> bool {
        todo!()
    }

    /// F7, preferences.ts:144.
    pub async fn agents_setup_write(&self, place_id: &str, agent: &str, change: Value) -> AccountResult<()> {
        todo!()
    }

    /// F7, preferences.ts:130: whether the icon is asked now (read at every ask).
    pub async fn icons_on(&self) -> AccountResult<bool> {
        todo!()
    }
}
