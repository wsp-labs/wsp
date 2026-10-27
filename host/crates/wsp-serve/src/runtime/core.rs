// SPDX-License-Identifier: AGPL-3.0-only
//! What `runtimeCore` itself decides (runtime.ts:491-992): the options' defaults, a thread's launch, the timers,
//! what stands on a computer, the place door's recording and the bus wiring. The state is wsp-core's; the calls the
//! core makes on the areas are `CoreCtx`. Spec: spec/runtime_core.md.
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use async_trait::async_trait;
use futures::FutureExt as _;
use indexmap::IndexMap;
use wsp_core::clock::Clock;
use wsp_core::console;
use wsp_core::devices::{DeviceDoor, MintMore, PairedDevice};
use wsp_core::types::events::EventBus;
use wsp_core::types::internal::{copy_key, DELETED_REASON, MAX_TIMER_MS};
use wsp_core::types::wiring::{
    until, RuntimeOptions, DAEMON_HELLO_TIMEOUT_MS, GONE_CONFIRM_MS, PROVIDER_READ_MS, VAULT_CAP_BYTES, VAULT_SKIP, WAKE_LATE_READ_MS,
};
use wsp_engine::machine::{ExecOptions, MachineState};
use wsp_machines::images::builders::StageAt;
use wsp_machines::places::types::CopyBuild;
use wsp_wire::types::env::{HOST_KEY_ENV, HOST_TOKEN_ENV, HOST_URL_ENV};
use wsp_wire::types::place_state::NAP_AFTER_MS;
use wsp_wire::types::recipe_file::RecipeFileFoldersValue;
use wsp_wire::views::golden_image::{GoldenStage, GoldenStageEvent};
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::preferences::{ProjectLook, ThreadMarkedEvent};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{ProjectView, ThreadLit};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_words::exit::HostError;

/// The core's options, defaults filled (spec C2).
#[derive(Clone, Debug, PartialEq)]
pub struct CoreDefaults {
    pub provider_read_ms: u64,
    pub gone_confirm_ms: u64,
    pub late_read_ms: u64,
    pub daemon_hello_timeout_ms: u64,
    pub vault_cap_bytes: u64,
    pub default_idle_window_ms: u64,
    pub host_id: String,
}

/// The option values the defaults are read from (`RuntimeOptions`' own fields of the same names).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DefaultsAsked {
    pub provider_read_ms: Option<u64>,
    pub gone_confirm_ms: Option<u64>,
    pub late_read_ms: Option<u64>,
    pub daemon_hello_timeout_ms: Option<u64>,
    pub vault_cap_bytes: Option<u64>,
    pub idle_default_window_ms: Option<u64>,
    pub host_id: Option<String>,
}

impl DefaultsAsked {
    /// The values `opts` names.
    pub fn of(opts: &RuntimeOptions) -> DefaultsAsked {
        DefaultsAsked {
            provider_read_ms: opts.provider_read_ms,
            gone_confirm_ms: opts.gone_confirm_ms,
            late_read_ms: opts.wake.as_ref().and_then(|w| w.late_read_ms),
            daemon_hello_timeout_ms: opts.daemon_hello_timeout_ms,
            vault_cap_bytes: opts.wake.as_ref().and_then(|w| w.vault_cap_bytes),
            idle_default_window_ms: opts.idle_default_window_ms,
            host_id: opts.host_id.clone(),
        }
    }
}

/// runtime.ts:498-515 (spec C2). `hostname` is this computer's, read by the caller.
pub fn core_defaults(asked: &DefaultsAsked, hostname: &str) -> CoreDefaults {
    CoreDefaults {
        provider_read_ms: asked.provider_read_ms.unwrap_or(PROVIDER_READ_MS),
        gone_confirm_ms: asked.gone_confirm_ms.unwrap_or(GONE_CONFIRM_MS),
        late_read_ms: asked.late_read_ms.unwrap_or(WAKE_LATE_READ_MS),
        daemon_hello_timeout_ms: asked.daemon_hello_timeout_ms.unwrap_or(DAEMON_HELLO_TIMEOUT_MS),
        vault_cap_bytes: asked.vault_cap_bytes.unwrap_or(VAULT_CAP_BYTES),
        default_idle_window_ms: asked.idle_default_window_ms.unwrap_or(NAP_AFTER_MS),
        host_id: asked.host_id.clone().unwrap_or_else(|| hostname.to_string()),
    }
}

/// `tookTheResume(reads)`: `running` or `starting` only (spec C3).
pub fn took_the_resume(reads: Option<wsp_engine::machine::MachineState>) -> bool {
    matches!(reads, Some(MachineState::Running | MachineState::Starting))
}

/// `readsState(machine)`: the provider's state bounded by `provider_read_ms` on the clock; `None` for a read that
/// failed or ran out (spec C3).
pub async fn reads_state(
    machine: &dyn wsp_engine::machine::Machine,
    clock: &Rc<dyn Clock>,
    provider_read_ms: u64,
) -> Option<wsp_engine::machine::MachineState> {
    let deadline = clock.now() + provider_read_ms as i64;
    until(machine.state().boxed_local(), deadline, &format!("state of {}", machine.id()), clock.clone()).await.ok()
}

/// The machine's vault paths (spec C5): `given` where the host named them, else the breadcrumb and `ls -A /root`.
pub async fn vault_paths_of(machine: &dyn wsp_engine::machine::Machine, given: Option<&[String]>) -> Result<Vec<String>, HostError> {
    if let Some(paths) = given {
        return Ok(paths.to_vec());
    }
    machine.exec("date -u +%FT%TZ >> /root/.wsp-upgraded", ExecOptions::default()).await?;
    let ls = machine.exec("ls -A /root", ExecOptions::default()).await?;
    if ls.exit_code.as_f64() != 0.0 {
        let chars = ls.stderr.chars().count();
        let tail: String = ls.stderr.chars().skip(chars.saturating_sub(200)).collect();
        return Err(HostError::new(format!("vault enumeration failed: {tail}")));
    }
    Ok(ls.stdout.split('\n').map(str::trim).filter(|s| !s.is_empty() && !VAULT_SKIP.contains(s)).map(|s| format!("/root/{s}")).collect())
}

/// The road a workspace has to this host, as `agentsReach` answers it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchReach {
    pub url: Option<String>,
    pub wsp: Option<McpServerSpec>,
}

/// What a thread's launch is handed (runtime.ts:543): the scoped device, its env, the wsp server.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThreadLaunch {
    pub scoped: Option<PairedDevice>,
    pub env: IndexMap<String, String>,
    pub wsp: Option<McpServerSpec>,
}

/// One live record, as the core's readings of a computer need it.
#[derive(Clone, Debug, PartialEq)]
pub struct StandingEntry {
    pub id: String,
    pub name: String,
    pub kind: WorkspaceKind,
    pub place: Option<String>,
    pub project: String,
    pub running: bool,
    pub machine_id: String,
    pub sub: bool,
}

/// What `standing_on` answers: the forks, folders and projects on a computer, and the rows a remove names them by.
#[derive(Clone, Debug, PartialEq)]
pub struct Standing {
    pub forks: Vec<StandingEntry>,
    pub folders: Vec<StandingEntry>,
    pub projects: Vec<ProjectView>,
    pub rows: StandingRows,
}

/// `{ forks: { name, threads }[], projects: { name, threads }[] }`, keys in that order.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct StandingRows {
    pub forks: Vec<NamedCount>,
    pub projects: Vec<NamedCount>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct NamedCount {
    pub name: String,
    pub threads: usize,
}

/// The calls the core makes on the areas and the state it reads (context.ts's members runtime.ts names). The
/// assembly implements it on the context; tests fake it.
#[async_trait(?Send)]
pub trait CoreCtx {
    fn bus(&self) -> Rc<dyn EventBus>;
    fn clock(&self) -> Rc<dyn Clock>;
    fn devices(&self) -> Rc<dyn DeviceDoor>;
    /// The place door's host key, where a place door was built.
    fn host_key(&self) -> Option<String>;
    async fn ready(&self) -> Result<(), HostError>;
    fn agents_reach(&self, workspace_id: &str) -> Option<LaunchReach>;
    fn brokered(&self, workspace_id: &str) -> bool;
    fn record_name(&self, workspace_id: &str) -> String;
    async fn reread_public(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn read_broker(&self) -> Result<(), HostError>;
    async fn daemon_settled(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn read_public_marketplaces(&self) -> Result<(), HostError>;
    /// `moduleOf(kind).turnRoad`: throws where the kind has no module (runtime.ts:653).
    fn turn_road(&self, kind: WorkspaceKind) -> Result<WorkspaceOrigin, HostError>;
    /// The live records, in the map's order.
    fn live(&self) -> Vec<StandingEntry>;
    fn projects_held(&self) -> Vec<ProjectView>;
    /// Every thread record's (thread id, workspace id), in the map's order.
    fn thread_records(&self) -> Vec<(String, String)>;
    /// Every session's (workspace id, thread id, title, turn id), in the map's order.
    fn session_views(&self) -> Vec<wsp_wire::views::session::SessionView>;
    fn cap_held(&self, turn_id: &str) -> bool;
    fn snoozed_until(&self, thread_id: &str) -> Option<f64>;
    fn limit_resume_at(&self, thread_id: &str) -> Option<f64>;
    fn thread_workspace(&self, thread_id: &str) -> Option<String>;
    async fn resume_after_limit(&self, thread_id: &str) -> Result<(), HostError>;
    async fn wake(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn exec(&self, workspace_id: &str, cmd: &str, timeout_ms: u64) -> Result<wsp_wire::wire::machine_link::ExecResult, HostError>;
    async fn delete_workspace(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn unsaved_fork(&self, workspace_id: &str, name: &str) -> Result<Option<String>, HostError>;
    async fn unsaved_project(&self, project: &ProjectView) -> Result<Option<String>, HostError>;
    async fn remove_project(&self, project_id: &str, force: bool, keep_look: bool) -> Result<(), HostError>;
    fn end_sessions(&self, workspace_id: &str, reason: &str);
    async fn drop_record(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn store_delete_project(&self, project_id: &str) -> Result<(), HostError>;
    async fn forget_icon(&self, project_id: &str) -> Result<(), HostError>;
    /// The person's look on a project (`preferences.projectLook[id]`).
    async fn project_look(&self, project_id: &str) -> Result<Option<ProjectLook>, HostError>;
    /// `preferences.set({ projectLook: { [id]: look or null } })`.
    async fn set_project_look(&self, project_id: &str, look: Option<ProjectLook>) -> Result<(), HostError>;
    /// `projectIcons.wear(id, image)`, or `projectIcons.set(id, null)` for `None`.
    async fn set_icon(&self, project_id: &str, image: Option<String>) -> Result<(), HostError>;
    /// `image.keepCurrent(placeId)`, not awaited by the core.
    fn keep_current(&self, place_id: &str);
    /// `placeDoor.nameOf(placeId)`.
    fn place_name(&self, place_id: &str) -> String;
    /// The stored workspaces on that place this host holds by a stand-in (`isHeldAway`), in the store's order.
    async fn held_away_on(&self, place_id: &str) -> Result<Vec<String>, HostError>;
    /// `hydrateWorkspace(raw)`: whether it came back as a live entry.
    async fn hydrate(&self, workspace_id: &str) -> Result<bool, HostError>;
    /// `syncDaemon(entry)`, not awaited.
    fn sync_daemon(&self, workspace_id: &str);
    async fn reread_held(&self, workspace_id: &str, why: &str) -> Result<(), HostError>;
    /// The held rows whose record is on that place.
    fn held_on(&self, place_id: &str) -> Vec<String>;
    async fn poll(&self, workspace_ids: Vec<String>) -> Result<(), HostError>;
    /// A computer this host holds no backend and no link to now (`placeAway`).
    fn place_away(&self, place_id: &str) -> bool;
    /// `bus.emit(event)`. The read side `EventBus` has no emit, so the context delivers it.
    fn emit(&self, event: EventUnion) -> Result<(), HostError>;
}

/// The maps the core writes and other areas read (runtime.ts:836-871): handles, so the context and the areas share them.
#[derive(Clone, Default)]
pub struct CoreHandles {
    /// What each place's row says about its copy (runtime.ts:840), as `copy_build_of` answers it.
    pub copy_rows: Rc<RefCell<IndexMap<String, Rc<CopyBuild>>>>,
    /// Each build's stage on a joined computer, by place and image name (runtime.ts:836).
    pub stage_at: Rc<RefCell<IndexMap<String, StageAt>>>,
    pub snooze_timers: Rc<RefCell<IndexMap<String, wsp_core::clock::Cancel>>>,
    pub resume_timers: Rc<RefCell<IndexMap<String, wsp_core::clock::Cancel>>>,
}

/// The core's timers and readings over a `CoreCtx` (runtime.ts:860-939). One per runtime.
pub struct Core {
    pub(crate) ctx: Rc<dyn CoreCtx>,
    pub(crate) copy_rows: Rc<RefCell<IndexMap<String, Rc<CopyBuild>>>>,
    pub(crate) stage_at: Rc<RefCell<IndexMap<String, StageAt>>>,
    pub(crate) snooze_timers: Rc<RefCell<IndexMap<String, wsp_core::clock::Cancel>>>,
    pub(crate) resume_timers: Rc<RefCell<IndexMap<String, wsp_core::clock::Cancel>>>,
}

/// A timer's delay for `wait` ms, cut to the timer's cap (runtime.ts:877, 896).
fn timer_ms(wait: f64) -> i64 {
    wait.min(MAX_TIMER_MS as f64) as i64
}

/// `new Error(placeDropStoppedLine(message, went))` keeping the refusal's fields (runtime.ts:661).
fn drop_stopped(e: HostError, went: &[String]) -> HostError {
    HostError { message: wsp_wire::wire::machine_link::place_drop_stopped_line(&e.message, went), ..e }
}

impl Core {
    pub fn new(ctx: Rc<dyn CoreCtx>) -> Rc<Core> {
        Core::with_handles(ctx, CoreHandles::default())
    }
    /// The core over maps the context also holds.
    pub fn with_handles(ctx: Rc<dyn CoreCtx>, handles: CoreHandles) -> Rc<Core> {
        Rc::new(Core {
            ctx,
            copy_rows: handles.copy_rows,
            stage_at: handles.stage_at,
            snooze_timers: handles.snooze_timers,
            resume_timers: handles.resume_timers,
        })
    }
    /// `threadLaunch(entry, threadId, rootThreadId, o)` (spec C6-C9).
    pub async fn thread_launch(
        &self,
        workspace_id: &str,
        kind: WorkspaceKind,
        thread_id: &str,
        root_thread_id: &str,
        aside: bool,
    ) -> Result<ThreadLaunch, HostError> {
        let Some(reach) = self.ctx.agents_reach(workspace_id) else {
            return Ok(ThreadLaunch::default());
        };
        if self.ctx.brokered(workspace_id) {
            let ctx = self.ctx.clone();
            let ws = workspace_id.to_string();
            let name = self.ctx.record_name(workspace_id);
            tokio::task::spawn_local(async move {
                if let Err(e) = ctx.reread_public(&ws).await {
                    console::warn(&format!("the visibility of {name}'s repo was not read again: {}", e.message));
                }
            });
            // The re-read starts before the launch goes on, as its synchronous start does in the TS.
            tokio::task::yield_now().await;
            let ctx = &self.ctx;
            tokio::try_join!(ctx.read_broker(), ctx.daemon_settled(workspace_id), ctx.read_public_marketplaces())?;
        }
        let scope = ThreadScope {
            kind: ThreadLit,
            thread_id: thread_id.to_string(),
            workspace_id: workspace_id.to_string(),
            root_thread_id: root_thread_id.to_string(),
        };
        let road = Some(self.ctx.turn_road(kind)?);
        let scoped = self
            .ctx
            .devices()
            .mint(
                &format!("thread {}", wsp_words::words::computer::thread_word(thread_id)),
                scope,
                self.ctx.clock().now(),
                MintMore { road, aside },
            )
            .await?;
        let mut env = IndexMap::new();
        env.insert(HOST_TOKEN_ENV.to_string(), scoped.device_token.clone());
        if let Some(url) = reach.url {
            env.insert(HOST_URL_ENV.to_string(), url);
            if let Some(key) = self.ctx.host_key() {
                env.insert(HOST_KEY_ENV.to_string(), key);
            }
        }
        Ok(ThreadLaunch { scoped: Some(scoped), env, wsp: reach.wsp })
    }
    /// `wakeAt(threadId, until)` (spec C10).
    pub fn wake_at(self: &Rc<Self>, thread_id: &str, until: f64) {
        let old = self.snooze_timers.borrow_mut().shift_remove(thread_id);
        if let Some(cancel) = old {
            cancel();
        }
        let wait = until - self.ctx.clock().now() as f64;
        if wait <= 0.0 {
            return;
        }
        let me = self.clone();
        let thread = thread_id.to_string();
        let cancel = self.ctx.clock().schedule(Box::new(move || me.snooze_fired(&thread, until)), timer_ms(wait));
        self.snooze_timers.borrow_mut().insert(thread_id.to_string(), cancel);
    }
    /// The body of a snooze's wake: says the thread marked only when the snooze stands and its time has come.
    fn snooze_fired(self: &Rc<Self>, thread_id: &str, until: f64) {
        self.snooze_timers.borrow_mut().shift_remove(thread_id);
        if self.ctx.snoozed_until(thread_id) != Some(until) {
            return;
        }
        if (self.ctx.clock().now() as f64) < until {
            self.wake_at(thread_id, until);
            return;
        }
        let Some(workspace_id) = self.ctx.thread_workspace(thread_id) else {
            return;
        };
        let marked = ThreadMarkedEvent { workspace_id, thread_ids: vec![thread_id.to_string()] };
        if let Err(e) = self.ctx.emit(EventUnion::ThreadMarked(Sequenced { event: marked, seq: None })) {
            console::warn(&e.message);
        }
    }
    /// `resumeOnReset(threadId, at)` (spec C11).
    pub fn resume_on_reset(self: &Rc<Self>, thread_id: &str, at: f64) {
        let old = self.resume_timers.borrow_mut().shift_remove(thread_id);
        if let Some(cancel) = old {
            cancel();
        }
        let me = self.clone();
        let thread = thread_id.to_string();
        let wait = (at - self.ctx.clock().now() as f64).max(0.0);
        let cancel = self.ctx.clock().schedule(Box::new(move || me.resume_fired(&thread, at)), timer_ms(wait));
        self.resume_timers.borrow_mut().insert(thread_id.to_string(), cancel);
    }
    /// The body of a reset's timer: resumes the thread only when its reset still stands, once the reset is reached.
    fn resume_fired(self: &Rc<Self>, thread_id: &str, at: f64) {
        self.resume_timers.borrow_mut().shift_remove(thread_id);
        if self.ctx.limit_resume_at(thread_id) != Some(at) {
            return;
        }
        if (self.ctx.clock().now() as f64) < at {
            self.resume_on_reset(thread_id, at);
            return;
        }
        let ctx = self.ctx.clone();
        let thread = thread_id.to_string();
        tokio::task::spawn_local(async move {
            if let Err(e) = ctx.resume_after_limit(&thread).await {
                console::warn(&format!(
                    "thread {} did not resume at its reset: {}",
                    wsp_words::words::computer::thread_word(&thread),
                    e.message
                ));
            }
        });
    }
    /// The armed snooze wakes and resumes, by thread, for the close and the tests.
    pub fn armed(&self) -> (Vec<String>, Vec<String>) {
        (self.snooze_timers.borrow().keys().cloned().collect(), self.resume_timers.borrow().keys().cloned().collect())
    }
    /// The threads of one workspace: the distinct thread ids of its thread records and of its sessions' views.
    fn threads_of(&self, workspace_id: &str) -> usize {
        let mut ids: HashSet<String> =
            self.ctx.thread_records().into_iter().filter(|(_, ws)| ws.as_str() == workspace_id).map(|(thread, _)| thread).collect();
        ids.extend(self.ctx.session_views().into_iter().filter(|s| s.workspace_id == workspace_id).filter_map(|s| s.thread_id));
        ids.len()
    }
    /// `standingOn(placeId)` (spec C12).
    pub async fn standing_on(&self, place_id: &str) -> Result<Standing, HostError> {
        self.ctx.ready().await?;
        let live = self.ctx.live();
        let standing: Vec<StandingEntry> = live.iter().filter(|e| e.place.as_deref() == Some(place_id)).cloned().collect();
        let forks: Vec<StandingEntry> = standing.iter().filter(|e| !wsp_words::projects::runs_in_folder(&e.kind)).cloned().collect();
        let folders: Vec<StandingEntry> = standing.iter().filter(|e| wsp_words::projects::runs_in_folder(&e.kind)).cloned().collect();
        let projects: Vec<ProjectView> = self.ctx.projects_held().into_iter().filter(|p| p.computer == place_id).collect();
        let fork_rows = forks.iter().map(|e| NamedCount { name: self.fork_name(e), threads: self.threads_of(&e.id) }).collect();
        let project_rows = projects
            .iter()
            .map(|p| {
                let threads =
                    live.iter().filter(|e| e.project == p.id && !forks.iter().any(|f| f.id == e.id)).map(|e| self.threads_of(&e.id)).sum();
                NamedCount { name: p.name.clone(), threads }
            })
            .collect();
        Ok(Standing { forks, folders, projects, rows: StandingRows { forks: fork_rows, projects: project_rows } })
    }
    /// `forkName(entry)` (spec C13).
    pub fn fork_name(&self, entry: &StandingEntry) -> String {
        let views: Vec<SessionView> = self.ctx.session_views().into_iter().filter(|s| s.workspace_id == entry.id).collect();
        let mut titles: Vec<String> = wsp_words::views::session::fold_threads(&views).into_iter().map(|t| t.title).collect();
        if titles.len() == 1 {
            titles.swap_remove(0)
        } else {
            entry.name.clone()
        }
    }
    /// `forgetProject(projectId)` (spec C14). The held map is the context's.
    pub async fn forget_project(&self, project_id: &str) -> Result<(), HostError> {
        self.ctx.store_delete_project(project_id).await?;
        let removed = wsp_wire::views::place_more::ProjectRemovedEvent { project_id: project_id.to_string() };
        self.ctx.emit(EventUnion::ProjectRemoved(Sequenced { event: removed, seq: None }))?;
        self.ctx.forget_icon(project_id).await
    }
    /// `recording.holdsOn(placeId, read)` (spec C16): the rows, with `unsaved`.
    pub async fn holds_on(&self, place_id: &str, read: bool) -> Result<(StandingRows, Vec<String>), HostError> {
        let standing = self.standing_on(place_id).await?;
        let mut unsaved = Vec::new();
        if !read {
            return Ok((standing.rows, unsaved));
        }
        for entry in &standing.forks {
            if let Some(line) = self.ctx.unsaved_fork(&entry.id, &self.fork_name(entry)).await? {
                unsaved.push(line);
            }
        }
        for project in &standing.projects {
            if let Some(line) = self.ctx.unsaved_project(project).await? {
                unsaved.push(line);
            }
        }
        Ok((standing.rows, unsaved))
    }
    /// `recording.dropOn(placeId)` (spec C17).
    pub async fn drop_on(&self, place_id: &str) -> Result<StandingRows, HostError> {
        let standing = self.standing_on(place_id).await?;
        // Each goes by its own road, so one that refuses stops the rest, and the refusal names those already gone.
        let mut went: Vec<String> = Vec::new();
        for entry in &standing.forks {
            let name = self.fork_name(entry);
            self.ctx.delete_workspace(&entry.id).await.map_err(|e| drop_stopped(e, &went))?;
            went.push(name);
        }
        // A project folder holding threads stops its project's remove, so its threads go with it first.
        for entry in &standing.folders {
            self.ctx.delete_workspace(&entry.id).await.map_err(|e| drop_stopped(e, &went))?;
            went.push(entry.name.clone());
        }
        for project in &standing.projects {
            self.ctx.remove_project(&project.id, true, false).await.map_err(|e| drop_stopped(e, &went))?;
            went.push(project.name.clone());
        }
        Ok(standing.rows)
    }
    /// `recording.forgetOn(placeId)` (spec C18).
    pub async fn forget_on(&self, place_id: &str) -> Result<StandingRows, HostError> {
        let standing = self.standing_on(place_id).await?;
        for entry in standing.forks.iter().chain(standing.folders.iter()) {
            self.ctx.end_sessions(&entry.id, DELETED_REASON);
            self.ctx.drop_record(&entry.id).await?;
        }
        for project in &standing.projects {
            self.forget_project(&project.id).await?;
        }
        Ok(standing.rows)
    }
    /// `recording.machinesOn(placeId)` (spec C19).
    pub async fn machines_on(&self, place_id: &str) -> Result<Vec<String>, HostError> {
        self.ctx.ready().await?;
        Ok(self
            .ctx
            .live()
            .into_iter()
            .filter(|e| !e.machine_id.is_empty() && e.place.as_deref() == Some(place_id))
            .map(|e| e.machine_id)
            .collect())
    }
    /// `recording.removeFolder(placeId, projectId)` (spec C20).
    pub async fn remove_folder(&self, place_id: &str, project_id: &str) -> Result<(), HostError> {
        let Some(project) = self.ctx.projects_held().into_iter().find(|p| p.computer == place_id && p.id == project_id) else {
            return Ok(());
        };
        let standing: Vec<String> = self.ctx.live().into_iter().filter(|e| e.project == project.id).map(|e| e.name).collect();
        if !standing.is_empty() {
            return Err(HostError::new(wsp_words::projects::project_in_use_refusal(&project.name, &standing)));
        }
        self.forget_project(&project.id).await
    }
    /// `folderLook(projectId, was, now)` (spec C21).
    pub async fn folder_look(&self, project_id: &str, was: &RecipeFileFoldersValue, now: &RecipeFileFoldersValue) -> Result<(), HostError> {
        let held = self.ctx.project_look(project_id).await?;
        let icon = if was.icon != now.icon { now.icon } else { held.as_ref().and_then(|h| h.icon) };
        let hue = if was.hue != now.hue { now.hue } else { held.as_ref().and_then(|h| h.hue) };
        let look = if icon.is_none() && hue.is_none() { None } else { Some(ProjectLook { icon, hue }) };
        self.ctx.set_project_look(project_id, look).await?;
        if was.image != now.image {
            self.ctx.set_icon(project_id, now.image.as_ref().map(|h| h.0.clone())).await?;
        }
        Ok(())
    }
    /// The bus listeners `runtimeCore` adds (spec C23, C26): a `place.setup` with an end keeps that place's copy
    /// current; a `golden.stage` naming a place writes or drops its copy row. Every other event does nothing.
    pub fn on_bus_event(&self, e: &serde_json::Value) {
        let field = |k: &str| e.get(k).and_then(serde_json::Value::as_str);
        match field("type") {
            Some("place.setup") => {
                if let Some(place) = field("placeId") {
                    if e.get("end").is_some_and(|end| !end.is_null()) {
                        self.ctx.keep_current(place);
                    }
                }
            }
            Some("golden.stage") => {
                let Some(place) = field("place") else {
                    return;
                };
                let Some(stage) = e.get("stage").and_then(|s| serde_json::from_value::<GoldenStage>(s.clone()).ok()) else {
                    return;
                };
                match wsp_words::words::image::copy_build_of(stage, field("detail")) {
                    None => {
                        self.copy_rows.borrow_mut().shift_remove(place);
                    }
                    Some(build) => {
                        let row = CopyBuild { line: build.line, stopped: build.stopped };
                        self.copy_rows.borrow_mut().insert(place.to_string(), Rc::new(row));
                    }
                }
            }
            _ => {}
        }
    }
    /// The place door's listeners (spec C15, C24, C25): `place.removed` drops the copy row; `place.absent` and
    /// `place.present` re-emit each stage on that place; `place.present` rehydrates the stand-ins there.
    pub fn on_place_event(self: &Rc<Self>, e: &serde_json::Value) {
        let Some(place) = e.get("placeId").and_then(serde_json::Value::as_str) else {
            return;
        };
        match e.get("type").and_then(serde_json::Value::as_str) {
            Some("place.removed") => {
                self.copy_rows.borrow_mut().shift_remove(place);
            }
            Some(ty @ ("place.absent" | "place.present")) => {
                let gone = ty == "place.absent";
                self.re_emit_stages(place, gone);
                if !gone {
                    self.rehydrate_held(place);
                }
            }
            _ => {}
        }
    }
    /// Says each stage on a place again: the dial-back line while the place is gone, else the frame's own detail.
    fn re_emit_stages(&self, place: &str, gone: bool) {
        let name = self.ctx.place_name(place);
        let stages: Vec<StageAt> = self.stage_at.borrow().values().filter(|at| at.place == place).cloned().collect();
        for at in stages {
            let detail = if gone { Some(wsp_wire::wire::place_link::place_dial_back_line(&name)) } else { at.frame.detail.clone() };
            let event = GoldenStageEvent {
                name: at.frame.name.clone(),
                stage: at.frame.stage,
                detail,
                step: at.frame.step,
                left: at.frame.left.clone(),
                place: at.named.then(|| at.place.clone()),
            };
            if let Err(e) = self.ctx.emit(EventUnion::GoldenStage(Sequenced { event, seq: None })) {
                console::warn(&e.message);
            }
        }
    }
    /// Reads the records a place holds by a stand-in again, then every held row there (spec C25).
    fn rehydrate_held(&self, place: &str) {
        let ctx = self.ctx.clone();
        let place = place.to_string();
        tokio::task::spawn_local(async move {
            let read: Result<(), HostError> = async {
                ctx.ready().await?;
                for id in ctx.held_away_on(&place).await? {
                    if ctx.hydrate(&id).await? {
                        ctx.sync_daemon(&id);
                    } else {
                        ctx.reread_held(&id, "record load").await?;
                    }
                }
                ctx.poll(ctx.held_on(&place)).await
            }
            .await;
            if let Err(e) = read {
                console::warn(&format!("the records on {} were not read again: {}", ctx.place_name(&place), e.message));
            }
        });
    }
    /// Keeps a build's stage on a joined computer, for the re-emits (runtime.ts:836).
    pub fn note_stage(&self, at: StageAt) {
        let key = copy_key(&at.place, &at.name);
        if matches!(at.frame.stage, GoldenStage::Ready | GoldenStage::Sealed | GoldenStage::Failed) {
            self.stage_at.borrow_mut().shift_remove(&key);
        } else {
            self.stage_at.borrow_mut().insert(key, at);
        }
    }
    /// The copy row of a place, as `copyRows.get(placeId)`.
    pub fn copy_row(&self, place_id: &str) -> Option<Rc<CopyBuild>> {
        self.copy_rows.borrow().get(place_id).cloned()
    }
    /// `rowSaysFailure(place, e)` (spec C27): false for a fork-nowhere or a provisioning refusal (wsp-machines'
    /// marks), an absent place (the engine's), or a place away.
    pub fn row_says_failure(&self, place_id: &str, e: &HostError) -> bool {
        !(wsp_machines::places::types::is_place_forks_nowhere(e)
            || wsp_machines::places::types::is_place_provisioning(e)
            || wsp_machines::places::setup::run::is_absent(e)
            || self.ctx.place_away(place_id))
    }
}
