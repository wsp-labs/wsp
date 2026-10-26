// SPDX-License-Identifier: AGPL-3.0-only
//! `images/image.ts` at 65f6b89c7: the image record, its copies per place, the copy build and the snapshot a fork
//! boots. Spec: `spec/image.md`.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;

use wsp_core::types::api::{ImageBuildAsk, PrepareAsk};
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::PlaceBackends;
use wsp_engine::machine::MachineBackend;
use wsp_wire::views::golden_image::{GoldenBuilderView, GoldenLogin, GoldenManifest, RecipeDigest, SealedImage, SealedImageBuilt, SealedImageCopy};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::snapshot_lineage::SealedImageView;
use wsp_words::exit::HostError;

use super::builders::places::PlaceAt;
use super::builders::{CopyRecipe, ImageDoor};
use super::engine::{GoldenRoads, KillConfirm, SealResult, StageFn};
use super::records::ImageOf;
use super::{BuilderRef, Builders, ImageError, ImageResult, Joined};
use std::cmp::Ordering;

use futures::FutureExt;

use wsp_core::types::internal::{copy_key, copy_key_parts, vault_key, GOLDENS, IMAGES, IMAGE_VAULTS};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::golden_image::{golden_head, GoldenStage, GoldenVersion};
use wsp_wire::wire::capabilities::builds_images;
use wsp_wire::wire::machine_link::{place_builds_no_image_line, vault_unlisted_refusal};
use wsp_wire::wire::place_link::place_went_away_line;
use wsp_words::exit::{refusal, Kind};
use wsp_words::js::collate;
use wsp_words::words::image::{copy_asks_sign_ins, copy_build_fix, copy_is_current, image_held_sign_ins_line, image_rebuild_fix};
use wsp_words::words::units::fmt_bytes;

/// `ctx.copyRows`' entry as the image area reads it: whether the place's copy row reads stopped. Two reads are the
/// same row only where `Rc::ptr_eq` (TS object identity, image.ts:137, 155).
#[derive(Debug, PartialEq, Eq)]
pub struct CopyRow {
    pub stopped: bool,
}

/// Every call the image area makes on the rest of the runtime.
#[async_trait(?Send)]
pub trait ImageCtx {
    fn store(&self) -> Rc<dyn Store>;
    fn roads(&self) -> Rc<dyn GoldenRoads>;
    fn places(&self) -> Rc<dyn PlaceBackends>;
    fn door(&self) -> Option<Rc<dyn ImageDoor>>;
    fn kill_confirm(&self) -> Option<KillConfirm>;
    fn builders(&self) -> Builders;
    /// `ctx.copyBuilds`, keyed `<place>/<name>`.
    fn copy_builds(&self) -> Rc<RefCell<IndexMap<String, Joined<SealedImageBuilt>>>>;
    /// `ctx.copyRows.get(place)`.
    fn copy_row(&self, place: &str) -> Option<Rc<CopyRow>>;
    /// `ctx.rowSaysFailure(place, e)`.
    fn row_says_failure(&self, place: &str, e: &ImageError) -> bool;
    /// `ctx.placeAway(place)`.
    fn place_away(&self, place: &str) -> bool;
    /// `ctx.frameStopped(place, name, why)`: `why` a sentence or the error.
    fn frame_stopped(&self, place: &str, name: &str, why: Stopped);
    async fn ready(&self) -> Result<(), HostError>;

    // The records area.
    async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError>;
    async fn copy_of(&self, place: &str, name: &str) -> Result<Option<GoldenManifest>, HostError>;
    async fn copy_recipe_of(&self, place: &str, name: &str, version: u32) -> Result<Option<RecipeDigest>, HostError>;
    async fn put_copy_recipe(&self, place: &str, name: &str, version: u32, digest: &RecipeDigest) -> Result<(), HostError>;
    async fn image_of(&self, snapshot_id: &str) -> Result<ImageOf, HostError>;

    // The builders area.
    async fn place_at(&self, word: &str) -> Result<PlaceAt, HostError>;
    fn place_name(&self, place: &str) -> String;
    fn place_kind_of(&self, place: &str) -> PlaceKind;
    async fn image_place(&self, name: &str) -> Result<String, HostError>;
    fn copy_recipe_or_throw(&self) -> Result<CopyRecipe, HostError>;
    fn stage_of(&self, name: &str, on: Option<&str>, named: bool) -> StageFn;
    async fn forget_if_gone(&self, entry: &BuilderRef, at: Arc<dyn MachineBackend>) -> Result<(), HostError>;
    async fn seal_entry(&self, entry: &BuilderRef, keep: bool, logins: Option<Vec<GoldenLogin>>, copy: Option<SealedImage>) -> ImageResult<SealResult>;

    // The golden area.
    async fn golden_prepare(&self, ask: PrepareAsk) -> ImageResult<GoldenBuilderView>;
}

/// What a stopped copy row is framed with.
#[derive(Clone, Debug)]
pub enum Stopped {
    Line(String),
    Error(ImageError),
}

/// `imageArea(ctx)` (image.ts:30).
pub struct ImageArea {
    pub ctx: Rc<dyn ImageCtx>,
}

/// The copy build's inputs (image.ts:163).
#[derive(Clone)]
pub struct CopyBuild {
    pub place: String,
    pub r#where: String,
    pub at: Arc<dyn MachineBackend>,
    pub name: String,
    pub record: SealedImage,
    pub signal: Option<wsp_engine::Cancel>,
}

/// `copyForFork`'s announce: `(where, rateUsdPerHour)`.
pub type Announce = Rc<dyn Fn(&str, f64)>;

impl ImageArea {
    pub fn new(ctx: Rc<dyn ImageCtx>) -> Rc<ImageArea> {
        Rc::new(ImageArea { ctx })
    }

    /// image.ts:37-41 (I1): restored sizes by snapshot id.
    pub async fn snapshot_sizes(&self, at: Arc<dyn MachineBackend>, ids: &[String]) -> IndexMap<String, f64> {
        if ids.is_empty() || !at.capabilities().snapshot_listing {
            return IndexMap::new();
        }
        let rows = at.list_snapshots(Some(ids)).await.unwrap_or_default();
        let mut sizes = IndexMap::new();
        for row in rows {
            let restored = row.restored_bytes.filter(|_| ids.contains(&row.id));
            if let Some(bytes) = restored {
                sizes.insert(row.id, bytes.0);
            }
        }
        sizes
    }

    /// image.ts:44-67 (I2).
    pub async fn copies_of(&self, name: &str) -> Result<Vec<SealedImageCopy>, HostError> {
        let store = self.ctx.store();
        let wired = self.ctx.places().wired();
        let mut heads: Vec<(String, GoldenVersion)> = Vec::new();
        for key in store.keys(GOLDENS).await? {
            let parts = copy_key_parts(&key);
            if parts.name != name {
                continue;
            }
            let manifest: Option<GoldenManifest> = match store.get(GOLDENS, &key).await? {
                Some(value) => Some(decode(value)?),
                None => None,
            };
            if let Some(head) = golden_head(manifest.as_ref()) {
                heads.push((parts.place.unwrap_or_else(|| wired.clone()), head.clone()));
            }
        }
        let mut copies = Vec::with_capacity(heads.len());
        for (place, head) in heads {
            let sizes = match self.backend_at_place(&place) {
                Some(at) => self.snapshot_sizes(at, std::slice::from_ref(&head.snapshot_id)).await,
                None => IndexMap::new(),
            };
            copies.push(copy_at(&place, &head, sizes.get(&head.snapshot_id).copied()));
        }
        copies.sort_by(|a, b| {
            if a.place == wired {
                Ordering::Less
            } else if b.place == wired {
                Ordering::Greater
            } else {
                collate(&a.place, &b.place)
            }
        });
        Ok(copies)
    }

    /// image.ts:69 (I3).
    pub fn conflict(&self, message: &str) -> HostError {
        HostError::with_kind(message, Kind::Conflict)
    }

    /// image.ts:72-79 (I4).
    pub async fn get(&self, name: Option<&str>) -> Result<SealedImageView, HostError> {
        self.ctx.ready().await?;
        let key = name.unwrap_or("default");
        let image = self.ctx.record_of(key).await?;
        let copies = self.copies_of(key).await?;
        Ok(SealedImageView { image, copies, projects: Vec::new() })
    }

    /// image.ts:81-90 (I5): the record and the vault's bytes.
    pub async fn vault(&self, name: Option<&str>) -> Result<(SealedImage, Vec<u8>), HostError> {
        self.ctx.ready().await?;
        let key = name.unwrap_or("default");
        let Some(record) = self.ctx.record_of(key).await? else {
            return Err(self.conflict(&format!("this host owns no image named {key} yet")));
        };
        let version = record.version.get();
        if record.vault.is_none() {
            return Err(self.conflict(&format!(
                "{key} v{version} was sealed before its sign-ins were held, so there is nothing to export; cut the next version to hold them"
            )));
        }
        let tar = self.ctx.store().get_blob(IMAGE_VAULTS, &vault_key(key, version as u32)).await?;
        let Some(tar) = tar else {
            return Err(self.conflict(&format!("the vault of {key} v{version} is not on this computer any more; cut the next version to take it again")));
        };
        Ok((record, tar))
    }

    /// image.ts:92-130 (I6 to I8).
    pub async fn build(self: &Rc<Self>, o: ImageBuildAsk) -> ImageResult<SealedImageBuilt> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        let name = o.name.clone().unwrap_or_else(|| "default".to_string());
        let force = o.force == Some(true);
        let target = ctx.place_at(&o.place).await?;
        let (place, at) = (target.place, target.at);
        let where_ = ctx.place_name(&place);
        if !builds_images(&at.capabilities()) {
            return Err(refusal(&place_builds_no_image_line(&where_), copy_build_fix::NO_COPY, Some(Kind::Conflict)).into());
        }
        let Some(record) = ctx.record_of(&name).await? else {
            return Err(refusal(&format!("this host owns no image named {name} yet"), copy_build_fix::NO_IMAGE, Some(Kind::Conflict)).into());
        };
        let version = record.version.get();
        if record.recipe.is_none() {
            return Err(refusal(
                &format!("{name} v{version} was sealed before the image record kept the recipe it was built from, so no other place can build it"),
                copy_build_fix::NO_RECIPE,
                Some(Kind::Conflict),
            )
            .into());
        }
        if copy_asks_sign_ins(&record) && !force {
            return Err(refusal(
                &format!("{name} v{version} holds no sign-ins, so a copy at {where_} would ask for every one of them again"),
                copy_build_fix::NO_VAULT,
                Some(Kind::Conflict),
            )
            .into());
        }
        let held = golden_head(ctx.copy_of(&place, &name).await?.as_ref()).cloned();
        if let (false, Some(head)) = (force, &held) {
            if copy_is_current(&record, &copy_at(&place, head, None)) {
                let standing = self.copies_of(&name).await?.into_iter().find(|c| c.place == place);
                return match standing {
                    Some(copy) => Ok(SealedImageBuilt { copy, built: false }),
                    None => Err(HostError::new(format!("{where_} holds {name} v{} and no copy of it was recorded there", head.version.0)).into()),
                };
            }
        }
        let key = copy_key(&place, &name);
        let running = ctx.copy_builds().borrow().get(&key).cloned();
        if let Some(starting) = &o.starting {
            starting();
        }
        if let Some(running) = running {
            return running.await;
        }
        let build = CopyBuild { place: place.clone(), r#where: where_.clone(), at, name: name.clone(), record, signal: o.signal.clone() };
        let area = self.clone();
        let done_ctx = ctx.clone();
        let done_key = key.clone();
        let (frame_place, frame_where, frame_name) = (place, where_, name);
        let run: Joined<SealedImageBuilt> = async move {
            let result = area.build_copy(build).await;
            if let Err(e) = &result {
                if done_ctx.copy_row(&frame_place).is_some_and(|row| !row.stopped) {
                    let why = if done_ctx.place_away(&frame_place) || e.is_place_absent() {
                        Stopped::Line(place_went_away_line(&frame_where))
                    } else {
                        Stopped::Error(e.clone())
                    };
                    done_ctx.frame_stopped(&frame_place, &frame_name, why);
                }
            }
            done_ctx.copy_builds().borrow_mut().shift_remove(&done_key);
            result
        }
        .boxed_local()
        .shared();
        ctx.copy_builds().borrow_mut().insert(key, run.clone());
        run.await
    }

    /// image.ts:132-158 (I9). Never fails.
    pub async fn keep_current(self: &Rc<Self>, place: &str, name: Option<&str>) {
        let ctx = self.ctx.clone();
        let key = name.unwrap_or("default").to_string();
        let before = ctx.copy_row(place);
        if let Err(e) = self.keep_current_steps(place, &key).await {
            let now = ctx.copy_row(place);
            if !same_row(&before, &now) && now.as_ref().is_some_and(|row| row.stopped) {
                return;
            }
            if ctx.row_says_failure(place, &e) {
                ctx.frame_stopped(place, &key, Stopped::Error(e));
            }
        }
    }

    /// image.ts:136-156: the keep, which a failure leaves to `keep_current` to frame.
    async fn keep_current_steps(self: &Rc<Self>, place: &str, key: &str) -> ImageResult<()> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        if ctx.record_of(key).await?.is_none() || ctx.place_away(place) {
            return Ok(());
        }
        let PlaceAt { at, .. } = ctx.place_at(place).await?;
        if !builds_images(&at.capabilities()) {
            return Ok(());
        }
        loop {
            let built = self.build(ImageBuildAsk { place: place.to_string(), name: Some(key.to_string()), force: None, signal: None, starting: None }).await?;
            match ctx.record_of(key).await? {
                Some(record) if !copy_is_current(&record, &built.copy) => {}
                _ => return Ok(()),
            }
        }
    }

    /// image.ts:163-206 (I10, I11).
    pub async fn build_copy(self: &Rc<Self>, o: CopyBuild) -> ImageResult<SealedImageBuilt> {
        let ctx = self.ctx.clone();
        let CopyBuild { place, r#where: where_, at, name, record, signal } = o;
        let provider = ctx.place_kind_of(&place) == PlaceKind::Provider;
        let vault = if provider { None } else { record.vault.clone() };
        let version = record.version.get();
        if vault.as_ref().is_some_and(|v| v.held.is_none()) {
            return Err(self.conflict(&vault_unlisted_refusal(&name, version)).into());
        }
        let tar = match &vault {
            None => None,
            Some(_) => match ctx.store().get_blob(IMAGE_VAULTS, &vault_key(&name, version as u32)).await? {
                Some(tar) => Some(tar),
                None => {
                    return Err(self
                        .conflict(&format!("the vault of {name} v{version} is not on this computer any more; cut the next version to take it again"))
                        .into())
                }
            },
        };
        if let (Some(tar), Some(held)) = (&tar, vault.as_ref().and_then(|v| v.held.as_ref())) {
            if let Err(message) = ctx.roads().refuse_foreign_members("import", tar, held) {
                return Err(self.conflict(&message).into());
            }
        }
        let copy_recipe = ctx.copy_recipe_or_throw()?;
        let recipe = copy_recipe(record.clone(), provider).await?;
        let view = ctx
            .golden_prepare(PrepareAsk { name: Some(name.clone()), place: Some(place.clone()), copy: Some(true), recipe: Some(recipe), signal, ..Default::default() })
            .await?;
        let found = ctx.builders().borrow().get(&view.id).cloned();
        let Some(entry) = found else {
            return Err(HostError::new(format!("the builder {} prepared at {where_} left no record here; nothing was sealed", view.id)).into());
        };
        let stage = ctx.stage_of(&name, Some(&place), true);
        let machine = entry.borrow().builder.machine.clone();
        if let Some(tar) = tar {
            stage(GoldenStage::UploadingFiles, Some(format!("the image's sign-ins, {}", fmt_bytes(tar.len() as f64))), None, None);
            if let Err(e) = ctx.roads().import_image_vault(machine.clone(), tar).await {
                let _ = ctx.roads().kill_until_gone(at.clone(), machine, ctx.kill_confirm()).await;
                ctx.forget_if_gone(&entry, at.clone()).await?;
                stage(GoldenStage::Failed, Some(e.to_string()), None, None);
                return Err(ImageError::Engine(e));
            }
        }
        let prior = golden_head(ctx.copy_of(&place, &name).await?.as_ref()).cloned();
        let sealed = ctx.seal_entry(&entry, false, Some(record.logins.clone()), Some(record.clone())).await?;
        let planned = match &prior {
            Some(prior) if place == ctx.image_place(&name).await? => ctx.copy_recipe_of(&place, &name, prior.version.0 as u32).await?,
            _ => None,
        };
        let sealed_version = sealed.version.version.0 as u32;
        if let Some(digest) = planned {
            ctx.put_copy_recipe(&place, &name, sealed_version, &digest).await?;
        }
        let copy = self.copies_of(&name).await?.into_iter().find(|c| c.place == place);
        match copy {
            Some(copy) => Ok(SealedImageBuilt { copy, built: true }),
            None => Err(HostError::new(format!("{where_} sealed {name} v{sealed_version} and no copy of it was recorded there")).into()),
        }
    }

    /// image.ts:210-216: the image whose head, at the place its own seal stands, is this snapshot.
    pub async fn image_head_named(&self, snapshot_id: &str) -> Result<Option<String>, HostError> {
        let wired = self.ctx.places().wired();
        for raw in self.ctx.store().list(IMAGES).await? {
            let image: SealedImage = decode(raw)?;
            let place = image.place.clone().unwrap_or_else(|| wired.clone());
            let head = golden_head(self.ctx.copy_of(&place, &image.name).await?.as_ref()).map(|h| h.snapshot_id.clone());
            if head.as_deref() == Some(snapshot_id) {
                return Ok(Some(image.name));
            }
        }
        Ok(None)
    }

    /// image.ts:220-227 (I13): the record a version is a copy of.
    pub async fn record_of_version(&self, records: &[SealedImage], version: Option<&GoldenVersion>) -> Result<Option<SealedImage>, HostError> {
        if let Some(version) = version {
            if let Some(built) = records.iter().find(|r| version.image_hash.as_deref() == Some(r.hash.as_str())) {
                return Ok(Some(built.clone()));
            }
            for r in records {
                let place = r.place.clone().unwrap_or_else(|| self.ctx.places().wired());
                let listed = self.ctx.copy_of(&place, &r.name).await?.is_some_and(|m| m.versions.iter().any(|x| x.snapshot_id == version.snapshot_id));
                if listed {
                    return Ok(Some(r.clone()));
                }
            }
        }
        Ok(match records {
            [only] => Some(only.clone()),
            _ => None,
        })
    }

    /// image.ts:232-240 (I13).
    pub async fn refuse_held_sign_ins(&self, snapshot: &str, place: &str) -> Result<(), HostError> {
        let mut records: Vec<SealedImage> = Vec::new();
        for raw in self.ctx.store().list(IMAGES).await? {
            records.push(decode(raw)?);
        }
        if records.is_empty() {
            return Ok(());
        }
        let found = self.ctx.image_of(snapshot).await?;
        let Some(version) = found.version else {
            return Ok(());
        };
        if version.no_secrets == Some(true) {
            return Ok(());
        }
        let record = self.record_of_version(&records, Some(&version)).await?;
        let home = match record {
            None => place.to_string(),
            Some(record) => record.place.unwrap_or_else(|| self.ctx.places().wired()),
        };
        Err(refusal(&image_held_sign_ins_line(&self.ctx.place_name(&home)), &image_rebuild_fix(&home), Some(Kind::Conflict)))
    }

    /// image.ts:247-252 (I12, I13).
    pub async fn copy_for_fork(self: &Rc<Self>, golden: &str, place_id: Option<&str>, announce: Option<Announce>) -> ImageResult<String> {
        let place = match place_id {
            Some(place) => place.to_string(),
            None => self.ctx.places().wired(),
        };
        let snapshot = self.snapshot_for_fork(golden, &place, announce).await?;
        if self.ctx.place_kind_of(&place) == PlaceKind::Provider {
            self.refuse_held_sign_ins(&snapshot, &place).await?;
        }
        Ok(snapshot)
    }

    /// image.ts:254-270 (I12).
    pub async fn snapshot_for_fork(self: &Rc<Self>, golden: &str, place: &str, announce: Option<Announce>) -> ImageResult<String> {
        let ctx = self.ctx.clone();
        let Some(name) = self.image_head_named(golden).await? else {
            return Ok(golden.to_string());
        };
        if place == ctx.image_place(&name).await? {
            return Ok(golden.to_string());
        }
        if announce.is_none() {
            let running = ctx.copy_builds().borrow().get(&copy_key(place, &name)).cloned();
            if let Some(running) = running {
                if let Ok(built) = running.await {
                    return Ok(built.copy.snapshot_id);
                }
            }
        }
        let Some(record) = ctx.record_of(&name).await? else {
            return Ok(golden.to_string());
        };
        let held = golden_head(ctx.copy_of(place, &name).await?.as_ref()).cloned();
        if let Some(head) = &held {
            if copy_is_current(&record, &copy_at(place, head, None)) {
                return Ok(head.snapshot_id.clone());
            }
        }
        let Some(announce) = announce else {
            return Ok(golden.to_string());
        };
        let PlaceAt { at, .. } = ctx.place_at(place).await?;
        if !builds_images(&at.capabilities()) {
            return Ok(golden.to_string());
        }
        let where_ = ctx.place_name(place);
        let priced = at.clone();
        let starting: Rc<dyn Fn()> = Rc::new(move || {
            let rate = priced.pricing().rate_usd_per_hour(priced.pricing().default_size());
            announce(&where_, rate);
        });
        let built = self.build(ImageBuildAsk { place: place.to_string(), name: Some(name), force: None, signal: None, starting: Some(starting) }).await?;
        Ok(built.copy.snapshot_id)
    }

    fn backend_at_place(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        self.ctx.places().backend(place).or_else(|| self.ctx.door().and_then(|d| d.backend_of(place)))
    }
}

fn same_row(a: &Option<Rc<CopyRow>>, b: &Option<Rc<CopyRow>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// A copy as the build lists it, from the head of a place's manifest (image.ts:44-67).
fn copy_at(place: &str, head: &GoldenVersion, size_bytes: Option<f64>) -> SealedImageCopy {
    SealedImageCopy {
        place: place.to_string(),
        version: JsInt(head.version),
        hash: head.image_hash.clone(),
        snapshot_id: head.snapshot_id.clone(),
        template_id: head.template_id.clone(),
        built_at: head.created_at.clone(),
        size_bytes: size_bytes.map(|bytes| JsInt(JsNumber(bytes))),
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, HostError> {
    serde_json::from_value(value).map_err(|e| HostError::new(e.to_string()))
}
