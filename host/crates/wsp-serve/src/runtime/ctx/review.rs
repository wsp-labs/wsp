// SPDX-License-Identifier: AGPL-3.0-only
//! The context's review and projects readings (workspaces.ts:1055-1247, projects.ts, project-landing.ts, images.ts and
//! the `ctx.` members they call): most of them one call on `opts`, the core's maps or the area that holds the rule.
//! The landing road and the project source the add reads ride over wsp-core's, with the deps the area keeps as data
//! turned into the closures the road reads (projects.ts:124-150).
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{Map, Value};
use wsp_account::account::channels::{GUEST_ROAD_EXTRA, WORKSPACE_FRAMES};
use wsp_account::account::core_types::AccountError;
use wsp_account::account::pull_requests::PullRequestsArea;
use wsp_account::projects::projects::add::{
    JoinedComputer, LandAsk, Landed, LandingDeps as AccountDeps, LandingKind, LandingRoad as AccountRoad,
    ProjectSourceModule as AccountSource, ResolveAsk, SourceResolved,
};
use wsp_account::projects::projects::moves::{LandHooks, MoveRoads, Progress};
use wsp_account::projects::start_from::{from_of, OpenAsk, Placed, StartKind};
use wsp_account::projects::workspaces::seam::{record_of, Entry};
use wsp_core::project_landing::{project_landing, LandRequest, LandSeed, LandingComputer, LandingDeps, ProjectLanding};
use wsp_core::project_sources::{project_source, FolderRemote, ProjectSourceModule, SourceDeps};
use wsp_core::types::api::DaemonEventListener;
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::internal::{ImportLanded, ImportReport};
use wsp_core::types::pending::{CacheRule, DaemonChannel};
use wsp_core::types::wiring::{Detach, ProjectImportOptions, SeedWiring, SysListener, WorkspaceRecord};
use wsp_engine::fetch::platform_fetch_manual;
use wsp_engine::machine::Machine;
use wsp_engine::project_state::{agents_on_machine, guest_agent_homes};
use wsp_engine::run_env::GITHUB_TOKEN_ENV;
use wsp_engine::vault::{
    export_folder, export_paths_into, land_bundle, ArchiveGroup, DownloadProgress, LandOptions, UploadProgress, VaultOptions,
};
use wsp_wire::types::pull_request::PullRequest;
use wsp_wire::types::start::{IssueRead, WorkspaceFrom, WorkspaceFromKind};
use wsp_wire::types::thread_defaults::ThreadDefaults;
use wsp_wire::views::harness::{HarnessCatalog, StartResult};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::project_bundle::ProjectAgentResult;
use wsp_wire::views::workspace::{scope_of, Caller, DaemonReachView, PortProbeView, PortReachView, ProjectSource, ProjectView};
use wsp_wire::views::workspace_more::ProjectBranch;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;
use wsp_words::workspace_state::{is_local_workspace, machine_word, reading_road, KindReading, ReadingRoad};

use super::Ctx;
use crate::runtime::join::spawn_detached;
use crate::runtime::probe::probe_port;

fn host_of(e: AccountError) -> HostError {
    e.into()
}

/// A daemon event as the JSON the channels area reads.
fn heard_of(listener: DaemonEventListener) -> Rc<dyn Fn(Value)> {
    Rc::new(move |event: Value| {
        if let Value::Object(map) = event {
            listener(map);
        }
    })
}

/// The entry's kind, read apart from its borrow.
fn kind_of(entry: &Entry) -> wsp_wire::views::workspace_kind::WorkspaceKind {
    entry.borrow().record.borrow().kind
}

/// The engine's upload progress as the move roads' (`part` and `parts` count the parts of an upload).
fn progress_of(p: UploadProgress) -> Progress {
    Progress { part: u64::from(p.part), parts: u64::from(p.parts), bytes: p.bytes, total: p.total }
}

fn upload_of(hooks: LandHooks) -> (Arc<dyn Fn(UploadProgress)>, Option<Arc<dyn Fn()>>, String, u64) {
    let on_part = hooks.on_part.clone();
    let part: Arc<dyn Fn(UploadProgress)> = Arc::new(move |p| on_part(progress_of(p)));
    let landing = hooks.on_landing.clone().map(|f| -> Arc<dyn Fn()> { Arc::new(move || f()) });
    (part, landing, hooks.tmp_dir, hooks.timeout_ms)
}

#[async_trait(?Send)]
impl wsp_account::projects::workspaces::review::ReviewCtx for Ctx {
    fn project_by_repo(&self, repo: &str, named: Option<&str>) -> Result<ProjectView, HostError> {
        self.start_from.get().project_by_repo(repo, named)
    }
    async fn issue_of(&self, remote: &str, number: i64) -> Result<IssueRead, HostError> {
        self.start_from.get().issue_of(remote, number).await
    }
    async fn pull_request_of(&self, remote: &str, number: i64) -> Result<PullRequest, HostError> {
        self.start_from.get().pull_request_of(remote, number).await
    }
    fn from_of(&self, kind: WorkspaceFromKind, repo: &str, read: &IssueRead, fact: Option<&PullRequest>) -> WorkspaceFrom {
        from_of(kind, repo, read, fact)
    }
    async fn picks_hold(&self, project: &ProjectView, picks: &StartPicksAsked) -> Result<(), HostError> {
        self.start_from.get().picks_hold(project, picks).await
    }
    async fn workspace_from(
        &self,
        project: &ProjectView,
        from: WorkspaceFrom,
        fact: Option<PullRequest>,
        kind: StartKind,
        origin: Option<Caller>,
    ) -> Result<Placed, HostError> {
        self.start_from.get().workspace_from(project, from, fact, kind, origin).await
    }
    async fn open_with(&self, placed: &Placed, o: OpenAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
        self.start_from.get().open_with(placed, o, origin).await
    }
    fn read_only_of(&self, agent: &str) -> Option<String> {
        self.start_from.get().read_only_of(agent)
    }
    fn reviewers(&self) -> Vec<String> {
        self.start_from.get().reviewers()
    }
    fn drop_page_cache(&self, remote: &str, number: i64) {
        self.github_cache.delete(&PullRequestsArea::page_key(remote, number));
    }
    fn reread_pull_request(&self, entry: &Entry) {
        let area = self.pull_requests.get();
        let entry = entry.clone();
        spawn_detached(async move {
            let _ = area.read_pull_request(&entry, true, false).await;
        });
    }
    fn label_of(&self, entry: &Entry, thread_id: Option<&str>) -> String {
        wsp_core::types::internal::label_of(&record_of(entry).name, thread_id)
    }
    async fn daemon_road(&self, entry: &Entry) -> Result<DaemonReachView, HostError> {
        let module = self.kinds.get().module_of(kind_of(entry))?;
        let road = module.daemon_road(&entry.borrow());
        road.await
    }
    fn forget_daemon_token(&self, machine_id: &str) {
        self.transcripts.get().daemon_tokens.borrow_mut().shift_remove(machine_id);
    }
    fn shared_daemon(&self, entry: &Entry) -> bool {
        self.kinds.get().module_of(kind_of(entry)).is_ok_and(|module| module.shared_daemon())
    }
    async fn copy_channel(&self, entry: &Entry, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        let channels = self.channels.get();
        let id = record_of(entry).id;
        let opened = channels.copy_channel(entry, heard_of(on_event), &WORKSPACE_FRAMES).await.map_err(host_of)?;
        Ok(channels.thread_tagged(&id, opened))
    }
    async fn shared_channel(&self, entry: &Entry, heard: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        let channels = self.channels.get();
        let record = record_of(entry);
        let id = record.id.clone();
        // The area keeps each workspace's pty pids by value, so the prune a `pty.exit` makes is made here, on the map
        // the rooted channel fills, and `port_pids` answers a map nothing else reads.
        let pruned = {
            let channels = channels.clone();
            let id = id.clone();
            Rc::new(move |event: &Map<String, Value>| {
                if event.get("type").and_then(Value::as_str) != Some("pty.exit") {
                    return;
                }
                let pty = event.get("ptyId").and_then(Value::as_str).unwrap_or_default().to_string();
                let had = channels.port_pids.borrow_mut().get_mut(&id).is_some_and(|ptys| ptys.shift_remove(&pty).is_some());
                if had {
                    channels.port_roots_moved(&id);
                }
            })
        };
        let listener: DaemonEventListener = Rc::new(move |event: Map<String, Value>| {
            pruned(&event);
            heard(event);
        });
        // `ctx.checkoutOf` is an argument ahead of the channel's own, so it refuses before a channel is opened.
        let folder = channels.checkout_of(&record)?;
        channels.port_pids.borrow_mut().entry(id.clone()).or_default();
        let opened = channels.copy_channel(entry, heard_of(listener), &WORKSPACE_FRAMES).await.map_err(host_of)?;
        let owned = channels.held_to_owner(&id, channels.thread_tagged(&id, opened));
        let rooted = channels.rooted_ports(&id, &folder, owned);
        let scopes = self.proc_scopes.get();
        let here = scopes.clone();
        let procs = scopes.scoped_procs(Rc::new(move || here.scopes_here()), rooted);
        Ok(channels.kill_guarded(procs, is_local_workspace(Some(&record.kind))))
    }
    fn port_pids(&self, _workspace_id: &str) -> Rc<RefCell<IndexMap<String, i64>>> {
        Rc::default()
    }
    fn port_roots_moved(&self, workspace_id: &str) {
        self.channels.get().port_roots_moved(workspace_id);
    }
    async fn guest_road_channel(&self, entry: &Entry, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        let frames: Vec<&str> = WORKSPACE_FRAMES.iter().chain(GUEST_ROAD_EXTRA.iter()).copied().collect();
        self.channels.get().copy_channel(entry, heard_of(on_event), &frames).await.map_err(host_of)
    }
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String> {
        let channels = self.channels.get();
        channels.served_by_its_computer(entry).map(|_| channels.computer_of(entry))
    }
    fn metrics_on_host(&self, entry: &Entry) -> bool {
        reading_road(&kind_of(entry), &KindReading::Metrics) == ReadingRoad::Host
    }
    fn machine_word(&self, entry: &Entry) -> String {
        machine_word(&kind_of(entry))
    }
    fn sys_samples(&self, listener: SysListener) -> Option<Detach> {
        let sample = self.opts.local.as_ref()?.sys_samples.clone()?;
        let attached: Rc<RefCell<Option<Detach>>> = Rc::default();
        let detached = Rc::new(std::cell::Cell::new(false));
        let started = sample(listener);
        {
            let (attached, detached) = (attached.clone(), detached.clone());
            spawn_detached(async move {
                if let Ok(detach) = started.await {
                    if detached.get() {
                        detach();
                    } else {
                        *attached.borrow_mut() = Some(detach);
                    }
                }
            });
        }
        Some(Box::new(move || {
            detached.set(true);
            if let Some(detach) = attached.borrow_mut().take() {
                detach();
            }
        }))
    }
    async fn kind_port_reach(&self, entry: &Entry, port: u16, standing: bool) -> Option<Result<PortReachView, HostError>> {
        let module = match self.kinds.get().module_of(kind_of(entry)) {
            Ok(module) => module,
            Err(e) => return Some(Err(e)),
        };
        let route = module.port_reach(&entry.borrow(), port, standing)?;
        Some(route.await.map(|route| PortReachView { url: route.url, expires_at: route.expires_at }))
    }
    async fn ws_port_reach(&self, entry: &Entry, port: u16) -> Result<PortReachView, HostError> {
        let ws = entry.borrow().ws.clone();
        let reach = ws.port_reach(port).await.map_err(|e| HostError::new(e.to_string()))?;
        Ok(PortReachView { url: reach.url, expires_at: reach.expires_at })
    }
    async fn ws_remint_port_reach(&self, entry: &Entry, port: u16) -> Result<PortReachView, HostError> {
        let ws = entry.borrow().ws.clone();
        let reach = ws.remint_port_reach(port).await.map_err(|e| HostError::new(e.to_string()))?;
        Ok(PortReachView { url: reach.url, expires_at: reach.expires_at })
    }
    async fn probe(&self, url: &str) -> Result<PortProbeView, HostError> {
        probe_port(&*platform_fetch_manual(), url).await
    }
    fn refusal_for(&self, record: Option<&WorkspaceRecord>, origin: Option<&Caller>) -> Result<Option<String>, HostError> {
        let like = record.map(wsp_account::account::rules::like_of);
        self.rules.get().refusal_for(like.as_ref(), origin)
    }
    fn reaches_row(&self, thread_id: &str, workspace_id: &str, origin: Option<&Caller>) -> Result<bool, HostError> {
        let row =
            wsp_machines::machines::boot::entries::RowAt { thread_id: Some(thread_id.to_string()), workspace_id: workspace_id.to_string() };
        self.boot.get().reaches_row(&row, origin, None)
    }
}

#[async_trait(?Send)]
impl wsp_account::projects::projects::ProjectsCtx for Ctx {
    fn state_folder(&self) -> Result<String, HostError> {
        self.kinds.get().state_folder()
    }
    fn has_state_path(&self) -> bool {
        self.opts.state_path.is_some()
    }
    fn seed_wiring(&self) -> Option<Rc<dyn SeedWiring>> {
        self.opts.seed.clone()
    }
    fn cloud_projects(&self) -> Option<bool> {
        self.opts.cloud_projects
    }
    fn vault_github_token(&self) -> Option<String> {
        let vault = self.opts.vault.as_ref()?;
        vault().get(GITHUB_TOKEN_ENV).cloned()
    }
    fn local_claude_home(&self) -> Option<String> {
        self.opts.local.as_ref().map(|local| (local.home)("claude"))
    }
    async fn homes_here(&self) -> Result<IndexMap<String, String>, HostError> {
        self.agents.get().homes_here().await
    }
    fn refuse_recording(&self, word: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        self.rules.get().refuse_recording(word, origin)
    }
    async fn computer_rows(&self) -> Result<Vec<wsp_account::projects::projects::add::ComputerRow>, HostError> {
        Ok(self.boot.get().computer_rows().await)
    }
    fn name_of_computer(&self, id: &str, rows: &[wsp_account::projects::projects::add::ComputerRow]) -> String {
        wsp_machines::machines::boot::BootArea::name_of_computer(id, rows)
    }
    async fn clone_here(&self, word: &str, source: &ProjectSource, into: &str) -> Result<String, HostError> {
        self.kinds.get().clone_here(word, source, into).await
    }
    async fn git_top_of(&self, path: &str) -> Result<Option<String>, HostError> {
        self.kinds.get().git_top_of(path).await.map_err(|e| e.into_host())
    }
    async fn remote_here(&self, folder: &str) -> Result<String, HostError> {
        self.kinds.get().remote_here(folder).await.map(|(remote, _)| remote).map_err(|e| e.into_host())
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        self.boot.get().landing_place(computer).await
    }
    async fn forking_at(&self, place_id: Option<&str>) -> Result<Arc<dyn wsp_engine::machine::MachineBackend>, HostError> {
        self.builders_area.get().forking_at(place_id).await?.ok_or_else(|| HostError::new("this host reads no backend for that place"))
    }
    async fn landing_backend(&self, place_id: &str) -> Result<Arc<dyn wsp_engine::machine::MachineBackend>, HostError> {
        self.builders_area.get().landing_backend(Some(place_id)).await
    }
    fn folder_computer(&self, place_id: &str) -> Option<JoinedComputer> {
        let found = self.wired_door()?.folder_computer(place_id)?;
        let machine: Arc<dyn Machine> = found.machine;
        Some(JoinedComputer { machine, home: found.home })
    }
    fn github_from_vault(&self, place_id: &str) -> bool {
        self.wired_door().is_some_and(|door| door.github_from_vault(place_id))
    }
    fn landing_road(&self, kind: LandingKind) -> Rc<dyn AccountRoad> {
        let word = match kind {
            LandingKind::Mac => "mac",
            LandingKind::Box => "box",
            LandingKind::Provider => "provider",
        };
        let road = project_landing(word).expect("the three landing kinds have a road");
        Rc::new(LandingOver { road, ctx: self.rc() })
    }
    fn project_source(&self, kind: &ProjectSource) -> Rc<dyn AccountSource> {
        let word = match kind {
            ProjectSource::Folder(_) => "folder",
            ProjectSource::Git(_) => "git",
            ProjectSource::Github(_) => "github",
            ProjectSource::Gitlab(_) => "gitlab",
        };
        let module = project_source(word).expect("the four source kinds have a module");
        Rc::new(SourceOver { module, ctx: self.rc() })
    }
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> {
        self.rules.get().remember_project(project).await
    }
    fn project_reached(&self, origin: Option<&Caller>, project_id: &str) -> Result<bool, HostError> {
        self.rules.get().project_reached(origin, project_id)
    }
    fn project_of_scope(&self, origin: Option<&Caller>) -> Option<String> {
        let scope = scope_of(origin)?;
        self.rules.get().project_of_scope(&scope)
    }
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &ProjectView, word: &str) -> Result<Option<HostError>, HostError> {
        self.rules.get().elsewhere_refusal(origin, project, word)
    }
    fn of_threads_repository(&self, origin: Option<&Caller>, project_id: &str) -> Result<bool, HostError> {
        self.rules.get().of_threads_repository(origin, project_id)
    }
    fn holds_thread(&self, workspace_id: &str) -> bool {
        self.folders.get().holds_thread(workspace_id)
    }
    async fn delete_workspace(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        wsp_core::types::api::WorkspacesApi::delete(&*self.workspaces.get(), id, origin).await
    }
    async fn copier_worktree_forget(&self, from: &str, home: &str, project: &str) -> Option<Result<(), HostError>> {
        let copier = self.opts.local.as_ref()?.copier.clone()?;
        let at = wsp_engine::copier::WorktreeProject { from: from.into(), home: home.into(), project: project.into() };
        Some(copier.worktree_forget(&at).await.map_err(|e| HostError::new(e.to_string())))
    }
    async fn forget_icon(&self, project_id: &str) -> Result<(), HostError> {
        self.project_icons.get().forget(project_id).await
    }
    async fn set_icon(&self, project_id: &str, png: Option<String>) -> Result<Option<String>, HostError> {
        self.project_icons.get().set(project_id, png).await
    }
    async fn read_icons(&self, hashes: Vec<String>) -> Result<IndexMap<String, Option<String>>, HostError> {
        self.project_icons.get().read(hashes).await
    }
    fn runs_agent(&self, agent: &str) -> bool {
        self.opts.adapters.contains_key(agent)
    }
    async fn agent_lists(&self, computer: &str, origin: Option<&Caller>) -> Result<Vec<HarnessCatalog>, HostError> {
        let agents = self.agents.get();
        let home = agents.computer_home(computer, origin.cloned()).await?;
        Ok(agents.agent_lists(home.as_ref()).await)
    }
    fn thread_defaults(&self, project_id: &str, first_agent: &str, lists: &[HarnessCatalog]) -> ThreadDefaults {
        let prefs = self.state.with(|s| s.preferences_held.clone()).unwrap_or_else(wsp_wire::views::preferences::default_preferences);
        let runs = |id: &str| self.opts.adapters.contains_key(id);
        let catalog_of = |id: &str| -> Option<HarnessCatalog> {
            let found = if runs(id) { lists.iter().find(|c| c.harness == id) } else { None }?;
            Some(wsp_words::thread_defaults::with_custom_models(found, prefs.agent_defaults.get(id).and_then(|d| d.models.as_ref())))
        };
        wsp_words::thread_defaults::resolve_thread_defaults(&wsp_words::thread_defaults::DefaultsAsk {
            first_agent,
            catalog_of: &catalog_of,
            runs: Some(&runs),
            prefs: &prefs,
            project: prefs.project_defaults.get(project_id).map(|v| &**v),
            named: None,
        })
    }
    fn first_agent(&self) -> String {
        wsp_catalog::catalog::catalog_agents()
            .into_iter()
            .find(|agent| self.opts.adapters.contains_key(&agent.id))
            .map_or_else(|| wsp_catalog::catalog::default_agent().id.clone(), |agent| agent.id.clone())
    }
}

#[async_trait(?Send)]
impl MoveRoads for Ctx {
    async fn agents_on_machine(&self, entry: &Entry, agents: &[String]) -> Result<Vec<String>, HostError> {
        let machine = entry.borrow().machine.clone();
        let present = agents_on_machine(&*machine, agents).await.map_err(|e| HostError::new(e.to_string()))?;
        Ok(agents.iter().filter(|id| present.contains(*id)).cloned().collect())
    }
    fn guest_agent_homes(&self) -> IndexMap<String, String> {
        guest_agent_homes()
    }
    fn scratch(&self, entry: &Entry) -> Result<String, HostError> {
        let module = self.kinds.get().module_of(kind_of(entry))?;
        Ok(module.scratch(&entry.borrow()))
    }
    async fn land_bundle(
        &self,
        entry: &Entry,
        tar: Vec<u8>,
        dest: &str,
        replace: Option<bool>,
        hooks: LandHooks,
    ) -> Result<u64, HostError> {
        let machine = entry.borrow().machine.clone();
        let (on_part, on_landing, tmp_dir, timeout_ms) = upload_of(hooks);
        let opts = LandOptions {
            replace: replace == Some(true),
            tmp_dir: Some(tmp_dir),
            timeout_ms: Some(timeout_ms),
            on_part: Some(on_part),
            on_landing,
            ..Default::default()
        };
        let landed = land_bundle(&machine, &tar, dest, &opts).await.map_err(|e| HostError::new(e.to_string()))?;
        Ok(u64::from(landed.parts))
    }
    async fn import_into(&self, entry: &Entry, tar: Vec<u8>, hooks: LandHooks) -> Result<(), HostError> {
        let machine = entry.borrow().machine.clone();
        let (on_part, _, tmp_dir, timeout_ms) = upload_of(hooks);
        let opts = VaultOptions {
            overlay: true,
            tmp_dir: Some(tmp_dir),
            timeout_ms: Some(timeout_ms),
            on_part: Some(on_part),
            ..Default::default()
        };
        wsp_engine::vault::import_into(machine, &tar, "/", &opts).await.map(|_| ()).map_err(|e| HostError::new(e.to_string()))
    }
    async fn merge_on_machine(&self, entry: &Entry, script: &str, agent: ProjectAgentResult) -> Result<ProjectAgentResult, HostError> {
        let machine = entry.borrow().machine.clone();
        wsp_core::types::wiring::merge_on_machine(&*machine, script, agent).await.map_err(|e| HostError::new(e.to_string()))
    }
    fn project_of(&self, dest: &str, bytes: u64) -> Option<Value> {
        let project = self.kinds.get().project_of(dest, bytes as f64);
        serde_json::to_value(project).ok()
    }
    async fn provider_row(&self, entry: &Entry) -> Result<bool, HostError> {
        let record = record_of(entry);
        let place = self.channels.get().place_id_of(&record);
        let rows = self.boot.get().computer_rows().await;
        Ok(rows
            .iter()
            .find(|row| Some(&row.id) == place.as_ref() || Some(&row.id) == record.provider.as_ref())
            .is_some_and(|row| row.kind == wsp_wire::views::place_kind::PlaceKind::Provider))
    }
    async fn export_folder(
        &self,
        entry: &Entry,
        source: &str,
        caches: &CacheRule,
        archive: &str,
        timeout_ms: u64,
        on_progress: Rc<dyn Fn(Progress)>,
    ) -> Result<Vec<String>, HostError> {
        let machine = entry.borrow().machine.clone();
        let opts = VaultOptions {
            timeout_ms: Some(timeout_ms),
            on_progress: Some(Arc::new(move |p: DownloadProgress| {
                on_progress(Progress { part: 0, parts: 0, bytes: p.bytes, total: p.total })
            })),
            ..Default::default()
        };
        let exported = export_folder(&*machine, source, caches, std::path::Path::new(archive), &opts)
            .await
            .map_err(|e| HostError::new(e.to_string()))?;
        Ok(exported.excluded)
    }
    async fn export_paths_into(
        &self,
        entry: &Entry,
        groups: Vec<(String, Vec<String>)>,
        archive: &str,
        timeout_ms: u64,
        on_progress: Rc<dyn Fn(Progress)>,
    ) -> Result<(), HostError> {
        let machine = entry.borrow().machine.clone();
        let groups: Vec<ArchiveGroup> = groups.into_iter().map(|(root, paths)| ArchiveGroup { root, paths }).collect();
        let opts = VaultOptions {
            timeout_ms: Some(timeout_ms),
            on_progress: Some(Arc::new(move |p: DownloadProgress| {
                on_progress(Progress { part: 0, parts: 0, bytes: p.bytes, total: p.total })
            })),
            ..Default::default()
        };
        export_paths_into(&*machine, &groups, std::path::Path::new(archive), &opts)
            .await
            .map(|_| ())
            .map_err(|e| HostError::new(e.to_string()))
    }
    async fn machine_run(&self, entry: &Entry, script: &str, deadline_ms: u64) -> Result<ExecResult, HostError> {
        let machine = entry.borrow().machine.clone();
        machine
            .run(script, wsp_engine::machine::RunOptions { deadline_ms, ..Default::default() })
            .await
            .map_err(|e| HostError::new(e.to_string()))
    }
    async fn machine_exec(&self, entry: &Entry, cmd: &str, timeout_ms: u64) -> Result<ExecResult, HostError> {
        let machine = entry.borrow().machine.clone();
        machine
            .exec(cmd, wsp_engine::machine::ExecOptions { timeout_ms: Some(timeout_ms), ..Default::default() })
            .await
            .map_err(|e| HostError::new(e.to_string()))
    }
    async fn kind_import(&self, entry: &Entry, o: ProjectImportOptions, report: ImportReport) -> Result<ImportLanded, HostError> {
        let module = self.kinds.get().module_of(kind_of(entry))?;
        let import = module.import(&entry.borrow(), o, report);
        import.await
    }
    fn temp_dir(&self) -> String {
        std::env::temp_dir().to_string_lossy().into_owned()
    }
}

/// The account's landing road over wsp-core's: the deps the area keeps as data become the closures the road reads
/// (projects.ts:124-150: the bytes landing, `GUEST_TMP`, `ctx.branchAt`, `clock.now`).
struct LandingOver {
    road: Rc<dyn ProjectLanding>,
    ctx: Rc<Ctx>,
}

impl LandingOver {
    fn deps_of(&self, deps: &AccountDeps) -> LandingDeps {
        let kinds = self.ctx.kinds.get();
        let clock = self.ctx.clock.clone();
        let env = deps.clone_env.clone();
        LandingDeps {
            land: Rc::new(|machine: Arc<dyn Machine>, path: String, bytes: Vec<u8>| -> LocalBoxFuture<'static, Result<(), HostError>> {
                async move {
                    wsp_engine::land_bytes::land_bytes(&*machine, &path, &bytes, Default::default())
                        .await
                        .map(|_| ())
                        .map_err(|e| HostError::new(e.to_string()))
                }
                .boxed_local()
            }),
            scratch: Rc::new(|_| wsp_engine::exec_detached::put::GUEST_TMP.to_string()),
            computer: deps.computer.as_ref().map(|c| LandingComputer { machine: c.machine.clone(), home: c.home.clone() }),
            mac_state_home: deps.mac_state_home.clone(),
            branch_here: Rc::new(move |path: &str| {
                let kinds = kinds.clone();
                let path = path.to_string();
                async move { kinds.branch_at(&path).await.ok().flatten() }.boxed_local()
            }),
            computer_name: deps.computer_name.clone(),
            clone_env: Some(Rc::new(move || env.clone())),
            now: Rc::new(move || clock.now()),
        }
    }
}

#[async_trait(?Send)]
impl AccountRoad for LandingOver {
    fn kind(&self) -> LandingKind {
        match self.road.kind() {
            "box" => LandingKind::Box,
            "provider" => LandingKind::Provider,
            _ => LandingKind::Mac,
        }
    }
    fn path(&self, name: &str, source: &ProjectSource, deps: &AccountDeps) -> Result<String, HostError> {
        self.road.path(name, source, &self.deps_of(deps))
    }
    fn memory_dir(&self, project: &ProjectView, memory_key: &str, deps: &AccountDeps) -> Result<String, HostError> {
        Ok(self.road.places(project, memory_key, &self.deps_of(deps))?.memory_dir)
    }
    fn lands_at_add(&self, seeding: bool) -> bool {
        self.road.lands_at_add(seeding)
    }
    async fn land(&self, ask: LandAsk, deps: &AccountDeps) -> Result<Landed, HostError> {
        let source = project_source(source_word(&ask.project.source))?;
        let request = LandRequest {
            project: ask.project,
            source,
            seed: ask.seed.map(|(tar, choice, plan)| LandSeed { tar, choice, plan }),
            report: ask.report,
        };
        let landed = self.road.land(request, &self.deps_of(deps)).await?;
        let mut fields = match serde_json::to_value(&landed) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        };
        let notice = match fields.remove("notice") {
            Some(Value::String(notice)) => Some(notice),
            _ => None,
        };
        Ok(Landed { fields, notice })
    }
    async fn remove(&self, project: &ProjectView, deps: &AccountDeps) -> Result<String, HostError> {
        self.road.remove(project, &self.deps_of(deps)).await
    }
    async fn folder_stands(&self, project: &ProjectView, deps: &AccountDeps) -> Option<Result<bool, HostError>> {
        self.road.folder_stands(project, &self.deps_of(deps)).await
    }
    fn takes_files(&self) -> bool {
        self.kind() == LandingKind::Box
    }
    async fn seed_into(&self, project: &ProjectView, tar: Vec<u8>, deps: &AccountDeps) -> Option<Result<(), HostError>> {
        self.road.seed_into(project, tar, &self.deps_of(deps)).await
    }
    async fn unsaved(&self, project: &ProjectView, deps: &AccountDeps) -> Option<Result<Option<String>, HostError>> {
        self.road.unsaved(project, &self.deps_of(deps)).await.map(Ok)
    }
    async fn branch(&self, project: &ProjectView, deps: &AccountDeps) -> Result<ProjectBranch, HostError> {
        Ok(self.road.branch(project, &self.deps_of(deps)).await)
    }
}

/// `Ctx` itself answers the account's `LandingRoad` as the provider road does, as it answers the machines' (boot.rs):
/// no TS member is both the context and a road, and the road a computer takes is `landing_road(kind)`.
fn review_provider_road(ctx: &Ctx) -> LandingOver {
    LandingOver { road: project_landing("provider").expect("the provider road is wired"), ctx: ctx.rc() }
}

#[async_trait(?Send)]
impl AccountRoad for Ctx {
    fn kind(&self) -> LandingKind {
        review_provider_road(self).kind()
    }
    fn path(&self, name: &str, source: &ProjectSource, deps: &AccountDeps) -> Result<String, HostError> {
        review_provider_road(self).path(name, source, deps)
    }
    fn memory_dir(&self, project: &ProjectView, memory_key: &str, deps: &AccountDeps) -> Result<String, HostError> {
        review_provider_road(self).memory_dir(project, memory_key, deps)
    }
    fn lands_at_add(&self, seeding: bool) -> bool {
        review_provider_road(self).lands_at_add(seeding)
    }
    async fn land(&self, ask: LandAsk, deps: &AccountDeps) -> Result<Landed, HostError> {
        review_provider_road(self).land(ask, deps).await
    }
    async fn remove(&self, project: &ProjectView, deps: &AccountDeps) -> Result<String, HostError> {
        review_provider_road(self).remove(project, deps).await
    }
    async fn folder_stands(&self, project: &ProjectView, deps: &AccountDeps) -> Option<Result<bool, HostError>> {
        review_provider_road(self).folder_stands(project, deps).await
    }
    fn takes_files(&self) -> bool {
        review_provider_road(self).takes_files()
    }
    async fn seed_into(&self, project: &ProjectView, tar: Vec<u8>, deps: &AccountDeps) -> Option<Result<(), HostError>> {
        review_provider_road(self).seed_into(project, tar, deps).await
    }
    async fn unsaved(&self, project: &ProjectView, deps: &AccountDeps) -> Option<Result<Option<String>, HostError>> {
        review_provider_road(self).unsaved(project, deps).await
    }
    async fn branch(&self, project: &ProjectView, deps: &AccountDeps) -> Result<ProjectBranch, HostError> {
        review_provider_road(self).branch(project, deps).await
    }
}

fn source_word(source: &ProjectSource) -> &'static str {
    match source {
        ProjectSource::Folder(_) => "folder",
        ProjectSource::Git(_) => "git",
        ProjectSource::Github(_) => "github",
        ProjectSource::Gitlab(_) => "gitlab",
    }
}

/// The account's project source over wsp-core's. A folder's remote is read through the kinds area, which answers the
/// default branch `ResolveAsk::folder_remote` (a plain remote) cannot carry (project-sources.ts:29).
struct SourceOver {
    module: Rc<dyn ProjectSourceModule>,
    ctx: Rc<Ctx>,
}

#[async_trait(?Send)]
impl AccountSource for SourceOver {
    async fn resolve(&self, source: &ProjectSource, ask: ResolveAsk) -> Result<SourceResolved, HostError> {
        let seed_plan = ask.seed_plan.clone();
        let kinds = self.ctx.kinds.get();
        let deps = SourceDeps {
            seeding: ask.seeding,
            seed_plan: Rc::new(move |folder: &str| seed_plan(folder.to_string())),
            folder_remote: Rc::new(move |folder: &str| {
                let kinds = kinds.clone();
                let folder = folder.to_string();
                async move {
                    let (remote, default_branch) = kinds.remote_here(&folder).await.map_err(|e| e.into_host())?;
                    Ok(FolderRemote { remote, default_branch })
                }
                .boxed_local()
            }),
        };
        let resolved = self.module.resolve(source, &deps).await?;
        let seed_memory_key = resolved.seed.as_ref().and_then(|plan| plan.memory.as_ref().map(|m| m.key.clone()));
        Ok(SourceResolved {
            name: resolved.name,
            remote: resolved.remote,
            default_branch: resolved.default_branch,
            seed: resolved.seed,
            seed_memory_key,
        })
    }
}

#[async_trait(?Send)]
impl wsp_account::projects::images::ProjectIconsCtx for Ctx {
    fn state_folder(&self) -> Result<PathBuf, HostError> {
        self.kinds.get().state_folder().map(PathBuf::from)
    }
    fn has_state_path(&self) -> bool {
        self.opts.state_path.is_some()
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.booted().await
    }
    fn project_held(&self, project_id: &str) -> bool {
        self.projects_held.borrow().contains_key(project_id)
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        self.preferences.get().get().await.map_err(host_of)
    }
    async fn change_preferences(&self, change: Box<dyn FnOnce(Preferences) -> Preferences>) -> Result<Preferences, HostError> {
        self.preferences.get().change_preferences(Box::new(move |held| Ok(change(held)))).await.map_err(host_of)
    }
    fn recipes(&self) -> Option<Rc<dyn wsp_core::types::wiring::RecipeShelf>> {
        self.opts.recipes.clone()
    }
    fn on_recipes_changed(&self, listener: Rc<dyn Fn()>) {
        let _ = self.hear("recipes.changed", move |_| listener());
    }
    fn ids(&self) -> Rc<dyn wsp_core::ids::Ids> {
        self.ids.clone()
    }
}
