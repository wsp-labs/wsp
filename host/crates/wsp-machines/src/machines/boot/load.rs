// SPDX-License-Identifier: AGPL-3.0-only
//! boot.ts:16-326 at 65f6b89c7: builders and hydration. Spec: spec/boot_load.md.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};

use wsp_core::types::internal::{
    absent_machine, dead_machine, gone_log_line, is_absent_machine, pid_alive, BuilderLife,
    BuilderRecord, HeldBy, LiveBuilder, StoredBuilder, BUILDERS, HELD_TTL_MS, WORKSPACES,
};
use wsp_core::types::pending::{Builder, Sighting};
use wsp_core::types::wiring::{FoundMachine, WorkspaceRecord};
use wsp_engine::errors::{is_missing, EngineError};
use wsp_engine::machine::{Machine, MachineBackend, MachineState};
use wsp_engine::orphans::date_parse;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::MachineKind;
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_words::exit::{HostError, Kind};
use wsp_words::words::thread::{gone_unconfirmed_line, gone_words, GoneSeenBy, GoneSighting};

use super::BootArea;
use crate::machines::area_error::AreaError;
use crate::machines::machines::create::{host_to_engine, GONE_READS};
use crate::machines::machines::Entry;

/// How long the load waits on the provider's reads of its records' machines, all together (boot.ts:18).
pub const LOAD_READS_MS: u64 = 1_500;

/// A provider read out, shared by the load's timer and the late load (boot.ts:149 `asked`).
pub type Asked = futures::future::Shared<
    futures::future::LocalBoxFuture<'static, Result<Arc<dyn Machine>, HostError>>,
>;

/// The load's one deadline and every read still out at it, by record id (boot.ts:21).
pub struct LoadReads {
    pub until: i64,
    pub late: std::cell::RefCell<indexmap::IndexMap<String, Asked>>,
}

/// `lateReadLine` (boot.ts:27).
pub fn late_read_line(machine_id: &str) -> String {
    format!("the provider has not yet answered for {machine_id}; it is read in once it does")
}

/// A provider's said line as `providerSaid` reads it, from a host error (status.ts:49).
fn said_of(e: &HostError) -> String {
    match e.status {
        Some(status) => format!("{} {}", status as i64, e.message),
        None => e.message.clone(),
    }
}

/// The kind a record's module is keyed by (`kindWords(kind)`).
fn kind_key(kind: wsp_wire::views::workspace_kind::WorkspaceKind) -> &'static str {
    match kind {
        wsp_wire::views::workspace_kind::WorkspaceKind::Cloud => "cloud",
        wsp_wire::views::workspace_kind::WorkspaceKind::Local => "local",
        wsp_wire::views::workspace_kind::WorkspaceKind::Place => "place",
    }
}

/// A machine state's provider word, as a line quotes it (`String(state)`).
fn state_word(state: Option<MachineState>) -> &'static str {
    match state {
        Some(MachineState::Starting) => "starting",
        Some(MachineState::Running) => "running",
        Some(MachineState::Paused) => "paused",
        Some(MachineState::Gone) => "gone",
        None => "undefined",
    }
}

/// One read as `readOnce` makes it (golden.ts:130): the handle `get` found and the state it carried, or one state read
/// on it; a 404 from either is a gone read.
async fn read_once(
    at: &Arc<dyn MachineBackend>,
    id: &str,
) -> Result<(MachineState, Option<Arc<dyn Machine>>), EngineError> {
    let found = match at.get(id).await {
        Ok(machine) => match machine.seen().map(|s| s.state) {
            Some(state) => Ok((state, Some(machine))),
            None => machine.state().await.map(|state| (state, Some(machine))),
        },
        Err(e) => Err(e),
    };
    match found {
        Err(e) if is_missing(&e) => Ok((MachineState::Gone, None)),
        other => other,
    }
}

/// `sightMachine` (golden.ts:138): gone only when every one of GONE_READS reads in a row said gone, else the first read
/// that found the machine, with its handle.
async fn sight(
    at: &Arc<dyn MachineBackend>,
    id: &str,
) -> Result<(MachineState, Option<Arc<dyn Machine>>), EngineError> {
    for _ in 0..GONE_READS {
        let seen = read_once(at, id).await?;
        if seen.0 != MachineState::Gone {
            return Ok(seen);
        }
    }
    Ok((MachineState::Gone, None))
}

impl BootArea {
    /// A stored row read into the type the Rust host keeps it in. The TS reads every row as it stands, so a row that
    /// does not read here is warned and left in the store, never a reason to stop the start.
    pub(crate) fn row_of<T: serde::de::DeserializeOwned>(
        &self,
        label: &str,
        raw: Value,
    ) -> Option<T> {
        match serde_json::from_value(raw) {
            Ok(row) => Some(row),
            Err(e) => {
                self.rt.warn(&format!("{label} is left as it was: {e}"));
                None
            }
        }
    }

    /// `sealableAt` (boot.ts:44), spec rule 2.
    pub fn sealable_at(&self, place: Option<&str>, first_life: bool) -> bool {
        if first_life {
            return true;
        }
        let wired = self.rt.places().wired();
        let at = self
            .rt
            .places()
            .backend(place.unwrap_or(&wired))
            .or_else(|| self.rt.door_backend_of(place.unwrap_or(&wired)));
        at.is_some_and(|at| at.capabilities().snapshots_any_life)
    }

    /// `lifeOf` (boot.ts:49), spec rule 3. `labels` are the machine's provider labels.
    pub fn life_of(
        &self,
        stored: &StoredBuilder,
        labels: Option<&IndexMap<String, String>>,
        sealable: bool,
    ) -> BuilderLife {
        self.life_from(
            stored.held_by.as_ref(),
            stored.building == Some(true),
            labels,
            sealable,
        )
    }

    /// `lifeOf` over the record's fields: the first rule that holds decides (boot.ts:49-61).
    fn life_from(
        &self,
        holder: Option<&HeldBy>,
        building: bool,
        labels: Option<&IndexMap<String, String>>,
        sealable: bool,
    ) -> BuilderLife {
        let owner = self.rt.owner();
        let foreign = labels
            .and_then(|l| l.get(wsp_engine::labels::OWNER_LABEL))
            .is_some_and(|label| *label != owner);
        let held = holder.is_some_and(|h| {
            let mine = h.host == self.config.host_id && h.pid == std::process::id();
            let beat_age = self.rt.clock().now() as f64 - date_parse(&h.heartbeat);
            let fresh = beat_age.is_nan() || beat_age < HELD_TTL_MS as f64;
            !mine && fresh && (h.host != self.config.host_id || pid_alive(h.pid as i32))
        });
        if foreign {
            BuilderLife::Foreign
        } else if held {
            BuilderLife::Held
        } else if !sealable || building {
            BuilderLife::Stale
        } else {
            BuilderLife::Reusable
        }
    }

    /// `liveOf` (boot.ts:62): a stored builder with its machine, its life and whether it can be sealed.
    fn live_builder_of(&self, record: BuilderRecord, machine: Arc<dyn Machine>) -> LiveBuilder {
        let sealable = self.sealable_at(record.place.as_deref(), record.first_life);
        let labels = machine.labels().cloned();
        let life = self.life_from(
            record.held_by.as_ref(),
            record.building == Some(true),
            labels.as_ref(),
            sealable,
        );
        LiveBuilder {
            builder: Builder {
                machine,
                kind: record.kind,
                base_template: record.base_template.clone(),
                setup_sha: record.setup_sha.clone(),
                created_at: record.created_at.clone(),
                first_life: record.first_life,
                size: record.size.clone(),
                import: record.import.clone(),
                parent_snapshot_id: record.parent_snapshot_id.clone(),
                base: record.base.clone(),
                retired: record.retired.clone(),
            },
            record,
            sealable,
            life,
            reach: None,
            recipe: None,
        }
    }

    /// `admit` (boot.ts:78), spec rule 5.
    async fn admit(self: &Rc<Self>, stored: StoredBuilder) -> Result<(), HostError> {
        let wired = self.rt.places().wired();
        let place = stored.place.clone().unwrap_or(wired);
        let at = match self
            .rt
            .places()
            .backend(&place)
            .or_else(|| self.rt.door_backend_of(&place))
        {
            Some(at) => at,
            None => return Ok(()),
        };
        let machine = match at.get(&stored.id).await {
            Ok(machine) => self.rt.observed(machine),
            Err(e) => {
                if matches!(
                    e,
                    EngineError::PlaceAbsent { .. } | EngineError::NoProvider { .. }
                ) {
                    return Ok(());
                }
                if !is_missing(&e) {
                    return Err(host_to_engine_back(e));
                }
                let (state, _) = sight(&at, &stored.id).await.map_err(host_to_engine_back)?;
                if state == MachineState::Gone {
                    self.rt.store().delete(BUILDERS, &stored.id).await?;
                }
                return Ok(());
            }
        };
        if self.rt.closed() {
            return Ok(());
        }
        let state = match machine.seen().map(|s| s.state) {
            Some(state) => state,
            None => machine.state().await.map_err(host_to_engine_back)?,
        };
        let first_life = stored.first_life == Some(true) && state == MachineState::Running;
        let size = match &stored.size {
            Some(size) => size.clone(),
            None => {
                let shape = self.rt.shape_of(&machine).await;
                let default = at.pricing().default_size();
                let asked = WorkspaceSize {
                    cpu: JsNumber(default.cpu),
                    mem_mb: JsNumber(default.mem_mb),
                };
                self.rt.size_built(shape.as_ref(), asked)
            }
        };
        let record = BuilderRecord {
            id: stored.id.clone(),
            name: stored.name.clone(),
            kind: stored.kind,
            base_template: stored.base_template.clone(),
            setup_sha: stored.setup_sha.clone(),
            created_at: stored.created_at.clone(),
            size,
            stream_url: stored.stream_url.clone(),
            first_life,
            held_by: stored.held_by.clone(),
            building: stored.building,
            import: stored.import.clone(),
            base: stored.base.clone(),
            parent_snapshot_id: stored.parent_snapshot_id.clone(),
            retired: stored.retired.clone(),
            sealed: stored.sealed.clone(),
            place: stored.place.clone(),
        };
        if stored.first_life == Some(true) && !first_life {
            self.rt
                .store()
                .put(
                    BUILDERS,
                    &record.id,
                    serde_json::to_value(&record).unwrap_or(Value::Null),
                )
                .await?;
        }
        let id = record.id.clone();
        let sealed_at = stored.sealed.as_ref().map(|s| s.at.clone());
        let live = self.live_builder_of(record, machine);
        self.rt
            .builders()
            .borrow_mut()
            .insert(id.clone(), Rc::new(RefCell::new(live)));
        if let Some(at) = sealed_at {
            self.rt.arm_grace(&id, &at);
        }
        Ok(())
    }

    /// `admitOnce` (boot.ts:111), spec rule 6: one read per stored id at a time, which a second caller shares.
    pub async fn admit_once(self: &Rc<Self>, stored: StoredBuilder) -> Result<(), HostError> {
        let id = stored.id.clone();
        let existing = self.admitting.borrow().get(&id).cloned();
        let read = match existing {
            Some(read) => read,
            None => {
                let area = self.clone();
                let key = id.clone();
                let read: futures::future::Shared<LocalBoxFuture<'static, Result<(), HostError>>> =
                    async move {
                        let out = area.admit(stored).await;
                        area.admitting.borrow_mut().shift_remove(&key);
                        out
                    }
                    .boxed_local()
                    .shared();
                self.admitting.borrow_mut().insert(id, read.clone());
                read
            }
        };
        read.await
    }

    /// `refreshBuilders` (boot.ts:126), spec rule 7: every call starts a numbered pass and becomes the latest.
    pub async fn refresh_builders(self: &Rc<Self>) -> Result<(), HostError> {
        let pass = self.passes.get() + 1;
        self.passes.set(pass);
        let area = self.clone();
        let refresh: futures::future::Shared<LocalBoxFuture<'static, Result<(), HostError>>> =
            async move { area.refresh_now(pass).await }
                .boxed_local()
                .shared();
        *self.latest.borrow_mut() = Some(refresh.clone());
        refresh.await
    }

    /// The pass's body; a pass a newer one has overtaken answers that one (boot.ts:130-147).
    async fn refresh_now(self: &Rc<Self>, pass: u64) -> Result<(), HostError> {
        let rows = self.rt.store().list(BUILDERS).await?;
        let mut seen: IndexSet<String> = IndexSet::new();
        for raw in rows {
            if pass != self.passes.get() {
                return self.await_latest().await;
            }
            let label = format!("builder {}", id_of(&raw));
            let Some(stored) = self.row_of::<StoredBuilder>(&label, raw) else {
                continue;
            };
            seen.insert(stored.id.clone());
            let current = self.rt.builders().borrow().get(&stored.id).cloned();
            match current {
                None => self.admit_once(stored).await?,
                Some(current) => {
                    let (life, machine, record_size, first) = {
                        let c = current.borrow();
                        (
                            c.life,
                            c.builder.machine.clone(),
                            c.record.size.clone(),
                            c.builder.first_life,
                        )
                    };
                    if life != BuilderLife::Own {
                        let mut record = stored.clone();
                        record.first_life = Some(stored.first_life == Some(true) && first);
                        if stored.size.is_none() {
                            record.size = Some(record_size);
                        }
                        let record_live = BuilderRecord {
                            id: record.id.clone(),
                            name: record.name.clone(),
                            kind: record.kind,
                            base_template: record.base_template.clone(),
                            setup_sha: record.setup_sha.clone(),
                            created_at: record.created_at.clone(),
                            size: record.size.clone().unwrap_or(WorkspaceSize {
                                cpu: JsNumber(0.0),
                                mem_mb: JsNumber(0.0),
                            }),
                            stream_url: record.stream_url.clone(),
                            first_life: record.first_life == Some(true),
                            held_by: record.held_by.clone(),
                            building: record.building,
                            import: record.import.clone(),
                            base: record.base.clone(),
                            parent_snapshot_id: record.parent_snapshot_id.clone(),
                            retired: record.retired.clone(),
                            sealed: record.sealed.clone(),
                            place: record.place.clone(),
                        };
                        let fresh = self.live_builder_of(record_live, machine);
                        let mut c = current.borrow_mut();
                        c.record = fresh.record;
                        c.builder = fresh.builder;
                        c.sealable = fresh.sealable;
                        c.life = fresh.life;
                    }
                }
            }
        }
        if pass != self.passes.get() {
            return self.await_latest().await;
        }
        let snapshot: Vec<(String, BuilderLife)> = self
            .rt
            .builders()
            .borrow()
            .iter()
            .map(|(id, b)| (id.clone(), b.borrow().life))
            .collect();
        for (id, life) in snapshot {
            if !seen.contains(&id) && life != BuilderLife::Own {
                self.rt.builders().borrow_mut().shift_remove(&id);
            }
        }
        Ok(())
    }

    /// The newest pass's answer, for a pass that a newer one overtook (boot.ts:127 `latest`).
    async fn await_latest(&self) -> Result<(), HostError> {
        let latest = self.latest.borrow().clone();
        match latest {
            Some(latest) => latest.await,
            None => Ok(()),
        }
    }

    /// `readWithin` (boot.ts:149), spec rule 8: at the load, a read still out at the deadline holds the record and is kept.
    async fn read_within(
        &self,
        at: &Arc<dyn MachineBackend>,
        stored: &WorkspaceRecord,
        load: Option<&Rc<LoadReads>>,
        held: &dyn Fn(HostError) -> Arc<dyn Machine>,
        asked: Asked,
    ) -> Result<Arc<dyn Machine>, HostError> {
        let Some(load) = load else { return asked.await };
        let _ = at;
        let (tx, rx) = oneshot::channel::<()>();
        let cancel = self.rt.clock().schedule(
            Box::new(move || {
                let _ = tx.send(());
            }),
            (load.until - self.rt.clock().now()).max(0),
        );
        let machine_id = stored.machine_id.clone();
        let read = asked.clone();
        futures::pin_mut!(read);
        let landed = futures::future::select(read, rx).await;
        match landed {
            futures::future::Either::Left((answer, _)) => {
                cancel();
                answer
            }
            futures::future::Either::Right(_) => {
                load.late.borrow_mut().insert(stored.id.clone(), asked);
                Ok(held(HostError::new(late_read_line(&machine_id))))
            }
        }
    }

    /// `loadLate` (boot.ts:170), spec rule 9.
    pub async fn load_late(
        self: &Rc<Self>,
        id: &str,
        read: Asked,
    ) -> Result<Option<Entry>, HostError> {
        self.rt.rereading().borrow_mut().insert(id.to_string());
        let mut landed = None;
        let _ = read.clone().await;
        let raw = self.rt.store().get(WORKSPACES, id).await;
        match raw {
            Ok(Some(raw)) if !self.rt.closed() => {
                match self.hydrate_workspace(raw, None, None, Some(read)).await {
                    Ok(entry) => landed = entry,
                    Err(e) => self.rt.warn(&format!(
                        "workspace {id} was not loaded once its provider answered: {}",
                        e.message
                    )),
                }
            }
            Ok(_) => {}
            Err(e) => self.rt.warn(&format!(
                "workspace {id} was not loaded once its provider answered: {}",
                e.message
            )),
        }
        self.rt.rereading().borrow_mut().shift_remove(id);
        if !self.rt.closed() && self.is_held_away(id) {
            self.reread_held(id, GoneSeenBy::RecordLoad).await?;
        }
        Ok(landed)
    }

    /// `isHeldAway` (boot.ts:190), spec rule 10.
    pub fn is_held_away(&self, id: &str) -> bool {
        let entry = self.rt.live().borrow().get(id).cloned();
        match entry {
            None => true,
            Some(entry) => {
                let machine = entry.borrow().machine.clone();
                is_absent_machine(machine.as_ref())
                    && entry.borrow().record.borrow().phase != WorkspacePhase::Queued
            }
        }
    }

    /// `rereadHeld` (boot.ts:200), spec rule 11.
    pub async fn reread_held(self: &Rc<Self>, id: &str, by: GoneSeenBy) -> Result<(), HostError> {
        let entry = self.rt.live().borrow().get(id).cloned();
        let Some(entry) = entry else { return Ok(()) };
        if !self.is_held_away(id) || self.rt.rereading().borrow().contains(id) {
            return Ok(());
        }
        self.rt.rereading().borrow_mut().insert(id.to_string());
        let out = self.reread_held_now(id, by, &entry).await;
        if !self.is_held_away(id) {
            self.held_answers.borrow_mut().shift_remove(id);
        }
        self.rt.rereading().borrow_mut().shift_remove(id);
        out
    }

    async fn reread_held_now(
        self: &Rc<Self>,
        id: &str,
        by: GoneSeenBy,
        entry: &Entry,
    ) -> Result<(), HostError> {
        let snapshot = entry.borrow().record.borrow().clone();
        let machine_id = snapshot.machine_id.clone();
        let at = self.rt.backend_for(&snapshot);
        tokio::task::yield_now().await;
        let seen = sight(&at, &machine_id).await;
        // `Promise.resolve().then(sight).catch(...)` answers turns after the sighting does: the promise the callback
        // returns is adopted in two, the catch passes it on in one, the `await` goes on in one, and the stored row is
        // read in one more.
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        let Ok((state, machine)) = seen else {
            return Ok(());
        };
        if self.rt.closed()
            || !self
                .rt
                .live()
                .borrow()
                .get(id)
                .is_some_and(|live| Rc::ptr_eq(live, entry))
        {
            return Ok(());
        }
        if let Some(machine) = machine {
            let raw = self.rt.store().get(WORKSPACES, id).await?;
            if let Some(raw) = raw {
                let found = FoundMachine {
                    sighting: Sighting::default(),
                    machine,
                };
                *self.sighted.borrow_mut() = Some(state);
                let again = self.hydrate_workspace(raw, Some(found), None, None).await;
                self.sighted.borrow_mut().take();
                if let Some(again) = again? {
                    self.rt.sync_daemon(&again);
                }
            }
            return Ok(());
        }
        let answer = self.held_answers.borrow().get(id).cloned();
        let sighting = GoneSighting {
            by,
            at: self.rt.clock().now() as f64,
            answer,
        };
        let words = gone_words(&machine_id, Some(&sighting));
        let attached = self
            .rt
            .attach(&entry.borrow().record.clone(), dead_machine(&machine_id));
        self.rt.mark_gone(&attached, &machine_id, &words).await?;
        Ok(())
    }

    /// `hydrateWorkspace` (boot.ts:229), spec rule 12. `asked` is a read already out (the late load's).
    pub async fn hydrate_workspace(
        self: &Rc<Self>,
        raw: serde_json::Value,
        seen: Option<FoundMachine>,
        load: Option<Rc<LoadReads>>,
        asked: Option<Asked>,
    ) -> Result<Option<Entry>, HostError> {
        let found = seen.is_some();
        let sighted = if found {
            self.sighted.borrow_mut().take()
        } else {
            None
        };
        let mut raw = raw;
        let unwired = raw
            .get("kind")
            .and_then(Value::as_str)
            .filter(|k| !matches!(*k, "cloud" | "local" | "place"))
            .map(str::to_string);
        if unwired.is_some() {
            raw["kind"] = json!("cloud");
        }
        let row_id = id_of(&raw);
        let stored: WorkspaceRecord = match record_of(raw) {
            Ok(stored) => stored,
            Err(e) => {
                self.rt.warn(&format!(
                    "workspace {row_id} is left as it was: {}",
                    e.message
                ));
                return Ok(None);
            }
        };
        let id = stored.id.clone();
        if stored.phase == WorkspacePhase::Queued {
            self.rt.attach_queued(&Rc::new(RefCell::new(stored)));
            return Ok(None);
        }
        if stored.phase == WorkspacePhase::Released
            || stored
                .sub
                .as_ref()
                .is_some_and(|s| s.cloud_gone == Some(true))
        {
            let copy = Rc::new(RefCell::new(stored.clone()));
            self.rt.attach(&copy, dead_machine(&stored.machine_id));
            return Ok(None);
        }
        if let Some(word) = unwired {
            self.rt.warn(&format!(
                "workspace {id} is left as it was: no module wired for {word}"
            ));
            return Ok(None);
        }
        let module = self.rt.module_of(kind_key(stored.kind));
        let at = match module.backend(&stored) {
            Ok(at) => at,
            Err(e) => {
                self.rt
                    .warn(&format!("workspace {id} is left as it was: {}", e.message));
                return Ok(None);
            }
        };
        let golden_kind = if stored.golden.is_empty() {
            MachineKind::Sandbox
        } else {
            let kind = self.rt.image_kind(&stored.golden).await;
            // The `.catch(() => undefined)` the TS reads it through passes the answer on one turn later.
            tokio::task::yield_now().await;
            kind.ok().flatten().unwrap_or(MachineKind::Sandbox)
        };
        let absent = Rc::new(Cell::new(false));
        let machine_id = stored.machine_id.clone();
        let held_away: Rc<dyn Fn(HostError) -> Arc<dyn Machine>> = {
            let absent = absent.clone();
            let machine_id = machine_id.clone();
            Rc::new(move |refusal: HostError| {
                absent.set(true);
                absent_machine(
                    &machine_id,
                    golden_kind,
                    Arc::new(move || host_to_engine(refusal.clone())),
                )
            })
        };
        let mut missing: Option<String> = None;
        let machine: Arc<dyn Machine> = match seen {
            Some(found) => found.machine,
            None => {
                let read = match asked {
                    Some(asked) => asked,
                    None => {
                        let (backend, id_for_read) = (at.clone(), machine_id.clone());
                        async move { backend.get(&id_for_read).await.map_err(host_to_engine_back) }
                            .boxed_local()
                            .shared()
                    }
                };
                match self
                    .read_within(&at, &stored, load.as_ref(), held_away.as_ref(), read)
                    .await
                {
                    Ok(machine) => machine,
                    Err(e) => {
                        if !is_missing_host(&e) {
                            held_away(e)
                        } else if stored.phase != WorkspacePhase::Gone {
                            let said = said_of(&e);
                            self.held_answers
                                .borrow_mut()
                                .insert(id.clone(), said.clone());
                            held_away(HostError::new(gone_unconfirmed_line(&machine_id, &said)))
                        } else {
                            missing = Some(said_of(&e));
                            dead_machine(&machine_id)
                        }
                    }
                }
            }
        };
        let at_provider: Option<MachineState> = if missing.is_some() {
            Some(MachineState::Gone)
        } else if absent.get() {
            None
        } else if found {
            sighted.or_else(|| machine.seen().map(|s| s.state))
        } else {
            match machine.seen().map(|s| s.state) {
                Some(state) => Some(state),
                None => machine.state().await.ok(),
            }
        };
        let phase = match at_provider {
            None => stored.phase,
            Some(_) if stored.phase == WorkspacePhase::Gone => at_provider
                .and_then(wsp_core::status::phase_leaving_gone)
                .unwrap_or(WorkspacePhase::Gone),
            Some(MachineState::Gone) => WorkspacePhase::Gone,
            Some(MachineState::Paused) => WorkspacePhase::Napping,
            Some(MachineState::Running) => WorkspacePhase::Running,
            Some(MachineState::Starting) => {
                if stored.phase == WorkspacePhase::Pausing {
                    WorkspacePhase::Napping
                } else {
                    stored.phase
                }
            }
        };
        let mut record = stored.clone();
        record.phase = phase;
        if let Some(url) = machine.stream_url() {
            record.screen = Some(wsp_wire::views::workspace::WorkspaceViewScreen {
                stream_url: url.to_string(),
            });
        }
        if phase == WorkspacePhase::Gone {
            record.gone = Some(match &stored.gone {
                Some(gone) => gone.clone(),
                None => {
                    let sighting = GoneSighting {
                        by: GoneSeenBy::RecordLoad,
                        at: self.rt.clock().now() as f64,
                        answer: missing.clone(),
                    };
                    gone_words(&stored.machine_id, Some(&sighting))
                }
            });
        } else {
            record.gone = None;
        }
        if phase == WorkspacePhase::Napping {
            record.disk_gone = None;
        }
        let paused_at = if phase == WorkspacePhase::Napping {
            machine
                .seen()
                .and_then(|s| s.paused_at.clone())
                .map(|p| date_parse(&p))
                .unwrap_or(f64::NAN)
        } else {
            f64::NAN
        };
        let nap_moved =
            paused_at.is_finite() && stored.napped_at.as_ref().map(|n| n.0) != Some(paused_at);
        if nap_moved {
            record.napped_at = Some(JsNumber(paused_at));
        }
        let rec = Rc::new(RefCell::new(record));
        let entry = self.rt.attach(&rec, machine.clone());
        if nap_moved && phase == stored.phase {
            let snapshot = rec.borrow().clone();
            self.rt.persist(&snapshot).await?;
        }
        if phase != stored.phase {
            let line = if phase == WorkspacePhase::Gone {
                let gone = rec.borrow().gone.clone().unwrap_or_default();
                gone_log_line(&id, &gone)
            } else {
                format!("workspace {id} was left {} and its machine is {} at the provider; the record hydrates {}", phase_word(stored.phase), state_word(at_provider), phase_word(phase))
            };
            self.rt.warn(&line);
            let snapshot = rec.borrow().clone();
            self.rt.persist(&snapshot).await?;
        }
        if phase != WorkspacePhase::Running || absent.get() {
            return Ok(None);
        }
        if stored.phase == WorkspacePhase::Napping || stored.phase == WorkspacePhase::Waking {
            entry.borrow_mut().unchecked = true;
        }
        self.rt.idle_touch(&id);
        Ok(Some(entry))
    }
}

/// A store row read as a record. The TS reads the row unchecked, so a key an older row never wrote reads as the value
/// the typed record needs, the one the tests' fixtures stand on (`fill_workspace`).
fn record_of(raw: Value) -> Result<WorkspaceRecord, HostError> {
    let mut row = match raw {
        Value::Object(row) => row,
        other => return serde_json::from_value(other).map_err(|e| HostError::new(e.to_string())),
    };
    for (key, fill) in [
        ("createdAt", Value::String(String::new())),
        ("spec", json!({})),
        ("size", json!({"cpu": 1, "memMb": 1})),
        ("firstLife", json!(true)),
    ] {
        row.entry(key.to_string()).or_insert(fill);
    }
    if let Some(Value::Object(worktree)) = row.get_mut("worktree") {
        worktree
            .entry("path".to_string())
            .or_insert_with(|| json!(""));
    }
    if let Some(Value::Object(sub)) = row.get_mut("sub") {
        for key in ["on", "repo", "branch", "folder"] {
            sub.entry(key.to_string()).or_insert_with(|| json!(""));
        }
        sub.entry("base".to_string())
            .or_insert_with(|| json!({"branch": "", "sha": ""}));
    }
    serde_json::from_value(Value::Object(row)).map_err(|e| HostError::new(e.to_string()))
}

/// A stored row's `id`, for the line a row that does not read is warned with.
pub(crate) fn id_of(raw: &Value) -> String {
    raw.get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// A provider error as the host error the boot area carries (`AreaError::into_host`).
fn host_to_engine_back(e: EngineError) -> HostError {
    AreaError::from(e).into_host()
}

/// Whether a host error is the provider's word that a machine is gone (`isMissing`).
fn is_missing_host(e: &HostError) -> bool {
    matches!(e.kind, Some(Kind::Missing))
}

/// A workspace phase's word, as a line names it.
fn phase_word(phase: WorkspacePhase) -> &'static str {
    match phase {
        WorkspacePhase::Running => "running",
        WorkspacePhase::Pausing => "pausing",
        WorkspacePhase::Napping => "napping",
        WorkspacePhase::Waking => "waking",
        WorkspacePhase::Gone => "gone",
        WorkspacePhase::Queued => "queued",
        WorkspacePhase::Released => "released",
    }
}
