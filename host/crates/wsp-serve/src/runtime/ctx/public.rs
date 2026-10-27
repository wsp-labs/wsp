// SPDX-License-Identifier: AGPL-3.0-only
//! What the core, the public members and the harness and status reads ask of the runtime (`CoreCtx`, `PublicCtx`,
//! `HarnessesCtx` and `StatusCtx`, runtime.ts:491-992 and 1017-1277): the context's own maps, a forward to the area
//! that holds each call, and the few lines no area has (the close's steps, the model lists' shaping).
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{join_all, LocalBoxFuture};
use futures::FutureExt as _;
use wsp_core::clock::Clock;
use wsp_core::devices::DeviceDoor;
use wsp_core::project_landing::{read_unsaved, RunHere};
use wsp_core::types::api::WorkspacesApi;
use wsp_core::types::events::EventBus;
use wsp_core::types::internal::{is_absent_machine, BUILDERS, WORKSPACES};
use wsp_core::types::wiring::AdoptedMachine;
use wsp_engine::errors::is_missing;
use wsp_engine::labels::CREATED_AT_LABEL;
use wsp_engine::machine::MachineBackend;
use wsp_engine::orphans::{reap, ListedMachine, ReapFailure, ReapOptions, ReapResult, ReapedMachine};
use wsp_machines::machines::boot::entries::EntryAsk;
use wsp_machines::machines::machines::Entry;
use wsp_machines::projects::branch_of_record::OnRecordEntry;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::{Preferences, PreferencesPatch, ProjectLook};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::{HostError, Kind};
use wsp_words::place_word::provider_key_name;
use wsp_words::thread_defaults::shape_models;
use wsp_words::words::thread::GoneSeenBy;

use super::Ctx;
use crate::runtime::core::{CoreCtx, LaunchReach, StandingEntry};
use crate::runtime::join::spawn_detached;
use crate::runtime::public::{CloseStep, HarnessesCtx, MachineRead, PublicCtx, StatusCtx, SweptBuilder, SweptEntry};

/// A thing the close waits on that nothing holds: the state's own joined futures, by name.
type Held = futures::future::Shared<LocalBoxFuture<'static, ()>>;

impl Ctx {
    /// The live entry of a workspace, where this host holds one.
    fn public_entry(&self, workspace_id: &str) -> Option<Entry> {
        self.live.borrow().get(workspace_id).cloned()
    }

    /// `placeDoor.nameOf(placeId)`, else this computer's hostname or the provider's own name (builders.ts:242).
    fn public_place_name(&self, place: &str) -> String {
        match self.wired_door() {
            Some(door) => door.name_of(place),
            None if place == HERE_PLACE_ID => wsp_core::host_id::hostname(),
            None => provider_key_name(place),
        }
    }

    /// `ctx.backendAt(place)`: the wired backend for the wired place, else the place's own.
    fn public_backend_at(&self, place: &str) -> Result<Arc<dyn MachineBackend>, HostError> {
        if place == self.places.wired() {
            return Ok(self.backend.clone());
        }
        self.places.backend(place).ok_or_else(|| {
            let held: Vec<String> = self.places.list().iter().map(|p| provider_key_name(p)).collect();
            HostError::with_kind(wsp_wire::wire::machine_link::no_such_place_refusal(place, &held), Kind::Missing)
        })
    }

    /// The `(record id, project)` an entry is named by, where a harness list reads it.
    fn public_named(entry: &Entry) -> (String, String) {
        let e = entry.borrow();
        let r = e.record.borrow();
        (r.id.clone(), r.project.clone())
    }

    async fn public_reread_held(&self, workspace_id: &str, why: &str) -> Result<(), HostError> {
        let by: GoneSeenBy = serde_json::from_value(serde_json::Value::String(why.to_string()))
            .map_err(|_| HostError::new(format!("{why} is no reason to read a record again")))?;
        self.boot.get().reread_held(workspace_id, by).await
    }

    /// The joined futures of every sync a daemon has in flight: asked again, a machine's sync answers the one standing.
    fn public_daemon_syncs(&self) -> Vec<LocalBoxFuture<'static, Result<(), HostError>>> {
        let daemon = self.daemon.get();
        let flying = daemon.daemon_syncs();
        let entries: Vec<Entry> = self.live.borrow().values().cloned().collect();
        entries.iter().filter(|e| flying.contains(&e.borrow().machine.id().to_string())).map(|e| daemon.sync_daemon(e)).collect()
    }

    fn public_cancel_all(timers: &std::cell::RefCell<indexmap::IndexMap<String, wsp_core::clock::Cancel>>) {
        let stops: Vec<wsp_core::clock::Cancel> = timers.borrow_mut().drain(..).map(|(_, stop)| stop).collect();
        for stop in stops {
            stop();
        }
    }
}

#[async_trait(?Send)]
impl CoreCtx for Ctx {
    fn bus(&self) -> Rc<dyn EventBus> {
        self.bus.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn devices(&self) -> Rc<dyn DeviceDoor> {
        self.device_door.clone()
    }
    fn host_key(&self) -> Option<String> {
        self.wired_door().map(|door| door.host_key())
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.booted().await
    }
    fn agents_reach(&self, workspace_id: &str) -> Option<LaunchReach> {
        let entry = self.public_entry(workspace_id)?;
        let reach = self.rules.get().agents_reach(&entry)?;
        Some(LaunchReach {
            url: reach.get("url").and_then(|v| v.as_str()).map(str::to_string),
            wsp: reach.get("wsp").and_then(|v| serde_json::from_value(v.clone()).ok()),
        })
    }
    fn brokered(&self, workspace_id: &str) -> bool {
        let Some(entry) = self.public_entry(workspace_id) else {
            return false;
        };
        let record = entry.borrow().record.borrow().clone();
        self.kinds.get().brokered(&record)
    }
    fn record_name(&self, workspace_id: &str) -> String {
        self.public_entry(workspace_id).map(|e| e.borrow().record.borrow().name.clone()).unwrap_or_default()
    }
    async fn reread_public(&self, workspace_id: &str) -> Result<(), HostError> {
        let Some(entry) = self.public_entry(workspace_id) else {
            return Ok(());
        };
        let on = {
            let e = entry.borrow();
            OnRecordEntry {
                record: e.record.clone(),
                opens_thread: e.opens_thread.clone(),
                of_record_held: e.of_record_held,
                machine: e.machine.clone(),
            }
        };
        self.branch_area().reread_public(workspace_id, &on).await
    }
    async fn read_broker(&self) -> Result<(), HostError> {
        self.kinds.get().read_broker().await;
        Ok(())
    }
    async fn daemon_settled(&self, workspace_id: &str) -> Result<(), HostError> {
        let Some(entry) = self.public_entry(workspace_id) else {
            return Ok(());
        };
        self.daemon.get().daemon_settled(&entry).await;
        Ok(())
    }
    async fn read_public_marketplaces(&self) -> Result<(), HostError> {
        self.branch_area().read_public_marketplaces().await;
        Ok(())
    }
    fn turn_road(&self, kind: WorkspaceKind) -> Result<WorkspaceOrigin, HostError> {
        Ok(self.kinds.get().module_of(kind)?.turn_road())
    }
    fn live(&self) -> Vec<StandingEntry> {
        self.live
            .borrow()
            .values()
            .map(|entry| {
                let e = entry.borrow();
                let r = e.record.borrow();
                StandingEntry {
                    id: r.id.clone(),
                    name: r.name.clone(),
                    kind: r.kind,
                    place: r.place.clone(),
                    project: r.project.clone(),
                    running: r.phase == wsp_wire::views::workspace::WorkspacePhase::Running,
                    machine_id: r.machine_id.clone(),
                    sub: r.sub.is_some(),
                }
            })
            .collect()
    }
    fn projects_held(&self) -> Vec<ProjectView> {
        self.projects_held.borrow().values().cloned().collect()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.thread_records.borrow().iter().map(|(thread, record)| (thread.clone(), record.workspace_id.clone())).collect()
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.sessions.borrow().values().map(|row| row.borrow().view.clone()).collect()
    }
    fn cap_held(&self, turn_id: &str) -> bool {
        self.threads.get().cap_held.borrow().contains_key(turn_id)
    }
    fn snoozed_until(&self, thread_id: &str) -> Option<f64> {
        self.thread_records.borrow().get(thread_id)?.snoozed_until.as_ref().map(|at| at.0)
    }
    fn limit_resume_at(&self, thread_id: &str) -> Option<f64> {
        self.thread_records.borrow().get(thread_id)?.limit_resume.as_ref().map(|resume| resume.at.0)
    }
    fn thread_workspace(&self, thread_id: &str) -> Option<String> {
        self.thread_records.borrow().get(thread_id).map(|record| record.workspace_id.clone())
    }
    async fn resume_after_limit(&self, thread_id: &str) -> Result<(), HostError> {
        self.boot.get().resume_after_limit(thread_id).await
    }
    async fn wake(&self, workspace_id: &str) -> Result<(), HostError> {
        self.workspaces.get().wake(workspace_id, None).await.map(|_| ())
    }
    async fn exec(&self, workspace_id: &str, cmd: &str, timeout_ms: u64) -> Result<wsp_wire::wire::machine_link::ExecResult, HostError> {
        self.workspaces.get().exec(workspace_id, cmd, Some(timeout_ms), None).await
    }
    async fn delete_workspace(&self, workspace_id: &str) -> Result<(), HostError> {
        self.workspaces.get().delete(workspace_id, None).await
    }
    async fn unsaved_fork(&self, workspace_id: &str, name: &str) -> Result<Option<String>, HostError> {
        let Some(entry) = self.public_entry(workspace_id) else {
            return Ok(None);
        };
        let record = entry.borrow().record.borrow().clone();
        let checkout = self.channels.get().checkout_of(&record)?;
        let workspaces = self.workspaces.get();
        let id = workspace_id.to_string();
        // Inside the fork, woken first where it naps: the daemon's own read of a stopped copy counts only its
        // checked-out branch and none of its edits.
        let run: RunHere = Rc::new(move |cmd: String, timeout_ms: u64| {
            let (workspaces, id, entry) = (workspaces.clone(), id.clone(), entry.clone());
            async move {
                if entry.borrow().record.borrow().phase != wsp_wire::views::workspace::WorkspacePhase::Running {
                    workspaces.wake(&id, None).await?;
                }
                workspaces.exec(&id, &cmd, Some(timeout_ms), None).await
            }
            .boxed_local()
        });
        Ok(read_unsaved(name, &checkout, run).await)
    }
    async fn unsaved_project(&self, project: &ProjectView) -> Result<Option<String>, HostError> {
        self.projects.get().unsaved(project).await
    }
    async fn remove_project(&self, project_id: &str, force: bool, keep_look: bool) -> Result<(), HostError> {
        let words = wsp_account::projects::projects::door::RemoveWords { force, check: false, keep_look };
        self.projects.get().remove_with(project_id, None, words).await.map(|_| ())
    }
    fn end_sessions(&self, workspace_id: &str, reason: &str) {
        self.machines.get().end_sessions(workspace_id, reason);
    }
    async fn drop_record(&self, workspace_id: &str) -> Result<(), HostError> {
        self.machines.get().drop(workspace_id).await
    }
    async fn store_delete_project(&self, project_id: &str) -> Result<(), HostError> {
        self.projects_held.borrow_mut().shift_remove(project_id);
        self.store.delete(wsp_core::types::internal::PROJECTS, project_id).await
    }
    async fn forget_icon(&self, project_id: &str) -> Result<(), HostError> {
        self.project_icons.get().forget(project_id).await
    }
    async fn project_look(&self, project_id: &str) -> Result<Option<ProjectLook>, HostError> {
        let held = self.preferences.get().get().await?;
        Ok(held.project_look.get(project_id).cloned())
    }
    async fn set_project_look(&self, project_id: &str, look: Option<ProjectLook>) -> Result<(), HostError> {
        let look = serde_json::to_value(look).map_err(|e| HostError::new(e.to_string()))?;
        let patch: PreferencesPatch = serde_json::from_value(serde_json::json!({ "projectLook": { project_id: look } }))
            .map_err(|e| HostError::new(e.to_string()))?;
        self.preferences.get().set(patch).await?;
        Ok(())
    }
    async fn set_icon(&self, project_id: &str, image: Option<String>) -> Result<(), HostError> {
        let icons = self.project_icons.get();
        match image {
            Some(hash) => icons.wear(project_id, Some(hash)).await,
            None => icons.set(project_id, None).await.map(|_| ()),
        }
    }
    fn keep_current(&self, place_id: &str) {
        let (image, place) = (self.image.get(), place_id.to_string());
        spawn_detached(async move {
            image.keep_current(&place, None).await;
        });
    }
    /// The TS reads `placeDoor!.nameOf` and throws where the host wired no door; the plain method names the place as
    /// the builders' own `placeName` does.
    fn place_name(&self, place_id: &str) -> String {
        self.public_place_name(place_id)
    }
    async fn held_away_on(&self, place_id: &str) -> Result<Vec<String>, HostError> {
        let boot = self.boot.get();
        let rows = self.store.list(WORKSPACES).await?;
        Ok(rows
            .iter()
            .filter(|raw| raw["place"].as_str() == Some(place_id))
            .filter_map(|raw| raw["id"].as_str())
            .filter(|id| boot.is_held_away(id))
            .map(str::to_string)
            .collect())
    }
    async fn hydrate(&self, workspace_id: &str) -> Result<bool, HostError> {
        let Some(raw) = self.store.get(WORKSPACES, workspace_id).await? else {
            return Ok(false);
        };
        Ok(self.boot.get().hydrate_workspace(raw, None, None, None).await?.is_some())
    }
    fn sync_daemon(&self, workspace_id: &str) {
        let Some(entry) = self.public_entry(workspace_id) else {
            return;
        };
        let sync = self.daemon.get().sync_daemon(&entry);
        spawn_detached(async move {
            if let Err(e) = sync.await {
                wsp_core::console::warn(&e.message);
            }
        });
    }
    async fn reread_held(&self, workspace_id: &str, why: &str) -> Result<(), HostError> {
        self.public_reread_held(workspace_id, why).await
    }
    fn held_on(&self, place_id: &str) -> Vec<String> {
        self.rules
            .get()
            .held()
            .iter()
            .map(|entry| {
                let e = entry.borrow();
                let r = e.record.borrow();
                (r.place.clone(), r.id.clone())
            })
            .filter(|(place, _)| place.as_deref() == Some(place_id))
            .map(|(_, id)| id)
            .collect()
    }
    async fn poll(&self, workspace_ids: Vec<String>) -> Result<(), HostError> {
        self.status_tracker.get().poll(&workspace_ids).await;
        Ok(())
    }
    fn place_away(&self, place_id: &str) -> bool {
        match self.wired_door() {
            Some(door) => self.places.backend(place_id).is_none() && door.link(place_id).is_none(),
            None => false,
        }
    }
    fn emit(&self, event: EventUnion) -> Result<(), HostError> {
        self.bus.emit(event).map(|_| ())
    }
}

#[async_trait(?Send)]
impl PublicCtx for Ctx {
    async fn ready(&self) -> Result<(), HostError> {
        self.booted().await
    }
    fn set_say_stops(&self, say: Rc<dyn Fn(&str)>) {
        self.state.with(|state| state.say_stops = Some(say));
    }
    fn wall_now(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
    }
    fn clock_now(&self) -> i64 {
        self.clock.now()
    }
    async fn refresh_builders(&self) -> Result<(), HostError> {
        self.boot.get().refresh_builders().await
    }
    async fn expire_grace(&self) -> Result<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError> {
        let expired = self.builders_area.get().expire_grace().await.map_err(|e| e.into_host())?;
        Ok((expired.reaped, expired.failed))
    }
    fn builders(&self) -> Vec<SweptBuilder> {
        self.builders
            .borrow()
            .values()
            .map(|held| {
                let b = held.borrow();
                SweptBuilder {
                    id: b.record.id.clone(),
                    life: b.life,
                    created_label: b.builder.machine.labels().and_then(|labels| labels.get(CREATED_AT_LABEL)).cloned(),
                    created_at: b.record.created_at.clone(),
                    building: b.record.building == Some(true),
                    place: b.record.place.clone(),
                    heartbeat_held: b.record.held_by.is_some(),
                }
            })
            .collect()
    }
    async fn hold(&self, builder_id: &str) -> Result<(), HostError> {
        let held = self.builders.borrow().get(builder_id).cloned();
        match held {
            Some(held) => self.builders_area.get().hold(&held).await,
            None => Ok(()),
        }
    }
    async fn stop_builder(&self, builder_id: &str, place: &str) -> Result<(), HostError> {
        let Some(held) = self.builders.borrow().get(builder_id).cloned() else {
            return Ok(());
        };
        let backend = self.public_backend_at(place)?;
        let machine = held.borrow().builder.machine.clone();
        self.gone.stop(backend, machine).await.map_err(HostError::from)
    }
    async fn forget_builder(&self, builder_id: &str) -> Result<(), HostError> {
        self.builders_area.get().forget_builder(builder_id).await
    }
    async fn replay_creates(
        &self,
        known: &dyn Fn() -> Vec<String>,
        failed: &mut Vec<ReapFailure>,
    ) -> Result<Vec<ReapedMachine>, HostError> {
        self.machines.get().replay_creates(known, failed).await
    }
    fn sweep_places(&self) -> Vec<(String, bool)> {
        let wired = self.places.wired();
        let rest = self.places.list().into_iter().filter(|place| *place != wired);
        let answers = |place: &str| place == wired || self.places.backend(place).is_some();
        std::iter::once(wired.clone()).chain(rest).map(|place| (place.clone(), answers(&place))).collect()
    }
    fn wired_place(&self) -> String {
        self.places.wired()
    }
    fn place_name(&self, place: &str) -> String {
        self.public_place_name(place)
    }
    async fn list_at(&self, place: &str) -> Result<Vec<ListedMachine>, HostError> {
        self.public_backend_at(place)?.list(None).await.map_err(HostError::from)
    }
    fn entries(&self) -> Vec<SweptEntry> {
        self.live
            .borrow()
            .values()
            .map(|entry| {
                let e = entry.borrow();
                let r = e.record.borrow();
                SweptEntry {
                    id: r.id.clone(),
                    machine_id: r.machine_id.clone(),
                    entry_machine_id: e.machine.id().to_string(),
                    creating: e.creating,
                    napping: e.napping.is_some(),
                    waking: e.waking.is_some(),
                    deleting: e.deleting.is_some(),
                    absent_machine: is_absent_machine(&*e.machine),
                    phase: r.phase,
                }
            })
            .collect()
    }
    async fn reread_held(&self, workspace_id: &str, why: &str) -> Result<(), HostError> {
        self.public_reread_held(workspace_id, why).await
    }
    async fn recover_gone(&self, workspace_id: &str) -> Result<(), HostError> {
        let entry = self.boot.get().entry_of(workspace_id, None, EntryAsk::default()).await?;
        self.machines.get().recover_gone(&entry).await.map(|_| ())
    }
    async fn read_machine(&self, workspace_id: &str) -> MachineRead {
        let Some(entry) = self.public_entry(workspace_id) else {
            return MachineRead::Failed;
        };
        let machine = entry.borrow().machine.clone();
        match machine.state().await {
            // The sweep reads `gone` the same whether the provider answers it or answers a missing machine
            // (runtime.ts:1161-1166), the second with the provider's words.
            Ok(wsp_engine::machine::MachineState::Gone) => MachineRead::Missing { answer: None },
            Ok(state) => MachineRead::State(state),
            Err(e) if is_missing(&e) => MachineRead::Missing { answer: Some(wsp_core::status::provider_said(&e)) },
            Err(_) => MachineRead::Failed,
        }
    }
    async fn adopt_gone(&self, workspace_id: &str, words: String) -> Result<(), HostError> {
        let Some(entry) = self.public_entry(workspace_id) else {
            return Ok(());
        };
        self.machines.get().adopt_gone(&entry, &words).await
    }
    fn known_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .live
            .borrow()
            .values()
            .flat_map(|entry| {
                let e = entry.borrow();
                let ids = [e.record.borrow().machine_id.clone(), e.machine.id().to_string()];
                ids
            })
            .collect();
        ids.extend(self.builders.borrow().keys().cloned());
        ids.extend(self.machines.get().inflight());
        ids.extend(self.gone.ids());
        ids
    }
    async fn adopt_lost(
        &self,
        place: &str,
        rows: &[ListedMachine],
        claimed: &mut Vec<String>,
        failed: &mut Vec<ReapFailure>,
    ) -> Result<(Vec<AdoptedMachine>, Vec<ReapedMachine>), HostError> {
        let listing: Vec<wsp_account::account::status::ListedRow> = rows
            .iter()
            .map(|row| wsp_account::account::status::ListedRow {
                id: row.id.clone(),
                state: serde_json::to_value(row.state).ok().and_then(|word| word.as_str().map(str::to_string)).unwrap_or_default(),
                labels: row.labels.clone(),
            })
            .collect();
        let mut known: BTreeSet<String> = claimed.iter().cloned().collect();
        let out = self.status.get().adopt_lost(&listing, &mut known, failed, place).await?;
        for id in known {
            if !claimed.contains(&id) {
                claimed.push(id);
            }
        }
        Ok(out)
    }
    async fn engine_reap(
        &self,
        place: &str,
        rows: Vec<ListedMachine>,
        known: Vec<String>,
        older_than_ms: Option<i64>,
    ) -> Result<ReapResult, HostError> {
        let backend = self.public_backend_at(place)?;
        let (gone, at) = (self.gone.clone(), backend.clone());
        let opts = ReapOptions {
            backend,
            listing: Some(rows),
            known_ids: Box::new(move || known.clone()),
            owner: self.state.with(|state| state.owner.clone()),
            stop: Arc::new(move |machine| {
                let (gone, at) = (gone.clone(), at.clone());
                async move { gone.stop(at, machine).await }.boxed_local()
            }),
            older_than_ms,
            now: None,
        };
        reap(opts).await.map_err(HostError::from)
    }
    async fn sweep_strays(
        &self,
        known: Vec<String>,
        failed: &mut Vec<ReapFailure>,
        places: &[String],
    ) -> Result<Vec<ReapedMachine>, HostError> {
        let backends = places.iter().map(|place| self.public_backend_at(place)).collect::<Result<Vec<_>, _>>()?;
        let known = move || known.clone();
        self.machines.get().sweep_strays(&known, failed, &backends).await
    }
    async fn close_step(&self, step: CloseStep) -> Result<(), HostError> {
        match step {
            CloseStep::MarkClosing => self.state.with(|state| state.closing = true),
            CloseStep::ClearRootsRecheck => {
                if let Some(stop) = self.state.with(|state| state.roots_recheck.take()) {
                    stop();
                }
            }
            CloseStep::RunHeldAtClose => {
                let writes: Vec<Rc<dyn Fn()>> = self.held_at_close.borrow().clone();
                for write in writes {
                    write();
                }
                self.held_at_close.borrow_mut().clear();
            }
            CloseStep::AwaitCopiesMoving => {
                if let Some(moving) = self.state.with(|state| state.copies_moving.clone()) {
                    moving.await;
                }
            }
            CloseStep::StopSweep => {
                self.state.with(|state| state.sweep_stopped = true);
                if let Some(stop) = self.state.with(|state| state.sweep_timer.take()) {
                    stop();
                }
            }
            CloseStep::AwaitSweeping => {
                if let Some(sweeping) = self.state.with(|state| state.sweeping.clone()) {
                    sweeping.await;
                }
            }
            CloseStep::StopDiskWatch => self.reach.get().stop_disk_watch().await,
            CloseStep::CloseIdle => self.idle.get().close(),
            // The plan alerts are the usage packet's; the context holds no slot for them to be closed through.
            CloseStep::CloseAlerts => {}
            CloseStep::CloseSlates => self.slates.get().close(),
            // `opts.agents_reader` is a marker with no close of its own until the agents reader is bridged (packet 16).
            CloseStep::CloseAgentsReader => {}
            CloseStep::SettleDaemonSyncs => {
                join_all(self.public_daemon_syncs()).await;
            }
            CloseStep::SettleKeptWrites => {
                let writes: Vec<Held> = self.transcripts.get().kept_writes.borrow().values().cloned().collect();
                join_all(writes).await;
            }
            CloseStep::SettleAgentAsks => {
                join_all(self.agents.get().agent_asks()).await;
            }
            // The readers a turn on a machine opens register no stop with the kinds area, so there is none to call.
            CloseStep::StopMachineReadings => self.kinds.get().machine_reading.borrow_mut().clear(),
            CloseStep::ClosePipes => {
                if let Some(pipes) = self.pipe.get() {
                    pipes.close();
                }
            }
            CloseStep::CloseLocal => {
                let close = self.opts.local.as_ref().and_then(|local| local.close.clone());
                if let Some(close) = close {
                    close().await?;
                }
            }
            CloseStep::ClosePlaceDoor => {
                if let Some(door) = self.wired_door() {
                    door.close().await?;
                }
            }
            CloseStep::MarkClosed => self.state.with(|state| state.closed = true),
            CloseStep::CancelBeat => {
                if let Some(beat) = self.state.with(|state| state.beat.take()) {
                    beat();
                }
            }
            CloseStep::CancelTimers => {
                Ctx::public_cancel_all(&self.builders_area.get().grace_timers);
                Ctx::public_cancel_all(&self.snooze_timers);
                Ctx::public_cancel_all(&self.resume_timers);
            }
            CloseStep::StopPrPolls => {
                let entries: Vec<Entry> = self.live.borrow().values().cloned().collect();
                for entry in entries {
                    let poll = entry.borrow_mut().pr_poll.take();
                    if let Some(stop) = poll {
                        stop();
                    }
                }
            }
            CloseStep::CloseGoneWatch => self.gone.close(),
            CloseStep::AwaitTicking => {
                if let Some(ticking) = self.state.with(|state| state.ticking.clone()) {
                    ticking.await;
                }
            }
            CloseStep::FreeOwnBuilders => {
                let own: Vec<_> = self
                    .builders
                    .borrow()
                    .values()
                    .filter(|held| {
                        held.borrow().life == wsp_core::types::internal::BuilderLife::Own && held.borrow().record.held_by.is_some()
                    })
                    .cloned()
                    .collect();
                for held in own {
                    let record = {
                        let mut b = held.borrow_mut();
                        b.record.held_by = None;
                        (b.record.id.clone(), serde_json::to_value(&b.record).map_err(|e| HostError::new(e.to_string()))?)
                    };
                    self.store.put(BUILDERS, &record.0, record.1).await?;
                }
            }
            CloseStep::FlushTranscripts => {
                let area = self.transcripts.get();
                let mut ids: Vec<String> = area.transcript_timers.borrow().keys().cloned().collect();
                for id in self.transcript_state.with(|t| t.pending.keys().cloned().collect::<Vec<_>>()) {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
                join_all(ids.iter().map(|id| area.flush_transcript(id))).await;
            }
            CloseStep::AwaitTranscriptWrites => {
                let mut writes: Vec<Held> = self.transcripts.get().transcript_queue.borrow().values().cloned().collect();
                writes.extend(self.index_flushes.with(|flushes| flushes.values().cloned().collect::<Vec<_>>()));
                join_all(writes).await;
            }
        }
        Ok(())
    }
    fn kept_agents(&self) -> Vec<String> {
        self.threads.get().kept_agents.borrow().keys().cloned().collect()
    }
    fn reap_kept(&self, thread_id: &str) -> LocalBoxFuture<'static, ()> {
        let threads = self.threads.get();
        let exited = threads.kept_agents.borrow().get(thread_id).map(|kept| kept.borrow().agent.exited());
        threads.reap_kept(thread_id, true);
        match exited {
            Some(exited) => exited.boxed_local(),
            None => async {}.boxed_local(),
        }
    }
}

#[async_trait(?Send)]
impl HarnessesCtx for Ctx {
    async fn preferences(&self) -> Result<Preferences, HostError> {
        Ok(self.preferences.get().get().await?)
    }
    fn talks_to_tree_on(&self, workspace_id: &str, origin: Option<&Caller>) -> bool {
        self.boot.get().talks_to_tree_on(workspace_id, origin).unwrap_or(false)
    }
    async fn entry_of(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<(String, String)>, HostError> {
        let entry = self.boot.get().entry_of(workspace_id, origin.as_ref(), EntryAsk::default()).await?;
        Ok(Some(Ctx::public_named(&entry)))
    }
    async fn project_home(&self, project: &str, origin: Option<Caller>) -> Result<Option<(String, String)>, HostError> {
        Ok(self.agents.get().project_home(project, origin).await?.map(|entry| Ctx::public_named(&entry)))
    }
    async fn computer_home(&self, computer: &str, origin: Option<Caller>) -> Result<Option<(String, String)>, HostError> {
        Ok(self.agents.get().computer_home(computer, origin).await?.map(|entry| Ctx::public_named(&entry)))
    }
    async fn agent_lists(&self, entry: Option<&(String, String)>) -> Result<Vec<HarnessCatalog>, HostError> {
        let held = entry.and_then(|(id, _)| self.public_entry(id));
        Ok(self.agents.get().agent_lists(held.as_ref()).await)
    }
    fn default_agent_of(&self, prefs: &Preferences, entry: Option<&(String, String)>) -> String {
        let held = entry.and_then(|(id, _)| self.public_entry(id));
        self.agents.get().default_agent_of(prefs, held.as_ref())
    }
    fn shaped(&self, catalog: &HarnessCatalog, prefs: &Preferences, project: Option<&str>) -> serde_json::Map<String, serde_json::Value> {
        let overrides = project.and_then(|id| prefs.project_defaults.get(id)).map(|fields| (**fields).clone());
        let defaults = wsp_threads::threads::agents::AgentsArea::defaults_on(catalog, prefs, overrides.as_ref());
        let own = prefs.agent_defaults.get(&catalog.harness).and_then(|fields| fields.models.as_ref());
        match serde_json::to_value(shape_models(&defaults.catalog, own)) {
            Ok(serde_json::Value::Object(shaped)) => shaped,
            _ => serde_json::Map::new(),
        }
    }
}

#[async_trait(?Send)]
impl StatusCtx for Ctx {
    async fn probe_list(
        &self,
        o: Option<wsp_core::status::StatusListOptions>,
    ) -> Result<Vec<wsp_wire::views::workspace_more::WorkspaceStatus>, HostError> {
        self.status_tracker.get().list(o.unwrap_or_default()).await.map_err(HostError::new)
    }
    fn listed_for(&self, origin: Option<&Caller>) -> Result<Vec<String>, HostError> {
        Ok(self.rules.get().listed_for(origin)?.iter().map(|entry| entry.borrow().record.borrow().id.clone()).collect())
    }
    fn refuse_named(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        self.rules.get().refuse_named(workspace_id, origin)
    }
    async fn history(&self, workspace_id: &str) -> Result<Vec<serde_json::Value>, HostError> {
        let points = self.status_tracker.get().history(workspace_id).await;
        points.iter().map(|point| serde_json::to_value(point).map_err(|e| HostError::new(e.to_string()))).collect()
    }
}
