// SPDX-License-Identifier: AGPL-3.0-only
//! `createRuntime` (runtime.ts:425-489) and `runtimeCore`'s wiring (runtime.ts:493-790, 841-846): the core and the
//! place door first, then each area in the TS order, each filled into its slot as it is built, with the bus and door
//! listeners the area crates leave to the host registered where the TS registers them.
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use futures::FutureExt as _;
use wsp_account::account::channels::ChannelsArea;
use wsp_account::account::preferences::PreferencesArea;
use wsp_account::account::proc_scopes::ProcScopesArea;
use wsp_account::account::pull_requests::PullRequestsArea;
use wsp_account::account::rules::RulesArea;
use wsp_account::account::status::StatusArea;
use wsp_account::account::usage::UsageArea;
use wsp_account::projects::folders::folders_area;
use wsp_account::projects::images::project_icons_area;
use wsp_account::projects::projects::projects_area;
use wsp_account::projects::start_from::start_from_area;
use wsp_account::projects::workspaces::workspaces_area;
use wsp_core::idle::{create_idle_policy, IdlePolicy, IdlePolicyOptions};
use wsp_core::pipe::area::pipe_area;
use wsp_core::status::POLL_INTERVAL_MS;
use wsp_core::types::events::{EventBus, EventListener, ANY_EVENT};
use wsp_core::types::wiring::{at_provider, RecipeShelf, WorkspaceRecord};
use wsp_machines::images::builders::BuildersArea;
use wsp_machines::images::golden::GoldenArea;
use wsp_machines::images::held::HeldImages;
use wsp_machines::images::image::ImageArea;
use wsp_machines::images::records::RecordsArea;
use wsp_machines::machines::boot::{BootArea, BootConfig};
use wsp_machines::machines::create::CreateArea;
use wsp_machines::machines::daemon::DaemonArea;
use wsp_machines::machines::kinds::KindsArea;
use wsp_machines::machines::machines::{MachinesArea, MachinesConfig};
use wsp_machines::machines::reach::ReachArea;
use wsp_machines::machines::sub_life::SubLifeArea;
use wsp_machines::machines::sub_queue::SubQueueArea;
use wsp_machines::machines::sub_spawn::SubSpawnArea;
use wsp_machines::machines::thread_ends::{PlaceEvent as EndEvent, ThreadEnds};
use wsp_machines::places::make_place_door;
use wsp_machines::places::types::{PlaceDoor, PlaceDoorOptions, PlaceEvent, PlaceWiring, Resolved, SetupEvent};
use wsp_slates::account::slates::slates_area;
use wsp_slates::lazy_slates::lazy_slates;
use wsp_slates::slates::{create_slates, Slates};
use wsp_threads::activity::Activity;
use wsp_threads::threads::agents::AgentsArea;
use wsp_threads::threads::conversations::ConversationsArea;
use wsp_threads::threads::launch::LaunchArea;
use wsp_threads::threads::sessions::SessionsArea;
use wsp_threads::threads::threads::ThreadsArea;
use wsp_threads::threads::transcripts::TranscriptsArea;
use wsp_threads::threads::turns::TurnsArea;
use wsp_threads::threads::waits::WaitsArea;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_words::exit::HostError;

use super::recording::Recording;
use super::Ctx;
use crate::runtime::core::{Core, CoreCtx, CoreHandles};

impl Ctx {
    /// A listener that hears every event, as the areas' `on_event` hooks are called (emit order).
    fn hear_all(&self, f: impl Fn(&EventUnion) + 'static) {
        let listener: EventListener = Rc::new(move |e: &EventUnion| {
            f(e);
            Ok(())
        });
        let _ = self.bus.on(ANY_EVENT, listener);
    }

    /// `runtimeCore`'s place door and the listeners it and the golden stage add (runtime.ts:493-846).
    pub(super) fn build_core(&self) {
        let me = self.rc();
        let handles = CoreHandles {
            copy_rows: self.copy_rows.clone(),
            stage_at: self.stage_at.clone(),
            snooze_timers: self.snooze_timers.clone(),
            resume_timers: self.resume_timers.clone(),
        };
        let core = Core::with_handles(me.clone() as Rc<dyn CoreCtx>, handles);
        self.core.fill(core.clone());
        let door = self.opts.place_links.clone().map(|links| self.place_door_over(links));
        self.place_door.fill(door.clone());
        if let Some(door) = door {
            let bus = self.bus.clone();
            door.on(Rc::new(move |e: &PlaceEvent| {
                if let Err(err) = bus.emit(event_of(e)) {
                    wsp_core::console::warn(&err.message);
                }
            }));
            let core_hears = core.clone();
            door.on(Rc::new(move |e: &PlaceEvent| {
                if let Ok(value) = serde_json::to_value(event_of(e)) {
                    core_hears.on_place_event(&value);
                }
            }));
        }
        for ty in ["place.setup", "golden.stage"] {
            let core = core.clone();
            let listener: EventListener = Rc::new(move |e: &EventUnion| {
                if let Ok(value) = serde_json::to_value(e) {
                    core.on_bus_event(&value);
                }
                Ok(())
            });
            let _ = self.bus.on(ty, listener);
        }
    }

    fn place_door_over(&self, links: Rc<dyn wsp_core::types::pending::PlaceWiring>) -> Rc<dyn PlaceDoor> {
        let any: Rc<dyn Any> = links;
        let wiring = any.downcast::<PlaceWiring>().unwrap_or_else(|_| panic!("opts.place_links is not the machines' PlaceWiring"));
        let me = self.me.clone();
        let bus = self.bus.clone();
        let setup_bus = self.bus.clone();
        let copy_rows = self.copy_rows.clone();
        let places = self.places.clone();
        let nap_ctx = self.me.clone();
        let owner_ctx = self.me.clone();
        let opts = &self.opts;
        make_place_door(PlaceDoorOptions {
            store: self.store.clone(),
            devices: self.device_door.clone(),
            wiring: (*wiring).clone(),
            recording: Rc::new(Recording::new(me)),
            providers: Some(Rc::new(move || places.clone())),
            on_daemon_event: None,
            on_stage: Some(Rc::new(move |e| {
                if let Err(err) = bus.emit(EventUnion::PlaceStage(Sequenced { event: e.clone(), seq: None })) {
                    wsp_core::console::warn(&err.message);
                }
            })),
            on_setup: Some(Rc::new(move |e: &SetupEvent| {
                let event = match e.clone() {
                    SetupEvent::Setup(v) => EventUnion::PlaceSetup(Sequenced { event: v, seq: None }),
                    SetupEvent::Pending(v) => EventUnion::PlacePending(Sequenced { event: v, seq: None }),
                    SetupEvent::Sync(v) => EventUnion::PlaceSync(Sequenced { event: v, seq: None }),
                    SetupEvent::RecipesChanged(v) => EventUnion::RecipesChanged(Sequenced { event: v, seq: None }),
                };
                if let Err(err) = setup_bus.emit(event) {
                    wsp_core::console::warn(&err.message);
                }
            })),
            copy_build: Some(Rc::new(move |place| copy_rows.borrow().get(place).map(|row| (**row).clone()))),
            nap_ms: Some(self.defaults.default_idle_window_ms),
            here_daemon: opts.local.as_ref().and_then(|local| local.here_daemon.clone()),
            nap_changed: Some(Rc::new(move |place| {
                if let Some(ctx) = nap_ctx.upgrade() {
                    ctx.touch_idle_on(place);
                }
            })),
            join_wait_ms: opts.place_join_wait_ms,
            update_wait_ms: opts.place_update_wait_ms,
            dial_wait_ms: opts.place_dial_wait_ms,
            frame_wait_ms: opts.place_frame_wait_ms,
            relink_wait_ms: opts.place_relink_wait_ms,
            seen_every_ms: None,
            vault: opts.vault.clone(),
            recipes: opts.recipes.clone().map(|shelf| {
                let resolver: Rc<dyn wsp_machines::places::types::RecipeResolver> = Rc::new(RecipeShelfResolver(shelf));
                Rc::new(move || Some(resolver.clone())) as Rc<dyn Fn() -> Option<Rc<dyn wsp_machines::places::types::RecipeResolver>>>
            }),
            owner: Some(Rc::new(move || {
                let ctx = owner_ctx.upgrade().expect("the context is alive while the door asks for the owner");
                Box::pin(async move {
                    let _ = ctx.boot.get().ready().await;
                    ctx.state.with(|state| state.owner.clone())
                })
            })),
            kill_confirm: opts.kill_confirm,
            clock: Some(self.clock.clone()),
        })
    }

    /// The nap window of a place changed: every running workspace there whose window is its place's starts over.
    fn touch_idle_on(&self, place: &str) {
        let channels = self.channels.get();
        let ids: Vec<String> = self
            .live
            .borrow()
            .values()
            .filter_map(|entry| {
                let entry = entry.borrow();
                let record = entry.record.borrow();
                let running = record.phase == wsp_wire::views::workspace::WorkspacePhase::Running;
                (running && record.idle_window_ms.is_none() && channels.place_id_of(&record).as_deref() == Some(place))
                    .then(|| record.id.clone())
            })
            .collect();
        let idle = self.idle.get();
        for id in ids {
            idle.touch(&id);
        }
    }

    /// Every area in the TS order of runtime.ts:427-487.
    pub(super) fn build_areas(&self) {
        let me = self.rc();
        build_areas_of(&me);
    }
}

/// `PlaceEvent` as the bus carries it.
fn event_of(e: &PlaceEvent) -> EventUnion {
    match e.clone() {
        PlaceEvent::Joined(v) => EventUnion::PlaceJoined(Sequenced { event: v, seq: None }),
        PlaceEvent::Changed(v) => EventUnion::PlaceChanged(Sequenced { event: v, seq: None }),
        PlaceEvent::Removed(v) => EventUnion::PlaceRemoved(Sequenced { event: v, seq: None }),
        PlaceEvent::Present(v) => EventUnion::PlacePresent(Sequenced { event: v, seq: None }),
        PlaceEvent::Absent(v) => EventUnion::PlaceAbsent(Sequenced { event: v, seq: None }),
    }
}

/// `PlaceEvent` as the thread-ends area hears it.
fn end_event_of(e: &PlaceEvent) -> EndEvent {
    match e {
        PlaceEvent::Present(v) => EndEvent::Present { place_id: v.place_id.clone() },
        PlaceEvent::Removed(v) => EndEvent::Removed { place_id: v.place_id.clone() },
        _ => EndEvent::Other,
    }
}

/// Whether a record's credentials stay with the host (`isBrokered` with the provider read off the place table).
fn brokered_record(ctx: &Ctx, record: &WorkspaceRecord) -> bool {
    record.kind == WorkspaceKind::Cloud && (record.place.is_none() || at_provider(record.place.as_deref(), &*ctx.places))
}

fn build_areas_of(ctx: &Rc<Ctx>) {
    ctx.activity.fill(Rc::new(Activity::new(ctx.clone())));
    ctx.kinds.fill(KindsArea::new(ctx.clone()));
    let ends = ctx.place_door.get().map(|door| {
        let ends = ThreadEnds::new(ctx.clone(), ctx.clone());
        let hears = ends.clone();
        door.on(Rc::new(move |e: &PlaceEvent| hears.on_place_event(&end_event_of(e))));
        ends
    });
    ctx.thread_ends.fill(ends);
    ctx.rules.fill(RulesArea::new(ctx.clone()));
    ctx.transcripts.fill(TranscriptsArea::new(ctx.clone()).unwrap_or_else(|e| panic!("{}", e.message)));

    let slates_slot: Rc<RefCell<Option<Rc<dyn Slates>>>> = Rc::default();
    let napped = slates_slot.clone();
    let wiring = slates_area(
        ctx.clone(),
        Rc::new(move |workspace: &str| {
            let slates = napped.borrow().clone();
            if let Some(slates) = slates {
                slates.napped(workspace);
            }
        }),
    );
    let slates = lazy_slates(Rc::new(wiring), Box::new(create_slates));
    *slates_slot.borrow_mut() = Some(slates.clone());
    ctx.slates.fill(slates);

    ctx.channels.fill(ChannelsArea::new(ctx.clone()));
    ctx.proc_scopes.fill(ProcScopesArea::new(ctx.clone()));
    ctx.records.fill(Rc::new(RecordsArea::new(ctx.clone())));
    ctx.held_images.fill(Rc::new(HeldImages::new(ctx.clone())));
    ctx.reach.fill(ReachArea::new(ctx.clone()));
    ctx.pull_requests.fill(PullRequestsArea::new(ctx.clone()));

    let daemon = DaemonArea::new(ctx.clone());
    ctx.daemon.fill(daemon.clone());
    ctx.hear_all(move |e| daemon.on_event(e));

    let machines = MachinesArea::new(
        ctx.clone(),
        MachinesConfig {
            gone_confirm_ms: ctx.defaults.gone_confirm_ms,
            late_read_ms: ctx.defaults.late_read_ms,
            vault_cap_bytes: ctx.defaults.vault_cap_bytes,
            default_idle_window_ms: Some(ctx.defaults.default_idle_window_ms as f64),
            host_id: ctx.defaults.host_id.clone(),
            poll_interval_ms: ctx.opts.status.as_ref().and_then(|s| s.poll_interval_ms).unwrap_or(POLL_INTERVAL_MS) as u64,
        },
    );
    ctx.machines.fill(machines.clone());
    let hears = machines.clone();
    ctx.hear_all(move |e| hears.on_event(e));
    ctx.idle.fill(idle_of(ctx, &machines));

    let boot = BootArea::new(ctx.clone(), BootConfig { host_id: ctx.defaults.host_id.clone() });
    ctx.boot.fill(boot.clone());
    ctx.hear_all(move |e| boot.on_event(e));

    let create = CreateArea::new(ctx.clone());
    ctx.create.fill(create.clone());
    ctx.hear_all(move |e| create.on_event(e));
    ctx.sub_spawn.fill(SubSpawnArea::new(ctx.clone()));
    ctx.sub_queue.fill(SubQueueArea::new(ctx.clone()));
    ctx.sub_life.fill(SubLifeArea::new(ctx.clone()));

    ctx.folders.fill(folders_area(ctx.clone()));
    ctx.start_from.fill(start_from_area(ctx.clone()));
    ctx.workspaces.fill(workspaces_area(ctx.clone()));

    ctx.agents.fill(AgentsArea::new(ctx.clone()));
    ctx.launch.fill(Rc::new(LaunchArea::new(ctx.clone())));
    ctx.threads.fill(ThreadsArea::new(ctx.clone()));
    ctx.turns.fill(Rc::new(TurnsArea::new(ctx.clone())));
    ctx.waits.fill(Rc::new(WaitsArea::new(ctx.clone())));
    ctx.sessions_area.fill(Rc::new(SessionsArea::new(ctx.clone())));

    ctx.builders_area.fill(BuildersArea::new(ctx.clone()));
    ctx.golden.fill(GoldenArea::new(ctx.clone()));
    ctx.image.fill(ImageArea::new(ctx.clone()));

    ctx.projects.fill(projects_area(ctx.clone()));
    ctx.usage.fill(UsageArea::new(ctx.clone()));
    ctx.conversations.fill(ConversationsArea::new(ctx.clone()));
    ctx.status.fill(StatusArea::new(ctx.clone()));
    ctx.preferences.fill(PreferencesArea::new(ctx.clone()));
    ctx.project_icons.fill(project_icons_area(ctx.clone()));

    let judge = Rc::downgrade(ctx);
    let brokered: Rc<dyn Fn(&str) -> bool> = Rc::new(move |workspace_id: &str| {
        let Some(ctx) = judge.upgrade() else { return false };
        let entry = ctx.live.borrow().get(workspace_id).cloned();
        entry.is_some_and(|entry| brokered_record(&ctx, &entry.borrow().record.borrow()))
    });
    ctx.pipe.fill(pipe_area(ctx.clone(), brokered));
}

/// The idle policy over the machines area's own callbacks (machines.ts:789-838): the cycle runs through weak handles.
fn idle_of(ctx: &Rc<Ctx>, machines: &Rc<MachinesArea>) -> IdlePolicy {
    let window = Rc::downgrade(machines);
    let nap = Rc::downgrade(machines);
    let busy = Rc::downgrade(machines);
    let backstop = Rc::downgrade(machines);
    create_idle_policy(IdlePolicyOptions {
        window_of: Rc::new(move |id: &str| window.upgrade()?.idle_window_of_id(id).flatten().map(|ms| ms as i64)),
        on_idle: Rc::new(move |id: &str, window_ms: i64| {
            let (area, id) = (nap.upgrade(), id.to_string());
            async move {
                match area {
                    Some(area) => area.on_idle(&id, window_ms as f64).await.map_err(|e| e.message),
                    None => Ok(()),
                }
            }
            .boxed_local()
        }),
        busy: Some(Rc::new(move |id: &str| busy.upgrade().is_some_and(|area| area.idle_busy(id)))),
        retry_ms: ctx.opts.status.as_ref().and_then(|s| s.poll_interval_ms).unwrap_or(POLL_INTERVAL_MS),
        on_backstop: Some(Rc::new(move |id: &str, until_ms: i64| {
            if let Some(area) = backstop.upgrade() {
                area.on_backstop(id, until_ms);
            }
        })),
        clock: Some(ctx.clock.clone()),
    })
}

/// `opts.recipes` as the place door reads it (the door's `RecipeResolver` over the shelf's `resolve` and `resolve_file`).
struct RecipeShelfResolver(Rc<dyn RecipeShelf>);

#[async_trait::async_trait(?Send)]
impl wsp_machines::places::types::RecipeResolver for RecipeShelfResolver {
    async fn resolve(&self, slug: &str) -> Result<Resolved, HostError> {
        let r = self.0.resolve(slug).await?;
        Ok(Resolved { file: r.file, items: r.items, hash: r.hash })
    }
    async fn resolve_file(&self, file: &RecipeFile) -> Result<Resolved, HostError> {
        let r = self.0.resolve_file(file.clone()).await?;
        Ok(Resolved { file: r.file, items: r.items, hash: r.hash })
    }
}
