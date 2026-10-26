// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:64-290 at 65f6b89c7: build, get, buildPlace, prepareAt, prepare, seal and recipe. Spec:
//! `spec/golden_prepare.md`.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::{select, Either, FutureExt, LocalBoxFuture, Shared};

use wsp_core::types::api::{BuildPlace, PrepareAsk, Sealed};
use wsp_core::types::internal::{copy_key, BuilderLife, BuilderRecord, PrepareOutcome, PrepareStoppedError, BUILDERS, HELD_TTL_MS};
use wsp_core::types::wiring::{GoldenBuildRequest, GoldenRecipe};
use wsp_engine::cancel::Cancel;
use wsp_engine::golden_seam::GoldenImport;
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend};
use wsp_wire::views::golden_image::{GoldenBuilderView, GoldenLogin, GoldenManifest, GoldenStage, RecipeDigest, ALREADY_APPLIED};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::wire::capabilities::forks_no_machines;
use wsp_words::exit::{HostError, Kind};
use wsp_words::place_word::{place_said, PlaceRef};
use wsp_words::words::thread::{build_place_ask_line, NO_BUILD_PLACE_LINE};

use super::super::builders::life::{CreateStop, CreatingFuture, ImportMark};
use super::super::builders::places::PlaceAt;
use super::super::builders::BuildPlace as PlaceRow;
use super::super::engine::{ApplyImportAsk, KillConfirm, MachineSize, PrepareBuilderAsk, StageFn};
use super::super::{BuilderRef, ImageError, ImageResult};
use super::{GoldenArea, GoldenCtx, Preparing};
use crate::places::types::{is_place_forks_nowhere, place_forks_nowhere};

/// `prepareAt`'s answer: the place and what a sentence calls it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedAt {
    pub place: String,
    pub name: String,
}

type StopFuture = Shared<LocalBoxFuture<'static, PrepareStoppedError>>;

/// One prepare as its run reads it (golden.ts:156-170).
struct Run {
    ctx: Rc<dyn GoldenCtx>,
    name: String,
    place: String,
    wired: String,
    at: Arc<dyn MachineBackend>,
    recipe: GoldenRecipe,
    import: Option<GoldenImport>,
    mark: Option<ImportMark>,
    kind: Option<wsp_wire::views::golden_image::MachineKind>,
    signal: Option<Cancel>,
    stage: StageFn,
}

/// The state a run's work and its stop share (golden.ts `mine`, `creating`, `stopping`, `same`).
#[derive(Clone)]
struct Held {
    ctx: Rc<dyn GoldenCtx>,
    at: Arc<dyn MachineBackend>,
    mine: Rc<RefCell<Option<BuilderRef>>>,
    creating: Rc<RefCell<Option<CreatingFuture>>>,
    stopping: Rc<RefCell<Option<StopFuture>>>,
    same: Option<BuilderRef>,
}

fn kind_word(kind: PlaceKind) -> &'static str {
    match kind {
        PlaceKind::Computer => "computer",
        PlaceKind::Provider => "provider",
    }
}

fn to_api_place(p: PlaceRow) -> BuildPlace {
    BuildPlace { place: p.place, name: p.name, kind: p.kind, backend: p.backend }
}

impl GoldenArea {
    /// golden.ts:80-97 (G1).
    pub async fn build(self: &Rc<Self>, o: GoldenBuildRequest) -> ImageResult<Sealed> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        let key = o.name.unwrap_or_else(|| "default".to_string());
        let wired = ctx.places().wired();
        let prior = ctx.copy_of(&wired, &key).await?;
        let mut labels = indexmap::IndexMap::new();
        labels.insert(wsp_engine::labels::WSP_LABEL.to_string(), "1".to_string());
        labels.insert(wsp_engine::labels::OWNER_LABEL.to_string(), ctx.owner());
        labels.insert(wsp_engine::labels::CREATED_AT_LABEL.to_string(), super::super::builders::iso_at(ctx.clock().now()));
        let claim = ctx.claiming(&format!("golden/{key}"), None, None);
        let backend = claim.backend();
        let built = ctx
            .roads()
            .build_golden(super::super::engine::BuildGoldenAsk {
                request: o.options,
                backend,
                name: key.clone(),
                host_id: ctx.image_mark(),
                labels,
                manifest: prior,
            })
            .await;
        drop(claim);
        let built = built.map_err(ImageError::Engine)?;
        ctx.put_copy(&wired, &key, &built.manifest).await?;
        Ok(Sealed { manifest: built.manifest, version: built.version })
    }

    /// golden.ts:98-102 (G2).
    pub async fn get(&self, name: Option<&str>) -> Result<Option<GoldenManifest>, HostError> {
        self.ctx.ready().await?;
        let key = name.unwrap_or("default");
        let place = self.ctx.image_place(key).await?;
        self.ctx.copy_of(&place, key).await
    }

    /// golden.ts:104-131 (G3).
    pub async fn build_place(&self, word: Option<&str>) -> ImageResult<BuildPlace> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        let target = if ctx.record_of("default").await?.is_some() { Some(ctx.image_place("default").await?) } else { word.map(str::to_string) };
        if let Some(target) = target {
            let at = ctx.place_at(&target).await?;
            return self.runs(&at.place, at.at).map_err(ImageError::from);
        }
        let door = ctx.door();
        let marked = match &door {
            Some(d) => d.default_place().await?,
            None => None,
        };
        let first = match (marked, &door) {
            (Some(id), Some(d)) => match d.forking_backend(&id).await {
                Ok(at) => self.runs(&id, at),
                Err(e) => Err(e),
            },
            _ => {
                let wired = ctx.places().wired();
                match ctx.backend_at(&wired) {
                    Ok(at) => self.runs(&wired, at),
                    Err(e) => Err(e),
                }
            }
        };
        match first {
            Ok(place) => return Ok(place),
            Err(e) if !is_place_forks_nowhere(&e) => return Err(e.into()),
            Err(_) => {}
        }
        let mut running = ctx.build_places().await?;
        match running.len() {
            0 => Err(ctx.conflict(NO_BUILD_PLACE_LINE).into()),
            1 => Ok(to_api_place(running.swap_remove(0))),
            _ => {
                let said: Vec<String> = running
                    .iter()
                    .map(|p| place_said(&PlaceRef { id: p.place.clone(), name: p.name.clone(), kind: Some(kind_word(p.kind).to_string()) }))
                    .collect();
                Err(ctx.conflict(&build_place_ask_line(&said)).into())
            }
        }
    }

    /// golden.ts:133-137 (G4).
    pub async fn prepare_at(&self, name: Option<&str>, place: Option<&str>) -> Result<PreparedAt, HostError> {
        self.ctx.ready().await?;
        let at = self.place_for_prepare(name.unwrap_or("default"), place, false).await?;
        Ok(PreparedAt { name: self.ctx.place_name(&at.place), place: at.place })
    }

    /// golden.ts:75-78: the place a prepare of `name` runs at.
    pub async fn place_for_prepare(&self, name: &str, word: Option<&str>, copy: bool) -> Result<PlaceAt, HostError> {
        let asked = if !copy && self.ctx.record_of(name).await?.is_some() {
            Some(self.ctx.image_place(name).await?)
        } else {
            word.map(str::to_string)
        };
        match asked {
            None => {
                let wired = self.ctx.places().wired();
                let at = self.ctx.backend_at(&wired)?;
                Ok(PlaceAt { place: wired, at })
            }
            Some(word) => self.ctx.place_at(&word).await,
        }
    }

    /// golden.ts:139-271 (G5 to G9).
    pub async fn prepare(self: &Rc<Self>, o: PrepareAsk) -> ImageResult<GoldenBuilderView> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        let recipe = ctx.recipe_or_throw(o.recipe.clone())?;
        let name = o.name.clone().unwrap_or_else(|| "default".to_string());
        let copy = o.copy == Some(true);
        let at = self.place_for_prepare(&name, o.place.as_deref(), copy).await?;
        let mark = recipe.import.as_ref().map(|imp| ctx.import_mark(imp));
        let hash = mark.as_ref().map(|m| m.recipe_hash.clone());
        let key = copy_key(&at.place, &name);
        let active = ctx.preparing().borrow().get(&key).cloned();
        if let Some(active) = active {
            if active.hash == hash {
                return active.run.await;
            }
            return Err(HostError::new(format!("a builder named {name} is still being prepared for a different recipe; wait for it to finish, then run again")).into());
        }
        let run = Rc::new(Run {
            wired: ctx.places().wired(),
            name: name.clone(),
            place: at.place.clone(),
            at: at.at.clone(),
            import: recipe.import.clone(),
            recipe,
            mark,
            kind: o.kind,
            signal: o.signal.clone(),
            stage: ctx.stage_of(&name, Some(&at.place), copy),
            ctx: ctx.clone(),
        });
        let body_ctx = ctx.clone();
        let body_run = run.clone();
        let body_key = key.clone();
        let future = async move {
            let claim = body_ctx.claiming(&format!("builder/{body_key}"), Some(body_run.at.clone()), Some(&body_run.place));
            let backend = claim.backend();
            let result = prepare_run(body_run, backend).await;
            drop(claim);
            body_ctx.preparing().borrow_mut().shift_remove(&body_key);
            result
        }
        .boxed_local()
        .shared();
        ctx.preparing().borrow_mut().insert(key, Preparing { hash, run: future.clone() });
        future.await
    }

    /// golden.ts:273-281 (G10).
    pub async fn seal(&self, builder_id: &str, logins: Option<Vec<GoldenLogin>>, keep_builder: Option<bool>) -> ImageResult<Sealed> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        ctx.refresh_builders().await?;
        let found = ctx.builders().borrow().get(builder_id).cloned();
        let Some(entry) = found else {
            return Err(HostError::new(format!("no such builder: {builder_id}")).into());
        };
        ctx.refuse_untouchable(&entry)?;
        let keep = keep_builder != Some(false) && entry.borrow().builder.import.as_ref().and_then(|i| i.recipe.as_ref()).is_some();
        let sealed = ctx.seal_entry(&entry, keep, logins, None).await?;
        Ok(Sealed { manifest: sealed.manifest, version: sealed.version })
    }

    /// golden.ts:283-290 (G11).
    pub async fn recipe(&self, name: Option<&str>) -> Result<Option<RecipeDigest>, HostError> {
        self.ctx.ready().await?;
        let key = name.unwrap_or("default");
        let place = self.ctx.image_place(key).await?;
        let Some(manifest) = self.ctx.copy_of(&place, key).await? else {
            return Ok(None);
        };
        self.ctx.copy_recipe_of(&place, key, manifest.head.0 as u32).await
    }

    /// golden.ts:98-131's `runs`: a place this host can build at, or the refusal that it forks nothing.
    fn runs(&self, place: &str, at: Arc<dyn MachineBackend>) -> Result<BuildPlace, HostError> {
        let name = self.ctx.place_name(place);
        if forks_no_machines(&at.capabilities()) {
            return Err(HostError { kind: Some(Kind::Conflict), ..place_forks_nowhere(&format!("{name} forks no machines, so your image cannot be built there")) });
        }
        Ok(BuildPlace { place: place.to_string(), name, kind: self.ctx.place_kind_of(place), backend: at })
    }
}

fn find_same(run: &Run, mark: &ImportMark) -> Option<BuilderRef> {
    let list: Vec<BuilderRef> = run.ctx.builders().borrow().values().cloned().collect();
    list.into_iter().find(|entry| {
        let x = entry.borrow();
        matches!(x.life, BuilderLife::Own | BuilderLife::Reusable)
            && x.record.building != Some(true)
            && x.record.sealed.is_none()
            && x.record.name == run.name
            && x.record.place.as_deref().unwrap_or(run.wired.as_str()) == run.place.as_str()
            && x.builder.import.as_ref().is_some_and(|i| i.recipe_hash == mark.recipe_hash)
    })
}

async fn prepare_run(run: Rc<Run>, backend: Arc<dyn MachineBackend>) -> ImageResult<GoldenBuilderView> {
    let ctx = run.ctx.clone();
    ctx.refresh_builders().await?;
    let same = match &run.mark {
        Some(mark) => find_same(&run, mark),
        None => None,
    };
    let held = Held {
        ctx: ctx.clone(),
        at: run.at.clone(),
        mine: Rc::new(RefCell::new(None)),
        creating: Rc::new(RefCell::new(None)),
        stopping: Rc::new(RefCell::new(None)),
        same: same.clone(),
    };
    if run.signal.as_ref().is_some_and(Cancel::is_cancelled) {
        held.start();
    }
    if let Some(entry) = &same {
        *held.mine.borrow_mut() = Some(entry.clone());
    }
    let work: LocalBoxFuture<'static, ImageResult<GoldenBuilderView>> = match (same, run.import.clone()) {
        (Some(entry), Some(imp)) => attach(held.clone(), run.clone(), imp, entry).boxed_local(),
        _ => fresh(held.clone(), run.clone(), backend).boxed_local(),
    };
    raced(work, &held, run.signal.clone()).await
}

/// golden.ts:177-206: the stop. One turn before it reads the machine the prepare holds, so a signal cancelled before the
/// prepare began still finds the builder an attach took.
async fn stop(ctx: Rc<dyn GoldenCtx>, at: Arc<dyn MachineBackend>, mine: Rc<RefCell<Option<BuilderRef>>>, creating: Rc<RefCell<Option<CreatingFuture>>>, same: Option<BuilderRef>) -> PrepareStoppedError {
    let pending = creating.borrow().clone();
    match pending {
        Some(create) => {
            let _ = create.await;
        }
        None => tokio::task::yield_now().await,
    }
    let placed = mine.borrow().clone();
    let Some(entry) = placed else {
        return PrepareStoppedError::new(None, None);
    };
    let id = entry.borrow().record.id.clone();
    if same.as_ref().is_some_and(|s| Rc::ptr_eq(s, &entry)) {
        let record = {
            let mut x = entry.borrow_mut();
            x.record.held_by = None;
            x.life = BuilderLife::Reusable;
            x.record.clone()
        };
        if put_record(ctx.as_ref(), &id, &record).await.is_err() {
            ctx.warn(&format!("hold on builder {id} not released; it ages out in {} minutes", HELD_TTL_MS / 60_000));
        }
        return PrepareStoppedError::new(Some(id), Some(PrepareOutcome::Kept));
    }
    let machine = entry.borrow().builder.machine.clone();
    let confirm: Option<KillConfirm> = ctx.kill_confirm();
    if let Err(e) = ctx.roads().kill_until_gone(at, machine, confirm).await {
        return PrepareStoppedError::new(Some(id), Some(PrepareOutcome::Left(e.to_string())));
    }
    if ctx.forget_builder(&id).await.is_err() {
        ctx.warn(&format!("record of builder {id} not dropped; the machine is gone and the next load drops it"));
    }
    PrepareStoppedError::new(Some(id), None)
}

async fn put_record(ctx: &dyn GoldenCtx, id: &str, record: &BuilderRecord) -> Result<(), HostError> {
    let value = serde_json::to_value(record).map_err(|e| HostError::new(e.to_string()))?;
    ctx.store().put(BUILDERS, id, value).await
}

impl Held {
    fn started(&self) -> bool {
        self.stopping.borrow().is_some()
    }

    /// Starts the stop once; a second start is the first one's.
    fn start(&self) {
        if self.started() {
            return;
        }
        let (ctx, at, mine, creating, same) = (self.ctx.clone(), self.at.clone(), self.mine.clone(), self.creating.clone(), self.same.clone());
        let future = async move { stop(ctx, at, mine, creating, same).await }.boxed_local().shared();
        *self.stopping.borrow_mut() = Some(future);
    }

    /// Once a stop has begun, its answer is the run's answer (golden.ts:197-200).
    async fn stop_error(&self) -> ImageError {
        let stop = self.stopping.borrow().clone();
        match stop {
            Some(stop) => ImageError::Stopped(stop.await),
            None => ImageError::Stopped(PrepareStoppedError::new(None, None)),
        }
    }
}

/// golden.ts:185-199 (`raced`): the work runs on under a stop, and a stop that began answers for the whole run.
async fn raced(work: LocalBoxFuture<'static, ImageResult<GoldenBuilderView>>, held: &Held, signal: Option<Cancel>) -> ImageResult<GoldenBuilderView> {
    let mut work = work;
    if !held.started() {
        let Some(signal) = signal else {
            return work.await;
        };
        if signal.is_cancelled() {
            held.start();
        } else {
            let abort = async move { signal.cancelled().await }.boxed_local();
            match select(work, abort).await {
                Either::Left((result, _abort)) => return result,
                Either::Right((_, pending)) => {
                    held.start();
                    work = pending;
                }
            }
        }
    }
    let stop = held.stopping.borrow().clone();
    let Some(stop) = stop else {
        return work.await;
    };
    match select(work, stop).await {
        Either::Left((_result, stop)) => Err(ImageError::Stopped(stop.await)),
        Either::Right((answer, _work)) => Err(ImageError::Stopped(answer)),
    }
}

/// golden.ts:207-235 (G7): an attach stages `creating` and `deploying-daemon` as already applied, then applies the
/// import, proves the machine answers, and holds it.
async fn attach(held: Held, run: Rc<Run>, imp: GoldenImport, entry: BuilderRef) -> ImageResult<GoldenBuilderView> {
    let ctx = held.ctx.clone();
    let machine = entry.borrow().builder.machine.clone();
    match attach_steps(&held, &run, &imp, &entry, &machine).await {
        Ok(()) => {
            (run.stage)(GoldenStage::Ready, None, None, None);
            Ok(ctx.builder_view(&entry))
        }
        Err(e) => {
            if held.started() {
                return Err(e);
            }
            let mut detail = e.message();
            if let Err(k) = ctx.roads().kill_until_gone(held.at.clone(), machine, ctx.kill_confirm()).await {
                detail.push_str(&format!("; {k}"));
            }
            if let Err(f) = ctx.forget_if_gone(&entry, held.at.clone()).await {
                return Err(f.into());
            }
            (run.stage)(GoldenStage::Failed, Some(detail), None, None);
            Err(e)
        }
    }
}

async fn attach_steps(held: &Held, run: &Run, imp: &GoldenImport, entry: &BuilderRef, machine: &Arc<dyn Machine>) -> ImageResult<()> {
    let stage = run.stage.clone();
    stage(GoldenStage::Creating, Some(ALREADY_APPLIED.to_string()), None, None);
    stage(GoldenStage::DeployingDaemon, Some(ALREADY_APPLIED.to_string()), None, None);
    let ledger = entry.borrow().builder.import.clone();
    let applied = held
        .ctx
        .roads()
        .apply_golden_import(machine.clone(), ApplyImportAsk { import: imp.clone(), setup: run.recipe.setup.clone(), ledger, on_stage: stage.clone() })
        .await?;
    let alive = machine.exec("true", ExecOptions::default()).await?;
    let code = alive.exit_code.get();
    if code != 0 {
        return Err(HostError::new(format!("the builder answered exit {code} to a no-op; it is not serving")).into());
    }
    if held.started() {
        return Err(held.stop_error().await);
    }
    {
        let mut x = entry.borrow_mut();
        x.builder.import = Some(applied.ledger);
        x.life = BuilderLife::Own;
        x.recipe = Some(run.recipe.clone());
    }
    held.ctx.hold(entry).await?;
    Ok(())
}

/// golden.ts:236-258 (G8): a fresh builder made on the recording backend, settled as the run's entry.
async fn fresh(held: Held, run: Rc<Run>, backend: Arc<dyn MachineBackend>) -> ImageResult<GoldenBuilderView> {
    let ctx = held.ctx.clone();
    let mine = held.mine.clone();
    let made: Rc<dyn Fn(BuilderRef)> = Rc::new(move |p: BuilderRef| *mine.borrow_mut() = Some(p));
    let creating = held.creating.clone();
    let began: Rc<dyn Fn(CreatingFuture)> = Rc::new(move |c: CreatingFuture| *creating.borrow_mut() = Some(c));
    let recording = ctx.recording_creates(
        backend,
        &run.name,
        run.mark.clone(),
        made,
        Some(CreateStop { signal: run.signal.clone(), began }),
        &run.place,
    );
    let ask = PrepareBuilderAsk {
        backend: recording,
        size: MachineSize { cpu: run.recipe.cpu, mem_mb: run.recipe.mem_mb, envs: run.recipe.envs.clone(), labels: None },
        setup: run.recipe.setup.clone(),
        base_template: run.recipe.base_template.clone(),
        kind: run.kind.clone(),
        deploy_daemon: run.recipe.deploy_daemon.clone(),
        import: run.import.clone(),
        on_stage: run.stage.clone(),
    };
    let builder = match ctx.roads().prepare_builder(ask).await {
        Ok(builder) => builder,
        Err(e) => {
            let placed = held.mine.borrow().clone();
            if let Some(placed) = placed {
                if !held.started() {
                    ctx.forget_if_gone(&placed, held.at.clone()).await?;
                }
            }
            return Err(e);
        }
    };
    if held.started() {
        return Err(held.stop_error().await);
    }
    let placeholder = held.mine.borrow().clone();
    let entry = ctx.settle_builder(&run.name, builder, placeholder, &run.place).await?;
    entry.borrow_mut().recipe = Some(run.recipe.clone());
    Ok(ctx.builder_view(&entry))
}
