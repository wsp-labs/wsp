// SPDX-License-Identifier: AGPL-3.0-only
//! builders.ts:80-189, 283-472 at 65f6b89c7: a builder's hold and heartbeat, its grace, its stages, the recording
//! create, the settle and the seal. Spec: `spec/builders_life.md`.
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{FutureExt, LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;

use wsp_catalog::signin_rows::{GH_LOGIN_PATHS, NEVER_ON_BROKERED};
use wsp_core::clock::Timer;
use wsp_core::types::internal::{copy_key, vault_key, BuilderLife, BuilderRecord, HeldBy, PrepareStoppedError, Sealed, StageFrame, BUILDERS, HEARTBEAT_MS, IMAGES, IMAGE_VAULTS};
use wsp_core::types::pending::ImportLedger;
use wsp_core::types::wiring::{GoldenRecipe, GRACE_MS};
use wsp_engine::box_backend::BoxLimits;
use wsp_engine::errors::{EngineError, Result as EngineResult};
use wsp_engine::labels::{CREATED_AT_LABEL, OWNER_LABEL, SMOKE_LABEL, WSP_LABEL};
use wsp_engine::machine::{
    BackendOffers, BackendPricing, BaseTemplates, Capabilities, CloudLimits, ExecResult, Lifecycle, Machine, MachineBackend, MachineListRow,
    MachineSpec, NamedMachine, PlaceCapacity, RootDisk, Size, SnapshotRow, TemplateRow, Waiting,
};
use wsp_engine::orphans::{ReapFailure, ReapReason, ReapedMachine};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::golden_image::{GoldenLogin, GoldenStage, GoldenStageEvent, GoldenStep, GoldenVersion, SealedImage, SealedVault};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_words::date_parse::date_parse;
use wsp_words::exit::HostError;

use super::{iso_at, BuildersArea, StageAt};
use crate::images::engine::{EngineBuilder, GoneReading, KillConfirm, MachineSize, SealGoldenAsk, SealResult, Signal, StageFn};
use crate::images::{BuilderRef, ImageError, ImageResult, LiveBuilder};

/// `expireGrace`'s answer (builders.ts:95).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Expired {
    pub reaped: Vec<ReapedMachine>,
    pub failed: Vec<ReapFailure>,
}

/// The import a recording create stamps on its placeholder (`Pick<GoldenImport, "recipeHash" | "recipe">`).
#[derive(Clone, Debug, PartialEq)]
pub struct ImportMark {
    pub recipe_hash: String,
    pub recipe: Option<wsp_wire::views::golden_image::RecipeDigest>,
}

/// A recording create in flight: shared, so the stop and the create both await it.
pub type CreatingFuture = Shared<LocalBoxFuture<'static, ImageResult<()>>>;

/// `recordingCreates`' stop: the prepare's signal and where it hands the create in flight.
#[derive(Clone)]
pub struct CreateStop {
    pub signal: Option<Signal>,
    /// Handed the creating future before the provider answers (TS `began(creating)`).
    pub began: Rc<dyn Fn(CreatingFuture)>,
}

/// The milliseconds a stamp is old by the context clock; none where the stamp does not parse.
fn age_ms(now: i64, stamp: &str) -> Option<i64> {
    let at = date_parse(stamp);
    (!at.is_nan()).then(|| now - at as i64)
}

fn json_of<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("a record serializes as JSON")
}

/// The failure a recording create answers with: an engine error stays whole, anything else is one its words carry.
fn engine_of(e: ImageError) -> EngineError {
    match e {
        ImageError::Engine(e) => e,
        ImageError::Host(h) => EngineError::Other { message: h.message, code: h.code },
        ImageError::Stopped(s) => EngineError::CreateCancelled { machine_id: s.builder_id.clone().unwrap_or_default(), why: "stopped".into(), line: s.message() },
        ImageError::SnapshotFailed { message, .. } => EngineError::Other { message, code: None },
    }
}

/// What a recording create's placeholder is made from.
#[derive(Clone)]
struct PlaceholderAsk {
    spec: MachineSpec,
    waiting: Option<Waiting>,
    sent: Option<String>,
    name: String,
    imp: Option<ImportMark>,
    made: Rc<dyn Fn(BuilderRef)>,
    place: String,
}

/// The backend a recording create answers with: every road is the inner backend's but `create`.
struct RecordingBackend {
    area: Rc<BuildersArea>,
    inner: Arc<dyn MachineBackend>,
    name: String,
    imp: Option<ImportMark>,
    made: Rc<dyn Fn(BuilderRef)>,
    stop: Option<CreateStop>,
    place: String,
}

/// The create itself, held as a shared future so a stop can wait on it; the machine it made lands in `slot`.
async fn make_placeholder(area: Rc<BuildersArea>, inner: Arc<dyn MachineBackend>, ask: PlaceholderAsk, slot: Rc<RefCell<Option<Arc<dyn Machine>>>>) -> ImageResult<()> {
    let spec = ask.spec;
    let machine = area.ctx.observed(inner.create(spec.clone(), ask.waiting, ask.sent).await?);
    let created_at = machine
        .labels()
        .and_then(|l| l.get(CREATED_AT_LABEL))
        .or_else(|| spec.labels.as_ref().and_then(|l| l.get(CREATED_AT_LABEL)))
        .cloned()
        .unwrap_or_else(|| iso_at(area.ctx.clock().now()));
    let defaults = area.ctx.backend().pricing().default_size();
    let size = WorkspaceSize { cpu: spec.cpu.unwrap_or(JsNumber(defaults.cpu)), mem_mb: spec.mem_mb.unwrap_or(JsNumber(defaults.mem_mb)) };
    let id = machine.id().to_string();
    let record = BuilderRecord {
        id: id.clone(),
        name: ask.name,
        kind: spec.kind,
        base_template: spec.template.clone().unwrap_or_default(),
        setup_sha: String::new(),
        created_at: created_at.clone(),
        size: size.clone(),
        stream_url: machine.stream_url().map(str::to_string),
        first_life: true,
        held_by: None,
        building: Some(true),
        import: ask.imp.as_ref().map(placeholder_ledger),
        base: None,
        parent_snapshot_id: None,
        retired: None,
        sealed: None,
        place: Some(ask.place),
    };
    let builder = EngineBuilder {
        machine: machine.clone(),
        kind: spec.kind,
        base_template: record.base_template.clone(),
        setup_sha: String::new(),
        created_at,
        first_life: true,
        size,
        import: None,
        parent_snapshot_id: None,
        base: None,
        retired: None,
    };
    let entry: BuilderRef = Rc::new(RefCell::new(LiveBuilder { record, builder, sealable: true, life: BuilderLife::Own, reach: None, recipe: None }));
    area.ctx.builders().borrow_mut().insert(id, entry.clone());
    (ask.made)(entry.clone());
    area.hold(&entry).await?;
    *slot.borrow_mut() = Some(machine);
    Ok(())
}

#[async_trait(?Send)]
impl MachineBackend for RecordingBackend {
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        self.inner.pricing()
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        self.inner.lifecycle()
    }
    fn base_templates(&self) -> Option<BaseTemplates> {
        self.inner.base_templates()
    }
    fn names_workspace(&self) -> bool {
        self.inner.names_workspace()
    }
    fn logins(&self) -> Option<String> {
        self.inner.logins()
    }
    fn projects(&self) -> Option<String> {
        self.inner.projects()
    }
    fn offers(&self) -> BackendOffers {
        self.inner.offers()
    }
    fn root_disk_of(&self, size: Size) -> Option<RootDisk> {
        self.inner.root_disk_of(size)
    }
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> EngineResult<Arc<dyn Machine>> {
        if self.stop.as_ref().and_then(|s| s.signal.as_ref()).is_some_and(|s| s.is_cancelled()) {
            return Err(EngineError::CreateCancelled { machine_id: String::new(), why: "stopped".into(), line: PrepareStoppedError::new(None, None).message() });
        }
        let slot = Rc::new(RefCell::new(None));
        let ask = PlaceholderAsk {
            spec,
            waiting,
            sent,
            name: self.name.clone(),
            imp: self.imp.clone(),
            made: self.made.clone(),
            place: self.place.clone(),
        };
        let creating: CreatingFuture = make_placeholder(self.area.clone(), self.inner.clone(), ask, slot.clone()).boxed_local().shared();
        if let Some(stop) = &self.stop {
            (stop.began)(creating.clone());
        }
        creating.await.map_err(engine_of)?;
        let machine = slot.borrow_mut().take().expect("a placeholder's machine is set before its create answers");
        Ok(machine)
    }
    async fn get(&self, id: &str) -> EngineResult<Arc<dyn Machine>> {
        self.inner.get(id).await
    }
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> EngineResult<Vec<MachineListRow>> {
        self.inner.list(labels).await
    }
    async fn delete_snapshot(&self, id: &str) -> EngineResult<()> {
        self.inner.delete_snapshot(id).await
    }
    async fn on_computer(&self, cmd: &str, timeout_ms: Option<u64>) -> EngineResult<ExecResult> {
        self.inner.on_computer(cmd, timeout_ms).await
    }
    async fn create_request(&self, spec: &MachineSpec) -> EngineResult<String> {
        self.inner.create_request(spec).await
    }
    async fn replay_create(&self, key: &str, bytes: &str, made_at: &str) -> EngineResult<Option<Arc<dyn Machine>>> {
        self.inner.replay_create(key, bytes, made_at).await
    }
    async fn named_after(&self, names: &BTreeSet<String>) -> EngineResult<Vec<NamedMachine>> {
        self.inner.named_after(names).await
    }
    async fn check_key(&self) -> EngineResult<Value> {
        self.inner.check_key().await
    }
    async fn place_limits(&self) -> EngineResult<Option<CloudLimits>> {
        self.inner.place_limits().await
    }
    async fn capacity(&self) -> EngineResult<PlaceCapacity> {
        self.inner.capacity().await
    }
    async fn limits(&self) -> EngineResult<BoxLimits> {
        self.inner.limits().await
    }
    async fn list_snapshots(&self, ids: Option<&[String]>) -> EngineResult<Vec<SnapshotRow>> {
        self.inner.list_snapshots(ids).await
    }
    async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> EngineResult<String> {
        self.inner.promote_snapshot(snapshot_id, name).await
    }
    async fn get_template(&self, id: &str) -> EngineResult<TemplateRow> {
        self.inner.get_template(id).await
    }
    async fn list_templates(&self) -> EngineResult<Vec<TemplateRow>> {
        self.inner.list_templates().await
    }
    async fn delete_template(&self, id: &str) -> EngineResult<()> {
        self.inner.delete_template(id).await
    }
}

impl BuildersArea {
    /// builders.ts:82-89 (L1).
    pub fn arm_grace(self: &Rc<Self>, id: &str, sealed_at: &str) {
        let previous = self.grace_timers.borrow().get(id).cloned();
        if let Some(cancel) = previous {
            cancel();
        }
        let left = age_ms(self.ctx.clock().now(), sealed_at).map_or(0, |age| (GRACE_MS as i64 - age).max(0));
        let weak: Weak<BuildersArea> = Rc::downgrade(self);
        let id_owned = id.to_string();
        let timer: Timer = Box::new(move || {
            let Some(area) = weak.upgrade() else { return };
            area.grace_timers.borrow_mut().shift_remove(&id_owned);
            let sweep = area.clone();
            area.ctx.spawn(
                async move {
                    if let Err(e) = sweep.expire_grace().await {
                        sweep.ctx.warn(&format!("grace sweep failed: {}", e.message()));
                    }
                }
                .boxed_local(),
            );
        });
        let cancel = self.ctx.clock().schedule(timer, left);
        self.grace_timers.borrow_mut().insert(id.to_string(), cancel);
    }

    /// builders.ts:91-94 (L2).
    pub fn in_window(&self, sealed_at: &str) -> bool {
        age_ms(self.ctx.clock().now(), sealed_at).is_some_and(|age| age < GRACE_MS as i64)
    }

    /// builders.ts:99-118 (L3, L4).
    pub async fn expire_grace(self: &Rc<Self>) -> ImageResult<Expired> {
        let running = self.expiring.borrow().clone();
        if let Some(pass) = running {
            return pass.await;
        }
        let me = self.clone();
        let pass = async move {
            let out = me.expire_once().await;
            me.expiring.borrow_mut().take();
            out
        }
        .boxed_local()
        .shared();
        *self.expiring.borrow_mut() = Some(pass.clone());
        pass.await
    }

    async fn expire_once(self: &Rc<Self>) -> ImageResult<Expired> {
        self.ctx.refresh_builders().await?;
        let mut out = Expired::default();
        let snapshot: Vec<BuilderRef> = self.ctx.builders().borrow().values().cloned().collect();
        for entry in snapshot {
            let (id, place, machine, life, sealed) = {
                let b = entry.borrow();
                (b.record.id.clone(), b.record.place.clone().unwrap_or_else(|| self.ctx.places().wired()), b.builder.machine.clone(), b.life, b.record.sealed.clone())
            };
            let sealed = match (sealed, life) {
                (Some(s), BuilderLife::Own | BuilderLife::Reusable) => s,
                _ => continue,
            };
            if self.in_window(&sealed.at) {
                continue;
            }
            let stop = match self.backend_at(&place) {
                Ok(at) => self.ctx.gone_stop(at, machine).await,
                Err(e) => Err(e),
            };
            if let Err(e) = stop {
                out.failed.push(ReapFailure { id: Some(id), message: format!("could not stop: {}; stays recorded, retried next sweep", e.message) });
                continue;
            }
            let cancel = self.grace_timers.borrow_mut().shift_remove(&id);
            if let Some(cancel) = cancel {
                cancel();
            }
            self.forget_builder(&id).await?;
            out.reaped.push(ReapedMachine {
                id,
                labels: None,
                builder: true,
                reason: ReapReason::Grace,
                age_ms: age_ms(self.ctx.clock().now(), &sealed.at),
                name: None,
            });
        }
        Ok(out)
    }

    /// builders.ts:141-147 (L5).
    pub async fn hold(self: &Rc<Self>, b: &BuilderRef) -> Result<(), HostError> {
        let id = b.borrow().record.id.clone();
        let builders = self.ctx.builders();
        let live = builders.borrow().get(&id).is_some_and(|held| Rc::ptr_eq(held, b));
        if !live {
            return Ok(());
        }
        let closed = self.ctx.closed();
        let record = {
            let mut entry = b.borrow_mut();
            if !closed {
                entry.record.held_by = Some(HeldBy { host: self.ctx.host_id(), pid: self.ctx.pid(), heartbeat: iso_at(self.ctx.clock().now()) });
            }
            json_of(&entry.record)
        };
        self.ctx.store().put(BUILDERS, &id, record).await?;
        if !closed {
            self.arm();
        }
        Ok(())
    }

    /// builders.ts:119-129 (L6): arms one heartbeat.
    pub fn arm(self: &Rc<Self>) {
        if self.ctx.closed() || self.ctx.beat().is_some() {
            return;
        }
        let weak: Weak<BuildersArea> = Rc::downgrade(self);
        let timer: Timer = Box::new(move || {
            let Some(area) = weak.upgrade() else { return };
            area.ctx.set_beat(None);
            let beat = area.clone();
            area.ctx.set_ticking(async move { beat.tick().await }.boxed_local());
        });
        let cancel = self.ctx.clock().schedule(timer, HEARTBEAT_MS as i64);
        self.ctx.set_beat(Some(cancel));
    }

    /// builders.ts:131-137 (L6): one beat.
    pub async fn tick(self: &Rc<Self>) {
        let builders = self.ctx.builders();
        let own: Vec<BuilderRef> = builders.borrow().values().filter(|b| b.borrow().life == BuilderLife::Own).cloned().collect();
        for entry in &own {
            let id = entry.borrow().record.id.clone();
            if let Err(e) = self.hold(entry).await {
                self.ctx.warn(&format!("heartbeat for builder {id} not written: {}", e.message));
            }
        }
        if !own.is_empty() {
            self.arm();
        }
    }

    /// builders.ts:151-155 (L7).
    pub fn refuse_untouchable(&self, entry: &BuilderRef) -> Result<(), HostError> {
        let b = entry.borrow();
        let id = &b.record.id;
        match b.life {
            BuilderLife::Foreign => {
                let owner = b.builder.machine.labels().and_then(|l| l.get(OWNER_LABEL)).cloned().unwrap_or_else(|| "undefined".to_string());
                Err(HostError::new(format!("{id} wears another setup's owner label ({owner}); it is never sealed or reached from here")))
            }
            BuilderLife::Held => {
                let pid = b.record.held_by.as_ref().map_or_else(|| "undefined".to_string(), |h| h.pid.to_string());
                Err(HostError::new(format!("{id} is in use by another wsp process (pid {pid}); it is never sealed or reached from here")))
            }
            _ if b.record.building == Some(true) => Err(HostError::new(format!("{id} is still being prepared; it is never sealed or reached until its stages finish"))),
            _ => Ok(()),
        }
    }

    /// builders.ts:157-160 (L8).
    pub async fn forget_builder(&self, id: &str) -> Result<(), HostError> {
        let builders = self.ctx.builders();
        builders.borrow_mut().shift_remove(id);
        self.ctx.store().delete(BUILDERS, id).await
    }

    /// builders.ts:166-172 (L9).
    pub async fn forget_if_gone(&self, entry: &BuilderRef, at: Arc<dyn MachineBackend>) -> Result<(), HostError> {
        let id = entry.borrow().record.id.clone();
        let gone = matches!(self.ctx.roads().read_gone(at, &id).await, Ok(GoneReading::Gone));
        if gone {
            self.forget_builder(&id).await?;
        }
        Ok(())
    }

    /// builders.ts:177-189 (L10).
    pub fn stage_of(&self, name: &str, on: Option<&str>, named: bool) -> StageFn {
        let ctx = self.ctx.clone();
        let stage_at = self.ctx.stage_at();
        let name = name.to_string();
        let on = on.map(str::to_string);
        Rc::new(move |stage: GoldenStage, detail: Option<String>, step: Option<GoldenStep>, left: Option<Vec<String>>| {
            let frame = StageFrame { name: name.clone(), stage, detail, step, left: left.filter(|l| !l.is_empty()) };
            if let Some(on) = &on {
                let key = copy_key(on, &name);
                match stage {
                    GoldenStage::Ready | GoldenStage::Sealed | GoldenStage::Failed => {
                        stage_at.borrow_mut().shift_remove(&key);
                    }
                    _ => {
                        let held = StageAt { place: on.clone(), name: name.clone(), named, frame: frame.clone() };
                        stage_at.borrow_mut().insert(key, held);
                    }
                }
            }
            ctx.emit_stage(GoldenStageEvent {
                name: frame.name,
                stage: frame.stage,
                detail: frame.detail,
                step: frame.step,
                left: frame.left,
                place: if named { on.clone() } else { None },
            });
        })
    }

    /// builders.ts:285-324 (L11): a backend that is `b` but for `create`.
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn recording_creates(
        self: &Rc<Self>,
        b: Arc<dyn MachineBackend>,
        name: &str,
        imp: Option<ImportMark>,
        made: Rc<dyn Fn(BuilderRef)>,
        stop: Option<CreateStop>,
        place: &str,
    ) -> Arc<dyn MachineBackend> {
        Arc::new(RecordingBackend { area: self.clone(), inner: b, name: name.to_string(), imp, made, stop, place: place.to_string() })
    }

    /// builders.ts:327-350 (L12).
    pub async fn settle_builder(self: &Rc<Self>, name: &str, builder: EngineBuilder, placeholder: Option<BuilderRef>, place: &str) -> Result<BuilderRef, HostError> {
        let created_at = match &placeholder {
            Some(p) => p.borrow().record.created_at.clone(),
            None => builder.created_at.clone(),
        };
        let id = builder.machine.id().to_string();
        let record = BuilderRecord {
            id: id.clone(),
            name: name.to_string(),
            kind: builder.kind,
            base_template: builder.base_template.clone(),
            setup_sha: builder.setup_sha.clone(),
            created_at,
            size: builder.size.clone(),
            stream_url: builder.machine.stream_url().map(str::to_string),
            first_life: true,
            held_by: None,
            building: None,
            import: builder.import.clone(),
            base: builder.base.clone(),
            parent_snapshot_id: builder.parent_snapshot_id.clone(),
            retired: builder.retired.clone(),
            sealed: None,
            place: Some(place.to_string()),
        };
        let entry = match placeholder {
            Some(p) => {
                {
                    let mut e = p.borrow_mut();
                    e.record = record;
                    e.builder = builder;
                }
                p
            }
            None => Rc::new(RefCell::new(LiveBuilder { record, builder, sealable: true, life: BuilderLife::Own, reach: None, recipe: None })),
        };
        self.ctx.builders().borrow_mut().insert(id, entry.clone());
        self.hold(&entry).await?;
        Ok(entry)
    }

    /// builders.ts:357-395 (L13).
    pub async fn record_seal(&self, place: &str, name: &str, entry: &BuilderRef, recipe: &GoldenRecipe, result: SealResult, copy: Option<&SealedImage>) -> Result<SealResult, HostError> {
        let mut hash = copy.map(|c| c.hash.clone());
        let no_secrets = self.place_kind_of(place) == PlaceKind::Provider;
        if copy.is_none() {
            let digest = entry.borrow().builder.import.as_ref().map(|i| i.recipe_hash.clone()).unwrap_or_default();
            let vault = result.vault.as_ref().map(|v| SealedVault {
                sha256: v.sha256.clone(),
                bytes: JsInt::from(v.tar.len() as i64),
                paths: JsInt::from(v.paths as i64),
                held: Some(v.held.clone()),
                taken_at: result.version.created_at.clone(),
            });
            let image_hash = self.ctx.roads().image_hash(&digest, vault.as_ref().map(|v| v.sha256.as_str()));
            let image = SealedImage {
                name: name.to_string(),
                version: JsInt(result.version.version),
                hash: image_hash.clone(),
                recipe_hash: digest,
                recipe: recipe.source.clone(),
                pins: None,
                logins: result.version.logins.clone().unwrap_or_default(),
                sealed_at: result.version.created_at.clone(),
                sealed_from: self.ctx.host_id(),
                vault,
                used_bytes: result.version.used_bytes,
                place: Some(place.to_string()),
                no_secrets: no_secrets.then_some(true),
            };
            let store = self.ctx.store();
            let replaced: Option<SealedImage> = store.get(IMAGES, name).await?.and_then(|v| serde_json::from_value(v).ok());
            if let Some(vault) = &result.vault {
                store.put_blob(IMAGE_VAULTS, &vault_key(name, result.version.version.0 as u32), vault.tar.clone()).await?;
            }
            store.put(IMAGES, name, json_of(&image)).await?;
            if let Some(old) = replaced {
                if old.version.0 .0 != result.version.version.0 {
                    store.delete_blob(IMAGE_VAULTS, &vault_key(name, old.version.0 .0 as u32)).await?;
                }
            }
            hash = Some(image_hash);
        }
        let Some(hash) = hash else { return Ok(result) };
        let mut version = result.version.clone();
        version.image_hash = Some(hash);
        if no_secrets {
            version.no_secrets = Some(true);
        }
        let mut manifest = result.manifest.clone();
        for v in manifest.versions.iter_mut() {
            if v.version == version.version {
                *v = version.clone();
            }
        }
        Ok(SealResult { manifest, version, builder_kept: result.builder_kept, vault: result.vault })
    }

    /// builders.ts:401-464 (L14 to L16).
    pub async fn seal_entry(self: &Rc<Self>, entry: &BuilderRef, keep: bool, logins: Option<Vec<GoldenLogin>>, copy: Option<SealedImage>) -> ImageResult<SealResult> {
        let (id, name, place, named_recipe) = {
            let b = entry.borrow();
            (b.record.id.clone(), b.record.name.clone(), b.record.place.clone().unwrap_or_else(|| self.ctx.places().wired()), b.recipe.clone())
        };
        let recipe = self.recipe_or_throw(named_recipe)?;
        let prior = self.ctx.copy_of(&place, &name).await?;
        let at = self.backend_at(&place)?;
        let brokered = self.place_kind_of(&place) == PlaceKind::Provider;
        let refuse: Vec<String> = if self.ctx.broker_ready() {
            NEVER_ON_BROKERED.clone()
        } else {
            NEVER_ON_BROKERED.iter().filter(|p| !GH_LOGIN_PATHS.contains(*p)).cloned().collect()
        };
        let mut labels = recipe.labels.clone().unwrap_or_default();
        labels.insert(WSP_LABEL.to_string(), "1".to_string());
        labels.insert(SMOKE_LABEL.to_string(), "1".to_string());
        labels.insert(OWNER_LABEL.to_string(), self.ctx.owner());
        labels.insert(CREATED_AT_LABEL.to_string(), iso_at(self.ctx.clock().now()));
        let kill_confirm: Option<KillConfirm> = self.ctx.opts().kill_confirm;

        let outcome: ImageResult<SealResult> = async {
            let builder = entry.borrow().builder.clone();
            let result = {
                let claim = self.ctx.claiming(&format!("smoke/{id}"), Some(at.clone()), Some(&place));
                let ask = SealGoldenAsk {
                    backend: self.ctx.observing(claim.backend()),
                    smoke: recipe.smoke.clone(),
                    size: MachineSize { cpu: recipe.cpu, mem_mb: recipe.mem_mb, envs: recipe.envs.clone(), labels: Some(labels) },
                    manifest: prior.clone(),
                    on_stage: self.stage_of(&name, Some(&place), copy.is_some()),
                    kill_confirm,
                    snapshot_retry_ms: self.ctx.opts().snapshot_retry_ms,
                    logins: logins.clone(),
                    vault_paths: if copy.is_none() && !brokered { recipe.vault_paths.clone() } else { None },
                    refuse: brokered.then(|| refuse.clone()),
                    keep_builder: keep,
                    name: name.clone(),
                    host_id: self.ctx.image_mark(),
                };
                self.ctx.roads().seal_golden(builder, ask).await?
            };
            let stamped = self.record_seal(&place, &name, entry, &recipe, result, copy.as_ref()).await?;
            self.ctx.put_copy(&place, &name, &stamped.manifest).await?;
            let digest = entry.borrow().builder.import.as_ref().and_then(|i| i.recipe.clone());
            if let Some(digest) = digest {
                self.ctx.put_copy_recipe(&place, &name, stamped.version.version.0 as u32, &digest).await?;
            }
            if copy.is_none() && name == "default" {
                if let Some(door) = self.ctx.door() {
                    if let Err(e) = door.mark_default_if_none(&place).await {
                        self.ctx.warn(&format!("the default place was not marked at the seal: {}", e.message));
                    }
                }
            }
            if stamped.builder_kept {
                let sealed_at = iso_at(self.ctx.clock().now());
                {
                    let mut e = entry.borrow_mut();
                    e.record.sealed = Some(Sealed { at: sealed_at.clone(), version: stamped.version.version.0 as u32 });
                    e.record.parent_snapshot_id = Some(stamped.version.snapshot_id.clone());
                    e.builder.parent_snapshot_id = Some(stamped.version.snapshot_id.clone());
                    e.life = BuilderLife::Own;
                }
                self.hold(entry).await?;
                self.arm_grace(&id, &sealed_at);
            } else {
                self.forget_builder(&id).await?;
            }
            Ok(stamped)
        }
        .await;

        match outcome {
            Ok(stamped) => Ok(stamped),
            Err(e) => {
                if let ImageError::SnapshotFailed { builder_state, .. } = &e {
                    if builder_state != "gone" {
                        return Err(e);
                    }
                }
                if let ImageError::Engine(EngineError::NotFirstLife { .. }) = &e {
                    let machine = entry.borrow().builder.machine.clone();
                    self.ctx.roads().kill_until_gone(at.clone(), machine, kill_confirm).await.map_err(ImageError::Engine)?;
                }
                self.forget_if_gone(entry, at).await?;
                Err(e)
            }
        }
    }

    /// builders.ts:469-472 (L17): `at` none is the runtime backend.
    pub async fn drop_image(&self, v: &GoldenVersion, at: Option<Arc<dyn MachineBackend>>) -> ImageResult<()> {
        let at = at.unwrap_or_else(|| self.ctx.backend());
        self.ctx.roads().drop_template(at.clone(), v.template_id.as_deref()).await?;
        at.delete_snapshot(&v.snapshot_id).await?;
        Ok(())
    }
}

/// The ledger a recording create stamps (builders.ts:312): `{recipeHash, recipe?, applied: [], smoke: "true"}`.
pub fn placeholder_ledger(imp: &ImportMark) -> ImportLedger {
    ImportLedger {
        recipe_hash: imp.recipe_hash.clone(),
        recipe: imp.recipe.clone(),
        applied: Vec::new(),
        smoke: "true".to_string(),
        ..Default::default()
    }
}
