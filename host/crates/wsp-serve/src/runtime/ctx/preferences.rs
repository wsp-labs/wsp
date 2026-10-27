// SPDX-License-Identifier: AGPL-3.0-only
//! The context's preferences, activity, slate and pipe readings (preferences.ts, activity.ts, account/slates.ts,
//! pipe.ts, runtime.ts:428, :435, :486, :488): most of them one call on the area that holds the rule, the rest the few
//! lines no crate has. The agents reader is built here on its first read (preferences.ts:99-151).
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_account::account::channels::ChannelsArea;
use wsp_account::account::core_types::{AccountError, AccountResult, Entry as AccountEntry};
use wsp_account::account::preferences::{AccessTable, CatalogAgentKeys, ChannelTarget, PreferencesCtx, ServerIcons as AccountServerIcons};
use wsp_account::account::rules::like_of;
use wsp_core::agent_setup::key_of;
use wsp_core::daemon_channel::{ChannelError, DaemonChannel, Frame};
use wsp_core::harness_catalog::harness_catalog;
use wsp_core::types::api::{DaemonEventListener, SessionsApi as _, StartOptions, WorkspacesApi as _};
use wsp_core::types::events::EventBus as _;
use wsp_core::types::internal::{ThreadRecord, PREFERENCES, PREFERENCES_ID};
use wsp_core::types::wiring::{at_provider, Detach, WorkspaceRecord};
use wsp_machines::agents_read::{
    AgentsPlaces, AgentsPorts, AgentsReadOptions, AgentsReads, AgentsWorkspace, ResolvedServer, ServerIcons as MachineIcons,
};
use wsp_machines::machines::machines::Entry;
use wsp_slates::account::slates::{ReadServer, ServerReadError};
use wsp_slates::slate_images::ImageRefusal;
use wsp_slates::slate_mcp_on::StdioRoad;
use wsp_slates::slates::{Delivered, DeliveryOutcome};
use wsp_threads::activity::DaemonAsk;
use wsp_threads::threads::agents::AgentsArea;
use wsp_wire::types::agents_report::{AgentsProject, AgentsTarget};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::thread_defaults::{AgentSetup, AgentSetupSet, AgentSetupView};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::{thread_key_of, SessionOrigin, SessionStatus, SessionView};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::daemon_version::STDIO_DAEMON_VERSION;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::exit::{HostError, Kind};
use wsp_words::words::computer::THIS_COMPUTER;
use wsp_words::workspace_state::is_local_workspace;

use super::Ctx;

/// The areas this packet's tests read at their own seams: `Ctx`'s slots are crate-private.
pub struct PreferencesSeams {
    pub preferences: Rc<wsp_account::account::preferences::PreferencesArea>,
    pub agents_read: Rc<AgentsReads>,
    pub activity: Rc<wsp_threads::activity::Activity>,
    pub slates: Rc<dyn wsp_slates::slates::Slates>,
    pub pipe: Option<Rc<wsp_core::pipe::area::PipeArea>>,
    pub agents: Rc<AgentsArea>,
    pub threads: Rc<wsp_threads::threads::threads::ThreadsArea>,
    pub transcripts: Rc<wsp_threads::threads::transcripts::TranscriptsArea>,
    pub folders: Rc<wsp_account::projects::folders::Folders>,
}

/// The kind of process a thread owns, as the proc-scopes area names it.
fn preferences_proc_kind(kind: wsp_frames::ProcKind) -> wsp_wire::wire::daemon_replies::ProcKind {
    serde_json::to_value(kind).ok().and_then(|word| serde_json::from_value(word).ok()).expect("both name a kind with the same word")
}

/// `os.homedir()`: the process's home folder.
fn preferences_home() -> String {
    std::env::home_dir().map(|home| home.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `setupView(setup)` (thread-defaults.ts:90-98): the setup as the app reads it, the names of its variables sorted and
/// never one value.
fn preferences_setup_view(setup: Option<&AgentSetup>) -> Value {
    let mut env_names: Vec<String> = setup.and_then(|s| s.env.as_ref()).map(|env| env.keys().cloned().collect()).unwrap_or_default();
    env_names.sort();
    let view = AgentSetupView {
        on: setup.is_none_or(|s| s.on.is_none()),
        program: setup.and_then(|s| s.program.clone()),
        config_dir: setup.and_then(|s| s.config_dir.clone()),
        args: setup.and_then(|s| s.args.clone()),
        env_names,
    };
    serde_json::to_value(view).expect("a setup view is JSON")
}

/// A channel error as the area's error (channels.ts:214's `ask`): a door's refusal and a token's keep their kind.
fn preferences_channel_failure(e: ChannelError) -> AccountError {
    match e.kind() {
        Some("refused") => HostError::with_kind(e.to_string(), Kind::Refused).into(),
        Some("reauth") => HostError::with_kind(e.to_string(), Kind::Reauth).into(),
        _ => HostError::new(e.to_string()).into(),
    }
}

/// A daemon event the listener of this file's channels takes as the JSON it is.
fn preferences_listener(on_event: DaemonEventListener) -> Rc<dyn Fn(Value)> {
    Rc::new(move |event: Value| {
        if let Value::Object(event) = event {
            on_event(event);
        }
    })
}

/// A listener that takes the event as JSON, handed to an area that reads it as a map.
fn preferences_json_listener(on_event: Rc<dyn Fn(Value)>) -> DaemonEventListener {
    Rc::new(move |event: Map<String, Value>| on_event(Value::Object(event)))
}

/// A channel as `overChannel` asks over it (channels.ts:212): one frame out, its reply through `replyOf`.
struct PreferencesAsk {
    channel: Rc<dyn DaemonChannel>,
    channels: Rc<ChannelsArea>,
}

impl DaemonAsk for PreferencesAsk {
    fn ask(&self, frame: Value) -> LocalBoxFuture<'static, Result<Map<String, Value>, AccountError>> {
        let channel = self.channel.clone();
        let channels = self.channels.clone();
        async move {
            let Value::Object(frame) = frame else {
                return Err(HostError::new("a daemon frame is an object").into());
            };
            let sent: Frame = frame.clone();
            let reply = channel.send(frame).await.map_err(preferences_channel_failure)?;
            channels.reply_of(&sent, reply)
        }
        .boxed_local()
    }
    fn close(&self) {
        self.channel.close();
    }
}

/// A computer's daemon road as the slate area dials it.
struct PreferencesStdio(wsp_account::account::channels::daemon::StdioRoad);

impl StdioRoad for PreferencesStdio {
    fn send(&self, frame: Value) -> LocalBoxFuture<'static, Result<Value, String>> {
        let send = self.0.send.clone();
        async move {
            let Value::Object(frame) = frame else {
                return Err("a daemon frame is an object".to_string());
            };
            send(frame).await.map(Value::Object).map_err(|e| e.message())
        }
        .boxed_local()
    }
    fn listen(&self, f: Rc<dyn Fn(&Value)>) -> Box<dyn FnOnce()> {
        let off = (self.0.listen)(Rc::new(move |event: Value| f(&event)));
        Box::new(move || off())
    }
    fn closed(&self) -> futures::future::Shared<LocalBoxFuture<'static, ()>> {
        let channel = self.0.channel.clone();
        async move {
            channel.closed().await;
        }
        .boxed_local()
        .shared()
    }
    fn close(&self) {
        self.0.channel.close();
    }
}

impl Ctx {
    pub fn preferences_seams(&self) -> PreferencesSeams {
        PreferencesSeams {
            preferences: self.preferences.get(),
            agents_read: self.agents_read(),
            activity: self.activity.get(),
            slates: self.slates.get(),
            pipe: self.pipe.get(),
            agents: self.agents.get(),
            threads: self.threads.get(),
            transcripts: self.transcripts.get(),
            folders: self.folders.get(),
        }
    }

    /// `bus.emit(event)` for an event written as the JSON a TS object literal is: a listener that fails is said, since
    /// the caller has no one to hand the failure to.
    fn preferences_emit_json(&self, event: Value) {
        match wsp_wire::wire::events_union::parse_event(&event) {
            Ok(event) => {
                if let Err(e) = self.bus.emit(event) {
                    wsp_core::console::warn(&e.message);
                }
            }
            Err(e) => wsp_core::console::warn(&format!("an event was not emitted: {e:?}")),
        }
    }

    fn preferences_record(entry: &Entry) -> WorkspaceRecord {
        entry.borrow().record.borrow().clone()
    }

    /// The host's agents ports (`opts.agentsReader` and its five siblings), read back from the one holder the options
    /// carry as `place_door_over` reads the place wiring. None where the host wired none, so every read answers the TS's
    /// refusal for a missing reader.
    pub(crate) fn agents_ports(&self) -> Option<Rc<AgentsPorts>> {
        let held = self.opts.agents_ports.clone()?;
        let any: Rc<dyn std::any::Any> = held;
        Some(any.downcast::<AgentsPorts>().unwrap_or_else(|_| panic!("opts.agents_ports is not the machines' AgentsPorts")))
    }

    /// `agentsReads({ reader, places, workspace, .. })` over the context (preferences.ts:99-151).
    pub(super) fn build_agents_read(&self) -> Rc<AgentsReads> {
        let ports = self.agents_ports().unwrap_or_default();
        let area = self.preferences.get();
        let agents = self.agents.get();
        let me = self.me.clone();
        let live = self.live.clone();
        let ids = self.ids.clone();

        let places = {
            let me = me.clone();
            Rc::new(move || -> Option<Rc<dyn AgentsPlaces>> {
                let ctx = me.upgrade()?;
                ctx.wired_door()?;
                Some(ctx)
            })
        };
        let workspace = {
            let area = area.clone();
            Rc::new(move |id: String, origin: Option<Caller>| -> LocalBoxFuture<'static, Result<AgentsWorkspace, HostError>> {
                let area = area.clone();
                let live = live.clone();
                async move {
                    let held = area.agents_workspace(&id, origin.as_ref()).await?;
                    let machine = live
                        .borrow()
                        .get(&id)
                        .map(|entry| entry.borrow().machine.clone())
                        .ok_or_else(|| HostError::new(format!("no workspace {id}")))?;
                    let phase = serde_json::from_value(Value::String(held.phase.clone())).map_err(|e| HostError::new(e.to_string()))?;
                    let pause_mode = match held.pause_mode {
                        Some(mode) => Some(serde_json::from_value(Value::String(mode)).map_err(|e| HostError::new(e.to_string()))?),
                        None => None,
                    };
                    Ok(AgentsWorkspace {
                        name: held.name,
                        phase,
                        local: held.local,
                        machine,
                        project: AgentsProject { id: held.project_id, name: held.project_name, path: held.project_path },
                        stores: Some(held.stores),
                        seeds: Some(held.seeds),
                        brokered: held.brokered.then_some(true),
                        pause_mode,
                    })
                }
                .boxed_local()
            })
        };
        let place_stores = {
            let agents = agents.clone();
            Rc::new(move |place: &str, home: &str| agents.place_stores(place, home).unwrap_or_default())
        };
        let stores_here = {
            let agents = agents.clone();
            Rc::new(move || -> LocalBoxFuture<'static, Result<IndexMap<String, String>, HostError>> {
                let agents = agents.clone();
                async move { agents.stores_here().await }.boxed_local()
            })
        };
        let projects = {
            let area = area.clone();
            Rc::new(move |place: String| -> LocalBoxFuture<'static, Result<Vec<AgentsProject>, HostError>> {
                let area = area.clone();
                async move {
                    Ok(area.agents_projects(&place).await.into_iter().map(|(id, name, path)| AgentsProject { id, name, path }).collect())
                }
                .boxed_local()
            })
        };
        let latest_on = {
            let area = area.clone();
            Rc::new(move || -> LocalBoxFuture<'static, bool> {
                let area = area.clone();
                async move { area.agents_latest_on().await.unwrap_or(true) }.boxed_local()
            })
        };
        let channel = {
            let area = area.clone();
            Rc::new(
                move |target: AgentsTarget,
                      on_event: Rc<dyn Fn(Map<String, Value>)>,
                      origin: Option<Caller>|
                      -> LocalBoxFuture<'static, Result<Rc<dyn DaemonChannel>, HostError>> {
                    let area = area.clone();
                    async move {
                        let target = match target {
                            AgentsTarget::V1(held) => ChannelTarget::Workspace(held.workspace_id),
                            AgentsTarget::V0(held) => ChannelTarget::Place(held.place_id),
                        };
                        let listen: Rc<dyn Fn(Value)> = Rc::new(move |event: Value| {
                            if let Value::Object(event) = event {
                                on_event(event);
                            }
                        });
                        area.agents_channel(target, listen, origin.as_ref()).await.map_err(HostError::from)
                    }
                    .boxed_local()
                },
            )
        };
        let changed = {
            let area = area.clone();
            Rc::new(move |target: Option<AgentsTarget>| area.agents_changed(target.and_then(|t| serde_json::to_value(&t).ok())))
        };
        let relayed = {
            let area = area.clone();
            Rc::new(move || area.agents_relayed())
        };
        let setup_of = {
            let area = area.clone();
            Rc::new(move |place: &str, agent: &str| -> Option<AgentSetupView> {
                area.agents_setup_of(place, agent).and_then(|view| serde_json::from_value(view).ok())
            })
        };
        let setup_write = {
            let area = area.clone();
            Rc::new(move |place: String, agent: String, change: AgentSetupSet| -> LocalBoxFuture<'static, Result<(), HostError>> {
                let area = area.clone();
                async move {
                    let change = serde_json::to_value(&change).map_err(|e| HostError::new(e.to_string()))?;
                    area.agents_setup_write(&place, &agent, change).await.map_err(HostError::from)
                }
                .boxed_local()
            })
        };
        let icons = ports.icons.clone().map(|icons| Rc::new(IconsWhenOn { icons, area: area.clone() }) as Rc<dyn MachineIcons>);
        AgentsReads::new(AgentsReadOptions {
            reader: ports.reader.clone(),
            places,
            workspace,
            place_stores: Some(place_stores),
            stores_here: Some(stores_here),
            projects: Some(projects),
            acts: ports.acts.clone(),
            skills: ports.skills.clone(),
            servers: ports.servers.clone(),
            plugins: ports.plugins.clone(),
            icons,
            latest_on: Some(latest_on),
            channel,
            changed,
            clock: self.clock.clone(),
            relayed: Some(relayed),
            log: None,
            setup_of: Some(setup_of),
            setup_write: Some(setup_write),
            random_bytes: Rc::new(move |n| ids.bytes(n)),
            warn: Rc::new(|line: &str| wsp_core::console::warn(line)),
        })
    }
}

#[async_trait::async_trait(?Send)]
impl PreferencesCtx for Ctx {
    fn env(&self) -> indexmap::IndexMap<String, String> {
        match &self.opts.env {
            Some(held) => held.iter().filter_map(|(k, v)| v.clone().map(|v| (k.clone(), v))).collect(),
            None => std::env::vars().collect(),
        }
    }
    fn server_icons(&self) -> Option<Rc<dyn AccountServerIcons>> {
        let icons = self.agents_ports()?.icons.clone()?;
        Some(Rc::new(HeldIcons(icons)))
    }
    fn home_dir(&self) -> String {
        preferences_home()
    }
    fn adapter_ids(&self) -> Vec<String> {
        self.opts.adapters.keys().cloned().collect()
    }
    fn access_table(&self, agent: &str) -> AccessTable {
        match harness_catalog(agent) {
            Some(catalog) => {
                AccessTable { label: catalog.label.clone(), table: serde_json::to_value(&catalog).expect("a catalog is JSON") }
            }
            None => {
                let label = AgentsArea::agent_label(agent);
                AccessTable { table: json!({ "label": label, "permissionModes": [] }), label }
            }
        }
    }
    fn project_held(&self, id: &str) -> bool {
        self.projects_held.borrow().contains_key(id)
    }
    fn project_of(&self, id: &str) -> Result<(String, String), HostError> {
        let project = self.channels.get().project_held(id)?;
        Ok((project.id, project.name))
    }
    fn default_agent_of(&self, prefs: &Preferences) -> String {
        self.agents.get().default_agent_of(prefs, None)
    }
    fn session_thread_keys(&self) -> Vec<String> {
        self.sessions.borrow().values().map(|row| thread_key_of(&row.borrow().view)).collect()
    }
    fn thread_record_held(&self, thread_id: &str) -> bool {
        self.thread_records.borrow().contains_key(thread_id)
    }
    /// A failed start ends the write in the TS; the trait's start answers plain, so the write goes on and reads what
    /// the store holds.
    async fn ready(&self) {
        let _ = self.booted().await;
    }
    async fn stored(&self) -> AccountResult<Option<Value>> {
        Ok(self.store.get(PREFERENCES, PREFERENCES_ID).await?)
    }
    async fn store(&self, prefs: &Preferences) -> AccountResult<()> {
        let value = serde_json::to_value(prefs).map_err(|e| HostError::new(e.to_string()))?;
        Ok(self.store.put(PREFERENCES, PREFERENCES_ID, value).await?)
    }
    fn held(&self) -> Option<Preferences> {
        self.state.with(|s| s.preferences_held.clone())
    }
    fn set_held(&self, prefs: Preferences) {
        self.state.with(|s| s.preferences_held = Some(prefs));
    }
    fn emit(&self, event: Value) {
        self.preferences_emit_json(event);
    }
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) {
        self.hear_workspace_deleted(f)
    }
    async fn entry_of(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<AccountEntry, HostError> {
        self.boot.get().entry_of(workspace_id, origin, Default::default()).await
    }
    fn catalog_agents(&self) -> Vec<CatalogAgentKeys> {
        wsp_catalog::catalog::catalog_agents()
            .into_iter()
            .map(|agent| CatalogAgentKeys {
                id: agent.id.clone(),
                state_home_env: agent.state_home_env.clone(),
                plugin_seed_env: agent.plugins.and_then(|plugins| plugins.seed()).map(|seed| seed.env().to_string()),
            })
            .collect()
    }
    fn thread_env(&self, entry: &AccountEntry, agent: &str) -> IndexMap<String, String> {
        self.agents.get().thread_env(entry, Some(agent))
    }
    fn checkout_of(&self, entry: &AccountEntry) -> Result<String, HostError> {
        self.channels.get().checkout_of(&Ctx::preferences_record(entry))
    }
    fn is_local(&self, entry: &AccountEntry) -> bool {
        is_local_workspace(Some(&Ctx::preferences_record(entry).kind))
    }
    fn at_provider(&self, entry: &AccountEntry) -> bool {
        let place = self.channels.get().place_id_of(&Ctx::preferences_record(entry));
        at_provider(place.as_deref(), &*self.places)
    }
    fn pause_mode(&self, entry: &AccountEntry) -> Option<String> {
        let view = self.records.get().view(&Ctx::preferences_record(entry)).ok()?;
        serde_json::to_value(view.pause_mode?).ok().and_then(|mode| mode.as_str().map(str::to_string))
    }
    fn projects_on(&self, place_id: &str) -> Vec<(String, String, String)> {
        self.projects_held
            .borrow()
            .values()
            .filter(|project| project.computer == place_id)
            .map(|project| (project.id.clone(), project.name.clone(), project.checkout.clone().unwrap_or_else(|| project.path.clone())))
            .collect()
    }
    async fn workspace_channel(
        &self,
        workspace_id: &str,
        on_event: Rc<dyn Fn(Value)>,
        origin: Option<&Caller>,
    ) -> AccountResult<Rc<dyn DaemonChannel>> {
        let workspaces = self.workspaces.get();
        Ok(workspaces.daemon_channel(workspace_id, preferences_json_listener(on_event), origin.cloned()).await?)
    }
    async fn local_channel(&self, on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>> {
        let road = self.kinds.get().local_road().await.map_err(|e| AccountError::from(e.into_host()))?;
        self.channels.get().channel_over(&road, THIS_COMPUTER, on_event).await
    }
    fn place_channel(&self, place_id: &str, on_event: Rc<dyn Fn(Value)>) -> Option<Rc<dyn DaemonChannel>> {
        self.wired_door()?.channel(place_id, preferences_json_listener(on_event))
    }
    fn place_name(&self, place_id: &str) -> Option<String> {
        self.wired_door().map(|door| door.name_of(place_id))
    }
    async fn setup_write(&self, place_id: &str, agent: &str, change: Value) -> AccountResult<()> {
        let change: AgentSetupSet = serde_json::from_value(change).map_err(|e| HostError::with_kind(e.to_string(), Kind::Usage))?;
        let agents = self.agents.get();
        let place = place_id.to_string();
        let folder = Rc::new(move |path: &str| {
            let agents = agents.clone();
            let place = place.clone();
            let path = path.to_string();
            async move { agents.config_folder_on(&place, &path).await }.boxed_local()
        });
        let options = wsp_core::agent_setup::SetupSetOptions { folder, agent_name: AgentsArea::agent_label(agent) };
        self.setups.set(place_id, agent, change, options).await?;
        Ok(())
    }
    fn setup_view(&self, place_id: &str, agent: &str) -> Value {
        preferences_setup_view(self.setups.get(place_id, agent).as_ref())
    }
    fn drop_setup_refusal(&self, place_id: &str, agent: &str) -> bool {
        self.agents.get().setup_refusals.borrow_mut().shift_remove(&key_of(place_id, agent)).is_some()
    }
    fn setup_refusal_moved(&self, place_id: &str, agent: &str) {
        self.boot.get().setup_refusal_moved(place_id, agent);
    }
    fn relayed(&self) -> bool {
        self.backend.capabilities().callback_relay
    }
    fn forget_read(&self, workspace_id: &str) {
        if self.agents_read.is_filled() {
            self.agents_read.get().forget(workspace_id);
        }
    }
}

/// The icon store of the host's ports as the preferences area reads it: its folder, and its forget failing in node's words.
struct HeldIcons(Rc<dyn MachineIcons>);

impl AccountServerIcons for HeldIcons {
    fn folder(&self) -> String {
        self.0.folder()
    }
    fn forget(&self) -> Result<(), String> {
        self.0.forget()
    }
}

/// preferences.ts:132: an icon is asked of no one while the person's switch for icons is off.
struct IconsWhenOn {
    icons: Rc<dyn MachineIcons>,
    area: Rc<wsp_account::account::preferences::PreferencesArea>,
}

#[async_trait::async_trait(?Send)]
impl MachineIcons for IconsWhenOn {
    fn folder(&self) -> String {
        self.icons.folder()
    }
    async fn icon(&self, host: &str, refresh: Option<bool>) -> Result<Option<String>, HostError> {
        if !self.area.get().await?.server_icons {
            return Ok(None);
        }
        self.icons.icon(host, refresh).await
    }
    fn forget(&self) -> Result<(), String> {
        self.icons.forget()
    }
}

/// With no icon store wired the trait answers plain where the TS has none: `server_icons` is then `None`.
impl AccountServerIcons for Ctx {
    fn folder(&self) -> String {
        String::new()
    }
    fn forget(&self) -> Result<(), String> {
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_threads::activity::ActivityCtx for Ctx {
    fn clock(&self) -> Rc<dyn wsp_core::clock::Clock> {
        self.clock.clone()
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.booted().await
    }
    fn has_door(&self) -> bool {
        self.wired_door().is_some()
    }
    fn place_name(&self, place: &str) -> Option<String> {
        self.wired_door().map(|door| door.name_of(place))
    }
    async fn place_rows(&self) -> Result<Vec<wsp_wire::views::place_more::PlaceView>, HostError> {
        match self.wired_door() {
            Some(door) => door.rows().await,
            None => Ok(Vec::new()),
        }
    }
    fn joined(&self, place: &str) -> bool {
        self.wired_door().is_some_and(|door| door.joined(place))
    }
    fn has_link(&self, place: &str) -> bool {
        self.wired_door().is_some_and(|door| door.link(place).is_some())
    }
    fn has_local_daemon(&self) -> bool {
        self.opts.local.as_ref().is_some_and(|local| local.daemon_road.is_some())
    }
    async fn local_channel(&self, on_event: DaemonEventListener, guarded: bool) -> Result<Rc<dyn DaemonAsk>, HostError> {
        let road = self.kinds.get().local_road().await.map_err(|e| e.into_host())?;
        let channels = self.channels.get();
        let channel = channels.channel_over(&road, THIS_COMPUTER, preferences_listener(on_event)).await?;
        let channel = if guarded { channels.kill_guarded(channel, true) } else { channel };
        Ok(Rc::new(PreferencesAsk { channel, channels }))
    }
    fn box_channel(
        &self,
        place: &str,
        on_event: DaemonEventListener,
    ) -> Option<LocalBoxFuture<'static, Result<Rc<dyn DaemonAsk>, HostError>>> {
        let channel = self.wired_door()?.channel(place, on_event)?;
        let channels = self.channels.get();
        Some(async move { Ok(Rc::new(PreferencesAsk { channel, channels }) as Rc<dyn DaemonAsk>) }.boxed_local())
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        self.sessions.clone()
    }
    fn records(&self) -> wsp_threads::threads::threads::ThreadRecords {
        self.thread_records.clone()
    }
    fn live_entry(&self, workspace_id: &str) -> Option<wsp_threads::threads::threads::Entry> {
        self.live.borrow().get(workspace_id).cloned()
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.threads.get().latest_on(thread_id)
    }
    fn thread_facts(&self, thread_id: &str) -> Option<wsp_wire::views::preferences::ThreadFacts> {
        self.boot.get().thread_facts(thread_id)
    }
    fn scopes_here(&self) -> Vec<wsp_wire::wire::daemon::ProcScope> {
        self.proc_scopes.get().scopes_here()
    }
    fn kept_pid(&self, thread_id: &str) -> Option<u32> {
        self.threads.get().kept_agents.borrow().get(thread_id).and_then(|kept| kept.borrow().pid)
    }
    fn reap_kept_now(&self, thread_id: &str) {
        self.threads.get().reap_kept(thread_id, true);
    }
    fn host_pid(&self) -> u32 {
        std::process::id()
    }
    fn drives(&self, kind: wsp_wire::views::workspace_kind::WorkspaceKind, caller: Option<&Caller>) -> bool {
        self.rules.get().drives(kind, None, caller)
    }
    fn refusal_for(&self, record: &WorkspaceRecord, caller: Option<&Caller>) -> Result<Option<HostError>, HostError> {
        Ok(self.rules.get().refusal_for(Some(&like_of(record)), caller)?.map(HostError::new))
    }
    fn settles_thread(&self, thread_id: &str, caller: Option<&Caller>) -> bool {
        self.threads.get().settles_thread(thread_id, caller)
    }
    /// `moduleOf` throws for a kind the host wired none of; the trait answers plain, so such a kind has no end of its own.
    fn end_thread(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        thread_id: &str,
        o: wsp_core::types::internal::EndThread,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, HostError>>> {
        let kind = Ctx::preferences_record(entry).kind;
        let module = self.kinds.get().module_of(kind).ok()?;
        let held = entry.borrow();
        module.end_thread(&held, thread_id, o)
    }
    fn with_daemon(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        frame: Value,
    ) -> LocalBoxFuture<'static, Result<Value, AccountError>> {
        let channels = self.channels.get();
        let entry = entry.clone();
        async move {
            let Value::Object(frame) = frame else {
                return Err(HostError::new("a daemon frame is an object").into());
            };
            channels.with_daemon(&entry, Box::new(move |ask| Box::pin(async move { ask(frame).await.map(Value::Object) }))).await
        }
        .boxed_local()
    }
    fn close_servers(&self, thread_id: &str) {
        self.slates.get().close_servers(thread_id);
    }
}

/// The adapter nothing but a stand-in holds: a runtime's context is not a channel, so it asks nothing.
impl DaemonAsk for Ctx {
    fn ask(&self, _frame: Value) -> LocalBoxFuture<'static, Result<Map<String, Value>, AccountError>> {
        async move { Err(HostError::new("this context holds no daemon channel to ask over").into()) }.boxed_local()
    }
    fn close(&self) {}
}

impl wsp_slates::account::slates::SlatesAreaCtx for Ctx {
    fn store(&self) -> Rc<dyn wsp_store::store::Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn wsp_core::clock::Clock> {
        self.clock.clone()
    }
    fn record(&self, event: wsp_wire::types::slate_wire::SessionSlateEvent) {
        self.transcripts.get().record(wsp_wire::views::session_events::SessionEvent::SessionSlateEvent(event));
    }
    fn emit(&self, event: wsp_slates::slates::SlateEmit) {
        let value = match event {
            wsp_slates::slates::SlateEmit::Values(event) => ("slate.values", serde_json::to_value(event)),
            wsp_slates::slates::SlateEmit::Run(event) => ("slate.run", serde_json::to_value(event)),
        };
        if let (kind, Ok(Value::Object(mut fields))) = value {
            let mut typed = Map::new();
            typed.insert("type".into(), Value::String(kind.into()));
            typed.append(&mut fields);
            self.preferences_emit_json(Value::Object(typed));
        }
    }
    fn latest_on(&self, thread: &str) -> Option<SessionView> {
        self.threads.get().latest_on(thread)
    }
    fn thread_record(&self, thread: &str) -> Option<ThreadRecord> {
        self.thread_records.borrow().get(thread).cloned()
    }
    fn running_turn(&self, thread: &str) -> Option<String> {
        self.sessions
            .borrow()
            .values()
            .map(|row| row.borrow())
            .find(|row| row.view.thread_id.as_deref() == Some(thread) && row.view.status == SessionStatus::Running)
            .map(|row| row.turn_id.clone())
    }
    fn live(&self, workspace: &str) -> Option<Entry> {
        self.live.borrow().get(workspace).cloned()
    }
    fn box_of(&self, thread: &str) -> Option<Entry> {
        self.transcripts.get().box_of(thread)
    }
    fn place_of(&self, thread: &str) -> Option<Arc<wsp_engine::place_machine::PlaceFolderMachine>> {
        let entry = self.transcripts.get().box_of(thread)?;
        let machine = entry.borrow().machine.clone();
        let place = entry.borrow().record.borrow().place.clone()?;
        // `machine instanceof PlaceFolderMachine`: the door keeps one folder machine per place, and a box thread on one
        // holds that instance.
        let folder = self.wired_door()?.folder_computer(&place)?.machine;
        (Arc::as_ptr(&folder) as *const () == Arc::as_ptr(&machine) as *const ()).then_some(folder)
    }
    fn folder_of(&self, workspace: &str, session: &str) -> Option<String> {
        self.channels.get().folder_of(workspace, session)
    }
    fn runs_in(&self, entry: &Entry, ran_in: Option<&str>, fallback: &str) -> String {
        self.folders.get().runs_in(entry, ran_in, fallback)
    }
    /// The TS throws for a kind the host wired none of; there the folder is the checkout's, which the caller falls to.
    fn kind_folder(&self, entry: &Entry) -> Option<String> {
        let record = Ctx::preferences_record(entry);
        self.kinds.get().module_of(record.kind).ok()?.folder(&record).ok().flatten()
    }
    fn checkout_of(&self, entry: &Entry) -> Result<String, HostError> {
        self.channels.get().checkout_of(&Ctx::preferences_record(entry))
    }
    fn path_exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
    fn home_dir(&self) -> String {
        preferences_home()
    }
    fn computer_of(&self, entry: &Entry) -> String {
        self.channels.get().computer_of(entry)
    }
    fn root_of(&self, thread: &str) -> String {
        self.rules.get().root_of(thread)
    }
    fn tool_asks(&self, harness: Option<&str>, mode: Option<&str>) -> wsp_slates::slate_mcp::ToolAsks {
        wsp_core::harness_catalog::tool_asks(harness, mode)
    }
    fn entry_rate_usd_per_hour(&self, entry: &Entry) -> Option<f64> {
        let record = Ctx::preferences_record(entry);
        let size = record.size.clone();
        let backend = self.kinds.get().backend_for(&record).ok()?;
        Some(backend.pricing().rate_usd_per_hour(wsp_engine::machine::Size { cpu: size.cpu.0, mem_mb: size.mem_mb.0 }))
    }
    fn at_provider(&self, place: &str) -> bool {
        at_provider(Some(place), &*self.places)
    }
    fn boot_work(&self, workspace: &str) -> LocalBoxFuture<'static, ()> {
        let held = self.boot_work.borrow().get(workspace).cloned();
        async move {
            if let Some(held) = held {
                held.await;
            }
        }
        .boxed_local()
    }
    fn wake(&self, workspace: &str) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let workspaces = self.workspaces.get();
        let workspace = workspace.to_string();
        async move { workspaces.wake(&workspace, None).await.map(|_| ()) }.boxed_local()
    }
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let ctx = self.rc();
        async move { ctx.booted().await }.boxed_local()
    }
    fn settled_now(&self, thread: &str, ms: Option<f64>) -> bool {
        self.boot.get().settled_now(thread, ms)
    }
    /// A record that cannot be read leaves the person's default, the same as one that was never written.
    fn settle_after(&self) -> LocalBoxFuture<'static, String> {
        let area = self.preferences.get();
        async move {
            let held = match area.get().await {
                Ok(held) => held,
                Err(_) => wsp_wire::views::preferences::default_preferences(),
            };
            serde_json::to_value(held.settle_after).ok().and_then(|word| word.as_str().map(str::to_string)).unwrap_or_default()
        }
        .boxed_local()
    }
    fn tree_under(&self, lead: &str) -> Vec<String> {
        self.threads.get().tree_under(lead)
    }
    fn thread_of_token(&self, token: &str) -> Result<String, HostError> {
        self.threads.get().thread_of_token(token)
    }
    fn own_process(&self, thread: &str, pid: u32, kind: wsp_frames::ProcKind) -> Detach {
        let release = self.proc_scopes.get().own_process(thread, i64::from(pid), preferences_proc_kind(kind));
        Box::new(move || release())
    }
    fn daemon_image(&self, entry: &Entry, path: String) -> LocalBoxFuture<'static, Result<Value, ImageRefusal>> {
        let channels = self.channels.get();
        let entry = entry.clone();
        async move {
            let mut frame = Frame::new();
            frame.insert("op".into(), json!("fs.image"));
            frame.insert("path".into(), json!(path));
            channels
                .with_daemon(&entry, Box::new(move |ask| Box::pin(async move { ask(frame).await.map(Value::Object) })))
                .await
                .map_err(|e| ImageRefusal { code: e.code().map(str::to_string), message: e.message() })
        }
        .boxed_local()
    }
    fn daemon_hash(&self, entry: &Entry, root: String, paths: Vec<String>) -> LocalBoxFuture<'static, Result<Value, String>> {
        let channels = self.channels.get();
        let entry = entry.clone();
        async move {
            let mut frame = Frame::new();
            frame.insert("op".into(), json!("fs.hash"));
            frame.insert("root".into(), json!(root));
            frame.insert("paths".into(), json!(paths));
            channels
                .with_daemon(&entry, Box::new(move |ask| Box::pin(async move { ask(frame).await.map(Value::Object) })))
                .await
                .map_err(|e| e.message())
        }
        .boxed_local()
    }
    fn agents_server(&self, target: AgentsTarget, agent: &str, name: &str) -> LocalBoxFuture<'static, Result<ReadServer, ServerReadError>> {
        let reads = self.agents_read();
        let (agent, name) = (agent.to_string(), name.to_string());
        async move {
            match reads.server(&target, &agent, &name, None).await {
                Ok(ResolvedServer { spec, login }) => match spec.downcast::<wsp_slates::slate_mcp::McpServerSpec>() {
                    Ok(spec) => Ok(ReadServer { spec: (*spec).clone(), login }),
                    Err(_) => {
                        Err(ServerReadError::Other("this host's agents reader answered a server that is no slate's server".to_string()))
                    }
                },
                Err(e) if wsp_machines::agents_read::is_server_not_set_up(&e) => Err(ServerReadError::NotSetUp(e.message)),
                Err(e) if wsp_machines::agents_read::is_server_off(&e) => Err(ServerReadError::Off(e.message)),
                Err(e) => Err(ServerReadError::Other(e.message)),
            }
        }
        .boxed_local()
    }
    /// `opts.agentsReader.server`: none where the reader is not wired or resolves no server.
    fn agents_reader_server(
        &self,
        projects: Vec<AgentsProject>,
        agent: &str,
        name: &str,
    ) -> Option<LocalBoxFuture<'static, Result<wsp_slates::slate_mcp::McpServerSpec, HostError>>> {
        let reader = self.agents_ports()?.reader.clone()?;
        if !reader.has_server() {
            return None;
        }
        let on = wsp_machines::agents_read::AgentsOn::Here { projects: Some(projects), stores: None };
        let (agent, name) = (agent.to_string(), name.to_string());
        Some(
            async move {
                let ResolvedServer { spec, .. } = reader.server(&on, &agent, &name).await?;
                match spec.downcast::<wsp_slates::slate_mcp::McpServerSpec>() {
                    Ok(spec) => Ok((*spec).clone()),
                    Err(_) => Err(HostError::new("this host's agents reader answered a server that is no slate's server")),
                }
            }
            .boxed_local(),
        )
    }
    fn stdio_road(&self, entry: &Entry, min_version: Option<i64>) -> LocalBoxFuture<'static, Result<Rc<dyn StdioRoad>, String>> {
        let channels = self.channels.get();
        let entry = entry.clone();
        async move {
            let road = channels.stdio_road(&entry, min_version.unwrap_or(STDIO_DAEMON_VERSION)).await.map_err(|e| e.message())?;
            Ok(Rc::new(PreferencesStdio(road)) as Rc<dyn StdioRoad>)
        }
        .boxed_local()
    }
    fn place_name_here(&self) -> String {
        match self.wired_door() {
            Some(door) => door.name_of(HERE_PLACE_ID),
            None => wsp_core::host_id::hostname(),
        }
    }
    fn agent_label(&self, agent: &str) -> String {
        AgentsArea::agent_label(agent)
    }
    fn rows_on(&self, thread: &str) -> Vec<SessionView> {
        self.agents.get().rows_on(thread)
    }
    /// The TS holds no transcript in memory where the store keeps rows, which this store always does.
    fn transcript(&self, _workspace: &str) -> Vec<Value> {
        Vec::new()
    }
    fn usage_account_key(&self, entry: &Entry, harness: &str) -> String {
        self.threads.get().usage_account_of(entry, harness, None).0
    }
    fn usage_accounts(&self) -> LocalBoxFuture<'static, Vec<wsp_slates::slate_sources::context::AccountRow>> {
        let usage = self.usage.get();
        async move { usage.usage_accounts().await.map(|answer| answer.accounts).unwrap_or_default() }.boxed_local()
    }
    fn project_name(&self, project: &str) -> Result<String, HostError> {
        Ok(self.channels.get().project_held(project)?.name)
    }
    fn accrued(&self, workspace: &str) -> Option<f64> {
        self.transcripts.get().accrued.borrow().get(workspace).copied()
    }
    fn start_session(
        &self,
        start: wsp_slates::slates::Delivery,
    ) -> LocalBoxFuture<'static, Result<wsp_slates::slates::Delivered, HostError>> {
        let sessions = self.sessions_area.get();
        async move {
            let wsp_slates::slates::Delivery { thread_id, workspace_id, prompt, request_id } = start;
            let opts = StartOptions {
                prompt,
                thread: Some(thread_id),
                request_id: Some(request_id),
                started_by: Some(SessionOrigin::Person),
                via_slate: true,
                ..Default::default()
            };
            let handle = sessions.start(&workspace_id, opts, None).await?;
            let outcome = match handle.outcome() {
                SessionStartOutcome::Started => DeliveryOutcome::Started,
                SessionStartOutcome::Steered => DeliveryOutcome::Steered,
                SessionStartOutcome::Queued | SessionStartOutcome::Held => DeliveryOutcome::Queued,
            };
            Ok(Delivered { outcome, turn_id: Some(handle.turn_id()) })
        }
        .boxed_local()
    }
    fn on_queued(&self, request_id: &str, f: Box<dyn Fn()>) -> Box<dyn FnOnce()> {
        let request = request_id.to_string();
        let unlisten = self.hear("session.queued", move |event| {
            if let EventUnion::SessionQueued(queued) = event {
                if queued.event.request_id.as_deref() == Some(request.as_str()) {
                    f();
                }
            }
        });
        Box::new(unlisten)
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
    fn poll_pull_request(&self, entry: &Entry) {
        self.pull_requests.get().poll_pull_request(entry, None);
    }
    fn local_env(&self) -> Option<indexmap::IndexMap<String, String>> {
        self.opts.local.as_ref().map(|local| (local.env)())
    }
    fn process_env(&self) -> indexmap::IndexMap<String, String> {
        std::env::vars().collect()
    }
    fn state_folder(&self) -> Option<String> {
        let path = self.opts.state_path.as_deref()?;
        let state = std::path::Path::new(path);
        Some(
            std::path::absolute(state)
                .unwrap_or_else(|_| state.to_path_buf())
                .parent()
                .map_or_else(|| "/".to_string(), |dir| dir.to_string_lossy().into_owned()),
        )
    }
    fn on_napped(&self, f: Box<dyn Fn(&str)>) {
        self.hear_json(
            "workspace.napped",
            Rc::new(move |event| {
                if let Some(workspace_id) = event["workspaceId"].as_str() {
                    f(workspace_id);
                }
            }),
        );
    }
}

impl wsp_core::pipe::area::PipeAreaContext for Ctx {
    fn broker(&self) -> Option<Rc<dyn wsp_core::pipe::area::PipeBroker>> {
        self.opts.broker.clone().map(|broker| Rc::new(PipedBroker(broker)) as Rc<dyn wsp_core::pipe::area::PipeBroker>)
    }
    fn live(&self, workspace_id: &str) -> Option<wsp_core::pipe::area::PipeLive> {
        let entry = self.live.borrow().get(workspace_id).cloned()?;
        let entry = entry.borrow();
        let record = entry.record.borrow();
        Some(wsp_core::pipe::area::PipeLive {
            id: record.id.clone(),
            phase: serde_json::to_value(record.phase).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
            machine_id: record.machine_id.clone(),
        })
    }
    fn live_ids(&self) -> Vec<String> {
        self.live.borrow().keys().cloned().collect()
    }
    fn daemon_reach_url(&self, workspace_id: &str) -> LocalBoxFuture<'static, Result<String, String>> {
        let entry = self.live.borrow().get(workspace_id).cloned();
        let id = workspace_id.to_string();
        async move {
            let entry = entry.ok_or_else(|| format!("no workspace {id}"))?;
            let ws = entry.borrow().ws.clone();
            ws.daemon_reach().await.map(|reach| reach.url).map_err(|e| e.to_string())
        }
        .boxed_local()
    }
    fn daemon_token_of(&self, workspace_id: &str) -> LocalBoxFuture<'static, Result<Option<String>, String>> {
        let entry = self.live.borrow().get(workspace_id).cloned();
        let transcripts = self.transcripts.get();
        let id = workspace_id.to_string();
        async move {
            let entry = entry.ok_or_else(|| format!("no workspace {id}"))?;
            let machine = entry.borrow().machine.clone();
            transcripts.daemon_token_of(machine, None).await.map_err(|e| e.message)
        }
        .boxed_local()
    }
    fn daemon_version(&self, workspace_id: &str) -> LocalBoxFuture<'static, Result<Option<i64>, String>> {
        let entry = self.live.borrow().get(workspace_id).cloned();
        let kinds = self.kinds.get();
        let id = workspace_id.to_string();
        async move {
            let entry = entry.ok_or_else(|| format!("no workspace {id}"))?;
            let kind = entry.borrow().record.borrow().kind;
            let module = kinds.module_of(kind).map_err(|e| e.message)?;
            let read = module.daemon_version(&entry.borrow());
            read.await.map(|version| version.map(i64::from)).map_err(|e| e.message)
        }
        .boxed_local()
    }
    fn forget_daemon_token(&self, machine_id: &str) {
        self.transcripts.get().daemon_tokens.borrow_mut().shift_remove(machine_id);
    }
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        self.clock.clone()
    }
    fn on_any(&self, listener: std::rc::Rc<dyn Fn(&wsp_wire::wire::events_union::EventUnion)>) -> Box<dyn FnOnce()> {
        let unlisten = self.bus.on(
            wsp_core::types::events::ANY_EVENT,
            Rc::new(move |e: &wsp_wire::wire::events_union::EventUnion| {
                listener(e);
                Ok(())
            }),
        );
        Box::new(unlisten)
    }
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), String>> {
        let ctx = self.rc();
        async move { ctx.booted().await.map_err(|e| e.message) }.boxed_local()
    }
}

/// The host's broker as the pipe pool reads it: a pipe the pool dials is joined to the duplex the broker takes.
struct PipedBroker(Rc<dyn wsp_core::types::wiring::Broker>);

impl wsp_core::pipe::area::PipeBroker for PipedBroker {
    fn ready(&self) -> bool {
        self.0.ready()
    }
    fn pipe(&self) -> Option<wsp_core::pipe::pool::PipeHandler> {
        if !self.0.has_pipe() {
            return None;
        }
        let broker = self.0.clone();
        Some(Rc::new(move |workspace_id: &str, open, io: wsp_core::pipe::pool::PipeDuplex| {
            let open = serde_json::to_value(&open)
                .and_then(serde_json::from_value::<wsp_frames::PipeOpen>)
                .map_err(|e| format!("a pipe's open frame was not read: {e}"))?;
            let (near, mut far) = tokio::io::duplex(64 * 1024);
            broker.pipe(workspace_id, open, near);
            let mut io = io;
            tokio::task::spawn_local(async move {
                let _ = tokio::io::copy_bidirectional(&mut io, &mut far).await;
            });
            Ok(())
        }))
    }
    fn revoke(&self, workspace_id: &str) -> Option<futures::future::LocalBoxFuture<'static, Result<(), String>>> {
        let revoked = self.0.revoke(workspace_id)?;
        Some(Box::pin(async move { revoked.await.map_err(|e| e.message) }))
    }
}
