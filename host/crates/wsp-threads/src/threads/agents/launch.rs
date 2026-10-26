// SPDX-License-Identifier: AGPL-3.0-only
//! agents_launch: config folders, files a turn carries, environments, servers, the adapter. agents.ts:325-681.
//! Spec: spec/agents_launch.md.
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde_json::Value;
use wsp_adapters::port::{AttachmentRoad, Env, ServerValues, TurnImage};
use wsp_catalog::catalog::{catalog_agents, default_agent, mcp_agents};
use wsp_catalog::mcp::McpServer;
use wsp_catalog::mcp_launch::{
    checkout_args, checkout_of, launch_server_road, turn_server_files, LaunchConfigs, LaunchServerRoad, TurnAgent, TurnFile,
};
use wsp_catalog::mcp_login::{addresses_to_ask, needs_own_credential};
use wsp_catalog::signin_rows::server_values_of;
use wsp_core::agent_setup::{key_of, real_folder_here, real_folder_script};
use wsp_core::harness_catalog::harness_catalog;
use wsp_core::machine_exec::TurnWaiting;
use wsp_core::types::harness::{HarnessAdapter, HarnessAdapterContext};
use wsp_core::types::internal::{server_values_cut_line, server_values_left_line, server_values_unread_line};
use wsp_engine::errors::EngineError;
use wsp_engine::golden_seam::{parse_configs, read_configs_cmd, read_reason, read_whole, ReadAsk};
use wsp_engine::golden_seam::{ScopeFile, ScopeRead};
use wsp_engine::land_bytes::{land_bytes, LandBytesOptions};
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_engine::project_state::agent_homes;
use wsp_machines::machines::kinds::BrokeredLaunch;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::env::HOST_TOKEN_ENV;
use wsp_wire::types::harness_picks::{efforts_for, every_model, marked_default, model_of};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::types::thread_defaults::{
    AccessChoice, AgentDefaults, DefaultFrom, Fields, ModelPicker, ProjectOverrides, ResolvedAccess, ResolvedFolder, ResolvedPick,
    ThreadDefaults,
};
use wsp_wire::views::harness::{HarnessCatalog, HarnessModel, HarnessOption};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_words::agents_report::config_unread_line;
use wsp_words::attachments::{drop_files_line, file_path_in, files_not_landed_line, image_path_in, land_files_line};
use wsp_words::exit::{HostError, Kind};
use wsp_words::js::js_trim;
use wsp_words::project_path::under_project;
use wsp_words::thread_defaults::{access_mode, config_dir_refusal};
use wsp_words::workspace_state::sign_in_refusal_line;

use super::super::threads::Entry;
use super::{AgentsArea, AgentsCtx, SignsIn, TurnLaunch};

pub use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
/// `{ images, dir? }` (agents.ts:448).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LandedImages {
    pub images: Vec<TurnImage>,
    pub dir: Option<String>,
}

/// `{ road, configs }` (agents.ts:534).
pub struct TurnConfigs {
    pub road: LaunchServerRoad,
    pub configs: LaunchConfigs,
}

/// `{ catalog, open }` (agents.ts:421).
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultsOn {
    pub catalog: HarnessCatalog,
    pub open_model: Option<String>,
    pub open_effort: Option<String>,
}

/// `Pick<HarnessStartOptions, "serverValues" | "offServers">` (agents.ts:615).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchServers {
    pub server_values: Option<ServerValues>,
    pub off_servers: Option<Vec<String>>,
}

/// What `adapter_for` answers: the harness and the adapter built for it (agents.ts:632).
#[derive(Clone)]
pub struct BuiltAdapter {
    pub harness: String,
    pub adapter: Arc<dyn HarnessAdapter>,
}

/// What `adapter_for` takes beside the entry (agents.ts:632).
#[derive(Clone, Default)]
pub struct AdapterAsk {
    pub named: Option<String>,
    pub turn_env: Option<Env>,
    pub waiting: Option<TurnWaiting>,
    pub servers: Env,
    pub thread: Option<String>,
    pub asks_until_stopped: bool,
}

/// An engine failure as the area hands it on (its message; the marks are the engine's own).
fn engine(e: EngineError) -> HostError {
    HostError::new(e.to_string())
}

/// Node's `path.dirname` for a normalized path.
fn posix_dirname(path: &str) -> String {
    match path.rfind('/') {
        None => ".".to_string(),
        Some(0) => "/".to_string(),
        Some(at) => path[..at].to_string(),
    }
}

/// Node's `path.posix.normalize`: `.` and empty segments fold, `..` pops where it can, a trailing slash stays.
fn posix_normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => match parts.last() {
                Some(&last) if last != ".." => {
                    parts.pop();
                }
                _ if !path.starts_with('/') => parts.push(".."),
                _ => {}
            },
            other => parts.push(other),
        }
    }
    let body = parts.join("/");
    let trailing = path.ends_with('/') && !body.is_empty();
    match (path.starts_with('/'), body.is_empty()) {
        (true, _) => format!("/{body}{}", if trailing { "/" } else { "" }),
        (false, true) => ".".to_string(),
        (false, false) => format!("{body}{}", if trailing { "/" } else { "" }),
    }
}

/// `configDirLaunchRefusal` (thread-defaults.ts:432).
fn config_dir_launch_refusal(agent: &str, agent_id: &str, folder: &str, why: &str) -> String {
    format!("{agent} does not start with its config folder {folder}: {why}. Set another with wsp agents setup {agent_id} --config, or put its own back with --reset config.")
}

/// `accessRefusal` (thread-defaults.ts:299) as the one sentence a named mode is refused with.
fn access_refusal_line(label: &str, catalog: Option<&HarnessCatalog>, word: AccessChoice) -> String {
    const WORDS: [(AccessChoice, &str); 4] =
        [(AccessChoice::Ask, "ask"), (AccessChoice::AutoEdit, "auto-edit"), (AccessChoice::Full, "full"), (AccessChoice::Plan, "plan")];
    let takes: Vec<&str> = match catalog {
        Some(catalog) => WORDS.iter().filter(|(choice, _)| access_mode(catalog, *choice).is_some()).map(|(_, text)| *text).collect(),
        None => Vec::new(),
    };
    if takes.is_empty() {
        return format!("{label} takes no access of wsp's words");
    }
    let text = WORDS.iter().find(|(choice, _)| *choice == word).map_or("", |(_, text)| text);
    format!("{label} takes no {text} access; it takes {}", takes.join(", "))
}

/// The word a harness's own mode stands for (thread-defaults.ts `accessWord`).
fn access_word_of(catalog: &HarnessCatalog, mode: &str) -> Option<AccessChoice> {
    [AccessChoice::Ask, AccessChoice::AutoEdit, AccessChoice::Full, AccessChoice::Plan]
        .into_iter()
        .find(|word| catalog.access.as_ref().and_then(|access| access.get(word)).map(String::as_str) == Some(mode))
}

/// `resolveThreadDefaults` (thread-defaults.ts:168): the agent, model, effort and access a new thread starts on.
/// Its stand-in until wsp-words ports thread-defaults.ts; the rule is the TS one, word for word.
fn resolve_thread_defaults(
    first_agent: &str,
    catalog_of: &dyn Fn(&str) -> Option<HarnessCatalog>,
    runs: &dyn Fn(&str) -> bool,
    named: Option<&str>,
    project: Option<&ProjectOverrides>,
    default_agent: Option<&str>,
    agent_defaults: &IndexMap<String, Fields<AgentDefaults>>,
) -> ThreadDefaults {
    let project_agent_name = project.and_then(|p| p.agent.clone());
    let agent = match (named, project_agent_name.as_deref(), default_agent) {
        (Some(named), _, _) => ResolvedPick { value: named.to_string(), from: DefaultFrom::Named },
        (None, Some(agent), _) if runs(agent) => ResolvedPick { value: agent.to_string(), from: DefaultFrom::Project },
        (None, _, Some(agent)) if runs(agent) => ResolvedPick { value: agent.to_string(), from: DefaultFrom::Default },
        _ => ResolvedPick { value: first_agent.to_string(), from: DefaultFrom::Catalog },
    };
    let Some(catalog) = catalog_of(&agent.value) else {
        return ThreadDefaults { agent, model: None, effort: None, access: None };
    };
    let project_agent = project_agent_name.is_none() || project_agent_name.as_deref() == Some(agent.value.as_str());
    let names_this_agent = project_agent_name.as_deref() == Some(agent.value.as_str());
    let own = agent_defaults.get(&agent.value).map(|f| (**f).clone());
    let own_model = own.as_ref().and_then(|o| o.model.clone());
    let own_effort = own.as_ref().and_then(|o| o.effort.clone());
    let own_access = own.as_ref().and_then(|o| o.access);
    let project_model = project.and_then(|p| p.model.clone());
    let project_effort = project.and_then(|p| p.effort.clone());
    let project_access = project.and_then(|p| p.access);
    let models = every_model(&catalog);
    let pick_from = |options: Vec<String>,
                     open: bool,
                     fallback_marked: Option<String>,
                     from_project: Option<String>,
                     from_default: Option<String>|
     -> Option<ResolvedPick> {
        let applies = |value: &str, for_this_agent: bool| if open { for_this_agent } else { options.iter().any(|o| o == value) };
        if let Some(value) = from_project.filter(|value| project_agent && applies(value, names_this_agent)) {
            return Some(ResolvedPick { value, from: DefaultFrom::Project });
        }
        if let Some(value) = from_default.filter(|value| applies(value, true)) {
            return Some(ResolvedPick { value, from: DefaultFrom::Default });
        }
        fallback_marked.map(|value| ResolvedPick { value, from: DefaultFrom::Catalog })
    };
    let model_options: Vec<String> = models.iter().map(|m| m.value.clone()).collect();
    let model = pick_from(model_options, models.is_empty(), marked_default(&models).map(|m| m.value.clone()), project_model, own_model);
    let efforts = efforts_for(&catalog, model_of(&catalog, model.as_ref().map(|m| m.value.as_str())).as_ref());
    let effort_options: Vec<String> = efforts.iter().map(|e| e.value.clone()).collect();
    let effort = pick_from(
        effort_options,
        catalog.efforts.is_empty(),
        marked_default(&efforts).map(|e| e.value.clone()),
        project_effort,
        own_effort,
    );
    let word = |from: DefaultFrom, value: Option<AccessChoice>| -> Option<ResolvedAccess> {
        let value = value?;
        let mode = access_mode(&catalog, value)?.to_string();
        Some(ResolvedAccess { value, mode, from })
    };
    let marked_mode = marked_default(&catalog.permission_modes).map(|o| o.value.clone());
    let access = word(DefaultFrom::Project, project_access)
        .or_else(|| word(DefaultFrom::Default, own_access))
        .or_else(|| word(DefaultFrom::Catalog, marked_mode.as_deref().and_then(|mode| access_word_of(&catalog, mode))));
    ThreadDefaults { agent, model, effort, access }
}

/// `markedFor` (thread-defaults.ts:215): the catalog's marks moved onto what a new thread here starts on.
fn marked_for(
    catalog: &HarnessCatalog,
    model: Option<&ResolvedPick>,
    effort: Option<&ResolvedPick>,
    access: Option<&ResolvedAccess>,
) -> HarnessCatalog {
    let moved = |from: &DefaultFrom| !matches!(from, DefaultFrom::Catalog | DefaultFrom::Named);
    let remark_option = |options: &[HarnessOption], value: &str| -> Vec<HarnessOption> {
        options.iter().map(|o| HarnessOption { is_default: (o.value == value).then_some(true), ..o.clone() }).collect()
    };
    let remark_model = |models: &[HarnessModel], value: &str| -> Vec<HarnessModel> {
        models.iter().map(|m| HarnessModel { is_default: (m.value == value).then_some(true), ..m.clone() }).collect()
    };
    let mut next = catalog.clone();
    if let Some(pick) = model.filter(|p| moved(&p.from)) {
        let in_models = catalog.models.iter().any(|m| m.value == pick.value);
        next.models = if in_models {
            remark_model(&next.models, &pick.value)
        } else {
            next.models.iter().map(|m| HarnessModel { is_default: None, ..m.clone() }).collect()
        };
        if let Some(legacy) = next.legacy_models.clone() {
            next.legacy_models = Some(if in_models { legacy } else { remark_model(&legacy, &pick.value) });
        }
    }
    if let Some(pick) = effort.filter(|p| moved(&p.from)) {
        let takes = |m: &HarnessModel| m.efforts.as_ref().is_none_or(|efforts| efforts.contains(&pick.value));
        let re_mark = |m: &HarnessModel| -> HarnessModel {
            if takes(m) && m.default_effort.is_some() {
                HarnessModel { default_effort: Some(pick.value.clone()), ..m.clone() }
            } else {
                m.clone()
            }
        };
        next.efforts = remark_option(&next.efforts, &pick.value);
        next.models = next.models.iter().map(re_mark).collect();
        if let Some(legacy) = next.legacy_models.clone() {
            next.legacy_models = Some(legacy.iter().map(re_mark).collect());
        }
    }
    if let Some(pick) = access.filter(|p| moved(&p.from)) {
        next.permission_modes = remark_option(&next.permission_modes, &pick.mode);
    }
    next
}

/// `openDefaults` (thread-defaults.ts:246): the model and effort an open list carries, which no mark can.
fn open_defaults(
    catalog: &HarnessCatalog,
    model: Option<&ResolvedPick>,
    effort: Option<&ResolvedPick>,
) -> (Option<String>, Option<String>) {
    let set = |pick: Option<&ResolvedPick>, open: bool| {
        pick.filter(|p| open && matches!(p.from, DefaultFrom::Project | DefaultFrom::Default)).map(|p| p.value.clone())
    };
    (set(model, every_model(catalog).is_empty()), set(effort, catalog.efforts.is_empty()))
}

/// `withCustomModels` (thread-defaults.ts:267): the person's own model ids added after the binary's.
fn with_custom_models(catalog: &HarnessCatalog, picker: Option<&ModelPicker>) -> HarnessCatalog {
    let known: IndexSet<String> = every_model(catalog).into_iter().map(|m| m.value).collect();
    let added: Vec<HarnessModel> = picker
        .and_then(|p| p.custom.as_ref())
        .into_iter()
        .flatten()
        .filter(|id| !known.contains(*id))
        .map(|id| HarnessModel {
            value: id.clone(),
            label: id.clone(),
            description: None,
            is_default: None,
            short: None,
            efforts: None,
            default_effort: None,
            context_windows: None,
            fast: None,
            added: Some(wsp_frames::True),
        })
        .collect();
    if added.is_empty() {
        return catalog.clone();
    }
    let mut next = catalog.clone();
    next.models.extend(added);
    next
}

/// The file-side part of a launch's scopes: each scope's files with the base they are read against.
fn scope(files: Vec<String>, base: String) -> ReadAsk {
    ReadAsk { files, base }
}

/// `{ folder, home, kept }` of the local computer, or of a joined computer's answer (agents.ts:337-349).
impl AgentsArea {
    /// The computer a workspace's agents run on, as the person's setup is keyed (agents.ts:327).
    pub fn setup_place(entry: &Entry) -> Option<String> {
        let live = entry.borrow();
        let record = live.record.borrow();
        if wsp_words::workspace_state::is_local_workspace(Some(&record.kind)) {
            Some(HERE_PLACE_ID.to_string())
        } else {
            record.place.clone()
        }
    }
    /// Whether the person turned this agent off on the computer the workspace stands on (agents.ts:329).
    pub fn agent_off(&self, entry: &Entry, agent: &str) -> bool {
        Self::setup_place(entry).is_some_and(|place| self.ctx.setup_off(&place, agent))
    }
    /// The harness table's label, else the id (agents.ts:333).
    pub fn agent_label(agent: &str) -> String {
        harness_catalog(agent).map_or_else(|| agent.to_string(), |c| c.label)
    }
    /// The config folder a path names on the computer it names (agents.ts:337).
    pub async fn config_folder_on(&self, place: &str, path: &str) -> Result<ResolvedFolder, HostError> {
        if place == HERE_PLACE_ID {
            let home = real_folder_here(&self.ctx.local_home_dir().unwrap_or_else(|| self.ctx.process_home())).await?;
            let mut kept = vec![real_folder_here(&format!("{home}/.wsp")).await?];
            if let Some(state) = self.ctx.state_path() {
                let state = Path::new(&state);
                let absolute = std::path::absolute(state).unwrap_or_else(|_| state.to_path_buf());
                let dir = absolute.parent().map_or_else(|| "/".to_string(), |p| p.to_string_lossy().into_owned());
                kept.push(real_folder_here(&dir).await?);
            }
            let folder = real_folder_here(path).await?;
            return Ok(ResolvedFolder { folder, home, kept });
        }
        let read = self.ctx.place_exec(place, &real_folder_script(path), INLINE_EXEC_MS).await?;
        if read.exit_code.get() == 3 {
            return Err(HostError::with_kind(js_trim(&read.stderr), Kind::Usage));
        }
        let said = js_trim(&read.stdout).to_string();
        let mut lines = said.split('\n');
        let folder = lines.next().map(str::to_string);
        let home = lines.next().map(str::to_string);
        match (folder, home) {
            (Some(folder), Some(home)) if read.exit_code.get() == 0 && home.starts_with('/') => {
                let kept = format!("{home}/.wsp");
                Ok(ResolvedFolder { folder: posix_normalize(&folder), home, kept: vec![kept] })
            }
            _ => {
                let why = if js_trim(&read.stderr).is_empty() {
                    format!("exit {}", read.exit_code.get())
                } else {
                    js_trim(&read.stderr).to_string()
                };
                Err(HostError::with_kind(format!("{} did not say where {path} is: {why}", self.ctx.place_name(place)), Kind::Invalid))
            }
        }
    }
    /// The agent's kept config folder on the computer the workspace stands on, if any (agents.ts:353 `keptFolder`).
    fn kept_folder(&self, entry: &Entry, harness: &str) -> Option<(String, String)> {
        let place = Self::setup_place(entry)?;
        let folder = self.ctx.setup_config_dir(&place, harness)?;
        Some((place, folder))
    }
    /// The one check on a kept config folder, read on its computer and remembered (agents.ts:356-375).
    async fn kept_refusal(&self, place: &str, harness: &str, folder: &str) -> Result<Option<String>, HostError> {
        let real = self.config_folder_on(place, folder).await?;
        let label = Self::agent_label(harness);
        let refused = config_dir_refusal(&label, folder, &real).map(|why| config_dir_launch_refusal(&label, harness, folder, &why));
        let key = key_of(place, harness);
        let was = match &refused {
            None => self.setup_refusals.borrow_mut().shift_remove(&key),
            Some(words) => self.setup_refusals.borrow_mut().insert(key, words.clone()),
        };
        if refused != was {
            self.ctx.setup_refusal_moved(place, harness);
        }
        Ok(refused)
    }
    /// The refusal for the agent's kept config folder on the workspace's computer, `None` where it stands
    /// (agents.ts:362-366).
    pub async fn setup_refusal(&self, entry: &Entry, named: Option<&str>) -> Result<Option<String>, HostError> {
        let harness = named.map_or_else(|| default_agent().id.clone(), str::to_string);
        match self.kept_folder(entry, &harness) {
            None => Ok(None),
            Some((place, folder)) => self.kept_refusal(&place, &harness, &folder).await,
        }
    }
    /// Each agent's home on this computer, refusing a kept folder that leads out (agents.ts:376-388).
    pub async fn homes_here(&self) -> Result<IndexMap<String, String>, HostError> {
        self.ctx.ready().await?;
        let mut homes = IndexMap::new();
        for agent in catalog_agents() {
            let kept = self.ctx.setup_config_dir(HERE_PLACE_ID, &agent.id);
            if let Some(folder) = kept.as_deref() {
                if let Some(refused) = self.kept_refusal(HERE_PLACE_ID, &agent.id, folder).await? {
                    return Err(HostError::with_kind(refused, Kind::Usage));
                }
            }
            let home = match kept.or_else(|| self.ctx.local_home(&agent.id)) {
                Some(home) => home,
                None => agent_homes(&self.ctx.process_home(), None)
                    .get(&agent.id)
                    .cloned()
                    .ok_or_else(|| HostError::new(format!("no home for {}", agent.id)))?,
            };
            homes.insert(agent.id.clone(), home);
        }
        Ok(homes)
    }
    /// The agents whose store on this computer is not their own under the local home (agents.ts:390-394).
    pub async fn stores_here(&self) -> Result<IndexMap<String, String>, HostError> {
        let homes = self.homes_here().await?;
        let home = self.ctx.local_home_dir().unwrap_or_else(|| self.ctx.process_home());
        Ok(catalog_agents()
            .into_iter()
            .filter_map(|a| {
                let here = homes.get(&a.id)?;
                (*here != format!("{home}/{}", a.state_home)).then(|| (a.id.clone(), here.clone()))
            })
            .collect())
    }
    /// Fails where the kept config folder leads out of the home; settles at once where none is kept (agents.ts:396).
    pub async fn confine_setup(&self, entry: &Entry, named: Option<&str>) -> Result<(), HostError> {
        let harness = named.map_or_else(|| default_agent().id.clone(), str::to_string);
        if self.kept_folder(entry, &harness).is_none() {
            return Ok(());
        }
        match self.setup_refusal(entry, named).await? {
            Some(refused) => Err(HostError::with_kind(refused, Kind::Usage)),
            None => Ok(()),
        }
    }
    /// The adapter for a launch that starts the agent: its folder checked, then the broker read, then the adapter
    /// (agents.ts:403-407).
    pub async fn launch_adapter_for(&self, entry: &Entry, ask: AdapterAsk) -> Result<BuiltAdapter, HostError> {
        self.confine_setup(entry, ask.named.as_deref()).await?;
        let carries_token = ask.turn_env.as_ref().is_some_and(|env| env.contains_key(HOST_TOKEN_ENV));
        if !carries_token {
            let harness = ask.named.clone().unwrap_or_else(|| default_agent().id.clone());
            self.ctx.know_broker(entry, &harness).await?;
        }
        self.adapter_for(entry, ask)
    }
    /// The agent a new thread runs when its start names none (agents.ts:412-416).
    pub fn default_agent_of(&self, prefs: &Preferences, entry: Option<&Entry>) -> String {
        let runs = |id: &str| self.ctx.adapters().contains_key(id) && entry.is_none_or(|e| !self.agent_off(e, id));
        let first = catalog_agents().into_iter().find(|a| runs(&a.id)).map_or_else(|| default_agent().id.clone(), |a| a.id.clone());
        let project = entry.and_then(|e| {
            let project = e.borrow().record.borrow().project.clone();
            prefs.project_defaults.get(&project).map(|f| (**f).clone())
        });
        let catalog_of = |id: &str| harness_catalog(id);
        resolve_thread_defaults(&first, &catalog_of, &runs, None, project.as_ref(), prefs.default_agent.as_deref(), &prefs.agent_defaults)
            .agent
            .value
    }
    /// One agent's lists with the person's models in and the marks on what a new thread starts on (agents.ts:421-425).
    pub fn defaults_on(catalog: &HarnessCatalog, prefs: &Preferences, project: Option<&ProjectOverrides>) -> DefaultsOn {
        let lists = with_custom_models(catalog, prefs.agent_defaults.get(&catalog.harness).and_then(|f| f.models.as_ref()));
        let harness = catalog.harness.clone();
        let served = lists.clone();
        let catalog_of = move |_: &str| Some(served.clone());
        let always: &dyn Fn(&str) -> bool = &|_: &str| true;
        let defaults = resolve_thread_defaults(
            &harness,
            &catalog_of,
            always,
            Some(&harness),
            project,
            prefs.default_agent.as_deref(),
            &prefs.agent_defaults,
        );
        let (open_model, open_effort) = open_defaults(&lists, defaults.model.as_ref(), defaults.effort.as_ref());
        DefaultsOn {
            catalog: marked_for(&lists, defaults.model.as_ref(), defaults.effort.as_ref(), defaults.access.as_ref()),
            open_model,
            open_effort,
        }
    }
    /// The harness's own mode for the word a start named, or the refusal naming the words it takes (agents.ts:428).
    pub fn named_mode(catalog: Option<&HarnessCatalog>, harness: &str, word: AccessChoice) -> Result<String, HostError> {
        if let Some(mode) = catalog.and_then(|c| access_mode(c, word)) {
            return Ok(mode.to_string());
        }
        let label = Self::agent_label(harness);
        Err(HostError::with_kind(access_refusal_line(&label, catalog, word), Kind::Usage))
    }
    /// The turn's images on the road its adapter declared (agents.ts:442-456).
    pub async fn land_images(
        &self,
        entry: &Entry,
        road: Option<AttachmentRoad>,
        folder: &str,
        dir: &str,
        images: &[Attachment],
    ) -> Result<LandedImages, HostError> {
        let Some(road) = road.filter(|_| !images.is_empty()) else {
            return Ok(LandedImages::default());
        };
        if matches!(road, AttachmentRoad::Inline) {
            let images = images
                .iter()
                .map(|image| TurnImage { media_type: image.media_type.clone(), bytes: image.bytes.clone(), path: None })
                .collect();
            return Ok(LandedImages { images, dir: None });
        }
        let machine = entry.borrow().machine.clone();
        self.land_folder(&machine, folder, dir).await?;
        let mut landed = Vec::new();
        for (index, image) in images.iter().enumerate() {
            let path = image_path_in(dir, index as f64, &image.media_type)?;
            land_bytes(
                machine.as_ref(),
                &path,
                &wsp_store::keys::node_base64(&image.bytes),
                LandBytesOptions { fetch: None, timeout_ms: None },
            )
            .await
            .map_err(engine)?;
            landed.push(TurnImage { media_type: image.media_type.clone(), bytes: image.bytes.clone(), path: Some(path) });
        }
        Ok(LandedImages { images: landed, dir: Some(dir.to_string()) })
    }
    /// Readies the send's own folder in one command under the inline exec's limit (agents.ts:442, 464).
    async fn land_folder(&self, machine: &Arc<dyn Machine>, folder: &str, dir: &str) -> Result<(), HostError> {
        let ready = machine
            .exec(&land_files_line(folder, dir), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), idempotency_key: None, stdin: None })
            .await
            .map_err(engine)?;
        if ready.exit_code.get() != 0 {
            return Err(HostError::new(files_not_landed_line(folder)));
        }
        Ok(())
    }
    /// The turn's files that are not images, landed in the folder the thread works in (agents.ts:464-476).
    pub async fn land_files(&self, entry: &Entry, folder: &str, dir: &str, files: &[Attachment]) -> Result<Vec<String>, HostError> {
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let machine = entry.borrow().machine.clone();
        self.land_folder(&machine, folder, dir).await?;
        let mut taken: IndexSet<String> = IndexSet::new();
        let mut paths = Vec::new();
        for file in files {
            let path = file_path_in(dir, file.name.as_deref(), &mut taken);
            land_bytes(
                machine.as_ref(),
                &path,
                &wsp_store::keys::node_base64(&file.bytes),
                LandBytesOptions { fetch: None, timeout_ms: None },
            )
            .await
            .map_err(engine)?;
            paths.push(path);
        }
        Ok(paths)
    }
    /// Takes these threads' attached files off the folders their turns ran in (agents.ts:481-492).
    pub async fn drop_thread_files(&self, entry: &Entry, threads: &[String]) {
        if entry.borrow().record.borrow().phase != WorkspacePhase::Running {
            return;
        }
        let records = self.ctx.thread_records();
        let mut by_folder: IndexMap<String, Vec<String>> = IndexMap::new();
        for thread in threads {
            let folders = records.borrow().get(thread).and_then(|record| record.files_in.clone()).unwrap_or_default();
            for folder in folders {
                by_folder.entry(folder).or_default().push(thread.clone());
            }
        }
        let machine = entry.borrow().machine.clone();
        for (folder, ids) in by_folder {
            let line = drop_files_line(&folder, &ids);
            if let Err(e) = machine.exec(&line, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), idempotency_key: None, stdin: None }).await
            {
                wsp_core::console::warn(&format!("attached files of {} not removed from {folder}: {}", ids.join(", "), e));
            }
        }
    }
    /// Takes one send's images off the machine once its turn is over (agents.ts:498-502).
    pub fn drop_images(&self, entry: &Entry, dir: &str) {
        let machine = entry.borrow().machine.clone();
        let id = entry.borrow().record.borrow().id.clone();
        let command = format!("rm -rf {}", shell_quote(dir));
        self.fire(
            async move {
                if let Err(e) =
                    machine.exec(&command, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), idempotency_key: None, stdin: None }).await
                {
                    wsp_core::console::warn(&format!("images for a finished turn not removed from {id}: {e}"));
                }
            }
            .boxed_local(),
        );
    }
    /// Where one agent keeps its store on this workspace (agents.ts:506).
    pub fn agent_home(&self, entry: &Entry, agent: &str) -> String {
        agent_home_on(self.ctx.as_ref(), entry, agent)
    }
    /// The environment a thread's processes start under (agents.ts:514-518).
    pub fn thread_env(&self, entry: &Entry, harness: Option<&str>) -> Env {
        let home = |id: &str| agent_home_on(self.ctx.as_ref(), entry, id);
        let mut env = self.ctx.kind_env(entry, harness, &home);
        if let Some(place) = Self::setup_place(entry) {
            let ids: Vec<String> = match harness {
                Some(agent) => vec![agent.to_string()],
                None => catalog_agents().into_iter().map(|a| a.id.clone()).collect(),
            };
            for id in ids {
                for (name, value) in self.ctx.setup_launch(&place, &id).env {
                    env.insert(name, value);
                }
            }
        }
        env
    }
    /// The folder each agent's store variable names on a joined computer (agents.ts:524-530).
    pub fn place_stores(&self, place: &str, home: &str) -> IndexMap<String, String> {
        let mut stores = IndexMap::new();
        for agent in catalog_agents() {
            let Some(variable) = agent.state_home_env.as_deref() else {
                continue;
            };
            let value = self
                .ctx
                .setup_launch(place, &agent.id)
                .env
                .get(variable)
                .cloned()
                .or_else(|| self.ctx.setup_config_dir(place, &agent.id))
                .unwrap_or_else(|| self.ctx.place_agent_home(place, home, &agent.id));
            stores.insert(agent.id.clone(), value);
        }
        stores
    }
    /// The agent's config files a launch in `folder` reads, as the turn's login (agents.ts:534-555).
    pub async fn turn_configs_on(&self, entry: &Entry, harness: &str, folder: &str) -> Result<Option<TurnConfigs>, HostError> {
        let Some(road) = launch_server_road(harness) else {
            return Ok(None);
        };
        let Some(mcp) = mcp_agents().into_iter().find(|a| a.id == harness).and_then(|a| a.mcp.clone()) else {
            return Ok(None);
        };
        let env = self.thread_env(entry, Some(harness));
        let home = env.get("HOME").cloned().unwrap_or_else(|| "~".to_string());
        let store_variable = catalog_agents().into_iter().find(|a| a.id == harness).and_then(|a| a.state_home_env.clone());
        let store = store_variable.and_then(|variable| env.get(&variable).cloned());
        let machine = entry.borrow().machine.clone();
        let quoted: Vec<String> = checkout_args(folder).iter().map(|arg| shell_quote(arg)).collect();
        let said = machine
            .exec(
                &format!("git {} 2>/dev/null", quoted.join(" ")),
                ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), idempotency_key: None, stdin: None },
            )
            .await
            .ok();
        let checkout = checkout_of(said.as_ref().filter(|s| s.exit_code.get() == 0).map(|s| s.stdout.as_str()), folder);
        let turn =
            TurnAgent { id: harness.to_string(), format: mcp.format, files: mcp.files.clone(), project_files: mcp.project_files.clone() };
        let files = turn_server_files(&turn, folder, &home, store.as_deref(), checkout.top.as_deref());
        let top = checkout.top.clone();
        let mut scopes = vec![scope(files.user.clone(), files.base.clone())];
        for project in files.projects {
            let base = match top.as_deref() {
                Some(top) if under_project(&project, top) => top.to_string(),
                _ => posix_dirname(&project),
            };
            scopes.push(scope(vec![project], base));
        }
        let res = machine
            .exec(&read_configs_cmd(&scopes), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), idempotency_key: None, stdin: None })
            .await
            .map_err(engine)?;
        let name = catalog_agents().into_iter().find(|a| a.id == harness).map_or_else(|| harness.to_string(), |a| a.name.clone());
        if res.exit_code.get() == 0 && !read_whole(&res.stdout) {
            let files: Vec<String> = scopes.iter().flat_map(|s| s.files.clone()).collect();
            return Err(HostError::new(server_values_cut_line(&name, &files)));
        }
        let read = if res.exit_code.get() == 0 { parse_configs(&res.stdout, &scopes) } else { None };
        let Some(read) = read else {
            return Err(HostError::new(server_values_unread_line(&name, &read_reason(&res, Some(INLINE_EXEC_MS as f64 / 1000.0)))));
        };
        if let Some(ScopeRead::Unread { path, unread }) = read.iter().flatten().find(|f| !matches!(f, ScopeRead::File(_))) {
            return Err(HostError::new(server_values_left_line(&name, &config_unread_line(path.as_str(), unread.as_str()))));
        }
        let mut reads = read.into_iter();
        let user = match reads.next().flatten() {
            Some(ScopeRead::File(file)) => Some(turn_file(file)),
            _ => None,
        };
        let projects: Vec<TurnFile> = reads
            .flatten()
            .filter_map(|f| match f {
                ScopeRead::File(file) => Some(turn_file(file)),
                ScopeRead::Unread { .. } => None,
            })
            .collect();
        Ok(Some(TurnConfigs {
            road,
            configs: LaunchConfigs { user, projects: Some(projects), folder: folder.to_string(), key: Some(checkout.key) },
        }))
    }
    /// The vault's values for the agent's own MCP servers, for a launch that hands them on the kind (agents.ts:561-568).
    pub async fn server_values_for(&self, entry: &Entry, harness: &str, folder: &str) -> Result<Option<ServerValues>, HostError> {
        let values = server_values_of(&self.ctx.vault());
        if self.ctx.kind_server_values(entry) != Some(wsp_core::types::internal::ServerValuesRoad::Launch) || values.is_empty() {
            return Ok(None);
        }
        let Some(read) = self.turn_configs_on(entry, harness, folder).await? else {
            return Ok(None);
        };
        let filled = read.road.fill(&read.configs, &values);
        let entries = filled.entries.filter(|e| !e.is_empty()).map(|entries| {
            entries
                .into_iter()
                .map(|(name, value)| {
                    let object: IndexMap<String, Value> = match value {
                        Value::Object(map) => map.into_iter().collect(),
                        _ => IndexMap::new(),
                    };
                    (name, object)
                })
                .collect::<IndexMap<String, IndexMap<String, Value>>>()
        });
        let config = filled.config.filter(|c| !c.is_empty());
        if entries.is_none() && config.is_none() {
            return Ok(None);
        }
        Ok(Some(ServerValues { entries, config }))
    }
    /// The servers a launch in `folder` on a cloud machine leaves out, read once per workspace, agent and folder
    /// (agents.ts:592-611).
    pub async fn off_servers_for(&self, entry: &Entry, harness: &str, folder: &str) -> Option<Vec<String>> {
        let record = entry.borrow().record.borrow().clone();
        let place = self.ctx.place_id_of(&record);
        if !self.ctx.at_provider(place.as_deref()) {
            return None;
        }
        let key = [record.id.as_str(), harness, folder].join("\0");
        let existing = self.cloud_servers.borrow().get(&key).cloned();
        let reading = match existing {
            Some(reading) => reading,
            None => {
                let area = self.me.upgrade()?;
                let (entry_c, harness_c, folder_c) = (entry.clone(), harness.to_string(), folder.to_string());
                let reading = async move {
                    match area.turn_configs_on(&entry_c, &harness_c, &folder_c).await {
                        Ok(read) => read.map(|read| read.road.servers(&read.configs)),
                        Err(e) => {
                            let machine = entry_c.borrow().machine.id().to_string();
                            wsp_core::console::warn(&format!(
                                "{harness_c} on {machine}: its servers that need a sign-in were not read ({}); the launch leaves none out",
                                e.message
                            ));
                            None
                        }
                    }
                }
                .boxed_local()
                .shared();
                self.cloud_servers.borrow_mut().insert(key.clone(), reading.clone());
                reading
            }
        };
        let Some(servers) = reading.await else {
            self.cloud_servers.borrow_mut().shift_remove(&key);
            return None;
        };
        let asked: Vec<McpServer> = servers.iter().map(|s| s.server.clone()).collect();
        let fallback: SignsIn = Rc::new(|_: &str| None);
        let signs_in = self.ctx.signs_in(addresses_to_ask(&asked)).await.unwrap_or(fallback);
        let mut off: IndexSet<String> = IndexSet::new();
        for server in &servers {
            if needs_own_credential(&server.server, signs_in.as_ref()) {
                off.insert(server.server.name.clone());
            }
        }
        Some(off.into_iter().collect())
    }
    /// What a launch hands the adapter of the agent's own servers, as far as the adapter takes either (agents.ts:615-619).
    pub async fn launch_servers_for(
        &self,
        entry: &Entry,
        harness: &str,
        adapter: &dyn HarnessAdapter,
        folder: &str,
    ) -> Result<LaunchServers, HostError> {
        let server_values = if adapter.mcp_servers() { self.server_values_for(entry, harness, folder).await? } else { None };
        let off_servers = if adapter.denies_servers() { self.off_servers_for(entry, harness, folder).await } else { None };
        Ok(LaunchServers { server_values, off_servers })
    }
    /// The adapter for a harness on this workspace's machine (agents.ts:632-667).
    pub fn adapter_for(&self, entry: &Entry, ask: AdapterAsk) -> Result<BuiltAdapter, HostError> {
        let harness = ask.named.clone().unwrap_or_else(|| default_agent().id.clone());
        let adapters = self.ctx.adapters();
        let Some(factory) = adapters.get(&harness).cloned() else {
            let agents: Vec<String> = adapters.keys().cloned().collect();
            return Err(HostError::new(wsp_words::words::thread::no_adapter_line(&harness, &agents)));
        };
        let token = ask.turn_env.as_ref().and_then(|env| env.get(HOST_TOKEN_ENV)).cloned();
        let brokered: Option<BrokeredLaunch> = self.ctx.brokered_launch(entry, &harness, token.as_deref());
        let vault = match brokered.as_ref() {
            Some(brokered) => brokered.vault.clone(),
            None => self.ctx.vault_on(entry),
        };
        let place = Self::setup_place(entry);
        let setup = place.as_ref().map(|place| self.ctx.setup_launch(place, &harness));
        let server_values = self.ctx.kind_server_values(entry);
        let mut env: Env = if brokered.is_none() && server_values == Some(wsp_core::types::internal::ServerValuesRoad::Environment) {
            ask.servers.clone()
        } else {
            Env::new()
        };
        for (name, value) in self.thread_env(entry, Some(&harness)) {
            env.insert(name, value);
        }
        if let Some(brokered) = brokered.as_ref() {
            for (name, value) in &brokered.env {
                env.insert(name.clone(), value.clone());
            }
        }
        for (name, value) in ask.turn_env.clone().unwrap_or_default() {
            env.insert(name, value);
        }
        let turn = TurnLaunch {
            thread: if ask.thread.is_some() && self.ctx.kind_ends_threads(entry) { ask.thread.clone() } else { None },
            asks_until_stopped: ask.asks_until_stopped,
            killed_why: entry.borrow().record.borrow().sub.is_some(),
        };
        let exec_stream = self.ctx.exec_factory_for(entry, turn, ask.waiting.clone());
        let machine = entry.borrow().machine.clone();
        let workspace_id = entry.borrow().record.borrow().id.clone();
        let kind_record = entry.borrow().record.borrow().kind;
        let context = HarnessAdapterContext {
            machine,
            workspace_id,
            exec_stream,
            home: {
                let ctx = self.ctx.clone();
                let entry = entry.clone();
                Arc::new(move |id: &str| agent_home_on(ctx.as_ref(), &entry, id))
            },
            env,
            project_key: self.ctx.kind_memory_key(entry, &harness),
            sign_in_refusal: sign_in_refusal_line(Some(&kind_record)),
            vault,
            brokered: brokered.as_ref().and_then(|b| b.claude.clone()),
            codex_broker: brokered.as_ref().and_then(|b| b.codex.clone()),
            login_stands: {
                let ctx = self.ctx.clone();
                let entry = entry.clone();
                Arc::new(move |id: &str| ctx.kind_login_stands(&entry, id))
            },
            launch: setup.and_then(|s| s.launch),
        };
        let adapter = factory(context)?;
        Ok(BuiltAdapter { harness, adapter })
    }
    /// The record a new thread on a project runs on, where it runs in the project's folder (agents.ts:671-675).
    pub async fn project_home(&self, reference: &str, origin: Option<Caller>) -> Result<Option<Entry>, HostError> {
        let project = self.ctx.resolve_project(reference, origin).await?;
        if !self.ctx.computer_runs_in_folder(&project.computer) {
            return Ok(None);
        }
        Ok(self.ctx.project_folder(&project).await.ok())
    }
    /// The record a computer's agents are read through (agents.ts:678-681).
    pub async fn computer_home(&self, computer: &str, origin: Option<Caller>) -> Result<Option<Entry>, HostError> {
        let projects = self.ctx.list_projects(origin.clone()).await?;
        let Some(project) = projects.into_iter().find(|p| p.computer == computer && self.ctx.computer_runs_in_folder(&p.computer)) else {
            return Ok(None);
        };
        self.project_home(&project.id, origin).await
    }
}

/// `agentHome` (agents.ts:506): the kept config folder on the entry's place, else the kind's home.
fn agent_home_on(ctx: &dyn AgentsCtx, entry: &Entry, agent: &str) -> String {
    let kept = AgentsArea::setup_place(entry).and_then(|place| ctx.setup_config_dir(&place, agent));
    kept.unwrap_or_else(|| ctx.kind_home(entry, agent))
}

/// The config file a scope read carries into a turn (agents.ts:548-553).
fn turn_file(file: ScopeFile) -> TurnFile {
    TurnFile { path: file.path, text: file.text }
}
