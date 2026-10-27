// SPDX-License-Identifier: AGPL-3.0-only
//! What `runtimeOf` decides (runtime.ts:1017-1277): the public members that are more than an area's own, the sweep
//! and the close. The calls it makes on the core and the areas are `PublicCtx`. Spec: spec/runtime_of.md.
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use async_trait::async_trait;

use super::api::{AgentsApiOf, ConversationsApiOf, GoldenApiOf, HarnessesApiOf, ImageApiOf, OriginStatusOf, PreferencesApiOf, UsageDoorOf};
use super::ctx::Ctx;
use wsp_account::projects::projects::Projects;
use wsp_account::projects::workspaces::Workspaces;
use wsp_core::types::api::{
    AgentsApi, ConversationsApi, DaemonEventListener, GoldenApi, HarnessesApi, ImageApi, PreferencesApi, ProjectsApi, SessionsApi,
    WorkspacesApi,
};
use wsp_core::types::api::{CatalogAt, SayStops};
use wsp_core::types::events::EventBus;
use wsp_core::types::internal::BuilderLife;
use wsp_core::types::pending::{DaemonChannel, DeviceDoor};
use wsp_core::types::wiring::{AdoptedMachine, SweepResult};
use wsp_core::types::wiring::{Detach, OriginStatusApi, RecipeShelf, SysListener, UsageDoor};
use wsp_core::types::wiring::{ResolvedRecipe, ShelvedRecipe};
use wsp_engine::golden_seam::BUILDER_IDLE_MS;
use wsp_engine::machine::MachineBackend;
use wsp_engine::orphans::{date_parse, ListedMachine, ReapFailure, ReapReason, ReapResult, ReapedMachine, SparedMachine};
use wsp_frames::ProcKind;
use wsp_machines::places::helpers::{login_here, vault_sign_in, HomesHere};
use wsp_threads::threads::sessions::SessionsArea;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::harness::{HarnessCatalog, HarnessCatalogUnshaped};
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_words::exit::HostError;
use wsp_words::words::thread::{gone_words, GoneSeenBy, GoneSighting};

/// A builder as the sweep reads it (runtime.ts:1122-1140).
#[derive(Clone, Debug, PartialEq)]
pub struct SweptBuilder {
    pub id: String,
    pub life: BuilderLife,
    /// The machine's `CREATED_AT_LABEL`, where it carries one.
    pub created_label: Option<String>,
    /// The record's `createdAt`.
    pub created_at: String,
    /// The record's `building` flag.
    pub building: bool,
    /// The place it was made at; the wired place where none.
    pub place: Option<String>,
    pub heartbeat_held: bool,
}

/// A live record as the sweep reads it (runtime.ts:1171-1191).
#[derive(Clone, Debug, PartialEq)]
pub struct SweptEntry {
    pub id: String,
    pub machine_id: String,
    pub entry_machine_id: String,
    pub creating: bool,
    pub napping: bool,
    pub waking: bool,
    pub deleting: bool,
    pub absent_machine: bool,
    pub phase: WorkspacePhase,
}

/// What a read of a record's machine came to: its state, missing (with the provider's answer), or another failure.
#[derive(Clone, Debug, PartialEq)]
pub enum MachineRead {
    State(wsp_engine::machine::MachineState),
    Missing { answer: Option<String> },
    Failed,
}

/// The calls `runtimeOf` makes, in the order the sweep and the close make them (spec U9-U17).
#[async_trait(?Send)]
pub trait PublicCtx {
    async fn ready(&self) -> Result<(), HostError>;
    fn set_say_stops(&self, say: Rc<dyn Fn(&str)>);
    fn wall_now(&self) -> i64;
    fn clock_now(&self) -> i64;
    async fn refresh_builders(&self) -> Result<(), HostError>;
    async fn expire_grace(&self) -> Result<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>;
    fn builders(&self) -> Vec<SweptBuilder>;
    async fn hold(&self, builder_id: &str) -> Result<(), HostError>;
    async fn stop_builder(&self, builder_id: &str, place: &str) -> Result<(), HostError>;
    async fn forget_builder(&self, builder_id: &str) -> Result<(), HostError>;
    /// `known` is `knownIds` (runtime.ts:1141-1147), read once per machine the replay finds.
    async fn replay_creates(&self, known: &dyn Fn() -> Vec<String>, failed: &mut Vec<ReapFailure>)
        -> Result<Vec<ReapedMachine>, HostError>;
    /// The wired place, then every other in `places.list()` order, each with whether a backend answers there.
    fn sweep_places(&self) -> Vec<(String, bool)>;
    fn wired_place(&self) -> String;
    fn place_name(&self, place: &str) -> String;
    async fn list_at(&self, place: &str) -> Result<Vec<ListedMachine>, HostError>;
    fn entries(&self) -> Vec<SweptEntry>;
    async fn reread_held(&self, workspace_id: &str, why: &str) -> Result<(), HostError>;
    async fn recover_gone(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn read_machine(&self, workspace_id: &str) -> MachineRead;
    async fn adopt_gone(&self, workspace_id: &str, words: String) -> Result<(), HostError>;
    /// The ids something claims: every record's machine ids, the builders, the in-flight creates, the gone watch's.
    fn known_ids(&self) -> Vec<String>;
    async fn adopt_lost(
        &self,
        place: &str,
        rows: &[ListedMachine],
        claimed: &mut Vec<String>,
        failed: &mut Vec<ReapFailure>,
    ) -> Result<(Vec<AdoptedMachine>, Vec<ReapedMachine>), HostError>;
    async fn engine_reap(
        &self,
        place: &str,
        rows: Vec<ListedMachine>,
        known: Vec<String>,
        older_than_ms: Option<i64>,
    ) -> Result<ReapResult, HostError>;
    async fn sweep_strays(
        &self,
        known: Vec<String>,
        failed: &mut Vec<ReapFailure>,
        places: &[String],
    ) -> Result<Vec<ReapedMachine>, HostError>;
    /// The close's steps, each named by the TS member it runs (spec U16), in the order `close` calls them.
    async fn close_step(&self, step: CloseStep) -> Result<(), HostError>;
    /// Thread ids of the kept agents, for the close's wait.
    fn kept_agents(&self) -> Vec<String>;
    /// Reaps one kept agent at once; answers its exit.
    fn reap_kept(&self, thread_id: &str) -> futures::future::LocalBoxFuture<'static, ()>;
}

/// One step of `close()`, in the order spec U16 lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseStep {
    MarkClosing,
    ClearRootsRecheck,
    RunHeldAtClose,
    AwaitCopiesMoving,
    StopSweep,
    AwaitSweeping,
    StopDiskWatch,
    CloseIdle,
    CloseAlerts,
    CloseSlates,
    CloseAgentsReader,
    SettleDaemonSyncs,
    SettleKeptWrites,
    SettleAgentAsks,
    StopMachineReadings,
    ClosePipes,
    CloseLocal,
    ClosePlaceDoor,
    MarkClosed,
    CancelBeat,
    CancelTimers,
    StopPrPolls,
    CloseGoneWatch,
    AwaitTicking,
    FreeOwnBuilders,
    FlushTranscripts,
    AwaitTranscriptWrites,
}

/// `KEPT_CLOSE_WAIT_MS` as the close reads it (types/internal.ts).
pub use wsp_core::types::internal::KEPT_CLOSE_WAIT_MS;

/// `rt.reap(olderThanMs, say)` (runtime.ts:1105-1216, spec U9-U15).
pub async fn reap(ctx: &dyn PublicCtx, older_than_ms: Option<i64>, say: Option<Rc<dyn Fn(&str)>>) -> Result<SweepAnswer, HostError> {
    if let Some(say) = say {
        ctx.set_say_stops(say);
    }
    ctx.ready().await?;
    ctx.refresh_builders().await?;
    let mut failed: Vec<ReapFailure> = vec![];
    let mut reaped: Vec<ReapedMachine> = vec![];
    match ctx.expire_grace().await {
        Ok((grace, grace_failed)) => {
            reaped.extend(grace);
            failed.extend(grace_failed);
        }
        Err(e) => failed.push(ReapFailure { id: None, message: format!("grace sweep: {}", e.message) }),
    }
    let now = ctx.wall_now();
    for b in ctx.builders() {
        if b.life == BuilderLife::Own {
            ctx.hold(&b.id).await?;
        }
        let born = date_parse(b.created_label.as_deref().unwrap_or(&b.created_at));
        let age_ms = (!born.is_nan()).then(|| now - born as i64);
        let expired = b.life == BuilderLife::Reusable && age_ms.is_none_or(|age| age >= BUILDER_IDLE_MS as i64);
        if b.life != BuilderLife::Stale && !expired {
            continue;
        }
        let place = b.place.clone().unwrap_or_else(|| ctx.wired_place());
        if let Err(e) = ctx.stop_builder(&b.id, &place).await {
            failed.push(ReapFailure {
                id: Some(b.id.clone()),
                message: format!("could not stop: {}; stays recorded, retried next sweep", e.message),
            });
            continue;
        }
        ctx.forget_builder(&b.id).await?;
        let (reason, age_ms) = if b.life == BuilderLife::Stale {
            (if b.building { ReapReason::Unfinished } else { ReapReason::Recorded }, None)
        } else {
            (ReapReason::Expired, age_ms)
        };
        reaped.push(ReapedMachine { id: b.id, labels: None, builder: true, reason, age_ms, name: None });
    }
    let known = || claimable(ctx, &reaped);
    match ctx.replay_creates(&known, &mut failed).await {
        Ok(replayed) => reaped.extend(replayed),
        Err(e) => failed.push(ReapFailure { id: None, message: format!("asking lost creates again: {}", e.message) }),
    }
    let wired = ctx.wired_place();
    let mut listings: Vec<(String, Vec<ListedMachine>)> = vec![];
    for (place, answers) in ctx.sweep_places() {
        if !answers {
            continue;
        }
        match ctx.list_at(&place).await {
            Ok(rows) => listings.push((place, rows)),
            Err(e) if place == wired => {
                failed.push(ReapFailure { id: None, message: e.message });
                return Ok(sweep_answer(reaped, vec![], failed, vec![]));
            }
            Err(e) => failed.push(ReapFailure { id: None, message: format!("{}: {}", ctx.place_name(&place), e.message) }),
        }
    }
    let listed: HashSet<&str> = listings.iter().flat_map(|(_, rows)| rows.iter().map(|row| row.id.as_str())).collect();
    for entry in ctx.entries() {
        if entry.creating || entry.napping || entry.waking || entry.deleting {
            continue;
        }
        if entry.absent_machine {
            if let Err(e) = ctx.reread_held(&entry.id, "sweep").await {
                failed.push(ReapFailure { id: None, message: e.message });
            }
            continue;
        }
        if listed.contains(entry.machine_id.as_str()) {
            if entry.phase == WorkspacePhase::Gone {
                ctx.recover_gone(&entry.id).await?;
            }
            continue;
        }
        if entry.phase == WorkspacePhase::Gone {
            continue;
        }
        if let MachineRead::Missing { answer } = ctx.read_machine(&entry.id).await {
            let seen = GoneSighting { by: GoneSeenBy::Sweep, at: ctx.clock_now() as f64, answer };
            ctx.adopt_gone(&entry.id, gone_words(&entry.machine_id, Some(&seen))).await?;
        }
    }
    let mut claimed = claimable(ctx, &reaped);
    let mut adopted: Vec<AdoptedMachine> = vec![];
    for (place, rows) in &listings {
        let (found, lost) = ctx.adopt_lost(place, rows, &mut claimed, &mut failed).await?;
        adopted.extend(found);
        reaped.extend(lost);
    }
    match engine_pass(ctx, &listings, &reaped, &claimed, older_than_ms, &mut failed).await {
        Ok((swept, strayed)) => {
            reaped.extend(swept.reaped);
            reaped.extend(strayed);
            failed.extend(swept.failed.unwrap_or_default());
            Ok(sweep_answer(reaped, swept.spared, failed, adopted))
        }
        Err(e) => {
            failed.push(ReapFailure { id: None, message: e.message });
            Ok(sweep_answer(reaped, vec![], failed, adopted))
        }
    }
}

/// The ids something claims: the ctx's own, then every id reaped so far.
fn claimable(ctx: &dyn PublicCtx, reaped: &[ReapedMachine]) -> Vec<String> {
    let mut ids = ctx.known_ids();
    ids.extend(reaped.iter().map(|r| r.id.clone()));
    ids
}

fn sweep_answer(
    reaped: Vec<ReapedMachine>,
    spared: Vec<SparedMachine>,
    failed: Vec<ReapFailure>,
    adopted: Vec<AdoptedMachine>,
) -> SweepAnswer {
    SweepAnswer { reaped, spared, failed: (!failed.is_empty()).then_some(failed), adopted: (!adopted.is_empty()).then_some(adopted) }
}

/// The engine's pass over each listing, then the strays named after this host's images (runtime.ts:1199-1214).
async fn engine_pass(
    ctx: &dyn PublicCtx,
    listings: &[(String, Vec<ListedMachine>)],
    reaped: &[ReapedMachine],
    claimed: &[String],
    older_than_ms: Option<i64>,
    failed: &mut Vec<ReapFailure>,
) -> Result<(ReapResult, Vec<ReapedMachine>), HostError> {
    let known = || [claimable(ctx, reaped), claimed.to_vec()].concat();
    let mut swept = ReapResult { reaped: vec![], spared: vec![], failed: None };
    for (place, rows) in listings {
        let one = ctx.engine_reap(place, rows.clone(), known(), older_than_ms).await?;
        swept.reaped.extend(one.reaped);
        swept.spared.extend(one.spared);
        if let Some(more) = one.failed {
            swept.failed.get_or_insert_with(Vec::new).extend(more);
        }
    }
    let places: Vec<String> = listings.iter().map(|(place, _)| place.clone()).collect();
    let strayed = match ctx.sweep_strays(known(), failed, &places).await {
        Ok(found) => found,
        Err(e) => {
            failed.push(ReapFailure { id: None, message: format!("machines named after this host's images: {}", e.message) });
            vec![]
        }
    };
    Ok((swept, strayed))
}

/// `{ reaped, spared, failed?, adopted? }` as the sweep answers it (runtime.ts:1142-1145).
#[derive(Clone, Debug, PartialEq)]
pub struct SweepAnswer {
    pub reaped: Vec<ReapedMachine>,
    pub spared: Vec<wsp_engine::orphans::SparedMachine>,
    pub failed: Option<Vec<ReapFailure>>,
    pub adopted: Option<Vec<AdoptedMachine>>,
}

impl From<SweepAnswer> for SweepResult {
    fn from(answer: SweepAnswer) -> SweepResult {
        SweepResult { reaped: ReapResult { reaped: answer.reaped, spared: answer.spared, failed: answer.failed }, adopted: answer.adopted }
    }
}

/// `rt.close()` (runtime.ts:1217-1275, spec U16-U17).
pub async fn close(ctx: &dyn PublicCtx) -> Result<(), HostError> {
    for step in [
        CloseStep::MarkClosing,
        CloseStep::ClearRootsRecheck,
        CloseStep::RunHeldAtClose,
        CloseStep::AwaitCopiesMoving,
        CloseStep::StopSweep,
        CloseStep::AwaitSweeping,
        CloseStep::StopDiskWatch,
        CloseStep::CloseIdle,
        CloseStep::CloseAlerts,
        CloseStep::CloseSlates,
        CloseStep::CloseAgentsReader,
        CloseStep::SettleDaemonSyncs,
        CloseStep::SettleKeptWrites,
        CloseStep::SettleAgentAsks,
    ] {
        ctx.close_step(step).await?;
    }
    // Spawned so a kept agent's reap runs to the end even when the wait below gives up on it.
    let exits: Vec<_> = ctx.kept_agents().iter().map(|id| tokio::task::spawn_local(ctx.reap_kept(id))).collect();
    if !exits.is_empty() {
        let _ = tokio::time::timeout(Duration::from_millis(KEPT_CLOSE_WAIT_MS), futures::future::join_all(exits)).await;
    }
    for step in [
        CloseStep::StopMachineReadings,
        CloseStep::ClosePipes,
        CloseStep::CloseLocal,
        CloseStep::ClosePlaceDoor,
        CloseStep::MarkClosed,
        CloseStep::CancelBeat,
        CloseStep::CancelTimers,
        CloseStep::StopPrPolls,
        CloseStep::CloseGoneWatch,
        CloseStep::AwaitTicking,
        CloseStep::FreeOwnBuilders,
        CloseStep::FlushTranscripts,
        CloseStep::AwaitTranscriptWrites,
    ] {
        ctx.close_step(step).await?;
    }
    Ok(())
}

/// What `harnesses.list` reads (runtime.ts:1078-1097, spec U7).
#[async_trait(?Send)]
pub trait HarnessesCtx {
    async fn preferences(&self) -> Result<wsp_wire::views::preferences::Preferences, HostError>;
    fn talks_to_tree_on(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<bool, HostError>;
    /// The entry the lists are read for: its record's id and project, or none.
    async fn entry_of(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<(String, String)>, HostError>;
    async fn project_home(&self, project: &str, origin: Option<Caller>) -> Result<Option<(String, String)>, HostError>;
    async fn computer_home(&self, computer: &str, origin: Option<Caller>) -> Result<Option<(String, String)>, HostError>;
    async fn agent_lists(&self, entry: Option<&(String, String)>) -> Result<Vec<HarnessCatalog>, HostError>;
    fn default_agent_of(&self, prefs: &wsp_wire::views::preferences::Preferences, entry: Option<&(String, String)>) -> String;
    /// `defaultsOn(c, prefs, project).catalog` then `shapeModels(.., own.models)`.
    fn shaped(
        &self,
        catalog: &HarnessCatalog,
        prefs: &wsp_wire::views::preferences::Preferences,
        project: Option<&str>,
    ) -> serde_json::Map<String, serde_json::Value>;
}

/// `harnesses.list(workspaceId, origin, at)` (spec U7): each catalog as a JSON object in the TS's key order.
pub async fn harnesses_list(
    ctx: &dyn HarnessesCtx,
    workspace_id: Option<String>,
    origin: Option<Caller>,
    at: Option<CatalogAt>,
) -> Result<Vec<serde_json::Map<String, serde_json::Value>>, HostError> {
    let prefs = ctx.preferences().await?;
    let entry = match workspace_id {
        Some(id) => {
            let origin = if ctx.talks_to_tree_on(&id, origin.as_ref())? { None } else { origin };
            ctx.entry_of(&id, origin).await?
        }
        None => match at {
            Some(CatalogAt { project: Some(project), .. }) => ctx.project_home(&project, origin).await?,
            Some(CatalogAt { computer: Some(computer), .. }) => ctx.computer_home(&computer, origin).await?,
            _ => None,
        },
    };
    let lists = ctx.agent_lists(entry.as_ref()).await?;
    let project = entry.as_ref().map(|e| e.1.as_str()).filter(|id| prefs.project_defaults.contains_key(*id));
    let agent = ctx.default_agent_of(&prefs, entry.as_ref());
    let mut out = Vec::with_capacity(lists.len());
    for c in &lists {
        let own = prefs.agent_defaults.get(c.harness.as_str());
        let mut row = ctx.shaped(c, &prefs, project);
        if own.is_some() || project.is_some() {
            let unshaped = HarnessCatalogUnshaped {
                models: c.models.clone(),
                legacy_models: c.legacy_models.clone(),
                efforts: c.efforts.clone(),
                permission_modes: c.permission_modes.clone(),
            };
            let value = serde_json::to_value(unshaped).map_err(|e| HostError::new(e.to_string()))?;
            row.insert("unshaped".into(), value);
        }
        if c.harness == agent {
            row.insert("isDefault".into(), serde_json::Value::Bool(true));
        }
        out.push(row);
    }
    Ok(out)
}

/// What `status.list` and `status.history` read (spec U5-U6).
#[async_trait(?Send)]
pub trait StatusCtx {
    async fn probe_list(
        &self,
        o: Option<wsp_core::status::StatusListOptions>,
    ) -> Result<Vec<wsp_wire::views::workspace_more::WorkspaceStatus>, HostError>;
    fn listed_for(&self, origin: Option<&Caller>) -> Result<Vec<String>, HostError>;
    fn refuse_named(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(), HostError>;
    async fn history(&self, workspace_id: &str) -> Result<Vec<serde_json::Value>, HostError>;
}

/// `status.list(o, origin)` with the caller's rows read after the probes (spec U5).
pub async fn status_list(
    ctx: &dyn StatusCtx,
    o: Option<wsp_core::status::StatusListOptions>,
    origin: Option<Caller>,
) -> Result<Vec<wsp_wire::views::workspace_more::WorkspaceStatus>, HostError> {
    let rows = ctx.probe_list(o).await?;
    let shown: HashSet<String> = ctx.listed_for(origin.as_ref())?.into_iter().collect();
    Ok(rows.into_iter().filter(|row| shown.contains(&row.id)).collect())
}

/// `status.history(workspaceId, origin)` (spec U6).
pub async fn status_history(ctx: &dyn StatusCtx, workspace_id: &str, origin: Option<Caller>) -> Result<Vec<serde_json::Value>, HostError> {
    ctx.refuse_named(workspace_id, origin.as_ref())?;
    ctx.history(workspace_id).await
}

/// What `watchPlaceSys` refuses this computer with when it reads nothing of itself (runtime.ts:1053).
pub const NO_SAMPLES_HERE: &str = "this host reads nothing of the computer it runs on";

/// The assembled runtime: the context and the public members over it. Implements wsp-core's `Runtime`, the one a socket
/// serves (spec U1).
pub struct AssembledRuntime {
    pub(crate) ctx: Rc<Ctx>,
    workspaces: Rc<Workspaces>,
    projects: Rc<Projects>,
    sessions: Rc<SessionsArea>,
    agents: AgentsApiOf,
    preferences: PreferencesApiOf,
    conversations: ConversationsApiOf,
    image: ImageApiOf,
    golden: GoldenApiOf,
    harnesses: HarnessesApiOf,
    /// Set once a close has run to its end and emptied the slots; a later close has nothing left to do and resolves, as
    /// node's does (runtime.ts:1217).
    closed: std::cell::Cell<bool>,
}

impl AssembledRuntime {
    pub(crate) fn over(ctx: Rc<Ctx>) -> AssembledRuntime {
        AssembledRuntime {
            workspaces: ctx.workspaces.get(),
            projects: ctx.projects.get(),
            sessions: ctx.sessions_area.get(),
            agents: AgentsApiOf(ctx.clone()),
            preferences: PreferencesApiOf(ctx.clone()),
            conversations: ConversationsApiOf(ctx.clone()),
            image: ImageApiOf(ctx.clone()),
            golden: GoldenApiOf(ctx.clone()),
            harnesses: HarnessesApiOf(ctx.clone()),
            closed: std::cell::Cell::new(false),
            ctx,
        }
    }

    /// The context every area reads, for a test that calls one of its traits.
    pub fn context(&self) -> Rc<Ctx> {
        self.ctx.clone()
    }
}

#[async_trait(?Send)]
impl wsp_core::types::api::Runtime for AssembledRuntime {
    type Places = dyn wsp_machines::places::types::PlaceDoor;
    type Slates = dyn wsp_slates::slates::Slates;
    type Activity = dyn wsp_threads::activity::ActivityDoor;
    fn events(&self) -> Rc<dyn EventBus> {
        self.ctx.bus.clone()
    }
    fn backend(&self) -> std::sync::Arc<dyn MachineBackend> {
        self.ctx.backend.clone()
    }
    fn places(&self) -> Option<Rc<Self::Places>> {
        self.ctx.place_door.get()
    }
    /// Where the host wired a shelf: its own, with the picks read against what this host can serve (runtime.ts:1030-1041).
    fn recipes(&self) -> Option<Rc<dyn RecipeShelf>> {
        let shelf = self.ctx.opts.recipes.clone()?;
        Some(Rc::new(ServableShelf { ctx: self.ctx.clone(), shelf }))
    }
    async fn here_channel(&self, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        let ctx = &self.ctx;
        let road = ctx.kinds.get().local_road().await.map_err(|e| e.into_host())?;
        let channels = ctx.channels.get();
        let events: Rc<dyn Fn(serde_json::Value)> = Rc::new(move |event| {
            if let serde_json::Value::Object(frame) = event {
                on_event(frame);
            }
        });
        let channel = channels.channel_over(&road, wsp_words::words::computer::THIS_COMPUTER, events).await?;
        let owned = channels.held_to_owner(HERE_PLACE_ID, channel);
        let scopes = ctx.proc_scopes.get();
        let here = scopes.clone();
        let scoped = scopes.scoped_procs(Rc::new(move || here.scopes_here()), owned);
        Ok(channels.kill_guarded(scoped, true))
    }
    fn place_kills(&self, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        self.ctx.channels.get().kill_guarded(channel, false)
    }
    async fn watch_place_sys(&self, place_id: &str, listener: SysListener) -> Result<Detach, HostError> {
        if place_id != HERE_PLACE_ID {
            let Some(door) = self.ctx.place_door.get() else {
                return Err(HostError::new(wsp_wire::wire::machine_link::no_such_place_refusal(place_id, &[])));
            };
            let release = door.watch_sys(place_id, listener).await?;
            return Ok(release);
        }
        match self.ctx.opts.local.as_ref().and_then(|local| local.sys_samples.clone()) {
            Some(samples) => samples(listener).await,
            None => Err(HostError::new(NO_SAMPLES_HERE)),
        }
    }
    fn own_process(&self, thread_id: &str, pid: u32, kind: ProcKind) -> Detach {
        let kind = serde_json::to_value(kind)
            .and_then(serde_json::from_value::<wsp_wire::wire::daemon::ProcKind>)
            .unwrap_or(wsp_wire::wire::daemon::ProcKind::Other);
        let release = self.ctx.proc_scopes.get().own_process(thread_id, i64::from(pid), kind);
        Box::new(move || release())
    }
    fn agents(&self) -> &dyn AgentsApi {
        &self.agents
    }
    fn workspaces(&self) -> &dyn WorkspacesApi {
        &*self.workspaces
    }
    fn projects(&self) -> &dyn ProjectsApi {
        &*self.projects
    }
    fn sessions(&self) -> &dyn SessionsApi {
        &*self.sessions
    }
    fn harnesses(&self) -> &dyn HarnessesApi {
        &self.harnesses
    }
    fn golden(&self) -> &dyn GoldenApi {
        &self.golden
    }
    fn image(&self) -> &dyn ImageApi {
        &self.image
    }
    fn status(&self) -> Rc<dyn OriginStatusApi> {
        Rc::new(OriginStatusOf(self.ctx.clone()))
    }
    fn usage(&self) -> Rc<dyn UsageDoor> {
        Rc::new(UsageDoorOf(self.ctx.clone()))
    }
    fn conversations(&self) -> &dyn ConversationsApi {
        &self.conversations
    }
    fn slates(&self) -> Rc<Self::Slates> {
        self.ctx.slates.get()
    }
    fn activity(&self) -> Rc<Self::Activity> {
        self.ctx.activity.get()
    }
    fn devices(&self) -> Rc<dyn DeviceDoor> {
        self.ctx.device_door.clone()
    }
    fn preferences(&self) -> &dyn PreferencesApi {
        &self.preferences
    }
    async fn owner(&self) -> Result<String, HostError> {
        self.ctx.booted().await?;
        Ok(self.ctx.state.with(|state| state.owner.clone()))
    }
    async fn reap(&self, older_than_ms: Option<u64>, say: Option<SayStops>) -> Result<SweepResult, HostError> {
        let swept = reap(&*self.ctx, older_than_ms.map(|ms| ms as i64), say).await?;
        Ok(swept.into())
    }
    async fn close(&self) -> Result<(), HostError> {
        if self.closed.get() {
            return Ok(());
        }
        close(&*self.ctx).await?;
        self.ctx.drop_areas();
        self.closed.set(true);
        Ok(())
    }
}

/// `opts.recipes` with its `options` read against what this host can serve: an agent's sign-in to copy is offered only
/// where the vault or this computer's login can serve it, read at every ask (runtime.ts:1030-1041).
struct ServableShelf {
    ctx: Rc<Ctx>,
    shelf: Rc<dyn RecipeShelf>,
}

#[async_trait(?Send)]
impl RecipeShelf for ServableShelf {
    async fn list(&self) -> Result<Vec<ShelvedRecipe>, HostError> {
        self.shelf.list().await
    }
    async fn read(&self, word: &str) -> Result<ShelvedRecipe, HostError> {
        self.shelf.read(word).await
    }
    async fn get(&self, word: &str) -> Result<(ShelvedRecipe, String), HostError> {
        self.shelf.get(word).await
    }
    async fn save(&self, file: serde_json::Value) -> Result<ShelvedRecipe, HostError> {
        self.shelf.save(file).await
    }
    async fn remove(&self, word: &str) -> Result<ShelvedRecipe, HostError> {
        self.shelf.remove(word).await
    }
    async fn options(&self, folders: Option<Vec<(String, String)>>) -> Result<serde_json::Value, HostError> {
        let mut options = self.shelf.options(folders).await?;
        let agents = self.ctx.agents.get();
        let homes: HomesHere = Rc::new(move || {
            let agents = agents.clone();
            Box::pin(async move { agents.homes_here().await })
        });
        let vault = self.ctx.opts.vault.as_ref().map(|vault| vault()).unwrap_or_default();
        let offered = options.get_mut("agents").and_then(serde_json::Value::as_array_mut);
        for agent in offered.into_iter().flatten() {
            let id = agent["id"].as_str().unwrap_or_default().to_string();
            let Some(signins) = agent["signins"].as_array_mut() else { continue };
            let offers_vault = signins.iter().any(|word| word == "vault");
            if offers_vault
                && !matches!(vault_sign_in(&id, &vault), AgentSignInState::VaultKey)
                && login_here(homes.clone(), &id).await.is_none()
            {
                signins.retain(|word| word != "vault");
            }
        }
        Ok(options)
    }
    async fn resolve(&self, slug: &str) -> Result<ResolvedRecipe, HostError> {
        self.shelf.resolve(slug).await
    }
    async fn resolve_file(&self, file: RecipeFile) -> Result<ResolvedRecipe, HostError> {
        self.shelf.resolve_file(file).await
    }
}
