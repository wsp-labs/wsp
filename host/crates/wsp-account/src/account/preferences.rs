// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/preferences.ts at 65f6b89c7: the person's defaults and the wiring of the
//! agents read. Spec: spec/preferences.md.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use async_trait::async_trait;
use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use wsp_core::daemon_channel::DaemonChannel;
use wsp_frames::True;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::{
    apply_preferences_patch, labs_from_env, preferences_from, server_icons_left_line, Preferences, PreferencesPatch,
};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::{HostError, Kind};
use wsp_words::projects::{bare_no_such_project_line, home_shortened};
use wsp_words::thread_defaults::{access_refusal, model_id_refusal};
use wsp_words::words::thread::no_adapter_line;
use wsp_words::workspace_state::absent_computer;

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
    async fn workspace_channel(
        &self,
        workspace_id: &str,
        on_event: Rc<dyn Fn(Value)>,
        origin: Option<&Caller>,
    ) -> AccountResult<Rc<dyn DaemonChannel>>;
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

/// The one throw the defaults refuse with (preferences.ts:36).
fn usage(line: &str) -> HostError {
    HostError::with_kind(line, Kind::Usage)
}

/// The harness's catalog as `accessRefusal` reads it: the table's own keys over the fields a catalog must carry.
fn catalog_of(agent: &str, table: &Value) -> Result<HarnessCatalog, HostError> {
    let mut full = json!({
        "source": "table", "version": null, "contextWindows": [], "steers": false, "renames": false, "images": false,
        "harness": agent, "label": "", "models": [], "efforts": [], "permissionModes": []
    });
    if let (Value::Object(base), Value::Object(own)) = (&mut full, table) {
        for (key, value) in own {
            base.insert(key.clone(), value.clone());
        }
    }
    serde_json::from_value(full).map_err(|e| HostError::new(e.to_string()))
}

/// `NodeError.message` as the TS reads it back: the code before and the call and path after taken off
/// (preferences.ts:29).
fn without_node_words(message: &str) -> String {
    let message = match message.find(": ") {
        Some(at) if at > 0 && message[..at].bytes().all(|b| b.is_ascii_uppercase()) => &message[at + 2..],
        _ => message,
    };
    for (at, _) in message.match_indices(", ") {
        if let Some((word, quoted)) = message[at + 2..].split_once(' ') {
            let inner = quoted.strip_prefix('\'').and_then(|q| q.strip_suffix('\''));
            if !word.is_empty() && word.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') && inner.is_some_and(|q| !q.contains('\''))
            {
                return message[..at].to_string();
            }
        }
    }
    message.to_string()
}

/// The record as the person's writes read it: the held one, else the stored one over the defaults with `labs` set
/// (preferences.ts:85).
async fn read_preferences(ctx: &dyn PreferencesCtx, labs: bool) -> AccountResult<Preferences> {
    if let Some(held) = ctx.held() {
        return Ok(held);
    }
    let stored = ctx.stored().await?;
    let mut prefs = preferences_from(stored.as_ref());
    prefs.labs = labs;
    ctx.set_held(prefs.clone());
    Ok(prefs)
}

/// The chain every write runs on (preferences.ts:17): each write waits for the one before, and a failed one does not
/// stop the next.
fn enqueue_write<T: 'static>(
    writes: &RefCell<Option<Shared<LocalBoxFuture<'static, ()>>>>,
    body: LocalBoxFuture<'static, AccountResult<T>>,
) -> LocalBoxFuture<'static, AccountResult<T>> {
    let prev = writes.borrow().clone();
    let (answer, answered) = oneshot::channel::<AccountResult<T>>();
    let tail: Shared<LocalBoxFuture<'static, ()>> = async move {
        if let Some(prev) = prev {
            prev.await;
        }
        let _ = answer.send(body.await);
    }
    .boxed_local()
    .shared();
    *writes.borrow_mut() = Some(tail.clone());
    async move {
        tail.await;
        answered.await.unwrap_or_else(|_| Err(HostError::new("the preferences write ended before it answered").into()))
    }
    .boxed_local()
}

impl PreferencesArea {
    /// F1, preferences.ts:13: reads `labs` off the env once, registers the `workspace.deleted` forget.
    pub fn new(ctx: Rc<dyn PreferencesCtx>) -> Rc<PreferencesArea> {
        let env: HashMap<String, String> = ctx.env().into_iter().collect();
        let labs = labs_from_env(&env);
        let forget_ctx = ctx.clone();
        ctx.on_workspace_deleted(Rc::new(move |workspace_id: &str| forget_ctx.forget_read(workspace_id)));
        Rc::new(PreferencesArea { ctx, writes: RefCell::new(None), labs: Value::Bool(labs) })
    }

    fn labs_flag(&self) -> bool {
        self.labs.as_bool().unwrap_or(false)
    }

    /// F2, preferences.ts:85.
    pub async fn get(&self) -> AccountResult<Preferences> {
        read_preferences(&*self.ctx, self.labs_flag()).await
    }

    /// F3, F4, F5, F6, preferences.ts:86.
    pub async fn set(self: &Rc<Self>, patch: PreferencesPatch) -> AccountResult<SetAnswer> {
        let area = self.clone();
        let check = patch.clone();
        let next = self
            .change_preferences(Box::new(move |held: Preferences| {
                let merged = apply_preferences_patch(&held, &check);
                let next = if check.threads_shut.is_none() || merged.threads_shut.is_none() {
                    merged
                } else {
                    let shut: IndexMap<String, bool> =
                        merged.threads_shut.iter().flat_map(|shut| shut.keys()).map(|lead| (lead.clone(), true)).collect();
                    let kept: IndexMap<String, True> = area.held_leads(&shut).into_keys().map(|lead| (lead, True)).collect();
                    Preferences { threads_shut: Some(kept), ..merged }
                };
                area.defaults_refusal(&check, &next)?;
                Ok(next)
            }))
            .await?;
        let notice = if patch.server_icons == Some(false) { self.icons_forgotten() } else { None };
        Ok(SetAnswer { preferences: next, notice })
    }

    /// F3, preferences.ts:72: the record moved by `change` in its turn among every other write.
    pub async fn change_preferences(
        self: &Rc<Self>,
        change: Box<dyn FnOnce(Preferences) -> AccountResult<Preferences>>,
    ) -> AccountResult<Preferences> {
        let ctx = self.ctx.clone();
        let labs = self.labs_flag();
        let body: LocalBoxFuture<'static, AccountResult<Preferences>> = async move {
            ctx.ready().await;
            let held = read_preferences(&*ctx, labs).await?;
            let next = change(held)?;
            ctx.store(&next).await?;
            ctx.set_held(next.clone());
            let mut event = Map::new();
            event.insert("type".to_string(), json!("preferences.changed"));
            event.insert("preferences".to_string(), serde_json::to_value(&next).unwrap_or(Value::Null));
            ctx.emit(Value::Object(event));
            Ok(next)
        }
        .boxed_local();
        enqueue_write(&self.writes, body).await
    }

    /// An access word the agent maps to none of its modes is refused, in the sentence the agent's table gives
    /// (preferences.ts:40).
    fn access_check(&self, agent: &str, access: Option<AccessChoice>) -> Result<(), HostError> {
        let Some(word) = access else {
            return Ok(());
        };
        let table = self.ctx.access_table(agent);
        let catalog = catalog_of(agent, &table.table)?;
        match access_refusal(&catalog, word) {
            Some(said) => Err(usage(&said)),
            None => Ok(()),
        }
    }

    /// F5, preferences.ts:35.
    pub fn defaults_refusal(&self, patch: &PreferencesPatch, next: &Preferences) -> Result<(), HostError> {
        let adapters = self.ctx.adapter_ids();
        let agent = |id: &str| -> Result<(), HostError> {
            if adapters.iter().any(|known| known == id) {
                Ok(())
            } else {
                Err(usage(&no_adapter_line(id, &adapters)))
            }
        };
        let model = |id: &str| -> Result<(), HostError> {
            match model_id_refusal(id) {
                Some(said) => Err(usage(&said)),
                None => Ok(()),
            }
        };
        if let Some(Some(id)) = &patch.default_agent {
            agent(id)?;
        }
        if let Some(map) = &patch.agent_defaults {
            for (id, set) in map {
                let Some(set) = set else {
                    continue;
                };
                agent(id)?;
                self.access_check(id, set.access.flatten())?;
                let custom = set.models.clone().flatten().and_then(|picker| picker.custom).unwrap_or_default();
                for named in set.model.clone().flatten().into_iter().chain(custom) {
                    model(&named)?;
                }
            }
        }
        if let Some(map) = &patch.project_defaults {
            for (project_id, set) in map {
                let Some(set) = set else {
                    continue;
                };
                if !self.ctx.project_held(project_id) {
                    return Err(usage(&bare_no_such_project_line(project_id)));
                }
                if let Some(agent_id) = set.agent.clone().flatten() {
                    agent(&agent_id)?;
                }
                if let Some(model_id) = set.model.clone().flatten() {
                    model(&model_id)?;
                }
                let kept = next.project_defaults.get(project_id).and_then(|fields| fields.clone().into_inner().agent);
                let landing = kept.unwrap_or_else(|| self.ctx.default_agent_of(next));
                self.access_check(&landing, set.access.flatten())?;
            }
        }
        Ok(())
    }

    /// F4, preferences.ts:67: the shut entries of leads still held.
    pub fn held_leads(&self, shut: &IndexMap<String, bool>) -> IndexMap<String, bool> {
        let rows = self.ctx.session_thread_keys();
        shut.iter()
            .filter(|(lead, _)| rows.iter().any(|row| row == *lead) || self.ctx.thread_record_held(lead))
            .map(|(lead, on)| (lead.clone(), *on))
            .collect()
    }

    /// F6, preferences.ts:21.
    pub fn icons_forgotten(&self) -> Option<String> {
        let icons = self.ctx.server_icons()?;
        match icons.forget() {
            Ok(()) => None,
            Err(message) => {
                let home = self.ctx.home_dir();
                Some(server_icons_left_line(&home_shortened(&icons.folder(), Some(&home)), &without_node_words(&message)))
            }
        }
    }

    /// F7, preferences.ts:102.
    pub async fn agents_workspace(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<AgentsWorkspace, HostError> {
        let entry = self.ctx.entry_of(workspace_id, origin).await?;
        let record = entry.borrow().record.borrow().clone();
        let (project_id, project_name) = self.ctx.project_of(&record.project)?;
        let local = self.ctx.is_local(&entry);
        let mut stores = IndexMap::new();
        let mut seeds = IndexMap::new();
        for agent in self.ctx.catalog_agents() {
            if !local {
                if let Some(key) = &agent.state_home_env {
                    if let Some(folder) = self.ctx.thread_env(&entry, &agent.id).get(key) {
                        stores.insert(agent.id.clone(), folder.clone());
                    }
                }
                if let Some(key) = &agent.plugin_seed_env {
                    if let Some(seed) = self.ctx.thread_env(&entry, &agent.id).get(key) {
                        seeds.insert(agent.id.clone(), seed.clone());
                    }
                }
            }
        }
        let brokered = !local && self.ctx.at_provider(&entry);
        let pause_mode = self.ctx.pause_mode(&entry);
        let project_path = self.ctx.checkout_of(&entry)?;
        let phase = serde_json::to_value(record.phase).ok().and_then(|word| word.as_str().map(str::to_string)).unwrap_or_default();
        Ok(AgentsWorkspace { name: record.name, phase, local, project_id, project_name, project_path, stores, seeds, brokered, pause_mode })
    }

    /// F7, preferences.ts:125.
    pub async fn agents_projects(&self, place_id: &str) -> Vec<(String, String, String)> {
        self.ctx.ready().await;
        self.ctx.projects_on(place_id)
    }

    /// F7, preferences.ts:135.
    pub async fn agents_channel(
        &self,
        target: ChannelTarget,
        on_event: Rc<dyn Fn(Value)>,
        origin: Option<&Caller>,
    ) -> AccountResult<Rc<dyn DaemonChannel>> {
        match target {
            ChannelTarget::Workspace(workspace_id) => self.ctx.workspace_channel(&workspace_id, on_event, origin).await,
            ChannelTarget::Place(place_id) if place_id == HERE_PLACE_ID => self.ctx.local_channel(on_event).await,
            ChannelTarget::Place(place_id) => match self.ctx.place_channel(&place_id, on_event) {
                Some(link) => Ok(link),
                None => {
                    let name = self.ctx.place_name(&place_id).unwrap_or_else(|| place_id.clone());
                    Err(HostError::new(absent_computer(&name, None).sentence).into())
                }
            },
        }
    }

    /// F7, preferences.ts:142.
    pub fn agents_changed(&self, target: Option<Value>) {
        let mut event = Map::new();
        event.insert("type".to_string(), json!("agents.changed"));
        if let Some(target) = target {
            event.insert("target".to_string(), target);
        }
        self.ctx.emit(Value::Object(event));
    }

    /// F7, preferences.ts:143: nothing for an agent with no adapter, else the context's `setup_view`.
    pub fn agents_setup_of(&self, place_id: &str, agent: &str) -> Option<Value> {
        if !self.ctx.adapter_ids().iter().any(|known| known == agent) {
            return None;
        }
        Some(self.ctx.setup_view(place_id, agent))
    }

    /// F7, preferences.ts:129: whether the person lets this host ask the vendors for each agent's newest version.
    pub async fn agents_latest_on(&self) -> AccountResult<bool> {
        Ok(read_preferences(&*self.ctx, self.labs_flag()).await?.agent_versions)
    }

    /// F7, preferences.ts:146: the backend's `callbackRelay`.
    pub fn agents_relayed(&self) -> bool {
        self.ctx.relayed()
    }

    /// F7, preferences.ts:144.
    pub async fn agents_setup_write(&self, place_id: &str, agent: &str, change: Value) -> AccountResult<()> {
        let ids = self.ctx.adapter_ids();
        if !ids.iter().any(|known| known == agent) {
            return Err(usage(&no_adapter_line(agent, &ids)).into());
        }
        self.ctx.setup_write(place_id, agent, change).await?;
        if self.ctx.drop_setup_refusal(place_id, agent) {
            self.ctx.setup_refusal_moved(place_id, agent);
        }
        Ok(())
    }

    /// F7, preferences.ts:130: whether the icon is asked now (read at every ask).
    pub async fn icons_on(&self) -> AccountResult<bool> {
        Ok(read_preferences(&*self.ctx, self.labs_flag()).await?.server_icons)
    }
}
