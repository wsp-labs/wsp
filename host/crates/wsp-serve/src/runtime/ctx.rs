// SPDX-License-Identifier: AGPL-3.0-only
//! The one context every area reads (runtime.ts:425-489, 491-992): the core's shared state, a slot per area filled in
//! `create_runtime` in the TS order, and `me`, so a method that must hand an area an `Rc<dyn XCtx>` can. It implements
//! every context trait the area crates declare; the impls sit one file a packet under `ctx/`, so a packet replaces
//! bodies in its own file. Everything lives on the owner thread (rules 3.1, 3.3): shared maps are `Rc<RefCell<..>>`
//! handles the areas also hold, and no borrow crosses an `.await`.
//!
//! A method body reads the core through the struct's own fields, never through another trait's method of the same
//! name: `live`, `sessions`, `thread_records` and about 15 more names are declared by several traits with different
//! signatures, so on the concrete type they are ambiguous (E0034); through `dyn Trait`, as the areas call them, they
//! are not.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use wsp_account::account::channels::ChannelsArea;
use wsp_account::account::preferences::PreferencesArea;
use wsp_account::account::proc_scopes::ProcScopesArea;
use wsp_account::account::pull_requests::PullRequestsArea;
use wsp_account::account::rules::RulesArea;
use wsp_account::account::status::StatusArea;
use wsp_account::account::usage::UsageArea;
use wsp_account::projects::folders::Folders;
use wsp_account::projects::images::ProjectIconsArea;
use wsp_account::projects::projects::Projects;
use wsp_account::projects::start_from::StartFrom;
use wsp_account::projects::workspaces::Workspaces;
use wsp_core::agent_setup::agent_setups;
use wsp_core::agent_setup::AgentSetups;
use wsp_core::clock::{real_clock, Clock};
use wsp_core::context::{Late, RuntimeState, StateCell};
use wsp_core::devices::make_devices;
use wsp_core::devices::DeviceDoor;
use wsp_core::github_cache::memory_github_cache;
use wsp_core::github_cache::GitHubCache;
use wsp_core::host_id;
use wsp_core::idle::IdlePolicy;
use wsp_core::ids::Ids;
use wsp_core::ids::OsIds;
use wsp_core::pipe::area::PipeArea;
use wsp_core::status::StatusTracker;
use wsp_core::types::events::Bus;
use wsp_core::types::events::{event_bus, EventBus, EventListener, Unlisten};
use wsp_core::types::wiring::{wired_place, WiredId};
use wsp_core::types::wiring::{PlaceBackends, RuntimeOptions};
use wsp_engine::gone_watch::GoneWatch;
use wsp_engine::gone_watch::GoneWatchOptions;
use wsp_engine::machine::MachineBackend;
use wsp_machines::agents_read::AgentsReads;
use wsp_machines::images::builders::{BuildersArea, StageAt};
use wsp_machines::images::golden::{GoldenArea, Preparing};
use wsp_machines::images::held::HeldImages;
use wsp_machines::images::image::ImageArea;
use wsp_machines::images::records::RecordsArea;
use wsp_machines::images::Builders;
use wsp_machines::machines::boot::BootArea;
use wsp_machines::machines::create::CreateArea;
use wsp_machines::machines::daemon::DaemonArea;
use wsp_machines::machines::kinds::KindsArea;
use wsp_machines::machines::machines::{Entry, MachinesArea};
use wsp_machines::machines::reach::ReachArea;
use wsp_machines::machines::sub_life::SubLifeArea;
use wsp_machines::machines::sub_queue::SubQueueArea;
use wsp_machines::machines::sub_spawn::SubSpawnArea;
use wsp_machines::machines::thread_ends::ThreadEnds;
use wsp_machines::places::types::{CopyBuild, PlaceDoor};
use wsp_slates::slates::Slates;
use wsp_store::store::Store;
use wsp_threads::activity::Activity;
use wsp_threads::threads::agents::AgentsArea;
use wsp_threads::threads::conversations::ConversationsArea;
use wsp_threads::threads::launch::LaunchArea;
use wsp_threads::threads::sessions::SessionsArea;
use wsp_threads::threads::threads::{Sessions, ThreadRecords, ThreadsArea};
use wsp_threads::threads::transcripts::{TranscriptState, TranscriptsArea};
use wsp_threads::threads::turns::{HeldAtClose, TurnsArea};
use wsp_threads::threads::waits::WaitsArea;
use wsp_wire::views::golden_image::SealedImageBuilt;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

use super::core::{core_defaults, Core, CoreDefaults, DefaultsAsked};

mod agents;
mod boot;
mod build;
mod builders;
mod create;
mod folders;
mod kinds;
mod machines;
mod phases;
mod preferences;
mod public;
mod recording;
mod review;
mod sessions;
mod sub_queue;
mod transcripts;
mod turns;
mod usage;

/// A promise more than one area awaits (the context's `Joined`).
pub type Joined = Shared<LocalBoxFuture<'static, ()>>;

/// `RuntimeContext`: the core and every area (context.ts:178-909).
// Each field is read by the packet that owns the trait methods using it.
#[allow(dead_code)]
pub struct Ctx {
    pub(crate) me: Weak<Ctx>,
    pub(crate) opts: RuntimeOptions,
    pub(crate) defaults: CoreDefaults,
    pub(crate) hostname: String,
    pub(crate) backend: Arc<dyn MachineBackend>,
    pub(crate) store: Rc<dyn Store>,
    pub(crate) clock: Rc<dyn Clock>,
    pub(crate) ids: Rc<dyn Ids>,
    pub(crate) bus: Rc<Bus>,
    pub(crate) places: Rc<dyn PlaceBackends>,
    pub(crate) device_door: Rc<dyn DeviceDoor>,
    pub(crate) github_cache: Rc<dyn GitHubCache>,
    pub(crate) setups: Rc<AgentSetups>,
    pub(crate) builders: Builders,
    pub(crate) gone: Rc<GoneWatch>,
    pub(crate) landing: Rc<RefCell<IndexMap<String, i64>>>,
    pub(crate) live: Rc<RefCell<IndexMap<String, Entry>>>,
    pub(crate) projects_held: Rc<RefCell<IndexMap<String, ProjectView>>>,
    pub(crate) preparing: Rc<RefCell<IndexMap<String, Preparing>>>,
    pub(crate) copy_builds: Rc<RefCell<IndexMap<String, wsp_machines::images::Joined<SealedImageBuilt>>>>,
    pub(crate) stage_at: Rc<RefCell<IndexMap<String, StageAt>>>,
    pub(crate) copy_rows: Rc<RefCell<IndexMap<String, Rc<CopyBuild>>>>,
    pub(crate) snooze_timers: Rc<RefCell<IndexMap<String, wsp_core::clock::Cancel>>>,
    pub(crate) resume_timers: Rc<RefCell<IndexMap<String, wsp_core::clock::Cancel>>>,
    pub(crate) thread_records: ThreadRecords,
    pub(crate) sessions: Sessions,
    pub(crate) held_at_close: HeldAtClose,
    pub(crate) boot_work: Rc<RefCell<IndexMap<String, Joined>>>,
    pub(crate) index_flushes: StateCell<IndexMap<String, Joined>>,
    pub(crate) transcript_state: StateCell<TranscriptState>,
    pub(crate) state: StateCell<RuntimeState>,
    pub(crate) execs: Rc<RefCell<IndexMap<u64, ExecHeld>>>,
    pub(crate) exec_seq: Cell<u64>,

    pub(crate) core: Late<Rc<Core>>,
    pub(crate) place_door: Late<Option<Rc<dyn PlaceDoor>>>,
    pub(crate) activity: Late<Rc<Activity>>,
    pub(crate) kinds: Late<Rc<KindsArea>>,
    pub(crate) thread_ends: Late<Option<Rc<ThreadEnds>>>,
    pub(crate) idle: Late<IdlePolicy>,
    pub(crate) rules: Late<Rc<RulesArea>>,
    pub(crate) transcripts: Late<Rc<TranscriptsArea>>,
    pub(crate) slates: Late<Rc<dyn Slates>>,
    pub(crate) channels: Late<Rc<ChannelsArea>>,
    pub(crate) proc_scopes: Late<Rc<ProcScopesArea>>,
    pub(crate) records: Late<Rc<RecordsArea>>,
    pub(crate) held_images: Late<Rc<HeldImages>>,
    pub(crate) reach: Late<Rc<ReachArea>>,
    pub(crate) pull_requests: Late<Rc<PullRequestsArea>>,
    pub(crate) daemon: Late<Rc<DaemonArea>>,
    pub(crate) machines: Late<Rc<MachinesArea>>,
    pub(crate) boot: Late<Rc<BootArea>>,
    pub(crate) create: Late<Rc<CreateArea>>,
    pub(crate) sub_spawn: Late<Rc<SubSpawnArea>>,
    pub(crate) sub_queue: Late<Rc<SubQueueArea>>,
    pub(crate) sub_life: Late<Rc<SubLifeArea>>,
    pub(crate) folders: Late<Rc<Folders>>,
    pub(crate) start_from: Late<Rc<StartFrom>>,
    pub(crate) workspaces: Late<Rc<Workspaces>>,
    pub(crate) agents: Late<Rc<AgentsArea>>,
    pub(crate) launch: Late<Rc<LaunchArea>>,
    pub(crate) threads: Late<Rc<ThreadsArea>>,
    pub(crate) turns: Late<Rc<TurnsArea>>,
    pub(crate) waits: Late<Rc<WaitsArea>>,
    pub(crate) sessions_area: Late<Rc<SessionsArea>>,
    pub(crate) builders_area: Late<Rc<BuildersArea>>,
    pub(crate) golden: Late<Rc<GoldenArea>>,
    pub(crate) image: Late<Rc<ImageArea>>,
    pub(crate) projects: Late<Rc<Projects>>,
    pub(crate) usage: Late<Rc<UsageArea>>,
    pub(crate) conversations: Late<Rc<ConversationsArea>>,
    pub(crate) status: Late<Rc<StatusArea>>,
    pub(crate) preferences: Late<Rc<PreferencesArea>>,
    pub(crate) agents_read: Late<Rc<AgentsReads>>,
    pub(crate) status_tracker: Late<Rc<StatusTracker>>,
    pub(crate) project_icons: Late<Rc<ProjectIconsArea>>,
    pub(crate) pipe: Late<Option<Rc<PipeArea>>>,
}

/// An exec stream still running, so its machine going away ends it the way it ends a session (runtime.ts:955).
pub struct ExecHeld {
    pub workspace_id: String,
    pub end: Rc<dyn Fn(&str)>,
}

impl Ctx {
    /// The context itself, for a method that hands an area `Rc<dyn XCtx>`.
    pub(crate) fn rc(&self) -> Rc<Ctx> {
        self.me.upgrade().expect("the context is alive while an area calls it")
    }

    /// The agents reader (preferences.ts:99): built on its first read, since nothing observes it before one.
    pub(crate) fn agents_read(&self) -> Rc<AgentsReads> {
        if !self.agents_read.is_filled() {
            self.agents_read.fill(self.build_agents_read());
        }
        self.agents_read.get()
    }

    /// `ctx.ready()`: the one start every road waits on.
    pub(crate) async fn booted(&self) -> Result<(), HostError> {
        self.boot.get().ready().await.map_err(|e| e.0)
    }

    /// `bus.on(kind, ..)` for a listener that takes the event: the listener's own failure never reaches the emitter.
    pub(crate) fn hear(&self, kind: &str, f: impl Fn(&EventUnion) + 'static) -> Unlisten {
        let listener: EventListener = Rc::new(move |e: &EventUnion| {
            f(e);
            Ok(())
        });
        self.bus.on(kind, listener)
    }

    /// `bus.on(kind, ..)` for a listener that reads the event as the JSON a TS object literal is.
    pub(crate) fn hear_json(&self, kind: &str, f: Rc<dyn Fn(&serde_json::Value)>) {
        let _ = self.hear(kind, move |e| {
            if let Ok(value) = serde_json::to_value(e) {
                f(&value);
            }
        });
    }

    /// `bus.on("workspace.deleted", e => f(e.workspaceId))`.
    pub(crate) fn hear_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) {
        let _ = self.hear("workspace.deleted", move |e| {
            if let EventUnion::WorkspaceDeleted(deleted) = e {
                f(&deleted.event.workspace_id);
            }
        });
    }

    fn new(me: Weak<Ctx>, opts: RuntimeOptions) -> Ctx {
        let backend = opts.backend.clone().expect("create_runtime needs a backend");
        let store = opts.store.clone().expect("create_runtime needs a store");
        let clock = opts.clock.clone().unwrap_or_else(real_clock);
        let ids: Rc<dyn Ids> = Rc::new(OsIds);
        let hostname = host_id::hostname();
        let defaults = core_defaults(&DefaultsAsked::of(&opts), &hostname);
        let bus = event_bus(ids.uuid());
        let places = opts.places.clone().unwrap_or_else(|| wired_place(WiredId::Fixed("default".into()), backend.clone()));
        let state = StateCell::new(RuntimeState::default());
        let say = state.clone();
        let warn: Arc<dyn Fn(&str)> = Arc::new(move |line: &str| match say.with(|s| s.say_stops.clone()) {
            Some(say) => say(line),
            None => wsp_core::console::warn(line),
        });
        let gone = Rc::new(GoneWatch::new(GoneWatchOptions {
            confirm: opts.kill_confirm.unwrap_or_default(),
            warn: Some(warn),
            ..Default::default()
        }));
        Ctx {
            me,
            defaults,
            hostname,
            device_door: make_devices(store.clone(), ids.clone()),
            github_cache: opts.github_cache.clone().unwrap_or_else(memory_github_cache),
            setups: Rc::new(agent_setups(store.clone())),
            places,
            bus,
            ids,
            clock,
            store,
            backend,
            opts,
            builders: Rc::default(),
            gone,
            landing: Rc::default(),
            live: Rc::default(),
            projects_held: Rc::default(),
            preparing: Rc::default(),
            copy_builds: Rc::default(),
            stage_at: Rc::default(),
            copy_rows: Rc::default(),
            snooze_timers: Rc::default(),
            resume_timers: Rc::default(),
            thread_records: Rc::default(),
            sessions: Rc::default(),
            held_at_close: Rc::default(),
            boot_work: Rc::default(),
            index_flushes: StateCell::new(IndexMap::new()),
            transcript_state: StateCell::new(TranscriptState::default()),
            state,
            execs: Rc::default(),
            exec_seq: Cell::new(0),
            core: Late::named("core"),
            place_door: Late::named("placeDoor"),
            activity: Late::named("activity"),
            kinds: Late::named("kinds"),
            thread_ends: Late::named("threadEnds"),
            idle: Late::named("idle"),
            rules: Late::named("rules"),
            transcripts: Late::named("transcripts"),
            slates: Late::named("slates"),
            channels: Late::named("channels"),
            proc_scopes: Late::named("procScopes"),
            records: Late::named("records"),
            held_images: Late::named("heldImages"),
            reach: Late::named("reach"),
            pull_requests: Late::named("pullRequests"),
            daemon: Late::named("daemon"),
            machines: Late::named("machines"),
            boot: Late::named("boot"),
            create: Late::named("create"),
            sub_spawn: Late::named("subSpawn"),
            sub_queue: Late::named("subQueue"),
            sub_life: Late::named("subLife"),
            folders: Late::named("folders"),
            start_from: Late::named("startFrom"),
            workspaces: Late::named("workspaces"),
            agents: Late::named("agents"),
            launch: Late::named("launch"),
            threads: Late::named("threads"),
            turns: Late::named("turns"),
            waits: Late::named("waits"),
            sessions_area: Late::named("sessions"),
            builders_area: Late::named("builders"),
            golden: Late::named("golden"),
            image: Late::named("image"),
            projects: Late::named("projects"),
            usage: Late::named("usage"),
            conversations: Late::named("conversations"),
            status: Late::named("status"),
            preferences: Late::named("preferences"),
            agents_read: Late::named("agentsRead"),
            status_tracker: Late::named("statusTracker"),
            project_icons: Late::named("projectIcons"),
            pipe: Late::named("pipe"),
        }
    }

    /// `createRuntime`'s context (runtime.ts:425-489): the core, then every area in the TS order, each filled into
    /// its slot after it is built so an area read before then panics with its name.
    pub fn build(opts: RuntimeOptions) -> Rc<Ctx> {
        let ctx = Rc::new_cyclic(|me| Ctx::new(me.clone(), opts));
        ctx.build_core();
        ctx.build_areas();
        ctx
    }
}
